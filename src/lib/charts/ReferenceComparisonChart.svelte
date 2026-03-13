<script lang="ts">
    import ChartSubPanel from './ChartSubPanel.svelte';
    import OutputSummaryStrip from './OutputSummaryStrip.svelte';
    import ToggleGroup from '../ui/controls/ToggleGroup.svelte';
    import type { BenchmarkFamily } from '../catalog/benchmarkCases';
    import type { BenchmarkRunResult } from '../benchmarkRunModel';
    import {
        coerceChartAxisState,
        getConfiguredXAxisOptions,
        resolveChartPanelDefinition,
        type ChartPanelEntry,
        type ChartXAxisOption,
    } from './chartPanelSelection';
    import type {
        RateChartLayoutConfig,
        RateChartPanelKey,
        RateChartScalePreset,
        RateChartXAxisMode,
    } from './rateChartLayoutConfig';
    import { buildReferenceComparisonSummaryItems } from './outputSummary';
    import {
        buildReferenceComparisonModel,
        getReferenceComparisonCaseColor,
    } from './referenceComparisonModel';

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
    let visibleCaseKeys = $state<Record<string, boolean>>({});
    let caseSelectorSignature = $state('');

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
        buildReferenceComparisonModel({
            family,
            results,
            xAxisMode,
            theme,
        }),
    );
    const visibleResults = $derived.by(() => {
        return overlayModel.orderedResults.filter((result) => visibleCaseKeys[result.key] ?? true);
    });

    $effect(() => {
        const orderedCaseKeys = overlayModel.orderedResults.map((result) => result.key);
        const nextSignature = orderedCaseKeys.join('|');
        if (caseSelectorSignature === nextSignature) return;

        const previousVisibility = visibleCaseKeys;
        caseSelectorSignature = nextSignature;
        visibleCaseKeys = Object.fromEntries(
            orderedCaseKeys.map((key) => [key, previousVisibility[key] ?? true]),
        );
    });

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

    const allXAxisOptions = $derived<ChartXAxisOption[]>([
        { value: 'time', label: 'Time' },
        { value: 'tD', label: 'tD', title: 'Dimensionless Time (t/τ)' },
        { value: 'pvi', label: 'PVI', title: 'PV Injected' },
        { value: 'pvp', label: 'PVP', title: 'PV Produced' },
        { value: 'cumLiquid', label: 'Cum Liq', title: 'Cumulative Liquid' },
        { value: 'cumInjection', label: 'Cum Inj', title: 'Cumulative Injection' },
        { value: 'logTime', label: 'Log Time', title: 'Log Time (Fetkovich)' },
    ]);

    const xAxisOptions = $derived.by(() => {
        return getConfiguredXAxisOptions(
            allXAxisOptions,
            layoutConfig?.rateChart?.xAxisOptions,
        );
    });

    $effect(() => {
        const nextAxisState = coerceChartAxisState({
            xAxisMode,
            xAxisOptions,
            logScale,
            allowLogScale: layoutConfig?.rateChart?.allowLogScale,
        });

        if (nextAxisState.xAxisMode !== xAxisMode) xAxisMode = nextAxisState.xAxisMode;
        if (nextAxisState.logScale !== logScale) logScale = nextAxisState.logScale;
    });

    function buildPanelEntries(panelKey: RateChartPanelKey): Array<ChartPanelEntry<(typeof overlayModel.panels)[RateChartPanelKey]['curves'][number], (typeof overlayModel.panels)[RateChartPanelKey]['series'][number]>> {
        return overlayModel.panels[panelKey].curves
            .map((curve, idx) => ({
                curve,
                series: overlayModel.panels[panelKey].series[idx] ?? [],
            }))
            .filter((entry) => !entry.curve.caseKey || (visibleCaseKeys[entry.curve.caseKey] ?? true));
    }

    function toggleCaseVisibility(resultKey: string) {
        visibleCaseKeys = {
            ...visibleCaseKeys,
            [resultKey]: !(visibleCaseKeys[resultKey] ?? true),
        };
    }

    function resolvePanelDefinition(panelKey: RateChartPanelKey, fallback: {
        title: string;
        curveKeys?: string[];
        scalePreset: RateChartScalePreset;
        allowLogToggle?: boolean;
    }) {
        return resolveChartPanelDefinition({
            override: layoutConfig?.rateChart?.panels?.[panelKey],
            fallback,
            entries: buildPanelEntries(panelKey),
            getScalePresetConfig,
        });
    }

    const ratesPanel = $derived(resolvePanelDefinition('rates', {
        title: family?.scenarioClass === 'buckley-leverett' ? 'Breakthrough' : 'Oil Rate',
        curveKeys: family?.scenarioClass === 'buckley-leverett'
            ? ['water-cut-sim', 'water-cut-reference', 'avg-water-sat']
            : ['oil-rate-sim', 'oil-rate-reference'],
        scalePreset: family?.scenarioClass === 'buckley-leverett' ? 'breakthrough' : 'rates',
        allowLogToggle: family?.scenarioClass === 'depletion',
    }));
    const cumulativePanel = $derived(resolvePanelDefinition('cumulative', {
        title: family?.scenarioClass === 'buckley-leverett' ? 'Recovery' : 'Cumulative Oil / Recovery',
        curveKeys: family?.scenarioClass === 'buckley-leverett'
            ? ['recovery-factor', 'cum-oil-sim', 'cum-oil-reference', 'cum-injection']
            : ['recovery-factor', 'cum-oil-sim', 'cum-oil-reference'],
        scalePreset: 'cumulative',
    }));
    const diagnosticsPanel = $derived(resolvePanelDefinition('diagnostics', {
        title: family?.scenarioClass === 'buckley-leverett' ? 'Pressure' : 'Pressure / Decline',
        curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
        scalePreset: 'pressure',
    }));
    const summaryItems = $derived.by(() => (
        buildReferenceComparisonSummaryItems({
            family,
            results: visibleResults,
        })
    ));
</script>

<div class="flex flex-col">
    <div
        class="flex flex-col gap-3 border-b border-border/50 px-4 pb-2 pt-4 md:px-5 md:pt-5"
    >
        <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="ui-section-kicker opacity-50">
                Output Comparison
            </div>
            <div class="ui-support-copy">
                {visibleResults.length} of {overlayModel.orderedResults.length} stored run(s) shown
            </div>
        </div>
        {#if overlayModel.orderedResults.length > 1}
            <div class="ui-support-copy">
                Charts keep their own case selectors. Run Table selection updates the profile and 3D outputs.
            </div>
        {/if}
        <OutputSummaryStrip items={summaryItems} />
        {#if overlayModel.orderedResults.length > 1}
            <div class="flex items-center gap-2 overflow-x-auto">
                <span class="ui-section-kicker shrink-0 opacity-50">Cases</span>
                {#each overlayModel.orderedResults as result, index}
                    <button
                        type="button"
                        class={`flex items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] font-medium transition-colors ${(visibleCaseKeys[result.key] ?? true)
                            ? 'border-primary/40 bg-muted/25 text-foreground'
                            : 'border-border/70 bg-transparent text-muted-foreground opacity-60 hover:opacity-90'}`}
                        onclick={() => toggleCaseVisibility(result.key)}
                        title={`${(visibleCaseKeys[result.key] ?? true) ? 'Hide' : 'Show'} ${result.label}`}
                    >
                        <svg width="14" height="3" class="overflow-visible shrink-0" viewBox="0 0 14 3">
                            <line
                                x1="0"
                                y1="1.5"
                                x2="14"
                                y2="1.5"
                                stroke={getReferenceComparisonCaseColor(index)}
                                stroke-width={result.variantKey === null ? 2.8 : 2.2}
                            />
                        </svg>
                        <span>{result.label}</span>
                    </button>
                {/each}
            </div>
        {/if}
        <div class="flex items-center gap-2 overflow-x-auto">
            <span class="ui-section-kicker shrink-0 opacity-50">X-axis</span>
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
        panelId="comparison-rates"
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
        panelId="comparison-cumulative"
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
        panelId="comparison-diagnostics"
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