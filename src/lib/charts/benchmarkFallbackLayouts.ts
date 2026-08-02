/**
 * benchmarkFallbackLayouts.ts — default layouts for a family with no scenario.
 *
 * A scenario declares its own layout. A benchmark family has none, so it falls
 * back to one of these, and *which* one is declared on the analytical method's
 * descriptor (`fallbackLayout`) rather than decided by an
 * `if (family.analyticalMethod === …)`.
 *
 * A leaf module on purpose: the registry needs these builders and
 * `referenceChartConfig` needs the registry, so holding them here keeps that a
 * line rather than a cycle (`pnpm run check:cycles`).
 */

import type { ChartLayoutConfig, ChartXAxisMode } from './chartLayoutConfig';

/** What a fallback layout needs to know about the family it is built for. */
export type FallbackLayoutInput = {
    /** Whether the analytical overlay is the primary reference or a secondary one. */
    analyticalOverlayPrimary: boolean;
    /** The family's preferred x-axis. */
    xAxis: ChartXAxisMode;
};

/** Displacement benchmarks: breakthrough leads, recovery and pressure follow. */
export function buildDisplacementBenchmarkLayout(input: FallbackLayoutInput): ChartLayoutConfig {
    const { analyticalOverlayPrimary } = input;
    return {
        chart: {
            xAxisMode: input.xAxis,
            xAxisOptions: ['pvi', 'time', 'cumInjection'],
            allowLogScale: false,
            logScale: false,
            panelOrder: ['rates', 'cumulative', 'avg_water_sat', 'diagnostics'],
            panels: {
                rates: {
                    title: 'Breakthrough',
                    curveKeys: analyticalOverlayPrimary
                        ? ['water-cut-sim', 'water-cut-reference']
                        : ['water-cut-sim'],
                    curveLabels: analyticalOverlayPrimary
                        ? ['Water Cut (Sim)', 'Water Cut (Reference Solution)']
                        : ['Water Cut (Sim)'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                    expanded: true,
                },
                // Average saturation is a different quantity from water cut
                // and gets its own panel rather than a second style in the
                // breakthrough plot.
                avg_water_sat: {
                    title: 'Average Water Saturation',
                    curveKeys: ['avg-water-sat'],
                    curveLabels: ['Avg Water Sat'],
                    scalePreset: 'fraction',
                    visible: true,
                    expanded: false,
                },
                cumulative: {
                    title: 'Recovery',
                    curveKeys: analyticalOverlayPrimary
                        ? ['recovery-factor', 'cum-oil-sim', 'cum-oil-reference', 'cum-injection']
                        : ['recovery-factor', 'cum-oil-sim', 'cum-injection'],
                    curveLabels: analyticalOverlayPrimary
                        ? ['Recovery Factor', 'Cum Oil', 'Cum Oil (Reference Solution)', 'Cum Injection']
                        : ['Recovery Factor', 'Cum Oil', 'Cum Injection'],
                    scalePreset: 'cumulative',
                    expanded: true,
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    curveLabels: ['Avg Pressure'],
                    scalePreset: 'pressure',
                    expanded: false,
                },
            },
        },
    };
}

/** Everything else: rate, cumulative and pressure against time. */
export function buildProductionBenchmarkLayout(input: FallbackLayoutInput): ChartLayoutConfig {
    const xAxisOptions: ChartXAxisMode[] = input.xAxis === 'tD'
        ? ['tD', 'time', 'logTime']
        : ['time', 'tD', 'logTime'];

    return {
        chart: {
            xAxisMode: input.xAxis,
            xAxisOptions,
            allowLogScale: true,
            logScale: false,
            panelOrder: ['rates', 'cumulative', 'diagnostics'],
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference', 'oil-rate-error'],
                    curveLabels: ['Oil Rate', 'Oil Rate (Reference Solution)', 'Oil Rate Error'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                    expanded: true,
                },
                cumulative: {
                    title: 'Cumulative Oil / Recovery',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference', 'recovery-factor'],
                    curveLabels: ['Cum Oil', 'Cum Oil (Reference Solution)', 'Recovery Factor'],
                    scalePreset: 'cumulative',
                    expanded: true,
                },
                diagnostics: {
                    title: 'Pressure / Decline',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
                    curveLabels: ['Avg Pressure', 'Avg Pressure (Reference Solution)'],
                    scalePreset: 'pressure',
                    expanded: true,
                },
            },
        },
    };
}
