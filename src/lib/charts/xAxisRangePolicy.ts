import type { RateChartXAxisMode, RateChartXAxisRangePolicy } from './rateChartLayoutConfig';

export type XYPoint = { x: number; y: number | null };

export type AxisMapping = {
    domainValues: Array<number | null>;
    rangeValues: Array<number | null>;
};

const TARGET_X_AXIS_TICK_COUNT = 6;

function getStepDecimalPlaces(step: number): number {
    if (!Number.isFinite(step) || step <= 0) return 0;

    for (let decimals = 0; decimals <= 12; decimals += 1) {
        const scaled = step * (10 ** decimals);
        if (Math.abs(scaled - Math.round(scaled)) <= 1e-9) return decimals;
    }

    return 12;
}

function normalizeFloatingValue(value: number, step?: number): number {
    if (!Number.isFinite(value)) return value;
    if (Number.isFinite(step) && Number(step) > 0) {
        return Number(value.toFixed(getStepDecimalPlaces(Number(step))));
    }
    return Number.parseFloat(value.toPrecision(12));
}

function getNiceStepSize(range: number, targetTickCount = TARGET_X_AXIS_TICK_COUNT): number {
    if (!Number.isFinite(range) || range <= 0) return 0;

    const targetIntervals = Math.max(1, targetTickCount - 1);
    const rawStep = range / targetIntervals;
    const exponent = Math.floor(Math.log10(rawStep));
    const magnitude = 10 ** exponent;
    const normalized = rawStep / magnitude;

    if (normalized <= 1) return magnitude;
    if (normalized <= 2) return 2 * magnitude;
    if (normalized <= 2.5) return 2.5 * magnitude;
    if (normalized <= 5) return 5 * magnitude;
    return 10 * magnitude;
}

function snapBoundaryToNiceValue(value: number, step: number): number {
    if (!Number.isFinite(value) || !Number.isFinite(step) || step <= 0) {
        return value;
    }

    const candidateSteps = [step, step / 2, step / 4].filter((candidate, index, values) => (
        Number.isFinite(candidate)
        && candidate > 0
        && values.indexOf(candidate) === index
    ));

    for (const candidateStep of candidateSteps) {
        const nearestMultiple = Math.round(value / candidateStep) * candidateStep;
        const tolerance = Math.max(candidateStep * 0.15, Math.abs(value) * 1e-9, 1e-12);

        if (Math.abs(value - nearestMultiple) <= tolerance) {
            return normalizeFloatingValue(nearestMultiple, candidateStep);
        }
    }

    return normalizeFloatingValue(value);
}

function snapSharedXAxisRange(range: { min: number; max: number }): { min: number; max: number } {
    const span = range.max - range.min;
    if (!Number.isFinite(span) || span <= 0) return range;

    const step = getNiceStepSize(span);
    if (step <= 0) return range;

    const snapped = {
        min: snapBoundaryToNiceValue(range.min, step),
        max: snapBoundaryToNiceValue(range.max, step),
    };

    return snapped.min < snapped.max ? snapped : range;
}

function computeSeriesExtent(seriesGroups: XYPoint[][]): { min: number; max: number } | undefined {
    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;

    for (const series of seriesGroups) {
        for (const point of series) {
            if (!Number.isFinite(point.x)) continue;
            min = Math.min(min, point.x);
            max = Math.max(max, point.x);
        }
    }

    if (!Number.isFinite(min) || !Number.isFinite(max) || min >= max) return undefined;
    return { min, max };
}

export function interpolateMappedAxisValue(
    targetDomainValue: number,
    mapping: AxisMapping,
    xAxisMode: RateChartXAxisMode,
): number | null {
    if (!Number.isFinite(targetDomainValue)) return null;
    if (targetDomainValue <= 1e-12) return xAxisMode === 'logTime' ? null : 0;

    let previousIndex = -1;
    for (let index = 0; index < mapping.domainValues.length; index += 1) {
        const domainValue = mapping.domainValues[index];
        const rangeValue = mapping.rangeValues[index];
        if (!Number.isFinite(domainValue) || !Number.isFinite(rangeValue)) continue;
        if (Math.abs(Number(domainValue) - targetDomainValue) <= 1e-9) return Number(rangeValue);
        if (Number(domainValue) > targetDomainValue) {
            if (previousIndex < 0) return Number(rangeValue);
            const d0 = Number(mapping.domainValues[previousIndex]);
            const r0 = Number(mapping.rangeValues[previousIndex]);
            const d1 = Number(domainValue);
            const r1 = Number(rangeValue);
            if (Math.abs(d1 - d0) <= 1e-12) return r1;
            const fraction = (targetDomainValue - d0) / (d1 - d0);
            return r0 + fraction * (r1 - r0);
        }
        previousIndex = index;
    }

    if (previousIndex >= 1) {
        const d0 = Number(mapping.domainValues[previousIndex - 1]);
        const r0 = Number(mapping.rangeValues[previousIndex - 1]);
        const d1 = Number(mapping.domainValues[previousIndex]);
        const r1 = Number(mapping.rangeValues[previousIndex]);
        if (Number.isFinite(d0) && Number.isFinite(r0) && Number.isFinite(d1) && Number.isFinite(r1) && Math.abs(d1 - d0) > 1e-12) {
            const fraction = (targetDomainValue - d0) / (d1 - d0);
            return r0 + fraction * (r1 - r0);
        }
    }

    return previousIndex >= 0 && Number.isFinite(mapping.rangeValues[previousIndex])
        ? Number(mapping.rangeValues[previousIndex])
        : null;
}

export function resolveSharedXAxisRange(input: {
    allSeries: XYPoint[][];
    rateSeries?: XYPoint[][];
    xAxisMode: RateChartXAxisMode;
    policy?: RateChartXAxisRangePolicy;
    pviMappings?: AxisMapping[];
}): { min: number; max: number } | undefined {
    const extent = computeSeriesExtent(input.allSeries);
    if (!extent) return undefined;

    const policy = input.policy ?? { mode: 'rate-tail-threshold' as const, relativeThreshold: 1e-7 };

    if (policy.mode === 'data-extent') return snapSharedXAxisRange(extent);

    if (policy.mode === 'rate-tail-threshold') {
        const rateSeries = input.rateSeries ?? [];
        let peakRate = 0;
        for (const series of rateSeries) {
            for (const point of series) {
                if (Number.isFinite(point.y) && Number(point.y) > peakRate) peakRate = Number(point.y);
            }
        }
        if (peakRate <= 0) return extent;

        const threshold = peakRate * (policy.relativeThreshold ?? 1e-7);
        let clippedMax = extent.min;
        for (const series of rateSeries) {
            for (const point of series) {
                if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) continue;
                if (Number(point.y) > threshold) clippedMax = Math.max(clippedMax, point.x);
            }
        }

        return snapSharedXAxisRange({
            min: extent.min,
            max: clippedMax > extent.min ? Math.min(extent.max, clippedMax) : extent.max,
        });
    }

    const minCandidates: number[] = [];
    const maxCandidates: number[] = [];
    if (policy.minPvi != null) {
        if (input.pviMappings && input.pviMappings.length > 0) {
            for (const mapping of input.pviMappings) {
                const mapped = interpolateMappedAxisValue(policy.minPvi, mapping, input.xAxisMode);
                if (Number.isFinite(mapped)) minCandidates.push(Number(mapped));
            }
        } else if (input.xAxisMode === 'pvi') {
            minCandidates.push(policy.minPvi);
        }
    }
    if (input.pviMappings && input.pviMappings.length > 0) {
        for (const mapping of input.pviMappings) {
            const mapped = interpolateMappedAxisValue(policy.maxPvi, mapping, input.xAxisMode);
            if (Number.isFinite(mapped)) maxCandidates.push(Number(mapped));
        }
    } else if (input.xAxisMode === 'pvi') {
        maxCandidates.push(policy.maxPvi);
    }

    const min = minCandidates.length > 0 ? Math.min(extent.min, Math.min(...minCandidates)) : extent.min;
    const max = maxCandidates.length > 0 ? Math.max(extent.max, Math.max(...maxCandidates)) : extent.max;

    return min < max ? snapSharedXAxisRange({ min, max }) : snapSharedXAxisRange(extent);
}