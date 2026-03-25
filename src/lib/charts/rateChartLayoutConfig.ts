export type RateChartXAxisMode =
    | 'time'
    | 'tD'
    | 'logTime'
    | 'pvi'
    | 'pvp'
    | 'cumLiquid'
    | 'cumInjection'
    | 'cumGas';

export type RateChartPrimaryPanelId = 'rates' | 'recovery' | 'cumulative' | 'diagnostics' | 'gor' | 'volumes' | 'oil_rate';

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
    'gor',
    'volumes',
    'oil_rate',
    'sweep_rf',
    'sweep_areal',
    'sweep_vertical',
    'sweep_combined',
    'sweep_combined_mobile_oil',
];

export type RateChartScalePreset = 'rates' | 'cumulative' | 'cumulative_volumes' | 'diagnostics' | 'breakthrough' | 'pressure' | 'gor' | 'recovery' | 'sweep' | 'sweep_rf';

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
    visible?: boolean;
    expanded?: boolean;
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