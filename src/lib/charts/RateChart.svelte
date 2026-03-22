<script lang="ts">
    import ChartSubPanel from "./ChartSubPanel.svelte";
    import type { CurveConfig } from "./chartTypes";
    import {
        coerceChartAxisState,
        getConfiguredXAxisOptions,
        resolveChartPanelDefinition,
        type ChartPanelDefinition,
        type ChartPanelEntry,
        type ChartPanelFallback,
        type ChartXAxisOption,
    } from "./chartPanelSelection";
    import type {
        RateChartLayoutConfig,
        RateChartPanelKey,
        RateChartScalePreset,
        RateChartXAxisMode,
    } from "./rateChartLayoutConfig";
    import type {
        RateHistoryPoint,
        AnalyticalProductionPoint,
    } from "../simulator-types";
    import ToggleGroup from "../ui/controls/ToggleGroup.svelte";
    import { computeCombinedSweep, getSweepComponentVisibility, type SweepGeometry } from "../analytical/sweepEfficiency";
    import type { RockProps, FluidProps } from "../analytical/fractionalFlow";
    import { resolveSharedXAxisRange } from "./xAxisRangePolicy";

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
        sweepEfficiencySimSeries?: Array<{ time: number; eA: number; eV: number; eVol: number }> | null;
        sweepRFAnalytical?: import("../analytical/sweepEfficiency").SweepRFResult | null;
    } = $props();

    type SimSweepSeries = Array<{ time: number; eA: number; eV: number; eVol: number }>;

    // --- X-axis state (shared across all panels) ---
    type XYPoint = { x: number; y: number | null };
    type PanelDefinition = ChartPanelDefinition<CurveConfig, XYPoint[]>;

    let xAxisMode = $state<RateChartXAxisMode>("time");
    let logScale = $state(false);
    let normalizeRates = $state(false);

    // --- Panel expand/collapse state ---
    let ratesExpanded = $state(true);
    let recoveryExpanded = $state(true);
    let cumulativeExpanded = $state(false);
    let diagnosticsExpanded = $state(false);
    let volumesExpanded = $state(false);
    let oilRateExpanded = $state(false);
    let sweepRFExpanded = $state(true);
    let sweepArealExpanded = $state(false);
    let sweepVerticalExpanded = $state(false);
    let sweepVolExpanded = $state(false);

    // --- Panel alignment state ---
    let nativeGutters = $state<Record<string, { left: number; right: number }>>(
        {},
    );
    let maxLeftGutter = $derived(
        Math.max(0, ...Object.values(nativeGutters).map((g) => g.left)),
    );
    let maxRightGutter = $derived(
        Math.max(0, ...Object.values(nativeGutters).map((g) => g.right)),
    );

    // --- Error metrics ---
    type MismatchSummary = {
        pointsCompared: number;
        mae: number;
        rmse: number;
        mape: number;
    };
    let mismatchSummary = $state<MismatchSummary>({
        pointsCompared: 0,
        mae: 0,
        rmse: 0,
        mape: 0,
    });

    // --- Scenario-aware panel defaults ---
    // --- Scenario-aware panel defaults ---
    $effect(() => {
        // Track the activeCase trigger affirmatively so it evaluates once upon case shift
        const cat = (activeMode ?? "").toLowerCase();
        const cs = (activeCase ?? "").toLowerCase();

        // Use untrack so setting the state variables right here does not re-trigger this exact effect!
        import("svelte").then(({ untrack }) => {
            untrack(() => {
                const conf = layoutConfig?.rateChart;
                if (conf) {
                    if (conf.logScale !== undefined) logScale = conf.logScale;
                    if (conf.xAxisMode !== undefined)
                        xAxisMode = conf.xAxisMode;
                    if (conf.ratesExpanded !== undefined)
                        ratesExpanded = conf.ratesExpanded;
                    if (conf.recoveryExpanded !== undefined)
                        recoveryExpanded = conf.recoveryExpanded;
                    if (conf.cumulativeExpanded !== undefined)
                        cumulativeExpanded = conf.cumulativeExpanded;
                    if (conf.diagnosticsExpanded !== undefined)
                        diagnosticsExpanded = conf.diagnosticsExpanded;
                    if (conf.volumesExpanded !== undefined)
                        volumesExpanded = conf.volumesExpanded;
                    if (conf.oilRateExpanded !== undefined)
                        oilRateExpanded = conf.oilRateExpanded;
                } else {
                    // Fallback to purely string-matched defaults when CaseParams lacks layout metadata
                    if (cat === "dep" || cat === "depletion" || cs.includes("depletion")) {
                        ratesExpanded = true;
                        cumulativeExpanded = false;
                        diagnosticsExpanded = true;
                    } else if (
                        cat === "wf" ||
                        cat === "waterflood" ||
                        cs.startsWith("wf_") ||
                        cs.includes("waterflood") ||
                        cs.startsWith("bl_")
                    ) {
                        ratesExpanded = true;
                        cumulativeExpanded = true;
                        diagnosticsExpanded = false;
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
            });
        });
    });

    // ════════════════════════════════════════════════════════════
    //  DATA COMPUTATION (shared across panels)
    // ══════════════════════════════════════════════════════════════

    function toXYSeries(
        xValues: Array<number | null>,
        yValues: Array<number | null | undefined>,
    ): XYPoint[] {
        const points: XYPoint[] = [];
        for (let idx = 0; idx < yValues.length; idx++) {
            const rawX = xValues[idx];
            const rawY = yValues[idx];
            if (!Number.isFinite(rawX)) continue;
            points.push({
                x: Number(rawX),
                y: Number.isFinite(rawY) ? Number(rawY) : null,
            });
        }
        return points;
    }

    // Computed data series
    let oilProd = $derived(
        rateHistory.map((p) => Math.abs(p.total_production_oil ?? 0)),
    );
    let liquidProd = $derived(
        rateHistory.map((p) => Math.abs(p.total_production_liquid ?? 0)),
    );
    let injection = $derived(rateHistory.map((p) => p.total_injection ?? 0));
    let waterProd = $derived(
        liquidProd.map((qL, idx) =>
            Math.max(0, Number(qL ?? 0) - Number(oilProd[idx] ?? 0)),
        ),
    );
    let timeValues = $derived(rateHistory.map((p) => Number(p.time)));

    let analyticalOilProd = $derived(
        rateHistory.map((_, idx) => {
            const v = analyticalProductionData[idx]?.oilRate;
            return Number.isFinite(v) ? v : null;
        }),
    );
    let analyticalWaterRate = $derived(
        rateHistory.map((_, idx) => {
            const v = analyticalProductionData[idx]?.waterRate;
            return Number.isFinite(v) ? v : null;
        }),
    );

    let ratesScaleFactor = $derived(
        normalizeRates && analyticalMeta?.q0 && analyticalMeta.q0 > 0
            ? 1.0 / analyticalMeta.q0
            : 1.0,
    );

    let normOilProd = $derived(
        oilProd.map((v) => Number(v) * ratesScaleFactor),
    );
    let normLiquidProd = $derived(
        liquidProd.map((v) => Number(v) * ratesScaleFactor),
    );
    let normInjection = $derived(
        injection.map((v) => Number(v) * ratesScaleFactor),
    );
    let normWaterProd = $derived(
        waterProd.map((v) => Number(v) * ratesScaleFactor),
    );
    let normAnalyticalOilProd = $derived(
        analyticalOilProd.map((v) =>
            v === null || v === undefined ? null : v * ratesScaleFactor,
        ),
    );
    let normAnalyticalWaterRate = $derived(
        analyticalWaterRate.map((v) =>
            v === null || v === undefined ? null : v * ratesScaleFactor,
        ),
    );
    let normOilRateAbsError = $derived(
        normOilProd.map((sim, idx) => {
            const analytical = normAnalyticalOilProd[idx];
            if (analytical === null || sim === null) return null;
            return Math.abs(sim - analytical);
        }),
    );

    // Cumulative computations
    let cumulatives = $derived.by(() => {
        let cumOil = 0,
            cumInj = 0,
            cumLiq = 0,
            cumWater = 0;
        const cumOilArr: number[] = [];
        const cumInjArr: number[] = [];
        const cumLiqArr: number[] = [];
        const cumWaterArr: number[] = [];
        const pviArr: number[] = [];
        const pvpArr: number[] = [];
        for (let i = 0; i < rateHistory.length; i++) {
            const dt =
                i > 0
                    ? rateHistory[i].time - rateHistory[i - 1].time
                    : rateHistory[i].time;
            cumOil += Number(oilProd[i] ?? 0) * dt;
            cumInj += Math.max(0, Number(injection[i] ?? 0)) * dt;
            cumLiq += Math.max(0, Number(liquidProd[i] ?? 0)) * dt;
            cumWater += Math.max(0, Number(waterProd[i] ?? 0)) * dt;
            cumOilArr.push(cumOil);
            cumInjArr.push(cumInj);
            cumLiqArr.push(cumLiq);
            cumWaterArr.push(cumWater);
            pviArr.push(poreVolumeM3 > 1e-12 ? cumInj / poreVolumeM3 : 0);
            pvpArr.push(poreVolumeM3 > 1e-12 ? cumLiq / poreVolumeM3 : 0);
        }
        return {
            cumOil: cumOilArr,
            cumInj: cumInjArr,
            cumLiq: cumLiqArr,
            cumWater: cumWaterArr,
            pvi: pviArr,
            pvp: pvpArr,
        };
    });

    let analyticalCumOil = $derived.by(() => {
        return rateHistory.map((_, idx) => {
            const v = analyticalProductionData[idx]?.cumulativeOil;
            if (Number.isFinite(v)) return v;
            // Fallback: integrate analytical oil rate
            let cum = 0;
            for (let i = 0; i <= idx; i++) {
                const oi = analyticalOilProd[i];
                if (!Number.isFinite(oi)) return null;
                const dt =
                    i > 0
                        ? Math.max(
                              0,
                              rateHistory[i].time - rateHistory[i - 1].time,
                          )
                        : Math.max(0, rateHistory[i].time);
                cum += (oi as number) * dt;
            }
            return cum;
        });
    });

    let recoveryFactor = $derived(
        cumulatives.cumOil.map((c) =>
            ooipM3 > 1e-12 ? Math.max(0, Math.min(1, c / ooipM3)) : null,
        ),
    );
    let analyticalRecoveryFactor = $derived(
        analyticalCumOil.map((c) =>
            c == null ? null : (ooipM3 > 1e-12 ? Math.max(0, Math.min(1, c / ooipM3)) : null),
        ),
    );

    // Diagnostic computations
    let avgPressure = $derived(
        rateHistory.map((p, idx) => {
            const v =
                avgReservoirPressureSeries?.[idx] ??
                p.avg_reservoir_pressure ??
                (p as any).average_reservoir_pressure ??
                (p as any).avg_pressure;
            return Number.isFinite(v) ? v : null;
        }),
    );
    let analyticalAvgPressure = $derived(
        rateHistory.map((_, idx) => {
            const v = (analyticalProductionData[idx] as any)?.avgPressure;
            return Number.isFinite(v) ? v : null;
        }),
    );
    let avgWaterSat = $derived(
        rateHistory.map((p, idx) => {
            const v = avgWaterSaturationSeries?.[idx] ?? p.avg_water_saturation;
            return Number.isFinite(v) ? v : null;
        }),
    );

    // VRR
    let vrrData = $derived.by(() => {
        let cumInjR = 0,
            cumProdR = 0;
        return rateHistory.map((p, idx) => {
            const dt =
                idx > 0
                    ? Math.max(
                          0,
                          rateHistory[idx].time - rateHistory[idx - 1].time,
                      )
                    : Math.max(0, rateHistory[idx].time);
            const injR = Number(
                (p as any).total_injection_reservoir ?? p.total_injection,
            );
            const prodR = Number(
                (p as any).total_production_liquid_reservoir ??
                    p.total_production_liquid,
            );
            if (dt > 0 && Number.isFinite(injR) && Number.isFinite(prodR)) {
                cumInjR += Math.max(0, injR) * dt;
                cumProdR += Math.max(0, prodR) * dt;
            }
            if (cumProdR <= 1e-12) return null;
            const raw = cumInjR / cumProdR;
            return Math.abs(raw - 1.0) < 1e-9 ? 1.0 : raw;
        });
    });

    // WOR
    let worSim = $derived(
        rateHistory.map((_, idx) => {
            const oil = Number(oilProd[idx]);
            const water = Number(waterProd[idx]);
            if (!Number.isFinite(oil) || oil <= 1e-12) return null;
            return Math.max(0, water / oil);
        }),
    );
    let worAnalytical = $derived(
        rateHistory.map((_, idx) => {
            const oil = Number(analyticalProductionData[idx]?.oilRate);
            const water = Number(analyticalProductionData[idx]?.waterRate);
            if (
                !Number.isFinite(oil) ||
                oil <= 1e-12 ||
                !Number.isFinite(water)
            )
                return null;
            return Math.max(0, water / oil);
        }),
    );

    // Water cut
    let waterCutSim = $derived(
        rateHistory.map((p, idx) => {
            const liquid = Number(p.total_production_liquid);
            if (!Number.isFinite(liquid) || liquid <= 1e-12) return 0;
            return Math.max(0, Math.min(1, waterProd[idx] / liquid));
        }),
    );
    let waterCutAnalytical = $derived(
        rateHistory.map((_, idx) => {
            const oil = Number(analyticalProductionData[idx]?.oilRate);
            const water = Number(analyticalProductionData[idx]?.waterRate);
            const total = oil + water;
            if (!Number.isFinite(total) || total <= 1e-12) return null;
            return Math.max(0, Math.min(1, water / total));
        }),
    );

    // MB Error
    let mbError = $derived(
        rateHistory.map((p) => {
            const v = Number((p as any).material_balance_error_m3);
            return Number.isFinite(v) ? v : null;
        }),
    );

    // Oil rate error
    let oilRateAbsError = $derived(
        oilProd.map((sim, idx) => {
            const analytical = Number(analyticalOilProd[idx]);
            const simVal = Number(sim);
            if (!Number.isFinite(analytical) || !Number.isFinite(simVal))
                return null;
            return Math.abs(simVal - analytical);
        }),
    );

    // Error stats
    $effect(() => {
        const validErrors = oilRateAbsError.filter(
            (v) => v !== null && Number.isFinite(v),
        ) as number[];
        const percentErrors = oilProd
            .map((sim, idx) => {
                const analytical = Number(analyticalOilProd[idx]);
                const simVal = Number(sim);
                if (!Number.isFinite(analytical) || !Number.isFinite(simVal))
                    return null;
                return (
                    (Math.abs(simVal - analytical) /
                        Math.max(Math.abs(analytical), 1e-9)) *
                    100
                );
            })
            .filter((v) => v !== null && Number.isFinite(v)) as number[];

        if (validErrors.length > 0) {
            mismatchSummary = {
                pointsCompared: validErrors.length,
                mae:
                    validErrors.reduce((a, v) => a + v, 0) / validErrors.length,
                rmse: Math.sqrt(
                    validErrors.reduce((a, v) => a + v * v, 0) /
                        validErrors.length,
                ),
                mape:
                    percentErrors.length > 0
                        ? percentErrors.reduce((a, v) => a + v, 0) /
                          percentErrors.length
                        : 0,
            };
        } else {
            mismatchSummary = { pointsCompared: 0, mae: 0, rmse: 0, mape: 0 };
        }
    });

    // PVI/PVP availability
    let pviAvailable = $derived(
        cumulatives.cumInj[cumulatives.cumInj.length - 1] > 1e-12,
    );
    let pvpAvailable = $derived(
        cumulatives.cumLiq[cumulatives.cumLiq.length - 1] > 1e-12,
    );

    // ══════════════════════════════════════════════════════════════
    //  X-AXIS VALUES (shared across panels)
    // ══════════════════════════════════════════════════════════════

    let xValues = $derived.by(() => {
        if (xAxisMode === "pvi") return cumulatives.pvi;
        if (xAxisMode === "pvp") return cumulatives.pvp;
        if (xAxisMode === "cumLiquid") return cumulatives.cumLiq;
        if (xAxisMode === "cumInjection") return cumulatives.cumInj;
        if (xAxisMode === "logTime")
            return timeValues.map((t) => (t > 0 ? Math.log10(t) : null));
        if (xAxisMode === "tD" && analyticalMeta?.tau && analyticalMeta.tau > 0)
            return timeValues.map((t) => t / analyticalMeta.tau);
        return timeValues;
    });

    function setXAxisMode(mode: RateChartXAxisMode) {
        if (mode === "pvi" && !pviAvailable) return;
        if (mode === "pvp" && !pvpAvailable) return;
        if (mode === "tD" && (!analyticalMeta?.tau || analyticalMeta.tau <= 0))
            return;
        xAxisMode = mode;
    }

    function getXAxisTitle(): string {
        if (xAxisMode === "pvi") return "PV Injected (PVI)";
        if (xAxisMode === "pvp") return "PV Produced (PVP)";
        if (xAxisMode === "cumLiquid")
            return "Cumulative Liquid Production (m³)";
        if (xAxisMode === "cumInjection") return "Cumulative Injection (m³)";
        if (xAxisMode === "logTime") return "Time (days) — log₁₀";
        if (xAxisMode === "tD") return "Dimensionless Time (tD = t/τ)";
        return "Time (days)";
    }

    function getSweepZeroXAxisValue(): number | null {
        return xAxisMode === "logTime" ? null : 0;
    }

    function mapPviToActiveXAxis(pvi: number): number | null {
        if (!Number.isFinite(pvi)) return null;
        if (xAxisMode === "pvi") return pvi;
        if (pvi <= 1e-12) return getSweepZeroXAxisValue();

        const domain = cumulatives.pvi;
        const range = xValues;
        if (domain.length === 0 || range.length === 0) return null;

        let previousIndex = -1;
        for (let index = 0; index < domain.length; index++) {
            const xDomain = domain[index];
            const xRange = range[index];
            if (!Number.isFinite(xDomain) || !Number.isFinite(xRange)) continue;
            if (Math.abs((xDomain as number) - pvi) <= 1e-9) return Number(xRange);
            if ((xDomain as number) > pvi) {
                if (previousIndex < 0) return Number(xRange);
                const d0 = Number(domain[previousIndex]);
                const r0 = Number(range[previousIndex]);
                const d1 = Number(xDomain);
                const r1 = Number(xRange);
                if (Math.abs(d1 - d0) <= 1e-12) return r1;
                const fraction = (pvi - d0) / (d1 - d0);
                return r0 + fraction * (r1 - r0);
            }
            previousIndex = index;
        }

        return previousIndex >= 0 && Number.isFinite(range[previousIndex])
            ? Number(range[previousIndex])
            : null;
    }

    // ══════════════════════════════════════════════════════════════
    //  PANEL CURVE CONFIGS + SERIES
    // ══════════════════════════════════════════════════════════════

    function applyCurveLayout(defaultCurves: CurveConfig[]): CurveConfig[] {
        const customCurves = layoutConfig?.rateChart?.curves;
        if (!customCurves) return defaultCurves;
        return defaultCurves.map((c) => {
            const override = customCurves[c.label];
            if (!override) return c;
            return {
                ...c,
                defaultVisible:
                    override.visible !== undefined
                        ? override.visible
                        : c.defaultVisible,
                disabled: override.disabled,
            };
        });
    }

    let neutralColor = $derived(theme === "dark" ? "#f8fafc" : "#0f172a");

    let baseRatesCurves = $derived.by((): CurveConfig[] => [
        { label: "Oil Rate", curveKey: "oil-rate-sim", toggleLabel: "Oil Rate", color: "#16a34a", borderWidth: 2.5, yAxisID: "y" },
        {
            label: "Oil Rate (Reference Solution)",
            curveKey: "oil-rate-reference",
            toggleLabel: "Reference Solution Oil Rate",
            color: neutralColor,
            borderWidth: 2,
            borderDash: [7, 4],
            yAxisID: "y",
        },
        {
            label: "Water Rate",
            curveKey: "water-rate-sim",
            toggleLabel: "Water Rate",
            color: "#1e3a8a",
            borderWidth: 2.5,
            yAxisID: "y",
        },
        {
            label: "Water Rate (Reference Solution)",
            curveKey: "water-rate-reference",
            toggleLabel: "Reference Solution Water Rate",
            color: neutralColor,
            borderWidth: 2,
            borderDash: [7, 4],
            yAxisID: "y",
        },
        {
            label: "Injection Rate",
            curveKey: "injection-rate",
            toggleLabel: "Injection Rate",
            color: "#06b6d4",
            borderWidth: 2.5,
            yAxisID: "y",
        },
        {
            label: "Liquid Rate",
            curveKey: "liquid-rate",
            toggleLabel: "Liquid Rate",
            color: "#2563eb",
            borderWidth: 2,
            yAxisID: "y",
            defaultVisible: false,
        },
    ]);

    let baseCumulativeCurves = $derived.by((): CurveConfig[] => [
        { label: "Cum Oil", curveKey: "cum-oil-sim", toggleLabel: "Cum Oil", color: "#0f5132", borderWidth: 2.5, yAxisID: "y" },
        {
            label: "Cum Oil (Reference Solution)",
            curveKey: "cum-oil-reference",
            toggleLabel: "Reference Solution Cum Oil",
            color: neutralColor,
            borderWidth: 2,
            borderDash: [7, 4],
            yAxisID: "y",
        },
        {
            label: "Cum Injection",
            curveKey: "cum-injection",
            toggleLabel: "Cum Injection",
            color: "#06b6d4",
            borderWidth: 2,
            borderDash: [3, 4],
            yAxisID: "y",
        },
        { label: "Cum Water", curveKey: "cum-water", toggleLabel: "Cum Water", color: "#1e3a8a", borderWidth: 2, yAxisID: "y" },
        {
            label: "Recovery Factor",
            curveKey: "recovery-factor",
            toggleLabel: "Recovery Factor",
            color: "#22c55e",
            borderWidth: 2,
            yAxisID: "y1",
            defaultVisible: false,
        },
        {
            label: "Recovery Factor (Primary)",
            curveKey: "recovery-factor-primary",
            toggleLabel: "Recovery Factor",
            color: "#22c55e",
            borderWidth: 2.2,
            yAxisID: "y",
        },
        {
            label: "Recovery Factor (Reference)",
            curveKey: "recovery-factor-reference",
            toggleLabel: "Reference Solution RF",
            color: neutralColor,
            borderWidth: 2,
            borderDash: [7, 4],
            yAxisID: "y",
        },
    ]);

    let baseDiagnosticsCurves = $derived.by((): CurveConfig[] => [
        {
            label: "Avg Pressure",
            curveKey: "avg-pressure-sim",
            toggleLabel: "Avg Pressure",
            color: "#dc2626",
            borderWidth: 2,
            yAxisID: "y",
        },
        {
            label: "Avg Pressure (Reference Solution)",
            curveKey: "avg-pressure-reference",
            toggleLabel: "Reference Solution Avg Pressure",
            color: neutralColor,
            borderWidth: 2,
            borderDash: [7, 4],
            yAxisID: "y",
        },
        {
            label: "VRR",
            curveKey: "vrr",
            toggleLabel: "VRR",
            color: "#7c3aed",
            borderWidth: 2.5,
            yAxisID: "y1",
            defaultVisible: false,
        },
        {
            label: "WOR (Sim)",
            curveKey: "wor-sim",
            toggleLabel: "WOR",
            color: "#d97706",
            borderWidth: 2.3,
            yAxisID: "y1",
            defaultVisible: false,
        },
        {
            label: "WOR (Reference Solution)",
            curveKey: "wor-reference",
            toggleLabel: "Reference Solution WOR",
            color: neutralColor,
            borderWidth: 2,
            borderDash: [7, 4],
            yAxisID: "y1",
            defaultVisible: false,
        },
        {
            label: "Avg Water Sat",
            curveKey: "avg-water-sat",
            toggleLabel: "Avg Water Sat",
            color: "#1d4ed8",
            borderWidth: 2,
            borderDash: [3, 4],
            yAxisID: "y1",
            defaultVisible: false,
        },
        {
            label: "Water Cut (Sim)",
            curveKey: "water-cut-sim",
            toggleLabel: "Water Cut",
            color: "#2563eb",
            borderWidth: 2.3,
            yAxisID: "y1",
            defaultVisible: false,
        },
        {
            label: "Water Cut (Reference Solution)",
            curveKey: "water-cut-reference",
            toggleLabel: "Reference Solution Water Cut",
            color: neutralColor,
            borderWidth: 2,
            borderDash: [7, 4],
            yAxisID: "y1",
            defaultVisible: false,
        },
        {
            label: "MB Error",
            curveKey: "mb-error",
            toggleLabel: "MB Error",
            color: "#ef4444",
            borderWidth: 1.5,
            borderDash: [3, 3],
            yAxisID: "y2",
            defaultVisible: false,
        },
    ]);

    // --- Build XY series for each panel ---
    let rateCurveSeries = $derived([
        toXYSeries(xValues, normOilProd),
        toXYSeries(xValues, normAnalyticalOilProd),
        toXYSeries(xValues, normWaterProd),
        toXYSeries(xValues, normAnalyticalWaterRate),
        toXYSeries(xValues, normInjection),
        toXYSeries(xValues, normLiquidProd),
    ]);

    let cumulativeCurveSeries = $derived([
        toXYSeries(xValues, cumulatives.cumOil),
        toXYSeries(xValues, analyticalCumOil as Array<number | null>),
        toXYSeries(xValues, cumulatives.cumInj),
        toXYSeries(xValues, cumulatives.cumWater),
        toXYSeries(xValues, recoveryFactor as Array<number | null>),
        toXYSeries(xValues, recoveryFactor as Array<number | null>),
        toXYSeries(xValues, analyticalRecoveryFactor as Array<number | null>),
    ]);

    let diagnosticsCurveSeries = $derived([
        toXYSeries(xValues, avgPressure as Array<number | null>),
        toXYSeries(xValues, analyticalAvgPressure as Array<number | null>),
        toXYSeries(xValues, vrrData as Array<number | null>),
        toXYSeries(xValues, worSim as Array<number | null>),
        toXYSeries(xValues, worAnalytical as Array<number | null>),
        toXYSeries(xValues, avgWaterSat as Array<number | null>),
        toXYSeries(xValues, waterCutSim as Array<number | null>),
        toXYSeries(xValues, waterCutAnalytical as Array<number | null>),
        toXYSeries(xValues, mbError as Array<number | null>),
    ]);

    const recoveryScales = {
        y: {
            type: "linear",
            display: true,
            position: "left",
            min: 0,
            max: 1,
            alignToPixels: true,
            title: { display: true, text: "Recovery Factor" },
            ticks: { count: 6 },
            _fraction: true,
        },
    };
    const cumulativeVolumesScales = {
        y: {
            type: "linear",
            display: true,
            position: "left",
            min: 0,
            alignToPixels: true,
            title: { display: true, text: "Cumulative (m³)" },
            ticks: { count: 6 },
        },
    };

    let ratesScales = $derived({
        y: {
            type: "linear",
            display: true,
            position: "left",
            min: 0,
            alignToPixels: true,
            title: {
                display: true,
                text: normalizeRates
                    ? "Normalized Rate (q/q₀)"
                    : "Rate (m³/day)",
            },
            ticks: { count: 6 },
        },
    });
    const breakthroughScales = {
        y1: {
            type: "linear",
            display: true,
            position: "right",
            min: 0,
            max: 1,
            alignToPixels: true,
            title: { display: true, text: "Water Cut / Saturation" },
            grid: { drawOnChartArea: false },
            ticks: { count: 6 },
            _fraction: true,
        },
    };
    const cumulativeScales = {
        y: {
            type: "linear",
            display: true,
            position: "left",
            min: 0,
            alignToPixels: true,
            title: { display: true, text: "Cumulative (m³)" },
            ticks: { count: 6 },
        },
        y1: {
            type: "linear",
            display: true,
            position: "right",
            min: 0,
            max: 1,
            alignToPixels: true,
            title: { display: true, text: "Recovery Factor" },
            grid: { drawOnChartArea: false },
            ticks: { count: 6 },
            _fraction: true,
        },
    };
    const pressureScales = {
        y: {
            type: "linear",
            display: true,
            position: "left",
            alignToPixels: true,
            title: { display: true, text: "Pressure (bar)" },
            ticks: { count: 6 },
            _auto: true,
        },
    };
    const diagnosticsScales = {
        y: {
            type: "linear",
            display: true,
            position: "left",
            alignToPixels: true,
            title: { display: true, text: "Pressure (bar)" },
            ticks: { count: 6 },
            _auto: true,
        },
        y1: {
            type: "linear",
            display: true,
            position: "right",
            min: 0,
            alignToPixels: true,
            title: { display: true, text: "Fraction" },
            grid: { drawOnChartArea: false },
            ticks: { count: 6 },
            _dynamicTitle: (labels: string[]) => {
                const parts: string[] = [];
                if (labels.some((l) => l.includes("VRR"))) parts.push("VRR");
                if (labels.some((l) => l.includes("WOR"))) parts.push("WOR");
                if (labels.some((l) => l.includes("Sat")))
                    parts.push("Saturation");
                if (labels.some((l) => l.includes("Cut")))
                    parts.push("Water Cut");
                return parts.length > 0 ? parts.join(" / ") : "Fraction";
            },
        },
        y2: {
            type: "linear",
            display: true,
            position: "right",
            min: 0,
            alignToPixels: true,
            title: { display: true, text: "MB Error (m³)" },
            grid: { drawOnChartArea: false },
            ticks: { count: 6 },
        },
    };
    let allXAxisOptions = $derived<ChartXAxisOption[]>([
        { value: "time", label: "Time" },
        {
            value: "tD",
            label: "tD",
            disabled: !analyticalMeta?.tau || analyticalMeta.tau <= 0,
            title: "Dimensionless Time (t/τ)",
        },
        {
            value: "pvi",
            label: "PVI",
            disabled: !pviAvailable,
            title: "PV Injected",
        },
        {
            value: "pvp",
            label: "PVP",
            disabled: !pvpAvailable,
            title: "PV Produced",
        },
        { value: "cumLiquid", label: "Cum Liq", title: "Cumulative Liquid" },
        {
            value: "cumInjection",
            label: "Cum Inj",
            title: "Cumulative Injection",
        },
        { value: "logTime", label: "Log Time", title: "Log Time (Fetkovich)" },
    ]);

    function getScalePresetConfig(scalePreset: RateChartScalePreset): Record<string, any> {
        if (scalePreset === "breakthrough") return breakthroughScales;
        if (scalePreset === "pressure") return pressureScales;
        if (scalePreset === "cumulative") return cumulativeScales;
        if (scalePreset === "cumulative_volumes") return cumulativeVolumesScales;
        if (scalePreset === "recovery") return recoveryScales;
        if (scalePreset === "diagnostics") return diagnosticsScales;
        return ratesScales;
    }

    const curveRegistry = $derived.by((): Array<ChartPanelEntry<CurveConfig, XYPoint[]>> => {
        return [
            ...baseRatesCurves.map((curve, idx) => ({
                curve,
                series: rateCurveSeries[idx] ?? [],
            })),
            ...baseCumulativeCurves.map((curve, idx) => ({
                curve,
                series: cumulativeCurveSeries[idx] ?? [],
            })),
            ...baseDiagnosticsCurves.map((curve, idx) => ({
                curve,
                series: diagnosticsCurveSeries[idx] ?? [],
            })),
        ];
    });

    function buildPanelDefinition(
        panelKey: RateChartPanelKey,
        input: ChartPanelFallback,
    ): PanelDefinition {
        const panelDefinition = resolveChartPanelDefinition({
            override: layoutConfig?.rateChart?.panels?.[panelKey],
            fallback: input,
            entries: curveRegistry,
            getScalePresetConfig,
        });

        return {
            ...panelDefinition,
            curves: applyCurveLayout(panelDefinition.curves),
        };
    }

    let ratesPanel = $derived(
        buildPanelDefinition("rates", {
            title: "Rates",
            curveKeys: baseRatesCurves.map((curve) => curve.curveKey ?? curve.label),
            curveLabels: baseRatesCurves.map((curve) => curve.label),
            scalePreset: "rates",
            allowLogToggle: true,
        }),
    );
    let recoveryPanel = $derived(
        buildPanelDefinition("recovery", {
            title: "Recovery Factor",
            curveKeys: ["recovery-factor-primary", "recovery-factor-reference"],
            scalePreset: "recovery",
        }),
    );
    let cumulativePanel = $derived(
        buildPanelDefinition("cumulative", {
            title: "Cum Oil",
            curveKeys: ["cum-oil-sim", "cum-oil-reference", "cum-injection"],
            scalePreset: "cumulative_volumes",
        }),
    );
    let diagnosticsPanel = $derived(
        buildPanelDefinition("diagnostics", {
            title: "Diagnostics",
            curveKeys: baseDiagnosticsCurves.map((curve) => curve.curveKey ?? curve.label),
            curveLabels: baseDiagnosticsCurves.map((curve) => curve.label),
            scalePreset: "diagnostics",
        }),
    );
    let volumesPanel = $derived(
        buildPanelDefinition("volumes", {
            title: "Cum Injection",
            curveKeys: ["cum-injection"],
            scalePreset: "cumulative_volumes",
        }),
    );
    let oilRatePanel = $derived(
        buildPanelDefinition("oil_rate", {
            title: "Oil Rate",
            curveKeys: ["oil-rate-sim", "oil-rate-reference"],
            scalePreset: "rates",
        }),
    );

    let ratesCurves = $derived(ratesPanel.curves);
    let recoveryCurves = $derived(recoveryPanel.curves);
    let cumulativeCurves = $derived(cumulativePanel.curves);
    let diagnosticsCurves = $derived(diagnosticsPanel.curves);
    let volumesCurves = $derived(volumesPanel.curves);
    let oilRateCurves = $derived(oilRatePanel.curves);
    let ratesSeries = $derived(ratesPanel.series);
    let recoverySeries = $derived(recoveryPanel.series);
    let cumulativeSeries = $derived(cumulativePanel.series);
    let diagnosticsSeries = $derived(diagnosticsPanel.series);
    let volumesSeries = $derived(volumesPanel.series);
    let oilRateSeries = $derived(oilRatePanel.series);

    const ratePanelSupportsNormalization = $derived(
        ratesCurves.some((curve) => curve.label.includes("Rate")),
    );

    // --- Sweep panels (sweep-domain scenarios only, remapped to active x-axis) ---
    //
    // PANEL ORDER (primary → diagnostic):
    //   1. Recovery Factor — RF_sim (solid) vs RF_sweep_analytical = E_vol×E_D_BL (dashed)
    //                        vs RF_1D_BL upper bound (light dashed). PRIMARY output.
    //   2. Areal Sweep Efficiency (E_A) — sim (solid) vs analytical (dashed). DIAGNOSTIC.
    //   3. Vertical Sweep Efficiency (E_V) — shown only when nz > 1.  DIAGNOSTIC.
    //   4. Volumetric Sweep Efficiency (E_vol = E_A × E_V).  DIAGNOSTIC.
    //
    // Chart convention (all sweep panels):
    //   Solid         = simulation (IMPES result or derived from sat_water grid)
    //   Dashed [7,4]  = primary analytical reference
    //   Dashed [4,4]  = supplemental analytical reference (e.g., perfect-sweep upper bound)
    //   Color         = single-run: fixed per metric; multi-variant: CASE_COLORS[index]
    //
    const sweepScaleConfig = {
        y: {
            type: "linear",
            display: true,
            position: "left",
            min: 0,
            max: 1,
            alignToPixels: true,
            title: { display: true, text: "Sweep Efficiency" },
            ticks: { count: 6 },
        },
    };

    const sweepRFScaleConfig = {
        y: {
            type: "linear",
            display: true,
            position: "left",
            min: 0,
            max: 1,
            alignToPixels: true,
            title: { display: true, text: "Recovery Factor" },
            ticks: {
                count: 6,
            },
            _tickFormatter: (v: string | number) =>
                typeof v === "number" ? (v * 100).toFixed(0) + "%" : v,
        },
    };

    // Map simulation sweep history entries onto PVI x-axis using rateHistory time series.
    function simSweepToXY(
        series: SimSweepSeries,
        getter: (pt: SimSweepSeries[0]) => number,
    ): Array<{ x: number; y: number | null }> {
        if (!series || series.length === 0) return [];
        const points = series.map((pt) => {
            if (pt.time <= 1e-12) {
                return { x: getSweepZeroXAxisValue() ?? 0, y: getter(pt) };
            }
            // Map simulation sweep history onto the currently selected x-axis.
            const tIdx = timeValues.findIndex((t) => t >= pt.time - 1e-9);
            const x = tIdx >= 0 ? (xValues[tIdx] ?? null) : (xValues.at(-1) ?? null);
            return { x: x ?? 0, y: getter(pt) };
        });
        const deduped: Array<{ x: number; y: number | null }> = [];
        for (const point of points) {
            const previous = deduped.at(-1);
            if (previous && Math.abs(previous.x - point.x) <= 1e-9) {
                previous.y = point.y;
                continue;
            }
            deduped.push(point);
        }
        return deduped;
    }

    const sweepPanels = $derived.by(() => {
        if (!showSweepPanel || !rockProps || !fluidProps) return null;

        const perms = layerPermeabilities.length > 0 ? layerPermeabilities : [100];
        const analytical = computeCombinedSweep(rockProps, fluidProps, perms, layerThickness, 3.0, 200, sweepGeometry);
        const visibility = getSweepComponentVisibility(sweepGeometry);
        const pviValues = analytical.arealSweep.curve.map((p) => p.pvi);
        const analyticalXY = (ys: number[]) => pviValues.map((pvi, i) => ({ x: mapPviToActiveXAxis(pvi) ?? 0, y: ys[i] ?? null }));

        const hasSim = sweepEfficiencySimSeries != null && sweepEfficiencySimSeries.length > 0;
        const showAreal = visibility.showAreal;
        const showVertical = visibility.showVertical;

        // Areal panel
        const arealCurves: CurveConfig[] = showAreal ? [
            ...(hasSim ? [{
                label: "E_A (Simulation)",
                curveKey: "sweep-areal-sim",
                toggleLabel: "E_A (Sim)",
                color: "#2563eb",
                borderWidth: 2.4,
                yAxisID: "y",
            } as CurveConfig] : []),
            {
                label: "E_A (Analytical)",
                curveKey: "sweep-areal-analytical",
                toggleLabel: "E_A (Analytical)",
                color: "#2563eb",
                borderWidth: 2.0,
                borderDash: [7, 4],
                yAxisID: "y",
            } as CurveConfig,
        ] : [];
        const arealSeries = showAreal ? [
            ...(hasSim ? [simSweepToXY(sweepEfficiencySimSeries!, (p) => p.eA)] : []),
            analyticalXY(analytical.arealSweep.curve.map((p) => p.efficiency)),
        ] : [];

        const verticalCurves: CurveConfig[] = showVertical ? [
            ...(hasSim ? [{
                label: "E_V (Simulation)",
                curveKey: "sweep-vertical-sim",
                toggleLabel: "E_V (Sim)",
                color: "#16a34a",
                borderWidth: 2.4,
                yAxisID: "y",
            } as CurveConfig] : []),
            {
                label: "E_V (Analytical)",
                curveKey: "sweep-vertical-analytical",
                toggleLabel: "E_V (Analytical)",
                color: "#16a34a",
                borderWidth: 2.0,
                borderDash: [7, 4],
                yAxisID: "y",
            } as CurveConfig,
        ] : [];
        const verticalSeries = showVertical ? [
            ...(hasSim ? [simSweepToXY(sweepEfficiencySimSeries!, (p) => p.eV)] : []),
            analyticalXY(analytical.verticalSweep.curve.map((p) => p.efficiency)),
        ] : [];

        // Volumetric panel
        const volCurves: CurveConfig[] = [
            ...(hasSim ? [{
                label: "E_vol (Simulation)",
                curveKey: "sweep-vol-sim",
                toggleLabel: "E_vol (Sim)",
                color: "#dc2626",
                borderWidth: 2.4,
                yAxisID: "y",
            } as CurveConfig] : []),
            {
                label: "E_vol (Analytical)",
                curveKey: "sweep-vol-analytical",
                toggleLabel: "E_vol (Analytical)",
                color: "#dc2626",
                borderWidth: 2.0,
                borderDash: [7, 4],
                yAxisID: "y",
            } as CurveConfig,
        ];
        const volSeries = [
            ...(hasSim ? [simSweepToXY(sweepEfficiencySimSeries!, (p) => p.eVol)] : []),
            analyticalXY(analytical.combined.map((p) => p.efficiency)),
        ];

        // Recovery Factor panel
        // Sim RF: map rateHistory → (pvi, rf) using cumulatives
        const simRFSeries = xValues.map((x, i) => ({ x: x ?? 0, y: recoveryFactor[i] ?? null }));

        const rfCurves: CurveConfig[] = [
            {
                label: "RF (Simulation)",
                curveKey: "sweep-rf-sim",
                toggleLabel: "RF (Sim)",
                color: "#15803d",
                borderWidth: 2.4,
                yAxisID: "y",
            } as CurveConfig,
            {
                label: "RF (Sweep — Craig×BL)",
                curveKey: "sweep-rf-sweep",
                toggleLabel: "RF Analytical (Sweep)",
                color: "#15803d",
                borderWidth: 2.0,
                borderDash: [7, 4],
                yAxisID: "y",
            } as CurveConfig,
            {
                label: "RF (1D BL — perfect sweep)",
                curveKey: "sweep-rf-bl1d",
                toggleLabel: "RF 1D BL (upper bound)",
                color: "#4ade80",
                borderWidth: 1.4,
                borderDash: [4, 4],
                yAxisID: "y",
                defaultVisible: false,
            } as CurveConfig,
        ];
        const rfSeries = sweepRFAnalytical
            ? [
                simRFSeries,
                                sweepRFAnalytical.curve.map((p) => ({ x: mapPviToActiveXAxis(p.pvi) ?? 0, y: p.rfSweep })),
                                sweepRFAnalytical.curve.map((p) => ({ x: mapPviToActiveXAxis(p.pvi) ?? 0, y: p.rfBL1D })),
              ]
            : [simRFSeries, [], []];

        return { rfCurves, rfSeries, arealCurves, arealSeries, verticalCurves, verticalSeries, volCurves, volSeries, showAreal, showVertical };
    });

    const sharedXRange = $derived.by(() => {
        return resolveSharedXAxisRange({
            allSeries: [
                ...ratesSeries,
                ...recoverySeries,
                ...cumulativeSeries,
                ...diagnosticsSeries,
                ...volumesSeries,
                ...oilRateSeries,
                ...(sweepPanels ? sweepPanels.rfSeries : []),
                ...(sweepPanels ? sweepPanels.arealSeries : []),
                ...(sweepPanels ? sweepPanels.verticalSeries : []),
                ...(sweepPanels ? sweepPanels.volSeries : []),
            ],
            rateSeries: ratesSeries,
            xAxisMode,
            policy: layoutConfig?.rateChart?.xAxisRangePolicy,
            pviMappings: [{ domainValues: cumulatives.pvi, rangeValues: xValues }],
        });
    });

    let xAxisOptions = $derived.by(() => {
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

    <!-- Rates panel -->
    <ChartSubPanel
        panelId="rates"
        title={ratesPanel.title}
        bind:expanded={ratesExpanded}
        curves={ratesCurves}
        seriesData={ratesSeries}
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

    <!-- Recovery Factor panel -->
    <ChartSubPanel
        panelId="recovery"
        title={recoveryPanel.title}
        bind:expanded={recoveryExpanded}
        curves={recoveryCurves}
        seriesData={recoverySeries}
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

    <!-- Cumulative panel -->
    <ChartSubPanel
        panelId="cumulative"
        title={cumulativePanel.title}
        bind:expanded={cumulativeExpanded}
        curves={cumulativeCurves}
        seriesData={cumulativeSeries}
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

    <!-- Diagnostics panel -->
    <ChartSubPanel
        panelId="diagnostics"
        title={diagnosticsPanel.title}
        bind:expanded={diagnosticsExpanded}
        curves={diagnosticsCurves}
        seriesData={diagnosticsSeries}
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

    <!-- Volumes panel (cum injection) — shown when panel has content -->
    {#if volumesCurves.length > 0}
        <ChartSubPanel
            panelId="volumes"
            title={volumesPanel.title}
            bind:expanded={volumesExpanded}
            curves={volumesCurves}
            seriesData={volumesSeries}
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

    <!-- Oil rate panel (waterflood: collapsed by default) — shown when panel has content -->
    {#if oilRateCurves.length > 0}
        <ChartSubPanel
            panelId="oil_rate"
            title={oilRatePanel.title}
            bind:expanded={oilRateExpanded}
            curves={oilRateCurves}
            seriesData={oilRateSeries}
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

    <!-- Sweep panels (sweep-domain scenarios only) — RF first (primary), efficiency panels diagnostic -->
    {#if sweepPanels}
        <!-- 1. Recovery Factor — primary output -->
        <ChartSubPanel
            panelId="sweep-rf"
            title="Recovery Factor — Sweep Analysis"
            bind:expanded={sweepRFExpanded}
            curves={sweepPanels.rfCurves}
            seriesData={sweepPanels.rfSeries}
            scaleConfigs={sweepRFScaleConfig}
            {theme}
            logScale={false}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            onGutterMeasure={(left: number, right: number) => {
                nativeGutters = { ...nativeGutters, "sweep-rf": { left, right } };
            }}
        />
        <!-- 2–4. Sweep efficiency decomposition (diagnostic) -->
        {#if sweepPanels.showAreal}
            <ChartSubPanel
                panelId="sweep-areal"
                title="Areal Sweep Efficiency (E_A)"
                bind:expanded={sweepArealExpanded}
                curves={sweepPanels.arealCurves}
                seriesData={sweepPanels.arealSeries}
                scaleConfigs={sweepScaleConfig}
                {theme}
                logScale={false}
                xRange={sharedXRange}
                targetLeftGutter={maxLeftGutter}
                targetRightGutter={maxRightGutter}
                onGutterMeasure={(left: number, right: number) => {
                    nativeGutters = { ...nativeGutters, "sweep-areal": { left, right } };
                }}
            />
        {/if}
        {#if sweepPanels.showVertical}
            <ChartSubPanel
                panelId="sweep-vertical"
                title="Vertical Sweep Efficiency (E_V)"
                bind:expanded={sweepVerticalExpanded}
                curves={sweepPanels.verticalCurves}
                seriesData={sweepPanels.verticalSeries}
                scaleConfigs={sweepScaleConfig}
                {theme}
                logScale={false}
                xRange={sharedXRange}
                targetLeftGutter={maxLeftGutter}
                targetRightGutter={maxRightGutter}
                onGutterMeasure={(left: number, right: number) => {
                    nativeGutters = { ...nativeGutters, "sweep-vertical": { left, right } };
                }}
            />
        {/if}
        <ChartSubPanel
            panelId="sweep-vol"
            title="Volumetric Sweep Efficiency (E_vol)"
            bind:expanded={sweepVolExpanded}
            curves={sweepPanels.volCurves}
            seriesData={sweepPanels.volSeries}
            scaleConfigs={sweepScaleConfig}
            {theme}
            logScale={false}
            xRange={sharedXRange}
            targetLeftGutter={maxLeftGutter}
            targetRightGutter={maxRightGutter}
            onGutterMeasure={(left: number, right: number) => {
                nativeGutters = { ...nativeGutters, "sweep-vol": { left, right } };
            }}
        />
    {/if}

    <!-- Error stats -->
    {#if mismatchSummary.pointsCompared > 0}
        <div class="ui-support-copy px-4 pb-4 pt-2 opacity-60 md:px-5 md:pb-5">
            Reference Solution: {mismatchSummary.pointsCompared} pts · MAE: {mismatchSummary.mae.toFixed(
                3,
            )} · RMSE: {mismatchSummary.rmse.toFixed(3)} · MAPE: {mismatchSummary.mape.toFixed(
                2,
            )}%
        </div>
    {/if}
</div>
