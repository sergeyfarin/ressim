export type RateChartXAxisMode =
    | 'time'
    | 'tD'
    | 'logTime'
    | 'pvi'
    | 'pvp'
    | 'cumLiquid'
    | 'cumInjection'
    | 'cumGas';

export type RateChartPrimaryPanelId = 'rates' | 'recovery' | 'cumulative' | 'diagnostics' | 'avg_water_sat' | 'mbe_ooip' | 'drive_indices' | 'pz' | 'pss_drawdown' | 'pss_productivity' | 'pss_shape_factor' | 'gor' | 'volumes' | 'oil_rate' | 'injection_rate' | 'producer_bhp' | 'injector_bhp' | 'control_limits';

export type RateChartSweepPanelId =
    | 'sweep_rf'
    | 'sweep_areal'
    | 'sweep_vertical'
    | 'sweep_combined'
    | 'sweep_combined_mobile_oil';

export type RateChartPanelId = RateChartPrimaryPanelId | RateChartSweepPanelId;

export type RateChartPanelKey = RateChartPrimaryPanelId;
export type RateChartAuxiliaryPanelKey = RateChartSweepPanelId;

export const DEFAULT_RATE_CHART_PANEL_ORDER: RateChartPanelId[] = [
    'rates',
    'recovery',
    'cumulative',
    'diagnostics',
    'avg_water_sat',
    'mbe_ooip',
    'drive_indices',
    'pz',
    'pss_drawdown',
    'pss_productivity',
    'pss_shape_factor',
    'gor',
    'volumes',
    'oil_rate',
    'injection_rate',
    'producer_bhp',
    'injector_bhp',
    'control_limits',
    'sweep_rf',
    'sweep_areal',
    'sweep_vertical',
    'sweep_combined',
    'sweep_combined_mobile_oil',
];

export type RateChartScalePreset = 'rates' | 'cumulative' | 'cumulative_volumes' | 'diagnostics' | 'breakthrough' | 'pressure' | 'productivity' | 'shape_factor' | 'ratio' | 'gor' | 'recovery' | 'sweep' | 'sweep_rf' | 'fraction';

export type RateChartXAxisRangePolicy =
    | { mode: 'data-extent' }
    | { mode: 'rate-tail-threshold'; relativeThreshold?: number }
    | { mode: 'pvi-window'; minPvi?: number; maxPvi: number };

export type RateChartCurveOverride = {
    disabled?: boolean;
    visible?: boolean;
};

export type RateChartPanelLayout = {
    title?: string;
    curveKeys?: string[];
    curveLabels?: string[];
    scalePreset?: RateChartScalePreset;
    allowLogToggle?: boolean;
    /**
     * Default y-axis scaling for this panel. Log scaling is per panel — a
     * shape factor spanning two orders of magnitude and a drawdown spanning
     * 35% do not belong on the same axis type. Falls back to the chart-level
     * `logScale` when unset.
     */
    logScale?: boolean;
    visible?: boolean;
    expanded?: boolean;
    /**
     * Suppress a short leading transient whose values exceed a robust median
     * operating rate. Opt-in because a high initial rate can be real physics.
     */
    suppressLeadingOutliers?: {
        medianRatio: number;
        maxLeadingFraction?: number;
    };
};

export type RateChartConfig = {
    logScale?: boolean;
    allowLogScale?: boolean;
    xAxisMode?: RateChartXAxisMode;
    xAxisOptions?: RateChartXAxisMode[];
    xAxisRangePolicy?: RateChartXAxisRangePolicy;
    ratesExpanded?: boolean;
    recoveryExpanded?: boolean;
    cumulativeExpanded?: boolean;
    diagnosticsExpanded?: boolean;
    volumesExpanded?: boolean;
    oilRateExpanded?: boolean;
    panelOrder?: RateChartPanelId[];
    panels?: Partial<Record<RateChartPanelId, RateChartPanelLayout>>;
    curves?: Record<string, RateChartCurveOverride>;
};

export type RateChartLayoutConfig = {
    rateChart?: RateChartConfig;
};
