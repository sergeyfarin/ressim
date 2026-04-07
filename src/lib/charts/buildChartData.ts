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

import type { BenchmarkFamily } from '../catalog/benchmarkCases';
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
    computeBLAnalyticalFromParams,
    computeDepletionAnalyticalFromParams,
    computeDepletionTau,
    computeGasOilBLAnalyticalFromParams,
    computeMbeDiagnostics,
    hasDistinctBuckleyLeverettOverlays,
    hasDistinctGasOilBLOverlays,
    resolveOverlayMode,
} from './analyticalParamAdapters';
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
import {
    buildBuckleyLeverettReference,
    buildDepletionReference,
    buildGasOilBLReference,
} from './referenceOverlayBuilders';
import { buildPreviewSweepPanels, buildSweepPanels } from './sweepPanelBuilder';

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
    for (const series of family.publishedReferenceSeries) {
        const targetPanel = panels[series.panelKey as RateChartPanelKey];
        if (!targetPanel) continue;
        appendSeries(targetPanel, {
            label: series.label,
            curveKey: series.curveKey,
            toggleGroupKey: 'published-reference',
            toggleLabel: 'Published reference',
            legendSection: 'published',
            legendSectionLabel: LEGEND_SECTIONS.published,
            color: publishedColor,
            borderWidth: 1.5,
            borderDash: PUBLISHED_DASH,
            yAxisID: series.yAxisID ?? 'y',
            pointRadius: 0,
        }, series.data.map((pt) => pt.x), series.data.map((pt) => pt.y));
    }
}

function stripReferenceCurveKeys(
    panel: ReferenceComparisonPanel,
    excludedCurveKeys: Set<string>,
): ReferenceComparisonPanel {
    const keptEntries = panel.curves
        .map((curve, index) => ({ curve, series: panel.series[index] ?? [] }))
        .filter((entry) => !excludedCurveKeys.has(entry.curve.curveKey ?? entry.curve.label));
    return {
        curves: keptEntries.map((entry) => entry.curve),
        series: keptEntries.map((entry) => entry.series),
    };
}

function suppressPrimaryAnalyticalPanels(
    panels: Record<RateChartPanelKey, ReferenceComparisonPanel>,
): Record<RateChartPanelKey, ReferenceComparisonPanel> {
    const excludedCurveKeys = new Set([
        'oil-rate-reference',
        'water-cut-reference',
        'gas-cut-reference',
        'recovery-factor-reference',
        'cum-oil-reference',
        'avg-pressure-reference',
        'p_z_reference',
    ]);
    return {
        rates: stripReferenceCurveKeys(panels.rates, excludedCurveKeys),
        recovery: stripReferenceCurveKeys(panels.recovery, excludedCurveKeys),
        cumulative: stripReferenceCurveKeys(panels.cumulative, excludedCurveKeys),
        diagnostics: stripReferenceCurveKeys(panels.diagnostics, excludedCurveKeys),
        gor: panels.gor,
        volumes: panels.volumes,
        oil_rate: stripReferenceCurveKeys(panels.oil_rate, excludedCurveKeys),
        injection_rate: panels.injection_rate,
        producer_bhp: panels.producer_bhp,
        injector_bhp: panels.injector_bhp,
        control_limits: panels.control_limits,
    };
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
 * Builds analytical-only preview panels before any simulation results exist.
 * Multi-variant arrays produce one colored curve per variant; single-variant
 * arrays use the neutral reference color.
 */
function buildAnalyticalPreviewPanels(
    variants: AnalyticalPreviewVariant[],
    xAxisMode: RateChartXAxisMode,
    analyticalMethod: string,
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

    if (variants.length === 0) return panels;

    const multiVariant = variants.length > 1;
    const neutralColor = getReferenceColor(theme);
    const legendGrey = getLegendGrey(theme);
    const getColor = (index: number) =>
        multiVariant ? getReferenceComparisonCaseColor(index) : neutralColor;
    const labelPrefix = (variant: AnalyticalPreviewVariant) =>
        multiVariant ? `${variant.label} — ` : '';
    const analyticalLabel = variants.length === 1
        ? 'Analytical solution'
        : `Analytical solution (${variants.length})`;

    if (analyticalMethod === 'buckley-leverett' || analyticalMethod === 'waterflood') {
        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;
            const curves = computeBLAnalyticalFromParams(variant.params);
            if (!curves) return;
            appendSeries(panels.rates, {
                label: `${prefix}Analytical Water Cut`,
                curveKey: 'water-cut-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.waterCut);
            appendSeries(panels.recovery, {
                label: `${prefix}Analytical Recovery Factor`,
                curveKey: 'recovery-factor-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.recovery);
        });
        return panels;
    }

    if (analyticalMethod === 'depletion') {
        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;
            const curves = computeDepletionAnalyticalFromParams(variant.params, xAxisMode);
            if (!curves) return;
            appendSeries(panels.rates, {
                label: `${prefix}Analytical Oil Rate`,
                curveKey: 'oil-rate-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.oilRates);
            appendSeries(panels.recovery, {
                label: `${prefix}Analytical Recovery Factor`,
                curveKey: 'recovery-factor-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.recoveryValues);
            appendSeries(panels.cumulative, {
                label: `${prefix}Analytical Cum Oil`,
                curveKey: 'cum-oil-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.cumulativeOilValues);
            appendSeries(panels.diagnostics, {
                label: `${prefix}Analytical Avg Pressure`,
                curveKey: 'avg-pressure-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.avgPressureValues);
        });
        return panels;
    }

    if (analyticalMethod === 'gas-oil-bl') {
        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;
            const curves = computeGasOilBLAnalyticalFromParams(variant.params);
            if (!curves) return;
            appendSeries(panels.rates, {
                label: `${prefix}Analytical Gas Cut`,
                curveKey: 'gas-cut-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.pviValues, curves.gasCut);
            appendSeries(panels.recovery, {
                label: `${prefix}Analytical Recovery Factor`,
                curveKey: 'recovery-factor-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.pviValues, curves.recovery);
        });
        return panels;
    }

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
    previewAnalyticalMethod?: string;
}): ReferenceComparisonModel {
    const family = input.family ?? null;
    const suppressPrimaryAnalyticalOverlays = family?.suppressPrimaryAnalyticalOverlays
        ?? (family?.showSweepPanel === true);
    const orderedResults = orderResults(input.results, input.previewVariantParams);
    const referenceColor = getReferenceColor(input.theme ?? 'dark');
    const legendGrey = getLegendGrey(input.theme ?? 'dark');
    const analyticalMethod = family?.analyticalMethod ?? input.previewAnalyticalMethod ?? null;
    const requestedOverlayMode = family?.analyticalOverlayMode ?? 'auto';
    const usesRunMappedAnalyticalXAxis = requiresRunMappedAnalyticalXAxis(
        analyticalMethod,
        input.xAxisMode,
    );
    const distinctBuckleyLeverettOverlays = hasDistinctBuckleyLeverettOverlays([
        ...orderedResults.map((result) => result.params),
        ...(input.pendingPreviewVariants ?? []).map((variant) => variant.params),
    ]);
    const distinctGasOilBLOverlays = hasDistinctGasOilBLOverlays([
        ...orderedResults.map((result) => result.params),
        ...(input.pendingPreviewVariants ?? []).map((variant) => variant.params),
    ]);
    const buckleyLeverettOverlayMode = resolveOverlayMode({
        requested: requestedOverlayMode,
        distinctByPhysics: distinctBuckleyLeverettOverlays,
    });
    const gasOilOverlayMode = resolveOverlayMode({
        requested: requestedOverlayMode,
        distinctByPhysics: distinctGasOilBLOverlays,
    });
    const depletionOverlayMode = resolveOverlayMode({
        requested: requestedOverlayMode,
        distinctByPhysics: false,
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
            if (requiresRunMappedAnalyticalXAxis(input.previewAnalyticalMethod, input.xAxisMode)) {
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
                const analyticalPreviewVariants = input.previewAnalyticalMethod === 'buckley-leverett'
                    && !usesRunMappedAnalyticalXAxis
                    && buckleyLeverettOverlayMode === 'shared'
                    ? [variants[0]]
                    : input.previewAnalyticalMethod === 'gas-oil-bl'
                    && !usesRunMappedAnalyticalXAxis
                    && gasOilOverlayMode === 'shared'
                    ? [variants[0]]
                    : variants;
                const previewPanels = suppressPrimaryAnalyticalOverlays
                    ? panels
                    : buildAnalyticalPreviewPanels(
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
                        const primaryPanels = suppressPrimaryAnalyticalOverlays
                            ? suppressPrimaryAnalyticalPanels(previewPanels)
                            : previewPanels;
                        appendPublishedReferenceSeries(primaryPanels, family);
                        return combinePanelMaps({
                            primary: primaryPanels,
                            sweep: family?.showSweepPanel === true
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

        if (family.analyticalMethod === 'buckley-leverett') {
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

        if (family.analyticalMethod === 'gas-oil-bl') {
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

        // Depletion (and any future method): standard oil-rate + pressure panels.
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

    if (family.analyticalMethod === 'buckley-leverett' && !suppressPrimaryAnalyticalOverlays) {
        const allSameAnalytical = buckleyLeverettOverlayMode === 'shared';

        if (allSameAnalytical && !usesRunMappedAnalyticalXAxis) {
            // Shared reference — one curve for all (analytical is grid/timestep-independent).
            const refOverlay = buildBuckleyLeverettReference(baseResult, baseDerived, input.xAxisMode);
            if (refOverlay.rates) {
                appendSeries(panels.rates, {
                    label: 'Reference Solution Water Cut',
                    curveKey: 'water-cut-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.rates.values);
            }
            if (refOverlay.cumulative) {
                appendSeries(panels.recovery, {
                    label: 'Reference Solution Recovery',
                    curveKey: 'recovery-factor-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
            }
        } else {
            // Per-result analytical — either the analytical physics differs by case,
            // or the selected x-axis requires remapping the same PVI solution onto
            // each completed run's own time/injection history.
            orderedResults.forEach((result, index) => {
                const derived = derivedByKey.get(result.key);
                if (!derived) return;
                const color = getReferenceComparisonCaseColor(index);
                const caseLabel = compactCaseLabel(result.label);
                const refOverlay = buildBuckleyLeverettReference(result, derived, input.xAxisMode);
                if (refOverlay.rates) {
                    appendSeries(panels.rates, {
                        label: `${result.label} — Reference`,
                        curveKey: 'water-cut-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.rates.values);
                }
                if (refOverlay.cumulative) {
                    appendSeries(panels.recovery, {
                        label: `${result.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                }
            });

            // Analytical-only overlay for variants still queued/running.
            // Color indices continue from orderedResults.length so each variant
            // keeps the same color from initial preview → in-progress → completed.
            if (input.pendingPreviewVariants?.length) {
                if (usesRunMappedAnalyticalXAxis) {
                    hidesPendingAnalyticalWithoutMapping = true;
                }
                if (!usesRunMappedAnalyticalXAxis) {
                    input.pendingPreviewVariants.forEach((variant, i) => {
                        const color = getReferenceComparisonCaseColor(orderedResults.length + i);
                        const curves = computeBLAnalyticalFromParams(variant.params);
                        if (!curves) return;
                        const vLabel = compactCaseLabel(variant.label);
                        appendSeries(panels.rates, {
                            label: `${variant.label} — Reference`,
                            curveKey: 'water-cut-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.xValues, curves.waterCut);
                        appendSeries(panels.recovery, {
                            label: `${variant.label} — Reference Recovery`,
                            curveKey: 'recovery-factor-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.xValues, curves.recovery);
                    });
                }
            }
        }
    } else if (family.analyticalMethod === 'gas-oil-bl') {
        const allSameAnalytical = gasOilOverlayMode === 'shared';

        if (allSameAnalytical && !usesRunMappedAnalyticalXAxis) {
            const refOverlay = buildGasOilBLReference(baseResult, baseDerived, input.xAxisMode);
            if (refOverlay.rates) {
                appendSeries(panels.rates, {
                    label: 'Reference Solution Gas Cut',
                    curveKey: 'gas-cut-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.rates.values);
            }
            if (refOverlay.cumulative) {
                appendSeries(panels.recovery, {
                    label: 'Reference Solution Recovery',
                    curveKey: 'recovery-factor-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                appendSeries(panels.cumulative, {
                    label: 'Reference Solution Cum Oil',
                    curveKey: 'cum-oil-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.cumulativeValues);
            }
        } else {
            orderedResults.forEach((result, index) => {
                const derived = derivedByKey.get(result.key);
                if (!derived) return;
                const color = getReferenceComparisonCaseColor(index);
                const caseLabel = compactCaseLabel(result.label);
                const refOverlay = buildGasOilBLReference(result, derived, input.xAxisMode);
                if (refOverlay.rates) {
                    appendSeries(panels.rates, {
                        label: `${result.label} — Reference`,
                        curveKey: 'gas-cut-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.rates.values);
                }
                if (refOverlay.cumulative) {
                    appendSeries(panels.recovery, {
                        label: `${result.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                }
            });

            if (input.pendingPreviewVariants?.length) {
                if (usesRunMappedAnalyticalXAxis) {
                    hidesPendingAnalyticalWithoutMapping = true;
                }
                if (!usesRunMappedAnalyticalXAxis) {
                    input.pendingPreviewVariants.forEach((variant, i) => {
                        const color = getReferenceComparisonCaseColor(orderedResults.length + i);
                        const curves = computeGasOilBLAnalyticalFromParams(variant.params);
                        if (!curves) return;
                        const vLabel = compactCaseLabel(variant.label);
                        appendSeries(panels.rates, {
                            label: `${variant.label} — Reference`,
                            curveKey: 'gas-cut-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.pviValues, curves.gasCut);
                        appendSeries(panels.recovery, {
                            label: `${variant.label} — Reference Recovery`,
                            curveKey: 'recovery-factor-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.pviValues, curves.recovery);
                    });
                }
            }
        }
    } else {
        // Depletion path.
        if (depletionOverlayMode === 'per-result' || usesRunMappedAnalyticalXAxis) {
            // Per-result analytical — each variant gets its own dashed reference curve
            // in its case color so the user can directly compare sim vs. analytical.
            orderedResults.forEach((result, index) => {
                const derived = derivedByKey.get(result.key);
                if (!derived) return;
                const color = getReferenceComparisonCaseColor(index);
                const caseLabel = compactCaseLabel(result.label);
                const refOverlay = buildDepletionReference(result, derived, input.xAxisMode);
                if (refOverlay.rates) {
                    appendSeries(panels.rates, {
                        label: `${result.label} — Reference`,
                        curveKey: 'oil-rate-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.rates.values);
                }
                if (refOverlay.cumulative) {
                    appendSeries(panels.recovery, {
                        label: `${result.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                    appendSeries(panels.cumulative, {
                        label: `${result.label} — Reference Cum Oil`,
                        curveKey: 'cum-oil-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.cumulativeValues);
                }
                if (refOverlay.diagnostics) {
                    appendSeries(panels.diagnostics, {
                        label: `${result.label} — Reference Pressure`,
                        curveKey: 'avg-pressure-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.diagnostics.values);
                }
            });

            // Analytical-only overlay for variants still queued/running (pending).
            if (input.pendingPreviewVariants?.length) {
                if (usesRunMappedAnalyticalXAxis) {
                    hidesPendingAnalyticalWithoutMapping = true;
                }
                if (!usesRunMappedAnalyticalXAxis) {
                    input.pendingPreviewVariants.forEach((variant, i) => {
                        const color = getReferenceComparisonCaseColor(orderedResults.length + i);
                        const curves = computeDepletionAnalyticalFromParams(variant.params, input.xAxisMode);
                        if (!curves) return;
                        const vLabel = compactCaseLabel(variant.label);
                        appendSeries(panels.rates, {
                            label: `${variant.label} — Reference`,
                            curveKey: 'oil-rate-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.xValues, curves.oilRates);
                        appendSeries(panels.recovery, {
                            label: `${variant.label} — Reference Recovery`,
                            curveKey: 'recovery-factor-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.xValues, curves.recoveryValues);
                        appendSeries(panels.cumulative, {
                            label: `${variant.label} — Reference Cum Oil`,
                            curveKey: 'cum-oil-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.xValues, curves.cumulativeOilValues);
                        appendSeries(panels.diagnostics, {
                            label: `${variant.label} — Reference Pressure`,
                            curveKey: 'avg-pressure-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.xValues, curves.avgPressureValues);
                    });
                }
            }
        } else {
            // Shared reference from base result — one curve for all cases.
            const refOverlay = buildDepletionReference(baseResult, baseDerived, input.xAxisMode);
            if (refOverlay.rates) {
                appendSeries(panels.rates, {
                    label: refOverlay.rates.label,
                    curveKey: 'oil-rate-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.rates.values);
            }
            if (refOverlay.cumulative) {
                appendSeries(panels.recovery, {
                    label: refOverlay.cumulative.recoveryLabel,
                    curveKey: 'recovery-factor-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                appendSeries(panels.cumulative, {
                    label: refOverlay.cumulative.cumulativeLabel,
                    curveKey: 'cum-oil-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.cumulativeValues);
            }
            if (refOverlay.diagnostics) {
                appendSeries(panels.diagnostics, {
                    label: refOverlay.diagnostics.label,
                    curveKey: 'avg-pressure-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.diagnostics.values);
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
                || family.showSweepPanel === true))
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

    const sweepPanels = (family.showSweepPanel === true)
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

    const visiblePanels = suppressPrimaryAnalyticalOverlays
        ? suppressPrimaryAnalyticalPanels(panels)
        : panels;

    // ── Published reference overlays (static benchmark data) ────────────────
    appendPublishedReferenceSeries(visiblePanels, family);

    return {
        orderedResults,
        previewCases: pendingPreviewCases,
        panels: combinePanelMaps({ primary: visiblePanels, sweep: sweepPanels }),
        axisMappingWarning: buildAnalyticalAxisWarning({
            usesRunMappedAnalyticalXAxis,
            hidesPendingAnalyticalWithoutMapping,
        }),
    };
}
