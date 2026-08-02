<script lang="ts">
    import { untrack } from 'svelte';
    import ChartSubPanel from './ChartSubPanel.svelte';
    import ToggleGroup from '../ui/controls/ToggleGroup.svelte';
    import type { BenchmarkFamily } from '../scenario/referenceTypes';
    import type { BenchmarkRunResult } from '../benchmarkRunModel';
    import {
        coerceChartAxisState,
        getConfiguredXAxisOptions,
        resolveChartPanelDefinition,
        resolveChartPanelLayout,
        suppressLeadingOutliers,
        type ChartPanelEntry,
        type ChartPanelFallback,
        type ChartXAxisOption,
    } from './chartPanelSelection';
    import { resolveSharedXAxisRange, type AxisMapping } from './xAxisRangePolicy';
    import { buildXAxisValues } from './axisAdapters';
    import { buildDerivedRunSeries } from './analyticalParamAdapters';
    import type {
        ChartLayoutConfig,
        ChartPanelId,
        ChartScalePreset,
        ChartXAxisMode,
    } from './chartLayoutConfig';
    import { DEFAULT_CHART_PANEL_ORDER } from './chartLayoutConfig';
    import {
        buildReferenceComparisonModel,
        getReferenceComparisonCaseColor,
        type AnalyticalPreviewVariant,
        type ReferenceComparisonPreviewCase,
    } from './buildChartData';
    import {
        SCALE_CUMULATIVE_VOLUMES,
        SCALE_CUMULATIVE,
        SCALE_PRESSURE,
        SCALE_PRODUCTIVITY,
        SCALE_SHAPE_FACTOR,
        SCALE_RATIO,
        SCALE_GOR,
        SCALE_FRACTION,
        SCALE_SWEEP,
    } from './scalePresetRegistry';
    import { PANEL_DEFS, getPanelFallback } from './panelDefs';
    import { getAnalyticalMethodDescriptor } from './analyticalMethodRegistry';
    import type { AnalyticalMethod } from '../catalog/scenarios';
    import { resolveHistoryDivider } from './historyDivider';
    import type { HistoryWindow } from '../catalog/scenarios';

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
        historyWindow = null,
    }: {
        results?: BenchmarkRunResult[];
        family?: BenchmarkFamily | null;
        layoutConfig?: ChartLayoutConfig;
        theme?: 'dark' | 'light';
        analyticalPerVariant?: boolean;
        /** Optional history/forecast divider marker (scenario-declared). */
        historyWindow?: HistoryWindow | null;
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
        previewAnalyticalMethod?: AnalyticalMethod;
    } = $props();

    function createDefaultPanelExpandedState(): Record<ChartPanelId, boolean> {
        return Object.fromEntries(
            DEFAULT_CHART_PANEL_ORDER.map((panelKey) => [panelKey, false]),
        ) as Record<ChartPanelId, boolean>;
    }

    function createDefaultPanelLogScaleState(): Record<ChartPanelId, boolean> {
        return Object.fromEntries(
            DEFAULT_CHART_PANEL_ORDER.map((panelKey) => [panelKey, false]),
        ) as Record<ChartPanelId, boolean>;
    }

    function equalPanelExpandedState(
        left: Record<ChartPanelId, boolean>,
        right: Record<ChartPanelId, boolean>,
    ): boolean {
        return DEFAULT_CHART_PANEL_ORDER.every((panelKey) => left[panelKey] === right[panelKey]);
    }

    let xAxisMode = $state<ChartXAxisMode>('time');
    const resolvedHistoryDivider = $derived(resolveHistoryDivider(historyWindow, xAxisMode));
    // Log scaling is per panel: panels on one chart carry different properties
    // over different dynamic ranges, so one shared axis type cannot suit them.
    let panelLogScale = $state<Record<ChartPanelId, boolean>>(createDefaultPanelLogScaleState());
    let panelExpanded = $state<Record<ChartPanelId, boolean>>(createDefaultPanelExpandedState());
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

    function setNativeGutter(panelKey: string, left: number, right: number) {
        const current = nativeGutters[panelKey];
        if (
            current &&
            Math.abs(current.left - left) < 0.5 &&
            Math.abs(current.right - right) < 0.5
        ) {
            return;
        }
        nativeGutters = { ...nativeGutters, [panelKey]: { left, right } };
    }

    $effect(() => {
        const config = layoutConfig?.chart;
        if (!config) return;
        if (config.xAxisMode !== undefined) xAxisMode = config.xAxisMode;
        const currentExpanded = untrack(() => panelExpanded);
        const nextExpanded = { ...currentExpanded };
        const currentLogScale = untrack(() => panelLogScale);
        const nextLogScale = { ...currentLogScale };
        const panelOrder = config.panelOrder ?? DEFAULT_CHART_PANEL_ORDER;
        for (const panelKey of panelOrder) {
            const expanded = config.panels?.[panelKey]?.expanded;
            if (expanded !== undefined) nextExpanded[panelKey] = expanded;
            // Panel default wins; the chart-level flag seeds panels that do
            // not state their own, preserving each layout's prior behaviour.
            const panelLog = config.panels?.[panelKey]?.logScale ?? config.logScale;
            if (panelLog !== undefined) {
                nextLogScale[panelKey] = config.allowLogScale === false ? false : panelLog;
            }
        }
        if (!equalPanelExpandedState(currentExpanded, nextExpanded)) {
            panelExpanded = nextExpanded;
        }
        if (DEFAULT_CHART_PANEL_ORDER.some((key) => currentLogScale[key] !== nextLogScale[key])) {
            panelLogScale = nextLogScale;
        }
    });

    const isPreviewMode = $derived(
        results.length === 0 &&
        (Boolean(previewBaseParams) || (previewVariantParams?.length ?? 0) > 0),
    );

    const activeDescriptor = $derived(getAnalyticalMethodDescriptor(family?.analyticalMethod));

    const showPerCaseAnalyticalIndicator = $derived(
        analyticalPerVariant || activeDescriptor.producesSweepPanels,
    );


    $effect(() => {
        // A PVI-native solution previews on its own axis until runs exist to
        // remap it; the registry owns which methods those are.
        if (isPreviewMode && getAnalyticalMethodDescriptor(previewAnalyticalMethod).nativeXAxis === 'pvi') {
            xAxisMode = 'pvi';
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

    function getScalePresetConfig(scalePreset: ChartScalePreset): Record<string, any> {
        if (scalePreset === 'sweep') return SCALE_SWEEP;
        if (scalePreset === 'sweep_rf') return SCALE_SWEEP;
        if (scalePreset === 'breakthrough') return breakthroughScales;
        if (scalePreset === 'pressure') return SCALE_PRESSURE;
        if (scalePreset === 'productivity') return SCALE_PRODUCTIVITY;
        if (scalePreset === 'shape_factor') return SCALE_SHAPE_FACTOR;
        if (scalePreset === 'ratio') return SCALE_RATIO;
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
            layoutConfig?.chart?.xAxisOptions,
        );
    });

    $effect(() => {
        const nextAxisState = coerceChartAxisState({
            xAxisMode,
            xAxisOptions,
            logScale: false,
            allowLogScale: layoutConfig?.chart?.allowLogScale,
        });

        if (nextAxisState.xAxisMode !== xAxisMode) xAxisMode = nextAxisState.xAxisMode;
    });

    function buildPanelEntries(panelKey: ChartPanelId): Array<ChartPanelEntry<NonNullable<(typeof overlayModel.panels)[ChartPanelId]>['curves'][number], NonNullable<(typeof overlayModel.panels)[ChartPanelId]>['series'][number]>> {
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

    // Panel presentation comes from the active analytical method's descriptor
    // (analyticalMethodRegistry.ts) layered over PANEL_DEFS. Scenario-specific
    // titles and curve choices are applied later from layoutConfig.
    const panelFallbacks = $derived.by((): Record<string, ChartPanelFallback | undefined> => {
        const presentation = activeDescriptor.panelPresentation;
        return {
            ...PANEL_DEFS,
            rates: {
                ...PANEL_DEFS.rates,
                ...presentation.rates,
            },
            cumulative: {
                ...PANEL_DEFS.cumulative,
                ...presentation.cumulative,
            },
            diagnostics: {
                ...PANEL_DEFS.diagnostics,
                ...presentation.diagnostics,
                scalePreset: 'pressure',
            },
        };
    });

    const resolvedPanels = $derived.by(() => {
        const panelOrder = layoutConfig?.chart?.panelOrder ?? DEFAULT_CHART_PANEL_ORDER;

        return panelOrder
            .map((panelKey) => {
                const fallback = panelFallbacks[panelKey] ?? getPanelFallback(panelKey);
                const panelLayout = resolveChartPanelLayout({
                    override: layoutConfig?.chart?.panels?.[panelKey],
                    fallback,
                });
                const panelDefinition = resolveChartPanelDefinition({
                    override: layoutConfig?.chart?.panels?.[panelKey],
                    fallback,
                    entries: buildPanelEntries(panelKey),
                    getScalePresetConfig,
                });

                return {
                    key: panelKey,
                    chartId: `comparison-${panelKey.replaceAll('_', '-')}`,
                    title: panelDefinition.title,
                    curves: panelDefinition.curves,
                    series: panelDefinition.series.map((series, index) => (
                        (panelDefinition.curves[index]?.referenceSourceType === 'simulation'
                            || panelDefinition.curves[index]?.curveKey?.endsWith('-sim'))
                            ? suppressLeadingOutliers(
                                series,
                                layoutConfig?.chart?.panels?.[panelKey]?.suppressLeadingOutliers,
                            )
                            : series
                    )),
                    scales: panelDefinition.scales,
                    allowLogToggle: panelDefinition.allowLogToggle || panelLayout.allowLogToggle,
                    visible: panelLayout.visible,
                    expanded: panelExpanded[panelKey] ?? panelLayout.expanded,
                };
            })
            .filter((panel) => panel.visible && panel.curves.length > 0);
    });

    /**
     * X-axis values for one run, from the shared derivation.
     *
     * This used to be a local re-implementation: its own `toFiniteNumber`, its
     * own cumulative-production integration, and its own `getPoreVolume` that —
     * unlike the shared one — ignored `cellDzPerLayer`, so the `pvp` axis would
     * have used a different pore volume than every other consumer. Latent only
     * because no layout currently offers that axis. One derivation now, in
     * `buildDerivedRunSeries` + `buildXAxisValues`.
     */
    function buildComparisonXAxisValues(
        result: BenchmarkRunResult,
        axisMode: ChartXAxisMode,
    ): Array<number | null> {
        return buildXAxisValues(buildDerivedRunSeries(result), axisMode);
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
            policy: layoutConfig?.chart?.xAxisRangePolicy,
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
                    xAxisMode = value as ChartXAxisMode;
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
            bind:logScale={panelLogScale[panel.key]}
            allowLogToggle={layoutConfig?.chart?.allowLogScale ?? panel.allowLogToggle}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            historyDivider={resolvedHistoryDivider}
            onGutterMeasure={(left: number, right: number) => {
                setNativeGutter(panel.key, left, right);
            }}
        />
    {/each}
</div>
