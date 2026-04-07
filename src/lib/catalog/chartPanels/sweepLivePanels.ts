/**
 * sweepLivePanels.ts — live chart panel definitions for sweep efficiency scenarios.
 *
 * Used by: sweep_areal, sweep_vertical, sweep_combined.
 *
 * Sweep panels extend the waterflood base panels with five additional panels:
 *   sweep_rf             Recovery Factor vs PVI (sim + analytical sweep RF)
 *   sweep_areal          Areal sweep efficiency E_A (analytical, + sim if 'areal'/'vertical')
 *   sweep_vertical       Vertical sweep efficiency E_V (analytical, + sim if 'vertical')
 *   sweep_combined       Volumetric E_vol (sim + analytical combined)
 *   sweep_combined_mobile_oil  E_vol vs mobile oil recovered (combined geometry only)
 *
 * getDataXY is used for analytical sweep curves because their x-axis values
 * come from PVI interpolation, not direct alignment with rateHistory.
 */

import type { UniversalPanelDef, LiveCurveContext } from '../../charts/universalChartTypes';
import { waterfloodLivePanels } from './waterfloodLivePanels';

// ─── PVI → active x-axis interpolation ───────────────────────────────────────

function mapPviToX(
    pvi: number,
    pviArr: number[],
    xValues: Array<number | null>,
    xAxisMode: import('../../charts/rateChartLayoutConfig').RateChartXAxisMode,
): number | null {
    if (!Number.isFinite(pvi)) return null;
    if (xAxisMode === 'pvi') return pvi;
    const zeroX = xAxisMode === 'logTime' ? null : 0;
    if (pvi <= 1e-12) return zeroX;

    let prev = -1;
    for (let i = 0; i < pviArr.length; i++) {
        const d = pviArr[i];
        const r = xValues[i];
        if (!Number.isFinite(d) || !Number.isFinite(r)) continue;
        if (Math.abs(Number(d) - pvi) <= 1e-9) return Number(r);
        if (Number(d) > pvi) {
            if (prev < 0) return Number(r);
            const d0 = pviArr[prev], r0 = Number(xValues[prev]);
            const d1 = Number(d),    r1 = Number(r);
            if (Math.abs(d1 - d0) <= 1e-12) return r1;
            return r0 + ((pvi - d0) / (d1 - d0)) * (r1 - r0);
        }
        prev = i;
    }
    return prev >= 0 && Number.isFinite(xValues[prev]) ? Number(xValues[prev]) : null;
}

/** Deduplicate consecutive XY points at the same x coordinate (keep last y). */
function dedupXY(
    pts: Array<{ x: number; y: number | null }>,
): Array<{ x: number; y: number | null }> {
    const out: typeof pts = [];
    for (const pt of pts) {
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

/** Map sweep sim series (time-based) onto the active x-axis. */
function simSweepToXY(
    ctx: LiveCurveContext,
    getter: (pt: NonNullable<LiveCurveContext['sweepSimSeries']>[0]) => number | null,
): Array<{ x: number; y: number | null }> {
    if (!ctx.sweepSimSeries || ctx.sweepSimSeries.length === 0) return [];
    return dedupXY(ctx.sweepSimSeries.map((pt) => {
        const y = getter(pt);
        if (pt.time <= 1e-12) {
            return { x: ctx.xAxisMode === 'logTime' ? 0 : 0, y: Number.isFinite(y) ? Number(y) : null };
        }
        const tIdx = ctx.rateHistory.findIndex((p) => p.time >= pt.time - 1e-9);
        const x = tIdx >= 0 ? (ctx.xValues[tIdx] ?? null) : (ctx.xValues.at(-1) ?? null);
        return { x: x ?? 0, y: Number.isFinite(y) ? Number(y) : null };
    }));
}

/** Map analytical sweep curve (PVI-indexed) onto the active x-axis. */
function analyticalSweepToXY(
    ctx: LiveCurveContext,
    curve: Array<{ pvi: number; efficiency: number }>,
): Array<{ x: number; y: number | null }> {
    return curve.map((p) => ({
        x: mapPviToX(p.pvi, ctx.pviArr, ctx.xValues, ctx.xAxisMode) ?? 0,
        y: p.efficiency,
    }));
}

// ─── Sweep panel defs (appended to waterflood base) ──────────────────────────

const sweepOnlyPanels: UniversalPanelDef[] = [
    // Recovery Factor — sweep view (RF sim vs sweep analytical vs 1D BL upper bound)
    {
        panelKey: 'sweep_rf',
        curves: [
            {
                key: 'sweep-rf-sim', label: 'RF (Simulation)', curveType: 'simulation',
                yAxisID: 'y', color: '#15803d',
                getData: (ctx) => ctx.sim.recoveryFactor,
            },
            {
                key: 'sweep-rf-sweep', label: 'RF (Analytical — Sweep)', curveType: 'analytical',
                yAxisID: 'y', color: '#15803d',
                getDataXY: (ctx) => {
                    if (!ctx.sweep) return [];
                    return ctx.sweep.rfResult.curve.map((p) => ({
                        x: mapPviToX(p.pvi, ctx.pviArr, ctx.xValues, ctx.xAxisMode) ?? 0,
                        y: p.rfSweep,
                    }));
                },
            },
            {
                key: 'sweep-rf-bl1d', label: 'RF (1D BL — perfect sweep)', curveType: 'analytical',
                yAxisID: 'y', color: '#4ade80', defaultVisible: false,
                getDataXY: (ctx) => {
                    if (!ctx.sweep) return [];
                    return ctx.sweep.rfResult.curve.map((p) => ({
                        x: mapPviToX(p.pvi, ctx.pviArr, ctx.xValues, ctx.xAxisMode) ?? 0,
                        y: p.rfBL1D,
                    }));
                },
            },
        ],
    },

    // Areal Sweep Efficiency E_A
    {
        panelKey: 'sweep_areal',
        curves: [
            {
                key: 'sweep-areal-sim', label: 'E_A (Simulation)', curveType: 'simulation',
                yAxisID: 'y', color: '#2563eb',
                getDataXY: (ctx) => {
                    if (!ctx.sweep?.showAreal) return [];
                    return simSweepToXY(ctx, (p) => p.eA);
                },
            },
            {
                key: 'sweep-areal-analytical', label: 'E_A (Analytical)', curveType: 'analytical',
                yAxisID: 'y', color: '#2563eb',
                getDataXY: (ctx) => {
                    if (!ctx.sweep?.showAreal) return [];
                    return analyticalSweepToXY(ctx, ctx.sweep.arealSweepCurve);
                },
            },
        ],
    },

    // Vertical Sweep Efficiency E_V
    {
        panelKey: 'sweep_vertical',
        curves: [
            {
                key: 'sweep-vertical-sim', label: 'E_V (Simulation)', curveType: 'simulation',
                yAxisID: 'y', color: '#16a34a',
                getDataXY: (ctx) => {
                    if (!ctx.sweep?.showVertical) return [];
                    return simSweepToXY(ctx, (p) => p.eV);
                },
            },
            {
                key: 'sweep-vertical-analytical', label: 'E_V (Analytical)', curveType: 'analytical',
                yAxisID: 'y', color: '#16a34a',
                getDataXY: (ctx) => {
                    if (!ctx.sweep?.showVertical) return [];
                    return analyticalSweepToXY(ctx, ctx.sweep.verticalSweepCurve);
                },
            },
        ],
    },

    // Volumetric Sweep Efficiency E_vol
    {
        panelKey: 'sweep_combined',
        curves: [
            {
                key: 'sweep-vol-sim', label: 'E_vol (Simulation)', curveType: 'simulation',
                yAxisID: 'y', color: '#dc2626',
                getDataXY: (ctx) => simSweepToXY(ctx, (p) => p.eVol),
            },
            {
                key: 'sweep-vol-analytical', label: 'E_vol (Analytical)', curveType: 'analytical',
                yAxisID: 'y', color: '#dc2626',
                getDataXY: (ctx) => {
                    if (!ctx.sweep) return [];
                    return analyticalSweepToXY(ctx, ctx.sweep.combinedSweepCurve);
                },
            },
        ],
    },

    // Mobile Oil Recovered (combined geometry only)
    {
        panelKey: 'sweep_combined_mobile_oil',
        curves: [
            {
                key: 'sweep-vol-mobile-oil-sim', label: 'Mobile Oil Recovered (Simulation)',
                curveType: 'simulation', yAxisID: 'y', color: '#f59e0b',
                getDataXY: (ctx) => simSweepToXY(ctx, (p) => p.mobileOilRecovered),
            },
            {
                key: 'sweep-vol-analytical-mobile-oil', label: 'E_vol (Analytical)',
                curveType: 'analytical', yAxisID: 'y', color: '#dc2626',
                getDataXY: (ctx) => {
                    if (!ctx.sweep) return [];
                    return analyticalSweepToXY(ctx, ctx.sweep.combinedSweepCurve);
                },
            },
        ],
    },
];

/** Full panel set for sweep scenarios (waterflood base + sweep panels). */
export const sweepLivePanels: UniversalPanelDef[] = [
    ...waterfloodLivePanels,
    ...sweepOnlyPanels,
];
