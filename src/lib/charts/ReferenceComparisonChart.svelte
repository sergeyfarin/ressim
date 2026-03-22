<script lang="ts">
    import ChartSubPanel from './ChartSubPanel.svelte';
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
    import { resolveSharedXAxisRange, type AxisMapping } from './xAxisRangePolicy';
    import type {
        RateChartLayoutConfig,
        RateChartPanelKey,
        RateChartScalePreset,
        RateChartXAxisMode,
    } from './rateChartLayoutConfig';
    import {
        buildReferenceComparisonModel,
        getReferenceComparisonCaseColor,
        type AnalyticalPreviewVariant,
        type ReferenceComparisonPreviewCase,
    } from './referenceComparisonModel';

    let {
        results = [],
        family = null,
        layoutConfig = {},
        theme = 'dark',
        analyticalPerVariant = false,
        previewVariantParams = undefined,
        pendingPreviewVariants = undefined,
        previewBaseParams = undefined,
        previewAnalyticalMethod = undefined,
    }: {
        results?: BenchmarkRunResult[];
        family?: BenchmarkFamily | null;
        layoutConfig?: RateChartLayoutConfig;
        theme?: 'dark' | 'light';
        analyticalPerVariant?: boolean;
        /** Per-variant preview curves shown before any runs complete (analyticalPerVariant=true). */
        previewVariantParams?: AnalyticalPreviewVariant[];
        /**
         * Analytical-only overlays for variants still queued/running (results not
         * yet in `results`). Keeps the chart from collapsing back to N=1 curves
         * while a sweep is in progress.
         */
        pendingPreviewVariants?: AnalyticalPreviewVariant[];
        /** Single-curve fallback preview (analyticalPerVariant=false). */
        previewBaseParams?: Record<string, any>;
        previewAnalyticalMethod?: string;
    } = $props();

    let xAxisMode = $state<RateChartXAxisMode>('time');
    let logScale = $state(false);
    let ratesExpanded = $state(true);
    let recoveryExpanded = $state(true);
    let cumulativeExpanded = $state(false);
    let diagnosticsExpanded = $state(false);
    let volumesExpanded = $state(false);
    let oilRateExpanded = $state(false);
    let sweepRfExpanded = $state(false);
    let sweepArealExpanded = $state(true);
    let sweepVerticalExpanded = $state(true);
    let sweepCombinedExpanded = $state(true);
    let visibleCaseKeys = $state<Record<string, boolean>>({});
    let caseSelectorSignature = $state('');
    const MAX_RECOMMENDED_VISIBLE_CASES = 20;

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
        if (config.recoveryExpanded !== undefined) recoveryExpanded = config.recoveryExpanded;
        if (config.cumulativeExpanded !== undefined) cumulativeExpanded = config.cumulativeExpanded;
        if (config.diagnosticsExpanded !== undefined) diagnosticsExpanded = config.diagnosticsExpanded;
        if (config.volumesExpanded !== undefined) volumesExpanded = config.volumesExpanded;
        if (config.oilRateExpanded !== undefined) oilRateExpanded = config.oilRateExpanded;
    });

    const isPreviewMode = $derived(
        results.length === 0 &&
        (Boolean(previewBaseParams) || (previewVariantParams?.length ?? 0) > 0),
    );

    const showPerCaseAnalyticalIndicator = $derived(
        analyticalPerVariant || family?.showSweepPanel === true,
    );


    $effect(() => {
        if (isPreviewMode && (previewAnalyticalMethod === 'buckley-leverett' || previewAnalyticalMethod === 'waterflood' || previewAnalyticalMethod === 'gas-oil-bl')) {
            xAxisMode = 'pvi';
        }
    });

    const isGasContext = $derived(
        family?.analyticalMethod === 'gas-oil-bl' || 
        family?.key === 'gas_drive' || 
        family?.key === 'gas_injection' ||
        (results[0]?.params?.injectedFluid === 'gas') ||
        (results[0]?.params?.initialGasSaturation > 0) ||
        (previewBaseParams?.injectedFluid === 'gas') ||
        (previewBaseParams?.initialGasSaturation > 0)
    );

    $effect(() => {
        if (isGasContext && layoutConfig?.rateChart?.diagnosticsExpanded === undefined) {
            diagnosticsExpanded = true;
        }
    });

    const overlayModel = $derived(
        buildReferenceComparisonModel({
            family,
            results,
            xAxisMode,
            theme,
            analyticalPerVariant,
            previewVariantParams,
            pendingPreviewVariants,
            previewBaseParams,
            previewAnalyticalMethod,
        }),
    );
    const visibleResults = $derived.by(() => {
        return overlayModel.orderedResults.filter((result) => visibleCaseKeys[result.key] ?? true);
    });
    const caseVolumeWarning = $derived.by(() => {
        if (visibleResults.length <= MAX_RECOMMENDED_VISIBLE_CASES) return null;
        return `Showing ${visibleResults.length} runs. Charts are designed to stay readable up to ${MAX_RECOMMENDED_VISIBLE_CASES}; above that, overlap and scale compression increase.`;
    });

    $effect(() => {
        // Track both completed results and pending/preview variant keys so toggling
        // works throughout the full lifecycle: pure preview → mid-sweep → completed.
        const resultKeys = overlayModel.orderedResults.map((r) => r.key);
        const previewKeys = overlayModel.previewCases.map((c) => c.key);
        const allKeys = [...resultKeys, ...previewKeys];
        const nextSignature = allKeys.join('|');
        if (caseSelectorSignature === nextSignature) return;

        const previousVisibility = visibleCaseKeys;
        caseSelectorSignature = nextSignature;
        visibleCaseKeys = Object.fromEntries(
            allKeys.map((key) => [key, previousVisibility[key] ?? true]),
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
    const recoveryScales = {
        y: {
            type: 'linear',
            display: true,
            position: 'left',
            min: 0,
            alignToPixels: true,
            title: { display: true, text: 'Recovery Factor' },
            ticks: { count: 6 },
            _fraction: true,
            _maxCap: 1,
        },
    };
    const cumulativeVolumesScales = {
        y: {
            type: 'linear',
            display: true,
            position: 'left',
            min: 0,
            alignToPixels: true,
            title: { display: true, text: 'Cumulative (m³)' },
            ticks: { count: 6 },
        },
    };
    const sweepScales = {
        y: {
            type: 'linear',
            display: true,
            position: 'left',
            min: 0,
            max: 1,
            alignToPixels: true,
            title: { display: true, text: 'Sweep Efficiency' },
            ticks: { count: 6 },
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
        if (scalePreset === 'recovery') return recoveryScales;
        if (scalePreset === 'cumulative_volumes') return cumulativeVolumesScales;
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

    function compactCaseLabel(label: string): string {
        const emDash = label.indexOf(' — ');
        if (emDash !== -1) return label.slice(emDash + 3).trim();
        const hyphen = label.indexOf(' - ');
        if (hyphen !== -1) return label.slice(hyphen + 3).trim();
        return label;
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
        title: family?.analyticalMethod === 'buckley-leverett' ? 'Breakthrough'
            : family?.analyticalMethod === 'gas-oil-bl' ? 'Gas Breakthrough'
            : 'Oil Rate',
        curveKeys: family?.analyticalMethod === 'buckley-leverett'
            ? ['water-cut-sim', 'water-cut-reference']
            : family?.analyticalMethod === 'gas-oil-bl'
            ? ['gas-cut-sim', 'gas-cut-reference']
            : ['oil-rate-sim', 'oil-rate-reference'],
        scalePreset: (family?.analyticalMethod === 'buckley-leverett' || family?.analyticalMethod === 'gas-oil-bl') ? 'breakthrough' : 'rates',
        allowLogToggle: family?.analyticalMethod === 'depletion',
    }));
    const recoveryPanel = $derived(resolvePanelDefinition('recovery', {
        title: 'Recovery Factor',
        curveKeys: ['recovery-factor'],
        scalePreset: 'recovery',
    }));
    const cumulativePanel = $derived(resolvePanelDefinition('cumulative', {
        title: 'Cum Oil',
        curveKeys: family?.analyticalMethod === 'buckley-leverett'
            ? ['cum-oil-sim', 'cum-oil-reference', 'cum-injection']
            : ['cum-oil-sim', 'cum-oil-reference'],
        scalePreset: 'cumulative_volumes',
    }));
    const diagnosticsPanel = $derived(resolvePanelDefinition('diagnostics', {
        title: isGasContext ? 'Material Balance (P/z)' : 'Pressure',
        curveKeys: isGasContext ? ['p_z_sim', 'p_z_reference'] : ['avg-pressure-sim', 'avg-pressure-reference'],
        scalePreset: 'pressure',
    }));
    const volumesPanel = $derived(resolvePanelDefinition('volumes', {
        title: 'Cum Injection',
        curveKeys: ['cum-injection'],
        scalePreset: 'cumulative_volumes',
    }));
    const oilRatePanel = $derived(resolvePanelDefinition('oil_rate', {
        title: 'Oil Rate',
        curveKeys: ['oil-rate-sim'],
        scalePreset: 'rates',
    }));
    function getVisibleSweepPanelEntries(panel: { curves: typeof overlayModel.panels.rates.curves; series: typeof overlayModel.panels.rates.series } | null) {
        if (!panel) return [];
        return panel.curves
            .map((curve, idx) => ({ curve, series: panel.series[idx] ?? [] }))
            .filter((entry) => !entry.curve.caseKey || (visibleCaseKeys[entry.curve.caseKey] ?? true));
    }

    const sweepRfEntries = $derived.by(() => getVisibleSweepPanelEntries(overlayModel.sweepPanels.rf));
    const sweepRfCurves = $derived(sweepRfEntries.map((entry) => entry.curve));
    const sweepRfSeries = $derived(sweepRfEntries.map((entry) => entry.series));

    const sweepArealEntries = $derived.by(() => getVisibleSweepPanelEntries(overlayModel.sweepPanels.areal));
    const sweepArealCurves = $derived(sweepArealEntries.map((entry) => entry.curve));
    const sweepArealSeries = $derived(sweepArealEntries.map((entry) => entry.series));

    const sweepVerticalEntries = $derived.by(() => getVisibleSweepPanelEntries(overlayModel.sweepPanels.vertical));
    const sweepVerticalCurves = $derived(sweepVerticalEntries.map((entry) => entry.curve));
    const sweepVerticalSeries = $derived(sweepVerticalEntries.map((entry) => entry.series));

    const sweepCombinedEntries = $derived.by(() => getVisibleSweepPanelEntries(overlayModel.sweepPanels.combined));
    const sweepCombinedCurves = $derived(sweepCombinedEntries.map((entry) => entry.curve));
    const sweepCombinedSeries = $derived(sweepCombinedEntries.map((entry) => entry.series));

    function toFiniteNumber(value: unknown, fallback: number): number {
        const numeric = Number(value);
        return Number.isFinite(numeric) ? numeric : fallback;
    }

    function getPoreVolume(params: Record<string, any>): number {
        return toFiniteNumber(params.nx, 1)
            * toFiniteNumber(params.ny, 1)
            * toFiniteNumber(params.nz, 1)
            * toFiniteNumber(params.cellDx, 10)
            * toFiniteNumber(params.cellDy, 10)
            * toFiniteNumber(params.cellDz, 1)
            * toFiniteNumber(params.reservoirPorosity ?? params.porosity, 0.2);
    }

    function buildComparisonXAxisValues(result: BenchmarkRunResult, axisMode: RateChartXAxisMode): Array<number | null> {
        const time = result.rateHistory.map((point) => toFiniteNumber(point.time, 0));
        if (axisMode === 'pvi') return [...result.pviSeries];
        if (axisMode === 'logTime') return time.map((value) => (value > 0 ? Math.log10(value) : null));
        if (axisMode === 'time' || axisMode === 'tD') return time;

        let cumulativeInjection = 0;
        let cumulativeLiquid = 0;
        let cumulativeGas = 0;
        const cumulativeInjectionSeries: Array<number | null> = [];
        const cumulativeLiquidSeries: Array<number | null> = [];
        const cumulativeGasSeries: Array<number | null> = [];

        for (let index = 0; index < result.rateHistory.length; index += 1) {
            const point = result.rateHistory[index];
            const dt = index > 0
                ? Math.max(0, toFiniteNumber(point.time, 0) - toFiniteNumber(result.rateHistory[index - 1]?.time, 0))
                : Math.max(0, toFiniteNumber(point.time, 0));
            cumulativeInjection += Math.max(0, toFiniteNumber(point.total_injection, 0)) * dt;
            cumulativeLiquid += Math.max(0, Math.abs(toFiniteNumber(point.total_production_liquid, 0))) * dt;
            cumulativeGas += Math.max(0, Math.abs(toFiniteNumber(point.total_production_gas, 0))) * dt;
            cumulativeInjectionSeries.push(cumulativeInjection);
            cumulativeLiquidSeries.push(cumulativeLiquid);
            cumulativeGasSeries.push(cumulativeGas);
        }

        if (axisMode === 'cumInjection') return cumulativeInjectionSeries;
        if (axisMode === 'cumLiquid') return cumulativeLiquidSeries;
        if (axisMode === 'cumGas') return cumulativeGasSeries;
        if (axisMode === 'pvp') {
            const poreVolume = getPoreVolume(result.params);
            return cumulativeLiquidSeries.map((value) => (
                poreVolume > 1e-12 && Number.isFinite(value) ? Number(value) / poreVolume : null
            ));
        }

        return time;
    }

    const visiblePviMappings = $derived.by((): AxisMapping[] => {
        return visibleResults.map((result) => ({
            domainValues: [...result.pviSeries],
            rangeValues: buildComparisonXAxisValues(result, xAxisMode),
        }));
    });

    const sharedXRange = $derived.by(() => {
        return resolveSharedXAxisRange({
            allSeries: [
                ...ratesPanel.series,
                ...recoveryPanel.series,
                ...cumulativePanel.series,
                ...diagnosticsPanel.series,
                ...volumesPanel.series,
                ...oilRatePanel.series,
                ...sweepRfSeries,
                ...sweepArealSeries,
                ...sweepVerticalSeries,
                ...sweepCombinedSeries,
            ],
            rateSeries: ratesPanel.series,
            xAxisMode,
            policy: layoutConfig?.rateChart?.xAxisRangePolicy,
            pviMappings: visiblePviMappings,
        });
    });
</script>

<div class="flex flex-col">
    <div
        class="flex flex-col gap-3 border-b border-border/50 px-4 pb-2 pt-4 md:px-5 md:pt-5"
    >
        <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="ui-section-kicker">
                Comparison Plots
            </div>
            {#if overlayModel.previewCases.length > 0 && overlayModel.orderedResults.length === 0}
                <div class="ui-support-copy text-muted-foreground/70">
                    Analytical preview — {overlayModel.previewCases.length} variant(s)
                </div>
            {/if}
        </div>
        {#if caseVolumeWarning}
            <div class="rounded-md border border-amber-300/70 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-700/60 dark:bg-amber-950/30 dark:text-amber-200">
                {caseVolumeWarning}
            </div>
        {/if}
        {#if overlayModel.orderedResults.length + overlayModel.previewCases.length > 1}
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
                        {#if showPerCaseAnalyticalIndicator}
                            <!-- Dual indicator: dashed = analytical, solid = simulation -->
                            <svg width="14" height="9" class="overflow-visible shrink-0" viewBox="0 0 14 9">
                                <line x1="0" y1="2" x2="14" y2="2"
                                    stroke={getReferenceComparisonCaseColor(index)}
                                    stroke-width="1.4" stroke-dasharray="5,3" />
                                <line x1="0" y1="7" x2="14" y2="7"
                                    stroke={getReferenceComparisonCaseColor(index)}
                                    stroke-width={result.variantKey === null ? 2.0 : 1.6} />
                            </svg>
                        {:else}
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
                        {/if}
                        <span title={result.label}>{compactCaseLabel(result.label)}</span>
                    </button>
                {/each}
                {#each overlayModel.previewCases as pc}
                    <button
                        type="button"
                        class={`flex items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] font-medium transition-colors ${(visibleCaseKeys[pc.key] ?? true)
                            ? 'border-primary/40 bg-muted/25 text-foreground'
                            : 'border-border/70 bg-transparent text-muted-foreground opacity-60 hover:opacity-90'}`}
                        onclick={() => toggleCaseVisibility(pc.key)}
                        title={`${(visibleCaseKeys[pc.key] ?? true) ? 'Hide' : 'Show'} ${pc.label} (analytical preview)`}
                    >
                        <svg width="14" height="3" class="overflow-visible shrink-0" viewBox="0 0 14 3">
                            <line
                                x1="0"
                                y1="1.5"
                                x2="14"
                                y2="1.5"
                                stroke={getReferenceComparisonCaseColor(pc.colorIndex)}
                                stroke-width="2"
                                stroke-dasharray="7,4"
                            />
                        </svg>
                        <span title={pc.label}>{compactCaseLabel(pc.label)}</span>
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
            {#if overlayModel.axisMappingWarning}
                <span class="text-[11px] text-muted-foreground">
                    {overlayModel.axisMappingWarning}
                </span>
            {/if}
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
        xRange={sharedXRange}
        targetLeftGutter={maxLeftGutter}
        targetRightGutter={maxRightGutter}
        onGutterMeasure={(left: number, right: number) => {
            nativeGutters = { ...nativeGutters, rates: { left, right } };
        }}
    />

    <ChartSubPanel
        panelId="comparison-recovery"
        title={recoveryPanel.title}
        bind:expanded={recoveryExpanded}
        curves={recoveryPanel.curves}
        seriesData={recoveryPanel.series}
        scaleConfigs={recoveryPanel.scales}
        {theme}
        logScale={false}
        xRange={sharedXRange}
        targetLeftGutter={maxLeftGutter}
        targetRightGutter={maxRightGutter}
        onGutterMeasure={(left: number, right: number) => {
            nativeGutters = { ...nativeGutters, recovery: { left, right } };
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
        xRange={sharedXRange}
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
        xRange={sharedXRange}
        targetLeftGutter={maxLeftGutter}
        targetRightGutter={maxRightGutter}
        onGutterMeasure={(left: number, right: number) => {
            nativeGutters = { ...nativeGutters, diagnostics: { left, right } };
        }}
    />

    {#if volumesPanel.curves.length > 0}
        <ChartSubPanel
            panelId="comparison-volumes"
            title={volumesPanel.title}
            bind:expanded={volumesExpanded}
            curves={volumesPanel.curves}
            seriesData={volumesPanel.series}
            scaleConfigs={volumesPanel.scales}
            {theme}
            logScale={false}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            onGutterMeasure={(left: number, right: number) => {
                nativeGutters = { ...nativeGutters, volumes: { left, right } };
            }}
        />
    {/if}

    {#if oilRatePanel.curves.length > 0}
        <ChartSubPanel
            panelId="comparison-oil-rate"
            title={oilRatePanel.title}
            bind:expanded={oilRateExpanded}
            curves={oilRatePanel.curves}
            seriesData={oilRatePanel.series}
            scaleConfigs={oilRatePanel.scales}
            {theme}
            logScale={false}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            onGutterMeasure={(left: number, right: number) => {
                nativeGutters = { ...nativeGutters, oil_rate: { left, right } };
            }}
        />
    {/if}

    {#if family?.showSweepPanel && sweepRfCurves.length > 0}
        <ChartSubPanel
            panelId="comparison-sweep-rf"
            title="Sweep Recovery Factor"
            bind:expanded={sweepRfExpanded}
            curves={sweepRfCurves}
            seriesData={sweepRfSeries}
            scaleConfigs={sweepScales}
            {theme}
            logScale={false}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            onGutterMeasure={(left: number, right: number) => {
                nativeGutters = { ...nativeGutters, sweep_rf: { left, right } };
            }}
        />
    {/if}

    {#if family?.showSweepPanel && sweepArealCurves.length > 0}
        <ChartSubPanel
            panelId="comparison-sweep-areal"
            title="Areal Sweep Efficiency"
            bind:expanded={sweepArealExpanded}
            curves={sweepArealCurves}
            seriesData={sweepArealSeries}
            scaleConfigs={sweepScales}
            {theme}
            logScale={false}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            onGutterMeasure={(left: number, right: number) => {
                nativeGutters = { ...nativeGutters, sweep_areal: { left, right } };
            }}
        />
    {/if}

    {#if family?.showSweepPanel && sweepVerticalCurves.length > 0}
        <ChartSubPanel
            panelId="comparison-sweep-vertical"
            title="Vertical Sweep Efficiency"
            bind:expanded={sweepVerticalExpanded}
            curves={sweepVerticalCurves}
            seriesData={sweepVerticalSeries}
            scaleConfigs={sweepScales}
            {theme}
            logScale={false}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            onGutterMeasure={(left: number, right: number) => {
                nativeGutters = { ...nativeGutters, sweep_vertical: { left, right } };
            }}
        />
    {/if}

    {#if family?.showSweepPanel && sweepCombinedCurves.length > 0}
        <ChartSubPanel
            panelId="comparison-sweep-combined"
            title="Volumetric Sweep Efficiency"
            bind:expanded={sweepCombinedExpanded}
            curves={sweepCombinedCurves}
            seriesData={sweepCombinedSeries}
            scaleConfigs={sweepScales}
            {theme}
            logScale={false}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            onGutterMeasure={(left: number, right: number) => {
                nativeGutters = { ...nativeGutters, sweep_combined: { left, right } };
            }}
        />
    {/if}
</div>