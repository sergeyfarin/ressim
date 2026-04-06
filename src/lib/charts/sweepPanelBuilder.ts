/**
 * sweepPanelBuilder.ts — builds sweep-efficiency panel data from simulation
 * rate history and analytical sweep computations.
 *
 * Exports two top-level builders consumed by buildChartData:
 *   buildSweepPanels       — for completed results (sim + analytical curves)
 *   buildPreviewSweepPanels — for analytical-only preview before any results exist
 *
 * All sweep data enters via `result.rateHistory[i].sweep` (populated by the Rust
 * simulator when a sweep config is set — see Phase 1, reporting.rs).
 */

import {
    computeDepletionTau,
    extractFluidProps,
    extractRockProps,
    getAverageLayerThickness,
    getLayerPermeabilities,
    buildDerivedRunSeries,
} from './analyticalParamAdapters';
import type { DerivedRunSeries, XYPoint } from './axisAdapters';
import { mapPviSeriesToXAxis, toXYSeries } from './axisAdapters';
import {
    computeCombinedSweep,
    computeSweepRecoveryFactor,
    getSweepComponentVisibility,
    type SweepAnalyticalMethod,
    type SweepGeometry,
} from '../analytical/sweepEfficiency';
import {
    ANALYTICAL_DASH,
    LEGEND_SECTIONS,
    SWEEP_DASH_AREAL,
    SWEEP_DASH_COMBINED,
    SWEEP_DASH_VERTICAL,
    simBorderWidth,
} from './curveStylePolicy';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import type { RateChartXAxisMode } from './rateChartLayoutConfig';
import {
    appendSeries,
    compactCaseLabel,
    createSweepPanels,
    finalizeSweepPanels,
    getLegendGrey,
    getReferenceColor,
    getReferenceComparisonCaseColor,
    type AnalyticalPreviewVariant,
    type ReferenceComparisonPanel,
    type ReferenceComparisonSweepPanels,
    type ReferenceComparisonTheme,
} from './referenceChartTypes';

// ─── Dedup helper ─────────────────────────────────────────────────────────────

/** Collapses consecutive points at the same x, keeping the last y. Used to
 *  clean up the initial x=0 anchor that is pushed before simulation steps. */
function dedupeSweepSeries(points: XYPoint[]): XYPoint[] {
    const deduped: XYPoint[] = [];
    for (const point of points) {
        const previous = deduped.at(-1);
        if (previous && Math.abs(previous.x - point.x) <= 1e-9) {
            if (deduped.length === 1 && Math.abs(previous.x) <= 1e-9) {
                continue;
            }
            previous.y = point.y;
            continue;
        }
        deduped.push({ ...point });
    }
    return deduped;
}

// ─── Simulation sweep series ──────────────────────────────────────────────────

function buildSimulationSweepSeries(
    result: BenchmarkRunResult,
    xAxisMode: RateChartXAxisMode,
    tau: number | null,
    derived: DerivedRunSeries,
    geometry: SweepGeometry,
): {
    rf: XYPoint[];
    areal: XYPoint[];
    vertical: XYPoint[];
    combined: XYPoint[];
    combinedMobileOil: XYPoint[];
    showAreal: boolean;
    showVertical: boolean;
} {
    const visibility = getSweepComponentVisibility(geometry);

    const areal: XYPoint[] = geometry === 'both' ? [] : [{ x: 0, y: 0 }];
    const vertical: XYPoint[] = geometry === 'both' ? [] : [{ x: 0, y: 0 }];
    const combined: XYPoint[] = [{ x: 0, y: 0 }];
    const combinedMobileOil: XYPoint[] = geometry === 'both' ? [{ x: 0, y: 0 }] : [];

    for (let i = 0; i < result.rateHistory.length; i++) {
        const sweep = result.rateHistory[i].sweep;
        if (!sweep) continue;
        const pvi = result.pviSeries[i] ?? null;
        if (pvi == null || !Number.isFinite(pvi)) continue;
        const selectedXAxis = mapPviSeriesToXAxis([pvi], derived, xAxisMode, tau)[0];
        if (!Number.isFinite(selectedXAxis)) continue;
        if (geometry !== 'both' && sweep.e_a != null) {
            areal.push({ x: Number(selectedXAxis), y: sweep.e_a });
        }
        if (geometry !== 'both' && sweep.e_v != null) {
            vertical.push({ x: Number(selectedXAxis), y: sweep.e_v });
        }
        combined.push({ x: Number(selectedXAxis), y: sweep.e_vol });
        if (geometry === 'both' && sweep.mobile_oil_recovered != null) {
            combinedMobileOil.push({ x: Number(selectedXAxis), y: sweep.mobile_oil_recovered });
        }
    }

    const sweepRfXValues = mapPviSeriesToXAxis(result.pviSeries, derived, xAxisMode, tau);

    return {
        rf: dedupeSweepSeries(toXYSeries(sweepRfXValues, result.recoverySeries)),
        areal: visibility.showAreal ? dedupeSweepSeries(areal) : [],
        vertical: visibility.showVertical ? dedupeSweepSeries(vertical) : [],
        combined: dedupeSweepSeries(combined),
        combinedMobileOil: geometry === 'both' ? dedupeSweepSeries(combinedMobileOil) : [],
        showAreal: visibility.showAreal,
        showVertical: visibility.showVertical,
    };
}

// ─── Analytical sweep series ──────────────────────────────────────────────────

function buildAnalyticalSweepSeries(
    params: Record<string, any>,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
    tau: number | null,
    geometry: SweepGeometry,
    method: SweepAnalyticalMethod,
): {
    xValues: Array<number | null>;
    rf: Array<number | null>;
    areal: Array<number | null>;
    vertical: Array<number | null>;
    combined: Array<number | null>;
    showAreal: boolean;
    showVertical: boolean;
} {
    const rock = extractRockProps(params);
    const fluid = extractFluidProps(params);
    const permeabilities = getLayerPermeabilities(params);
    const thickness = getAverageLayerThickness(params);
    const visibility = getSweepComponentVisibility(geometry);
    const sweep = computeCombinedSweep(rock, fluid, permeabilities, thickness, 3.0, 200, geometry, method);
    const recovery = computeSweepRecoveryFactor(rock, fluid, permeabilities, thickness, 3.0, 200, geometry, method);
    const xValues = mapPviSeriesToXAxis(sweep.arealSweep.curve.map((point) => point.pvi), derived, xAxisMode, tau);
    return {
        xValues,
        rf: recovery.curve.map((point) => point.rfSweep),
        areal: sweep.arealSweep.curve.map((point) => point.efficiency),
        vertical: sweep.verticalSweep.curve.map((point) => point.efficiency),
        combined: sweep.combined.map((point) => point.efficiency),
        showAreal: visibility.showAreal,
        showVertical: visibility.showVertical,
    };
}

// ─── Append helpers ───────────────────────────────────────────────────────────

function appendAnalyticalSweepCurves(
    panels: Record<keyof ReferenceComparisonSweepPanels, ReferenceComparisonPanel>,
    input: {
        label: string;
        caseKey?: string;
        toggleLabel: string;
        color: string;
        params: Record<string, any>;
        derived: DerivedRunSeries;
        xAxisMode: RateChartXAxisMode;
        tau: number | null;
        geometry: SweepGeometry;
        theme: ReferenceComparisonTheme;
        method: SweepAnalyticalMethod;
    },
) {
    const analytical = buildAnalyticalSweepSeries(input.params, input.derived, input.xAxisMode, input.tau, input.geometry, input.method);
    const toggleGroupKey = input.caseKey ? `${input.caseKey}__ref` : 'analytical';
    const legendColor = input.caseKey ? undefined : getLegendGrey(input.theme);
    const borderWidth = input.caseKey ? 1.5 : 2;
    const analyticalRfLabel = input.method === 'stiles'
        ? `${input.label} — Analytical Total RF (Stiles Layered BL)`
        : `${input.label} — Analytical Total RF (Dykstra-Parsons)`;
    const analyticalEvolLabel = input.method === 'stiles'
        ? `${input.label} — Analytical Total E_vol (Stiles Layered BL)`
        : `${input.label} — Analytical Total E_vol (Dykstra-Parsons)`;

    appendSeries(panels.rf, {
        label: analyticalRfLabel,
        curveKey: 'sweep-rf-reference',
        ...(input.caseKey ? { caseKey: input.caseKey } : {}),
        toggleGroupKey,
        toggleLabel: input.toggleLabel,
        legendSection: 'analytical',
        legendSectionLabel: LEGEND_SECTIONS.analytical,
        color: input.color,
        ...(legendColor ? { legendColor } : {}),
        borderWidth,
        borderDash: ANALYTICAL_DASH,
        yAxisID: 'y',
    }, analytical.xValues, analytical.rf);

    if (analytical.showAreal) {
        appendSeries(panels.areal, {
            label: `${input.label} — Analytical E_A (diagnostic decomposition)`,
            curveKey: 'sweep-areal-reference',
            ...(input.caseKey ? { caseKey: input.caseKey } : {}),
            toggleGroupKey,
            toggleLabel: input.toggleLabel,
            legendSection: 'analytical',
            legendSectionLabel: LEGEND_SECTIONS.analytical,
            color: input.color,
            ...(legendColor ? { legendColor } : {}),
            borderWidth,
            borderDash: SWEEP_DASH_AREAL,
            yAxisID: 'y',
        }, analytical.xValues, analytical.areal);
    }

    if (analytical.showVertical) {
        appendSeries(panels.vertical, {
            label: `${input.label} — Analytical E_V (diagnostic decomposition)`,
            curveKey: 'sweep-vertical-reference',
            ...(input.caseKey ? { caseKey: input.caseKey } : {}),
            toggleGroupKey,
            toggleLabel: input.toggleLabel,
            legendSection: 'analytical',
            legendSectionLabel: LEGEND_SECTIONS.analytical,
            color: input.color,
            ...(legendColor ? { legendColor } : {}),
            borderWidth,
            borderDash: SWEEP_DASH_VERTICAL,
            yAxisID: 'y',
        }, analytical.xValues, analytical.vertical);
    }

    appendSeries(panels.combined, {
        label: analyticalEvolLabel,
        curveKey: 'sweep-combined-reference',
        ...(input.caseKey ? { caseKey: input.caseKey } : {}),
        toggleGroupKey,
        toggleLabel: input.toggleLabel,
        legendSection: 'analytical',
        legendSectionLabel: LEGEND_SECTIONS.analytical,
        color: input.color,
        ...(legendColor ? { legendColor } : {}),
        borderWidth,
        borderDash: SWEEP_DASH_COMBINED,
        yAxisID: 'y',
    }, analytical.xValues, analytical.combined);

    if (input.geometry === 'both') {
        appendSeries(panels.combinedMobileOil, {
            label: analyticalEvolLabel,
            curveKey: 'sweep-combined-reference',
            ...(input.caseKey ? { caseKey: input.caseKey } : {}),
            toggleGroupKey,
            toggleLabel: input.toggleLabel,
            legendSection: 'analytical',
            legendSectionLabel: LEGEND_SECTIONS.analytical,
            color: input.color,
            ...(legendColor ? { legendColor } : {}),
            borderWidth,
            borderDash: SWEEP_DASH_COMBINED,
            yAxisID: 'y',
        }, analytical.xValues, analytical.combined);
    }
}

function appendSimulationSweepCurves(
    panels: Record<keyof ReferenceComparisonSweepPanels, ReferenceComparisonPanel>,
    result: BenchmarkRunResult,
    color: string,
    xAxisMode: RateChartXAxisMode,
    tau: number | null,
    derived: DerivedRunSeries,
    geometry: SweepGeometry,
) {
    const simulation = buildSimulationSweepSeries(result, xAxisMode, tau, derived, geometry);
    const caseLabel = compactCaseLabel(result.label);

    if (simulation.rf.length > 0) {
        panels.rf.curves.push({
            label: `${result.label} RF`,
            curveKey: 'sweep-rf-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.rf.series.push(simulation.rf);
    }

    if (simulation.areal.length > 0) {
        panels.areal.curves.push({
            label: `${result.label} E_A`,
            curveKey: 'sweep-areal-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.areal.series.push(simulation.areal);
    }

    if (simulation.vertical.length > 0) {
        panels.vertical.curves.push({
            label: `${result.label} E_V`,
            curveKey: 'sweep-vertical-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.vertical.series.push(simulation.vertical);
    }

    if (simulation.combined.length > 0) {
        panels.combined.curves.push({
            label: `${result.label} E_vol`,
            curveKey: 'sweep-combined-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.combined.series.push(simulation.combined);
    }

    if (simulation.combinedMobileOil.length > 0) {
        panels.combinedMobileOil.curves.push({
            label: `${result.label} Mobile Oil Recovered`,
            curveKey: 'sweep-combined-mobile-oil-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: 1.8,
            borderDash: [3, 3],
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.combinedMobileOil.series.push(simulation.combinedMobileOil);
    }
}

// ─── Empty derived series for preview contexts ────────────────────────────────

function emptyDerivedSeries(): DerivedRunSeries {
    return {
        time: [], historyTime: [], oilRate: [], injectionRate: [],
        waterCut: [], gasCut: [], avgWaterSat: [], pressure: [],
        producerBhp: [], injectorBhp: [], recovery: [], cumulativeOil: [],
        cumulativeInjection: [], cumulativeLiquid: [], cumulativeGas: [],
        p_z: [], pvi: [], pvp: [], gor: [],
        producerBhpLimitedFraction: [], injectorBhpLimitedFraction: [],
    };
}

// ─── Public builders ──────────────────────────────────────────────────────────

/**
 * Builds sweep panels for analytical-only preview (no simulation results yet).
 * One analytical curve per variant; color indexed from the case palette for
 * multi-variant previews.
 */
export function buildPreviewSweepPanels(input: {
    variants: AnalyticalPreviewVariant[];
    theme: ReferenceComparisonTheme;
    geometry: SweepGeometry;
    method: SweepAnalyticalMethod;
}): ReferenceComparisonSweepPanels {
    const panels = createSweepPanels();
    const multiVariant = input.variants.length > 1;
    const referenceColor = getReferenceColor(input.theme);
    const previewDerived = emptyDerivedSeries();

    input.variants.forEach((variant, index) => {
        const color = multiVariant ? getReferenceComparisonCaseColor(index) : referenceColor;
        const label = variant.label || 'Analytical';
        appendAnalyticalSweepCurves(panels, {
            label,
            ...(multiVariant ? { caseKey: variant.variantKey } : {}),
            toggleLabel: multiVariant ? compactCaseLabel(label) : 'Analytical solution',
            color,
            params: variant.params,
            theme: input.theme,
            derived: previewDerived,
            xAxisMode: 'pvi',
            tau: null,
            geometry: input.geometry,
            method: input.method,
        });
    });

    return finalizeSweepPanels(panels);
}

/**
 * Builds sweep panels for completed + pending simulation results.
 * Simulation curves (solid) + analytical curves (dashed) per result.
 * Pending variants appear as analytical-only dashed overlays at their
 * declaration-order color index, so colors stay stable as the sweep progresses.
 */
export function buildSweepPanels(input: {
    orderedResults: BenchmarkRunResult[];
    pendingPreviewVariants?: AnalyticalPreviewVariant[];
    previewVariantParams?: AnalyticalPreviewVariant[];
    theme: ReferenceComparisonTheme;
    xAxisMode: RateChartXAxisMode;
    derivedByKey: Map<string, DerivedRunSeries>;
    geometry: SweepGeometry;
    method: SweepAnalyticalMethod;
}): ReferenceComparisonSweepPanels {
    const panels = createSweepPanels();

    input.orderedResults.forEach((result, index) => {
        const color = getReferenceComparisonCaseColor(index);
        const derived = input.derivedByKey.get(result.key);
        if (!derived) return;
        const tau = computeDepletionTau(result.params);
        appendSimulationSweepCurves(panels, result, color, input.xAxisMode, tau, derived, input.geometry);
        appendAnalyticalSweepCurves(panels, {
            label: result.label,
            caseKey: result.key,
            toggleLabel: compactCaseLabel(result.label),
            color,
            params: result.params,
            theme: input.theme,
            derived,
            xAxisMode: input.xAxisMode,
            tau,
            geometry: input.geometry,
            method: input.method,
        });
    });

    if (input.pendingPreviewVariants?.length && input.xAxisMode === 'pvi') {
        const declarationOrder = new Map(
            (input.previewVariantParams ?? []).map((variant, index) => [variant.variantKey, index]),
        );
        const previewDerived = input.orderedResults[0]
            ? buildDerivedRunSeries(input.orderedResults[0])
            : emptyDerivedSeries();

        input.pendingPreviewVariants.forEach((variant, fallbackIndex) => {
            const colorIndex = declarationOrder.get(variant.variantKey)
                ?? (input.orderedResults.length + fallbackIndex);
            appendAnalyticalSweepCurves(panels, {
                label: variant.label,
                caseKey: variant.variantKey,
                toggleLabel: compactCaseLabel(variant.label),
                color: getReferenceComparisonCaseColor(colorIndex),
                params: variant.params,
                theme: input.theme,
                derived: previewDerived,
                xAxisMode: input.xAxisMode,
                tau: null,
                geometry: input.geometry,
                method: input.method,
            });
        });
    }

    return finalizeSweepPanels(panels);
}
