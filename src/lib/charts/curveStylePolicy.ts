/**
 * curveStylePolicy.ts — shared dash patterns, border widths, and legend section
 * labels for all charts.
 *
 * Single source of truth for visual conventions:
 *   - Dash patterns → ANALYTICAL_DASH, PUBLISHED_DASH, AUXILIARY_DASH, SWEEP_DASH_*
 *   - Border widths → SIM_BORDER_*, ANALYTICAL_BORDER, PUBLISHED_BORDER
 *   - Composite style objects → ANALYTICAL_STYLE, PUBLISHED_STYLE, AUXILIARY_STYLE
 *   - Legend section header strings → LEGEND_SECTIONS
 *   - Sim width helper → simBorderWidth()
 */

// ─── Dash patterns ────────────────────────────────────────────────────────────

/** Primary analytical reference curves (moderately dashed). */
export const ANALYTICAL_DASH = [7, 4] as number[];

/** Published data / external reference curves (medium short dash). */
export const PUBLISHED_DASH = [4, 4] as number[];

/** Auxiliary analytical curves — supplemental or upper-bound overlays. */
export const AUXILIARY_DASH = [2, 4] as number[];

// Sweep-specific dash patterns (semantic aliases — distinct per metric for readability).
/** Areal sweep efficiency E_A (medium dash). */
export const SWEEP_DASH_AREAL    = [7, 4]  as number[];
/** Vertical sweep efficiency E_V (short dash). */
export const SWEEP_DASH_VERTICAL = [3, 4]  as number[];
/** Combined / volumetric sweep efficiency E_vol (long dash). */
export const SWEEP_DASH_COMBINED = [12, 4] as number[];

// ─── Border widths ────────────────────────────────────────────────────────────

/** Simulation line — single run (no variant sweep). Bolder for impact. */
export const SIM_BORDER_SINGLE = 2.8;
/** Simulation line — one of many variants in a sweep. Thinner to reduce clutter. */
export const SIM_BORDER_MULTI  = 2.2;
/** Analytical reference — one shared curve (prominent). */
export const ANALYTICAL_BORDER = 2.0;
/** Analytical reference — per-result (one per case; lighter so many don't saturate). */
export const ANALYTICAL_BORDER_MULTI = 1.5;
/** Published data / external reference. */
export const PUBLISHED_BORDER  = 1.5;

/**
 * Returns the appropriate simulation border width.
 * Single-run results are drawn bolder; multi-variant sweeps use a thinner line.
 */
export function simBorderWidth(variantKey: string | null | undefined): number {
    return variantKey == null ? SIM_BORDER_SINGLE : SIM_BORDER_MULTI;
}

// ─── Composite style objects ──────────────────────────────────────────────────
// Spread these into CurveConfig objects to apply both dash and width at once.

/** Shared analytical reference (one curve shown). */
export const ANALYTICAL_STYLE = {
    borderWidth: ANALYTICAL_BORDER,
    borderDash:  ANALYTICAL_DASH,
} as const;

/** Per-result analytical reference (one per case; lighter). */
export const ANALYTICAL_STYLE_MULTI = {
    borderWidth: ANALYTICAL_BORDER_MULTI,
    borderDash:  ANALYTICAL_DASH,
} as const;

/** Published / external data reference. */
export const PUBLISHED_STYLE = {
    borderWidth: PUBLISHED_BORDER,
    borderDash:  PUBLISHED_DASH,
} as const;

/** Auxiliary supplemental overlay (upper bound, derived, etc.). */
export const AUXILIARY_STYLE = {
    borderWidth: PUBLISHED_BORDER,
    borderDash:  AUXILIARY_DASH,
} as const;

// ─── Reference simulation style ──────────────────────────────────────────────

/** Reference simulation line — another simulator's output. Solid but thinner. */
export const REF_SIM_BORDER = 1.5;

/** Reference simulation style object — spread into CurveConfig. No borderDash = solid. */
export const REF_SIM_STYLE = {
    borderWidth: REF_SIM_BORDER,
} as const;

// ─── Auto-style from CurveType ────────────────────────────────────────────────

import type { CurveType } from './universalChartTypes';

/**
 * Returns the borderWidth + optional borderDash that match a CurveType.
 * Spread the result into a CurveConfig to apply the visual convention automatically.
 *
 *   simulation          solid 2.5px
 *   analytical          dashed [7,4] 2.0px
 *   reference           dotted [4,4] 1.5px
 *   reference-simulation solid 1.5px
 */
export function applyCurveTypeStyle(curveType: CurveType): {
    borderWidth: number;
    borderDash?: number[];
} {
    switch (curveType) {
        case 'simulation':          return { borderWidth: SIM_BORDER_SINGLE };
        case 'analytical':          return { borderWidth: ANALYTICAL_BORDER, borderDash: ANALYTICAL_DASH };
        case 'reference':           return { borderWidth: PUBLISHED_BORDER,  borderDash: PUBLISHED_DASH };
        case 'reference-simulation': return { borderWidth: REF_SIM_BORDER };
    }
}

// ─── Legend section labels ────────────────────────────────────────────────────

export const LEGEND_SECTIONS = {
    sim:          'Simulation (solid lines):',
    analytical:   'Analytical (dashed lines):',
    published:    'Published reference (dotted lines):',
    refSim:       'Reference simulation (thin solid lines):',
    driveIndices: 'Drive Indices:',
} as const;

/** Maps CurveType to the appropriate legend section header. */
export const CURVE_TYPE_LEGEND_SECTION: Record<CurveType, string> = {
    simulation:             LEGEND_SECTIONS.sim,
    analytical:             LEGEND_SECTIONS.analytical,
    reference:              LEGEND_SECTIONS.published,
    'reference-simulation': LEGEND_SECTIONS.refSim,
};
