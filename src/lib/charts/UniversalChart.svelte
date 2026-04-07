<!--
    UniversalChart.svelte — scenario- and domain-agnostic chart renderer.

    This component knows nothing about oil, water, sweeps, or simulation data.
    It owns only chart UI state (x-axis mode, log scale, normalization, panel expansion)
    and delegates all data computation to the caller via buildCurveContext().

    Data flow (caller-owned):
      raw data → buildCurveContext(xAxisMode, normalizeRates) → LiveCurveContext
                                                              ↓
    UniversalChart:  panelDefs × ctx → buildUniversalChartData → datasets → ChartSubPanel[]
-->
<script lang="ts">
    import ChartSubPanel from "./ChartSubPanel.svelte";
    import type { CurveConfig } from "./chartTypes";
    import {
        coerceChartAxisState,
        getConfiguredXAxisOptions,
        resolveChartPanelDefinition,
        resolveChartPanelLayout,
        type ChartXAxisOption,
    } from "./chartPanelSelection";
    import type {
        RateChartLayoutConfig,
        RateChartXAxisMode,
    } from "./rateChartLayoutConfig";
    import ToggleGroup from "../ui/controls/ToggleGroup.svelte";
    import { resolveSharedXAxisRange } from "./xAxisRangePolicy";
    import { buildGetScalePresetConfig } from "./buildRateChartData";
    import { buildUniversalChartData } from "./buildUniversalChartData";
    import { PANEL_DEFS } from "./panelDefs";
    import type { UniversalPanelDef, LiveCurveContext } from "./universalChartTypes";

    let {
        panelDefs = [],
        buildCurveContext,
        xAxisOptions = [],
        supportsNormalization = false,
        defaultXAxisMode,
        defaultPanelExpanded,
        theme = "dark",
        layoutConfig,
    }: {
        panelDefs?: UniversalPanelDef[];
        /** Called whenever xAxisMode or normalizeRates changes. Returns the full curve context. */
        buildCurveContext: (xAxisMode: RateChartXAxisMode, normalizeRates: boolean) => LiveCurveContext;
        /** Pre-computed x-axis options (with disabled flags) — provided by the adapter. */
        xAxisOptions?: ChartXAxisOption[];
        /** Whether to show the normalize rates toggle. */
        supportsNormalization?: boolean;
        /** When this changes, UniversalChart resets its x-axis mode to match. */
        defaultXAxisMode?: RateChartXAxisMode;
        /** When this changes, UniversalChart resets panel expansion to match. */
        defaultPanelExpanded?: Record<string, boolean>;
        theme?: "dark" | "light";
        layoutConfig?: RateChartLayoutConfig;
    } = $props();

    // ── Chart UI state ────────────────────────────────────────────────────────
    let xAxisMode = $state<RateChartXAxisMode>("time");
    let logScale = $state(false);
    let normalizeRates = $state(false);

    // ── Panel expand / collapse state ─────────────────────────────────────────
    function initPanelExpanded(): Record<string, boolean> {
        return Object.fromEntries(
            Object.entries(PANEL_DEFS).map(([k, def]) => [k, def.expanded ?? false]),
        );
    }
    let panelExpanded = $state<Record<string, boolean>>(initPanelExpanded());

    // Initialize any panel keys from panelDefs not in PANEL_DEFS.
    $effect(() => {
        const missing: Record<string, boolean> = {};
        for (const d of panelDefs) {
            if (!(d.panelKey in panelExpanded)) missing[d.panelKey] = false;
        }
        if (Object.keys(missing).length > 0) {
            panelExpanded = { ...panelExpanded, ...missing };
        }
    });

    // ── Sync defaults from adapter (fires when scenario / layout changes) ─────
    $effect(() => {
        if (defaultXAxisMode !== undefined) xAxisMode = defaultXAxisMode;
    });

    $effect(() => {
        if (defaultPanelExpanded && Object.keys(defaultPanelExpanded).length > 0) {
            panelExpanded = { ...panelExpanded, ...defaultPanelExpanded };
        }
    });

    // ── Panel gutter alignment ────────────────────────────────────────────────
    let nativeGutters = $state<Record<string, { left: number; right: number }>>({});
    let maxLeftGutter = $derived(
        Math.max(0, ...Object.values(nativeGutters).map((g) => g.left)),
    );
    let maxRightGutter = $derived(
        Math.max(0, ...Object.values(nativeGutters).map((g) => g.right)),
    );

    // ── Build chart data via the provided context factory ─────────────────────
    let ctx = $derived(buildCurveContext(xAxisMode, normalizeRates));
    let liveData = $derived(buildUniversalChartData(panelDefs, ctx));
    let mismatchSummary = $derived(liveData.mismatchSummary);
    let getScalePresetConfig = $derived(buildGetScalePresetConfig(normalizeRates));

    // ── Curve layout overrides from layoutConfig ──────────────────────────────
    function applyCurveLayout(curves: CurveConfig[]): CurveConfig[] {
        const customCurves = layoutConfig?.rateChart?.curves;
        if (!customCurves) return curves;
        return curves.map((c) => {
            const override = customCurves[c.label];
            if (!override) return c;
            return {
                ...c,
                defaultVisible:
                    override.visible !== undefined ? override.visible : c.defaultVisible,
                disabled: override.disabled,
            };
        });
    }

    // ── Resolved panel list ───────────────────────────────────────────────────
    let resolvedPanels = $derived.by(() => {
        return liveData.datasets
            .map((ds) => {
                const panelKey = ds.panelKey;
                const fallback = PANEL_DEFS[panelKey] ?? {
                    title: panelKey,
                    scalePreset: 'rates' as const,
                    visible: true,
                    expanded: false,
                };

                const panelLayout = resolveChartPanelLayout({
                    override: layoutConfig?.rateChart?.panels?.[panelKey],
                    fallback,
                });

                const entries = ds.curves.map((curve, i) => ({
                    curve,
                    series: ds.series[i] ?? [],
                }));

                const panelDefinition = resolveChartPanelDefinition({
                    override: layoutConfig?.rateChart?.panels?.[panelKey],
                    fallback,
                    entries,
                    getScalePresetConfig,
                });

                return {
                    key: panelKey,
                    chartId: panelKey.replaceAll("_", "-"),
                    title: panelDefinition.title,
                    curves: applyCurveLayout(panelDefinition.curves),
                    series: panelDefinition.series,
                    scales: panelDefinition.scales,
                    allowLogToggle:
                        panelDefinition.allowLogToggle || panelLayout.allowLogToggle,
                    visible: panelLayout.visible,
                };
            })
            .filter((panel) => panel.visible && panel.curves.length > 0);
    });

    // ── Derived UI flags ──────────────────────────────────────────────────────
    let showsPrimaryAnalyticalCurves = $derived(
        resolvedPanels.some((p) =>
            p.curves.some(
                (c) =>
                    c.legendSection === "Analytical (dashed lines):" ||
                    c.legendSection === "Published reference (dotted lines):",
            ),
        ),
    );

    let sharedXRange = $derived.by(() =>
        resolveSharedXAxisRange({
            allSeries: resolvedPanels.flatMap((p) => p.series),
            rateSeries: resolvedPanels.flatMap((p) => p.series),
            xAxisMode,
            policy: layoutConfig?.rateChart?.xAxisRangePolicy,
            pviMappings: [{ domainValues: liveData.pviArr, rangeValues: liveData.xValues }],
        }),
    );

    let configuredXAxisOptions = $derived(
        getConfiguredXAxisOptions(xAxisOptions, layoutConfig?.rateChart?.xAxisOptions),
    );

    function setXAxisMode(mode: RateChartXAxisMode) {
        const opt = xAxisOptions.find((o) => o.value === mode);
        if (opt?.disabled) return;
        xAxisMode = mode;
    }

    // ── Coerce axis state when options change ─────────────────────────────────
    $effect(() => {
        const next = coerceChartAxisState({
            xAxisMode,
            xAxisOptions: configuredXAxisOptions,
            logScale,
            allowLogScale: layoutConfig?.rateChart?.allowLogScale,
        });
        if (next.xAxisMode !== xAxisMode) xAxisMode = next.xAxisMode;
        if (next.logScale !== logScale) logScale = next.logScale;
    });
</script>

<div class="flex flex-col">
    <div
        class="flex flex-col gap-3 border-b border-border/50 px-4 pb-2 pt-4 md:px-5 md:pt-5"
    >
        <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
            {#if configuredXAxisOptions.length > 0}
                <div class="flex items-center gap-2 overflow-x-auto">
                    <span class="ui-section-kicker opacity-50 shrink-0">X-axis</span>
                    <ToggleGroup
                        options={configuredXAxisOptions}
                        bind:value={xAxisMode}
                        onChange={(val) => setXAxisMode(val as RateChartXAxisMode)}
                    />
                </div>
            {/if}

            {#if supportsNormalization}
                <div class="flex items-center gap-2 overflow-x-auto sm:ml-4">
                    <span class="ui-section-kicker opacity-50 shrink-0">Y-axis</span>
                    <label class="flex items-center gap-1.5 cursor-pointer select-none">
                        <input
                            type="checkbox"
                            bind:checked={normalizeRates}
                            class="rounded border-border text-primary focus:ring-primary h-3.5 w-3.5"
                        />
                        <span class="ui-support-copy whitespace-nowrap">
                            Normalize Rates (q/q₀)
                        </span>
                    </label>
                </div>
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

    {#if showsPrimaryAnalyticalCurves && mismatchSummary.pointsCompared > 0}
        <div class="ui-support-copy px-4 pb-4 pt-2 opacity-60 md:px-5 md:pb-5">
            Reference Solution: {mismatchSummary.pointsCompared} pts · MAE: {mismatchSummary.mae.toFixed(
                3,
            )} · RMSE: {mismatchSummary.rmse.toFixed(3)} · MAPE: {mismatchSummary.mape.toFixed(
                2,
            )}%
        </div>
    {/if}
</div>
