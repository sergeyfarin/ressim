/**
 * Compositional run series — the per-report-step quantities a chart can plot.
 *
 * **Why this is not `DerivedRunSeries`.** That type is a closed struct of black-oil named fields:
 * `oilRate`, `waterCut`, `gor`. A compositional run has none of those and has something that type
 * cannot express — a *variable number* of per-component series. Widening it would mean every
 * black-oil consumer carrying nullable compositional fields, and it would make it possible for a
 * compositional series to reach a black-oil analytical overlay, which the execution plan forbids
 * by name: *"do not silently feed compositional output into black-oil GOR/material-balance
 * analytical curves"*. Keeping the types apart makes that structurally impossible rather than a
 * rule someone has to remember.
 *
 * Everything here is derived from the snapshots the engine already posts. Nothing is recomputed
 * from physics, because the engine owns that and a second implementation would be a second thing
 * to keep right.
 */

import type { CompositionalSnapshot } from './types';

/** One run's worth of compositional series, in report-step order. */
export interface CompositionalRunSeries {
  /** Component identifiers, in the engine's order. Everything per-component is indexed by this. */
  componentIds: string[];
  timeDays: number[];
  /** Mean cell pressure [bar]. */
  averagePressure: Array<number | null>;
  /** Pore-volume-weighted mean vapour saturation, or `null` where a cell would not flash. */
  averageVapourSaturation: Array<number | null>;
  /** Fraction of cells the flash reports as two-phase. */
  twoPhaseCellFraction: Array<number | null>;
  /** `componentInventory[component][step]` — total moles held by the grid. */
  componentInventory: Array<Array<number | null>>;
  /** `componentNetWellMoles[component][step]` — cumulative, positive into the reservoir. */
  componentNetWellMoles: Array<Array<number | null>>;
  /**
   * Produced separator-liquid rate [m³/day], summed over producing wells at each report.
   *
   * **Not a black-oil oil rate.** The engine's surface separation is a compositional flash under
   * the specification's own surface conditions, not a `Bo` table. `null` means the snapshot did not
   * report surface rates — never that the rate was zero.
   */
  surfaceLiquidRate: Array<number | null>;
  /** Produced separator-vapour rate [m³/day], summed over producing wells. */
  surfaceVapourRate: Array<number | null>;
  /**
   * Separator gas/liquid ratio at the surface [m³/m³], or `null` when no separator liquid is
   * produced. Deliberately not called GOR: a black-oil GOR is a property of a different fluid
   * model and separator convention.
   */
  separatorGasLiquidRatio: Array<number | null>;
  /** Produced water cut at the surface, water / (liquid + water), or `null` with no stream. */
  surfaceWaterCut: Array<number | null>;
  /** Cumulative produced separator-liquid volume [m³]. */
  cumulativeSurfaceLiquid: Array<number | null>;
  /** Cumulative produced separator-vapour volume [m³]. */
  cumulativeSurfaceVapour: Array<number | null>;
  /** Cumulative produced surface water volume [m³], or `null` when the case has no water. */
  cumulativeSurfaceWater: Array<number | null>;
  /**
   * Relative conservation error at each step: how far `inventory(t) - inventory(0)` is from the
   * net moles the wells moved. This is the diagnostic the execution plan asks C13 to surface —
   * a run that stops conserving should be visible in the UI, not only in a Rust test.
   */
  conservationResidual: Array<number | null>;
}

const EMPTY: CompositionalRunSeries = {
  componentIds: [],
  timeDays: [],
  averagePressure: [],
  averageVapourSaturation: [],
  twoPhaseCellFraction: [],
  componentInventory: [],
  componentNetWellMoles: [],
  surfaceLiquidRate: [],
  surfaceVapourRate: [],
  separatorGasLiquidRatio: [],
  surfaceWaterCut: [],
  cumulativeSurfaceLiquid: [],
  cumulativeSurfaceVapour: [],
  cumulativeSurfaceWater: [],
  conservationResidual: [],
};

function mean(values: number[]): number | null {
  const finite = values.filter((value) => Number.isFinite(value));
  return finite.length === 0 ? null : finite.reduce((a, b) => a + b, 0) / finite.length;
}

/**
 * Derive the series from a run's snapshots.
 *
 * Snapshots are expected in report order. A snapshot whose component set differs from the first
 * one's is **dropped**, not merged: that can only happen if two runs' snapshots were mixed, and
 * silently interleaving them would produce a chart of two different cases.
 */
export function buildCompositionalRunSeries(
  snapshots: readonly CompositionalSnapshot[],
): CompositionalRunSeries {
  if (snapshots.length === 0) {
    return { ...EMPTY };
  }

  const componentIds = [...snapshots[0].component_ids];
  const usable = snapshots.filter(
    (snapshot) =>
      snapshot.component_ids.length === componentIds.length &&
      snapshot.component_ids.every((id, index) => id === componentIds[index]),
  );
  if (usable.length === 0) {
    return { ...EMPTY, componentIds };
  }

  const n = componentIds.length;
  const series: CompositionalRunSeries = {
    componentIds,
    timeDays: usable.map((s) => s.time_days),
    averagePressure: usable.map((s) => mean(s.pressure)),
    averageVapourSaturation: usable.map((s) => mean(s.vapour_saturation)),
    twoPhaseCellFraction: usable.map((s) =>
      s.phase_state.length === 0
        ? null
        : s.phase_state.filter((state) => state === 'two-phase').length / s.phase_state.length,
    ),
    componentInventory: Array.from({ length: n }, (_, i) =>
      usable.map((s) => (i < s.inventory.length ? s.inventory[i] : null)),
    ),
    componentNetWellMoles: Array.from({ length: n }, (_, i) =>
      usable.map((s) => netWellMoles(s, i)),
    ),
    surfaceLiquidRate: usable.map((s) => producedSurface(s)?.liquid ?? null),
    surfaceVapourRate: usable.map((s) => producedSurface(s)?.vapour ?? null),
    separatorGasLiquidRatio: usable.map((s) => {
      const produced = producedSurface(s);
      if (!produced || !(produced.liquid > 0)) return null;
      return produced.vapour / produced.liquid;
    }),
    surfaceWaterCut: usable.map((s) => {
      const produced = producedSurface(s);
      if (!produced || produced.water === null) return null;
      const total = produced.liquid + produced.water;
      return total > 0 ? produced.water / total : null;
    }),
    cumulativeSurfaceLiquid: usable.map((s) => producedSurfaceCumulative(s)?.liquid ?? null),
    cumulativeSurfaceVapour: usable.map((s) => producedSurfaceCumulative(s)?.vapour ?? null),
    cumulativeSurfaceWater: usable.map((s) => producedSurfaceCumulative(s)?.water ?? null),
    conservationResidual: [],
  };

  const initialInventory = usable[0].inventory;
  series.conservationResidual = usable.map((snapshot) => {
    let worst: number | null = null;
    for (let i = 0; i < n; i += 1) {
      const held = snapshot.inventory[i];
      const start = initialInventory[i];
      const moved = netWellMoles(snapshot, i);
      if (!Number.isFinite(held) || !Number.isFinite(start) || moved === null) {
        continue;
      }
      const expected = start + moved;
      // Scaled by what the cell holds, so a trace component's tiny absolute error does not
      // dominate — the same reasoning as C8's per-component row scaling in the engine.
      const scale = Math.max(Math.abs(held), Math.abs(start), 1);
      const relative = Math.abs(held - expected) / scale;
      worst = worst === null ? relative : Math.max(worst, relative);
    }
    return worst;
  });

  return series;
}

type ProducedSurface = {
  liquid: number;
  vapour: number;
  /** `null` when the case has no water phase. */
  water: number | null;
};

function produced(value: number | undefined): number {
  return Number.isFinite(value) ? Math.max(0, -(value as number)) : 0;
}

function nonnegative(value: number | undefined): number {
  return Number.isFinite(value) ? Math.max(0, value as number) : 0;
}

/**
 * Sum the **producing** side of each well's surface ledger.
 *
 * Per-well rates are signed, so summing them directly would let a gas injector cancel a gas
 * producer. The charted quantities are production, so each well contributes only its negative
 * side, made positive. A snapshot without the field returns `null` rather than a zero that would
 * claim the run produced nothing.
 */
function producedSurface(snapshot: CompositionalSnapshot): ProducedSurface | null {
  const wells = snapshot.well_surface_rates;
  if (wells === undefined) return null;
  let liquid = 0;
  let vapour = 0;
  let water = 0;
  let hasWater = false;
  for (const well of wells) {
    liquid += produced(well.liquid_m3_per_day);
    vapour += produced(well.vapour_m3_per_day);
    if (well.water_m3_per_day !== undefined) {
      hasWater = true;
      water += produced(well.water_m3_per_day);
    }
  }
  return { liquid, vapour, water: hasWater ? water : null };
}

function producedSurfaceCumulative(snapshot: CompositionalSnapshot): ProducedSurface | null {
  // Gross production, not the signed net balance. A well can reverse direction, so clipping the
  // net would let a later injection reduce a cumulative that already happened.
  const wells = snapshot.cumulative_well_surface_production_volumes;
  if (wells === undefined) return null;
  let liquid = 0;
  let vapour = 0;
  let water = 0;
  let hasWater = false;
  for (const well of wells) {
    liquid += nonnegative(well.liquid_m3);
    vapour += nonnegative(well.vapour_m3);
    if (well.water_m3 !== undefined) {
      hasWater = true;
      water += nonnegative(well.water_m3);
    }
  }
  return { liquid, vapour, water: hasWater ? water : null };
}

function netWellMoles(snapshot: CompositionalSnapshot, component: number): number | null {
  if (snapshot.cumulative_well_moles.length === 0) {
    return 0;
  }
  let total = 0;
  for (const well of snapshot.cumulative_well_moles) {
    if (component >= well.length || !Number.isFinite(well[component])) {
      return null;
    }
    total += well[component];
  }
  return total;
}
