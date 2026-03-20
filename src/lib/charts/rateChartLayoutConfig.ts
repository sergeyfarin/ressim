export type RateChartXAxisMode =
    | 'time'
    | 'tD'
    | 'logTime'
    | 'pvi'
    | 'pvp'
    | 'cumLiquid'
    | 'cumInjection';

export type RateChartPanelKey = 'rates' | 'recovery' | 'cumulative' | 'diagnostics' | 'volumes' | 'oil_rate';

export type RateChartScalePreset = 'rates' | 'cumulative' | 'cumulative_volumes' | 'diagnostics' | 'breakthrough' | 'pressure' | 'recovery';

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
};

export type RateChartConfig = {
    logScale?: boolean;
    allowLogScale?: boolean;
    xAxisMode?: RateChartXAxisMode;
    xAxisOptions?: RateChartXAxisMode[];
    ratesExpanded?: boolean;
    recoveryExpanded?: boolean;
    cumulativeExpanded?: boolean;
    diagnosticsExpanded?: boolean;
    volumesExpanded?: boolean;
    oilRateExpanded?: boolean;
    panels?: Partial<Record<RateChartPanelKey, RateChartPanelLayout>>;
    curves?: Record<string, RateChartCurveOverride>;
};

export type RateChartLayoutConfig = {
    rateChart?: RateChartConfig;
};