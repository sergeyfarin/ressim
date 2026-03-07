import type { BenchmarkFamily } from '../catalog/benchmarkCases';
import type { BenchmarkReferencePolicy } from '../benchmarkRunModel';
import type { RateChartLayoutConfig, RateChartXAxisMode } from './rateChartLayoutConfig';

function toXAxisMode(value: BenchmarkFamily['displayDefaults']['xAxis']): RateChartXAxisMode {
    if (value === 'pvi') return 'pvi';
    if (value === 'tD') return 'tD';
    return 'time';
}

export function getReferenceRateChartLayoutConfig(input: {
    family: BenchmarkFamily | null | undefined;
    referencePolicy?: BenchmarkReferencePolicy | null;
}): RateChartLayoutConfig {
    const family = input.family ?? null;
    if (!family) return {};

    if (family.scenarioClass === 'buckley-leverett') {
        const analyticalOverlayPrimary = input.referencePolicy?.analyticalOverlayRole !== 'secondary';

        return {
            rateChart: {
                xAxisMode: toXAxisMode(family.displayDefaults.xAxis),
                xAxisOptions: ['pvi', 'time', 'cumInjection'],
                allowLogScale: false,
                logScale: false,
                ratesExpanded: true,
                cumulativeExpanded: true,
                diagnosticsExpanded: false,
                panels: {
                    rates: {
                        title: 'Breakthrough',
                        curveLabels: analyticalOverlayPrimary
                            ? ['Water Cut (Sim)', 'Water Cut (Analytical)', 'Avg Water Sat']
                            : ['Water Cut (Sim)', 'Avg Water Sat'],
                        scalePreset: 'breakthrough',
                        allowLogToggle: false,
                    },
                    cumulative: {
                        title: 'Recovery',
                        curveLabels: analyticalOverlayPrimary
                            ? ['Recovery Factor', 'Cum Oil', 'Cum Oil (Analytical)', 'Cum Injection']
                            : ['Recovery Factor', 'Cum Oil', 'Cum Injection'],
                        scalePreset: 'cumulative',
                    },
                    diagnostics: {
                        title: 'Pressure',
                        curveLabels: ['Avg Pressure'],
                        scalePreset: 'pressure',
                    },
                },
            },
        };
    }

    const depletionXAxis = family.key === 'fetkovich_exp' ? 'logTime' : toXAxisMode(family.displayDefaults.xAxis);
    const depletionXAxisOptions: RateChartXAxisMode[] = family.key === 'fetkovich_exp'
        ? ['logTime', 'time', 'tD']
        : ['time', 'tD', 'logTime'];

    return {
        rateChart: {
            xAxisMode: depletionXAxis,
            xAxisOptions: depletionXAxisOptions,
            allowLogScale: true,
            logScale: family.key === 'fetkovich_exp',
            ratesExpanded: true,
            cumulativeExpanded: true,
            diagnosticsExpanded: true,
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveLabels: ['Oil Rate', 'Oil Rate (Analytical)', 'Oil Rate Error'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                cumulative: {
                    title: 'Cumulative Oil / Recovery',
                    curveLabels: ['Cum Oil', 'Cum Oil (Analytical)', 'Recovery Factor'],
                    scalePreset: 'cumulative',
                },
                diagnostics: {
                    title: 'Pressure / Decline',
                    curveLabels: ['Avg Pressure', 'Avg Pressure (Analytical)'],
                    scalePreset: 'pressure',
                },
            },
        },
    };
}