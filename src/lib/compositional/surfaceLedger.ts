/**
 * Surface production totals from a compositional snapshot's per-well ledger.
 *
 * This is a leaf module on purpose: the chart series and the termination evaluator both need the
 * same sign convention, and neither should have to import the other's module to get it.
 */

import type { CompositionalSnapshot } from './types';

export type ProducedSurfaceRates = {
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
export function producedSurfaceRates(
  snapshot: CompositionalSnapshot,
): ProducedSurfaceRates | null {
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

/** Gross produced surface volumes [m³], using the same producing-side rule as the rates. */
export function producedSurfaceCumulative(
  snapshot: CompositionalSnapshot,
): ProducedSurfaceRates | null {
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
