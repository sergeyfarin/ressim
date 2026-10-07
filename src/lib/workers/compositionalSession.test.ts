import { describe, expect, it, vi } from 'vitest';

import { COMPOSITIONAL_CASE_SCHEMA, COMPOSITIONAL_CHECKPOINT_SCHEMA } from '../compositional/types';
import type {
  CompositionalCaseConfig,
  CompositionalCheckpoint,
  CompositionalSnapshot,
  CompositionalStepOutcome,
} from '../compositional/types';
import {
  CompositionalSession,
  describeCompositionalStop,
  type CompositionalEngine,
  type CompositionalEngineFactory,
} from './compositionalSession';

function config(): CompositionalCaseConfig {
  return {
    schema: COMPOSITIONAL_CASE_SCHEMA,
    fluid: 'pinned-ternary',
    grid: {
      cells: 3,
      dx_m: 60,
      dy_m: 6,
      dz_m: 6,
      porosity: 0.1,
      permeability_md: 100,
      rock_reference_pressure_bar: 68.9476,
      rock_compressibility_per_bar: 0,
    },
    relperm: { model: 'linear' },
    initial_pressure_bar: 75,
    initial_composition: [0.1, 0.3, 0.6],
    wells: [],
    gravity_enabled: false,
  };
}

/**
 * An engine that advances a clock and can be told to fail at a given step, or to accept at most
 * `maxDtDays` of any step it is asked for — which is what the real engine does after a cut.
 */
function fakeEngine(
  options: { failAtStep?: number; failKind?: string; maxDtDays?: number } = {},
): CompositionalEngine {
  let time = 0;
  let step = 0;
  const snapshot = (): CompositionalSnapshot => ({
    time_days: time,
    component_ids: ['CO2', 'C1', 'C10'],
    pressure: [75, 75, 75],
    composition: [
      [0.1, 0.1, 0.1],
      [0.3, 0.3, 0.3],
      [0.6, 0.6, 0.6],
    ],
    phase_state: ['two-phase', 'two-phase', 'two-phase'],
    vapour_saturation: [0.3, 0.3, 0.3],
    inventory: [1, 3, 6],
    cumulative_well_moles: [],
  });
  return {
    step(dtDays: number): CompositionalStepOutcome {
      step += 1;
      if (options.failAtStep !== undefined && step > options.failAtStep) {
        return {
          accepted_dt_days: null,
          time_days: time,
          attempts: 6,
          newton_iterations: 30,
          failure: { kind: options.failKind ?? 'Flash', message: 'the flash did not resolve' },
        };
      }
      const accepted = Math.min(dtDays, options.maxDtDays ?? Infinity);
      time += accepted;
      return {
        accepted_dt_days: accepted,
        time_days: time,
        attempts: 1,
        newton_iterations: 3,
        failure: null,
      };
    },
    getSnapshot: snapshot,
    getTime: () => time,
    checkpoint: (): CompositionalCheckpoint => ({
      schema: COMPOSITIONAL_CHECKPOINT_SCHEMA,
      config: config(),
      time_days: time,
      pressure: [75, 75, 75],
      composition: [
        [0.1, 0.3, 0.6],
        [0.1, 0.3, 0.6],
        [0.1, 0.3, 0.6],
      ],
    }),
  };
}

function factory(engine: CompositionalEngine): CompositionalEngineFactory {
  return { create: () => engine, restore: () => engine };
}

describe('CompositionalSession', () => {
  it('runs a batch and reports a snapshot per step by default', async () => {
    const session = CompositionalSession.create(factory(fakeEngine()), config());
    const seen: number[] = [];
    const stop = await session.run({ steps: 4, dtDays: 0.5 }, (snapshot) => seen.push(snapshot.time_days));

    expect(stop).toEqual({ reason: 'completed', completedSteps: 4 });
    expect(seen).toEqual([0.5, 1, 1.5, 2]);
    expect(session.timeDays).toBe(2);
  });

  it('honours snapshotEvery, and always reports the final step', async () => {
    const session = CompositionalSession.create(factory(fakeEngine()), config());
    const seen: number[] = [];
    await session.run({ steps: 5, dtDays: 1, snapshotEvery: 2 }, (snapshot) =>
      seen.push(snapshot.time_days),
    );
    // Steps 2 and 4 by the interval, and step 5 because it is the last.
    expect(seen).toEqual([2, 4, 5]);
  });

  it('ends the batch on a failed step and reports where it got to', async () => {
    const session = CompositionalSession.create(factory(fakeEngine({ failAtStep: 2 })), config());
    const seen: number[] = [];
    const stop = await session.run({ steps: 10, dtDays: 0.5 }, (snapshot) =>
      seen.push(snapshot.time_days),
    );

    expect(stop.reason).toBe('failed');
    expect(stop.completedSteps).toBe(2);
    if (stop.reason === 'failed') {
      expect(stop.failure.kind).toBe('Flash');
      expect(stop.failure.timeDays).toBe(1);
    }
    // The last accepted state is still reported, so a UI shows where the run actually reached.
    expect(seen[seen.length - 1]).toBe(1);
  });

  it('does not advance past a failure', async () => {
    const engine = fakeEngine({ failAtStep: 1 });
    const stepSpy = vi.spyOn(engine, 'step');
    const session = CompositionalSession.create(factory(engine), config());
    await session.run({ steps: 20, dtDays: 0.5 }, () => {});
    // One good step, one that fails, and then it stops asking.
    expect(stepSpy).toHaveBeenCalledTimes(2);
  });

  it('stops when asked, before taking the next step', async () => {
    const engine = fakeEngine();
    const session = CompositionalSession.create(factory(engine), config());
    let calls = 0;
    const stop = await session.run({ steps: 10, dtDays: 1 }, () => {
      calls += 1;
      if (calls === 2) {
        session.requestStop();
      }
    });
    expect(stop.reason).toBe('user');
    expect(stop.completedSteps).toBe(2);
  });

  it('reaches every report time when the engine accepts a cut step', async () => {
    // The engine accepts at most 0.2 d of each request, as it does after cutting a step. A report
    // step is simulated time, so each one takes several calls and still lands on its time.
    const engine = fakeEngine({ maxDtDays: 0.2 });
    const stepSpy = vi.spyOn(engine, 'step');
    const session = CompositionalSession.create(factory(engine), config());
    const seen: number[] = [];
    const stop = await session.run({ steps: 4, dtDays: 0.5 }, (snapshot) => seen.push(snapshot.time_days));

    expect(stop).toEqual({ reason: 'completed', completedSteps: 4 });
    expect(seen.map((t) => Number(t.toFixed(9)))).toEqual([0.5, 1, 1.5, 2]);
    expect(session.timeDays).toBeCloseTo(2, 12);
    // The remainder is requested, not a fresh full step: 0.2 + 0.2 + 0.1 per report step.
    expect(stepSpy.mock.calls.slice(0, 3).map(([dt]) => Number(dt.toFixed(9)))).toEqual([0.5, 0.3, 0.1]);
  });

  it('continues from the engine clock, so a second batch reports at its own times', async () => {
    const session = CompositionalSession.create(factory(fakeEngine()), config());
    await session.run({ steps: 2, dtDays: 1 }, () => {});
    const seen: number[] = [];
    await session.run({ steps: 2, dtDays: 1 }, (snapshot) => seen.push(snapshot.time_days));
    expect(seen).toEqual([3, 4]);
  });

  it('yields after every report step, so a stop request can arrive mid-batch', async () => {
    const session = CompositionalSession.create(factory(fakeEngine()), config());
    let yields = 0;
    const stop = await session.run({ steps: 10, dtDays: 1 }, () => {}, async () => {
      yields += 1;
      // What the worker's message handler does when the stop message lands during a yield.
      if (yields === 3) session.requestStop();
    });
    expect(stop).toEqual({ reason: 'user', completedSteps: 3 });
  });

  it('restores from a checkpoint and keeps its config', () => {
    const engine = fakeEngine();
    const checkpoint = engine.checkpoint();
    const session = CompositionalSession.restore(factory(engine), checkpoint);
    expect(session.config).toEqual(checkpoint.config);
  });
});

describe('describeCompositionalStop', () => {
  it('translates each failure kind into something actionable', () => {
    const cases: Array<[string, RegExp]> = [
      ['Flash', /fluid description could not be resolved/i],
      ['Admissibility', /not physical/i],
      ['Nonlinear', /could not converge/i],
      ['Budget', /cut as far as it is allowed/i],
    ];
    for (const [kind, expected] of cases) {
      const message = describeCompositionalStop({
        reason: 'failed',
        completedSteps: 3,
        failure: { kind, message: 'raw', timeDays: 1.5 },
      });
      expect(message).toMatch(expected);
      // No solver vocabulary leaks through.
      expect(message).not.toMatch(/jacobian|residual|newton/i);
    }
  });

  it('says how far a run got, in days', () => {
    const message = describeCompositionalStop({
      reason: 'failed',
      completedSteps: 3,
      failure: { kind: 'Flash', message: 'raw', timeDays: 1.5 },
    });
    expect(message).toContain('1.500 days');
  });

  it('reports a completed and a user-stopped batch plainly', () => {
    expect(describeCompositionalStop({ reason: 'completed', completedSteps: 7 })).toContain('7');
    expect(describeCompositionalStop({ reason: 'user', completedSteps: 2 })).toMatch(/stopped/i);
  });
});
