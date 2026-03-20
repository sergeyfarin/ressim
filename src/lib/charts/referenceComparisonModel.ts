import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { calculateAnalyticalProduction, calculateGasOilAnalyticalProduction } from '../analytical/fractionalFlow';
import type { RockProps, FluidProps, GasOilRockProps, GasOilFluidProps } from '../analytical/fractionalFlow';
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
    /**
     * Preview/pending variant entries for the cases selector UI.
     * Populated when:
     *  - Pure preview (no results): multi-variant analytical preview cases.
     *  - Mid-sweep (some results done): remaining queued/running variants.
     * Empty when all variants have completed results (orderedResults covers everything).
     */
    previewCases: ReferenceComparisonPreviewCase[];
    panels: Record<RateChartPanelKey, ReferenceComparisonPanel>;
    sweepPanel: ReferenceComparisonPanel | null;
    axisMappingWarning: string | null;
};

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
    gasCut: Array<number | null>;
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

function requiresRunMappedAnalyticalXAxis(
    scenarioClass: string | null | undefined,
    xAxisMode: RateChartXAxisMode,
): boolean {
    if (scenarioClass === 'buckley-leverett' || scenarioClass === 'waterflood' || scenarioClass === 'gas-oil-bl') {
        return xAxisMode !== 'pvi';
    }
    return false;
}

function buildAnalyticalAxisWarning(input: {
    usesRunMappedAnalyticalXAxis: boolean;
    hidesPendingAnalyticalWithoutMapping: boolean;
}): string | null {
    const parts: string[] = [];
    if (input.usesRunMappedAnalyticalXAxis) {
        parts.push('Analytical overlays on this axis are remapped from each completed simulation run.');
    }
    if (input.hidesPendingAnalyticalWithoutMapping) {
        parts.push('Analytical curves without completed simulation runs are hidden on this axis until remapping data exists.');
    }
    return parts.length > 0 ? parts.join(' ') : null;
}

/** Tableau 20 — 20 perceptually distinct colors for categorical data. */
const CASE_COLORS = [
    '#4e79a7',
    '#f28e2b',
    '#e15759',
    '#76b7b2',
    '#59a14f',
    '#edc948',
    '#b07aa1',
    '#ff9da7',
    '#9c755f',
    '#bab0ac',
    '#af7aa1',
    '#d37295',
    '#fabfd2',
    '#b6992d',
    '#499894',
    '#86bcb6',
    '#8cd17d',
    '#f1ce63',
    '#a0cbe8',
    '#ffbe7d',
];

export function getReferenceComparisonCaseColor(index: number): string {
    return CASE_COLORS[index % CASE_COLORS.length];
}

function getReferenceColor(theme: ReferenceComparisonTheme): string {
    return theme === 'dark' ? '#f8fafc' : '#0f172a';
}

/** Neutral grey used as the toggle-group line indicator for simulation and
 *  analytical legend items in comparison charts. The actual line colors come
 *  from the case-color palette (shown in the Cases selector above the charts). */
function getLegendGrey(theme: ReferenceComparisonTheme): string {
    return theme === 'dark' ? '#94a3b8' : '#64748b';
}

/**
 * Strips the scenario-name prefix from a case label so sub-panel legend buttons
 * stay compact. E.g. "Rate Decline — s=0 (clean)" → "s=0 (clean)".
 * Falls back to the full label when no separator is found.
 */
function compactCaseLabel(label: string): string {
    const emDash = label.indexOf(' — ');
    if (emDash !== -1) return label.slice(emDash + 3).trim();
    const hyphen = label.indexOf(' - ');
    if (hyphen !== -1) return label.slice(hyphen + 3).trim();
    return label;
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
        gasCut: result.rateHistory.map((point) => {
            const gasRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_gas, 0)));
            const oilRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0)));
            const total = gasRate + oilRate;
            return total > 1e-12 ? gasRate / total : 0;
        }),
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
        nx: toFiniteNumber(baseResult.params.nx, 1),
        ny: toFiniteNumber(baseResult.params.ny, 1),
        producerI: baseResult.params.producerI != null ? toFiniteNumber(baseResult.params.producerI, 0) : undefined,
        producerJ: baseResult.params.producerJ != null ? toFiniteNumber(baseResult.params.producerJ, 0) : undefined,
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

function extractGasOilRockProps(params: Record<string, any>): GasOilRockProps {
    return {
        s_wc: toFiniteNumber(params.s_wc, 0.2),
        s_gc: toFiniteNumber(params.s_gc, 0.05),
        s_gr: toFiniteNumber(params.s_gr, 0.05),
        s_org: toFiniteNumber(params.s_org, 0.20),
        n_o: toFiniteNumber(params.n_o, 2),
        n_g: toFiniteNumber(params.n_g, 1.5),
        k_ro_max: toFiniteNumber(params.k_ro_max, 1),
        k_rg_max: toFiniteNumber(params.k_rg_max, 0.8),
    };
}

function extractGasOilFluidProps(params: Record<string, any>): GasOilFluidProps {
    return {
        mu_o: toFiniteNumber(params.mu_o, 2),
        mu_g: toFiniteNumber(params.mu_g, 0.02),
    };
}

function computeGasOilBLAnalyticalFromParams(params: Record<string, any>): {
    pviValues: number[];
    gasCut: Array<number | null>;
    recovery: Array<number | null>;
} | null {
    const N = 150;
    const pviMax = 3.0;
    const pviValues = Array.from({ length: N }, (_, i) => (i / (N - 1)) * pviMax);
    const injRates = new Array(N).fill(1);
    let analyticalProduction: Array<{ oilRate: number; gasRate: number; cumulativeOil: number }>;
    try {
        analyticalProduction = calculateGasOilAnalyticalProduction(
            extractGasOilRockProps(params),
            extractGasOilFluidProps(params),
            toFiniteNumber(params.initialGasSaturation, 0),
            pviValues,
            injRates,
            1, // unit pore volume — PVI = cumulative injection directly
        );
    } catch {
        return null;
    }
    if (!analyticalProduction.length) return null;

    const s_wc = toFiniteNumber(params.s_wc, 0.2);
    const ooip = 1 - s_wc; // oil initially in place as fraction of PV

    const gasCut = analyticalProduction.map((point) => {
        const total = Math.max(0, point.oilRate + point.gasRate);
        return total > 1e-12 ? point.gasRate / total : 0;
    });
    const recovery = analyticalProduction.map((point) => (
        ooip > 1e-12 ? Math.max(0, Math.min(1, point.cumulativeOil / ooip)) : null
    ));
    return { pviValues, gasCut, recovery };
}

function buildGasOilBLReference(
    baseResult: BenchmarkRunResult,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
): AnalyticalOverlay {
    const poreVolume = getPoreVolume(baseResult.params);
    const ooip = getOoip(baseResult.params);
    const analyticalProduction = calculateGasOilAnalyticalProduction(
        extractGasOilRockProps(baseResult.params),
        extractGasOilFluidProps(baseResult.params),
        toFiniteNumber(baseResult.params.initialGasSaturation, 0),
        derived.time,
        baseResult.rateHistory.map((point) => Math.max(0, toFiniteNumber(point.total_injection, 0))),
        poreVolume,
    );

    const gasCut = analyticalProduction.map((point) => {
        const total = Math.max(0, point.oilRate + point.gasRate);
        return total > 1e-12 ? point.gasRate / total : 0;
    });
    const recovery = analyticalProduction.map((point) => (
        ooip > 1e-12 ? Math.max(0, Math.min(1, point.cumulativeOil / ooip)) : null
    ));

    return {
        rates: { label: 'Reference Solution Gas Cut', values: gasCut },
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

function computeDepletionTau(params: Record<string, any>): number | null {
    try {
        const result = calculateDepletionAnalyticalProduction({
            reservoir: {
                length: toFiniteNumber(params.nx, 1) * toFiniteNumber(params.cellDx, 10),
                area: toFiniteNumber(params.ny, 1)
                    * toFiniteNumber(params.cellDy, 10)
                    * toFiniteNumber(params.nz, 1)
                    * toFiniteNumber(params.cellDz, 1),
                porosity: toFiniteNumber(params.reservoirPorosity ?? params.porosity, 0.2),
            },
            timeHistory: [1],
            initialSaturation: toFiniteNumber(params.initialSaturation, 0.3),
            nz: toFiniteNumber(params.nz, 1),
            permMode: String(params.permMode ?? 'uniform'),
            uniformPermX: toFiniteNumber(params.uniformPermX, 100),
            uniformPermY: toFiniteNumber(params.uniformPermY ?? params.uniformPermX, 100),
            layerPermsX: Array.isArray(params.layerPermsX) ? params.layerPermsX.map(Number) : [],
            layerPermsY: Array.isArray(params.layerPermsY) ? params.layerPermsY.map(Number) : [],
            cellDx: toFiniteNumber(params.cellDx, 10),
            cellDy: toFiniteNumber(params.cellDy, 10),
            cellDz: toFiniteNumber(params.cellDz, 1),
            wellRadius: toFiniteNumber(params.well_radius, 0.1),
            wellSkin: toFiniteNumber(params.well_skin, 0),
            muO: toFiniteNumber(params.mu_o, 1),
            sWc: toFiniteNumber(params.s_wc, 0.1),
            sOr: toFiniteNumber(params.s_or, 0.1),
            nO: toFiniteNumber(params.n_o, 2),
            c_o: toFiniteNumber(params.c_o, 1e-5),
            c_w: toFiniteNumber(params.c_w, 3e-6),
            cRock: toFiniteNumber(params.rock_compressibility, 1e-6),
            initialPressure: toFiniteNumber(params.initialPressure, 300),
            producerBhp: toFiniteNumber(params.producerBhp, 100),
            depletionRateScale: toFiniteNumber(params.analyticalDepletionRateScale, 1),
            nx: toFiniteNumber(params.nx, 1),
            ny: toFiniteNumber(params.ny, 1),
            producerI: params.producerI != null ? toFiniteNumber(params.producerI, 0) : undefined,
            producerJ: params.producerJ != null ? toFiniteNumber(params.producerJ, 0) : undefined,
        });
        return result.meta.tau ?? null;
    } catch {
        return null;
    }
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

/**
 * Shared Buckley-Leverett analytical computation over a fixed unit PVI grid [0..3].
 *
 * Used both by the all-analytical preview (no results yet) and by the mid-sweep
 * pending-variant overlay (some results done, others still queued). Returns null
 * when the calculation fails so callers can skip the curve independently.
 */
function computeBLAnalyticalFromParams(params: Record<string, any>): {
    pviValues: number[];
    waterCut: Array<number | null>;
    recovery: Array<number | null>;
} | null {
    const N = 150;
    const pviMax = 3.0;
    const pviValues = Array.from({ length: N }, (_, i) => (i / (N - 1)) * pviMax);
    const injRates = new Array(N).fill(1);
    let analyticalProduction: Array<{ oilRate: number; waterRate: number; cumulativeOil: number }>;
    try {
        analyticalProduction = calculateAnalyticalProduction(
            extractRockProps(params),
            extractFluidProps(params),
            toFiniteNumber(params.initialSaturation, toFiniteNumber(params.s_wc, 0.1)),
            pviValues,
            injRates,
            1, // poreVolume = 1 → time = PVI
        );
    } catch {
        return null;
    }
    const waterCut = analyticalProduction.map((pt) => {
        const total = Math.max(0, pt.oilRate + pt.waterRate);
        return total > 1e-12 ? pt.waterRate / total : 0;
    });
    const recovery = analyticalProduction.map((pt) =>
        Math.max(0, Math.min(1, pt.cumulativeOil / 1)), // ooip = 1 (unit pore volume)
    );
    return { pviValues, waterCut, recovery };
}

/**
 * Shared depletion analytical computation over a synthesised time grid derived
 * from scenario params (steps × delta_t_days). Used for preview and pending-
 * variant overlays where no simulation result (and therefore no real time axis)
 * is available yet.
 *
 * Returns null on numerical failure so callers can skip the curve independently.
 */
function computeDepletionAnalyticalFromParams(
    params: Record<string, any>,
    xAxisMode: RateChartXAxisMode,
): {
    xValues: (number | null)[];
    oilRates: (number | null)[];
    recoveryValues: (number | null)[];
    cumulativeOilValues: (number | null)[];
    avgPressureValues: (number | null)[];
} | null {
    const steps = toFiniteNumber(params.steps, 200);
    const dt = toFiniteNumber(params.delta_t_days, 5);
    const timeHistory = Array.from({ length: steps }, (_, i) => (i + 1) * dt);

    let analyticalResult: ReturnType<typeof calculateDepletionAnalyticalProduction>;
    try {
        analyticalResult = calculateDepletionAnalyticalProduction({
            reservoir: {
                length: toFiniteNumber(params.nx, 1) * toFiniteNumber(params.cellDx, 10),
                area: toFiniteNumber(params.ny, 1)
                    * toFiniteNumber(params.cellDy, 10)
                    * toFiniteNumber(params.nz, 1)
                    * toFiniteNumber(params.cellDz, 1),
                porosity: toFiniteNumber(params.reservoirPorosity ?? params.porosity, 0.2),
            },
            timeHistory,
            initialSaturation: toFiniteNumber(params.initialSaturation, 0.3),
            nz: toFiniteNumber(params.nz, 1),
            permMode: String(params.permMode ?? 'uniform'),
            uniformPermX: toFiniteNumber(params.uniformPermX, 100),
            uniformPermY: toFiniteNumber(params.uniformPermY ?? params.uniformPermX, 100),
            layerPermsX: Array.isArray(params.layerPermsX) ? params.layerPermsX.map(Number) : [],
            layerPermsY: Array.isArray(params.layerPermsY) ? params.layerPermsY.map(Number) : [],
            cellDx: toFiniteNumber(params.cellDx, 10),
            cellDy: toFiniteNumber(params.cellDy, 10),
            cellDz: toFiniteNumber(params.cellDz, 1),
            wellRadius: toFiniteNumber(params.well_radius, 0.1),
            wellSkin: toFiniteNumber(params.well_skin, 0),
            muO: toFiniteNumber(params.mu_o, 1),
            sWc: toFiniteNumber(params.s_wc, 0.1),
            sOr: toFiniteNumber(params.s_or, 0.1),
            nO: toFiniteNumber(params.n_o, 2),
            c_o: toFiniteNumber(params.c_o, 1e-5),
            c_w: toFiniteNumber(params.c_w, 3e-6),
            cRock: toFiniteNumber(params.rock_compressibility, 1e-6),
            initialPressure: toFiniteNumber(params.initialPressure, 300),
            producerBhp: toFiniteNumber(params.producerBhp, 100),
            depletionRateScale: toFiniteNumber(params.analyticalDepletionRateScale, 1),
            nx: toFiniteNumber(params.nx, 1),
            ny: toFiniteNumber(params.ny, 1),
            producerI: params.producerI != null ? toFiniteNumber(params.producerI, 0) : undefined,
            producerJ: params.producerJ != null ? toFiniteNumber(params.producerJ, 0) : undefined,
        });
    } catch {
        return null;
    }

    const ooip = getOoip(params);
    const tau = analyticalResult.meta.tau ?? null;
    const xMode = xAxisMode === 'logTime' ? 'logTime' : xAxisMode === 'tD' ? 'tD' : 'time';
    const xValues = analyticalResult.production.map((pt) => {
        if (xMode === 'logTime') return pt.time > 0 ? Math.log10(pt.time) : null;
        if (xMode === 'tD' && Number.isFinite(tau) && (tau as number) > 0) return pt.time / (tau as number);
        return pt.time;
    });

    return {
        xValues,
        oilRates: analyticalResult.production.map((pt) => pt.oilRate),
        recoveryValues: analyticalResult.production.map((pt) =>
            ooip > 1e-12 ? Math.max(0, Math.min(1, pt.cumulativeOil / ooip)) : null,
        ),
        cumulativeOilValues: analyticalResult.production.map((pt) => pt.cumulativeOil),
        avgPressureValues: analyticalResult.production.map((pt) => pt.avgPressure),
    };
}

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
        volumes: { curves: [], series: [] },
        oil_rate: { curves: [], series: [] },
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

    if (scenarioClass === 'buckley-leverett' || scenarioClass === 'waterflood') {
        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;

            const curves = computeBLAnalyticalFromParams(variant.params);
            if (!curves) return; // numerical failure — skip, don't abort other variants

            appendSeries(panels.rates, {
                label: `${prefix}Analytical Water Cut`,
                curveKey: 'water-cut-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, curves.pviValues, curves.waterCut);
            appendSeries(panels.recovery, {
                label: `${prefix}Analytical Recovery Factor`,
                curveKey: 'recovery-factor-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, curves.pviValues, curves.recovery);
        });

        return panels;
    }

    if (scenarioClass === 'depletion') {
        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;

            const curves = computeDepletionAnalyticalFromParams(variant.params, xAxisMode);
            if (!curves) return; // bad params or numerical failure — skip, don't abort others

            appendSeries(panels.rates, {
                label: `${prefix}Analytical Oil Rate`,
                curveKey: 'oil-rate-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: 2,
                borderDash: [7, 4],
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
                borderWidth: 2,
                borderDash: [7, 4],
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
                borderWidth: 2,
                borderDash: [7, 4],
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
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, curves.xValues, curves.avgPressureValues);
        });

        return panels;
    }

    if (scenarioClass === 'gas-oil-bl') {
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
                borderWidth: 2,
                borderDash: [7, 4],
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
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            }, curves.pviValues, curves.recovery);
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
    previewScenarioClass?: string;
}): ReferenceComparisonModel {
    const family = input.family ?? null;
    const orderedResults = orderResults(input.results, input.previewVariantParams);
    const referenceColor = getReferenceColor(input.theme ?? 'dark');
    const legendGrey = getLegendGrey(input.theme ?? 'dark');
    const scenarioClass = family?.scenarioClass ?? input.previewScenarioClass ?? null;
    const usesRunMappedAnalyticalXAxis = requiresRunMappedAnalyticalXAxis(
        scenarioClass,
        input.xAxisMode,
    );
    let hidesPendingAnalyticalWithoutMapping = false;

    const panels: Record<RateChartPanelKey, ReferenceComparisonPanel> = {
        rates: { curves: [], series: [] },
        recovery: { curves: [], series: [] },
        cumulative: { curves: [], series: [] },
        diagnostics: { curves: [], series: [] },
        volumes: { curves: [], series: [] },
        oil_rate: { curves: [], series: [] },
    };

    if (!family || orderedResults.length === 0) {
        if (orderedResults.length === 0 && input.previewScenarioClass) {
            if (requiresRunMappedAnalyticalXAxis(input.previewScenarioClass, input.xAxisMode)) {
                hidesPendingAnalyticalWithoutMapping = Boolean(
                    input.previewBaseParams || (input.previewVariantParams?.length ?? 0) > 0,
                );
                return {
                    orderedResults,
                    previewCases: [],
                    panels,
                    sweepPanel: null,
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
                const previewPanels = buildAnalyticalPreviewPanels(
                    variants,
                    input.xAxisMode,
                    input.previewScenarioClass,
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
                    panels: previewPanels,
                    sweepPanel: null,
                    axisMappingWarning: null,
                };
            }
        }
        return { orderedResults, previewCases: [], panels, sweepPanel: null, axisMappingWarning: null };
    }

    const derivedByKey = new Map<string, DerivedRunSeries>(
        orderedResults.map((result) => [result.key, buildDerivedRunSeries(result)]),
    );
    const baseResult = getBaseResult(orderedResults);

    orderedResults.forEach((result, index) => {
        const derived = derivedByKey.get(result.key);
        if (!derived) return;
        const color = getReferenceComparisonCaseColor(index);
        const tau = scenarioClass === 'depletion' ? computeDepletionTau(result.params) : null;
        const xValues = buildXAxisValues(derived, input.xAxisMode, tau);
        const defaultVisible = true;

        const caseLabel = compactCaseLabel(result.label);

        if (family.scenarioClass === 'buckley-leverett') {
            appendSeries(
                panels.rates,
                {
                    label: `${result.label} Water Cut`,
                    curveKey: 'water-cut-sim',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: 'Simulation (solid lines):',
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
                    // No toggleGroupKey override — falls back to curveKey so all cases
                    // share one "Avg Sw" toggle, keeping it out of the per-case section.
                    toggleLabel: 'Avg Sw',
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
                    curveKey: 'recovery-factor-primary',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: 'Simulation (solid lines):',
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
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: 'Simulation (solid lines):',
                    color,
                    borderWidth: result.variantKey === null ? 2.8 : 2.2,
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.cumulativeOil,
            );
            appendSeries(
                panels.oil_rate,
                {
                    label: `${result.label} Oil Rate`,
                    curveKey: 'oil-rate-sim',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: 'Simulation (solid lines):',
                    color,
                    borderWidth: result.variantKey === null ? 2.8 : 2.2,
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.oilRate,
            );
            appendSeries(
                panels.volumes,
                {
                    label: `${result.label} Cum Injection`,
                    curveKey: 'cum-injection',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: 'Simulation (solid lines):',
                    color,
                    borderWidth: result.variantKey === null ? 2.8 : 2.2,
                    yAxisID: 'y',
                    defaultVisible,
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
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: 'Simulation (solid lines):',
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

        if (family.scenarioClass === 'gas-oil-bl') {
            appendSeries(panels.rates, {
                label: `${result.label} Gas Cut`,
                curveKey: 'gas-cut-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: 'Simulation (solid lines):',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
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
                legendSectionLabel: 'Simulation (solid lines):',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
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
                legendSectionLabel: 'Simulation (solid lines):',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
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
                legendSectionLabel: 'Simulation (solid lines):',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.oilRate);
            appendSeries(panels.volumes, {
                label: `${result.label} Cum Injection`,
                curveKey: 'cum-injection',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: 'Simulation (solid lines):',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
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
                legendSectionLabel: 'Simulation (solid lines):',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.pressure);
            return;
        }

        appendSeries(
            panels.rates,
            {
                label: `${result.label} Oil Rate`,
                curveKey: 'oil-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: 'Simulation (solid lines):',
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
                curveKey: 'recovery-factor-primary',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: 'Simulation (solid lines):',
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
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: 'Simulation (solid lines):',
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
                yAxisID: 'y',
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
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: 'Simulation (solid lines):',
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
        return {
            orderedResults,
            previewCases: [],
            panels,
            sweepPanel: null,
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
            panels,
            sweepPanel: null,
            axisMappingWarning: buildAnalyticalAxisWarning({
                usesRunMappedAnalyticalXAxis,
                hidesPendingAnalyticalWithoutMapping,
            }),
        };
    }

    if (family.scenarioClass === 'buckley-leverett') {
        const allSameAnalytical = !input.analyticalPerVariant;

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
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
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
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                appendSeries(panels.cumulative, {
                    label: 'Reference Solution Cum Oil',
                    curveKey: 'cum-oil-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.cumulativeValues);
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
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
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
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                }
            });

            // Analytical-only overlay for variants still queued/running.
            // Color indices continue from orderedResults.length so each variant
            // keeps the same color from initial preview → in-progress → completed.
            // Uses the same unit-PVI grid as the all-analytical preview for a
            // consistent x-axis (BL scenarios default to PVI).
            if (input.pendingPreviewVariants?.length) {
                if (usesRunMappedAnalyticalXAxis) {
                    hidesPendingAnalyticalWithoutMapping = true;
                }
                if (!usesRunMappedAnalyticalXAxis) {
                input.pendingPreviewVariants.forEach((variant, i) => {
                    const color = getReferenceComparisonCaseColor(orderedResults.length + i);
                    const curves = computeBLAnalyticalFromParams(variant.params);
                    if (!curves) return; // bad params — skip this variant
                    const vLabel = compactCaseLabel(variant.label);
                    appendSeries(panels.rates, {
                        label: `${variant.label} — Reference`,
                        curveKey: 'water-cut-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, curves.pviValues, curves.waterCut);
                    appendSeries(panels.recovery, {
                        label: `${variant.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, curves.pviValues, curves.recovery);
                });
                }
            }
        }
    } else if (family.scenarioClass === 'gas-oil-bl') {
        const allSameAnalytical = !input.analyticalPerVariant;

        if (allSameAnalytical && !usesRunMappedAnalyticalXAxis) {
            const refOverlay = buildGasOilBLReference(baseResult, baseDerived, input.xAxisMode);
            if (refOverlay.rates) {
                appendSeries(panels.rates, {
                    label: 'Reference Solution Gas Cut',
                    curveKey: 'gas-cut-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
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
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                appendSeries(panels.cumulative, {
                    label: 'Reference Solution Cum Oil',
                    curveKey: 'cum-oil-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
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
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
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
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
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
                            legendSectionLabel: 'Analytical (dashed lines):',
                            color,
                            borderWidth: 1.5,
                            borderDash: [7, 4],
                            yAxisID: 'y',
                        }, curves.pviValues, curves.gasCut);
                        appendSeries(panels.recovery, {
                            label: `${variant.label} — Reference Recovery`,
                            curveKey: 'recovery-factor-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: 'Analytical (dashed lines):',
                            color,
                            borderWidth: 1.5,
                            borderDash: [7, 4],
                            yAxisID: 'y',
                        }, curves.pviValues, curves.recovery);
                    });
                }
            }
        }
    } else {
        // Depletion path.
        if (input.analyticalPerVariant || usesRunMappedAnalyticalXAxis) {
            // Per-result analytical — each variant gets its own dashed reference curve
            // in its case color so the user can directly compare sim vs. analytical.
            // This path also handles shared analytical physics on axes whose x-values
            // are derived from each completed run's simulation history.
            orderedResults.forEach((result, index) => {
                const derived = derivedByKey.get(result.key);
                if (!derived) return;
                const color = getReferenceComparisonCaseColor(index);
                const caseLabel = compactCaseLabel(result.label);
                let refOverlay: ReturnType<typeof buildDepletionReference>;
                try {
                    refOverlay = buildDepletionReference(result, derived, input.xAxisMode);
                } catch {
                    return; // bad params — skip this result's reference curve
                }
                if (refOverlay.rates) {
                    appendSeries(panels.rates, {
                        label: `${result.label} — Reference`,
                        curveKey: 'oil-rate-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
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
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                    appendSeries(panels.cumulative, {
                        label: `${result.label} — Reference Cum Oil`,
                        curveKey: 'cum-oil-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
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
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.diagnostics.values);
                }
            });

            // Analytical-only overlay for variants still queued/running (pending).
            // Color indices continue from orderedResults.length so each variant
            // keeps its color throughout preview → in-progress → completed.
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
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, curves.xValues, curves.oilRates);
                    appendSeries(panels.recovery, {
                        label: `${variant.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, curves.xValues, curves.recoveryValues);
                    appendSeries(panels.cumulative, {
                        label: `${variant.label} — Reference Cum Oil`,
                        curveKey: 'cum-oil-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
                        yAxisID: 'y',
                    }, curves.xValues, curves.cumulativeOilValues);
                    appendSeries(panels.diagnostics, {
                        label: `${variant.label} — Reference Pressure`,
                        curveKey: 'avg-pressure-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: 'Analytical (dashed lines):',
                        color,
                        borderWidth: 1.5,
                        borderDash: [7, 4],
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
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
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
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                appendSeries(panels.cumulative, {
                    label: refOverlay.cumulative.cumulativeLabel,
                    curveKey: 'cum-oil-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
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
                    legendSectionLabel: 'Analytical (dashed lines):',
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: 2,
                    borderDash: [7, 4],
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
        (input.pendingPreviewVariants?.length && input.analyticalPerVariant && !usesRunMappedAnalyticalXAxis)
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

    const sweepPanel = (family.scenarioClass === 'buckley-leverett' && family.showSweepPanel === true)
        ? buildSweepPanel({
            orderedResults,
            theme: input.theme ?? 'dark',
            analyticalPerVariant: input.analyticalPerVariant ?? false,
        })
        : null;

    return {
        orderedResults,
        previewCases: pendingPreviewCases,
        panels,
        sweepPanel,
        axisMappingWarning: buildAnalyticalAxisWarning({
            usesRunMappedAnalyticalXAxis,
            hidesPendingAnalyticalWithoutMapping,
        }),
    };
}