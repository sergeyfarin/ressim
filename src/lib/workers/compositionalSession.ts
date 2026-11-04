/**
 * The compositional run loop, as a plain module.
 *
 * The worker keeps the WASM lifecycle and the message plumbing; everything that *decides* anything
 * lives here, behind an interface, so it can be tested without a browser or a `.wasm` file. That is
 * the same split the engine side uses (`compositional/api.rs` decides, `compositional/frontend.rs`
 * converts), and for the same reason: a run loop that can only be exercised through a worker is a
 * run loop that does not get exercised.
 *
 * It does **not** reuse the black-oil `run` loop. That loop owns rate history and `loadState`,
 * which do not mean the same thing here, and C13's exit criterion is that existing black-oil
 * scenarios are unchanged. Termination policy *is* shared, through the neutral rate adapter in
 * `terminationPolicy.ts`, so the two models evaluate the same condition vocabulary.
 */

import type {
  CompositionalCaseConfig,
  CompositionalCheckpoint,
  CompositionalSnapshot,
  CompositionalStepOutcome,
} from '../compositional/types';
import type { ScenarioTerminationPolicy } from '../catalog/scenarios';
import {
  evaluateCompositionalTerminationPolicy,
  type TerminationMatch,
} from './terminationPolicy';

/** What the session needs from a simulator. `CompositionalSimulator` satisfies this. */
export interface CompositionalEngine {
  step(dtDays: number): CompositionalStepOutcome;
  getSnapshot(): CompositionalSnapshot;
  getTime(): number;
  checkpoint(): CompositionalCheckpoint;
  free?(): void;
}

/** How the session obtains one. Injected so a test can supply a fake. */
export interface CompositionalEngineFactory {
  create(config: CompositionalCaseConfig): CompositionalEngine;
  restore(checkpoint: CompositionalCheckpoint): CompositionalEngine;
}

/** Why a batch stopped. */
export type CompositionalRunStop =
  | {
      reason: 'completed';
      completedSteps: number;
      /** Present when a termination policy stopped the batch before its requested horizon. */
      termination?: TerminationMatch;
    }
  | { reason: 'user'; completedSteps: number }
  | { reason: 'failed'; completedSteps: number; failure: CompositionalStepFailureDetail };

export interface CompositionalStepFailureDetail {
  kind: string;
  message: string;
  /** Simulated time when it gave up. */
  timeDays: number;
}

export interface CompositionalRunOptions {
  steps: number;
  dtDays: number;
  /**
   * Report a snapshot every `snapshotEvery` accepted steps, and always on the last one. Defaults
   * to 1. Raising it is how a long run avoids posting a snapshot per step; the engine's snapshot
   * is small but it is not free.
   */
  snapshotEvery?: number;
  /**
   * Evaluate this policy after every report step and end the batch at the first match. The stop
   * is a normal `completed` result carrying the matched condition, not a solver failure.
   */
  terminationPolicy?: ScenarioTerminationPolicy | null;
}

/** A configured compositional run. */
export class CompositionalSession {
  #engine: CompositionalEngine;
  #config: CompositionalCaseConfig;
  #stopRequested = false;

  private constructor(engine: CompositionalEngine, config: CompositionalCaseConfig) {
    this.#engine = engine;
    this.#config = config;
  }

  static create(
    factory: CompositionalEngineFactory,
    config: CompositionalCaseConfig,
  ): CompositionalSession {
    return new CompositionalSession(factory.create(config), config);
  }

  static restore(
    factory: CompositionalEngineFactory,
    checkpoint: CompositionalCheckpoint,
  ): CompositionalSession {
    return new CompositionalSession(factory.restore(checkpoint), checkpoint.config);
  }

  get config(): CompositionalCaseConfig {
    return this.#config;
  }

  get timeDays(): number {
    return this.#engine.getTime();
  }

  snapshot(): CompositionalSnapshot {
    return this.#engine.getSnapshot();
  }

  checkpoint(): CompositionalCheckpoint {
    return this.#engine.checkpoint();
  }

  /** Ask the current batch to stop after the step in flight. */
  requestStop(): void {
    this.#stopRequested = true;
  }

  dispose(): void {
    this.#engine.free?.();
  }

  /**
   * Run a batch, reporting snapshots as it goes.
   *
   * A report step is `dtDays` of **simulated time**, not one engine call. The engine's `step(dt)`
   * may cut `dt` and accept the smaller step — it reports what it accepted and returns — so one
   * call does not always reach the report time. Counting calls as report steps left every snapshot
   * after the first cut early by the cut's remainder, and a refined grid that cuts often ended its
   * run days short of the horizon (40 cells over 400 × 0.05 d stopped at 16.6 d, not 20). The loop
   * below steps on the remainder until the report time is reached.
   *
   * A failed step **ends the batch and is reported**, rather than being retried here: the engine's
   * timestep lifecycle already cuts and retries internally, so a failure that reaches this point
   * means it exhausted that. Continuing past it would be advancing a run whose last accepted state
   * is not what the caller thinks it is.
   *
   * `yieldControl` is awaited after every report step. The worker passes one that occasionally
   * yields to its event loop, which is the only way a stop request can arrive mid-batch.
   */
  async run(
    options: CompositionalRunOptions,
    onSnapshot: (snapshot: CompositionalSnapshot, stepIndex: number) => void,
    yieldControl?: () => Promise<void>,
  ): Promise<CompositionalRunStop> {
    const snapshotEvery = Math.max(1, Math.floor(options.snapshotEvery ?? 1));
    // Below this, the remainder is round-off in the clock, not time still to simulate.
    const tolerance = options.dtDays * 1e-9;
    const startDays = this.#engine.getTime();
    this.#stopRequested = false;

    for (let step = 0; step < options.steps; step += 1) {
      if (this.#stopRequested) {
        onSnapshot(this.#engine.getSnapshot(), step);
        return { reason: 'user', completedSteps: step };
      }

      const targetDays = startDays + (step + 1) * options.dtDays;
      let remaining = targetDays - this.#engine.getTime();
      while (remaining > tolerance) {
        const outcome = this.#engine.step(remaining);
        if (outcome.accepted_dt_days === null) {
          const failure = outcome.failure ?? { kind: 'Unknown', message: 'the step did not succeed' };
          // Report the last accepted state, so a UI shows where it actually got to.
          onSnapshot(this.#engine.getSnapshot(), step);
          return {
            reason: 'failed',
            completedSteps: step,
            failure: { ...failure, timeDays: outcome.time_days },
          };
        }
        remaining = targetDays - this.#engine.getTime();
      }

      const isLast = step === options.steps - 1;
      const isReportStep = isLast || (step + 1) % snapshotEvery === 0;
      // A termination check needs the surface rates, which only the snapshot carries. Fetch it
      // once and reuse it, so a matching step is reported even when `snapshotEvery` would skip it.
      // The engine exposes no cheaper well-surface accessor today; a large case with a policy pays
      // one snapshot marshalling per report step.
      const snapshot = isReportStep || options.terminationPolicy
        ? this.#engine.getSnapshot()
        : null;
      const termination = snapshot
        ? evaluateCompositionalTerminationPolicy(options.terminationPolicy, snapshot)
        : null;
      if (snapshot && (isReportStep || termination)) {
        onSnapshot(snapshot, step);
      }
      if (termination) {
        // A match on the requested final step is not an early stop: the run reached its horizon,
        // so reporting it as a termination would tell the user it was cut short.
        return isLast
          ? { reason: 'completed', completedSteps: step + 1 }
          : { reason: 'completed', completedSteps: step + 1, termination };
      }
      await yieldControl?.();
    }

    return { reason: 'completed', completedSteps: options.steps };
  }
}

/**
 * The stop fields the worker posts, without the profile that only the worker can measure.
 *
 * Kept here rather than inline in `sim.worker.ts` so the payload — especially the early-stop
 * summary — can be tested without a browser.
 */
export function compositionalStopMessage(stop: CompositionalRunStop): {
  reason: CompositionalRunStop['reason'];
  completedSteps: number;
  message: string;
  terminationSummary?: string;
  failure?: CompositionalStepFailureDetail;
} {
  return {
    reason: stop.reason,
    completedSteps: stop.completedSteps,
    message: describeCompositionalStop(stop),
    ...(stop.reason === 'completed' && stop.termination
      ? { terminationSummary: stop.termination.summary }
      : {}),
    ...(stop.reason === 'failed' ? { failure: stop.failure } : {}),
  };
}

/**
 * Turn a stop into something worth showing someone, without implementation jargon.
 *
 * The engine's failure kinds are the vocabulary of a Newton solve, not of a reservoir. C13 asks for
 * actionable messages, so each one is translated into what it means for the case.
 */
export function describeCompositionalStop(stop: CompositionalRunStop): string {
  if (stop.reason === 'completed') {
    return stop.termination
      ? `Finished ${stop.completedSteps} steps early: ${stop.termination.summary}`
      : `Finished ${stop.completedSteps} steps.`;
  }
  if (stop.reason === 'user') {
    return `Stopped after ${stop.completedSteps} steps.`;
  }
  const at = `after ${stop.completedSteps} steps (${stop.failure.timeDays.toFixed(3)} days)`;
  switch (stop.failure.kind) {
    case 'Flash':
      return `The fluid description could not be resolved ${at}. The run reached conditions outside the range this fluid has been validated over.`;
    case 'Admissibility':
      return `A cell reached a state that is not physical ${at} — a negative pressure or component amount. Try a smaller timestep.`;
    case 'Nonlinear':
      return `The solver could not converge ${at}. Try a smaller timestep.`;
    case 'Linear':
      return `The linear solve failed ${at}.`;
    case 'Budget':
      return `The timestep was cut as far as it is allowed to go and still did not converge ${at}.`;
    default:
      return `The run stopped ${at}: ${stop.failure.message}`;
  }
}
