/**
 * referenceOverlayBuilders.ts — builds analytical reference overlays from a
 * completed simulation result.
 *
 * Each function takes a `BenchmarkRunResult` (with params and rate history) and
 * the pre-computed `DerivedRunSeries`, and returns an `AnalyticalOverlay` holding
 * labeled y-series + x-values ready for panel assembly in buildChartData.
 *
 * All three builders are pure functions: no side effects, no Chart.js, no DOM.
 */

import { calculateGasOilAnalyticalProduction } from '../analytical/fractionalFlow';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import {
    computeBLAnalyticalFromParams,
    computeDepletionOnTimeAxis,
    computeGasOilBLAnalyticalFromParams,
    extractGasOilFluidProps,
    extractGasOilRockProps,
    getOoip,
    getPoreVolume,
    toFiniteNumber,
} from './analyticalParamAdapters';
import type { DerivedRunSeries } from './axisAdapters';
import { buildXAxisValues } from './axisAdapters';
import type { AnalyticalOverlay } from './referenceChartTypes';
import type { RateChartXAxisMode } from './rateChartLayoutConfig';

export type { AnalyticalOverlay };

// ─── Empty overlay helper ─────────────────────────────────────────────────────

function emptyOverlay(): AnalyticalOverlay {
    return { rates: null, cumulative: null, diagnostics: null, xValues: [] };
}

// ─── Buckley-Leverett waterflood overlay ─────────────────────────────────────

export function buildBuckleyLeverettReference(
    baseResult: BenchmarkRunResult,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
): AnalyticalOverlay {
    if (xAxisMode === 'pvi') {
        const analytical = computeBLAnalyticalFromParams(baseResult.params);
        if (!analytical) return emptyOverlay();
        return {
            rates: { label: 'Reference Solution Water Cut', values: analytical.waterCut },
            cumulative: {
                recoveryLabel: 'Reference Solution Recovery',
                recoveryValues: analytical.recovery,
                cumulativeLabel: 'Reference Solution Cum Oil',
                cumulativeValues: analytical.cumulativeOil,
            },
            diagnostics: null,
            xValues: analytical.xValues,
        };
    }

    const analytical = computeBLAnalyticalFromParams(baseResult.params, {
        xValues: buildXAxisValues(derived, xAxisMode),
        timeHistory: derived.time,
        injectionRateSeries: baseResult.rateHistory.map((point) => Math.max(0, toFiniteNumber(point.total_injection, 0))),
        poreVolume: getPoreVolume(baseResult.params),
        recoveryDenominator: getOoip(baseResult.params),
    });
    if (!analytical) return emptyOverlay();

    return {
        rates: { label: 'Reference Solution Water Cut', values: analytical.waterCut },
        cumulative: {
            recoveryLabel: 'Reference Solution Recovery',
            recoveryValues: analytical.recovery,
            cumulativeLabel: 'Reference Solution Cum Oil',
            cumulativeValues: analytical.cumulativeOil,
        },
        diagnostics: null,
        xValues: analytical.xValues,
    };
}

// ─── Depletion (Arps / radial flow) overlay ───────────────────────────────────

export function buildDepletionReference(
    baseResult: BenchmarkRunResult,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
): AnalyticalOverlay {
    let analyticalResult: ReturnType<typeof computeDepletionOnTimeAxis>;
    try {
        analyticalResult = computeDepletionOnTimeAxis(baseResult.params, derived.time);
    } catch {
        return emptyOverlay();
    }

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
            { ...derived, time: analyticalResult.production.map((point) => point.time) },
            xAxisMode,
            tau,
        ),
    };
}

// ─── Gas-oil Buckley-Leverett overlay ────────────────────────────────────────

export function buildGasOilBLReference(
    baseResult: BenchmarkRunResult,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
): AnalyticalOverlay {
    if (xAxisMode === 'pvi') {
        const analytical = computeGasOilBLAnalyticalFromParams(baseResult.params);
        if (analytical) {
            const ooip = getOoip(baseResult.params);
            return {
                rates: { label: 'Reference Solution Gas Cut', values: analytical.gasCut },
                cumulative: {
                    recoveryLabel: 'Reference Solution Recovery',
                    recoveryValues: analytical.recovery,
                    cumulativeLabel: 'Reference Solution Cum Oil',
                    cumulativeValues: analytical.cumulativeOil.map((value) => (
                        Number.isFinite(value) && ooip > 1e-12 ? Number(value) * ooip : null
                    )),
                },
                diagnostics: null,
                xValues: analytical.pviValues,
            };
        }
    }

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
