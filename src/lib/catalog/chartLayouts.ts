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
            panelOrder: ['rates', 'recovery', 'oil_rate', 'cumulative', 'avg_water_sat', 'diagnostics', 'volumes'],
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
                // Its own panel — average saturation is not water cut, and
                // one property per plot is what keeps every ResSim curve solid.
                avg_water_sat: {
                    title: 'Average Water Saturation',
                    curveKeys: ['avg-water-sat'],
                    scalePreset: 'fraction',
                    visible: true,
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
            panelOrder: ['sweep_rf', 'sweep_areal', 'sweep_vertical', 'sweep_combined', 'sweep_combined_mobile_oil', 'rates', 'recovery', 'cumulative', 'avg_water_sat', 'diagnostics'],
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
                    // Hidden: sweep_rf is the same recovery factor with the
                    // analytical reference beside it, so this panel repeated
                    // the chart at the top of the page with the reference
                    // stripped out.
                    visible: false,
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

    /**
     * Dry-gas material balance. The chart opens on cumulative gas produced
     * rather than time, because that is the axis on which the balance is a
     * straight line — on a time axis the same curves are two indistinguishable
     * declines and the reserves statement is invisible.
     */
    gas_material_balance: {
        rateChart: {
            xAxisMode: 'cumGas',
            xAxisOptions: ['cumGas', 'time'],
            xAxisRangePolicy: { mode: 'data-extent' },
            allowLogScale: false,
            logScale: false,
            panelOrder: ['pz', 'diagnostics', 'recovery', 'gas_rate', 'control_limits'],
            panels: {
                // Gas recovery, not oil. This reservoir holds no oil, and until
                // 2026-08-02 the shared "OOIP" was a reservoir volume of
                // everything that is not water, so a dry-gas case had a recovery
                // factor quoted against 480,000 m3 of oil that does not exist.
                // The oil curve is now null here by construction and the gas
                // curve is the one that means something: G_p/G, which on the p/z
                // plot above is the x-intercept the straight line is drawn to.
                recovery: {
                    title: 'Recovery Factor — Gas (of GIIP)',
                    curveKeys: ['recovery-factor-gas'],
                    scalePreset: 'recovery',
                    visible: true,
                    expanded: false,
                },
                pz: {
                    title: 'p/z',
                    curveKeys: [
                        'p-over-z-sim',
                        'p-over-z-reference',
                        'p-over-z-compaction-reference',
                    ],
                    scalePreset: 'pressure',
                    visible: true,
                    expanded: true,
                },
                diagnostics: {
                    title: 'Average Reservoir Pressure',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
                    scalePreset: 'pressure',
                    expanded: true,
                },
                // ResSim's own gas rate, not its oil rate. Until 2026-08-02
                // this panel was titled "Gas Rate" and drew `oil-rate-sim`,
                // which in a reservoir with no oil is identically zero.
                gas_rate: {
                    title: 'Gas Rate',
                    curveKeys: ['gas-rate-sim'],
                    scalePreset: 'rates',
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
            panelOrder: ['diagnostics', 'producer_bhp', 'injector_bhp', 'control_limits', 'gor', 'oil_rate', 'injection_rate', 'rates', 'recovery', 'cumulative', 'cumulative_gas', 'volumes'],
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
            panelOrder: ['gor', 'recovery', 'rates', 'cumulative', 'diagnostics', 'mbe_ooip', 'drive_indices'],
            panels: {
                // The material-balance check. These cases carry no closed-form
                // overlay, so this is the only curve on the chart that the run
                // can be judged against — and it is judged against itself:
                // N_mbe/N_volumetric away from 1.0 is a balance the simulation
                // did not close, and the drive indices say which mechanism is
                // actually producing the oil rather than leaving the reader to
                // infer it from the GOR.
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
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                    expanded: false,
                },
                gor: {
                    title: 'Producing GOR',
                    curveKeys: ['gor-sim'],
                    scalePreset: 'gor',
                    visible: true,
                    expanded: true,
                },
                recovery: {
                    // Both phases: a solution-gas-drive case recovers oil and
                    // gas, and they are fractions of different volumes in place.
                    title: 'Recovery Factor — Oil and Gas',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-gas'],
                    scalePreset: 'recovery',
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
