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
 * A completion stores a flat cell index. A 1D column is therefore `i = cell`; a structured box
 * (#47) decodes it with the same row-major `(k*ny + j)*nx + i` order the engine builds. A well with
 * a hydrocarbon stream or `inject_water` is an injector; that is the engine's own rule.
 *
 * `bhp` is the configured pressure a BHP-controlled well asks for. `flowing_bhp`, when a snapshot
 * supplies it, is the pressure the accepted solve actually ran at — for a rate-controlled well
 * those are different quantities, and only the snapshot knows the second.
 */
export function compositionalWellState(
  config: CompositionalCaseConfig,
  snapshot?: CompositionalSnapshot,
): WellState {
  const box = config.grid.box_grid;
  const location = (cell: number) =>
    box
      ? { i: cell % box.nx, j: Math.floor(cell / box.nx) % box.ny, k: Math.floor(cell / (box.nx * box.ny)) }
      : { i: cell, j: 0, k: 0 };
  return config.wells.flatMap((well, wellIndex) => {
    const flowing = snapshot?.well_bhp_bar?.[wellIndex];
    return well.completions.map((completion) => ({
      physical_well_id: well.id,
      ...location(completion.cell),
      injector: well.injection_composition !== undefined || well.inject_water === true,
      ...(well.control === 'bhp' ? { bhp: well.target_bar } : {}),
      ...(flowing !== undefined && Number.isFinite(flowing) ? { flowing_bhp: flowing } : {}),
    }));
  });
}

/**
 * A compositional snapshot in the 3D view's per-cell vocabulary.
 *
 * The vapour saturation is the gas saturation and water is carried when the case has it; oil takes
 * what is left. That is a relabelling of the engine's own values, not a model: nothing is
 * recomputed. A cell whose flash did not resolve has no saturation, and it stays not-a-number
 * rather than being filled in. The overall composition rides along unchanged, so each component
 * can be shown per cell.
 */
export function toSpatialSnapshot(
  snapshot: CompositionalSnapshot,
  wells: WellState,
): SimulatorSnapshot {
  const cells = snapshot.pressure.length;
  const satGas = Float64Array.from(snapshot.vapour_saturation, (value) =>
    Number.isFinite(value) ? value : Number.NaN,
  );
  const satWater = snapshot.water
    ? Float64Array.from(snapshot.water.saturation)
    : new Float64Array(cells);
  const grid: GridState = {
    pressure: Float64Array.from(snapshot.pressure),
    sat_water: satWater,
    sat_oil: Float64Array.from(satGas, (sv, index) =>
      Number.isFinite(sv) ? 1 - sv - satWater[index] : Number.NaN,
    ),
    sat_gas: satGas,
    composition: {
      componentIds: [...snapshot.component_ids],
      values: snapshot.composition.map((values) => Float64Array.from(values)),
    },
  };
  return { time: snapshot.time_days, grid, wells };
}
