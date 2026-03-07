import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { calculateAnalyticalProduction } from '../analytical/fractionalFlow';
import type { BenchmarkFamily } from '../catalog/benchmarkCases';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import type { CurveConfig } from './ChartSubPanel.svelte';
import type { RateChartPanelKey, RateChartXAxisMode } from './rateChartLayoutConfig';

export type XYPoint = { x: number; y: number | null };

export type BenchmarkOverlayPanel = {
    curves: CurveConfig[];
    series: XYPoint[][];
};

export type BenchmarkOverlayModel = {
    orderedResults: BenchmarkRunResult[];
    panels: Record<RateChartPanelKey, BenchmarkOverlayPanel>;
};

export type BenchmarkOverlayTheme = 'dark' | 'light';

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

function getReferenceColor(theme: BenchmarkOverlayTheme): string {
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
    if (xAxisMode === 'pvi') return derived.pvi;
    if (xAxisMode === 'pvp') return derived.pvp;
    if (xAxisMode === 'cumInjection') return derived.cumulativeInjection;
    if (xAxisMode === 'cumLiquid') return derived.cumulativeLiquid;
    if (xAxisMode === 'logTime') return derived.time.map((value) => (value > 0 ? Math.log10(value) : null));
    if (xAxisMode === 'tD' && Number.isFinite(tau) && (tau as number) > 0) {
        return derived.time.map((value) => value / (tau as number));
    }
    return derived.time;
}

function getBaseResult(results: BenchmarkRunResult[]): BenchmarkRunResult | null {
    return results.find((result) => result.variantKey === null) ?? results[0] ?? null;
}

function orderResults(results: BenchmarkRunResult[]): BenchmarkRunResult[] {
    return [...results].sort((left, right) => {
        if (left.variantKey === null && right.variantKey !== null) return -1;
        if (left.variantKey !== null && right.variantKey === null) return 1;
        return left.label.localeCompare(right.label);
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
        rates: { label: 'Analytical Water Cut', values: waterCut },
        cumulative: {
            recoveryLabel: 'Analytical Recovery',
            recoveryValues: recovery,
            cumulativeLabel: 'Analytical Cum Oil',
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
            label: 'Analytical Oil Rate',
            values: analyticalResult.production.map((point) => point.oilRate),
        },
        cumulative: {
            recoveryLabel: 'Analytical Recovery',
            recoveryValues: analyticalResult.production.map((point) => (
                ooip > 1e-12 ? Math.max(0, Math.min(1, point.cumulativeOil / ooip)) : null
            )),
            cumulativeLabel: 'Analytical Cum Oil',
            cumulativeValues: analyticalResult.production.map((point) => point.cumulativeOil),
        },
        diagnostics: {
            label: 'Analytical Avg Pressure',
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
    panel: BenchmarkOverlayPanel,
    curve: CurveConfig,
    xValues: Array<number | null>,
    yValues: Array<number | null>,
) {
    panel.curves.push(curve);
    panel.series.push(toXYSeries(xValues, yValues));
}

export function buildBenchmarkComparisonModel(input: {
    family: BenchmarkFamily | null | undefined;
    results: BenchmarkRunResult[];
    xAxisMode: RateChartXAxisMode;
    theme?: BenchmarkOverlayTheme;
}): BenchmarkOverlayModel {
    const family = input.family ?? null;
    const orderedResults = orderResults(input.results);
    const referenceColor = getReferenceColor(input.theme ?? 'dark');
    const panels: Record<RateChartPanelKey, BenchmarkOverlayPanel> = {
        rates: { curves: [], series: [] },
        cumulative: { curves: [], series: [] },
        diagnostics: { curves: [], series: [] },
    };

    if (!family || orderedResults.length === 0) {
        return { orderedResults, panels };
    }

    const derivedByKey = new Map<string, DerivedRunSeries>(
        orderedResults.map((result) => [result.key, buildDerivedRunSeries(result)]),
    );

    orderedResults.forEach((result, index) => {
        const derived = derivedByKey.get(result.key);
        if (!derived) return;
        const color = CASE_COLORS[index % CASE_COLORS.length];
        const xValues = buildXAxisValues(derived, input.xAxisMode);

        if (family.scenarioClass === 'buckley-leverett') {
            appendSeries(
                panels.rates,
                {
                    label: `${result.label} Water Cut`,
                    color,
                    borderWidth: result.variantKey === null ? 2.8 : 2.2,
                    yAxisID: 'y',
                },
                xValues,
                derived.waterCut,
            );
            appendSeries(
                panels.rates,
                {
                    label: `${result.label} Avg Water Sat`,
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
                panels.cumulative,
                {
                    label: `${result.label} Recovery`,
                    color,
                    borderWidth: result.variantKey === null ? 2.8 : 2.2,
                    yAxisID: 'y1',
                },
                xValues,
                derived.recovery,
            );
            appendSeries(
                panels.cumulative,
                {
                    label: `${result.label} Cum Oil`,
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
                    color,
                    borderWidth: result.variantKey === null ? 2.8 : 2.2,
                    yAxisID: 'y',
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
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
                yAxisID: 'y',
            },
            xValues,
            derived.oilRate,
        );
        appendSeries(
            panels.cumulative,
            {
                label: `${result.label} Recovery`,
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
                yAxisID: 'y1',
            },
            xValues,
            derived.recovery,
        );
        appendSeries(
            panels.cumulative,
            {
                label: `${result.label} Cum Oil`,
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
                color,
                borderWidth: result.variantKey === null ? 2.8 : 2.2,
                yAxisID: 'y',
            },
            xValues,
            derived.pressure,
        );
    });

    const baseResult = getBaseResult(orderedResults);
    if (!baseResult) {
        return { orderedResults, panels };
    }

    const baseDerived = derivedByKey.get(baseResult.key);
    if (!baseDerived) {
        return { orderedResults, panels };
    }

    const referenceOverlay = family.scenarioClass === 'buckley-leverett'
        ? buildBuckleyLeverettReference(baseResult, baseDerived, input.xAxisMode)
        : buildDepletionReference(baseResult, baseDerived, input.xAxisMode);

    if (referenceOverlay.rates) {
        appendSeries(
            panels.rates,
            {
                label: referenceOverlay.rates.label,
                color: referenceColor,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            },
            referenceOverlay.xValues,
            referenceOverlay.rates.values,
        );
    }

    if (referenceOverlay.cumulative) {
        appendSeries(
            panels.cumulative,
            {
                label: referenceOverlay.cumulative.recoveryLabel,
                color: referenceColor,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y1',
            },
            referenceOverlay.xValues,
            referenceOverlay.cumulative.recoveryValues,
        );
        appendSeries(
            panels.cumulative,
            {
                label: referenceOverlay.cumulative.cumulativeLabel,
                color: referenceColor,
                borderWidth: 1.4,
                borderDash: [3, 5],
                yAxisID: 'y',
                defaultVisible: false,
            },
            referenceOverlay.xValues,
            referenceOverlay.cumulative.cumulativeValues,
        );
    }

    if (referenceOverlay.diagnostics) {
        appendSeries(
            panels.diagnostics,
            {
                label: referenceOverlay.diagnostics.label,
                color: referenceColor,
                borderWidth: 2,
                borderDash: [7, 4],
                yAxisID: 'y',
            },
            referenceOverlay.xValues,
            referenceOverlay.diagnostics.values,
        );
    }

    return { orderedResults, panels };
}