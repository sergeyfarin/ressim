<script lang="ts">
    import { untrack } from "svelte";
    import ChartSubPanel from "./ChartSubPanel.svelte";
    import type { CurveConfig } from "./chartTypes";
    import {
        coerceChartAxisState,
        getConfiguredXAxisOptions,
        resolveChartPanelDefinition,
        resolveChartPanelLayout,
        type ChartPanelDefinition,
        type ChartPanelEntry,
        type ChartXAxisOption,
    } from "./chartPanelSelection";
    import type {
        RateChartLayoutConfig,
        RateChartPanelId,
        RateChartXAxisMode,
    } from "./rateChartLayoutConfig";
    import { DEFAULT_RATE_CHART_PANEL_ORDER } from "./rateChartLayoutConfig";
    import type {
        RateHistoryPoint,
        AnalyticalProductionPoint,
    } from "../simulator-types";
    import ToggleGroup from "../ui/controls/ToggleGroup.svelte";
    import type { SweepAnalyticalMethod, SweepGeometry } from "../analytical/sweepEfficiency";
    import type { RockProps, FluidProps } from "../analytical/fractionalFlow";
    import { resolveSharedXAxisRange } from "./xAxisRangePolicy";
    import {
        buildRateChartData,
        buildGetScalePresetConfig,
        type XYPoint,
    } from "./buildRateChartData";

    let {
        rateHistory = [],
        analyticalProductionData = [],
        avgReservoirPressureSeries = [],
        avgWaterSaturationSeries = [],
        ooipM3 = 0,
        poreVolumeM3 = 0,
        activeMode = "",
        activeCase = "",
        theme = "dark",
        analyticalMeta,
        layoutConfig,
        rockProps,
        fluidProps,
        layerPermeabilities = [],
        layerThickness = 1,
        showSweepPanel = false,
        sweepGeometry = 'both',
        sweepAnalyticalMethod = 'dykstra-parsons',
        sweepEfficiencySimSeries = null,
        sweepRFAnalytical = null,
    }: {
        rateHistory?: RateHistoryPoint[];
        analyticalProductionData?: AnalyticalProductionPoint[];
        avgReservoirPressureSeries?: Array<number | null>;
        avgWaterSaturationSeries?: Array<number | null>;
        ooipM3?: number;
        poreVolumeM3?: number;
        activeMode?: string;
        activeCase?: string;
        theme?: "dark" | "light";
        analyticalMeta?: any;
        layoutConfig?: RateChartLayoutConfig;
        rockProps?: RockProps;
        fluidProps?: FluidProps;
        layerPermeabilities?: number[];
        layerThickness?: number;
        showSweepPanel?: boolean;
        sweepGeometry?: SweepGeometry;
        sweepAnalyticalMethod?: SweepAnalyticalMethod;
        sweepEfficiencySimSeries?: Array<{ time: number; eA: number | null; eV: number | null; eVol: number; mobileOilRecovered: number | null }> | null;
        sweepRFAnalytical?: import("../analytical/sweepEfficiency").SweepRFResult | null;
    } = $props();

    type PanelDefinition = ChartPanelDefinition<CurveConfig, XYPoint[]>;

    // --- X-axis state (shared across all panels) ---
    let xAxisMode = $state<RateChartXAxisMode>("time");
    let logScale = $state(false);
    let normalizeRates = $state(false);

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

    // --- Panel expand/collapse state ---
    let panelExpanded = $state<Record<RateChartPanelId, boolean>>(createDefaultPanelExpandedState());

    // --- Panel alignment state ---
    let nativeGutters = $state<Record<string, { left: number; right: number }>>({});
    let maxLeftGutter = $derived(
        Math.max(0, ...Object.values(nativeGutters).map((g) => g.left)),
    );
    let maxRightGutter = $derived(
        Math.max(0, ...Object.values(nativeGutters).map((g) => g.right)),
    );

    // --- Live chart data (pure computation) ---
    let liveData = $derived.by(() => buildRateChartData({
        rateHistory,
        analyticalProductionData,
        avgReservoirPressureSeries,
        avgWaterSaturationSeries,
        ooipM3,
        poreVolumeM3,
        theme,
        xAxisMode,
        normalizeRates,
        analyticalMeta,
        showSweepPanel,
        sweepGeometry,
        sweepAnalyticalMethod,
        sweepEfficiencySimSeries: sweepEfficiencySimSeries ?? null,
        sweepRFAnalytical: sweepRFAnalytical ?? null,
        rockProps,
        fluidProps,
        layerPermeabilities,
        layerThickness,
    }));

    let pviAvailable = $derived(liveData.pviAvailable);
    let pvpAvailable = $derived(liveData.pvpAvailable);
    let mismatchSummary = $derived(liveData.mismatchSummary);
    let getScalePresetConfig = $derived(buildGetScalePresetConfig(normalizeRates));

    // --- Scenario-aware panel defaults ---
    $effect(() => {
        const cat = (activeMode ?? "").toLowerCase();
        const cs = (activeCase ?? "").toLowerCase();

        const currentExpanded = untrack(() => panelExpanded);
        const nextExpanded = { ...currentExpanded };
        const conf = layoutConfig?.rateChart;
        if (conf) {
            if (conf.logScale !== undefined) logScale = conf.logScale;
            if (conf.xAxisMode !== undefined) xAxisMode = conf.xAxisMode;
            for (const panelKey of conf.panelOrder ?? DEFAULT_RATE_CHART_PANEL_ORDER) {
                const expanded = conf.panels?.[panelKey]?.expanded;
                if (expanded !== undefined) nextExpanded[panelKey] = expanded;
            }
        } else {
            if (cat === "dep" || cat === "depletion" || cs.includes("depletion")) {
                nextExpanded.rates = true;
                nextExpanded.cumulative = false;
                nextExpanded.diagnostics = true;
            } else if (
                cat === "wf" ||
                cat === "waterflood" ||
                cs.startsWith("wf_") ||
                cs.includes("waterflood") ||
                cs.startsWith("bl_")
            ) {
                nextExpanded.rates = true;
                nextExpanded.cumulative = true;
                nextExpanded.diagnostics = false;
            }
            if (
                cat === "wf" ||
                cs.startsWith("wf_") ||
                cs.startsWith("bl_") ||
                cs === "waterflood_custom_subcase"
            ) {
                xAxisMode = pviAvailable ? "pvi" : "time";
            } else {
                xAxisMode = "time";
                logScale = false;
            }
        }

        if (!equalPanelExpandedState(currentExpanded, nextExpanded)) {
            panelExpanded = nextExpanded;
        }
    });

    function setXAxisMode(mode: RateChartXAxisMode) {
        if (mode === "pvi" && !pviAvailable) return;
        if (mode === "pvp" && !pvpAvailable) return;
        if (mode === "tD" && (!analyticalMeta?.tau || analyticalMeta.tau <= 0))
            return;
        xAxisMode = mode;
    }

    function applyCurveLayout(defaultCurves: CurveConfig[]): CurveConfig[] {
        const customCurves = layoutConfig?.rateChart?.curves;
        if (!customCurves) return defaultCurves;
        return defaultCurves.map((c) => {
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

    let allXAxisOptions = $derived<ChartXAxisOption[]>([
        { value: "time", label: "Time" },
        {
            value: "tD",
            label: "tD",
            disabled: !analyticalMeta?.tau || analyticalMeta.tau <= 0,
            title: "Dimensionless Time (t/τ)",
        },
        { value: "pvi", label: "PVI", disabled: !pviAvailable, title: "PV Injected" },
        { value: "pvp", label: "PVP", disabled: !pvpAvailable, title: "PV Produced" },
        { value: "cumLiquid", label: "Cum Liq", title: "Cumulative Liquid" },
        { value: "cumInjection", label: "Cum Inj", title: "Cumulative Injection" },
        { value: "logTime", label: "Log Time", title: "Log Time (Fetkovich)" },
    ]);

    function buildPanelDefinition(
        panelKey: RateChartPanelId,
        entries: Array<ChartPanelEntry<CurveConfig, XYPoint[]>>,
    ): PanelDefinition {
        const panelDefinition = resolveChartPanelDefinition({
            override: layoutConfig?.rateChart?.panels?.[panelKey],
            fallback: liveData.panelFallbacks[panelKey],
            entries,
            getScalePresetConfig,
        });
        return {
            ...panelDefinition,
            curves: applyCurveLayout(panelDefinition.curves),
        };
    }

    function toPanelEntries(
        curves: CurveConfig[],
        series: XYPoint[][],
    ): Array<ChartPanelEntry<CurveConfig, XYPoint[]>> {
        return curves.map((curve, index) => ({
            curve,
            series: series[index] ?? [],
        }));
    }

    const panelEntriesByKey = $derived.by((): Record<RateChartPanelId, Array<ChartPanelEntry<CurveConfig, XYPoint[]>>> => ({
        rates: liveData.curveRegistry,
        recovery: liveData.curveRegistry,
        cumulative: liveData.curveRegistry,
        diagnostics: liveData.curveRegistry,
        gor: liveData.curveRegistry,
        volumes: liveData.curveRegistry,
        oil_rate: liveData.curveRegistry,
        injection_rate: liveData.curveRegistry,
        producer_bhp: liveData.curveRegistry,
        injector_bhp: liveData.curveRegistry,
        control_limits: liveData.curveRegistry,
        sweep_rf: liveData.sweepPanels ? toPanelEntries(liveData.sweepPanels.rfCurves, liveData.sweepPanels.rfSeries) : [],
        sweep_areal: liveData.sweepPanels ? toPanelEntries(liveData.sweepPanels.arealCurves, liveData.sweepPanels.arealSeries) : [],
        sweep_vertical: liveData.sweepPanels ? toPanelEntries(liveData.sweepPanels.verticalCurves, liveData.sweepPanels.verticalSeries) : [],
        sweep_combined: liveData.sweepPanels ? toPanelEntries(liveData.sweepPanels.volCurves, liveData.sweepPanels.volSeries) : [],
        sweep_combined_mobile_oil: liveData.sweepPanels ? toPanelEntries(liveData.sweepPanels.mobileOilCurves, liveData.sweepPanels.mobileOilSeries) : [],
    }));

    const resolvedPanels = $derived.by(() => {
        const panelOrder = layoutConfig?.rateChart?.panelOrder ?? DEFAULT_RATE_CHART_PANEL_ORDER;

        return panelOrder
            .map((panelKey) => {
                const panelLayout = resolveChartPanelLayout({
                    override: layoutConfig?.rateChart?.panels?.[panelKey],
                    fallback: liveData.panelFallbacks[panelKey],
                });
                const panelDefinition = buildPanelDefinition(panelKey, panelEntriesByKey[panelKey]);

                return {
                    key: panelKey,
                    chartId: panelKey.replaceAll('_', '-'),
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

    const ratePanelSupportsNormalization = $derived(
        (resolvedPanels.find((panel) => panel.key === 'rates')?.curves ?? []).some((curve) => curve.label.includes("Rate")),
    );

    const showsPrimaryAnalyticalCurves = $derived.by(() =>
        resolvedPanels
            .filter((panel) => !panel.key.startsWith('sweep_'))
            .flatMap((panel) => panel.curves)
            .some((curve) => (curve.curveKey ?? "").includes("-reference")),
    );

    const sharedXRange = $derived.by(() =>
        resolveSharedXAxisRange({
            allSeries: resolvedPanels.flatMap((panel) => panel.series),
            rateSeries: resolvedPanels.find((panel) => panel.key === 'rates')?.series ?? [],
            xAxisMode,
            policy: layoutConfig?.rateChart?.xAxisRangePolicy,
            pviMappings: [{ domainValues: liveData.pviValues, rangeValues: liveData.xValues }],
        }),
    );

    let xAxisOptions = $derived(
        getConfiguredXAxisOptions(allXAxisOptions, layoutConfig?.rateChart?.xAxisOptions),
    );

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
</script>

<div class="flex flex-col">
    <div
        class="flex flex-col gap-3 border-b border-border/50 px-4 pb-2 pt-4 md:px-5 md:pt-5"
    >
        <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
            <div class="flex items-center gap-2 overflow-x-auto">
                <span
                    class="ui-section-kicker opacity-50 shrink-0"
                    >X-axis</span
                >
                <ToggleGroup
                    options={xAxisOptions}
                    bind:value={xAxisMode}
                    onChange={(val) => setXAxisMode(val as RateChartXAxisMode)}
                />
            </div>

            <div class="flex items-center gap-2 overflow-x-auto sm:ml-4">
                {#if ratePanelSupportsNormalization && analyticalMeta?.q0 && analyticalMeta.q0 > 0}
                    <span
                        class="ui-section-kicker opacity-50 shrink-0"
                        >Y-axis</span
                    >
                    <label
                        class="flex items-center gap-1.5 cursor-pointer select-none"
                    >
                        <input
                            type="checkbox"
                            bind:checked={normalizeRates}
                            class="rounded border-border text-primary focus:ring-primary h-3.5 w-3.5"
                        />
                        <span
                            class="ui-support-copy whitespace-nowrap"
                            >Normalize Rates (q/q₀)</span
                        >
                    </label>
                {/if}
            </div>
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

    <!-- Error stats -->
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
