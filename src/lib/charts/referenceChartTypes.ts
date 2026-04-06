/**
 * referenceChartTypes.ts — shared types, color palette, and small panel utilities
 * for the reference-comparison chart layer.
 *
 * Imported by referenceOverlayBuilders, sweepPanelBuilder, and buildChartData.
 * No analytical logic or Chart.js here — pure data-structure definitions and
 * tiny construction/mutation helpers.
 */

import type { CurveConfig } from './chartTypes';
import type { XYPoint } from './axisAdapters';
import { toXYSeries } from './axisAdapters';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import type { RateChartPanelKey, RateChartAuxiliaryPanelKey } from './rateChartLayoutConfig';

// ─── Panel types ──────────────────────────────────────────────────────────────

export type ReferenceComparisonPanel = {
    curves: CurveConfig[];
    series: XYPoint[][];
};

export type ReferenceComparisonSweepPanels = {
    rf: ReferenceComparisonPanel | null;
    areal: ReferenceComparisonPanel | null;
    vertical: ReferenceComparisonPanel | null;
    combined: ReferenceComparisonPanel | null;
    combinedMobileOil: ReferenceComparisonPanel | null;
};

export type ReferenceComparisonPrimaryPanelMap = Record<RateChartPanelKey, ReferenceComparisonPanel>;
export type ReferenceComparisonAuxiliaryPanelMap = Record<RateChartAuxiliaryPanelKey, ReferenceComparisonPanel | null>;
export type ReferenceComparisonPanelMap = ReferenceComparisonPrimaryPanelMap & ReferenceComparisonAuxiliaryPanelMap;

// ─── Model output type ────────────────────────────────────────────────────────

export type ReferenceComparisonModel = {
    orderedResults: BenchmarkRunResult[];
    /**
     * Preview/pending variant entries for the cases selector UI.
     * Populated when:
     *  - Pure preview (no results): multi-variant analytical preview cases.
     *  - Mid-sweep (some results done): remaining queued/running variants.
     * Empty when all variants have completed results.
     */
    previewCases: ReferenceComparisonPreviewCase[];
    panels: ReferenceComparisonPanelMap;
    axisMappingWarning: string | null;
};

// ─── Variant / preview types ──────────────────────────────────────────────────

export type ReferenceComparisonTheme = 'dark' | 'light';

/**
 * A preview or pending variant entry shown in the cases selector UI.
 * Used both in pure-preview mode (no results yet) and during mid-sweep
 * (some results done, others still queued/running).
 */
export type ReferenceComparisonPreviewCase = {
    /** Variant key — matches caseKey on the chart series for visibility toggling. */
    key: string;
    /** Display label for the cases selector button. */
    label: string;
    /** Color palette index used for this variant's curves. */
    colorIndex: number;
};

/**
 * One entry in the multi-variant analytical preview shown before any simulation
 * results exist. Each entry produces its own colored analytical curve so the user
 * can see how the sensitivity variants differ analytically without running first.
 */
export type AnalyticalPreviewVariant = {
    /** Display label used in curve legends (e.g. "Favorable", "Base"). */
    label: string;
    /** Variant key used as caseKey on chart series for toggle support. */
    variantKey: string;
    /** Full merged params (base scenario params + variant paramPatch). */
    params: Record<string, any>;
};

// ─── Analytical overlay intermediate type ────────────────────────────────────

/** Intermediate result of a reference-overlay builder — holds labeled y-series
 *  and x-values before they are assembled into chart panels. */
export type AnalyticalOverlay = {
    rates: { label: string; values: Array<number | null> } | null;
    cumulative: {
        recoveryLabel: string;
        recoveryValues: Array<number | null>;
        cumulativeLabel: string;
        cumulativeValues: Array<number | null>;
    } | null;
    diagnostics: { label: string; values: Array<number | null> } | null;
    xValues: Array<number | null>;
};

// ─── Color palette ────────────────────────────────────────────────────────────

/** Tableau 20 — 20 perceptually distinct colors for categorical data. */
const CASE_COLORS = [
    '#4e79a7', '#f28e2b', '#e15759', '#76b7b2', '#59a14f',
    '#edc948', '#b07aa1', '#ff9da7', '#9c755f', '#bab0ac',
    '#af7aa1', '#d37295', '#fabfd2', '#b6992d', '#499894',
    '#86bcb6', '#8cd17d', '#f1ce63', '#a0cbe8', '#ffbe7d',
];

export function getReferenceComparisonCaseColor(index: number): string {
    return CASE_COLORS[index % CASE_COLORS.length];
}

/** Neutral high-contrast color for single-variant reference curves. */
export function getReferenceColor(theme: ReferenceComparisonTheme): string {
    return theme === 'dark' ? '#f8fafc' : '#0f172a';
}

/** Neutral grey used as the toggle-group line indicator in legend buttons.
 *  Actual line colors come from the case-color palette. */
export function getLegendGrey(theme: ReferenceComparisonTheme): string {
    return theme === 'dark' ? '#94a3b8' : '#64748b';
}

// ─── Label utilities ──────────────────────────────────────────────────────────

/**
 * Strips the scenario-name prefix from a case label so sub-panel legend buttons
 * stay compact. E.g. "Rate Decline — s=0 (clean)" → "s=0 (clean)".
 * Falls back to the full label when no separator is found.
 */
export function compactCaseLabel(label: string): string {
    const emDash = label.indexOf(' — ');
    if (emDash !== -1) return label.slice(emDash + 3).trim();
    const hyphen = label.indexOf(' - ');
    if (hyphen !== -1) return label.slice(hyphen + 3).trim();
    return label;
}

// ─── Panel construction helpers ───────────────────────────────────────────────

/** Mutates `panel` by appending a new curve + its xy series. */
export function appendSeries(
    panel: ReferenceComparisonPanel,
    curve: CurveConfig,
    xValues: Array<number | null>,
    yValues: Array<number | null>,
): void {
    panel.curves.push(curve);
    panel.series.push(toXYSeries(xValues, yValues));
}

export function createReferenceComparisonPanel(): ReferenceComparisonPanel {
    return { curves: [], series: [] };
}

export function createSweepPanels(): Record<keyof ReferenceComparisonSweepPanels, ReferenceComparisonPanel> {
    return {
        rf: createReferenceComparisonPanel(),
        areal: createReferenceComparisonPanel(),
        vertical: createReferenceComparisonPanel(),
        combined: createReferenceComparisonPanel(),
        combinedMobileOil: createReferenceComparisonPanel(),
    };
}

export function finalizeSweepPanels(
    panels: Record<keyof ReferenceComparisonSweepPanels, ReferenceComparisonPanel>,
): ReferenceComparisonSweepPanels {
    return {
        rf: panels.rf.curves.length > 0 ? panels.rf : null,
        areal: panels.areal.curves.length > 0 ? panels.areal : null,
        vertical: panels.vertical.curves.length > 0 ? panels.vertical : null,
        combined: panels.combined.curves.length > 0 ? panels.combined : null,
        combinedMobileOil: panels.combinedMobileOil.curves.length > 0 ? panels.combinedMobileOil : null,
    };
}
