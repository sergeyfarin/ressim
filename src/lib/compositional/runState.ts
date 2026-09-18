/**
 * Runtime state for a compositional run.
 *
 * **A plain class, not a runes store, and that is deliberate.** This project's vitest has no Svelte
 * plugin, so a `.svelte.ts` class using `$state` cannot be unit-tested at all — `$state` is simply
 * undefined there. Svelte 5 deep-proxies a plain object assigned to `$state`, so holding an instance
 * of this in a component or store gives reactivity anyway. The logic is what needs testing; the
 * reactivity is the framework's job.
 *
 * It is also **separate from `runtimeStore`, while sharing its worker**. One worker means one WASM
 * instance, which is what we want; but a compositional snapshot has components and phase labels
 * where a black-oil one has water and oil saturations, and folding them together would make every
 * consumer ask which kind it had. `runtimeStore` forwards the three `compositional*` messages here
 * and leaves its own state untouched — C13's exit criterion is that black-oil scenarios are
 * unchanged.
 */

import { compositionalRunQuantities, type CompositionalRunQuantity } from './runQuantities';
import { buildCompositionalRunSeries, type CompositionalRunSeries } from './runSeries';
import type { CompositionalCaseConfig, CompositionalCheckpoint, CompositionalSnapshot } from './types';
import type { WorkerMessage } from '../simulator-types';

export type CompositionalRunStatus = 'idle' | 'running' | 'completed' | 'stopped' | 'failed';

export class CompositionalRunState {
  /** Report-step snapshots, in order. The first is the initial state. */
  snapshots: CompositionalSnapshot[] = [];
  /** The case that produced them, as the engine accepted it. */
  config: CompositionalCaseConfig | null = null;
  status: CompositionalRunStatus = 'idle';
  /** Ready to show: `describeCompositionalStop`'s output, with no solver vocabulary. */
  message = '';
  failure: { kind: string; message: string; timeDays: number } | null = null;
  /** The most recent checkpoint, when one has been requested. */
  checkpoint: CompositionalCheckpoint | null = null;

  /** Derived on read. The runs are tens of report steps, so memoizing would be premature. */
  get series(): CompositionalRunSeries {
    return buildCompositionalRunSeries(this.snapshots);
  }

  get quantities(): CompositionalRunQuantity[] {
    return compositionalRunQuantities(this.series);
  }

  /** True once there is a case to run, whether or not it has stepped. */
  get isConfigured(): boolean {
    return this.config !== null;
  }

  get latest(): CompositionalSnapshot | null {
    return this.snapshots.length === 0 ? null : this.snapshots[this.snapshots.length - 1];
  }

  /**
   * Handle a worker message, returning whether it was ours.
   *
   * Returning a boolean rather than throwing on an unknown type is what lets `runtimeStore` forward
   * every message here first and keep its own handling unchanged.
   */
  handleMessage(message: WorkerMessage): boolean {
    switch (message.type) {
      case 'compositionalState': {
        if (message.config) {
          // A create or restore: a new run, so nothing from the previous one survives.
          this.config = message.config;
          this.snapshots = [message.data];
          this.status = 'idle';
          this.message = '';
          this.failure = null;
        } else {
          this.snapshots = [...this.snapshots, message.data];
        }
        return true;
      }
      case 'compositionalStopped': {
        this.status =
          message.reason === 'failed'
            ? 'failed'
            : message.reason === 'user'
              ? 'stopped'
              : 'completed';
        this.message = message.message;
        this.failure = message.failure ?? null;
        return true;
      }
      case 'compositionalCheckpoint': {
        this.checkpoint = message.checkpoint;
        return true;
      }
      default:
        return false;
    }
  }

  /** Mark a batch as started. The worker does not announce this, so the caller does. */
  markRunning(): void {
    this.status = 'running';
    this.message = '';
    this.failure = null;
  }

  reset(): void {
    this.snapshots = [];
    this.config = null;
    this.status = 'idle';
    this.message = '';
    this.failure = null;
    this.checkpoint = null;
  }
}
