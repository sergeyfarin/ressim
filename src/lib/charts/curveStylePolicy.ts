/**
 * curveStylePolicy.ts — the only place a line style is decided.
 *
 * Three tiers, distinguished by stroke pattern; runs within a tier are
 * distinguished by colour, never by pattern:
 *
 *   ResSim simulation      solid              — our solver's output
 *   Analytical reference   dashed  [7,4]      — a closed-form/reference solution
 *   Additional reference   dotted  [1,3]      — an external source (published data,
 *                                               another simulator such as OPM Flow)
 *
 * Sensitivity variants differ only by colour: every ResSim curve stays solid,
 * and a variant's analytical twin is dashed in that same variant's colour.
 * There is deliberately no fourth pattern — no "auxiliary" or per-metric dash.
 * A quantity that needs to be distinguished from its neighbours belongs in its
 * own panel (one property per plot), not in a fourth line style.
 *
 * Spread the composite STYLE objects into a CurveConfig rather than restating
 * borderWidth/borderDash by hand; `no-literal-border-dash.test.ts` enforces that
 * no dash array is written outside this module.
 */

// ─── Dash patterns ────────────────────────────────────────────────────────────

/** Analytical reference curves. */
export const ANALYTICAL_DASH = [7, 4] as number[];

/** Additional reference sources — published data and other simulators alike. */
export const REFERENCE_DASH = [1, 3] as number[];

// ─── Border widths ────────────────────────────────────────────────────────────

/** Simulation line — single run (no variant sweep). Bolder for impact. */
export const SIM_BORDER_SINGLE = 2.8;
/** Simulation line — one of many variants in a sweep. Thinner to reduce clutter. */
export const SIM_BORDER_MULTI  = 2.2;
/** Simulation line — a secondary quantity in its own panel. */
export const SIM_BORDER_SECONDARY = 1.8;
/** Analytical reference — one shared curve (prominent). */
export const ANALYTICAL_BORDER = 2.0;
/** Analytical reference — per-result (one per case; lighter so many don't saturate). */
export const ANALYTICAL_BORDER_MULTI = 1.5;
/** Additional reference source (published data / other simulator). */
export const REFERENCE_BORDER = 1.5;
/**
 * Additional reference source carrying a panel on its own, with no ResSim curve
 * beside it. Prominent, but still dotted — solid means ResSim and nothing else.
 */
export const REFERENCE_BORDER_PRIMARY = 2.5;

/**
 * Returns the appropriate simulation border width.
 * Single-run results are drawn bolder; multi-variant sweeps use a thinner line.
 */
export function simBorderWidth(variantKey: string | null | undefined): number {
    return variantKey == null ? SIM_BORDER_SINGLE : SIM_BORDER_MULTI;
}

// ─── Composite style objects ──────────────────────────────────────────────────
// Spread these into CurveConfig objects to apply both dash and width at once.

/** ResSim curve carrying a panel. Colour comes from the case/variant. */
export const SIM_STYLE = {
    borderWidth: SIM_BORDER_SINGLE,
} as const;

/** ResSim curve for a secondary quantity in its own panel. */
export const SIM_STYLE_SECONDARY = {
    borderWidth: SIM_BORDER_SECONDARY,
} as const;

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

/** Additional reference source overlaid beside a ResSim curve. */
export const REFERENCE_STYLE = {
    borderWidth: REFERENCE_BORDER,
    borderDash:  REFERENCE_DASH,
} as const;

/** Additional reference source shown without a ResSim curve beside it. */
export const REFERENCE_STYLE_PRIMARY = {
    borderWidth: REFERENCE_BORDER_PRIMARY,
    borderDash:  REFERENCE_DASH,
} as const;

// ─── Auto-style from CurveType ────────────────────────────────────────────────

/**
 * How a curve relates to the run, which is what its styling encodes: a solid
 * line means ResSim, dashes mean something else. Moved here from
 * `universalChartTypes.ts` when the live-chart path was deleted (2026-08-02) —
 * this module is its only consumer.
 */
export type CurveType =
    | 'simulation'
    | 'analytical'
    | 'reference'
    | 'reference-simulation';

/**
 * Returns the borderWidth + optional borderDash that match a CurveType.
 * Spread the result into a CurveConfig to apply the visual convention automatically.
 */
export function applyCurveTypeStyle(curveType: CurveType): {
    borderWidth: number;
    borderDash?: number[];
} {
    switch (curveType) {
        case 'simulation':           return { ...SIM_STYLE };
        case 'analytical':           return { ...ANALYTICAL_STYLE };
        case 'reference':            return { ...REFERENCE_STYLE };
        case 'reference-simulation': return { ...REFERENCE_STYLE };
    }
}

// ─── Legend section labels ────────────────────────────────────────────────────

export const LEGEND_SECTIONS = {
    sim:          'Simulation (solid lines):',
    analytical:   'Analytical (dashed lines):',
    published:    'Published reference (dotted lines):',
    refSim:       'Reference simulation (dotted lines):',
    driveIndices: 'Drive Indices:',
} as const;

/** Maps CurveType to the appropriate legend section header. */
export const CURVE_TYPE_LEGEND_SECTION: Record<CurveType, string> = {
    simulation:             LEGEND_SECTIONS.sim,
    analytical:             LEGEND_SECTIONS.analytical,
    reference:              LEGEND_SECTIONS.published,
    'reference-simulation': LEGEND_SECTIONS.refSim,
};
