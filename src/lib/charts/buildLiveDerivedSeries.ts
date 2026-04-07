/**
 * buildLiveDerivedSeries.ts — compute all live-chart series from raw simulation output.
 *
 * Pure function: no Svelte reactivity, no Chart.js, no curve configs.
 * Takes rateHistory + analytical production data and returns LiveDerivedSeries —
 * the data context consumed by every UniversalCurveDef.getData callback.
 */

import type { RateHistoryPoint, AnalyticalProductionPoint } from '../simulator-types';
import type { LiveDerivedSeries, MismatchSummary } from './universalChartTypes';

export function buildLiveDerivedSeries(
    rateHistory: RateHistoryPoint[],
    analyticalProductionData: AnalyticalProductionPoint[],
    avgReservoirPressureSeries: Array<number | null>,
    avgWaterSaturationSeries: Array<number | null>,
    ooipM3: number,
    poreVolumeM3: number,
): LiveDerivedSeries {
    const n = rateHistory.length;

    // ── Simulation rates ───────────────────────────────────────────────────────
    const oilRate   = rateHistory.map((p) => Math.abs(p.total_production_oil ?? 0));
    const liquidRate = rateHistory.map((p) => Math.abs(p.total_production_liquid ?? 0));
    const injectionRate = rateHistory.map((p) => p.total_injection ?? 0);
    const waterRate = liquidRate.map((qL, i) => Math.max(0, qL - oilRate[i]));
    const time = rateHistory.map((p) => Number(p.time));

    // ── Analytical rates ───────────────────────────────────────────────────────
    const analyticalOilRate = rateHistory.map((_, i) => {
        const v = analyticalProductionData[i]?.oilRate;
        return Number.isFinite(v) ? v as number : null;
    });
    const analyticalWaterRate = rateHistory.map((_, i) => {
        const v = analyticalProductionData[i]?.waterRate;
        return Number.isFinite(v) ? v as number : null;
    });

    // ── Simulation cumulatives ─────────────────────────────────────────────────
    let cumOilAcc = 0, cumInjAcc = 0, cumLiqAcc = 0, cumWaterAcc = 0;
    const cumOil: number[] = [];
    const cumInjection: number[] = [];
    const cumLiquid: number[] = [];
    const cumWater: number[] = [];
    const pvi: number[] = [];
    const pvp: number[] = [];

    for (let i = 0; i < n; i++) {
        const dt = i > 0 ? rateHistory[i].time - rateHistory[i - 1].time : rateHistory[i].time;
        cumOilAcc   += oilRate[i] * dt;
        cumInjAcc   += Math.max(0, injectionRate[i]) * dt;
        cumLiqAcc   += Math.max(0, liquidRate[i]) * dt;
        cumWaterAcc += Math.max(0, waterRate[i]) * dt;
        cumOil.push(cumOilAcc);
        cumInjection.push(cumInjAcc);
        cumLiquid.push(cumLiqAcc);
        cumWater.push(cumWaterAcc);
        pvi.push(poreVolumeM3 > 1e-12 ? cumInjAcc / poreVolumeM3 : 0);
        pvp.push(poreVolumeM3 > 1e-12 ? cumLiqAcc / poreVolumeM3 : 0);
    }

    // ── Recovery factor ────────────────────────────────────────────────────────
    const recoveryFactor = cumOil.map((c) =>
        ooipM3 > 1e-12 ? Math.max(0, Math.min(1, c / ooipM3)) : null,
    );

    // ── Analytical cumulatives ─────────────────────────────────────────────────
    const analyticalCumOil: Array<number | null> = rateHistory.map((_, idx) => {
        const v = analyticalProductionData[idx]?.cumulativeOil;
        if (Number.isFinite(v)) return v as number;
        // Fallback: integrate analytical oil rate up to this index
        let cum = 0;
        for (let i = 0; i <= idx; i++) {
            const oi = analyticalOilRate[i];
            if (!Number.isFinite(oi)) return null;
            const dt = i > 0
                ? Math.max(0, rateHistory[i].time - rateHistory[i - 1].time)
                : Math.max(0, rateHistory[i].time);
            cum += (oi as number) * dt;
        }
        return cum;
    });

    const analyticalRecoveryFactor = analyticalCumOil.map((c) =>
        c == null ? null : (ooipM3 > 1e-12 ? Math.max(0, Math.min(1, c / ooipM3)) : null),
    );

    // ── Diagnostics — simulation ───────────────────────────────────────────────
    const avgPressure = rateHistory.map((p, i) => {
        const v = avgReservoirPressureSeries?.[i]
            ?? p.avg_reservoir_pressure
            ?? (p as any).average_reservoir_pressure
            ?? (p as any).avg_pressure;
        return Number.isFinite(v) ? v as number : null;
    });

    const avgWaterSat = rateHistory.map((p, i) => {
        const v = avgWaterSaturationSeries?.[i] ?? p.avg_water_saturation;
        return Number.isFinite(v) ? v as number : null;
    });

    let cumInjR = 0, cumProdR = 0;
    const vrr = rateHistory.map((p, i) => {
        const dt = i > 0
            ? Math.max(0, rateHistory[i].time - rateHistory[i - 1].time)
            : Math.max(0, rateHistory[i].time);
        const injR  = Number((p as any).total_injection_reservoir ?? p.total_injection);
        const prodR = Number((p as any).total_production_liquid_reservoir ?? p.total_production_liquid);
        if (dt > 0 && Number.isFinite(injR) && Number.isFinite(prodR)) {
            cumInjR  += Math.max(0, injR)  * dt;
            cumProdR += Math.max(0, prodR) * dt;
        }
        if (cumProdR <= 1e-12) return null;
        const raw = cumInjR / cumProdR;
        return Math.abs(raw - 1.0) < 1e-9 ? 1.0 : raw;
    });

    const worSim = rateHistory.map((_, i) => {
        const oil   = oilRate[i];
        const water = waterRate[i];
        return oil > 1e-12 ? Math.max(0, water / oil) : null;
    });

    const waterCutSim = rateHistory.map((p, i) => {
        const liquid = Number(p.total_production_liquid);
        if (!Number.isFinite(liquid) || liquid <= 1e-12) return 0;
        return Math.max(0, Math.min(1, waterRate[i] / liquid));
    });

    const mbError = rateHistory.map((p) => {
        const v = Number((p as any).material_balance_error_m3);
        return Number.isFinite(v) ? v : null;
    });

    // ── Diagnostics — analytical counterparts ─────────────────────────────────
    const analyticalAvgPressure = rateHistory.map((_, i) => {
        const v = (analyticalProductionData[i] as any)?.avgPressure;
        return Number.isFinite(v) ? v as number : null;
    });

    const worAnalytical = rateHistory.map((_, i) => {
        const oil   = Number(analyticalProductionData[i]?.oilRate);
        const water = Number(analyticalProductionData[i]?.waterRate);
        return Number.isFinite(oil) && oil > 1e-12 && Number.isFinite(water)
            ? Math.max(0, water / oil) : null;
    });

    const waterCutAnalytical = rateHistory.map((_, i) => {
        const oil   = Number(analyticalProductionData[i]?.oilRate);
        const water = Number(analyticalProductionData[i]?.waterRate);
        const total = oil + water;
        return Number.isFinite(total) && total > 1e-12
            ? Math.max(0, Math.min(1, water / total)) : null;
    });

    return {
        time,
        oilRate,
        waterRate,
        liquidRate,
        injectionRate,
        cumOil,
        cumWater,
        cumLiquid,
        cumInjection,
        pvi,
        pvp,
        recoveryFactor,
        analyticalCumOil,
        analyticalRecoveryFactor,
        avgPressure,
        avgWaterSat,
        vrr,
        worSim,
        waterCutSim,
        mbError,
        analyticalAvgPressure,
        worAnalytical,
        waterCutAnalytical,
    };
}

// ─── Mismatch summary ─────────────────────────────────────────────────────────

/** Compute MAE / RMSE / MAPE between sim and analytical oil rate (normalized). */
export function buildMismatchSummary(
    sim: LiveDerivedSeries,
    analyticalProductionData: AnalyticalProductionPoint[],
    scaleFactor: number,
): MismatchSummary {
    const absErrors: number[] = [];
    const pctErrors: number[] = [];
    for (let i = 0; i < sim.oilRate.length; i++) {
        const simVal        = sim.oilRate[i] * scaleFactor;
        const analyticalVal = analyticalProductionData[i]?.oilRate;
        if (!Number.isFinite(simVal) || analyticalVal == null) continue;
        const analytical = analyticalVal * scaleFactor;
        const absErr = Math.abs(simVal - analytical);
        absErrors.push(absErr);
        pctErrors.push(absErr / Math.max(Math.abs(analytical), 1e-9) * 100);
    }
    if (absErrors.length === 0) return { pointsCompared: 0, mae: 0, rmse: 0, mape: 0 };
    return {
        pointsCompared: absErrors.length,
        mae:  absErrors.reduce((a, v) => a + v, 0) / absErrors.length,
        rmse: Math.sqrt(absErrors.reduce((a, v) => a + v * v, 0) / absErrors.length),
        mape: pctErrors.reduce((a, v) => a + v, 0) / pctErrors.length,
    };
}

// ─── X-axis values ────────────────────────────────────────────────────────────

/** Compute the active x-axis value array from LiveDerivedSeries. */
export function buildXValues(
    sim: LiveDerivedSeries,
    xAxisMode: import('./rateChartLayoutConfig').RateChartXAxisMode,
    analyticalMeta?: { tau?: number; q0?: number } | null,
): Array<number | null> {
    if (xAxisMode === 'pvi')          return sim.pvi;
    if (xAxisMode === 'pvp')          return sim.pvp;
    if (xAxisMode === 'cumLiquid')    return sim.cumLiquid;
    if (xAxisMode === 'cumInjection') return sim.cumInjection;
    if (xAxisMode === 'logTime')
        return sim.time.map((t) => t > 0 ? Math.log10(t) : null);
    if (xAxisMode === 'tD' && analyticalMeta?.tau && analyticalMeta.tau > 0)
        return sim.time.map((t) => t / (analyticalMeta!.tau as number));
    return sim.time;
}
