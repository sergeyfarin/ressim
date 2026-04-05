/**
 * axisAdapters.ts — axis-conversion utilities for the chart layer.
 *
 * All functions that map between PVI, time, cumulative, dimensionless-time,
 * and log-time x-axis modes live here. No chart-building or curve-assembly
 * logic — pure data transformation.
 *
 * Consumed by referenceComparisonModel.ts today; will be consumed directly by
 * buildChartData.ts (Phase 4) when referenceComparisonModel.ts is replaced.
 */

import type { RateChartXAxisMode } from './rateChartLayoutConfig';

// ─── Shared series type ───────────────────────────────────────────────────────

/**
 * Pre-computed per-run series derived from a BenchmarkRunResult's rateHistory.
 * This is the primary data structure consumed by axis adapters.
 *
 * Each array is aligned to rateHistory indices (same length as rateHistory).
 * historyTime aligns to snapshot history (may differ in length from rateHistory).
 */
export type DerivedRunSeries = {
    time: number[];
    historyTime: number[];
    oilRate: Array<number | null>;
    injectionRate: Array<number | null>;
    waterCut: Array<number | null>;
    gasCut: Array<number | null>;
    avgWaterSat: Array<number | null>;
    pressure: Array<number | null>;
    producerBhp: Array<number | null>;
    injectorBhp: Array<number | null>;
    recovery: Array<number | null>;
    cumulativeOil: Array<number | null>;
    cumulativeInjection: Array<number | null>;
    cumulativeLiquid: Array<number | null>;
    cumulativeGas: Array<number | null>;
    p_z: Array<number | null>;
    pvi: Array<number | null>;
    pvp: Array<number | null>;
    gor: Array<number | null>;
    producerBhpLimitedFraction: Array<number | null>;
    injectorBhpLimitedFraction: Array<number | null>;
};

// ─── XY series helper ─────────────────────────────────────────────────────────

export type XYPoint = { x: number; y: number | null };

/**
 * Zips a parallel x-values array and y-values array into `{ x, y }` points,
 * skipping entries where x is non-finite. y is set to null when non-finite.
 */
export function toXYSeries(
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

// ─── x-axis building ──────────────────────────────────────────────────────────

/**
 * Returns the x-axis value array for a completed simulation run, given a
 * DerivedRunSeries and the currently selected axis mode.
 *
 * `tau` (days) is only required for `tD` (dimensionless time) mode; pass null
 * or omit for all other modes.
 */
export function buildXAxisValues(
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
    tau: number | null = null,
): Array<number | null> {
    if (xAxisMode === 'pvi') return [...derived.pvi];
    if (xAxisMode === 'pvp') return [...derived.pvp];
    if (xAxisMode === 'cumInjection') return [...derived.cumulativeInjection];
    if (xAxisMode === 'cumLiquid') return [...derived.cumulativeLiquid];
    if (xAxisMode === 'cumGas') return [...derived.cumulativeGas];
    if (xAxisMode === 'logTime') return derived.time.map((value) => (value > 0 ? Math.log10(value) : null));
    if (xAxisMode === 'tD' && Number.isFinite(tau) && (tau as number) > 0) {
        return derived.time.map((value) => value / (tau as number));
    }
    return [...derived.time];
}

/**
 * The x-axis value that represents "zero" for sweep efficiency panels.
 * Returns `null` for log-time (log(0) is undefined), and `0` for all other modes.
 */
export function getSweepZeroXAxisValue(xAxisMode: RateChartXAxisMode): number | null {
    return xAxisMode === 'logTime' ? null : 0;
}

// ─── PVI remapping ────────────────────────────────────────────────────────────

/**
 * Remaps a PVI-indexed analytical curve onto the currently selected x-axis.
 *
 * Analytical solutions (BL, sweep) are natively computed over a uniform PVI
 * grid. When the user selects a non-PVI axis (time, cum injection, etc.),
 * each PVI value must be mapped to the corresponding x-axis value of a
 * completed simulation run, via linear interpolation through the run's own
 * PVI series.
 *
 * Returns an x-axis-aligned array of the same length as `pviValues`.
 */
export function mapPviSeriesToXAxis(
    pviValues: Array<number | null>,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
    tau: number | null,
): Array<number | null> {
    if (xAxisMode === 'pvi') return [...pviValues];

    const mappedAxis = buildXAxisValues(derived, xAxisMode, tau);
    return pviValues.map((targetPvi) => {
        if (!Number.isFinite(targetPvi)) return null;
        if ((targetPvi as number) <= 1e-12) return getSweepZeroXAxisValue(xAxisMode);

        let previousIndex = -1;
        for (let index = 0; index < derived.pvi.length; index += 1) {
            const domain = derived.pvi[index];
            const range = mappedAxis[index];
            if (!Number.isFinite(domain) || !Number.isFinite(range)) continue;
            if (Math.abs((domain as number) - (targetPvi as number)) <= 1e-9) return Number(range);
            if ((domain as number) > (targetPvi as number)) {
                if (previousIndex < 0) return Number(range);
                const d0 = Number(derived.pvi[previousIndex]);
                const r0 = Number(mappedAxis[previousIndex]);
                const d1 = Number(domain);
                const r1 = Number(range);
                if (Math.abs(d1 - d0) <= 1e-12) return r1;
                const fraction = ((targetPvi as number) - d0) / (d1 - d0);
                return r0 + fraction * (r1 - r0);
            }
            previousIndex = index;
        }

        return previousIndex >= 0 && Number.isFinite(mappedAxis[previousIndex])
            ? Number(mappedAxis[previousIndex])
            : null;
    });
}

/**
 * Interpolates an x-axis series (e.g. cumulative injection) at a set of
 * target time values, using a source (time, xAxis) pair as the domain.
 *
 * Used when an analytical solution is natively in time and must be sampled at
 * the same time points as the simulation history.
 */
export function interpolateXAxisAtTimes(
    sourceTimes: Array<number | null>,
    sourceXAxis: Array<number | null>,
    targetTimes: Array<number | null>,
): Array<number | null> {
    const result: Array<number | null> = [];
    let previousIndex = -1;

    for (const rawTarget of targetTimes) {
        if (!Number.isFinite(rawTarget)) {
            result.push(null);
            continue;
        }

        const target = Number(rawTarget);
        while (previousIndex + 1 < sourceTimes.length) {
            const nextTime = sourceTimes[previousIndex + 1];
            if (!Number.isFinite(nextTime) || Number(nextTime) < target) {
                previousIndex += 1;
                continue;
            }
            break;
        }

        if (previousIndex < 0) {
            result.push(Number.isFinite(sourceXAxis[0]) ? Number(sourceXAxis[0]) : null);
            continue;
        }

        if (previousIndex + 1 >= sourceTimes.length) {
            result.push(Number.isFinite(sourceXAxis[previousIndex]) ? Number(sourceXAxis[previousIndex]) : null);
            continue;
        }

        const x0 = sourceTimes[previousIndex];
        const x1 = sourceTimes[previousIndex + 1];
        const y0 = sourceXAxis[previousIndex];
        const y1 = sourceXAxis[previousIndex + 1];

        if (!Number.isFinite(x0) || !Number.isFinite(x1) || !Number.isFinite(y0) || !Number.isFinite(y1)) {
            result.push(Number.isFinite(y0) ? Number(y0) : null);
            continue;
        }

        if (Math.abs(Number(x1) - Number(x0)) <= 1e-12) {
            result.push(Number(y1));
            continue;
        }

        const fraction = (target - Number(x0)) / (Number(x1) - Number(x0));
        result.push(Number(y0) + fraction * (Number(y1) - Number(y0)));
    }

    return result;
}

// ─── Analytical overlay axis-mapping predicates ───────────────────────────────

/**
 * Returns true when the selected x-axis mode requires analytical overlays
 * to be remapped from each completed simulation run's own axis values,
 * rather than plotted directly on their native PVI grid.
 *
 * BL-family solutions are natively PVI; any non-PVI axis requires run-based
 * remapping. Depletion solutions are natively in time; no remapping needed.
 */
export function requiresRunMappedAnalyticalXAxis(
    analyticalMethod: string | null | undefined,
    xAxisMode: RateChartXAxisMode,
): boolean {
    if (
        analyticalMethod === 'buckley-leverett' ||
        analyticalMethod === 'waterflood' ||
        analyticalMethod === 'gas-oil-bl'
    ) {
        return xAxisMode !== 'pvi';
    }
    return false;
}

/**
 * Builds a user-visible warning string when analytical overlays cannot be
 * shown at full fidelity on the selected axis (e.g. no run data to remap from).
 */
export function buildAnalyticalAxisWarning(input: {
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
