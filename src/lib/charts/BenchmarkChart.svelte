<script lang="ts">
    import ChartSubPanel from './ChartSubPanel.svelte';
    import ToggleGroup from '../ui/controls/ToggleGroup.svelte';
    import type { BenchmarkFamily } from '../catalog/benchmarkCases';
    import type { BenchmarkRunResult } from '../benchmarkRunModel';
    import type {
        RateChartLayoutConfig,
        RateChartPanelKey,
        RateChartScalePreset,
        RateChartXAxisMode,
    } from './rateChartLayoutConfig';
    import { buildBenchmarkComparisonModel } from './benchmarkComparisonModel';

    let {
        results = [],
        family = null,
        layoutConfig = {},
        theme = 'dark',
    }: {
        results?: BenchmarkRunResult[];
        family?: BenchmarkFamily | null;
        layoutConfig?: RateChartLayoutConfig;
        theme?: 'dark' | 'light';
    } = $props();

    let xAxisMode = $state<RateChartXAxisMode>('time');
    let logScale = $state(false);
    let ratesExpanded = $state(true);
    let cumulativeExpanded = $state(true);
    let diagnosticsExpanded = $state(false);

    let nativeGutters = $state<Record<string, { left: number; right: number }>>({});
    let maxLeftGutter = $derived(
        Math.max(0, ...Object.values(nativeGutters).map((g) => g.left)),
    );
    let maxRightGutter = $derived(
        Math.max(0, ...Object.values(nativeGutters).map((g) => g.right)),
    );

    $effect(() => {
        const config = layoutConfig?.rateChart;
        if (!config) return;
        if (config.xAxisMode !== undefined) xAxisMode = config.xAxisMode;
        if (config.logScale !== undefined) logScale = config.logScale;
        if (config.ratesExpanded !== undefined) ratesExpanded = config.ratesExpanded;
        if (config.cumulativeExpanded !== undefined) cumulativeExpanded = config.cumulativeExpanded;
        if (config.diagnosticsExpanded !== undefined) diagnosticsExpanded = config.diagnosticsExpanded;
    });

    const overlayModel = $derived(
        buildBenchmarkComparisonModel({
            family,
            results,
            xAxisMode,
            theme,
        }),
    );

    const breakthroughScales = {
        y: {
            type: 'linear',
            display: true,
            position: 'left',
            min: 0,
            max: 1,
            alignToPixels: true,
            title: { display: true, text: 'Water Cut / Saturation' },
            ticks: { count: 6 },
            _fraction: true,
        },
    };
    const rateScales = {
        y: {
            type: 'linear',
            display: true,
            position: 'left',
            min: 0,
            alignToPixels: true,
            title: { display: true, text: 'Rate (m³/day)' },
            ticks: { count: 6 },
        },
    };
    const cumulativeScales = {
        y: {
            type: 'linear',
            display: true,
            position: 'left',
            min: 0,
            alignToPixels: true,
            title: { display: true, text: 'Cumulative (m³)' },
            ticks: { count: 6 },
        },
        y1: {
            type: 'linear',
            display: true,
            position: 'right',
            min: 0,
            max: 1,
            alignToPixels: true,
            title: { display: true, text: 'Recovery Factor' },
            grid: { drawOnChartArea: false },
            ticks: { count: 6 },
            _fraction: true,
        },
    };
    const pressureScales = {
        y: {
            type: 'linear',
            display: true,
            position: 'left',
            alignToPixels: true,
            title: { display: true, text: 'Pressure (bar)' },
            ticks: { count: 6 },
            _auto: true,
        },
    };

    function getScalePresetConfig(scalePreset: RateChartScalePreset): Record<string, any> {
        if (scalePreset === 'breakthrough') return breakthroughScales;
        if (scalePreset === 'pressure') return pressureScales;
        if (scalePreset === 'cumulative') return cumulativeScales;
        return rateScales;
    }

    const allXAxisOptions = $derived([
        { value: 'time', label: 'Time' },
        { value: 'tD', label: 'tD', title: 'Dimensionless Time (t/τ)' },
        { value: 'pvi', label: 'PVI', title: 'PV Injected' },
        { value: 'pvp', label: 'PVP', title: 'PV Produced' },
        { value: 'cumLiquid', label: 'Cum Liq', title: 'Cumulative Liquid' },
        { value: 'cumInjection', label: 'Cum Inj', title: 'Cumulative Injection' },
        { value: 'logTime', label: 'Log Time', title: 'Log Time (Fetkovich)' },
    ]);

    const xAxisOptions = $derived.by(() => {
        const configured = layoutConfig?.rateChart?.xAxisOptions;
        if (!Array.isArray(configured) || configured.length === 0) return allXAxisOptions;
        const allowed = new Set(configured);
        return allXAxisOptions.filter((option) => allowed.has(option.value as RateChartXAxisMode));
    });

    $effect(() => {
        const allowedModes = xAxisOptions.map((option) => option.value as RateChartXAxisMode);
        if (!allowedModes.includes(xAxisMode) && allowedModes.length > 0) {
            xAxisMode = allowedModes[0];
        }
        if (layoutConfig?.rateChart?.allowLogScale === false && logScale) {
            logScale = false;
        }
    });

    function resolvePanelDefinition(panelKey: RateChartPanelKey, fallback: {
        title: string;
        scalePreset: RateChartScalePreset;
        allowLogToggle?: boolean;
    }) {
        const override = layoutConfig?.rateChart?.panels?.[panelKey];
        return {
            title: override?.title ?? fallback.title,
            scales: getScalePresetConfig(override?.scalePreset ?? fallback.scalePreset),
            allowLogToggle: override?.allowLogToggle ?? fallback.allowLogToggle ?? false,
            curves: overlayModel.panels[panelKey].curves,
            series: overlayModel.panels[panelKey].series,
        };
    }

    const ratesPanel = $derived(resolvePanelDefinition('rates', {
        title: family?.scenarioClass === 'buckley-leverett' ? 'Breakthrough' : 'Oil Rate',
        scalePreset: family?.scenarioClass === 'buckley-leverett' ? 'breakthrough' : 'rates',
        allowLogToggle: family?.scenarioClass === 'depletion',
    }));
    const cumulativePanel = $derived(resolvePanelDefinition('cumulative', {
        title: family?.scenarioClass === 'buckley-leverett' ? 'Recovery' : 'Cumulative Oil / Recovery',
        scalePreset: 'cumulative',
    }));
    const diagnosticsPanel = $derived(resolvePanelDefinition('diagnostics', {
        title: family?.scenarioClass === 'buckley-leverett' ? 'Pressure' : 'Pressure / Decline',
        scalePreset: 'pressure',
    }));
</script>

<div class="flex flex-col">
    <div
        class="flex flex-col gap-2 border-b border-border/50 px-4 pb-2 pt-4 md:px-5 md:pt-5"
    >
        <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="text-[11px] uppercase tracking-wide opacity-50">
                Benchmark Comparison
            </div>
            <div class="text-[11px] text-muted-foreground">
                {overlayModel.orderedResults.length} stored run(s)
            </div>
        </div>
        <div class="flex items-center gap-2 overflow-x-auto">
            <span class="text-[11px] uppercase tracking-wide opacity-50 shrink-0">X-axis</span>
            <ToggleGroup
                options={xAxisOptions}
                bind:value={xAxisMode}
                onChange={(value) => {
                    xAxisMode = value as RateChartXAxisMode;
                }}
            />
        </div>
    </div>

    <ChartSubPanel
        panelId="benchmark-rates"
        title={ratesPanel.title}
        bind:expanded={ratesExpanded}
        curves={ratesPanel.curves}
        seriesData={ratesPanel.series}
        scaleConfigs={ratesPanel.scales}
        {theme}
        bind:logScale
        allowLogToggle={layoutConfig?.rateChart?.allowLogScale ?? ratesPanel.allowLogToggle}
        targetLeftGutter={maxLeftGutter}
        targetRightGutter={maxRightGutter}
        onGutterMeasure={(left: number, right: number) => {
            nativeGutters = { ...nativeGutters, rates: { left, right } };
        }}
    />

    <ChartSubPanel
        panelId="benchmark-cumulative"
        title={cumulativePanel.title}
        bind:expanded={cumulativeExpanded}
        curves={cumulativePanel.curves}
        seriesData={cumulativePanel.series}
        scaleConfigs={cumulativePanel.scales}
        {theme}
        logScale={false}
        targetLeftGutter={maxLeftGutter}
        targetRightGutter={maxRightGutter}
        onGutterMeasure={(left: number, right: number) => {
            nativeGutters = { ...nativeGutters, cumulative: { left, right } };
        }}
    />

    <ChartSubPanel
        panelId="benchmark-diagnostics"
        title={diagnosticsPanel.title}
        bind:expanded={diagnosticsExpanded}
        curves={diagnosticsPanel.curves}
        seriesData={diagnosticsPanel.series}
        scaleConfigs={diagnosticsPanel.scales}
        {theme}
        logScale={false}
        targetLeftGutter={maxLeftGutter}
        targetRightGutter={maxRightGutter}
        onGutterMeasure={(left: number, right: number) => {
            nativeGutters = { ...nativeGutters, diagnostics: { left, right } };
        }}
    />
</div>