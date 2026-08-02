export type ChartXAxisMode =
    | 'time'
    | 'tD'
    | 'logTime'
    | 'pvi'
    | 'pvp'
    | 'cumLiquid'
    | 'cumInjection'
    | 'cumGas';

/**
 * The panels the builder emits today.
 *
 * A *documented set*, not a closed type: `ChartPanelId` accepts any string,
 * so a panel can be introduced beside the code that fills it instead of by
 * editing this union first. The literals are kept because they still buy
 * autocomplete and catch typos in the ~20 places that name a panel directly —
 * and because the order below is the default render order.
 *
 * Step 3 of `docs/CHART_ARCHITECTURE_REVIEW_2026-08-02.md`. The panel names
 * themselves are reservoir vocabulary and stay so until panels carry their own
 * descriptors; what changed is that the *type* no longer forces a shared edit.
 */
export const KNOWN_PRIMARY_PANEL_IDS = [
    'rates', 'recovery', 'cumulative', 'diagnostics', 'avg_water_sat', 'mbe_ooip',
    'drive_indices', 'pz', 'pss_drawdown', 'pss_productivity', 'pss_shape_factor',
    'gor', 'volumes', 'oil_rate', 'gas_rate', 'injection_rate', 'producer_bhp', 'injector_bhp',
    'control_limits', 'cumulative_gas',
] as const;

export const KNOWN_SWEEP_PANEL_IDS = [
    'sweep_rf', 'sweep_areal', 'sweep_vertical', 'sweep_combined', 'sweep_combined_mobile_oil',
] as const;

export type KnownPrimaryPanelId = typeof KNOWN_PRIMARY_PANEL_IDS[number];
export type KnownSweepPanelId = typeof KNOWN_SWEEP_PANEL_IDS[number];

// `(string & {})` keeps editor completion for the known ids while accepting any
// other string — the standard TypeScript idiom for an open enumeration.
export type ChartPrimaryPanelId = KnownPrimaryPanelId | (string & {});
export type ChartSweepPanelId = KnownSweepPanelId | (string & {});

export type ChartPanelId = ChartPrimaryPanelId | ChartSweepPanelId;

/**
 * Keys of the builder's *current* panel map.
 *
 * Deliberately the closed known set while `buildChartData` still constructs one
 * record with a slot per panel. `ChartPanelId` above is the open type — what
 * a layout or a scenario may name. The two converge when panels carry their own
 * descriptors and the fixed record goes away (step 4 of the review).
 */
export type ChartPanelKey = KnownPrimaryPanelId;
export type ChartAuxiliaryPanelKey = KnownSweepPanelId;

export const DEFAULT_CHART_PANEL_ORDER: ChartPanelId[] = [
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
    'gas_rate',
    'injection_rate',
    'producer_bhp',
    'injector_bhp',
    'control_limits',
    'cumulative_gas',
    'sweep_rf',
    'sweep_areal',
    'sweep_vertical',
    'sweep_combined',
    'sweep_combined_mobile_oil',
];

export type ChartScalePreset = 'rates' | 'cumulative' | 'cumulative_volumes' | 'diagnostics' | 'breakthrough' | 'pressure' | 'productivity' | 'shape_factor' | 'ratio' | 'gor' | 'recovery' | 'sweep' | 'sweep_rf' | 'fraction' | 'p_over_z';

export type ChartXAxisRangePolicy =
    | { mode: 'data-extent' }
    | { mode: 'rate-tail-threshold'; relativeThreshold?: number }
    | { mode: 'pvi-window'; minPvi?: number; maxPvi: number };

export type ChartCurveOverride = {
    disabled?: boolean;
    visible?: boolean;
};

export type ChartPanelLayout = {
    title?: string;
    curveKeys?: string[];
    curveLabels?: string[];
    scalePreset?: ChartScalePreset;
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

export type ChartConfig = {
    logScale?: boolean;
    allowLogScale?: boolean;
    xAxisMode?: ChartXAxisMode;
    xAxisOptions?: ChartXAxisMode[];
    xAxisRangePolicy?: ChartXAxisRangePolicy;
    ratesExpanded?: boolean;
    recoveryExpanded?: boolean;
    cumulativeExpanded?: boolean;
    diagnosticsExpanded?: boolean;
    volumesExpanded?: boolean;
    oilRateExpanded?: boolean;
    panelOrder?: ChartPanelId[];
    panels?: Partial<Record<ChartPanelId, ChartPanelLayout>>;
    curves?: Record<string, ChartCurveOverride>;
};

export type ChartLayoutConfig = {
    chart?: ChartConfig;
};
