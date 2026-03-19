import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { calculateAnalyticalProduction } from '../analytical/fractionalFlow';
import type { RockProps, FluidProps } from '../analytical/fractionalFlow';
import { computeCombinedSweep } from '../analytical/sweepEfficiency';
import type { BenchmarkFamily } from '../catalog/benchmarkCases';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import type { CurveConfig } from './ChartSubPanel.svelte';
import type { RateChartPanelKey, RateChartXAxisMode } from './rateChartLayoutConfig';

export type XYPoint = { x: number; y: number | null };

export type ReferenceComparisonPanel = {
    curves: CurveConfig[];
    series: XYPoint[][];
};

export type ReferenceComparisonModel = {
    orderedResults: BenchmarkRunResult[];
    panels: Record<RateChartPanelKey, ReferenceComparisonPanel>;
    sweepPanel: ReferenceComparisonPanel | null;
};

export type ReferenceComparisonTheme = 'dark' | 'light';

/**
 * One entry in the multi-variant analytical preview shown before any simulation
 * results exist. Each entry produces its own colored analytical curve so the user
 * can see how the 4 mobility (or other sensitivity) variants differ analytically
 * without having to run anything first.
 */
export type AnalyticalPreviewVariant = {
    /** Display label used in curve legends (e.g. "Favorable", "Base"). */
    label: string;
    /** Variant key used as caseKey on chart series for future toggle support. */
    variantKey: string;
    /** Full merged params (base scenario params + variant paramPatch). */
    params: Record<string, any>;
};

type DerivedRunSeries = {
    time: number[];
    oilRate: Array<number | null>;
    waterCut: Array<number | null>;
    avgWaterSat: Array<number | null>;
    pressure: Array<number | null>;
    recovery: Array<number | null>;
    cumulativeOil: Array<number | null>;
    cumulativeInjection: Array<number | null>;
    cumulativeLiquid: Array<number | null>;
    pvi: Array<number | null>;
    pvp: Array<number | null>;
};

type AnalyticalOverlay = {
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

const CASE_COLORS = [
    '#0f766e',
    '#2563eb',
    '#dc2626',
    '#7c3aed',
    '#ea580c',
    '#0891b2',
    '#4f46e5',
    '#65a30d',
];

export function getReferenceComparisonCaseColor(index: number): string {
    return CASE_COLORS[index % CASE_COLORS.length];
}

function getReferenceColor(theme: ReferenceComparisonTheme): string {
    return theme === 'dark' ? '#f8fafc' : '#0f172a';
}

function toFiniteNumber(value: unknown, fallback: number): number {
    const numeric = Number(value);
    return Number.isFinite(numeric) ? numeric : fallback;
}

function getPoreVolume(params: Record<string, any>): number {
    return toFiniteNumber(params.nx, 1)
        * toFiniteNumber(params.ny, 1)
        * toFiniteNumber(params.nz, 1)
        * toFiniteNumber(params.cellDx, 10)
        * toFiniteNumber(params.cellDy, 10)
        * toFiniteNumber(params.cellDz, 1)
        * toFiniteNumber(params.reservoirPorosity ?? params.porosity, 0.2);
}

function getOoip(params: Record<string, any>): number {
    const poreVolume = getPoreVolume(params);
    const initialSaturation = toFiniteNumber(params.initialSaturation, 0.3);
    return poreVolume * Math.max(0, 1 - initialSaturation);
}

function toXYSeries(
    xValues: Array<number | null>,
    yValues: Array<number | null | undefined>,
): XYPoint[] {
    const points: XYPoint[] = [];
    for (let index = 0; index < yValues.length; index += 1) {
        const rawX = xValues[index];
        const rawY = yValues[index];
        if (!Number.isFinite(rawX)) continue;
        points.push({
            x: Number(rawX),
            y: Number.isFinite(rawY) ? Number(rawY) : null,
        });
    }
    return points;
}

function buildDerivedRunSeries(result: BenchmarkRunResult): DerivedRunSeries {
    const poreVolume = getPoreVolume(result.params);
    const ooip = getOoip(result.params);
    let cumulativeInjection = 0;
    let cumulativeLiquid = 0;

    const cumulativeInjectionSeries: Array<number | null> = [];
    const cumulativeLiquidSeries: Array<number | null> = [];

    for (let index = 0; index < result.rateHistory.length; index += 1) {
        const point = result.rateHistory[index];
        const dt = index > 0
            ? Math.max(0, toFiniteNumber(point.time, 0) - toFiniteNumber(result.rateHistory[index - 1]?.time, 0))
            : Math.max(0, toFiniteNumber(point.time, 0));
        cumulativeInjection += Math.max(0, toFiniteNumber(point.total_injection, 0)) * dt;
        cumulativeLiquid += Math.max(0, Math.abs(toFiniteNumber(point.total_production_liquid, 0))) * dt;
        cumulativeInjectionSeries.push(cumulativeInjection);
        cumulativeLiquidSeries.push(cumulativeLiquid);
    }

    return {
        time: result.rateHistory.map((point) => toFiniteNumber(point.time, 0)),
        oilRate: result.rateHistory.map((point) => Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0)))),
        waterCut: [...result.watercutSeries],
        avgWaterSat: result.rateHistory.map((point) => {
            const value = point.avg_water_saturation;
            return Number.isFinite(value) ? Number(value) : null;
        }),
        pressure: [...result.pressureSeries],
        recovery: [...result.recoverySeries],
        cumulativeOil: result.recoverySeries.map((value) => (
            Number.isFinite(value) && ooip > 1e-12 ? Number(value) * ooip : null
        )),
        cumulativeInjection: cumulativeInjectionSeries,
        cumulativeLiquid: cumulativeLiquidSeries,
        pvi: [...result.pviSeries],
        pvp: cumulativeLiquidSeries.map((value) => (
            poreVolume > 1e-12 && Number.isFinite(value) ? Number(value) / poreVolume : null
        )),
    };
}

function buildXAxisValues(
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
    tau: number | null = null,
): Array<number | null> {
    if (xAxisMode === 'pvi') return [...derived.pvi];
    if (xAxisMode === 'pvp') return [...derived.pvp];
    if (xAxisMode === 'cumInjection') return [...derived.cumulativeInjection];
    if (xAxisMode === 'cumLiquid') return [...derived.cumulativeLiquid];
    if (xAxisMode === 'logTime') return derived.time.map((value) => (value > 0 ? Math.log10(value) : null));
    if (xAxisMode === 'tD' && Number.isFinite(tau) && (tau as number) > 0) {
        return derived.time.map((value) => value / (tau as number));
    }
    return [...derived.time];
}

function getBaseResult(results: BenchmarkRunResult[]): BenchmarkRunResult | null {
    return results.find((result) => result.variantKey === null) ?? results[0] ?? null;
}

function orderResults(results: BenchmarkRunResult[]): BenchmarkRunResult[] {
    return [...results];
}

function buildBuckleyLeverettReference(
    baseResult: BenchmarkRunResult,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
): AnalyticalOverlay {
    const poreVolume = getPoreVolume(baseResult.params);
    const ooip = getOoip(baseResult.params);
    const analyticalProduction = calculateAnalyticalProduction(
        {
            s_wc: toFiniteNumber(baseResult.params.s_wc, 0.1),
            s_or: toFiniteNumber(baseResult.params.s_or, 0.1),
            n_w: toFiniteNumber(baseResult.params.n_w, 2),
            n_o: toFiniteNumber(baseResult.params.n_o, 2),
            k_rw_max: toFiniteNumber(baseResult.params.k_rw_max, 1),
            k_ro_max: toFiniteNumber(baseResult.params.k_ro_max, 1),
        },
        {
            mu_w: toFiniteNumber(baseResult.params.mu_w, 0.5),
            mu_o: toFiniteNumber(baseResult.params.mu_o, 1),
        },
        toFiniteNumber(baseResult.params.initialSaturation, toFiniteNumber(baseResult.params.s_wc, 0.1)),
        derived.time,
        baseResult.rateHistory.map((point) => Math.max(0, toFiniteNumber(point.total_injection, 0))),
        poreVolume,
    );

    const waterCut = analyticalProduction.map((point) => {
        const total = Math.max(0, point.oilRate + point.waterRate);
        return total > 1e-12 ? point.waterRate / total : 0;
    });
    const recovery = analyticalProduction.map((point) => (
        ooip > 1e-12 ? Math.max(0, Math.min(1, point.cumulativeOil / ooip)) : null
    ));

    return {
        rates: { label: 'Reference Solution Water Cut', values: waterCut },
        cumulative: {
            recoveryLabel: 'Reference Solution Recovery',
            recoveryValues: recovery,
            cumulativeLabel: 'Reference Solution Cum Oil',
            cumulativeValues: analyticalProduction.map((point) => point.cumulativeOil),
        },
        diagnostics: null,
        xValues: buildXAxisValues(derived, xAxisMode),
    };
}

function buildDepletionReference(
    baseResult: BenchmarkRunResult,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
): AnalyticalOverlay {
    const analyticalResult = calculateDepletionAnalyticalProduction({
        reservoir: {
            length: toFiniteNumber(baseResult.params.nx, 1) * toFiniteNumber(baseResult.params.cellDx, 10),
            area: toFiniteNumber(baseResult.params.ny, 1)
                * toFiniteNumber(baseResult.params.cellDy, 10)
                * toFiniteNumber(baseResult.params.nz, 1)
                * toFiniteNumber(baseResult.params.cellDz, 1),
            porosity: toFiniteNumber(baseResult.params.reservoirPorosity ?? baseResult.params.porosity, 0.2),
        },
        timeHistory: derived.time,
        initialSaturation: toFiniteNumber(baseResult.params.initialSaturation, 0.3),
        nz: toFiniteNumber(baseResult.params.nz, 1),
        permMode: String(baseResult.params.permMode ?? 'uniform'),
        uniformPermX: toFiniteNumber(baseResult.params.uniformPermX, 100),
        uniformPermY: toFiniteNumber(baseResult.params.uniformPermY ?? baseResult.params.uniformPermX, 100),
        layerPermsX: Array.isArray(baseResult.params.layerPermsX) ? baseResult.params.layerPermsX.map(Number) : [],
        layerPermsY: Array.isArray(baseResult.params.layerPermsY) ? baseResult.params.layerPermsY.map(Number) : [],
        cellDx: toFiniteNumber(baseResult.params.cellDx, 10),
        cellDy: toFiniteNumber(baseResult.params.cellDy, 10),
        cellDz: toFiniteNumber(baseResult.params.cellDz, 1),
        wellRadius: toFiniteNumber(baseResult.params.well_radius, 0.1),
        wellSkin: toFiniteNumber(baseResult.params.well_skin, 0),
        muO: toFiniteNumber(baseResult.params.mu_o, 1),
        sWc: toFiniteNumber(baseResult.params.s_wc, 0.1),
        sOr: toFiniteNumber(baseResult.params.s_or, 0.1),
        nO: toFiniteNumber(baseResult.params.n_o, 2),
        c_o: toFiniteNumber(baseResult.params.c_o, 1e-5),
        c_w: toFiniteNumber(baseResult.params.c_w, 3e-6),
        cRock: toFiniteNumber(baseResult.params.rock_compressibility, 1e-6),
        initialPressure: toFiniteNumber(baseResult.params.initialPressure, 300),
        producerBhp: toFiniteNumber(baseResult.params.producerBhp, 100),
        depletionRateScale: toFiniteNumber(baseResult.params.analyticalDepletionRateScale, 1),
    });
    const ooip = getOoip(baseResult.params);
    const tau = analyticalResult.meta.tau ?? null;

    return {
        rates: {
            label: 'Reference Solution Oil Rate',
            values: analyticalResult.production.map((point) => point.oilRate),
        },
        cumulative: {
            recoveryLabel: 'Reference Solution Recovery',
            recoveryValues: analyticalResult.production.map((point) => (
                ooip > 1e-12 ? Math.max(0, Math.min(1, point.cumulativeOil / ooip)) : null
            )),
            cumulativeLabel: 'Reference Solution Cum Oil',
            cumulativeValues: analyticalResult.production.map((point) => point.cumulativeOil),
        },
        diagnostics: {
            label: 'Reference Solution Avg Pressure',
            values: analyticalResult.production.map((point) => point.avgPressure),
        },
        xValues: buildXAxisValues(
            {
                ...derived,
                time: analyticalResult.production.map((point) => point.time),
            },
            xAxisMode,
            tau,
        ),
    };
}

function appendSeries(
    panel: ReferenceComparisonPanel,
    curve: CurveConfig,
    xValues: Array<number | null>,
    yValues: Array<number | null>,
) {
    panel.curves.push(curve);
    panel.series.push(toXYSeries(xValues, yValues));
}

function extractRockProps(params: Record<string, any>): RockProps {
    return {
        s_wc: toFiniteNumber(params.s_wc, 0.1),
        s_or: toFiniteNumber(params.s_or, 0.1),
        n_w: toFiniteNumber(params.n_w, 2),
        n_o: toFiniteNumber(params.n_o, 2),
        k_rw_max: toFiniteNumber(params.k_rw_max, 1),
        k_ro_max: toFiniteNumber(params.k_ro_max, 1),
    };
}

function extractFluidProps(params: Record<string, any>): FluidProps {
    return {
        mu_w: toFiniteNumber(params.mu_w, 0.5),
        mu_o: toFiniteNumber(params.mu_o, 1),
    };
}

function getLayerPermeabilities(params: Record<string, any>): number[] {
    const nz = toFiniteNumber(params.nz, 1);
    if (String(params.permMode) === 'perLayer'
        && Array.isArray(params.layerPermsX)
        && params.layerPermsX.length > 1) {
        return params.layerPermsX.map(Number);
    }
    if (nz > 1) {
        return Array.from({ length: nz }, () => toFiniteNumber(params.uniformPermX, 100));
    }
    return [toFiniteNumber(params.uniformPermX, 100)];
}

// ─── Chart Visual Convention ──────────────────────────────────────────────────
//
//  Solid line   = simulation result (primary output from IMPES solver)
//  Dashed line  = analytical reference (Craig, Dykstra-Parsons, Buckley-Leverett, etc.)
//  Color        = sensitivity variant / case index (CASE_COLORS[index])
//
//  Sweep panel — all curves are analytical (no simulation sweep efficiency output
//  exists yet). Within a single variant color, dash PATTERN distinguishes curve type:
//    E_A  (areal):    [7, 4]  medium dash,  weight 2.0
//    E_V  (vertical): [3, 4]  short dash,   weight 1.6  (hidden by default)
//    E_vol (combined):[12, 4] long dash,    weight 2.4  ← boldest = key result
//
//  Future: add simulation sweep efficiency (E_A_sim computed from saturation
//  snapshots) as solid lines alongside these analytical dashed curves — see TODO.md.
// ─────────────────────────────────────────────────────────────────────────────

const SWEEP_DASH_AREAL    = [7, 4]  as number[];  // medium dash — E_A
const SWEEP_DASH_VERTICAL = [3, 4]  as number[];  // short dash  — E_V
const SWEEP_DASH_COMBINED = [12, 4] as number[];  // long dash   — E_vol

/**
 * Build analytical-only preview panels before any simulation results exist.
 *
 * Accepts an array of variant entries so multiple colored curves can be rendered
 * when the selected sensitivity dimension affects the analytical solution (e.g.
 * mobility ratio). Each entry is computed independently — a numerical failure in
 * one variant is silently skipped rather than aborting the whole preview.
 *
 * Single-entry arrays use the neutral reference color (current base-preview
 * behavior). Multi-entry arrays use per-variant case colors.
 */
function buildAnalyticalPreviewPanels(
    variants: AnalyticalPreviewVariant[],
    xAxisMode: RateChartXAxisMode,
    scenarioClass: string,
    theme: ReferenceComparisonTheme,
): Record<RateChartPanelKey, ReferenceComparisonPanel> {
    const panels: Record<RateChartPanelKey, ReferenceComparisonPanel> = {
        rates: { curves: [], series: [] },
        recovery: { curves: [], series: [] },
        cumulative: { curves: [], series: [] },
        diagnostics: { curves: [], series: [] },
    };

    if (variants.length === 0) return panels;

    const multiVariant = variants.length > 1;
    const neutralColor = getReferenceColor(theme);

    const getColor = (index: number) =>
        multiVariant ? getReferenceComparisonCaseColor(index) : neutralColor;

    const labelPrefix = (variant: AnalyticalPreviewVariant) =>
        multiVariant ? `${variant.label} — ` : '';

    if (scenarioClass === 'buckley-leverett' || scenarioClass === 'waterflood') {
        const N = 150;
        const pviMax = 3.0;
        const pviValues = Array.from({ length: N }, (_, i) => (i / (N - 1)) * pviMax);
        const injRates = new Array(N).fill(1);
        const ooip = 1; // poreVolume = 1, so RF = cumOil

        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;

            let analyticalProduction: Array<{ oilRate: number; waterRate: number; cumulativeOil: number }>;
            try {
                analyticalProduction = calculateAnalyticalProduction(
                    extractRockProps(variant.params),
                    extractFluidProps(variant.params),
                    toFiniteNumber(variant.params.initialSaturation, toFiniteNumber(variant.params.s_wc, 0.1)),
                    pviValues,
                    injRates,
                    1, // poreVolume = 1 → time = PVI
                );
            } catch {
                // Skip this variant — bad params or numerical failure, don't abort others.
                return;
            }

            const waterCut = analyticalProduction.map((pt) => {
                const total = Math.max(0, pt.oilRate + pt.waterRate);
                return total > 1e-12 ? pt.waterRate / total : 0;
            });
            const recovery = analyticalProduction.map((pt) =>
                Math.max(0, Math.min(1, pt.cumulativeOil / ooip)),
            );

            appendSeries(panels.rates, {
                label: `${prefix}Analytical Water Cut`,
                curveKey: 'water-cut-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleLabel: 'Analytical Water Cut',
                color,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, pviValues, waterCut);
            appendSeries(panels.recovery, {
                label: `${prefix}Analytical Recovery Factor`,
                curveKey: 'recovery-factor',
                ...(caseKey ? { caseKey } : {}),
                toggleLabel: 'Analytical Recovery Factor',
                color,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, pviValues, recovery);
        });

        return panels;
    }

    if (scenarioClass === 'depletion') {
        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;

            const steps = toFiniteNumber(variant.params.steps, 200);
            const dt = toFiniteNumber(variant.params.delta_t_days, 5);
            const timeHistory = Array.from({ length: steps }, (_, i) => (i + 1) * dt);

            let analyticalResult: ReturnType<typeof calculateDepletionAnalyticalProduction>;
            try {
                analyticalResult = calculateDepletionAnalyticalProduction({
                    reservoir: {
                        length: toFiniteNumber(variant.params.nx, 1) * toFiniteNumber(variant.params.cellDx, 10),
                        area: toFiniteNumber(variant.params.ny, 1)
                            * toFiniteNumber(variant.params.cellDy, 10)
                            * toFiniteNumber(variant.params.nz, 1)
                            * toFiniteNumber(variant.params.cellDz, 1),
                        porosity: toFiniteNumber(variant.params.reservoirPorosity ?? variant.params.porosity, 0.2),
                    },
                    timeHistory,
                    initialSaturation: toFiniteNumber(variant.params.initialSaturation, 0.3),
                    nz: toFiniteNumber(variant.params.nz, 1),
                    permMode: String(variant.params.permMode ?? 'uniform'),
                    uniformPermX: toFiniteNumber(variant.params.uniformPermX, 100),
                    uniformPermY: toFiniteNumber(variant.params.uniformPermY ?? variant.params.uniformPermX, 100),
                    layerPermsX: Array.isArray(variant.params.layerPermsX) ? variant.params.layerPermsX.map(Number) : [],
                    layerPermsY: Array.isArray(variant.params.layerPermsY) ? variant.params.layerPermsY.map(Number) : [],
                    cellDx: toFiniteNumber(variant.params.cellDx, 10),
                    cellDy: toFiniteNumber(variant.params.cellDy, 10),
                    cellDz: toFiniteNumber(variant.params.cellDz, 1),
                    wellRadius: toFiniteNumber(variant.params.well_radius, 0.1),
                    wellSkin: toFiniteNumber(variant.params.well_skin, 0),
                    muO: toFiniteNumber(variant.params.mu_o, 1),
                    sWc: toFiniteNumber(variant.params.s_wc, 0.1),
                    sOr: toFiniteNumber(variant.params.s_or, 0.1),
                    nO: toFiniteNumber(variant.params.n_o, 2),
                    c_o: toFiniteNumber(variant.params.c_o, 1e-5),
                    c_w: toFiniteNumber(variant.params.c_w, 3e-6),
                    cRock: toFiniteNumber(variant.params.rock_compressibility, 1e-6),
                    initialPressure: toFiniteNumber(variant.params.initialPressure, 300),
                    producerBhp: toFiniteNumber(variant.params.producerBhp, 100),
                    depletionRateScale: toFiniteNumber(variant.params.analyticalDepletionRateScale, 1),
                });
            } catch {
                // Skip this variant — bad params or numerical failure, don't abort others.
                return;
            }

            const ooip = getOoip(variant.params);
            const tau = analyticalResult.meta.tau ?? null;
            const xMode = xAxisMode === 'logTime' ? 'logTime' : xAxisMode === 'tD' ? 'tD' : 'time';
            const xValues = analyticalResult.production.map((pt) => {
                if (xMode === 'logTime') return pt.time > 0 ? Math.log10(pt.time) : null;
                if (xMode === 'tD' && Number.isFinite(tau) && (tau as number) > 0) return pt.time / (tau as number);
                return pt.time;
            });

            appendSeries(panels.rates, {
                label: `${prefix}Analytical Oil Rate`,
                curveKey: 'oil-rate-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleLabel: 'Analytical Oil Rate',
                color,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, xValues, analyticalResult.production.map((pt) => pt.oilRate));
            appendSeries(panels.recovery, {
                label: `${prefix}Analytical Recovery Factor`,
                curveKey: 'recovery-factor',
                ...(caseKey ? { caseKey } : {}),
                toggleLabel: 'Analytical Recovery Factor',
                color,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, xValues, analyticalResult.production.map((pt) => (
                ooip > 1e-12 ? Math.max(0, Math.min(1, pt.cumulativeOil / ooip)) : null
            )));
            appendSeries(panels.diagnostics, {
                label: `${prefix}Analytical Avg Pressure`,
                curveKey: 'avg-pressure-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleLabel: 'Analytical Avg Pressure',
                color,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, xValues, analyticalResult.production.map((pt) => pt.avgPressure));
        });

        return panels;
    }

    return panels;
}

function buildSweepPanel(input: {
    orderedResults: BenchmarkRunResult[];
    theme: ReferenceComparisonTheme;
    analyticalPerVariant: boolean;
}): ReferenceComparisonPanel {
    const panel: ReferenceComparisonPanel = { curves: [], series: [] };
    const referenceColor = getReferenceColor(input.theme);
    const pviMax = 3.0;

    if (input.analyticalPerVariant) {
        input.orderedResults.forEach((result, index) => {
            const color = getReferenceComparisonCaseColor(index);
            const rock = extractRockProps(result.params);
            const fluid = extractFluidProps(result.params);
            const perms = getLayerPermeabilities(result.params);
            const thickness = toFiniteNumber(result.params.cellDz, 1);
            const sweep = computeCombinedSweep(rock, fluid, perms, thickness, pviMax);
            const pviValues = sweep.arealSweep.curve.map((p) => p.pvi);

            appendSeries(panel, {
                label: `${result.label} E_A`,
                curveKey: 'sweep-areal',
                caseKey: result.key,
                toggleLabel: 'Areal (E_A)',
                color,
                borderWidth: 2.0,
                borderDash: SWEEP_DASH_AREAL,
                yAxisID: 'y',
            }, pviValues, sweep.arealSweep.curve.map((p) => p.efficiency));

            appendSeries(panel, {
                label: `${result.label} E_V`,
                curveKey: 'sweep-vertical',
                caseKey: result.key,
                toggleLabel: 'Vertical (E_V)',
                color,
                borderWidth: 1.6,
                borderDash: SWEEP_DASH_VERTICAL,
                yAxisID: 'y',
                defaultVisible: false,
            }, pviValues, sweep.verticalSweep.curve.map((p) => p.efficiency));

            appendSeries(panel, {
                label: `${result.label} E_vol`,
                curveKey: 'sweep-combined',
                caseKey: result.key,
                toggleLabel: 'Combined (E_vol)',
                color,
                borderWidth: 2.4,
                borderDash: SWEEP_DASH_COMBINED,
                yAxisID: 'y',
            }, pviValues, sweep.combined.map((p) => p.efficiency));
        });
    } else {
        const baseResult = getBaseResult(input.orderedResults);
        if (!baseResult) return panel;

        const rock = extractRockProps(baseResult.params);
        const fluid = extractFluidProps(baseResult.params);
        const perms = getLayerPermeabilities(baseResult.params);
        const thickness = toFiniteNumber(baseResult.params.cellDz, 1);
        const sweep = computeCombinedSweep(rock, fluid, perms, thickness, pviMax);
        const pviValues = sweep.arealSweep.curve.map((p) => p.pvi);

        appendSeries(panel, {
            label: 'Areal (E_A)',
            curveKey: 'sweep-areal',
            toggleLabel: 'Areal (E_A)',
            color: referenceColor,
            borderWidth: 2.0,
            borderDash: SWEEP_DASH_AREAL,
            yAxisID: 'y',
        }, pviValues, sweep.arealSweep.curve.map((p) => p.efficiency));

        appendSeries(panel, {
            label: 'Vertical (E_V)',
            curveKey: 'sweep-vertical',
            toggleLabel: 'Vertical (E_V)',
            color: referenceColor,
            borderWidth: 1.6,
            borderDash: SWEEP_DASH_VERTICAL,
            yAxisID: 'y',
            defaultVisible: false,
        }, pviValues, sweep.verticalSweep.curve.map((p) => p.efficiency));

        appendSeries(panel, {
            label: 'Combined (E_vol)',
            curveKey: 'sweep-combined',
            toggleLabel: 'Combined (E_vol)',
            color: referenceColor,
            borderWidth: 2.4,
            borderDash: SWEEP_DASH_COMBINED,
            yAxisID: 'y',
        }, pviValues, sweep.combined.map((p) => p.efficiency));
    }

    return panel;
}

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
    /** Fallback single-curve preview (used when analyticalPerVariant is false). */
    previewBaseParams?: Record<string, any>;
    previewScenarioClass?: string;
}): ReferenceComparisonModel {
    const family = input.family ?? null;
    const orderedResults = orderResults(input.results);
    const referenceColor = getReferenceColor(input.theme ?? 'dark');
    const panels: Record<RateChartPanelKey, ReferenceComparisonPanel> = {
        rates: { curves: [], series: [] },
        recovery: { curves: [], series: [] },
        cumulative: { curves: [], series: [] },
        diagnostics: { curves: [], series: [] },
    };

    if (!family || orderedResults.length === 0) {
        if (orderedResults.length === 0 && input.previewScenarioClass) {
            // Prefer per-variant preview when available; fall back to single base preview.
            const variants: AnalyticalPreviewVariant[] =
                input.previewVariantParams?.length
                    ? input.previewVariantParams
                    : input.previewBaseParams
                        ? [{ label: '', variantKey: 'base', params: input.previewBaseParams }]
                        : [];
            if (variants.length > 0) {
                const previewPanels = buildAnalyticalPreviewPanels(
                    variants,
                    input.xAxisMode,
                    input.previewScenarioClass,
                    input.theme ?? 'dark',
                );
                return { orderedResults, panels: previewPanels, sweepPanel: null };
            }
        }
        return { orderedResults, panels, sweepPanel: null };
    }

    const derivedByKey = new Map<string, DerivedRunSeries>(
        orderedResults.map((result) => [result.key, buildDerivedRunSeries(result)]),
    );
    const baseResult = getBaseResult(orderedResults);

    orderedResults.forEach((result, index) => {
        const derived = derivedByKey.get(result.key);
        if (!derived) return;
        const color = getReferenceComparisonCaseColor(index);
        const xValues = buildXAxisValues(derived, input.xAxisMode);
        const defaultVisible = true;

        if (family.scenarioClass === 'buckley-leverett') {
            appendSeries(
                panels.rates,
                {
                    label: `${result.label} Water Cut`,
                    curveKey: 'water-cut-sim',
                    caseKey: result.key,
                    toggleLabel: 'Water Cut',
                    color,
                    borderWidth: result.variantKey === null ? 2.8 : 2.2,
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.waterCut,
            );
            appendSeries(
                panels.rates,
                {
                    label: `${result.label} Avg Water Sat`,
                    curveKey: 'avg-water-sat',
                    caseKey: result.key,
                    toggleLabel: 'Avg Water Sat',
                    color,
                    borderWidth: 1.6,
                    borderDash: [2, 4],
                    yAxisID: 'y',
                    defaultVisible: false,
                },
                xValues,
                derived.avgWaterSat,
            );
            appendSeries(
                panels.recovery,
                {
                    label: `${result.label} Recovery`,
                    curveKey: 'recovery-factor',
                    caseKey: result.key,
                    toggleLabel: 'Recovery Factor',
                    color,
                    borderWidth: result.variantKey === null ? 2.8 : 2.2,
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.recovery,
            );
            appendSeries(
                panels.cumulative,
                {
                    label: `${result.label} Cum Oil`,
                    curveKey: 'cum-oil-sim',
                    caseKey: result.key,
                    toggleLabel: 'Cum Oil',
                    color,
                    borderWidth: 1.4,
                    borderDash: [5, 4],
                    yAxisID: 'y',
                    defaultVisible: false,
                },
                xValues,
                derived.cumulativeOil,
            );
            appendSeries(
                panels.cumulative,
                {
                    label: `${result.label} Cum Injection`,
                    curveKey: 'cum-injection',
                    caseKey: result.key,
                    toggleLabel: 'Cum Injection',
                    color,
                    borderWidth: 1.2,
                    borderDash: [3, 3],
                    yAxisID: 'y',
                    defaultVisible: false,
                },
                xValues,
                derived.cumulativeInjection,
            );
            appendSeries(
                panels.diagnostics,
                {
                    label: `${result.label} Avg Pressure`,
                    curveKey: 'avg-pressure-sim',
                    caseKey: result.key,
                    toggleLabel: 'Avg Pressure',
                    color,
                    borderWidth: result.variantKey === null ? 2.8 : 2.2,
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.pressure,
            );
            return;
        }

        appendSeries(
            panels.rates,
            {
                label: `${result.label} Oil Rate`,
                curveKey: 'oil-rate-sim',
                caseKey: result.key,
                toggleLabel: 'Oil Rate',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.oilRate,
        );
        appendSeries(
            panels.recovery,
            {
                label: `${result.label} Recovery`,
                curveKey: 'recovery-factor',
                caseKey: result.key,
                toggleLabel: 'Recovery Factor',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.recovery,
        );
        appendSeries(
            panels.cumulative,
            {
                label: `${result.label} Cum Oil`,
                curveKey: 'cum-oil-sim',
                caseKey: result.key,
                toggleLabel: 'Cum Oil',
                color,
                borderWidth: 1.4,
                borderDash: [5, 4],
                yAxisID: 'y',
                defaultVisible: false,
            },
            xValues,
            derived.cumulativeOil,
        );
        appendSeries(
            panels.diagnostics,
            {
                label: `${result.label} Avg Pressure`,
                curveKey: 'avg-pressure-sim',
                caseKey: result.key,
                toggleLabel: 'Avg Pressure',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.pressure,
        );
    });

    if (!baseResult) {
        return { orderedResults, panels, sweepPanel: null };
    }

    const baseDerived = derivedByKey.get(baseResult.key);
    if (!baseDerived) {
        return { orderedResults, panels, sweepPanel: null };
    }

    if (family.scenarioClass === 'buckley-leverett') {
        const allSameAnalytical = !input.analyticalPerVariant;

        if (allSameAnalytical) {
            // Shared reference — one curve for all (analytical is grid/timestep-independent).
            const refOverlay = buildBuckleyLeverettReference(baseResult, baseDerived, input.xAxisMode);
            if (refOverlay.rates) {
                appendSeries(panels.rates, {
                    label: 'Reference Solution Water Cut',
                    curveKey: 'water-cut-reference',
                    toggleLabel: 'Reference Solution Water Cut',
                    color: referenceColor,
                    borderWidth: 2,
                    borderDash: [7, 4],
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.rates.values);
            }
            if (refOverlay.cumulative) {
                appendSeries(panels.recovery, {
                    label: 'Reference Solution Recovery',
                    curveKey: 'recovery-factor',
                    toggleLabel: 'Reference Solution Recovery',
                    color: referenceColor,
                    borderWidth: 2,
                    borderDash: [7, 4],
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                appendSeries(panels.cumulative, {
                    label: 'Reference Solution Cum Oil',
                    curveKey: 'cum-oil-reference',
                    toggleLabel: 'Reference Solution Cum Oil',
                    color: referenceColor,
                    borderWidth: 1.4,
                    borderDash: [3, 5],
                    yAxisID: 'y',
                    defaultVisible: false,
                }, refOverlay.xValues, refOverlay.cumulative.cumulativeValues);
            }
        } else {
            // Per-result analytical — viscosity/rel-perm differ so each result gets
            // its own analytical curve in the same color as its simulation (dashed).
            orderedResults.forEach((result, index) => {
                const derived = derivedByKey.get(result.key);
                if (!derived) return;
                const color = getReferenceComparisonCaseColor(index);
                const refOverlay = buildBuckleyLeverettReference(result, derived, input.xAxisMode);
                if (refOverlay.rates) {
                    appendSeries(panels.rates, {
                        label: `${result.label} — Reference`,
                        curveKey: 'water-cut-reference',
                        caseKey: result.key,
                        toggleLabel: 'Reference Water Cut',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.rates.values);
                }
                if (refOverlay.cumulative) {
                    appendSeries(panels.recovery, {
                        label: `${result.label} — Reference Recovery`,
                        curveKey: 'recovery-factor',
                        caseKey: result.key,
                        toggleLabel: 'Reference Recovery',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                }
            });
        }
    } else {
        // Depletion: single shared reference from base result.
        const refOverlay = buildDepletionReference(baseResult, baseDerived, input.xAxisMode);
        if (refOverlay.rates) {
            appendSeries(panels.rates, {
                label: refOverlay.rates.label,
                curveKey: 'oil-rate-reference',
                toggleLabel: refOverlay.rates.label,
                color: referenceColor,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, refOverlay.xValues, refOverlay.rates.values);
        }
        if (refOverlay.cumulative) {
            appendSeries(panels.recovery, {
                label: refOverlay.cumulative.recoveryLabel,
                curveKey: 'recovery-factor',
                toggleLabel: 'Reference Solution Recovery',
                color: referenceColor,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
            appendSeries(panels.cumulative, {
                label: refOverlay.cumulative.cumulativeLabel,
                curveKey: 'cum-oil-reference',
                toggleLabel: refOverlay.cumulative.cumulativeLabel,
                color: referenceColor,
                borderWidth: 1.4,
                borderDash: [3, 5],
                yAxisID: 'y',
                defaultVisible: false,
            }, refOverlay.xValues, refOverlay.cumulative.cumulativeValues);
        }
        if (refOverlay.diagnostics) {
            appendSeries(panels.diagnostics, {
                label: refOverlay.diagnostics.label,
                curveKey: 'avg-pressure-reference',
                toggleLabel: refOverlay.diagnostics.label,
                color: referenceColor,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, refOverlay.xValues, refOverlay.diagnostics.values);
        }
    }

    const sweepPanel = (family.scenarioClass === 'buckley-leverett' && family.showSweepPanel === true)
        ? buildSweepPanel({
            orderedResults,
            theme: input.theme ?? 'dark',
            analyticalPerVariant: input.analyticalPerVariant ?? false,
        })
        : null;

    return { orderedResults, panels, sweepPanel };
}