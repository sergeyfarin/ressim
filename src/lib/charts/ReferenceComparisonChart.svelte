<script lang="ts">
    import { untrack } from 'svelte';
    import ChartSubPanel from './ChartSubPanel.svelte';
    import ToggleGroup from '../ui/controls/ToggleGroup.svelte';
    import type { BenchmarkFamily } from '../catalog/benchmarkCases';
    import type { BenchmarkRunResult } from '../benchmarkRunModel';
    import {
        coerceChartAxisState,
        getConfiguredXAxisOptions,
        resolveChartPanelDefinition,
        resolveChartPanelLayout,
        type ChartPanelEntry,
        type ChartPanelFallback,
        type ChartXAxisOption,
    } from './chartPanelSelection';
    import { resolveSharedXAxisRange, type AxisMapping } from './xAxisRangePolicy';
    import type {
        RateChartLayoutConfig,
        RateChartPanelId,
        RateChartScalePreset,
        RateChartXAxisMode,
    } from './rateChartLayoutConfig';
    import { DEFAULT_RATE_CHART_PANEL_ORDER } from './rateChartLayoutConfig';
    import {
        buildReferenceComparisonModel,
        getReferenceComparisonCaseColor,
        type AnalyticalPreviewVariant,
        type ReferenceComparisonPreviewCase,
    } from './referenceComparisonModel';
    import {
        SCALE_CUMULATIVE_VOLUMES,
        SCALE_CUMULATIVE,
        SCALE_PRESSURE,
        SCALE_GOR,
        SCALE_FRACTION,
        SCALE_SWEEP,
    } from './scalePresetRegistry';
    import { PANEL_DEFS } from './panelDefs';

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

    function createDefaultPanelExpandedState(): Record<RateChartPanelId, boolean> {
        return Object.fromEntries(
            DEFAULT_RATE_CHART_PANEL_ORDER.map((panelKey) => [panelKey, false]),
        ) as Record<RateChartPanelId, boolean>;
    }

    function equalPanelExpandedState(
        left: Record<RateChartPanelId, boolean>,
        right: Record<RateChartPanelId, boolean>,
    ): boolean {
        return DEFAULT_RATE_CHART_PANEL_ORDER.every((panelKey) => left[panelKey] === right[panelKey]);
    }

    let xAxisMode = $state<RateChartXAxisMode>('time');
    let logScale = $state(false);
    let panelExpanded = $state<Record<RateChartPanelId, boolean>>(createDefaultPanelExpandedState());
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
        const currentExpanded = untrack(() => panelExpanded);
        const nextExpanded = { ...currentExpanded };
        const panelOrder = config.panelOrder ?? DEFAULT_RATE_CHART_PANEL_ORDER;
        for (const panelKey of panelOrder) {
            const expanded = config.panels?.[panelKey]?.expanded;
            if (expanded !== undefined) nextExpanded[panelKey] = expanded;
        }
        if (!equalPanelExpandedState(currentExpanded, nextExpanded)) {
            panelExpanded = nextExpanded;
        }
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
        if (isGasContext && layoutConfig?.rateChart?.panels?.diagnostics?.expanded === undefined) {
            const currentExpanded = untrack(() => panelExpanded);
            if (!currentExpanded.diagnostics) {
                panelExpanded = { ...currentExpanded, diagnostics: true };
            }
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
    const diagnosticsScales = {
        y: {
            type: 'linear',
            display: true,
            position: 'left',
            alignToPixels: true,
            title: { display: true, text: 'Pressure (bar)' },
            ticks: { count: 6 },
            _auto: true,
        },
        y1: {
            type: 'linear',
            display: true,
            position: 'right',
            min: 0,
            max: 1,
            alignToPixels: true,
            title: { display: true, text: 'BHP-limited fraction' },
            grid: { drawOnChartArea: false },
            ticks: { count: 6 },
            _fraction: true,
        },
    };

    function getScalePresetConfig(scalePreset: RateChartScalePreset): Record<string, any> {
        if (scalePreset === 'sweep') return SCALE_SWEEP;
        if (scalePreset === 'sweep_rf') return SCALE_SWEEP;
        if (scalePreset === 'breakthrough') return breakthroughScales;
        if (scalePreset === 'pressure') return SCALE_PRESSURE;
        if (scalePreset === 'gor') return SCALE_GOR;
        if (scalePreset === 'diagnostics') return diagnosticsScales;
        if (scalePreset === 'fraction') return SCALE_FRACTION;
        if (scalePreset === 'recovery') return recoveryScales;
        if (scalePreset === 'cumulative_volumes') return SCALE_CUMULATIVE_VOLUMES;
        if (scalePreset === 'cumulative') return SCALE_CUMULATIVE;
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

    function buildPanelEntries(panelKey: RateChartPanelId): Array<ChartPanelEntry<NonNullable<(typeof overlayModel.panels)[RateChartPanelId]>['curves'][number], NonNullable<(typeof overlayModel.panels)[RateChartPanelId]>['series'][number]>> {
        const panel = overlayModel.panels[panelKey];
        if (!panel) return [];

        return panel.curves
            .map((curve, idx) => ({
                curve,
                series: panel.series[idx] ?? [],
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

    const panelFallbacks = $derived.by((): Record<RateChartPanelId, ChartPanelFallback> => {
        const isBL = family?.analyticalMethod === 'buckley-leverett';
        const isGasOilBL = family?.analyticalMethod === 'gas-oil-bl';
        const isSweep = family?.showSweepPanel === true;
        return {
            ...PANEL_DEFS,
            rates: {
                ...PANEL_DEFS.rates,
                title: isSweep ? 'Watercut' : isBL ? 'Breakthrough' : isGasOilBL ? 'Gas Breakthrough' : 'Oil Rate',
                curveKeys: isSweep
                    ? ['water-cut-sim']
                    : isBL
                    ? ['water-cut-sim', 'water-cut-reference']
                    : isGasOilBL
                    ? ['gas-cut-sim', 'gas-cut-reference']
                    : ['oil-rate-sim', 'oil-rate-reference'],
                scalePreset: (isBL || isGasOilBL) ? 'breakthrough' : 'rates',
                allowLogToggle: family?.analyticalMethod === 'depletion',
            },
            cumulative: {
                ...PANEL_DEFS.cumulative,
                curveKeys: isSweep
                    ? ['cum-oil-sim']
                    : isBL
                    ? ['cum-oil-sim', 'cum-oil-reference', 'cum-injection']
                    : ['cum-oil-sim', 'cum-oil-reference'],
            },
            diagnostics: {
                ...PANEL_DEFS.diagnostics,
                title: isGasContext ? 'Material Balance (P/z)' : 'Pressure',
                curveKeys: isSweep
                    ? ['avg-pressure-sim']
                    : isGasContext
                    ? ['p_z_sim', 'p_z_reference']
                    : ['avg-pressure-sim', 'avg-pressure-reference'],
                scalePreset: 'pressure',
            },
        };
    });

    const resolvedPanels = $derived.by(() => {
        const panelOrder = layoutConfig?.rateChart?.panelOrder ?? DEFAULT_RATE_CHART_PANEL_ORDER;

        return panelOrder
            .map((panelKey) => {
                const panelLayout = resolveChartPanelLayout({
                    override: layoutConfig?.rateChart?.panels?.[panelKey],
                    fallback: panelFallbacks[panelKey],
                });
                const panelDefinition = resolveChartPanelDefinition({
                    override: layoutConfig?.rateChart?.panels?.[panelKey],
                    fallback: panelFallbacks[panelKey],
                    entries: buildPanelEntries(panelKey),
                    getScalePresetConfig,
                });

                return {
                    key: panelKey,
                    chartId: `comparison-${panelKey.replaceAll('_', '-')}`,
                    title: panelDefinition.title,
                    curves: panelDefinition.curves,
                    series: panelDefinition.series,
                    scales: panelDefinition.scales,
                    allowLogToggle: panelDefinition.allowLogToggle || panelLayout.allowLogToggle,
                    visible: panelLayout.visible,
                    expanded: panelExpanded[panelKey] ?? panelLayout.expanded,
                };
            })
            .filter((panel) => panel.visible && panel.curves.length > 0);
    });

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
            allSeries: resolvedPanels.flatMap((panel) => panel.series),
            rateSeries: resolvedPanels.find((panel) => panel.key === 'rates')?.series ?? [],
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

    {#each resolvedPanels as panel (panel.key)}
        <ChartSubPanel
            panelId={panel.chartId}
            title={panel.title}
            bind:expanded={panelExpanded[panel.key]}
            curves={panel.curves}
            seriesData={panel.series}
            scaleConfigs={panel.scales}
            {theme}
            bind:logScale
            allowLogToggle={layoutConfig?.rateChart?.allowLogScale ?? panel.allowLogToggle}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            onGutterMeasure={(left: number, right: number) => {
                nativeGutters = { ...nativeGutters, [panel.key]: { left, right } };
            }}
        />
    {/each}
</div>