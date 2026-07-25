/**
 * analyticalMethodRegistry.ts — one descriptor per `AnalyticalMethod`, the single
 * routing table for the comparison-chart stack.
 *
 * Before this module, `buildChartData.ts` carried a four-way `if
 * (family.analyticalMethod === …)` ladder duplicated across four contexts
 * (shared overlay / per-result overlay / pending-variant overlay / pure preview),
 * and `ReferenceComparisonChart.svelte`, `benchmarkDisclosure.ts` and
 * `axisAdapters.ts` each carried their own smaller copies of the same branch.
 * Adding an analytical method meant editing all four.
 *
 * Now a method declares:
 *   - `slots`             which panel/curve pairs its reference solution produces,
 *                         and in which of the four contexts each one appears
 *   - `fromResult`        overlay computed against a completed simulation run
 *   - `fromParams`        overlay computed from params alone (preview / pending)
 *   - `resolveOverlayMode` whether one shared curve or one curve per case is correct
 *   - `nativeXAxis`       the axis the closed-form solution is expressed in
 *   - `panelPresentation` method-dependent panel titles / curve keys / scale preset
 *   - `referenceLabel`    disclosure wording for the reference solution
 *
 * `buildChartData.ts` then walks `slots` in a single generic loop. Adding a
 * method is: write its overlay builder in `referenceOverlayBuilders.ts` /
 * `analyticalParamAdapters.ts`, then add one entry here. No orchestrator edit.
 *
 * Deliberately free of Chart.js, Svelte, and the scenario catalog: it imports
 * `AnalyticalMethod` as a *type* only, so it stays a leaf next to the other
 * chart builder modules and cannot form a runtime import cycle.
 */

import type { AnalyticalMethod, AnalyticalOverlayMode } from '../catalog/scenarios';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import type { ChartPanelFallback } from './chartPanelSelection';
import type { DerivedRunSeries } from './axisAdapters';
import type { RateChartPanelId, RateChartPanelKey, RateChartXAxisMode } from './rateChartLayoutConfig';
import {
    computeBLAnalyticalFromParams,
    computeDepletionAnalyticalFromParams,
    computeGasOilBLAnalyticalFromParams,
    computeWellTestFromParams,
    hasDistinctBuckleyLeverettOverlays,
    hasDistinctGasOilBLOverlays,
    resolveOverlayMode,
} from './analyticalParamAdapters';
import {
    buildBuckleyLeverettReference,
    buildDepletionReference,
    buildGasOilBLReference,
    buildWellTestReference,
} from './referenceOverlayBuilders';
import type { AnalyticalOverlay } from './referenceChartTypes';

// ─── Contexts ─────────────────────────────────────────────────────────────────

/**
 * The four situations in which an analytical curve is drawn. They differ in
 * label shape, color source and line weight — not in physics — so a slot opts
 * into the ones where it is meaningful rather than each context re-deciding.
 *
 *   shared      one neutral curve standing for every case (physics identical)
 *   per-result  one curve per completed run, in that run's case color
 *   pending     one curve per variant still queued/running (params only)
 *   preview     analytical-only view before any simulation has been started
 */
export type AnalyticalOverlayContext = 'shared' | 'per-result' | 'pending' | 'preview';

const ALL_CONTEXTS: readonly AnalyticalOverlayContext[] = ['shared', 'per-result', 'pending', 'preview'];

// ─── Curve slots ──────────────────────────────────────────────────────────────

/**
 * One reference curve a method produces: where it lands, what it is called in
 * each context, and how to pull its y-values out of a computed curve set.
 */
export type AnalyticalCurveSlot = {
    /** Panel this curve is appended to. */
    panelKey: RateChartPanelKey;
    /** Curve key — also the lookup key into `AnalyticalCurveSet.values`. */
    curveKey: string;
    /** Full label in the `shared` context (no case prefix). */
    sharedLabel: string;
    /**
     * Suffix after `${caseLabel} — Reference` in the `per-result` and `pending`
     * contexts. Empty string yields the bare `${caseLabel} — Reference`.
     */
    perCaseSuffix: string;
    /** Label in the `preview` context, after any `${variantLabel} — ` prefix. */
    previewLabel: string;
    /** Contexts this curve appears in. */
    contexts: readonly AnalyticalOverlayContext[];
};

/**
 * A computed analytical solution, normalized to `curveKey → y-series` so the
 * panel-assembly loop never needs to know which method produced it.
 * A missing or null entry means "this method produced no such curve here" and
 * the slot is skipped.
 */
export type AnalyticalCurveSet = {
    xValues: Array<number | null>;
    values: Record<string, Array<number | null> | null | undefined>;
};

// ─── Descriptor ───────────────────────────────────────────────────────────────

export type AnalyticalOverlayModeInput = {
    /** Scenario/dimension-declared preference; 'auto' defers to the method. */
    requested: AnalyticalOverlayMode;
    /** Params of every case in play (completed results + pending variants). */
    paramSets: Array<Record<string, any>>;
    /** Comparison-chart hint that the active variants perturb analytical inputs. */
    analyticalPerVariant?: boolean;
};

export type AnalyticalMethodDescriptor = {
    method: AnalyticalMethod;
    /**
     * Axis the closed-form solution is naturally expressed in. A 'pvi'-native
     * method plotted on any other axis must be remapped through a completed
     * run's own time/injection history; a 'time'-native one never is.
     */
    nativeXAxis: 'pvi' | 'time';
    slots: readonly AnalyticalCurveSlot[];
    /** Overlay from a completed run. Null when the method has no reference solution. */
    fromResult:
        | ((result: BenchmarkRunResult, derived: DerivedRunSeries, xAxisMode: RateChartXAxisMode) => AnalyticalCurveSet | null)
        | null;
    /** Overlay from params alone — used for preview and still-pending variants. */
    fromParams:
        | ((params: Record<string, any>, xAxisMode: RateChartXAxisMode) => AnalyticalCurveSet | null)
        | null;
    resolveOverlayMode: (input: AnalyticalOverlayModeInput) => 'shared' | 'per-result';
    /** Method-dependent panel presentation, layered over PANEL_DEFS. */
    panelPresentation: Partial<Record<RateChartPanelId, Partial<ChartPanelFallback>>>;
    /** Reference-solution wording for the benchmark disclosure card. */
    referenceLabel: string;
};

// ─── Overlay → curve-set adapters ─────────────────────────────────────────────

function curveSet(
    xValues: Array<number | null>,
    values: AnalyticalCurveSet['values'],
): AnalyticalCurveSet {
    return { xValues, values };
}

/** Shared shape mapping for the three BL/depletion-style overlays. */
function fromOverlay(
    overlay: AnalyticalOverlay,
    keys: { rates?: string; recovery?: string; cumulative?: string; diagnostics?: string; producerBhp?: string },
): AnalyticalCurveSet {
    const values: AnalyticalCurveSet['values'] = {};
    if (keys.rates) values[keys.rates] = overlay.rates?.values ?? null;
    if (keys.recovery) values[keys.recovery] = overlay.cumulative?.recoveryValues ?? null;
    if (keys.cumulative) values[keys.cumulative] = overlay.cumulative?.cumulativeValues ?? null;
    if (keys.diagnostics) values[keys.diagnostics] = overlay.diagnostics?.values ?? null;
    if (keys.producerBhp) values[keys.producerBhp] = overlay.producerBhp?.values ?? null;
    return curveSet(overlay.xValues, values);
}

// ─── Descriptors ──────────────────────────────────────────────────────────────

const NO_OVERLAY_PRESENTATION: AnalyticalMethodDescriptor['panelPresentation'] = {
    rates: {
        title: 'Oil Rate',
        curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
        scalePreset: 'rates',
        allowLogToggle: false,
    },
    cumulative: {
        curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
    },
};

const buckleyLeverett: AnalyticalMethodDescriptor = {
    method: 'buckley-leverett',
    nativeXAxis: 'pvi',
    slots: [
        {
            panelKey: 'rates',
            curveKey: 'water-cut-reference',
            sharedLabel: 'Reference Solution Water Cut',
            perCaseSuffix: '',
            previewLabel: 'Analytical Water Cut',
            contexts: ALL_CONTEXTS,
        },
        {
            panelKey: 'recovery',
            curveKey: 'recovery-factor-reference',
            sharedLabel: 'Reference Solution Recovery',
            perCaseSuffix: ' Recovery',
            previewLabel: 'Analytical Recovery Factor',
            contexts: ALL_CONTEXTS,
        },
    ],
    fromResult: (result, derived, xAxisMode) => fromOverlay(
        buildBuckleyLeverettReference(result, derived, xAxisMode),
        { rates: 'water-cut-reference', recovery: 'recovery-factor-reference' },
    ),
    fromParams: (params) => {
        const curves = computeBLAnalyticalFromParams(params);
        if (!curves) return null;
        return curveSet(curves.xValues, {
            'water-cut-reference': curves.waterCut,
            'recovery-factor-reference': curves.recovery,
        });
    },
    resolveOverlayMode: ({ requested, paramSets }) => resolveOverlayMode({
        requested,
        distinctByPhysics: hasDistinctBuckleyLeverettOverlays(paramSets),
    }),
    panelPresentation: {
        rates: {
            title: 'Breakthrough',
            curveKeys: ['water-cut-sim', 'water-cut-reference'],
            scalePreset: 'breakthrough',
            allowLogToggle: false,
        },
        cumulative: {
            curveKeys: ['cum-oil-sim', 'cum-oil-reference', 'cum-injection'],
        },
    },
    referenceLabel: 'Buckley-Leverett reference solution',
};

const gasOilBL: AnalyticalMethodDescriptor = {
    method: 'gas-oil-bl',
    nativeXAxis: 'pvi',
    slots: [
        {
            panelKey: 'rates',
            curveKey: 'gas-cut-reference',
            sharedLabel: 'Reference Solution Gas Cut',
            perCaseSuffix: '',
            previewLabel: 'Analytical Gas Cut',
            contexts: ALL_CONTEXTS,
        },
        {
            panelKey: 'recovery',
            curveKey: 'recovery-factor-reference',
            sharedLabel: 'Reference Solution Recovery',
            perCaseSuffix: ' Recovery',
            previewLabel: 'Analytical Recovery Factor',
            contexts: ALL_CONTEXTS,
        },
        {
            // Shared-only, matching the pre-registry behavior. The per-case and
            // preview contexts never drew a gas-oil cumulative-oil reference.
            // Tracked in TODO.md as an asymmetry to resolve deliberately rather
            // than silently during this consolidation.
            panelKey: 'cumulative',
            curveKey: 'cum-oil-reference',
            sharedLabel: 'Reference Solution Cum Oil',
            perCaseSuffix: ' Cum Oil',
            previewLabel: 'Analytical Cum Oil',
            contexts: ['shared'],
        },
    ],
    fromResult: (result, derived, xAxisMode) => fromOverlay(
        buildGasOilBLReference(result, derived, xAxisMode),
        {
            rates: 'gas-cut-reference',
            recovery: 'recovery-factor-reference',
            cumulative: 'cum-oil-reference',
        },
    ),
    fromParams: (params) => {
        const curves = computeGasOilBLAnalyticalFromParams(params);
        if (!curves) return null;
        return curveSet(curves.pviValues, {
            'gas-cut-reference': curves.gasCut,
            'recovery-factor-reference': curves.recovery,
        });
    },
    resolveOverlayMode: ({ requested, paramSets }) => resolveOverlayMode({
        requested,
        distinctByPhysics: hasDistinctGasOilBLOverlays(paramSets),
    }),
    panelPresentation: {
        rates: {
            title: 'Gas Breakthrough',
            curveKeys: ['gas-cut-sim', 'gas-cut-reference'],
            scalePreset: 'breakthrough',
            allowLogToggle: false,
        },
        cumulative: {
            curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
        },
    },
    referenceLabel: 'Gas-oil Buckley-Leverett reference solution',
};

const depletion: AnalyticalMethodDescriptor = {
    method: 'depletion',
    nativeXAxis: 'time',
    slots: [
        {
            panelKey: 'rates',
            curveKey: 'oil-rate-reference',
            sharedLabel: 'Reference Solution Oil Rate',
            perCaseSuffix: '',
            previewLabel: 'Analytical Oil Rate',
            contexts: ALL_CONTEXTS,
        },
        {
            panelKey: 'recovery',
            curveKey: 'recovery-factor-reference',
            sharedLabel: 'Reference Solution Recovery',
            perCaseSuffix: ' Recovery',
            previewLabel: 'Analytical Recovery Factor',
            contexts: ALL_CONTEXTS,
        },
        {
            panelKey: 'cumulative',
            curveKey: 'cum-oil-reference',
            sharedLabel: 'Reference Solution Cum Oil',
            perCaseSuffix: ' Cum Oil',
            previewLabel: 'Analytical Cum Oil',
            contexts: ALL_CONTEXTS,
        },
        {
            panelKey: 'diagnostics',
            curveKey: 'avg-pressure-reference',
            sharedLabel: 'Reference Solution Avg Pressure',
            perCaseSuffix: ' Pressure',
            previewLabel: 'Analytical Avg Pressure',
            contexts: ALL_CONTEXTS,
        },
    ],
    fromResult: (result, derived, xAxisMode) => fromOverlay(
        buildDepletionReference(result, derived, xAxisMode),
        {
            rates: 'oil-rate-reference',
            recovery: 'recovery-factor-reference',
            cumulative: 'cum-oil-reference',
            diagnostics: 'avg-pressure-reference',
        },
    ),
    fromParams: (params, xAxisMode) => {
        const curves = computeDepletionAnalyticalFromParams(params, xAxisMode);
        if (!curves) return null;
        return curveSet(curves.xValues, {
            'oil-rate-reference': curves.oilRates,
            'recovery-factor-reference': curves.recoveryValues,
            'cum-oil-reference': curves.cumulativeOilValues,
            'avg-pressure-reference': curves.avgPressureValues,
        });
    },
    resolveOverlayMode: ({ requested, analyticalPerVariant }) => resolveOverlayMode({
        requested,
        distinctByPhysics: false,
        analyticalPerVariant,
    }),
    panelPresentation: {
        rates: {
            title: 'Oil Rate',
            curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
            scalePreset: 'rates',
            allowLogToggle: true,
        },
        cumulative: {
            curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
        },
    },
    referenceLabel: 'Depletion reference solution',
};

const wellTest: AnalyticalMethodDescriptor = {
    method: 'well-test',
    nativeXAxis: 'time',
    slots: [
        {
            panelKey: 'producer_bhp',
            curveKey: 'producer-bhp-reference',
            sharedLabel: 'Reference Solution Flowing BHP',
            perCaseSuffix: ' Flowing BHP',
            previewLabel: 'Analytical Flowing BHP',
            contexts: ['per-result', 'pending', 'preview'],
        },
        {
            // No 'pending' context, matching the pre-registry behavior: a queued
            // well-test variant drew only its flowing-BHP reference, never the
            // controlled oil rate. Recorded in TODO.md alongside the gas-oil
            // cumulative asymmetry.
            panelKey: 'oil_rate',
            curveKey: 'oil-rate-reference',
            sharedLabel: 'Reference Solution Oil Rate',
            perCaseSuffix: ' Oil Rate',
            previewLabel: 'Analytical Oil Rate',
            contexts: ['per-result', 'preview'],
        },
    ],
    fromResult: (result, derived, xAxisMode) => fromOverlay(
        buildWellTestReference(result, derived, xAxisMode),
        { rates: 'oil-rate-reference', producerBhp: 'producer-bhp-reference' },
    ),
    fromParams: (params, xAxisMode) => {
        const curves = computeWellTestFromParams(params, xAxisMode);
        if (!curves) return null;
        return curveSet(curves.xValues, {
            'producer-bhp-reference': curves.flowingBhp,
            'oil-rate-reference': curves.oilRates,
        });
    },
    // Always per-result: a drawdown reference depends on k, skin and rate, which
    // is exactly what these scenarios vary, so a shared curve would be wrong for
    // every variant but one.
    resolveOverlayMode: () => 'per-result',
    panelPresentation: NO_OVERLAY_PRESENTATION,
    referenceLabel: 'Well-test reference solution',
};

const digitizedReference: AnalyticalMethodDescriptor = {
    method: 'digitized-reference',
    nativeXAxis: 'time',
    slots: [],
    fromResult: null,
    fromParams: null,
    resolveOverlayMode: () => 'shared',
    panelPresentation: NO_OVERLAY_PRESENTATION,
    referenceLabel: 'Published benchmark reference curves',
};

const noMethod: AnalyticalMethodDescriptor = {
    method: 'none',
    nativeXAxis: 'time',
    slots: [],
    fromResult: null,
    fromParams: null,
    resolveOverlayMode: () => 'shared',
    panelPresentation: NO_OVERLAY_PRESENTATION,
    referenceLabel: 'Depletion reference solution',
};

export const ANALYTICAL_METHOD_DESCRIPTORS: Record<AnalyticalMethod, AnalyticalMethodDescriptor> = {
    'buckley-leverett': buckleyLeverett,
    'gas-oil-bl': gasOilBL,
    'depletion': depletion,
    'well-test': wellTest,
    'digitized-reference': digitizedReference,
    'none': noMethod,
};

/**
 * Descriptor lookup. An absent or unrecognized method resolves to 'none' —
 * a chart with no family yet must render its simulation curves rather than throw.
 */
export function getAnalyticalMethodDescriptor(
    method: AnalyticalMethod | null | undefined,
): AnalyticalMethodDescriptor {
    return (method && ANALYTICAL_METHOD_DESCRIPTORS[method]) || noMethod;
}

/** Slots this method draws in the given context. */
export function slotsForContext(
    descriptor: AnalyticalMethodDescriptor,
    context: AnalyticalOverlayContext,
): AnalyticalCurveSlot[] {
    return descriptor.slots.filter((slot) => slot.contexts.includes(context));
}
