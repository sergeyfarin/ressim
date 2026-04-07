/**
 * buildRateChartData.ts — pure curve-assembly for the live rate chart.
 *
 * Takes raw simulation output (rateHistory, analytical curves, sweep series)
 * and builds curveRegistry + panelFallbacks + sweepPanels ready for RateChart.svelte
 * to hand to resolveChartPanelDefinition + ChartSubPanel.
 *
 * No Svelte reactivity here — pure data transformation.
 */

import type { CurveConfig } from './chartTypes';
import type { ChartPanelEntry, ChartPanelFallback } from './chartPanelSelection';
import type { RateChartPanelId, RateChartXAxisMode } from './rateChartLayoutConfig';
import type { RateHistoryPoint, AnalyticalProductionPoint } from '../simulator-types';
import { ANALYTICAL_DASH } from './curveStylePolicy';
import { PANEL_DEFS } from './panelDefs';
import {
    computeCombinedSweep,
    computeSweepRecoveryFactor,
    getSweepComponentVisibility,
    type SweepAnalyticalMethod,
    type SweepGeometry,
    type SweepRFResult,
} from '../analytical/sweepEfficiency';
import type { RockProps, FluidProps } from '../analytical/fractionalFlow';
import {
    SCALE_CUMULATIVE_VOLUMES,
    SCALE_CUMULATIVE,
    SCALE_PRESSURE,
    SCALE_GOR,
    SCALE_FRACTION,
    SCALE_SWEEP,
} from './scalePresetRegistry';

// ─── Public types ─────────────────────────────────────────────────────────────

export type XYPoint = { x: number; y: number | null };

export type MismatchSummary = {
    pointsCompared: number;
    mae: number;
    rmse: number;
    mape: number;
};

export type LiveSweepPanels = {
    rfCurves: CurveConfig[];
    rfSeries: XYPoint[][];
    arealCurves: CurveConfig[];
    arealSeries: XYPoint[][];
    verticalCurves: CurveConfig[];
    verticalSeries: XYPoint[][];
    volCurves: CurveConfig[];
    volSeries: XYPoint[][];
    mobileOilCurves: CurveConfig[];
    mobileOilSeries: XYPoint[][];
    showAreal: boolean;
    showVertical: boolean;
};

export type LiveChartData = {
    curveRegistry: Array<ChartPanelEntry<CurveConfig, XYPoint[]>>;
    panelFallbacks: Record<RateChartPanelId, ChartPanelFallback>;
    sweepPanels: LiveSweepPanels | null;
    /** Raw PVI series — used by xAxisRange + pviAvailable check */
    pviValues: number[];
    /** X-values corresponding to xAxisMode */
    xValues: Array<number | null>;
    pviAvailable: boolean;
    pvpAvailable: boolean;
    mismatchSummary: MismatchSummary;
};

export type LiveChartInput = {
    rateHistory: RateHistoryPoint[];
    analyticalProductionData: AnalyticalProductionPoint[];
    avgReservoirPressureSeries: Array<number | null>;
    avgWaterSaturationSeries: Array<number | null>;
    ooipM3: number;
    poreVolumeM3: number;
    theme: 'dark' | 'light';
    xAxisMode: RateChartXAxisMode;
    normalizeRates: boolean;
    analyticalMeta?: { tau?: number; q0?: number } | null;
    showSweepPanel: boolean;
    sweepGeometry: SweepGeometry;
    sweepAnalyticalMethod: SweepAnalyticalMethod;
    sweepEfficiencySimSeries: Array<{
        time: number;
        eA: number | null;
        eV: number | null;
        eVol: number;
        mobileOilRecovered: number | null;
    }> | null;
    sweepRFAnalytical: SweepRFResult | null;
    rockProps: RockProps | undefined;
    fluidProps: FluidProps | undefined;
    layerPermeabilities: number[];
    layerThickness: number;
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

function toXYSeries(
    xValues: Array<number | null>,
    yValues: Array<number | null | undefined>,
): XYPoint[] {
    const points: XYPoint[] = [];
    for (let i = 0; i < yValues.length; i++) {
        const rawX = xValues[i];
        const rawY = yValues[i];
        if (!Number.isFinite(rawX)) continue;
        points.push({ x: Number(rawX), y: Number.isFinite(rawY) ? Number(rawY) : null });
    }
    return points;
}

/** Map a PVI value onto the currently selected x-axis by interpolating through
 *  the cumulative PVI series. Returns null if no mapping is possible. */
function mapPviToXAxis(
    pvi: number,
    pviSeries: number[],
    xValues: Array<number | null>,
    xAxisMode: RateChartXAxisMode,
): number | null {
    if (!Number.isFinite(pvi)) return null;
    if (xAxisMode === 'pvi') return pvi;
    const zeroX = xAxisMode === 'logTime' ? null : 0;
    if (pvi <= 1e-12) return zeroX;

    let prev = -1;
    for (let i = 0; i < pviSeries.length; i++) {
        const d = pviSeries[i];
        const r = xValues[i];
        if (!Number.isFinite(d) || !Number.isFinite(r)) continue;
        if (Math.abs(Number(d) - pvi) <= 1e-9) return Number(r);
        if (Number(d) > pvi) {
            if (prev < 0) return Number(r);
            const d0 = pviSeries[prev];
            const r0 = Number(xValues[prev]);
            const d1 = Number(d);
            const r1 = Number(r);
            if (Math.abs(d1 - d0) <= 1e-12) return r1;
            return r0 + ((pvi - d0) / (d1 - d0)) * (r1 - r0);
        }
        prev = i;
    }
    return prev >= 0 && Number.isFinite(xValues[prev]) ? Number(xValues[prev]) : null;
}

function dedupXY(
    points: Array<{ x: number; y: number | null }>,
): Array<{ x: number; y: number | null }> {
    const out: typeof points = [];
    for (const pt of points) {
        const prev = out.at(-1);
        if (prev && Math.abs(prev.x - pt.x) <= 1e-9) {
            if (out.length === 1 && Math.abs(prev.x) <= 1e-9) continue;
            prev.y = pt.y;
            continue;
        }
        out.push({ ...pt });
    }
    return out;
}

// ─── Scale configs ────────────────────────────────────────────────────────────

const recoveryScales = {
    y: {
        type: 'linear', display: true, position: 'left', min: 0, max: 1,
        alignToPixels: true, title: { display: true, text: 'Recovery Factor' },
        ticks: { count: 6 }, _fraction: true,
    },
};

const breakthroughScales = {
    y1: {
        type: 'linear', display: true, position: 'right', min: 0, max: 1,
        alignToPixels: true, title: { display: true, text: 'Water Cut / Saturation' },
        grid: { drawOnChartArea: false }, ticks: { count: 6 }, _fraction: true,
    },
};

const diagnosticsScales = {
    y: {
        type: 'linear', display: true, position: 'left', alignToPixels: true,
        title: { display: true, text: 'Pressure (bar)' }, ticks: { count: 6 }, _auto: true,
    },
    y1: {
        type: 'linear', display: true, position: 'right', min: 0, alignToPixels: true,
        title: { display: true, text: 'Fraction' }, grid: { drawOnChartArea: false },
        ticks: { count: 6 },
        _dynamicTitle: (labels: string[]) => {
            const parts: string[] = [];
            if (labels.some((l) => l.includes('VRR'))) parts.push('VRR');
            if (labels.some((l) => l.includes('WOR'))) parts.push('WOR');
            if (labels.some((l) => l.includes('Sat'))) parts.push('Saturation');
            if (labels.some((l) => l.includes('Cut'))) parts.push('Water Cut');
            return parts.length > 0 ? parts.join(' / ') : 'Fraction';
        },
    },
    y2: {
        type: 'linear', display: true, position: 'right', min: 0, alignToPixels: true,
        title: { display: true, text: 'MB Error (m³)' },
        grid: { drawOnChartArea: false }, ticks: { count: 6 },
    },
};

const sweepRFScaleConfig = {
    y: {
        type: 'linear', display: true, position: 'left', min: 0, max: 1,
        alignToPixels: true, title: { display: true, text: 'Recovery Factor' },
        ticks: {
            count: 6,
            _tickFormatter: (v: string | number) =>
                typeof v === 'number' ? (v * 100).toFixed(0) + '%' : v,
        },
    },
};

// ─── Main builder ─────────────────────────────────────────────────────────────

export function buildRateChartData(input: LiveChartInput): LiveChartData {
    const {
        rateHistory, analyticalProductionData,
        avgReservoirPressureSeries, avgWaterSaturationSeries,
        ooipM3, poreVolumeM3, theme, xAxisMode, normalizeRates, analyticalMeta,
        showSweepPanel, sweepGeometry, sweepAnalyticalMethod,
        sweepEfficiencySimSeries, sweepRFAnalytical,
        rockProps, fluidProps, layerPermeabilities, layerThickness,
    } = input;

    const neutralColor = theme === 'dark' ? '#f8fafc' : '#0f172a';

    // ── Rate series ────────────────────────────────────────────────────────────
    const oilProd = rateHistory.map((p) => Math.abs(p.total_production_oil ?? 0));
    const liquidProd = rateHistory.map((p) => Math.abs(p.total_production_liquid ?? 0));
    const injection = rateHistory.map((p) => p.total_injection ?? 0);
    const waterProd = liquidProd.map((qL, i) => Math.max(0, qL - oilProd[i]));
    const timeValues = rateHistory.map((p) => Number(p.time));

    const analyticalOilProd = rateHistory.map((_, i) => {
        const v = analyticalProductionData[i]?.oilRate;
        return Number.isFinite(v) ? v as number : null;
    });
    const analyticalWaterRate = rateHistory.map((_, i) => {
        const v = analyticalProductionData[i]?.waterRate;
        return Number.isFinite(v) ? v as number : null;
    });

    const ratesScaleFactor = normalizeRates && analyticalMeta?.q0 && analyticalMeta.q0 > 0
        ? 1.0 / analyticalMeta.q0 : 1.0;

    const normOilProd = oilProd.map((v) => v * ratesScaleFactor);
    const normLiquidProd = liquidProd.map((v) => v * ratesScaleFactor);
    const normInjection = injection.map((v) => v * ratesScaleFactor);
    const normWaterProd = waterProd.map((v) => v * ratesScaleFactor);
    const normAnalyticalOilProd = analyticalOilProd.map((v) => v == null ? null : v * ratesScaleFactor);
    const normAnalyticalWaterRate = analyticalWaterRate.map((v) => v == null ? null : v * ratesScaleFactor);

    // ── Cumulatives ────────────────────────────────────────────────────────────
    let cumOil = 0, cumInj = 0, cumLiq = 0, cumWater = 0;
    const cumOilArr: number[] = [];
    const cumInjArr: number[] = [];
    const cumLiqArr: number[] = [];
    const cumWaterArr: number[] = [];
    const pviArr: number[] = [];
    const pvpArr: number[] = [];
    for (let i = 0; i < rateHistory.length; i++) {
        const dt = i > 0
            ? rateHistory[i].time - rateHistory[i - 1].time
            : rateHistory[i].time;
        cumOil += oilProd[i] * dt;
        cumInj += Math.max(0, injection[i]) * dt;
        cumLiq += Math.max(0, liquidProd[i]) * dt;
        cumWater += Math.max(0, waterProd[i]) * dt;
        cumOilArr.push(cumOil);
        cumInjArr.push(cumInj);
        cumLiqArr.push(cumLiq);
        cumWaterArr.push(cumWater);
        pviArr.push(poreVolumeM3 > 1e-12 ? cumInj / poreVolumeM3 : 0);
        pvpArr.push(poreVolumeM3 > 1e-12 ? cumLiq / poreVolumeM3 : 0);
    }
    const pviAvailable = (pviArr.at(-1) ?? 0) > 1e-12;
    const pvpAvailable = (pvpArr.at(-1) ?? 0) > 1e-12;

    // ── Analytical cumulatives ─────────────────────────────────────────────────
    const analyticalCumOil = rateHistory.map((_, idx) => {
        const v = analyticalProductionData[idx]?.cumulativeOil;
        if (Number.isFinite(v)) return v as number;
        let cum = 0;
        for (let i = 0; i <= idx; i++) {
            const oi = analyticalOilProd[i];
            if (!Number.isFinite(oi)) return null;
            const dt = i > 0
                ? Math.max(0, rateHistory[i].time - rateHistory[i - 1].time)
                : Math.max(0, rateHistory[i].time);
            cum += (oi as number) * dt;
        }
        return cum;
    });

    const recoveryFactor = cumOilArr.map((c) =>
        ooipM3 > 1e-12 ? Math.max(0, Math.min(1, c / ooipM3)) : null,
    );
    const analyticalRecoveryFactor = analyticalCumOil.map((c) =>
        c == null ? null : (ooipM3 > 1e-12 ? Math.max(0, Math.min(1, c / ooipM3)) : null),
    );

    // ── Diagnostic series ─────────────────────────────────────────────────────
    const avgPressure = rateHistory.map((p, i) => {
        const v = avgReservoirPressureSeries?.[i]
            ?? p.avg_reservoir_pressure
            ?? (p as any).average_reservoir_pressure
            ?? (p as any).avg_pressure;
        return Number.isFinite(v) ? v as number : null;
    });
    const analyticalAvgPressure = rateHistory.map((_, i) => {
        const v = (analyticalProductionData[i] as any)?.avgPressure;
        return Number.isFinite(v) ? v as number : null;
    });
    const avgWaterSat = rateHistory.map((p, i) => {
        const v = avgWaterSaturationSeries?.[i] ?? p.avg_water_saturation;
        return Number.isFinite(v) ? v as number : null;
    });

    let cumInjR = 0, cumProdR = 0;
    const vrrData = rateHistory.map((p, i) => {
        const dt = i > 0
            ? Math.max(0, rateHistory[i].time - rateHistory[i - 1].time)
            : Math.max(0, rateHistory[i].time);
        const injR = Number((p as any).total_injection_reservoir ?? p.total_injection);
        const prodR = Number((p as any).total_production_liquid_reservoir ?? p.total_production_liquid);
        if (dt > 0 && Number.isFinite(injR) && Number.isFinite(prodR)) {
            cumInjR += Math.max(0, injR) * dt;
            cumProdR += Math.max(0, prodR) * dt;
        }
        if (cumProdR <= 1e-12) return null;
        const raw = cumInjR / cumProdR;
        return Math.abs(raw - 1.0) < 1e-9 ? 1.0 : raw;
    });

    const worSim = rateHistory.map((_, i) => {
        const oil = oilProd[i];
        const water = waterProd[i];
        return oil > 1e-12 ? Math.max(0, water / oil) : null;
    });
    const worAnalytical = rateHistory.map((_, i) => {
        const oil = Number(analyticalProductionData[i]?.oilRate);
        const water = Number(analyticalProductionData[i]?.waterRate);
        return Number.isFinite(oil) && oil > 1e-12 && Number.isFinite(water)
            ? Math.max(0, water / oil) : null;
    });

    const waterCutSim = rateHistory.map((p, i) => {
        const liquid = Number(p.total_production_liquid);
        if (!Number.isFinite(liquid) || liquid <= 1e-12) return 0;
        return Math.max(0, Math.min(1, waterProd[i] / liquid));
    });
    const waterCutAnalytical = rateHistory.map((_, i) => {
        const oil = Number(analyticalProductionData[i]?.oilRate);
        const water = Number(analyticalProductionData[i]?.waterRate);
        const total = oil + water;
        return Number.isFinite(total) && total > 1e-12
            ? Math.max(0, Math.min(1, water / total)) : null;
    });

    const mbError = rateHistory.map((p) => {
        const v = Number((p as any).material_balance_error_m3);
        return Number.isFinite(v) ? v : null;
    });

    // ── Mismatch summary ──────────────────────────────────────────────────────
    const absErrors: number[] = [];
    const pctErrors: number[] = [];
    for (let i = 0; i < rateHistory.length; i++) {
        const simVal = normOilProd[i];
        const analytical = normAnalyticalOilProd[i];
        if (!Number.isFinite(simVal) || analytical == null) continue;
        const absErr = Math.abs(simVal - analytical);
        absErrors.push(absErr);
        pctErrors.push(absErr / Math.max(Math.abs(analytical), 1e-9) * 100);
    }
    const mismatchSummary: MismatchSummary = absErrors.length > 0 ? {
        pointsCompared: absErrors.length,
        mae: absErrors.reduce((a, v) => a + v, 0) / absErrors.length,
        rmse: Math.sqrt(absErrors.reduce((a, v) => a + v * v, 0) / absErrors.length),
        mape: pctErrors.length > 0 ? pctErrors.reduce((a, v) => a + v, 0) / pctErrors.length : 0,
    } : { pointsCompared: 0, mae: 0, rmse: 0, mape: 0 };

    // ── X-axis values ─────────────────────────────────────────────────────────
    let xValues: Array<number | null>;
    if (xAxisMode === 'pvi') xValues = pviArr;
    else if (xAxisMode === 'pvp') xValues = pvpArr;
    else if (xAxisMode === 'cumLiquid') xValues = cumLiqArr;
    else if (xAxisMode === 'cumInjection') xValues = cumInjArr;
    else if (xAxisMode === 'logTime')
        xValues = timeValues.map((t) => t > 0 ? Math.log10(t) : null);
    else if (xAxisMode === 'tD' && analyticalMeta?.tau && analyticalMeta.tau > 0)
        xValues = timeValues.map((t) => t / (analyticalMeta!.tau as number));
    else xValues = timeValues;

    // ── Curve configs ─────────────────────────────────────────────────────────
    const ratesCurves: CurveConfig[] = [
        { label: 'Oil Rate', curveKey: 'oil-rate-sim', toggleLabel: 'Oil Rate', color: '#16a34a', borderWidth: 2.5, yAxisID: 'y' },
        { label: 'Oil Rate (Reference Solution)', curveKey: 'oil-rate-reference', toggleLabel: 'Reference Solution Oil Rate', color: neutralColor, borderWidth: 2, borderDash: ANALYTICAL_DASH, yAxisID: 'y' },
        { label: 'Water Rate', curveKey: 'water-rate-sim', toggleLabel: 'Water Rate', color: '#1e3a8a', borderWidth: 2.5, yAxisID: 'y' },
        { label: 'Water Rate (Reference Solution)', curveKey: 'water-rate-reference', toggleLabel: 'Reference Solution Water Rate', color: neutralColor, borderWidth: 2, borderDash: ANALYTICAL_DASH, yAxisID: 'y' },
        { label: 'Injection Rate', curveKey: 'injection-rate', toggleLabel: 'Injection Rate', color: '#06b6d4', borderWidth: 2.5, yAxisID: 'y' },
        { label: 'Liquid Rate', curveKey: 'liquid-rate', toggleLabel: 'Liquid Rate', color: '#2563eb', borderWidth: 2, yAxisID: 'y', defaultVisible: false },
    ];
    const ratesSeries = [
        toXYSeries(xValues, normOilProd),
        toXYSeries(xValues, normAnalyticalOilProd),
        toXYSeries(xValues, normWaterProd),
        toXYSeries(xValues, normAnalyticalWaterRate),
        toXYSeries(xValues, normInjection),
        toXYSeries(xValues, normLiquidProd),
    ];

    const ratesScales = {
        y: {
            type: 'linear', display: true, position: 'left', min: 0, alignToPixels: true,
            title: { display: true, text: normalizeRates ? 'Normalized Rate (q/q₀)' : 'Rate (m³/day)' },
            ticks: { count: 6 },
        },
    };

    const cumulativeCurves: CurveConfig[] = [
        { label: 'Cum Oil', curveKey: 'cum-oil-sim', toggleLabel: 'Cum Oil', color: '#0f5132', borderWidth: 2.5, yAxisID: 'y' },
        { label: 'Cum Oil (Reference Solution)', curveKey: 'cum-oil-reference', toggleLabel: 'Reference Solution Cum Oil', color: neutralColor, borderWidth: 2, borderDash: ANALYTICAL_DASH, yAxisID: 'y' },
        { label: 'Cum Injection', curveKey: 'cum-injection', toggleLabel: 'Cum Injection', color: '#06b6d4', borderWidth: 2, borderDash: ANALYTICAL_DASH, yAxisID: 'y' },
        { label: 'Cum Water', curveKey: 'cum-water', toggleLabel: 'Cum Water', color: '#1e3a8a', borderWidth: 2, yAxisID: 'y' },
        { label: 'Recovery Factor', curveKey: 'recovery-factor', toggleLabel: 'Recovery Factor', color: '#22c55e', borderWidth: 2, yAxisID: 'y1', defaultVisible: false },
        { label: 'Recovery Factor (Primary)', curveKey: 'recovery-factor-primary', toggleLabel: 'Recovery Factor', color: '#22c55e', borderWidth: 2.2, yAxisID: 'y' },
        { label: 'Recovery Factor (Reference)', curveKey: 'recovery-factor-reference', toggleLabel: 'Reference Solution RF', color: neutralColor, borderWidth: 2, borderDash: ANALYTICAL_DASH, yAxisID: 'y' },
    ];
    const cumulativeSeries = [
        toXYSeries(xValues, cumOilArr),
        toXYSeries(xValues, analyticalCumOil as Array<number | null>),
        toXYSeries(xValues, cumInjArr),
        toXYSeries(xValues, cumWaterArr),
        toXYSeries(xValues, recoveryFactor as Array<number | null>),
        toXYSeries(xValues, recoveryFactor as Array<number | null>),
        toXYSeries(xValues, analyticalRecoveryFactor as Array<number | null>),
    ];

    const diagnosticsCurves: CurveConfig[] = [
        { label: 'Avg Pressure', curveKey: 'avg-pressure-sim', toggleLabel: 'Avg Pressure', color: '#dc2626', borderWidth: 2, yAxisID: 'y' },
        { label: 'Avg Pressure (Reference Solution)', curveKey: 'avg-pressure-reference', toggleLabel: 'Reference Solution Avg Pressure', color: neutralColor, borderWidth: 2, borderDash: ANALYTICAL_DASH, yAxisID: 'y' },
        { label: 'VRR', curveKey: 'vrr', toggleLabel: 'VRR', color: '#7c3aed', borderWidth: 2.5, yAxisID: 'y1', defaultVisible: false },
        { label: 'WOR (Sim)', curveKey: 'wor-sim', toggleLabel: 'WOR', color: '#d97706', borderWidth: 2.3, yAxisID: 'y1', defaultVisible: false },
        { label: 'WOR (Reference Solution)', curveKey: 'wor-reference', toggleLabel: 'Reference Solution WOR', color: neutralColor, borderWidth: 2, borderDash: ANALYTICAL_DASH, yAxisID: 'y1', defaultVisible: false },
        { label: 'Avg Water Sat', curveKey: 'avg-water-sat', toggleLabel: 'Avg Water Sat', color: '#1d4ed8', borderWidth: 2, borderDash: ANALYTICAL_DASH, yAxisID: 'y1', defaultVisible: false },
        { label: 'Water Cut (Sim)', curveKey: 'water-cut-sim', toggleLabel: 'Water Cut', color: '#2563eb', borderWidth: 2.0, yAxisID: 'y1', defaultVisible: false },
        { label: 'Water Cut (Reference Solution)', curveKey: 'water-cut-reference', toggleLabel: 'Reference Solution Water Cut', color: neutralColor, borderWidth: 2, borderDash: ANALYTICAL_DASH, yAxisID: 'y1', defaultVisible: false },
        { label: 'MB Error', curveKey: 'mb-error', toggleLabel: 'MB Error', color: '#ef4444', borderWidth: 2.0, borderDash: ANALYTICAL_DASH, yAxisID: 'y2', defaultVisible: false },
    ];
    const diagnosticsSeries = [
        toXYSeries(xValues, avgPressure),
        toXYSeries(xValues, analyticalAvgPressure),
        toXYSeries(xValues, vrrData as Array<number | null>),
        toXYSeries(xValues, worSim),
        toXYSeries(xValues, worAnalytical),
        toXYSeries(xValues, avgWaterSat),
        toXYSeries(xValues, waterCutSim),
        toXYSeries(xValues, waterCutAnalytical),
        toXYSeries(xValues, mbError),
    ];

    // ── curveRegistry ─────────────────────────────────────────────────────────
    const curveRegistry: Array<ChartPanelEntry<CurveConfig, XYPoint[]>> = [
        ...ratesCurves.map((curve, i) => ({ curve, series: ratesSeries[i] ?? [] })),
        ...cumulativeCurves.map((curve, i) => ({ curve, series: cumulativeSeries[i] ?? [] })),
        ...diagnosticsCurves.map((curve, i) => ({ curve, series: diagnosticsSeries[i] ?? [] })),
    ];

    // ── panelFallbacks ────────────────────────────────────────────────────────
    const panelFallbacks: Record<RateChartPanelId, ChartPanelFallback> = {
        ...PANEL_DEFS,
        rates: {
            ...PANEL_DEFS.rates,
            curveKeys: ratesCurves.map((c) => c.curveKey ?? c.label),
            curveLabels: ratesCurves.map((c) => c.label),
            scalePreset: 'rates',
        } as ChartPanelFallback & { scalePreset: any },
        recovery: { ...PANEL_DEFS.recovery, curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'] },
        cumulative: { ...PANEL_DEFS.cumulative, curveKeys: ['cum-oil-sim', 'cum-oil-reference', 'cum-injection'] },
        diagnostics: {
            ...PANEL_DEFS.diagnostics,
            title: 'Diagnostics',
            curveKeys: diagnosticsCurves.map((c) => c.curveKey ?? c.label),
            curveLabels: diagnosticsCurves.map((c) => c.label),
            scalePreset: 'diagnostics',
        } as ChartPanelFallback & { scalePreset: any },
        oil_rate: { ...PANEL_DEFS.oil_rate, curveKeys: ['oil-rate-sim', 'oil-rate-reference'] },
        injection_rate: { ...PANEL_DEFS.injection_rate, visible: false },
        sweep_rf: { ...PANEL_DEFS.sweep_rf, title: 'Recovery Factor — Sweep Analysis', expanded: true },
    };

    // ── Sweep panels ──────────────────────────────────────────────────────────
    let sweepPanels: LiveSweepPanels | null = null;

    if (showSweepPanel && rockProps && fluidProps) {
        const perms = layerPermeabilities.length > 0 ? layerPermeabilities : [100];
        const analytical = computeCombinedSweep(rockProps, fluidProps, perms, layerThickness, 3.0, 200, sweepGeometry, sweepAnalyticalMethod);
        const rfAnalytical = computeSweepRecoveryFactor(rockProps, fluidProps, perms, layerThickness, 3.0, 200, sweepGeometry, sweepAnalyticalMethod);
        const visibility = getSweepComponentVisibility(sweepGeometry);
        const { showAreal, showVertical } = visibility;

        const pviXY = (ys: number[], pvis: number[]) =>
            pvis.map((pvi, i) => ({ x: mapPviToXAxis(pvi, pviArr, xValues, xAxisMode) ?? 0, y: ys[i] ?? null }));

        const hasSim = sweepEfficiencySimSeries != null && sweepEfficiencySimSeries.length > 0;
        const hasComponentSim = hasSim && sweepGeometry !== 'both';

        const simToXY = (getter: (pt: NonNullable<typeof sweepEfficiencySimSeries>[0]) => number | null) => {
            if (!sweepEfficiencySimSeries) return [];
            return dedupXY(sweepEfficiencySimSeries.map((pt) => {
                const y = getter(pt);
                if (pt.time <= 1e-12) return { x: xAxisMode === 'logTime' ? 0 : 0, y: Number.isFinite(y) ? Number(y) : null };
                const tIdx = timeValues.findIndex((t) => t >= pt.time - 1e-9);
                const x = tIdx >= 0 ? (xValues[tIdx] ?? null) : (xValues.at(-1) ?? null);
                return { x: x ?? 0, y: Number.isFinite(y) ? Number(y) : null };
            }));
        };

        const analyticalPviValues = analytical.arealSweep.curve.map((p) => p.pvi);
        const arealCurves: CurveConfig[] = showAreal ? [
            ...(hasComponentSim ? [{ label: 'E_A (Simulation)', curveKey: 'sweep-areal-sim', toggleLabel: 'E_A (Sim)', color: '#2563eb', borderWidth: 2.4, yAxisID: 'y' } as CurveConfig] : []),
            { label: 'E_A (Analytical)', curveKey: 'sweep-areal-analytical', toggleLabel: 'E_A (Analytical)', color: '#2563eb', borderWidth: 2.0, borderDash: ANALYTICAL_DASH, yAxisID: 'y' } as CurveConfig,
        ] : [];
        const arealSeries: XYPoint[][] = showAreal ? [
            ...(hasComponentSim ? [simToXY((p) => p.eA)] : []),
            pviXY(analytical.arealSweep.curve.map((p) => p.efficiency), analyticalPviValues),
        ] : [];

        const verticalCurves: CurveConfig[] = showVertical ? [
            ...(hasComponentSim ? [{ label: 'E_V (Simulation)', curveKey: 'sweep-vertical-sim', toggleLabel: 'E_V (Sim)', color: '#16a34a', borderWidth: 2.4, yAxisID: 'y' } as CurveConfig] : []),
            { label: 'E_V (Analytical)', curveKey: 'sweep-vertical-analytical', toggleLabel: 'E_V (Analytical)', color: '#16a34a', borderWidth: 2.0, borderDash: ANALYTICAL_DASH, yAxisID: 'y' } as CurveConfig,
        ] : [];
        const verticalSeries: XYPoint[][] = showVertical ? [
            ...(hasComponentSim ? [simToXY((p) => p.eV)] : []),
            pviXY(analytical.verticalSweep.curve.map((p) => p.efficiency), analyticalPviValues),
        ] : [];

        const rfLabel = rfAnalytical?.method === 'stiles'
            ? 'RF (Analytical Total — Stiles Layered BL)'
            : 'RF (Analytical Total — Dykstra-Parsons)';
        const eVolLabel = rfAnalytical?.method === 'stiles'
            ? 'E_vol (Analytical Total — Stiles Layered BL)'
            : 'E_vol (Analytical Total — Dykstra-Parsons)';

        const volCurves: CurveConfig[] = [
            ...(hasSim ? [{ label: 'E_vol (Simulation)', curveKey: 'sweep-vol-sim', toggleLabel: 'E_vol (Sim)', color: '#dc2626', borderWidth: 2.4, yAxisID: 'y' } as CurveConfig] : []),
            { label: eVolLabel, curveKey: 'sweep-vol-analytical', toggleLabel: 'E_vol (Analytical)', color: '#dc2626', borderWidth: 2.0, borderDash: ANALYTICAL_DASH, yAxisID: 'y' } as CurveConfig,
        ];
        const volSeries: XYPoint[][] = [
            ...(hasSim ? [simToXY((p) => p.eVol)] : []),
            pviXY(analytical.combined.map((p) => p.efficiency), analyticalPviValues),
        ];

        const mobileOilCurves: CurveConfig[] = sweepGeometry === 'both' ? [
            ...(hasSim ? [{ label: 'Mobile Oil Recovered (Simulation)', curveKey: 'sweep-vol-mobile-oil-sim', toggleLabel: 'Mobile Oil Recovered (Sim)', color: '#f59e0b', borderWidth: 2.0, yAxisID: 'y' } as CurveConfig] : []),
            { label: eVolLabel, curveKey: 'sweep-vol-analytical-mobile-oil', toggleLabel: 'E_vol (Analytical)', color: '#dc2626', borderWidth: 2.0, borderDash: ANALYTICAL_DASH, yAxisID: 'y' } as CurveConfig,
        ] : [];
        const mobileOilSeries: XYPoint[][] = sweepGeometry === 'both' ? [
            ...(hasSim ? [simToXY((p) => p.mobileOilRecovered)] : []),
            pviXY(analytical.combined.map((p) => p.efficiency), analyticalPviValues),
        ] : [];

        const simRFSeries = (xValues as Array<number | null>).map((x, i) => ({
            x: x ?? 0, y: (recoveryFactor as Array<number | null>)[i] ?? null,
        }));
        const sweepRFRef = sweepRFAnalytical ?? rfAnalytical;
        const rfCurves: CurveConfig[] = [
            { label: 'RF (Simulation)', curveKey: 'sweep-rf-sim', toggleLabel: 'RF (Sim)', color: '#15803d', borderWidth: 2.0, yAxisID: 'y' } as CurveConfig,
            { label: rfLabel, curveKey: 'sweep-rf-sweep', toggleLabel: 'RF Analytical (Sweep)', color: '#15803d', borderWidth: 2.0, borderDash: ANALYTICAL_DASH, yAxisID: 'y' } as CurveConfig,
            ...(sweepGeometry === 'both' ? [] : [{ label: 'RF (1D BL — perfect sweep)', curveKey: 'sweep-rf-bl1d', toggleLabel: 'RF 1D BL (upper bound)', color: '#4ade80', borderWidth: 2.0, borderDash: ANALYTICAL_DASH, yAxisID: 'y', defaultVisible: false } as CurveConfig]),
        ];
        const rfSeries: XYPoint[][] = [
            simRFSeries,
            sweepRFRef.curve.map((p) => ({ x: mapPviToXAxis(p.pvi, pviArr, xValues, xAxisMode) ?? 0, y: p.rfSweep })),
            ...(sweepGeometry === 'both' ? [] : [sweepRFRef.curve.map((p) => ({ x: mapPviToXAxis(p.pvi, pviArr, xValues, xAxisMode) ?? 0, y: p.rfBL1D }))]),
        ];

        sweepPanels = {
            rfCurves, rfSeries,
            arealCurves, arealSeries,
            verticalCurves, verticalSeries,
            volCurves, volSeries,
            mobileOilCurves, mobileOilSeries,
            showAreal, showVertical,
        };
    }

    return {
        curveRegistry,
        panelFallbacks,
        sweepPanels,
        pviValues: pviArr,
        xValues,
        pviAvailable,
        pvpAvailable,
        mismatchSummary,
    };
}

/** Build a getScalePresetConfig function using the live scales. Returned as a
 *  closure so RateChart can call resolveChartPanelDefinition without importing
 *  the scale objects directly. */
export function buildGetScalePresetConfig(normalizeRates: boolean): (preset: string) => Record<string, any> {
    const ratesScales = {
        y: {
            type: 'linear', display: true, position: 'left', min: 0, alignToPixels: true,
            title: { display: true, text: normalizeRates ? 'Normalized Rate (q/q₀)' : 'Rate (m³/day)' },
            ticks: { count: 6 },
        },
    };
    return (scalePreset: string) => {
        if (scalePreset === 'sweep') return SCALE_SWEEP;
        if (scalePreset === 'sweep_rf') return sweepRFScaleConfig;
        if (scalePreset === 'breakthrough') return breakthroughScales;
        if (scalePreset === 'pressure') return SCALE_PRESSURE;
        if (scalePreset === 'gor') return SCALE_GOR;
        if (scalePreset === 'cumulative') return SCALE_CUMULATIVE;
        if (scalePreset === 'cumulative_volumes') return SCALE_CUMULATIVE_VOLUMES;
        if (scalePreset === 'recovery') return recoveryScales;
        if (scalePreset === 'diagnostics') return diagnosticsScales;
        if (scalePreset === 'fraction') return SCALE_FRACTION;
        return ratesScales;
    };
}
