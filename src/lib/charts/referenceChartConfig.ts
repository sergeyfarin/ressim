/**
 * referenceChartConfig.ts — default chart layouts for a *benchmark* family.
 *
 * A scenario declares its own layout (`chartLayoutKey` + `chartLayoutPatch`).
 * A benchmark family has no scenario, so it falls back to one of the layouts in
 * `benchmarkFallbackLayouts.ts` — and *which* one is a property of the
 * analytical method, declared on its descriptor as `fallbackLayout`, not decided
 * by an `if (family.analyticalMethod === …)` here. That branch was the last of
 * the method ladders `analyticalMethodRegistry.ts` was created to end.
 */

import type { BenchmarkFamily } from '../scenario/referenceTypes';
import type { BenchmarkReferencePolicy } from '../benchmarkRunModel';
import type { ChartLayoutConfig, ChartXAxisMode } from './chartLayoutConfig';
import { getAnalyticalMethodDescriptor } from './analyticalMethodRegistry';

function toXAxisMode(value: BenchmarkFamily['displayDefaults']['xAxis']): ChartXAxisMode {
    if (value === 'pvi') return 'pvi';
    if (value === 'tD') return 'tD';
    return 'time';
}

export function getReferenceChartLayoutConfig(input: {
    family: BenchmarkFamily | null | undefined;
    referencePolicy?: BenchmarkReferencePolicy | null;
}): ChartLayoutConfig {
    const family = input.family ?? null;
    if (!family) return {};

    return getAnalyticalMethodDescriptor(family.analyticalMethod).fallbackLayout({
        analyticalOverlayPrimary: input.referencePolicy?.analyticalOverlayRole !== 'secondary',
        xAxis: toXAxisMode(family.displayDefaults.xAxis),
    });
}
