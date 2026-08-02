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
import type { ChartPanelKey, ChartXAxisMode } from './chartLayoutConfig';
import {
    ANALYTICAL_BORDER, ANALYTICAL_BORDER_MULTI, ANALYTICAL_DASH,
    LEGEND_SECTIONS, REFERENCE_STYLE, REFERENCE_STYLE_PRIMARY,
    SIM_BORDER_SECONDARY, simBorderWidth,
} from './curveStylePolicy';
import {
    type DerivedRunSeries,
    buildXAxisValues,
    interpolateXAxisAtTimes,
    mapReferenceTimesToXAxis,
    requiresRunMappedAnalyticalXAxis,
    buildAnalyticalAxisWarning,
} from './axisAdapters';
import {
    buildDerivedRunSeries,
    computeDietzPssSimulationDiagnostics,
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
import { simulationCurvesForSet, resolveSimulationCurve } from './simulationCurves';
import { DEFAULT_SWEEP_METHOD } from '../analytical/sweepMethods';
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
    panels: Record<ChartPanelKey, ReferenceComparisonPanel>,
    family: BenchmarkFamily | null,
    xAxisMode: ChartXAxisMode,
) {
    if (!family?.publishedReferenceSeries?.length) return;

    const publishedColor = '#e74c3c';
    const opmFlowColor = '#15803d';
    for (const series of family.publishedReferenceSeries) {
        const targetPanel = panels[series.panelKey as ChartPanelKey];
        if (!targetPanel) continue;
        // Reference data is recorded against days. On an injection-based axis
        // it has to be converted through the reference run's own mapping, and
        // where that is impossible the series is dropped rather than drawn at
        // an x it does not have.
        const xValues = mapReferenceTimesToXAxis(
            series.data.map((pt) => pt.x),
            xAxisMode,
            series.xAxisMap,
        );
        if (xValues === null) continue;
        const isOpmFlow = series.sourceType === 'opm-flow-precomputed';
        // Prerun-artifacts scenarios render their bundled series as prominent
        // content — there is no live simulation curve, the artifact IS the
        // exhibit. Prominence is width only: the curve stays dotted, because a
        // solid line means ResSim and an external reference must never read as
        // one. See curveStylePolicy.ts.
        const isPrimary = series.primary === true;
        appendSeries(targetPanel, {
            label: series.label,
            curveKey: series.curveKey,
            referenceSourceType: series.sourceType ?? 'published-reference',
            toggleGroupKey: isOpmFlow ? `opm-flow-${series.sourceArtifactKey ?? 'reference'}` : 'published-reference',
            toggleLabel: isOpmFlow ? 'OPM Flow reference' : 'Published reference',
            legendSection: 'published',
            legendSectionLabel: isOpmFlow ? 'OPM Flow reference (dotted lines):' : LEGEND_SECTIONS.published,
            color: isOpmFlow ? opmFlowColor : publishedColor,
            ...(isPrimary ? REFERENCE_STYLE_PRIMARY : REFERENCE_STYLE),
            yAxisID: series.yAxisID ?? 'y',
            pointRadius: 0,
            defaultVisible: series.defaultVisible,
        }, xValues, series.data.map((pt) => pt.y));
    }
}

function emptyPanelMap(): ReferenceComparisonPanelMap {
    return {
        rates: createReferenceComparisonPanel(),
        recovery: createReferenceComparisonPanel(),
        cumulative: createReferenceComparisonPanel(),
        diagnostics: createReferenceComparisonPanel(),
        avg_water_sat: createReferenceComparisonPanel(),
        mbe_ooip: createReferenceComparisonPanel(),
        drive_indices: createReferenceComparisonPanel(),
        pz: createReferenceComparisonPanel(),
        pss_drawdown: createReferenceComparisonPanel(),
        pss_productivity: createReferenceComparisonPanel(),
        pss_shape_factor: createReferenceComparisonPanel(),
        gor: createReferenceComparisonPanel(),
        volumes: createReferenceComparisonPanel(),
        oil_rate: createReferenceComparisonPanel(),
        gas_rate: createReferenceComparisonPanel(),
        injection_rate: createReferenceComparisonPanel(),
        producer_bhp: createReferenceComparisonPanel(),
        injector_bhp: createReferenceComparisonPanel(),
        control_limits: createReferenceComparisonPanel(),
        cumulative_gas: createReferenceComparisonPanel(),
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
        avg_water_sat: input.primary.avg_water_sat,
        mbe_ooip: input.primary.mbe_ooip,
        drive_indices: input.primary.drive_indices,
        pz: input.primary.pz,
        pss_drawdown: input.primary.pss_drawdown,
        pss_productivity: input.primary.pss_productivity,
        pss_shape_factor: input.primary.pss_shape_factor,
        gor: input.primary.gor,
        volumes: input.primary.volumes,
        oil_rate: input.primary.oil_rate,
        gas_rate: input.primary.gas_rate,
        injection_rate: input.primary.injection_rate,
        producer_bhp: input.primary.producer_bhp,
        injector_bhp: input.primary.injector_bhp,
        control_limits: input.primary.control_limits,
        cumulative_gas: input.primary.cumulative_gas,
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
    panels: Record<ChartPanelKey, ReferenceComparisonPanel>,
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
        borderWidth: ANALYTICAL_BORDER_MULTI,
        borderDash: ANALYTICAL_DASH,
        yAxisID: slot.yAxisID ?? 'y',
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
        yAxisID: slot.yAxisID ?? 'y',
    });
}

/**
 * Builds analytical-only preview panels before any simulation results exist.
 * Multi-variant arrays produce one colored curve per variant; single-variant
 * arrays use the neutral reference color.
 */
function buildAnalyticalPreviewPanels(
    variants: AnalyticalPreviewVariant[],
    xAxisMode: ChartXAxisMode,
    analyticalMethod: AnalyticalMethod,
    theme: ReferenceComparisonTheme,
): Record<ChartPanelKey, ReferenceComparisonPanel> {
    const panels: Record<ChartPanelKey, ReferenceComparisonPanel> = {
        rates: createReferenceComparisonPanel(),
        recovery: createReferenceComparisonPanel(),
        cumulative: createReferenceComparisonPanel(),
        diagnostics: createReferenceComparisonPanel(),
        avg_water_sat: createReferenceComparisonPanel(),
        mbe_ooip: createReferenceComparisonPanel(),
        drive_indices: createReferenceComparisonPanel(),
        pz: createReferenceComparisonPanel(),
        pss_drawdown: createReferenceComparisonPanel(),
        pss_productivity: createReferenceComparisonPanel(),
        pss_shape_factor: createReferenceComparisonPanel(),
        gor: createReferenceComparisonPanel(),
        volumes: createReferenceComparisonPanel(),
        oil_rate: createReferenceComparisonPanel(),
        gas_rate: createReferenceComparisonPanel(),
        injection_rate: createReferenceComparisonPanel(),
        producer_bhp: createReferenceComparisonPanel(),
        injector_bhp: createReferenceComparisonPanel(),
        control_limits: createReferenceComparisonPanel(),
        cumulative_gas: createReferenceComparisonPanel(),
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
            yAxisID: slot.yAxisID ?? 'y',
        }));
    });

    return panels;
}

// ─── Main builder ─────────────────────────────────────────────────────────────

export function buildReferenceComparisonModel(input: {
    family: BenchmarkFamily | null | undefined;
    results: BenchmarkRunResult[];
    xAxisMode: ChartXAxisMode;
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

    const panels: Record<ChartPanelKey, ReferenceComparisonPanel> = {
        rates: createReferenceComparisonPanel(),
        recovery: createReferenceComparisonPanel(),
        cumulative: createReferenceComparisonPanel(),
        diagnostics: createReferenceComparisonPanel(),
        avg_water_sat: createReferenceComparisonPanel(),
        mbe_ooip: createReferenceComparisonPanel(),
        drive_indices: createReferenceComparisonPanel(),
        pz: createReferenceComparisonPanel(),
        pss_drawdown: createReferenceComparisonPanel(),
        pss_productivity: createReferenceComparisonPanel(),
        pss_shape_factor: createReferenceComparisonPanel(),
        gor: createReferenceComparisonPanel(),
        volumes: createReferenceComparisonPanel(),
        oil_rate: createReferenceComparisonPanel(),
        gas_rate: createReferenceComparisonPanel(),
        injection_rate: createReferenceComparisonPanel(),
        producer_bhp: createReferenceComparisonPanel(),
        injector_bhp: createReferenceComparisonPanel(),
        control_limits: createReferenceComparisonPanel(),
        cumulative_gas: createReferenceComparisonPanel(),
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
                        appendPublishedReferenceSeries(panels, family, input.xAxisMode);
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
                        appendPublishedReferenceSeries(previewPanels, family, input.xAxisMode);
                        return combinePanelMaps({
                            primary: previewPanels,
                            sweep: descriptor.producesSweepPanels
                                ? buildPreviewSweepPanels({
                                    variants,
                                    theme: input.theme ?? 'dark',
                                    geometry: family?.sweepGeometry ?? 'both',
                                    method: family?.sweepAnalyticalMethod ?? DEFAULT_SWEEP_METHOD,
                                })
                                : emptySweepPanels(),
                        });
                    })(),
                    axisMappingWarning: null,
                };
            }
        }
        appendPublishedReferenceSeries(panels, family, input.xAxisMode);
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
        const tau = descriptor.definesCharacteristicTime ? computeDepletionTau(result.params) : null;
        const xValues = buildXAxisValues(derived, input.xAxisMode, tau);
        const defaultVisible = true;
        const caseLabel = compactCaseLabel(result.label);

        // ── Simulation curves ───────────────────────────────────────────────
        // One table, iterated. This was three branches of ~20 hand-written
        // appendSeries blocks, one per curve set, each naming a panel and a
        // derived-series field in code — and they had already drifted: the
        // recovery curve was relabelled in the oil-rate branch only. The table
        // is `simulationCurves.ts`; the quantities it places are
        // `runQuantities.ts`.
        const historyXAxis = interpolateXAxisAtTimes(derived.time, xValues, derived.historyTime);
        for (const curve of simulationCurvesForSet(descriptor.simulationCurveSet)) {
            const { label, property, values } = resolveSimulationCurve(curve, derived);
            appendSeries(panels[curve.panel], {
                label: `${result.label} ${label}`,
                curveKey: curve.curveKey,
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
                property,
            }, curve.axis === 'history' ? historyXAxis : xValues, values);
        }

        // Control-limit fractions accompany the well-pressure curves, so they
        // follow the same sets those curves belong to.
        if (descriptor.simulationCurveSet !== 'water-cut') {
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
        }


        const dietzPss = computeDietzPssSimulationDiagnostics(result, derived);
        if (dietzPss) {
            const diagnosticXAxis = interpolateXAxisAtTimes(
                derived.time,
                xValues,
                dietzPss.time,
            );
            // Drawdown is plotted from t=0, unlike PI and C_A: the rise from
            // zero to the Dietz asymptote *is* the approach to pseudo-steady
            // state, and is the only part of this case that moves in time.
            appendSeries(panels.pss_drawdown, {
                label: `${result.label} Numerical Drawdown`,
                curveKey: 'pss-drawdown-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, diagnosticXAxis, dietzPss.drawdown);
            appendSeries(panels.pss_productivity, {
                label: `${result.label} Numerical PI`,
                curveKey: 'pss-productivity-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, diagnosticXAxis, dietzPss.productivity);
            appendSeries(panels.pss_shape_factor, {
                label: `${result.label} Inferred C_A`,
                curveKey: 'pss-shape-factor-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, diagnosticXAxis, dietzPss.shapeFactor);
        }

        // ── MBE diagnostics (Havlena-Odeh) ─────────────────────────────────
        // Gated on the run, not on the analytical method: a material balance is
        // a check the simulation performs on itself and does not need a
        // closed-form overlay to exist. Gating it on `analyticalMethod ===
        // 'depletion'` hid it from every black-oil depletion case in the
        // catalog — `gas_drive`, `dep_pvt` — which are exactly the cases with
        // no other reference. `computeMbeDiagnostics` reports `applicable:
        // false` where this balance has no injection or influx term for what
        // the run did.
        const mbe = computeMbeDiagnostics(result, derived);
        if (mbe.applicable) {
            appendSeries(panels.mbe_ooip, {
                label: `${result.label} MBE OOIP Ratio`,
                curveKey: 'mbe-ooip-ratio',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: SIM_BORDER_SECONDARY,
                yAxisID: 'y',
                defaultVisible: false,
            }, xValues, mbe.ooipRatio);

            // ── Drive mechanism indices ─────────────────────────────────────
            appendSeries(panels.drive_indices, {
                label: `${result.label} Drive: Compaction`,
                curveKey: 'drive-compaction',
                caseKey: result.key,
                toggleGroupKey: `${result.key}-drive`,
                toggleLabel: caseLabel,
                legendSection: 'drive',
                legendSectionLabel: LEGEND_SECTIONS.driveIndices,
                color: '#e67e22',
                borderWidth: SIM_BORDER_SECONDARY,
                yAxisID: 'y',
                defaultVisible: false,
            }, xValues, mbe.driveCompaction);
            appendSeries(panels.drive_indices, {
                label: `${result.label} Drive: Oil Expansion`,
                curveKey: 'drive-oil-expansion',
                caseKey: result.key,
                toggleGroupKey: `${result.key}-drive`,
                toggleLabel: caseLabel,
                legendSection: 'drive',
                legendSectionLabel: LEGEND_SECTIONS.driveIndices,
                color: '#27ae60',
                borderWidth: SIM_BORDER_SECONDARY,
                yAxisID: 'y',
                defaultVisible: false,
            }, xValues, mbe.driveOilExpansion);
            appendSeries(panels.drive_indices, {
                label: `${result.label} Drive: Free-Gas Expansion`,
                curveKey: 'drive-gas-cap',
                caseKey: result.key,
                toggleGroupKey: `${result.key}-drive`,
                toggleLabel: caseLabel,
                legendSection: 'drive',
                legendSectionLabel: LEGEND_SECTIONS.driveIndices,
                color: '#2980b9',
                borderWidth: SIM_BORDER_SECONDARY,
                yAxisID: 'y',
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
            method: family.sweepAnalyticalMethod ?? DEFAULT_SWEEP_METHOD,
        })
        : emptySweepPanels();

    // ── Published reference overlays (static benchmark data) ────────────────
    appendPublishedReferenceSeries(panels, family, input.xAxisMode);

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
