import type { BenchmarkFamily } from '../scenario/referenceTypes';
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

    if (family.analyticalMethod === 'buckley-leverett') {
        const analyticalOverlayPrimary = input.referencePolicy?.analyticalOverlayRole !== 'secondary';

        return {
            rateChart: {
                xAxisMode: toXAxisMode(family.displayDefaults.xAxis),
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

    const depletionXAxis = toXAxisMode(family.displayDefaults.xAxis);
    const depletionXAxisOptions: RateChartXAxisMode[] = depletionXAxis === 'tD'
        ? ['tD', 'time', 'logTime']
        : ['time', 'tD', 'logTime'];

    return {
        rateChart: {
            xAxisMode: depletionXAxis,
            xAxisOptions: depletionXAxisOptions,
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
