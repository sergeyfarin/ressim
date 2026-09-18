/**
 * The compositional run loop, as a plain module.
 *
 * The worker keeps the WASM lifecycle and the message plumbing; everything that *decides* anything
 * lives here, behind an interface, so it can be tested without a browser or a `.wasm` file. That is
 * the same split the engine side uses (`compositional/api.rs` decides, `compositional/frontend.rs`
 * converts), and for the same reason: a run loop that can only be exercised through a worker is a
 * run loop that does not get exercised.
 *
 * It does **not** reuse the black-oil `run` loop. That loop owns rate history, the termination
 * policy and `loadState`, none of which mean the same thing here, and C13's exit criterion is that
 * existing black-oil scenarios are unchanged.
 */

import type {
  CompositionalCaseConfig,
  CompositionalCheckpoint,
  CompositionalSnapshot,
  CompositionalStepOutcome,
} from '../compositional/types';

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
  | { reason: 'completed'; completedSteps: number }
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
   * A failed step **ends the batch and is reported**, rather than being retried here: the engine's
   * timestep lifecycle already cuts and retries internally, so a failure that reaches this point
   * means it exhausted that. Continuing past it would be advancing a run whose last accepted state
   * is not what the caller thinks it is.
   */
  run(
    options: CompositionalRunOptions,
    onSnapshot: (snapshot: CompositionalSnapshot, stepIndex: number) => void,
  ): CompositionalRunStop {
    const snapshotEvery = Math.max(1, Math.floor(options.snapshotEvery ?? 1));
    this.#stopRequested = false;

    for (let step = 0; step < options.steps; step += 1) {
      if (this.#stopRequested) {
        onSnapshot(this.#engine.getSnapshot(), step);
        return { reason: 'user', completedSteps: step };
      }

      const outcome = this.#engine.step(options.dtDays);
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

      const isLast = step === options.steps - 1;
      if (isLast || (step + 1) % snapshotEvery === 0) {
        onSnapshot(this.#engine.getSnapshot(), step);
      }
    }

    return { reason: 'completed', completedSteps: options.steps };
  }
}

/**
 * Turn a stop into something worth showing someone, without implementation jargon.
 *
 * The engine's failure kinds are the vocabulary of a Newton solve, not of a reservoir. C13 asks for
 * actionable messages, so each one is translated into what it means for the case.
 */
export function describeCompositionalStop(stop: CompositionalRunStop): string {
  if (stop.reason === 'completed') {
    return `Finished ${stop.completedSteps} steps.`;
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
