export type RateChartXAxisMode =
    | 'time'
    | 'tD'
    | 'logTime'
    | 'pvi'
    | 'pvp'
    | 'cumLiquid'
    | 'cumInjection';

export type RateChartPanelKey = 'rates' | 'cumulative' | 'diagnostics';

export type RateChartScalePreset = 'rates' | 'cumulative' | 'diagnostics' | 'breakthrough' | 'pressure';

export type RateChartCurveOverride = {
    disabled?: boolean;
    visible?: boolean;
};

export type RateChartPanelLayout = {
    title?: string;
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
    cumulativeExpanded?: boolean;
    diagnosticsExpanded?: boolean;
    panels?: Partial<Record<RateChartPanelKey, RateChartPanelLayout>>;
    curves?: Record<string, RateChartCurveOverride>;
};

export type RateChartLayoutConfig = {
    rateChart?: RateChartConfig;
};