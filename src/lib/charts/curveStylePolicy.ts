/**
 * curveStylePolicy.ts — shared dash patterns and legend section labels for all charts.
 *
 * Single source of truth for visual conventions:
 *   - Analytical reference curves → ANALYTICAL_DASH
 *   - Published data reference curves → PUBLISHED_DASH
 *   - Auxiliary / supplemental curves → AUXILIARY_DASH
 *   - Sweep-specific dash aliases → SWEEP_DASH_*
 *   - Legend section header strings → LEGEND_SECTIONS
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

// ─── Legend section labels ────────────────────────────────────────────────────

export const LEGEND_SECTIONS = {
    sim:          'Simulation (solid lines):',
    analytical:   'Analytical (dashed lines):',
    published:    'Published reference (dashed lines):',
    driveIndices: 'Drive Indices:',
} as const;
