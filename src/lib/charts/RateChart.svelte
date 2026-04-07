<!--
    RateChart.svelte — domain adapter for UniversalChart.

    This is the only component that knows about simulation data structures
    (rateHistory, ooipM3, sweep geometry, etc.) and scenario categories.

    Responsibilities:
      - Build LiveDerivedSeries from raw simulation output
      - Build LiveSweepContext when sweep panel is active
      - Create the buildCurveContext closure that UniversalChart calls on demand
      - Compute x-axis options (with pviAvailable / pvpAvailable / tauAvailable flags)
      - Derive scenario-aware defaults (x-axis mode, panel expansion)
      - Pass everything to UniversalChart — which stays fully agnostic
-->
<script lang="ts">
    import UniversalChart from "./UniversalChart.svelte";
    import type { ChartXAxisOption } from "./chartPanelSelection";
    import type {
        RateChartLayoutConfig,
        RateChartXAxisMode,
    } from "./rateChartLayoutConfig";
    import type { RateHistoryPoint, AnalyticalProductionPoint } from "../simulator-types";
    import type { SweepAnalyticalMethod, SweepGeometry } from "../analytical/sweepEfficiency";
    import {
        computeCombinedSweep,
        computeSweepRecoveryFactor,
        getSweepComponentVisibility,
    } from "../analytical/sweepEfficiency";
    import type { RockProps, FluidProps } from "../analytical/fractionalFlow";
    import { buildLiveDerivedSeries, buildXValues } from "./buildLiveDerivedSeries";
    import type { LiveCurveContext, LiveSweepContext, UniversalPanelDef } from "./universalChartTypes";
    import { DEFAULT_RATE_CHART_PANEL_ORDER } from "./rateChartLayoutConfig";

    let {
        panelDefs = [],
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
        panelDefs?: UniversalPanelDef[];
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
        sweepEfficiencySimSeries?: Array<{
            time: number;
            eA: number | null;
            eV: number | null;
            eVol: number;
            mobileOilRecovered: number | null;
        }> | null;
        sweepRFAnalytical?: import("../analytical/sweepEfficiency").SweepRFResult | null;
    } = $props();

    // ── Pre-computed simulation series ────────────────────────────────────────
    let sim = $derived.by(() =>
        buildLiveDerivedSeries(
            rateHistory,
            analyticalProductionData,
            avgReservoirPressureSeries,
            avgWaterSaturationSeries,
            ooipM3,
            poreVolumeM3,
        ),
    );

    // ── Sweep analytical context ──────────────────────────────────────────────
    let sweepCtx = $derived.by((): LiveSweepContext | null => {
        if (!showSweepPanel || !rockProps || !fluidProps) return null;
        const perms = layerPermeabilities.length > 0 ? layerPermeabilities : [100];
        const analytical = computeCombinedSweep(
            rockProps, fluidProps, perms, layerThickness,
            3.0, 200, sweepGeometry, sweepAnalyticalMethod,
        );
        const rfResult =
            sweepRFAnalytical ??
            computeSweepRecoveryFactor(
                rockProps, fluidProps, perms, layerThickness,
                3.0, 200, sweepGeometry, sweepAnalyticalMethod,
            );
        const { showAreal, showVertical } = getSweepComponentVisibility(sweepGeometry);
        return {
            arealSweepCurve: analytical.arealSweep.curve,
            verticalSweepCurve: analytical.verticalSweep.curve,
            combinedSweepCurve: analytical.combined,
            rfResult,
            showAreal,
            showVertical,
        };
    });

    // ── Context factory passed to UniversalChart ──────────────────────────────
    // Re-derives when sim, sweepCtx, or any closed-over prop changes.
    let buildCurveContext = $derived.by(
        () =>
            (xAxisMode: RateChartXAxisMode, normalizeRates: boolean): LiveCurveContext => {
                const xValues = buildXValues(sim, xAxisMode, analyticalMeta);
                const scaleFactor =
                    normalizeRates && analyticalMeta?.q0 && analyticalMeta.q0 > 0
                        ? 1 / (analyticalMeta.q0 as number)
                        : 1;
                const neutralColor = theme === "dark" ? "#f8fafc" : "#0f172a";
                return {
                    sim,
                    analytical: analyticalProductionData,
                    xValues,
                    xAxisMode,
                    pviArr: sim.pvi,
                    scaleFactor,
                    neutralColor,
                    ooipM3,
                    rateHistory,
                    sweep: sweepCtx,
                    sweepSimSeries: sweepEfficiencySimSeries ?? null,
                };
            },
    );

    // ── X-axis options (availability derived from sim data) ───────────────────
    let pviAvailable = $derived((sim.pvi.at(-1) ?? 0) > 1e-12);
    let pvpAvailable = $derived((sim.pvp.at(-1) ?? 0) > 1e-12);
    let tauAvailable = $derived(
        analyticalMeta?.tau != null && analyticalMeta.tau > 0,
    );

    let xAxisOptions = $derived<ChartXAxisOption[]>([
        { value: "time", label: "Time" },
        {
            value: "tD",
            label: "tD",
            disabled: !tauAvailable,
            title: "Dimensionless Time (t/τ)",
        },
        { value: "pvi", label: "PVI", disabled: !pviAvailable, title: "PV Injected" },
        { value: "pvp", label: "PVP", disabled: !pvpAvailable, title: "PV Produced" },
        { value: "cumLiquid", label: "Cum Liq", title: "Cumulative Liquid" },
        { value: "cumInjection", label: "Cum Inj", title: "Cumulative Injection" },
        { value: "logTime", label: "Log Time", title: "Log Time (Fetkovich)" },
    ]);

    // ── Scenario-aware defaults (passed to UniversalChart as reactive props) ──
    let supportsNormalization = $derived(
        !!(analyticalMeta?.q0 && analyticalMeta.q0 > 0),
    );

    let defaultXAxisMode = $derived.by((): RateChartXAxisMode => {
        const conf = layoutConfig?.rateChart;
        if (conf?.xAxisMode !== undefined) return conf.xAxisMode;
        const cs = (activeCase ?? "").toLowerCase();
        const cat = (activeMode ?? "").toLowerCase();
        if (
            cat === "wf" ||
            cs.startsWith("wf_") ||
            cs.startsWith("bl_") ||
            cs === "waterflood_custom_subcase"
        ) {
            return pviAvailable ? "pvi" : "time";
        }
        return "time";
    });

    let defaultPanelExpanded = $derived.by((): Record<string, boolean> => {
        const conf = layoutConfig?.rateChart;
        if (conf) {
            const overrides: Record<string, boolean> = {};
            for (const panelKey of conf.panelOrder ?? DEFAULT_RATE_CHART_PANEL_ORDER) {
                const expanded = conf.panels?.[panelKey]?.expanded;
                if (expanded !== undefined) overrides[panelKey] = expanded;
            }
            return overrides;
        }
        const cat = (activeMode ?? "").toLowerCase();
        const cs = (activeCase ?? "").toLowerCase();
        if (cat === "dep" || cat === "depletion" || cs.includes("depletion")) {
            return { rates: true, cumulative: false, diagnostics: true };
        }
        if (
            cat === "wf" ||
            cat === "waterflood" ||
            cs.startsWith("wf_") ||
            cs.includes("waterflood") ||
            cs.startsWith("bl_")
        ) {
            return { rates: true, cumulative: true, diagnostics: false };
        }
        return {};
    });
</script>

<UniversalChart
    {panelDefs}
    {buildCurveContext}
    {xAxisOptions}
    {supportsNormalization}
    {defaultXAxisMode}
    {defaultPanelExpanded}
    {theme}
    {layoutConfig}
/>
