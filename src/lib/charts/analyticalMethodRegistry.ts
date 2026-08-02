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
 *   - `producesSweepPanels` whether its output is the E_A/E_V/E_vol panels instead
 *
 * A method with no reference solution ('none', 'digitized-reference', 'sweep')
 * declares no slots and emits nothing. It must never fall through to another
 * method's overlay path.
 *
 * `buildChartData.ts` then walks `slots` in a single generic loop. Adding a
 * method is: write its overlay builder in `referenceOverlayBuilders.ts` /
 * `analyticalParamAdapters.ts`, then add one entry here. No orchestrator edit.
 *
 * Deliberately free of Chart.js, Svelte, and the scenario catalog: it imports
 * `AnalyticalMethod` as a *type* only, so it stays a leaf next to the other
 * chart builder modules and cannot form a runtime import cycle.
 */

import type { AnalyticalMethod, AnalyticalOverlayMode, PrimaryRateCurve } from '../catalog/scenarios';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import type { ChartPanelFallback } from './chartPanelSelection';
import { buildXAxisValues, type DerivedRunSeries } from './axisAdapters';
import type { ChartLayoutConfig, ChartPanelId, ChartPanelKey, ChartXAxisMode } from './chartLayoutConfig';
import {
    buildDisplacementBenchmarkLayout,
    buildProductionBenchmarkLayout,
    type FallbackLayoutInput,
} from './benchmarkFallbackLayouts';
import {
    computeBLAnalyticalFromParams,
    computeDepletionAnalyticalFromParams,
    computeGasOilBLAnalyticalFromParams,
    computeGasMaterialBalanceCurves,
    computeWellTestFromParams,
    computeWellTestOnTimeAxis,
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
    panelKey: ChartPanelKey;
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
    /** Optional non-primary axis for mixed-unit diagnostic panels. */
    yAxisID?: 'y' | 'y1';
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
     * Whether this method's output is the dedicated E_A / E_V / E_vol sweep
     * panels rather than primary rate/recovery overlays. True only for 'sweep'.
     */
    producesSweepPanels: boolean;
    /**
     * Which family of *simulation* curves this method's charts show. Sweep and
     * Buckley-Leverett both show the water-cut set: a sweep chart compares
     * contacted fraction against a waterflood's producing water cut, so the
     * simulation side is identical even though the reference side is not.
     */
    simulationCurveSet: PrimaryRateCurve;
    /**
     * Axis the closed-form solution is naturally expressed in. A 'pvi'-native
     * method plotted on any other axis must be remapped through a completed
     * run's own time/injection history; a 'time'-native one never is.
     */
    nativeXAxis: 'pvi' | 'time';
    /**
     * Whether this method defines a characteristic time τ, which the chart uses
     * for the dimensionless-time (`tD`) axis. Declared here rather than tested
     * for with `analyticalMethod === 'depletion'` at the point of use.
     */
    definesCharacteristicTime: boolean;
    slots: readonly AnalyticalCurveSlot[];
    /** Overlay from a completed run. Null when the method has no reference solution. */
    fromResult:
        | ((result: BenchmarkRunResult, derived: DerivedRunSeries, xAxisMode: ChartXAxisMode) => AnalyticalCurveSet | null)
        | null;
    /** Overlay from params alone — used for preview and still-pending variants. */
    fromParams:
        | ((params: Record<string, any>, xAxisMode: ChartXAxisMode) => AnalyticalCurveSet | null)
        | null;
    resolveOverlayMode: (input: AnalyticalOverlayModeInput) => 'shared' | 'per-result';
    /** Method-dependent panel presentation, layered over PANEL_DEFS. */
    panelPresentation: Partial<Record<ChartPanelId, Partial<ChartPanelFallback>>>;
    /** Reference-solution wording for the benchmark disclosure card. */
    referenceLabel: string;
    /**
     * Default chart layout for a *benchmark* family, which has no scenario to
     * declare one. Method-specific presentation belongs with the method: this
     * was the last `family.analyticalMethod === …` branch outside this registry,
     * living in `referenceChartConfig.ts`.
     */
    fallbackLayout: (input: FallbackLayoutInput) => ChartLayoutConfig;
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
    simulationCurveSet: 'water-cut',
    producesSweepPanels: false,
    nativeXAxis: 'pvi',
    definesCharacteristicTime: false,
    fallbackLayout: buildDisplacementBenchmarkLayout,
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
    simulationCurveSet: 'gas-cut',
    producesSweepPanels: false,
    nativeXAxis: 'pvi',
    definesCharacteristicTime: false,
    fallbackLayout: buildProductionBenchmarkLayout,
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
    simulationCurveSet: 'oil-rate',
    producesSweepPanels: false,
    nativeXAxis: 'time',
    definesCharacteristicTime: true,
    fallbackLayout: buildProductionBenchmarkLayout,
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
    simulationCurveSet: 'oil-rate',
    producesSweepPanels: false,
    nativeXAxis: 'time',
    definesCharacteristicTime: false,
    fallbackLayout: buildProductionBenchmarkLayout,
    slots: [
        {
            panelKey: 'producer_bhp',
            curveKey: 'producer-bhp-reference',
            sharedLabel: 'Reference Solution Flowing BHP',
            perCaseSuffix: ' Flowing BHP',
            previewLabel: 'Analytical Flowing BHP',
            contexts: ['shared', 'per-result', 'pending', 'preview'],
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
            contexts: ['shared', 'per-result', 'preview'],
        },
        {
            panelKey: 'pss_drawdown',
            curveKey: 'pss-drawdown-reference',
            sharedLabel: 'Dietz PSS Drawdown',
            perCaseSuffix: ' Dietz PSS Drawdown',
            previewLabel: 'Analytical Dietz PSS Drawdown',
            contexts: ['shared', 'per-result', 'pending', 'preview'],
        },
        {
            panelKey: 'pss_productivity',
            curveKey: 'pss-productivity-reference',
            sharedLabel: 'Dietz Productivity Index',
            perCaseSuffix: ' Dietz PI',
            previewLabel: 'Analytical Dietz PI',
            contexts: ['shared', 'per-result', 'pending', 'preview'],
        },
        {
            panelKey: 'pss_shape_factor',
            curveKey: 'pss-shape-factor-reference',
            sharedLabel: 'Dietz Shape Factor C_A',
            perCaseSuffix: ' Dietz C_A',
            previewLabel: 'Analytical Dietz C_A',
            contexts: ['shared', 'per-result', 'pending', 'preview'],
        },
    ],
    fromResult: (result, derived, xAxisMode) => {
        const overlay = buildWellTestReference(result, derived, xAxisMode);
        const solution = computeWellTestOnTimeAxis(result.params, derived.time);
        if (!solution) return null;
        return curveSet(buildXAxisValues(derived, xAxisMode), {
            'producer-bhp-reference': overlay.producerBhp?.values ?? null,
            'oil-rate-reference': overlay.rates?.values ?? null,
            'pss-drawdown-reference': solution.pssDrawdown,
            'pss-productivity-reference': solution.pssProductivity,
            'pss-shape-factor-reference': solution.pssShapeFactor,
        });
    },
    fromParams: (params, xAxisMode) => {
        const curves = computeWellTestFromParams(params, xAxisMode);
        if (!curves) return null;
        return curveSet(curves.xValues, {
            'producer-bhp-reference': curves.flowingBhp,
            'oil-rate-reference': curves.oilRates,
            'pss-drawdown-reference': curves.pssDrawdown,
            'pss-productivity-reference': curves.pssProductivity,
            'pss-shape-factor-reference': curves.pssShapeFactor,
        });
    },
    // Skin/permeability studies request per-result curves because their
    // references move. Grid studies explicitly request one shared reference:
    // their analytical physics is unchanged and duplicate curves add clutter.
    resolveOverlayMode: ({ requested }) => requested === 'shared' ? 'shared' : 'per-result',
    panelPresentation: NO_OVERLAY_PRESENTATION,
    referenceLabel: 'Well-test reference solution',
};

const digitizedReference: AnalyticalMethodDescriptor = {
    method: 'digitized-reference',
    simulationCurveSet: 'oil-rate',
    producesSweepPanels: false,
    nativeXAxis: 'time',
    definesCharacteristicTime: false,
    fallbackLayout: buildProductionBenchmarkLayout,
    slots: [],
    fromResult: null,
    fromParams: null,
    resolveOverlayMode: () => 'shared',
    panelPresentation: NO_OVERLAY_PRESENTATION,
    referenceLabel: 'Published benchmark reference curves',
};

const noMethod: AnalyticalMethodDescriptor = {
    method: 'none',
    simulationCurveSet: 'oil-rate',
    producesSweepPanels: false,
    nativeXAxis: 'time',
    definesCharacteristicTime: false,
    fallbackLayout: buildProductionBenchmarkLayout,
    slots: [],
    fromResult: null,
    fromParams: null,
    resolveOverlayMode: () => 'shared',
    panelPresentation: NO_OVERLAY_PRESENTATION,
    referenceLabel: 'Depletion reference solution',
};

/**
 * Sweep correlations (Craig areal, Dykstra-Parsons / Stiles vertical).
 *
 * Before this became a method of its own, sweep scenarios declared
 * `analyticalMethod: 'buckley-leverett'` — true of the correlation's internals,
 * since displacement inside the contacted region is BL — plus a `showSweepPanel`
 * capability flag. The stack then built BL water-cut and recovery reference
 * curves and a second mechanism stripped them out again, deciding what to strip
 * by scanning the chart layout for curve keys containing '-reference'.
 *
 * A sweep correlation answers "what fraction of the pattern is contacted", not
 * "what is the producing water cut at time t", so it declares no primary curve
 * slots. Nothing is built, so nothing needs stripping. The E_A / E_V / E_vol
 * panels are assembled separately by `sweepPanelBuilder.ts`, which owns the
 * geometry and Stiles/Dykstra-Parsons method choice.
 */
const sweep: AnalyticalMethodDescriptor = {
    method: 'sweep',
    simulationCurveSet: 'water-cut',
    producesSweepPanels: true,
    nativeXAxis: 'pvi',
    definesCharacteristicTime: false,
    fallbackLayout: buildProductionBenchmarkLayout,
    slots: [],
    fromResult: null,
    fromParams: null,
    resolveOverlayMode: () => 'per-result',
    panelPresentation: {
        rates: {
            title: 'Watercut',
            curveKeys: ['water-cut-sim'],
            scalePreset: 'breakthrough',
            allowLogToggle: false,
        },
        cumulative: {
            curveKeys: ['cum-oil-sim'],
        },
        diagnostics: {
            curveKeys: ['avg-pressure-sim'],
        },
    },
    referenceLabel: 'Sweep reference solution',
};

/**
 * Dry-gas material balance — the p/z straight line.
 *
 * Unlike every other method here, the reference is not a function of time: it
 * is a function of how much gas has been produced. That has two consequences
 * the descriptor encodes. The curve is evaluated on a completed run's own
 * cumulative production, so it lands correctly on whichever axis the chart is
 * showing without any remapping (`nativeXAxis: 'time'` disables it). And there
 * is no `fromParams` path: before a run exists there is no production history
 * to evaluate against, so the preview and pending contexts draw nothing rather
 * than inventing a schedule. The slots opt out of those two contexts to match.
 */
const gasMaterialBalance: AnalyticalMethodDescriptor = {
    method: 'gas-material-balance',
    simulationCurveSet: 'oil-rate',
    producesSweepPanels: false,
    nativeXAxis: 'time',
    definesCharacteristicTime: false,
    fallbackLayout: buildProductionBenchmarkLayout,
    slots: [
        {
            panelKey: 'pz',
            curveKey: 'p-over-z-reference',
            sharedLabel: 'Volumetric p/z Line',
            perCaseSuffix: ' Volumetric p/z Line',
            previewLabel: 'Volumetric p/z Line',
            contexts: ['shared', 'per-result'],
        },
        {
            panelKey: 'pz',
            curveKey: 'p-over-z-compaction-reference',
            sharedLabel: 'p/z With Compaction and Water Expansion',
            perCaseSuffix: ' p/z With Compaction',
            previewLabel: 'p/z With Compaction',
            contexts: ['shared', 'per-result'],
        },
        {
            panelKey: 'diagnostics',
            curveKey: 'avg-pressure-reference',
            sharedLabel: 'Material-Balance Average Pressure',
            perCaseSuffix: ' Material-Balance Pressure',
            previewLabel: 'Material-Balance Pressure',
            contexts: ['shared', 'per-result'],
        },
    ],
    fromResult: (result, derived, xAxisMode) => {
        const curves = computeGasMaterialBalanceCurves(result.params, derived.cumulativeGas);
        if (!curves) return null;
        return curveSet(buildXAxisValues(derived, xAxisMode), {
            'p-over-z-reference': curves.pOverZ,
            'p-over-z-compaction-reference': curves.pOverZCompactionCorrected,
            'avg-pressure-reference': curves.pressure,
        });
    },
    fromParams: null,
    // The straight line is built from each case's own gas in place and its own
    // gas law, both of which the sensitivity dimensions deliberately vary, so a
    // single shared curve would be wrong wherever it mattered most.
    resolveOverlayMode: () => 'per-result',
    panelPresentation: {
        rates: {
            title: 'Gas Rate',
            curveKeys: ['oil-rate-sim'],
            scalePreset: 'rates',
            allowLogToggle: false,
        },
        pz: {
            title: 'p/z',
            curveKeys: ['p-over-z-sim', 'p-over-z-reference', 'p-over-z-compaction-reference'],
            scalePreset: 'pressure',
        },
        diagnostics: {
            curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
        },
    },
    referenceLabel: 'Dry-gas material balance (p/z)',
};

export const ANALYTICAL_METHOD_DESCRIPTORS: Record<AnalyticalMethod, AnalyticalMethodDescriptor> = {
    'buckley-leverett': buckleyLeverett,
    'gas-oil-bl': gasOilBL,
    'sweep': sweep,
    'depletion': depletion,
    'well-test': wellTest,
    'gas-material-balance': gasMaterialBalance,
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
