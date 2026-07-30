import type {
    RateChartConfig,
    RateChartLayoutConfig,
} from '../charts/rateChartLayoutConfig';

export const CHART_LAYOUTS: Record<string, RateChartLayoutConfig> = {
    waterflood: {
        rateChart: {
            xAxisMode: 'pvi',
            xAxisOptions: ['pvi', 'time', 'cumInjection'],
            xAxisRangePolicy: { mode: 'rate-tail-threshold', relativeThreshold: 1e-7 },
            allowLogScale: false,
            logScale: false,
            panelOrder: ['rates', 'recovery', 'oil_rate', 'cumulative', 'diagnostics', 'volumes'],
            panels: {
                rates: {
                    title: 'Watercut',
                    curveKeys: ['water-cut-sim', 'water-cut-reference'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                    expanded: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary'],
                    scalePreset: 'recovery',
                    expanded: true,
                },
                cumulative: {
                    title: 'Cum Oil',
                    // No 'cum-oil-reference': the Buckley-Leverett method emits
                    // water-cut and recovery references only. See TODO.md.
                    curveKeys: ['cum-oil-sim'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                    expanded: false,
                },
                volumes: {
                    title: 'Cum Injection',
                    curveKeys: ['cum-injection'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
                oil_rate: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    scalePreset: 'rates',
                    expanded: false,
                },
            },
        },
    },

    sweep: {
        rateChart: {
            xAxisMode: 'pvi',
            xAxisOptions: ['pvi', 'time'],
            xAxisRangePolicy: { mode: 'pvi-window', minPvi: 0, maxPvi: 2.5 },
            allowLogScale: false,
            logScale: false,
            panelOrder: ['sweep_rf', 'sweep_areal', 'sweep_vertical', 'sweep_combined', 'sweep_combined_mobile_oil', 'rates', 'recovery', 'cumulative', 'diagnostics'],
            panels: {
                rates: {
                    title: 'Watercut',
                    curveKeys: ['water-cut-sim'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                    expanded: false,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary'],
                    scalePreset: 'recovery',
                    expanded: false,
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                    expanded: false,
                },
                sweep_rf: {
                    title: 'Recovery Factor — Sweep Analysis',
                    scalePreset: 'sweep_rf',
                    visible: true,
                    expanded: true,
                },
                sweep_areal: {
                    title: 'Areal Sweep Efficiency (E_A)',
                    scalePreset: 'sweep',
                    visible: true,
                    expanded: true,
                },
                sweep_vertical: {
                    title: 'Vertical Sweep Efficiency (E_V)',
                    scalePreset: 'sweep',
                    visible: true,
                    expanded: true,
                },
                sweep_combined: {
                    title: 'Combined Sweep Efficiency (E_vol)',
                    scalePreset: 'sweep',
                    visible: true,
                    expanded: true,
                },
                sweep_combined_mobile_oil: {
                    title: 'Analytical Total E_vol vs Simulated Mobile Oil Recovered',
                    scalePreset: 'sweep',
                    visible: false,
                    expanded: false,
                },
            },
        },
    },

    oil_depletion: {
        rateChart: {
            xAxisMode: 'time',
            xAxisOptions: ['time', 'tD', 'logTime'],
            xAxisRangePolicy: { mode: 'data-extent' },
            allowLogScale: true,
            logScale: false,
            panelOrder: ['rates', 'recovery', 'cumulative', 'diagnostics', 'mbe_ooip', 'drive_indices'],
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                    expanded: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                    expanded: true,
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
                diagnostics: {
                    title: 'Average Reservoir Pressure',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
                    scalePreset: 'pressure',
                    expanded: true,
                },
                mbe_ooip: {
                    title: 'Material-Balance OOIP Ratio',
                    curveKeys: ['mbe-ooip-ratio'],
                    scalePreset: 'ratio',
                    visible: true,
                    expanded: false,
                },
                drive_indices: {
                    title: 'Material-Balance Drive Indices',
                    curveKeys: ['drive-compaction', 'drive-oil-expansion', 'drive-gas-cap'],
                    scalePreset: 'fraction',
                    visible: true,
                    expanded: false,
                },
            },
        },
    },

    fetkovich: {
        rateChart: {
            xAxisMode: 'logTime',
            xAxisOptions: ['logTime', 'time', 'tD'],
            xAxisRangePolicy: { mode: 'data-extent' },
            allowLogScale: true,
            logScale: true,
            panelOrder: ['rates', 'recovery', 'cumulative', 'diagnostics', 'mbe_ooip', 'drive_indices'],
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                    expanded: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                    expanded: true,
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
                diagnostics: {
                    title: 'Average Reservoir Pressure',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
                    scalePreset: 'pressure',
                    expanded: true,
                },
                mbe_ooip: {
                    title: 'Material-Balance OOIP Ratio',
                    curveKeys: ['mbe-ooip-ratio'],
                    scalePreset: 'ratio',
                    visible: true,
                    expanded: false,
                },
                drive_indices: {
                    title: 'Material-Balance Drive Indices',
                    curveKeys: ['drive-compaction', 'drive-oil-expansion', 'drive-gas-cap'],
                    scalePreset: 'fraction',
                    visible: true,
                    expanded: false,
                },
            },
        },
    },

    /**
     * Pressure-transient layout. Opens on a log-time axis because that is the
     * plot the analysis is done on — the infinite-acting radial period is a
     * straight line there and nowhere else. The flowing-BHP panel leads; there
     * is no recovery or cumulative panel, because nothing meaningful is
     * produced over the hours-to-days span of a test.
     */
    well_test: {
        rateChart: {
            xAxisMode: 'logTime',
            xAxisOptions: ['logTime', 'time'],
            xAxisRangePolicy: { mode: 'data-extent' },
            allowLogScale: false,
            logScale: false,
            panelOrder: ['producer_bhp', 'diagnostics', 'oil_rate', 'control_limits'],
            panels: {
                producer_bhp: {
                    title: 'Flowing BHP',
                    curveKeys: ['producer-bhp-sim', 'producer-bhp-reference'],
                    scalePreset: 'pressure',
                    // The shared producer-BHP panel is hidden by default;
                    // well-test interpretation makes it the primary exhibit.
                    visible: true,
                    expanded: true,
                },
                oil_rate: {
                    title: 'Oil Rate (Control Check)',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
                    scalePreset: 'rates',
                    // Constant rate is the imposed test condition, not the
                    // response being interpreted. Keep it available to verify
                    // control without letting coincident flat lines dominate.
                    expanded: false,
                },
                diagnostics: {
                    title: 'Average Reservoir Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                    expanded: false,
                },
                control_limits: {
                    title: 'Control-Limit Fraction',
                    curveKeys: ['producer-bhp-limited-sim'],
                    scalePreset: 'fraction',
                    visible: true,
                    expanded: false,
                },
            },
        },
    },

    gas_oil_bl: {
        rateChart: {
            xAxisMode: 'pvi',
            xAxisOptions: ['pvi', 'time', 'cumInjection'],
            xAxisRangePolicy: { mode: 'rate-tail-threshold', relativeThreshold: 1e-7 },
            allowLogScale: false,
            logScale: false,
            panelOrder: ['rates', 'recovery', 'cumulative', 'diagnostics', 'volumes', 'oil_rate'],
            panels: {
                rates: {
                    title: 'Gas Breakthrough',
                    curveKeys: ['gas-cut-sim', 'gas-cut-reference'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                    expanded: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                    expanded: true,
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                    expanded: false,
                },
                volumes: {
                    title: 'Cum Injection',
                    curveKeys: ['cum-injection'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
                oil_rate: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    scalePreset: 'rates',
                    expanded: false,
                },
            },
        },
    },

    spe1: {
        rateChart: {
            xAxisMode: 'time',
            xAxisOptions: ['time', 'logTime'],
            xAxisRangePolicy: { mode: 'data-extent' },
            allowLogScale: false,
            logScale: false,
            panelOrder: ['diagnostics', 'producer_bhp', 'injector_bhp', 'control_limits', 'gor', 'oil_rate', 'injection_rate', 'rates', 'recovery', 'cumulative', 'volumes'],
            panels: {
                diagnostics: {
                    title: 'Reservoir Pressure',
                    curveKeys: ['avg-pressure-sim', 'published-pressure'],
                    scalePreset: 'pressure',
                    expanded: true,
                },
                producer_bhp: {
                    title: 'Producer WBHP',
                    curveKeys: ['producer-bhp-sim', 'published-producer-bhp'],
                    scalePreset: 'pressure',
                    expanded: true,
                },
                injector_bhp: {
                    title: 'Injector WBHP',
                    curveKeys: ['injector-bhp-sim', 'published-injector-bhp'],
                    scalePreset: 'pressure',
                    expanded: true,
                },
                control_limits: {
                    title: 'Control-Limit Fraction',
                    curveKeys: ['producer-bhp-limited-sim', 'injector-bhp-limited-sim'],
                    scalePreset: 'fraction',
                    expanded: false,
                },
                gor: {
                    title: 'GOR',
                    curveKeys: ['gor-sim', 'published-gor'],
                    scalePreset: 'gor',
                    visible: true,
                    expanded: true,
                },
                oil_rate: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'published-oil-rate'],
                    scalePreset: 'rates',
                    expanded: true,
                },
                injection_rate: {
                    title: 'Gas Injection Rate',
                    curveKeys: ['injection-rate-sim', 'published-injection-rate'],
                    scalePreset: 'rates',
                    visible: true,
                    expanded: true,
                },
                rates: {
                    title: 'Gas Cut',
                    curveKeys: ['gas-cut-sim'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                    expanded: false,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary'],
                    scalePreset: 'recovery',
                    expanded: false,
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
                volumes: {
                    title: 'Cum Injection',
                    curveKeys: ['cum-injection'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
            },
        },
    },

    gas: {
        rateChart: {
            xAxisMode: 'time',
            xAxisOptions: ['time', 'logTime'],
            xAxisRangePolicy: { mode: 'data-extent' },
            allowLogScale: true,
            logScale: false,
            panelOrder: ['rates', 'recovery', 'gor', 'cumulative', 'diagnostics'],
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                    expanded: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary'],
                    scalePreset: 'recovery',
                    expanded: true,
                },
                gor: {
                    title: 'Producing GOR',
                    curveKeys: ['gor-sim'],
                    scalePreset: 'gor',
                    visible: true,
                    expanded: true,
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim'],
                    scalePreset: 'cumulative_volumes',
                    expanded: false,
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                    expanded: true,
                },
            },
        },
    },
};

function mergeObjectMap<T extends Record<string, unknown>>(
    base?: Record<string, T>,
    patch?: Record<string, T>,
): Record<string, T> | undefined {
    const keys = new Set([...Object.keys(base ?? {}), ...Object.keys(patch ?? {})]);
    if (keys.size === 0) return undefined;

    const merged: Record<string, T> = {};
    for (const key of keys) {
        const baseValue = base?.[key];
        const patchValue = patch?.[key];
        if (baseValue && patchValue) {
            merged[key] = { ...baseValue, ...patchValue };
            continue;
        }
        if (patchValue) {
            merged[key] = { ...patchValue };
            continue;
        }
        if (baseValue) {
            merged[key] = { ...baseValue };
        }
    }
    return merged;
}

function mergeRateChartConfig(
    base?: RateChartConfig,
    patch?: RateChartConfig,
): RateChartConfig | undefined {
    if (!base && !patch) return undefined;

    return {
        ...(base ?? {}),
        ...(patch ?? {}),
        panelOrder: patch?.panelOrder ?? base?.panelOrder,
        panels: mergeObjectMap(
            base?.panels as Record<string, Record<string, unknown>> | undefined,
            patch?.panels as Record<string, Record<string, unknown>> | undefined,
        ) as RateChartConfig['panels'],
        curves: mergeObjectMap(
            base?.curves as Record<string, Record<string, unknown>> | undefined,
            patch?.curves as Record<string, Record<string, unknown>> | undefined,
        ) as RateChartConfig['curves'],
    };
}

export function mergeChartLayoutConfig(
    base?: RateChartLayoutConfig,
    patch?: RateChartLayoutConfig,
): RateChartLayoutConfig {
    if (!base && !patch) return {};

    return {
        ...(base ?? {}),
        ...(patch ?? {}),
        rateChart: mergeRateChartConfig(base?.rateChart, patch?.rateChart),
    };
}

export function getChartLayout(layoutKey: string | null | undefined): RateChartLayoutConfig {
    const baseLayout = CHART_LAYOUTS[layoutKey ?? ''];
    return baseLayout ? mergeChartLayoutConfig({}, baseLayout) : {};
}
