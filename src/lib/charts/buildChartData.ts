/**
 * buildChartData.ts — assembles the full ReferenceComparisonModel from benchmark
 * run results and family configuration.
 *
 * Orchestrates: simulation curves → analytical reference overlays → sweep panels
 * → published reference series → panel map ready for ReferenceComparisonChart.
 *
 * Extraction map:
 *   types, palette, panel utils         → referenceChartTypes.ts
 *   BL / depletion / gas-oil overlays   → referenceOverlayBuilders.ts
 *   sweep panel builders                → sweepPanelBuilder.ts
 */

import type { BenchmarkFamily } from '../scenario/referenceTypes';
import type { CurveConfig } from './chartTypes';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import type { RateChartPanelKey, RateChartXAxisMode } from './rateChartLayoutConfig';
import {
    ANALYTICAL_BORDER, ANALYTICAL_DASH, AUXILIARY_DASH,
    LEGEND_SECTIONS, PUBLISHED_DASH, simBorderWidth,
} from './curveStylePolicy';
import {
    type DerivedRunSeries,
    buildXAxisValues,
    interpolateXAxisAtTimes,
    requiresRunMappedAnalyticalXAxis,
    buildAnalyticalAxisWarning,
} from './axisAdapters';
import {
    buildDerivedRunSeries,
    computeDepletionTau,
    computeMbeDiagnostics,
} from './analyticalParamAdapters';
import {
    getAnalyticalMethodDescriptor,
    slotsForContext,
    type AnalyticalCurveSet,
    type AnalyticalCurveSlot,
    type AnalyticalMethodDescriptor,
    type AnalyticalOverlayContext,
} from './analyticalMethodRegistry';
import {
    appendSeries,
    compactCaseLabel,
    createReferenceComparisonPanel,
    getLegendGrey,
    getReferenceColor,
    getReferenceComparisonCaseColor,
    type AnalyticalPreviewVariant,
    type ReferenceComparisonModel,
    type ReferenceComparisonPanel,
    type ReferenceComparisonPanelMap,
    type ReferenceComparisonPrimaryPanelMap,
    type ReferenceComparisonPreviewCase,
    type ReferenceComparisonSweepPanels,
    type ReferenceComparisonTheme,
} from './referenceChartTypes';
import { buildPreviewSweepPanels, buildSweepPanels } from './sweepPanelBuilder';
import type { AnalyticalMethod } from '../catalog/scenarios';

export { getReferenceComparisonCaseColor };
export type {
    AnalyticalPreviewVariant,
    ReferenceComparisonModel,
    ReferenceComparisonPanel,
    ReferenceComparisonPanelMap,
    ReferenceComparisonPreviewCase,
    ReferenceComparisonTheme,
};

// ─── Private helpers ──────────────────────────────────────────────────────────

function getBaseResult(results: BenchmarkRunResult[]): BenchmarkRunResult | null {
    return results.find((result) => result.variantKey === null) ?? results[0] ?? null;
}

function orderResults(
    results: BenchmarkRunResult[],
    variantOrder?: AnalyticalPreviewVariant[],
): BenchmarkRunResult[] {
    if (!variantOrder?.length) return [...results];
    const orderIndex = new Map(variantOrder.map((v, i) => [v.variantKey, i]));
    return [...results].sort((a, b) => {
        const ai = a.variantKey != null ? orderIndex.get(a.variantKey) ?? Infinity : -1;
        const bi = b.variantKey != null ? orderIndex.get(b.variantKey) ?? Infinity : -1;
        return ai - bi;
    });
}

function appendBhpLimitDiagnostics(
    panel: ReferenceComparisonPanel,
    input: {
        label: string;
        caseKey: string;
        toggleLabel: string;
        borderWidth: number;
        defaultVisible: boolean;
        xValues: Array<number | null>;
        producerValues: Array<number | null>;
        injectorValues: Array<number | null>;
    },
) {
    appendSeries(panel, {
        label: `${input.label} Producer BHP-limited`,
        curveKey: 'producer-bhp-limited-sim',
        caseKey: input.caseKey,
        toggleGroupKey: input.caseKey,
        toggleLabel: input.toggleLabel,
        legendSection: 'sim',
        legendSectionLabel: LEGEND_SECTIONS.sim,
        color: '#c2410c',
        borderWidth: input.borderWidth,
        yAxisID: 'y',
        defaultVisible: input.defaultVisible,
    }, input.xValues, input.producerValues);

    appendSeries(panel, {
        label: `${input.label} Injector BHP-limited`,
        curveKey: 'injector-bhp-limited-sim',
        caseKey: input.caseKey,
        toggleGroupKey: input.caseKey,
        toggleLabel: input.toggleLabel,
        legendSection: 'sim',
        legendSectionLabel: LEGEND_SECTIONS.sim,
        color: '#0369a1',
        borderWidth: input.borderWidth,
        yAxisID: 'y',
        defaultVisible: input.defaultVisible,
    }, input.xValues, input.injectorValues);
}

function appendPublishedReferenceSeries(
    panels: Record<RateChartPanelKey, ReferenceComparisonPanel>,
    family: BenchmarkFamily | null,
) {
    if (!family?.publishedReferenceSeries?.length) return;

    const publishedColor = '#e74c3c';
    const opmFlowColor = '#15803d';
    for (const series of family.publishedReferenceSeries) {
        const targetPanel = panels[series.panelKey as RateChartPanelKey];
        if (!targetPanel) continue;
        const isOpmFlow = series.sourceType === 'opm-flow-precomputed';
        // Prerun-artifacts scenarios render their bundled series as primary
        // (solid, prominent) content — there is no live simulation curve, the
        // artifact IS the exhibit — rather than a dashed reference overlay.
        const isPrimary = series.primary === true;
        appendSeries(targetPanel, {
            label: series.label,
            curveKey: series.curveKey,
            referenceSourceType: series.sourceType ?? 'published-reference',
            toggleGroupKey: isOpmFlow ? `opm-flow-${series.sourceArtifactKey ?? 'reference'}` : 'published-reference',
            toggleLabel: isOpmFlow ? 'OPM Flow reference' : 'Published reference',
            legendSection: 'published',
            legendSectionLabel: isOpmFlow ? 'OPM Flow reference' : LEGEND_SECTIONS.published,
            color: isOpmFlow ? opmFlowColor : publishedColor,
            borderWidth: isPrimary ? 2.5 : 1.5,
            borderDash: isPrimary ? undefined : PUBLISHED_DASH,
            yAxisID: series.yAxisID ?? 'y',
            pointRadius: 0,
        }, series.data.map((pt) => pt.x), series.data.map((pt) => pt.y));
    }
}

function emptyPanelMap(): ReferenceComparisonPanelMap {
    return {
        rates: createReferenceComparisonPanel(),
        recovery: createReferenceComparisonPanel(),
        cumulative: createReferenceComparisonPanel(),
        diagnostics: createReferenceComparisonPanel(),
        gor: createReferenceComparisonPanel(),
        volumes: createReferenceComparisonPanel(),
        oil_rate: createReferenceComparisonPanel(),
        injection_rate: createReferenceComparisonPanel(),
        producer_bhp: createReferenceComparisonPanel(),
        injector_bhp: createReferenceComparisonPanel(),
        control_limits: createReferenceComparisonPanel(),
        sweep_rf: null,
        sweep_areal: null,
        sweep_vertical: null,
        sweep_combined: null,
        sweep_combined_mobile_oil: null,
    };
}

function combinePanelMaps(input: {
    primary: ReferenceComparisonPrimaryPanelMap;
    sweep?: ReferenceComparisonSweepPanels;
}): ReferenceComparisonPanelMap {
    return {
        ...emptyPanelMap(),
        rates: input.primary.rates,
        recovery: input.primary.recovery,
        cumulative: input.primary.cumulative,
        diagnostics: input.primary.diagnostics,
        gor: input.primary.gor,
        volumes: input.primary.volumes,
        oil_rate: input.primary.oil_rate,
        injection_rate: input.primary.injection_rate,
        producer_bhp: input.primary.producer_bhp,
        injector_bhp: input.primary.injector_bhp,
        control_limits: input.primary.control_limits,
        sweep_rf: input.sweep?.rf ?? null,
        sweep_areal: input.sweep?.areal ?? null,
        sweep_vertical: input.sweep?.vertical ?? null,
        sweep_combined: input.sweep?.combined ?? null,
        sweep_combined_mobile_oil: input.sweep?.combinedMobileOil ?? null,
    };
}

function emptySweepPanels(): ReferenceComparisonSweepPanels {
    return { rf: null, areal: null, vertical: null, combined: null, combinedMobileOil: null };
}

/**
 * Emits one analytical curve per registry slot into `panels`.
 *
 * All four overlay contexts share this: they differ only in label shape, color
 * source and line weight, which the caller supplies. A slot whose curve the
 * method did not produce (null/absent in the curve set) is skipped, matching the
 * per-overlay-section null guards this replaced.
 */
function appendAnalyticalSlots(
    panels: Record<RateChartPanelKey, ReferenceComparisonPanel>,
    descriptor: AnalyticalMethodDescriptor,
    context: AnalyticalOverlayContext,
    curveSet: AnalyticalCurveSet,
    style: (slot: AnalyticalCurveSlot) => CurveConfig,
): void {
    for (const slot of slotsForContext(descriptor, context)) {
        const values = curveSet.values[slot.curveKey];
        if (!values) continue;
        appendSeries(panels[slot.panelKey], style(slot), curveSet.xValues, values);
    }
}

/**
 * Curve style for the `per-result` and `pending` contexts: one dashed curve per
 * case in that case's color, grouped under its own legend toggle.
 */
function perCaseAnalyticalStyle(input: {
    label: string;
    caseKey: string;
    color: string;
}): (slot: AnalyticalCurveSlot) => CurveConfig {
    const toggleLabel = compactCaseLabel(input.label);
    return (slot) => ({
        label: `${input.label} — Reference${slot.perCaseSuffix}`,
        curveKey: slot.curveKey,
        caseKey: input.caseKey,
        toggleGroupKey: input.caseKey + '__ref',
        toggleLabel,
        legendSection: 'analytical',
        legendSectionLabel: LEGEND_SECTIONS.analytical,
        color: input.color,
        borderWidth: 1.5,
        borderDash: ANALYTICAL_DASH,
        yAxisID: 'y',
    });
}

/**
 * Curve style for the `shared` context: a single neutral curve standing for
 * every case, valid only when the analytical physics is identical across them.
 */
function sharedAnalyticalStyle(input: {
    referenceColor: string;
    legendGrey: string;
}): (slot: AnalyticalCurveSlot) => CurveConfig {
    return (slot) => ({
        label: slot.sharedLabel,
        curveKey: slot.curveKey,
        toggleGroupKey: 'analytical-shared',
        toggleLabel: 'Analytical solution',
        legendSection: 'analytical',
        legendSectionLabel: LEGEND_SECTIONS.analytical,
        color: input.referenceColor,
        legendColor: input.legendGrey,
        borderWidth: ANALYTICAL_BORDER,
        borderDash: ANALYTICAL_DASH,
        yAxisID: 'y',
    });
}

/**
 * Builds analytical-only preview panels before any simulation results exist.
 * Multi-variant arrays produce one colored curve per variant; single-variant
 * arrays use the neutral reference color.
 */
function buildAnalyticalPreviewPanels(
    variants: AnalyticalPreviewVariant[],
    xAxisMode: RateChartXAxisMode,
    analyticalMethod: AnalyticalMethod,
    theme: ReferenceComparisonTheme,
): Record<RateChartPanelKey, ReferenceComparisonPanel> {
    const panels: Record<RateChartPanelKey, ReferenceComparisonPanel> = {
        rates: createReferenceComparisonPanel(),
        recovery: createReferenceComparisonPanel(),
        cumulative: createReferenceComparisonPanel(),
        diagnostics: createReferenceComparisonPanel(),
        gor: createReferenceComparisonPanel(),
        volumes: createReferenceComparisonPanel(),
        oil_rate: createReferenceComparisonPanel(),
        injection_rate: createReferenceComparisonPanel(),
        producer_bhp: createReferenceComparisonPanel(),
        injector_bhp: createReferenceComparisonPanel(),
        control_limits: createReferenceComparisonPanel(),
    };

    const descriptor = getAnalyticalMethodDescriptor(analyticalMethod);
    if (variants.length === 0 || !descriptor.fromParams) return panels;

    const multiVariant = variants.length > 1;
    const neutralColor = getReferenceColor(theme);
    const legendGrey = getLegendGrey(theme);
    const analyticalLabel = variants.length === 1
        ? 'Analytical solution'
        : `Analytical solution (${variants.length})`;

    variants.forEach((variant, index) => {
        const curveSet = descriptor.fromParams!(variant.params, xAxisMode);
        if (!curveSet) return;
        const color = multiVariant ? getReferenceComparisonCaseColor(index) : neutralColor;
        const prefix = multiVariant ? `${variant.label} — ` : '';
        const caseKey = multiVariant ? variant.variantKey : undefined;
        appendAnalyticalSlots(panels, descriptor, 'preview', curveSet, (slot) => ({
            label: `${prefix}${slot.previewLabel}`,
            curveKey: slot.curveKey,
            ...(caseKey ? { caseKey } : {}),
            toggleGroupKey: 'analytical',
            toggleLabel: analyticalLabel,
            color,
            legendColor: legendGrey,
            borderWidth: ANALYTICAL_BORDER,
            borderDash: ANALYTICAL_DASH,
            yAxisID: 'y',
        }));
    });

    return panels;
}

// ─── Main builder ─────────────────────────────────────────────────────────────

export function buildReferenceComparisonModel(input: {
    family: BenchmarkFamily | null | undefined;
    results: BenchmarkRunResult[];
    xAxisMode: RateChartXAxisMode;
    theme?: ReferenceComparisonTheme;
    /** True when the active sensitivity variants change parameters that feed the
     *  analytical solution (e.g. viscosity → fractional flow). Each result then
     *  gets its own analytical curve. False (default) → one shared reference. */
    analyticalPerVariant?: boolean;
    /**
     * When provided and no results exist yet, render one analytical curve per
     * variant so the user can see the spread before running any simulations.
     * Takes priority over previewBaseParams when non-empty.
     */
    previewVariantParams?: AnalyticalPreviewVariant[];
    /**
     * Variants whose simulations are still queued/running (results not yet in
     * `results`). Rendered as analytical-only dashed overlays alongside the
     * completed results so the chart never collapses from N preview curves to
     * fewer as the sweep progresses. Colors continue the case-color sequence
     * from where orderedResults leaves off so each variant keeps its color
     * throughout preview → in-progress → completed.
     */
    pendingPreviewVariants?: AnalyticalPreviewVariant[];
    /** Fallback single-curve preview (used when analyticalPerVariant is false). */
    previewBaseParams?: Record<string, any>;
    previewAnalyticalMethod?: AnalyticalMethod;
}): ReferenceComparisonModel {
    const family = input.family ?? null;
    const orderedResults = orderResults(input.results, input.previewVariantParams);
    const referenceColor = getReferenceColor(input.theme ?? 'dark');
    const legendGrey = getLegendGrey(input.theme ?? 'dark');
    const analyticalMethod = family?.analyticalMethod ?? input.previewAnalyticalMethod ?? null;
    const descriptor = getAnalyticalMethodDescriptor(analyticalMethod);
    const requestedOverlayMode = family?.analyticalOverlayMode ?? 'auto';
    const usesRunMappedAnalyticalXAxis = requiresRunMappedAnalyticalXAxis(
        analyticalMethod ? descriptor.nativeXAxis : null,
        input.xAxisMode,
    );
    const overlayMode = descriptor.resolveOverlayMode({
        requested: requestedOverlayMode,
        paramSets: [
            ...orderedResults.map((result) => result.params),
            ...(input.pendingPreviewVariants ?? []).map((variant) => variant.params),
        ],
        analyticalPerVariant: input.analyticalPerVariant,
    });
    let hidesPendingAnalyticalWithoutMapping = false;

    const panels: Record<RateChartPanelKey, ReferenceComparisonPanel> = {
        rates: createReferenceComparisonPanel(),
        recovery: createReferenceComparisonPanel(),
        cumulative: createReferenceComparisonPanel(),
        diagnostics: createReferenceComparisonPanel(),
        gor: createReferenceComparisonPanel(),
        volumes: createReferenceComparisonPanel(),
        oil_rate: createReferenceComparisonPanel(),
        injection_rate: createReferenceComparisonPanel(),
        producer_bhp: createReferenceComparisonPanel(),
        injector_bhp: createReferenceComparisonPanel(),
        control_limits: createReferenceComparisonPanel(),
    };

    if (!family || orderedResults.length === 0) {
        if (orderedResults.length === 0 && input.previewAnalyticalMethod) {
            if (requiresRunMappedAnalyticalXAxis(descriptor.nativeXAxis, input.xAxisMode)) {
                hidesPendingAnalyticalWithoutMapping = Boolean(
                    input.previewBaseParams || (input.previewVariantParams?.length ?? 0) > 0,
                );
                return {
                    orderedResults,
                    previewCases: [],
                    panels: (() => {
                        appendPublishedReferenceSeries(panels, family);
                        return combinePanelMaps({ primary: panels });
                    })(),
                    axisMappingWarning: buildAnalyticalAxisWarning({
                        usesRunMappedAnalyticalXAxis: false,
                        hidesPendingAnalyticalWithoutMapping,
                    }),
                };
            }
            // Prefer per-variant preview when available; fall back to single base preview.
            const variants: AnalyticalPreviewVariant[] =
                input.previewVariantParams?.length
                    ? input.previewVariantParams
                    : input.previewBaseParams
                        ? [{ label: '', variantKey: 'base', params: input.previewBaseParams }]
                        : [];
            if (variants.length > 0) {
                // A PVI-native solution that is identical across variants collapses
                // to a single neutral preview curve; time-native methods keep one
                // curve per variant because their solutions differ by construction.
                const analyticalPreviewVariants =
                    descriptor.nativeXAxis === 'pvi'
                        && !usesRunMappedAnalyticalXAxis
                        && overlayMode === 'shared'
                        ? [variants[0]]
                        : variants;
                const previewPanels = buildAnalyticalPreviewPanels(
                    analyticalPreviewVariants,
                    input.xAxisMode,
                    input.previewAnalyticalMethod,
                    input.theme ?? 'dark',
                );
                // Expose multi-variant preview entries so the cases selector can
                // render toggle buttons even before any simulations have completed.
                const previewCases: ReferenceComparisonPreviewCase[] = variants.length > 1
                    ? variants.map((v, i) => ({ key: v.variantKey, label: v.label, colorIndex: i }))
                    : [];
                return {
                    orderedResults,
                    previewCases,
                    panels: (() => {
                        appendPublishedReferenceSeries(previewPanels, family);
                        return combinePanelMaps({
                            primary: previewPanels,
                            sweep: descriptor.producesSweepPanels
                                ? buildPreviewSweepPanels({
                                    variants,
                                    theme: input.theme ?? 'dark',
                                    geometry: family?.sweepGeometry ?? 'both',
                                    method: family?.sweepAnalyticalMethod ?? 'dykstra-parsons',
                                })
                                : emptySweepPanels(),
                        });
                    })(),
                    axisMappingWarning: null,
                };
            }
        }
        appendPublishedReferenceSeries(panels, family);
        return {
            orderedResults,
            previewCases: [],
            panels: combinePanelMaps({ primary: panels }),
            axisMappingWarning: null,
        };
    }

    const derivedByKey = new Map<string, DerivedRunSeries>(
        orderedResults.map((result) => [result.key, buildDerivedRunSeries(result)]),
    );
    const baseResult = getBaseResult(orderedResults);

    orderedResults.forEach((result, index) => {
        const derived = derivedByKey.get(result.key);
        if (!derived) return;
        const color = getReferenceComparisonCaseColor(index);
        const tau = analyticalMethod === 'depletion' ? computeDepletionTau(result.params) : null;
        const xValues = buildXAxisValues(derived, input.xAxisMode, tau);
        const defaultVisible = true;
        const caseLabel = compactCaseLabel(result.label);

        if (descriptor.simulationCurveSet === 'water-cut') {
            appendSeries(panels.rates, {
                label: `${result.label} Water Cut`,
                curveKey: 'water-cut-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.waterCut);
            appendSeries(panels.rates, {
                label: `${result.label} Avg Water Sat`,
                curveKey: 'avg-water-sat',
                caseKey: result.key,
                // No toggleGroupKey override — falls back to curveKey so all cases
                // share one "Avg Sw" toggle, keeping it out of the per-case section.
                toggleLabel: 'Avg Sw',
                color,
                borderWidth: 1.6,
                borderDash: AUXILIARY_DASH,
                yAxisID: 'y',
                defaultVisible: false,
            }, xValues, derived.avgWaterSat);
            appendSeries(panels.recovery, {
                label: `${result.label} Recovery`,
                curveKey: 'recovery-factor-primary',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.recovery);
            appendSeries(panels.cumulative, {
                label: `${result.label} Cum Oil`,
                curveKey: 'cum-oil-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.cumulativeOil);
            appendSeries(panels.oil_rate, {
                label: `${result.label} Oil Rate`,
                curveKey: 'oil-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.oilRate);
            appendSeries(panels.injection_rate, {
                label: `${result.label} Injection Rate`,
                curveKey: 'injection-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.injectionRate);
            appendSeries(panels.volumes, {
                label: `${result.label} Cum Injection`,
                curveKey: 'cum-injection',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.cumulativeInjection);
            appendSeries(panels.diagnostics, {
                label: `${result.label} Avg Pressure`,
                curveKey: 'avg-pressure-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.pressure);
            appendSeries(panels.diagnostics, {
                label: `${result.label} P/z`,
                curveKey: 'p_z_sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.p_z);
            appendSeries(panels.gor, {
                label: `${result.label} GOR`,
                curveKey: 'gor-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.gor);
            return;
        }

        if (descriptor.simulationCurveSet === 'gas-cut') {
            const historyXAxis = interpolateXAxisAtTimes(derived.time, xValues, derived.historyTime);
            appendSeries(panels.rates, {
                label: `${result.label} Gas Cut`,
                curveKey: 'gas-cut-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.gasCut);
            appendSeries(panels.recovery, {
                label: `${result.label} Recovery`,
                curveKey: 'recovery-factor-primary',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.recovery);
            appendSeries(panels.cumulative, {
                label: `${result.label} Cum Oil`,
                curveKey: 'cum-oil-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.cumulativeOil);
            appendSeries(panels.oil_rate, {
                label: `${result.label} Oil Rate`,
                curveKey: 'oil-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.oilRate);
            appendSeries(panels.injection_rate, {
                label: `${result.label} Injection Rate`,
                curveKey: 'injection-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.injectionRate);
            appendSeries(panels.volumes, {
                label: `${result.label} Cum Injection`,
                curveKey: 'cum-injection',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.cumulativeInjection);
            appendSeries(panels.diagnostics, {
                label: `${result.label} Avg Pressure`,
                curveKey: 'avg-pressure-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.pressure);
            appendSeries(panels.diagnostics, {
                label: `${result.label} P/z`,
                curveKey: 'p_z_sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.p_z);
            appendSeries(panels.gor, {
                label: `${result.label} GOR`,
                curveKey: 'gor-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.gor);
            appendSeries(panels.producer_bhp, {
                label: `${result.label} Producer WBHP`,
                curveKey: 'producer-bhp-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, historyXAxis, derived.producerBhp);
            appendSeries(panels.injector_bhp, {
                label: `${result.label} Injector WBHP`,
                curveKey: 'injector-bhp-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, historyXAxis, derived.injectorBhp);
            appendBhpLimitDiagnostics(panels.control_limits, {
                label: result.label,
                caseKey: result.key,
                toggleLabel: caseLabel,
                borderWidth: simBorderWidth(result.variantKey),
                defaultVisible,
                xValues,
                producerValues: derived.producerBhpLimitedFraction,
                injectorValues: derived.injectorBhpLimitedFraction,
            });
            return;
        }

        // simulationCurveSet 'oil-rate': standard oil-rate + pressure panels.
        appendSeries(panels.rates, {
            label: `${result.label} Oil Rate`,
            curveKey: 'oil-rate-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible,
        }, xValues, derived.oilRate);
        appendSeries(panels.recovery, {
            label: `${result.label} Recovery`,
            curveKey: 'recovery-factor-primary',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible,
        }, xValues, derived.recovery);
        appendSeries(panels.cumulative, {
            label: `${result.label} Cum Oil`,
            curveKey: 'cum-oil-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
        }, xValues, derived.cumulativeOil);
        appendSeries(panels.oil_rate, {
            label: `${result.label} Oil Rate`,
            curveKey: 'oil-rate-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible,
        }, xValues, derived.oilRate);
        appendSeries(panels.injection_rate, {
            label: `${result.label} Injection Rate`,
            curveKey: 'injection-rate-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible,
        }, xValues, derived.injectionRate);
        appendSeries(panels.diagnostics, {
            label: `${result.label} Avg Pressure`,
            curveKey: 'avg-pressure-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible,
        }, xValues, derived.pressure);
        appendSeries(panels.diagnostics, {
            label: `${result.label} P/z`,
            curveKey: 'p_z_sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible,
        }, xValues, derived.p_z);
        appendSeries(panels.gor, {
            label: `${result.label} GOR`,
            curveKey: 'gor-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible,
        }, xValues, derived.gor);
        const historyXAxis = interpolateXAxisAtTimes(derived.time, xValues, derived.historyTime);
        appendSeries(panels.producer_bhp, {
            label: `${result.label} Producer WBHP`,
            curveKey: 'producer-bhp-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible,
        }, historyXAxis, derived.producerBhp);
        appendSeries(panels.injector_bhp, {
            label: `${result.label} Injector WBHP`,
            curveKey: 'injector-bhp-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible,
        }, historyXAxis, derived.injectorBhp);
        appendBhpLimitDiagnostics(panels.control_limits, {
            label: result.label,
            caseKey: result.key,
            toggleLabel: caseLabel,
            borderWidth: simBorderWidth(result.variantKey),
            defaultVisible,
            xValues,
            producerValues: derived.producerBhpLimitedFraction,
            injectorValues: derived.injectorBhpLimitedFraction,
        });

        // ── MBE diagnostics (Havlena-Odeh) ─────────────────────────────────
        if (analyticalMethod === 'depletion') {
            const mbe = computeMbeDiagnostics(result, derived);
            appendSeries(panels.diagnostics, {
                label: `${result.label} MBE OOIP Ratio`,
                curveKey: 'mbe-ooip-ratio',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: 1.6,
                borderDash: [2, 3],
                yAxisID: 'y1',
                defaultVisible: false,
            }, xValues, mbe.ooipRatio);

            // ── Drive mechanism indices ─────────────────────────────────────
            appendSeries(panels.diagnostics, {
                label: `${result.label} Drive: Compaction`,
                curveKey: 'drive-compaction',
                caseKey: result.key,
                toggleGroupKey: `${result.key}-drive`,
                toggleLabel: caseLabel,
                legendSection: 'drive',
                legendSectionLabel: LEGEND_SECTIONS.driveIndices,
                color: '#e67e22',
                borderWidth: 1.4,
                yAxisID: 'y1',
                defaultVisible: false,
            }, xValues, mbe.driveCompaction);
            appendSeries(panels.diagnostics, {
                label: `${result.label} Drive: Oil Expansion`,
                curveKey: 'drive-oil-expansion',
                caseKey: result.key,
                toggleGroupKey: `${result.key}-drive`,
                toggleLabel: caseLabel,
                legendSection: 'drive',
                legendSectionLabel: LEGEND_SECTIONS.driveIndices,
                color: '#27ae60',
                borderWidth: 1.4,
                yAxisID: 'y1',
                defaultVisible: false,
            }, xValues, mbe.driveOilExpansion);
            appendSeries(panels.diagnostics, {
                label: `${result.label} Drive: Gas Cap`,
                curveKey: 'drive-gas-cap',
                caseKey: result.key,
                toggleGroupKey: `${result.key}-drive`,
                toggleLabel: caseLabel,
                legendSection: 'drive',
                legendSectionLabel: LEGEND_SECTIONS.driveIndices,
                color: '#2980b9',
                borderWidth: 1.4,
                yAxisID: 'y1',
                defaultVisible: false,
            }, xValues, mbe.driveGasCap);
        }
    });

    if (!baseResult) {
        return {
            orderedResults,
            previewCases: [],
            panels: combinePanelMaps({ primary: panels }),
            axisMappingWarning: buildAnalyticalAxisWarning({
                usesRunMappedAnalyticalXAxis,
                hidesPendingAnalyticalWithoutMapping,
            }),
        };
    }

    const baseDerived = derivedByKey.get(baseResult.key);
    if (!baseDerived) {
        return {
            orderedResults,
            previewCases: [],
            panels: combinePanelMaps({ primary: panels }),
            axisMappingWarning: buildAnalyticalAxisWarning({
                usesRunMappedAnalyticalXAxis,
                hidesPendingAnalyticalWithoutMapping,
            }),
        };
    }

    // ── Analytical reference overlays ──────────────────────────────────────
    // Registry-driven (analyticalMethodRegistry.ts): the method descriptor owns
    // which reference curves exist, which panel each lands in, and whether one
    // shared curve or one curve per case is the correct representation. A method
    // with no reference solution ('none', 'digitized-reference') has no slots and
    // therefore emits nothing — it does not fall through to another method.
    if (descriptor.fromResult) {
        const useSharedOverlay = overlayMode === 'shared' && !usesRunMappedAnalyticalXAxis;

        if (useSharedOverlay) {
            // One curve for all cases — valid only when the analytical physics is
            // identical across them and no per-run axis remapping is needed.
            const curveSet = descriptor.fromResult(baseResult, baseDerived, input.xAxisMode);
            if (curveSet) {
                appendAnalyticalSlots(
                    panels,
                    descriptor,
                    'shared',
                    curveSet,
                    sharedAnalyticalStyle({ referenceColor, legendGrey }),
                );
            }
        } else {
            // Per-result — either the analytical physics differs by case, or the
            // selected x-axis requires remapping the solution onto each completed
            // run's own time/injection history.
            orderedResults.forEach((result, index) => {
                const derived = derivedByKey.get(result.key);
                if (!derived) return;
                const curveSet = descriptor.fromResult!(result, derived, input.xAxisMode);
                if (!curveSet) return;
                appendAnalyticalSlots(
                    panels,
                    descriptor,
                    'per-result',
                    curveSet,
                    perCaseAnalyticalStyle({
                        label: result.label,
                        caseKey: result.key,
                        color: getReferenceComparisonCaseColor(index),
                    }),
                );
            });

            // Analytical-only overlay for variants still queued/running. Color
            // indices continue from orderedResults.length so each variant keeps
            // the same color from preview → in-progress → completed.
            if (input.pendingPreviewVariants?.length) {
                if (usesRunMappedAnalyticalXAxis) {
                    hidesPendingAnalyticalWithoutMapping = true;
                } else if (descriptor.fromParams) {
                    input.pendingPreviewVariants.forEach((variant, i) => {
                        const curveSet = descriptor.fromParams!(variant.params, input.xAxisMode);
                        if (!curveSet) return;
                        appendAnalyticalSlots(
                            panels,
                            descriptor,
                            'pending',
                            curveSet,
                            perCaseAnalyticalStyle({
                                label: variant.label,
                                caseKey: variant.variantKey,
                                color: getReferenceComparisonCaseColor(orderedResults.length + i),
                            }),
                        );
                    });
                }
            }
        }
    }

    // Pending preview cases for mid-sweep: variants whose results haven't landed yet.
    // These appear in the cases selector alongside completed orderedResults entries.
    // Color indices use declaration order from previewVariantParams so each variant
    // keeps the same color throughout preview → in-progress → completed.
    const pendingPreviewCases: ReferenceComparisonPreviewCase[] =
        (input.pendingPreviewVariants?.length
            && ((input.analyticalPerVariant && !usesRunMappedAnalyticalXAxis)
                || descriptor.producesSweepPanels))
            ? (() => {
                const declOrder = new Map(
                    (input.previewVariantParams ?? []).map((v, i) => [v.variantKey, i]),
                );
                return input.pendingPreviewVariants!.map((v) => ({
                    key: v.variantKey,
                    label: v.label,
                    colorIndex: declOrder.get(v.variantKey) ?? orderedResults.length,
                }));
            })()
            : [];

    const sweepPanels = descriptor.producesSweepPanels
        ? buildSweepPanels({
            orderedResults,
            theme: input.theme ?? 'dark',
            pendingPreviewVariants: input.pendingPreviewVariants,
            previewVariantParams: input.previewVariantParams,
            xAxisMode: input.xAxisMode,
            derivedByKey,
            geometry: family.sweepGeometry ?? 'both',
            method: family.sweepAnalyticalMethod ?? 'dykstra-parsons',
        })
        : emptySweepPanels();

    // ── Published reference overlays (static benchmark data) ────────────────
    appendPublishedReferenceSeries(panels, family);

    return {
        orderedResults,
        previewCases: pendingPreviewCases,
        panels: combinePanelMaps({ primary: panels, sweep: sweepPanels }),
        axisMappingWarning: buildAnalyticalAxisWarning({
            usesRunMappedAnalyticalXAxis,
            hidesPendingAnalyticalWithoutMapping,
        }),
    };
}
