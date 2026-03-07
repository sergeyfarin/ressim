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
                        curveKeys: analyticalOverlayPrimary
                            ? ['water-cut-sim', 'water-cut-reference', 'avg-water-sat']
                            : ['water-cut-sim', 'avg-water-sat'],
                        curveLabels: analyticalOverlayPrimary
                            ? ['Water Cut (Sim)', 'Water Cut (Reference Solution)', 'Avg Water Sat']
                            : ['Water Cut (Sim)', 'Avg Water Sat'],
                        scalePreset: 'breakthrough',
                        allowLogToggle: false,
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
                    },
                    diagnostics: {
                        title: 'Pressure',
                        curveKeys: ['avg-pressure-sim'],
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
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference', 'oil-rate-error'],
                    curveLabels: ['Oil Rate', 'Oil Rate (Reference Solution)', 'Oil Rate Error'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                cumulative: {
                    title: 'Cumulative Oil / Recovery',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference', 'recovery-factor'],
                    curveLabels: ['Cum Oil', 'Cum Oil (Reference Solution)', 'Recovery Factor'],
                    scalePreset: 'cumulative',
                },
                diagnostics: {
                    title: 'Pressure / Decline',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
                    curveLabels: ['Avg Pressure', 'Avg Pressure (Reference Solution)'],
                    scalePreset: 'pressure',
                },
            },
        },
    };
}