/**
 * A finished compositional run, as a scenario run set stores it.
 *
 * A compositional case goes through the same run-set queue as a black-oil one — same specs, same
 * case identity, same comparison chart — so its result is an ordinary `RunResult` carrying this
 * record beside the (empty) black-oil fields. The record is what the chart stack sources its
 * curves from; the black-oil series of such a result are never read for it.
 *
 * Two derived views live here so that no consumer recomputes them:
 *
 * - `series`, the per-report-step quantities (`runSeries.ts`);
 * - `toSpatialSnapshot`, the per-cell state in the 3D view's vocabulary.
 */

import type { GridState, SimulatorSnapshot, WellState } from '../simulator-types';
import { buildCompositionalRunSeries, type CompositionalRunSeries } from './runSeries';
import type { CompositionalCaseConfig, CompositionalSnapshot } from './types';

export type CompositionalRunStopReason = 'completed' | 'user' | 'failed';

export interface CompositionalRunRecord {
  config: CompositionalCaseConfig;
  /** Report-step snapshots, in order. The first is the initial state. */
  snapshots: CompositionalSnapshot[];
  series: CompositionalRunSeries;
  stop: {
    reason: CompositionalRunStopReason;
    /** Ready to show: `describeCompositionalStop`'s output, with no solver vocabulary. */
    message: string;
  };
}

export function buildCompositionalRunRecord(input: {
  config: CompositionalCaseConfig;
  snapshots: readonly CompositionalSnapshot[];
  stop: CompositionalRunRecord['stop'];
}): CompositionalRunRecord {
  const snapshots = [...input.snapshots];
  return {
    config: input.config,
    snapshots,
    series: buildCompositionalRunSeries(snapshots),
    stop: { ...input.stop },
  };
}

/**
 * The wells as the 3D view draws them.
 *
 * The compositional grid is a 1D column of `cells`, so a completion's cell index is its `i`. A well
 * with an injection stream is an injector; that is the engine's own rule.
 */
export function compositionalWellState(config: CompositionalCaseConfig): WellState {
  return config.wells.flatMap((well) =>
    well.completions.map((completion) => ({
      physical_well_id: well.id,
      i: completion.cell,
      j: 0,
      k: 0,
      injector: well.injection_composition !== undefined,
      ...(well.control === 'bhp' ? { bhp: well.target_bar } : {}),
    })),
  );
}

/**
 * A compositional snapshot in the 3D view's per-cell vocabulary.
 *
 * The case has two hydrocarbon phases and no water, so the vapour saturation is the gas
 * saturation, the liquid takes the rest, and water is zero everywhere. That is a relabelling of the
 * engine's own values, not a model: nothing is recomputed. A cell whose flash did not resolve has
 * no saturation, and it stays not-a-number rather than being filled in.
 */
export function toSpatialSnapshot(
  snapshot: CompositionalSnapshot,
  wells: WellState,
): SimulatorSnapshot {
  const cells = snapshot.pressure.length;
  const satGas = Float64Array.from(snapshot.vapour_saturation, (value) =>
    Number.isFinite(value) ? value : Number.NaN,
  );
  const grid: GridState = {
    pressure: Float64Array.from(snapshot.pressure),
    sat_water: new Float64Array(cells),
    sat_oil: Float64Array.from(satGas, (sv) => (Number.isFinite(sv) ? 1 - sv : Number.NaN)),
    sat_gas: satGas,
  };
  return { time: snapshot.time_days, grid, wells };
}
