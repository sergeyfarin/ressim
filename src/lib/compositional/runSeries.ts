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
