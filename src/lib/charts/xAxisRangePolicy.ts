import type { RateChartXAxisMode, RateChartXAxisRangePolicy } from './rateChartLayoutConfig';

export type XYPoint = { x: number; y: number | null };

export type AxisMapping = {
    domainValues: Array<number | null>;
    rangeValues: Array<number | null>;
};

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

    if (policy.mode === 'data-extent') return extent;

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

        return {
            min: extent.min,
            max: clippedMax > extent.min ? Math.min(extent.max, clippedMax) : extent.max,
        };
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

    return min < max ? { min, max } : extent;
}