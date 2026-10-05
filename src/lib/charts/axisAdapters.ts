/**
 * axisAdapters.ts — axis-conversion utilities for the chart layer.
 *
 * All functions that map between PVI, time, cumulative, dimensionless-time,
 * and log-time x-axis modes live here. No chart-building or curve-assembly
 * logic — pure data transformation.
 *
 * Consumed by buildChartData.ts, referenceOverlayBuilders.ts, and sweepPanelBuilder.ts.
 */

import type { ChartXAxisMode } from './chartLayoutConfig';

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
    /** Produced gas rate at surface conditions [Sm³/day]. */
    gasRate: Array<number | null>;
    injectionRate: Array<number | null>;
    waterCut: Array<number | null>;
    gasCut: Array<number | null>;
    avgWaterSat: Array<number | null>;
    pressure: Array<number | null>;
    producerBhp: Array<number | null>;
    injectorBhp: Array<number | null>;
    recovery: Array<number | null>;
    /** Gas recovery (produced gas / GIIP); null where the case holds no gas. */
    recoveryGas: Array<number | null>;
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
    xAxisMode: ChartXAxisMode,
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
export function getSweepZeroXAxisValue(xAxisMode: ChartXAxisMode): number | null {
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
    xAxisMode: ChartXAxisMode,
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
                if (previousIndex < 0) {
                    return xAxisMode !== 'logTime' && Number(domain) > 0
                        ? Number(targetPvi) / Number(domain) * Number(range)
                        : Number(range);
                }
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
 * target time values, using chronological source (time, xAxis) samples.
 * Queries may arrive in any order. Missing samples are skipped; originValue
 * supplies an explicit t=0 anchor when the selected axis has a known origin.
 *
 * Used when an analytical solution is natively in time and must be sampled at
 * the same time points as the simulation history.
 */
export function interpolateXAxisAtTimes(
    sourceTimes: Array<number | null>,
    sourceXAxis: Array<number | null>,
    targetTimes: Array<number | null>,
    originValue?: number,
): Array<number | null> {
    const samples = sourceTimes.flatMap((time, index) =>
        Number.isFinite(time) && Number.isFinite(sourceXAxis[index])
            ? [{ time: Number(time), value: Number(sourceXAxis[index]) }] : []);
    if (originValue !== undefined && samples.length > 0 && samples[0].time > 0) {
        samples.unshift({ time: 0, value: originValue });
    }
    return targetTimes.map((rawTarget) => {
        if (!Number.isFinite(rawTarget) || samples.length === 0) return null;
        const target = Number(rawTarget);
        const first = samples[0];
        if (target <= first.time) return first.value;
        if (target >= samples[samples.length - 1].time) return samples[samples.length - 1].value;
        // Binary search keeps arbitrary query order from making long histories quadratic.
        let lo = 1;
        let hi = samples.length - 1;
        while (lo < hi) {
            const mid = Math.floor((lo + hi) / 2);
            if (samples[mid].time < target) lo = mid + 1;
            else hi = mid;
        }
        const next = samples[lo];
        const previous = samples[lo - 1];
        const span = next.time - previous.time;
        if (span <= 1e-12) return next.value;
        return previous.value + (target - previous.time) / span * (next.value - previous.value);
    });
}

// ─── Reference-series axis mapping ────────────────────────────────────────────

/**
 * The time -> axis mapping a precomputed reference run carries with it.
 *
 * Reference series (OPM Flow artifacts, digitized published data) are recorded
 * against days. Drawing them on a pore-volumes-injected axis needs the
 * reference run's *own* injected volume and pore volume — not the scenario's,
 * which may have been re-parameterised since, and not the simulation's, which
 * is a different run.
 */
export type ReferenceXAxisMap = {
    /** Report times of the reference run, in days. */
    timeDays: number[];
    /**
     * Pore volumes injected at each of those times. Absent for a case with no
     * injector, which is not a defect — a depletion run has no PVI to publish.
     */
    pvi?: number[];
    /**
     * Cumulative injection at *surface* conditions at each of those times, Sm³:
     * the basis of the simulation's own cumulative-injection axis. The reservoir
     * volume the PVI mapping is built from is not interchangeable with it; for gas
     * the two differ by Bg (#20).
     */
    cumulativeInjectionSm3?: number[];
    /** Cumulative surface gas production at each of those times, Sm³. */
    cumulativeGasSm3?: number[];
};

/**
 * Maps a reference series' time values (days) onto the selected x-axis.
 *
 * Returns `null` when the axis cannot be honoured — either because the series
 * carries no mapping or because the mode has no reference-side counterpart
 * (produced volumes, dimensionless time). Callers must drop the series in that
 * case: a reference curve plotted at the wrong x is worse than an absent one,
 * because it reads as disagreement between the simulator and its ground truth.
 */
export function mapReferenceTimesToXAxis(
    timeDays: Array<number | null>,
    xAxisMode: ChartXAxisMode,
    map: ReferenceXAxisMap | null | undefined,
): Array<number | null> | null {
    if (xAxisMode === 'time') {
        return timeDays.map((value) => (Number.isFinite(value) ? Number(value) : null));
    }
    if (xAxisMode === 'logTime') {
        return timeDays.map((value) => (
            Number.isFinite(value) && Number(value) > 0 ? Math.log10(Number(value)) : null
        ));
    }
    if (!map || map.timeDays.length === 0) return null;
    if (xAxisMode === 'pvi') {
        return map.pvi ? interpolateXAxisAtTimes(map.timeDays, map.pvi, timeDays) : null;
    }
    if (xAxisMode === 'cumInjection') {
        return map.cumulativeInjectionSm3
            ? interpolateXAxisAtTimes(map.timeDays, map.cumulativeInjectionSm3, timeDays)
            : null;
    }
    if (xAxisMode === 'cumGas') {
        return map.cumulativeGasSm3
            ? interpolateXAxisAtTimes(map.timeDays, map.cumulativeGasSm3, timeDays)
            : null;
    }
    // 'tD', 'pvp', 'cumLiquid': the reference run publishes nothing that pins
    // these, so there is no honest x for its points.
    return null;
}

// ─── Analytical overlay axis-mapping predicates ───────────────────────────────

/**
 * Returns true when the selected x-axis mode requires analytical overlays
 * to be remapped from each completed simulation run's own axis values,
 * rather than plotted directly on their native PVI grid.
 *
 * Takes the solution's native axis rather than the method name: which methods
 * are PVI-native is declared once in `analyticalMethodRegistry.ts`, so this
 * module stays a leaf and cannot drift out of sync with the method list.
 * BL-family solutions require run-based remapping away from PVI. Time-native
 * solutions require it on volume axes; time, log time and characteristic time
 * can be built without completed injection/production histories.
 */
export function requiresRunMappedAnalyticalXAxis(
    nativeXAxis: 'pvi' | 'time' | null | undefined,
    xAxisMode: ChartXAxisMode,
): boolean {
    return nativeXAxis === 'pvi' ? xAxisMode !== 'pvi'
        : nativeXAxis === 'time' && !['time', 'logTime', 'tD'].includes(xAxisMode);
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
