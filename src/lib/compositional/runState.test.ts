import { describe, expect, it } from 'vitest';

import { COMPOSITIONAL_CASE_SCHEMA, COMPOSITIONAL_CHECKPOINT_SCHEMA } from './types';
import type { CompositionalCaseConfig, CompositionalSnapshot } from './types';
import type { WorkerMessage } from '../simulator-types';
import { CompositionalRunState } from './runState';

function config(): CompositionalCaseConfig {
  return {
    schema: COMPOSITIONAL_CASE_SCHEMA,
    fluid: 'pinned-ternary',
    grid: {
      cells: 2,
      dx_m: 60,
      dy_m: 6,
      dz_m: 6,
      porosity: 0.1,
      permeability_md: 100,
      rock_reference_pressure_bar: 1,
      rock_compressibility_per_bar: 0,
    },
    relperm: { model: 'linear' },
    initial_pressure_bar: 75,
    initial_composition: [0.1, 0.3, 0.6],
    wells: [],
    gravity_enabled: false,
  };
}

function snapshot(time: number, inventory = [100, 300, 600]): CompositionalSnapshot {
  return {
    time_days: time,
    component_ids: ['CO2', 'C1', 'C10'],
    pressure: [75, 75],
    composition: [
      [0.1, 0.1],
      [0.3, 0.3],
      [0.6, 0.6],
    ],
    phase_state: ['two-phase', 'two-phase'],
    vapour_saturation: [0.3, 0.3],
    inventory,
    cumulative_well_moles: [],
  };
}

describe('CompositionalRunState', () => {
  it('starts idle and unconfigured', () => {
    const store = new CompositionalRunState();
    expect(store.isConfigured).toBe(false);
    expect(store.status).toBe('idle');
    expect(store.latest).toBeNull();
  });

  it('takes a create message as the start of a new run', () => {
    const store = new CompositionalRunState();
    const consumed = store.handleMessage({
      type: 'compositionalState',
      data: snapshot(0),
      config: config(),
    } as WorkerMessage);

    expect(consumed).toBe(true);
    expect(store.isConfigured).toBe(true);
    expect(store.snapshots).toHaveLength(1);
    expect(store.series.timeDays).toEqual([0]);
  });

  it('appends step snapshots', () => {
    const store = new CompositionalRunState();
    store.handleMessage({ type: 'compositionalState', data: snapshot(0), config: config() } as WorkerMessage);
    store.handleMessage({ type: 'compositionalState', data: snapshot(1) } as WorkerMessage);
    store.handleMessage({ type: 'compositionalState', data: snapshot(2) } as WorkerMessage);

    expect(store.snapshots).toHaveLength(3);
    expect(store.latest?.time_days).toBe(2);
    expect(store.series.timeDays).toEqual([0, 1, 2]);
  });

  it('discards the previous run when a new case is created', () => {
    // A create carries a config; that is how a second run is told apart from a continuation.
    const store = new CompositionalRunState();
    store.handleMessage({ type: 'compositionalState', data: snapshot(0), config: config() } as WorkerMessage);
    store.handleMessage({ type: 'compositionalState', data: snapshot(1) } as WorkerMessage);
    store.handleMessage({ type: 'compositionalState', data: snapshot(0), config: config() } as WorkerMessage);

    expect(store.snapshots).toHaveLength(1);
    expect(store.series.timeDays).toEqual([0]);
  });

  it('records a failure with a message a person can read', () => {
    const store = new CompositionalRunState();
    store.markRunning();
    expect(store.status).toBe('running');

    store.handleMessage({
      type: 'compositionalStopped',
      reason: 'failed',
      completedSteps: 3,
      message: 'The fluid description could not be resolved after 3 steps (1.500 days).',
      failure: { kind: 'Flash', message: 'raw', timeDays: 1.5 },
      profile: { batchMs: 1, avgStepMs: 1, snapshotsSent: 3 },
    } as WorkerMessage);

    expect(store.status).toBe('failed');
    expect(store.failure?.kind).toBe('Flash');
    expect(store.message).not.toMatch(/newton|jacobian/i);
  });

  it('maps each stop reason to a status', () => {
    const cases: Array<['completed' | 'user' | 'failed', string]> = [
      ['completed', 'completed'],
      ['user', 'stopped'],
      ['failed', 'failed'],
    ];
    for (const [reason, expected] of cases) {
      const store = new CompositionalRunState();
      store.handleMessage({
        type: 'compositionalStopped',
        reason,
        completedSteps: 1,
        message: '',
        profile: { batchMs: 1, avgStepMs: 1, snapshotsSent: 1 },
      } as WorkerMessage);
      expect(store.status).toBe(expected);
    }
  });

  it('keeps a checkpoint when one arrives', () => {
    const store = new CompositionalRunState();
    store.handleMessage({
      type: 'compositionalCheckpoint',
      checkpoint: {
        schema: COMPOSITIONAL_CHECKPOINT_SCHEMA,
        config: config(),
        time_days: 2,
        pressure: [75, 75],
        composition: [
          [0.1, 0.3, 0.6],
          [0.1, 0.3, 0.6],
        ],
      },
    } as WorkerMessage);
    expect(store.checkpoint?.time_days).toBe(2);
  });

  it('leaves black-oil messages alone', () => {
    // This is what lets runtimeStore forward everything here first without changing its own
    // handling: anything not ours is declined, not swallowed.
    const store = new CompositionalRunState();
    for (const type of ['state', 'ready', 'stopped', 'batchComplete', 'error'] as const) {
      expect(store.handleMessage({ type } as WorkerMessage)).toBe(false);
    }
    expect(store.snapshots).toHaveLength(0);
    expect(store.status).toBe('idle');
  });

  it('derives quantities that follow the run', () => {
    const store = new CompositionalRunState();
    store.handleMessage({ type: 'compositionalState', data: snapshot(0, [100, 300, 600]), config: config() } as WorkerMessage);
    store.handleMessage({ type: 'compositionalState', data: snapshot(1, [150, 290, 580]) } as WorkerMessage);

    const co2 = store.quantities.find((q) => q.id === 'comp-inventory-CO2');
    expect(co2?.source(store.series)).toEqual([100, 150]);
  });

  it('resets', () => {
    const store = new CompositionalRunState();
    store.handleMessage({ type: 'compositionalState', data: snapshot(0), config: config() } as WorkerMessage);
    store.reset();
    expect(store.isConfigured).toBe(false);
    expect(store.snapshots).toHaveLength(0);
  });
});
