<script lang="ts">
    import ChartSubPanel from './ChartSubPanel.svelte';
    import type { CurveConfig } from './ChartSubPanel.svelte';
    import type { RateHistoryPoint, AnalyticalProductionPoint } from './simulator-types';

    let {
        rateHistory = [],
        analyticalProductionData = [],
        avgReservoirPressureSeries = [],
        avgWaterSaturationSeries = [],
        ooipM3 = 0,
        poreVolumeM3 = 0,
        activeCategory = '',
        activeCase = '',
        theme = 'dark'
    }: {
        rateHistory?: RateHistoryPoint[];
        analyticalProductionData?: AnalyticalProductionPoint[];
        avgReservoirPressureSeries?: Array<number | null>;
        avgWaterSaturationSeries?: Array<number | null>;
        ooipM3?: number;
        poreVolumeM3?: number;
        activeCategory?: string;
        activeCase?: string;
        theme?: 'dark' | 'light';
    } = $props();

    // --- X-axis state (shared across all panels) ---
    type XAxisMode = 'time' | 'logTime' | 'pvi' | 'cumLiquid' | 'cumInjection';
    type XYPoint = { x: number; y: number | null };

    let xAxisMode = $state<XAxisMode>('time');
    let logScale = $state(false);

    // --- Panel expand/collapse state ---
    let ratesExpanded = $state(true);
    let cumulativeExpanded = $state(false);
    let diagnosticsExpanded = $state(false);

    // --- Panel alignment state ---
    let nativeGutters = $state<Record<string, { left: number; right: number }>>({});
    let maxLeftGutter = $derived(Math.max(0, ...Object.values(nativeGutters).map(g => g.left)));
    let maxRightGutter = $derived(Math.max(0, ...Object.values(nativeGutters).map(g => g.right)));

    // --- Error metrics ---
    type MismatchSummary = { pointsCompared: number; mae: number; rmse: number; mape: number };
    let mismatchSummary = $state<MismatchSummary>({ pointsCompared: 0, mae: 0, rmse: 0, mape: 0 });

    // --- Scenario-aware panel defaults ---
    // --- Scenario-aware panel defaults ---
    $effect(() => {
        const cat = (activeCategory ?? '').toLowerCase();
        const cs = (activeCase ?? '').toLowerCase();
        if (cat === 'depletion' || cs.includes('depletion')) {
            ratesExpanded = true;
            cumulativeExpanded = false;
            diagnosticsExpanded = true;
        } else if (cat === 'waterflood' || cs.includes('waterflood') || cs.startsWith('bl_')) {
            ratesExpanded = true;
            cumulativeExpanded = true;
            diagnosticsExpanded = false;
        }
        // Set default x-axis
        if (cs.startsWith('bl_') || cs === 'waterflood_custom_subcase') {
            xAxisMode = pviAvailable ? 'pvi' : 'time';
        }
    });

    // ════════════════════════════════════════════════════════════
    //  DATA COMPUTATION (shared across panels)
    // ══════════════════════════════════════════════════════════════

    function toXYSeries(xValues: Array<number | null>, yValues: Array<number | null | undefined>): XYPoint[] {
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
    let oilProd = $derived(rateHistory.map(p => p.total_production_oil ?? 0));
    let liquidProd = $derived(rateHistory.map(p => p.total_production_liquid ?? 0));
    let injection = $derived(rateHistory.map(p => p.total_injection ?? 0));
    let waterProd = $derived(liquidProd.map((qL, idx) => Math.max(0, Number(qL ?? 0) - Number(oilProd[idx] ?? 0))));
    let timeValues = $derived(rateHistory.map(p => Number(p.time)));

    let analyticalOilProd = $derived(rateHistory.map((_, idx) => {
        const v = analyticalProductionData[idx]?.oilRate;
        return Number.isFinite(v) ? v : null;
    }));
    let analyticalWaterRate = $derived(rateHistory.map((_, idx) => {
        const v = analyticalProductionData[idx]?.waterRate;
        return Number.isFinite(v) ? v : null;
    }));

    // Cumulative computations
    let cumulatives = $derived.by(() => {
        let cumOil = 0, cumInj = 0, cumLiq = 0, cumWater = 0;
        const cumOilArr: number[] = [];
        const cumInjArr: number[] = [];
        const cumLiqArr: number[] = [];
        const cumWaterArr: number[] = [];
        const pviArr: number[] = [];
        for (let i = 0; i < rateHistory.length; i++) {
            const dt = i > 0 ? rateHistory[i].time - rateHistory[i-1].time : rateHistory[i].time;
            cumOil += Number(oilProd[i] ?? 0) * dt;
            cumInj += Math.max(0, Number(injection[i] ?? 0)) * dt;
            cumLiq += Math.max(0, Number(liquidProd[i] ?? 0)) * dt;
            cumWater += Math.max(0, Number(waterProd[i] ?? 0)) * dt;
            cumOilArr.push(cumOil);
            cumInjArr.push(cumInj);
            cumLiqArr.push(cumLiq);
            cumWaterArr.push(cumWater);
            pviArr.push(poreVolumeM3 > 1e-12 ? cumInj / poreVolumeM3 : 0);
        }
        return { cumOil: cumOilArr, cumInj: cumInjArr, cumLiq: cumLiqArr, cumWater: cumWaterArr, pvi: pviArr };
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
                const dt = i > 0 ? Math.max(0, rateHistory[i].time - rateHistory[i - 1].time) : Math.max(0, rateHistory[i].time);
                cum += (oi as number) * dt;
            }
            return cum;
        });
    });

    let recoveryFactor = $derived(cumulatives.cumOil.map(c => ooipM3 > 1e-12 ? Math.max(0, Math.min(1, c / ooipM3)) : null));

    // Diagnostic computations
    let avgPressure = $derived(rateHistory.map((p, idx) => {
        const v = avgReservoirPressureSeries?.[idx] ?? p.avg_reservoir_pressure ?? (p as any).average_reservoir_pressure ?? (p as any).avg_pressure;
        return Number.isFinite(v) ? v : null;
    }));
    let analyticalAvgPressure = $derived(rateHistory.map((_, idx) => {
        const v = (analyticalProductionData[idx] as any)?.avgPressure;
        return Number.isFinite(v) ? v : null;
    }));
    let avgWaterSat = $derived(rateHistory.map((p, idx) => {
        const v = avgWaterSaturationSeries?.[idx] ?? p.avg_water_saturation;
        return Number.isFinite(v) ? v : null;
    }));

    // VRR
    let vrrData = $derived.by(() => {
        let cumInjR = 0, cumProdR = 0;
        return rateHistory.map((p, idx) => {
            const dt = idx > 0 ? Math.max(0, rateHistory[idx].time - rateHistory[idx-1].time) : Math.max(0, rateHistory[idx].time);
            const injR = Number((p as any).total_injection_reservoir ?? p.total_injection);
            const prodR = Number((p as any).total_production_liquid_reservoir ?? p.total_production_liquid);
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
    let worSim = $derived(rateHistory.map((_, idx) => {
        const oil = Number(oilProd[idx]);
        const water = Number(waterProd[idx]);
        if (!Number.isFinite(oil) || oil <= 1e-12) return null;
        return Math.max(0, water / oil);
    }));
    let worAnalytical = $derived(rateHistory.map((_, idx) => {
        const oil = Number(analyticalProductionData[idx]?.oilRate);
        const water = Number(analyticalProductionData[idx]?.waterRate);
        if (!Number.isFinite(oil) || oil <= 1e-12 || !Number.isFinite(water)) return null;
        return Math.max(0, water / oil);
    }));

    // Water cut
    let waterCutSim = $derived(rateHistory.map((p, idx) => {
        const liquid = Number(p.total_production_liquid);
        if (!Number.isFinite(liquid) || liquid <= 1e-12) return 0;
        return Math.max(0, Math.min(1, waterProd[idx] / liquid));
    }));
    let waterCutAnalytical = $derived(rateHistory.map((_, idx) => {
        const oil = Number(analyticalProductionData[idx]?.oilRate);
        const water = Number(analyticalProductionData[idx]?.waterRate);
        const total = oil + water;
        if (!Number.isFinite(total) || total <= 1e-12) return null;
        return Math.max(0, Math.min(1, water / total));
    }));

    // MB Error
    let mbError = $derived(rateHistory.map(p => {
        const v = Number((p as any).material_balance_error_m3);
        return Number.isFinite(v) ? v : null;
    }));

    // Oil rate error
    let oilRateAbsError = $derived(oilProd.map((sim, idx) => {
        const analytical = Number(analyticalOilProd[idx]);
        const simVal = Number(sim);
        if (!Number.isFinite(analytical) || !Number.isFinite(simVal)) return null;
        return Math.abs(simVal - analytical);
    }));

    // Error stats
    $effect(() => {
        const validErrors = oilRateAbsError.filter(v => v !== null && Number.isFinite(v)) as number[];
        const percentErrors = oilProd.map((sim, idx) => {
            const analytical = Number(analyticalOilProd[idx]);
            const simVal = Number(sim);
            if (!Number.isFinite(analytical) || !Number.isFinite(simVal)) return null;
            return (Math.abs(simVal - analytical) / Math.max(Math.abs(analytical), 1e-9)) * 100;
        }).filter(v => v !== null && Number.isFinite(v)) as number[];

        if (validErrors.length > 0) {
            mismatchSummary = {
                pointsCompared: validErrors.length,
                mae: validErrors.reduce((a, v) => a + v, 0) / validErrors.length,
                rmse: Math.sqrt(validErrors.reduce((a, v) => a + v * v, 0) / validErrors.length),
                mape: percentErrors.length > 0 ? percentErrors.reduce((a, v) => a + v, 0) / percentErrors.length : 0,
            };
        } else {
            mismatchSummary = { pointsCompared: 0, mae: 0, rmse: 0, mape: 0 };
        }
    });

    // PVI availability
    let pviAvailable = $derived(cumulatives.cumInj[cumulatives.cumInj.length - 1] > 1e-12);

    // ══════════════════════════════════════════════════════════════
    //  X-AXIS VALUES (shared across panels)
    // ══════════════════════════════════════════════════════════════

    let xValues = $derived.by(() => {
        if (xAxisMode === 'pvi') return cumulatives.pvi;
        if (xAxisMode === 'cumLiquid') return cumulatives.cumLiq;
        if (xAxisMode === 'cumInjection') return cumulatives.cumInj;
        if (xAxisMode === 'logTime') return timeValues.map(t => t > 0 ? Math.log10(t) : null);
        return timeValues;
    });

    function setXAxisMode(mode: XAxisMode) {
        if (mode === 'pvi' && !pviAvailable) return;
        xAxisMode = mode;
    }

    function getXAxisTitle(): string {
        if (xAxisMode === 'pvi') return 'PV Injected (PVI)';
        if (xAxisMode === 'cumLiquid') return 'Cumulative Liquid Production (m³)';
        if (xAxisMode === 'cumInjection') return 'Cumulative Injection (m³)';
        if (xAxisMode === 'logTime') return 'Time (days) — log₁₀';
        return 'Time (days)';
    }

    // ══════════════════════════════════════════════════════════════
    //  PANEL CURVE CONFIGS + SERIES
    // ══════════════════════════════════════════════════════════════

    const ratesCurves: CurveConfig[] = [
        { label: 'Oil Rate', color: '#16a34a', borderWidth: 2.5, yAxisID: 'y' },
        { label: 'Oil Rate (Analytical)', color: '#15803d', borderWidth: 2, borderDash: [5, 5], yAxisID: 'y' },
        { label: 'Water Rate', color: '#1e3a8a', borderWidth: 2.5, yAxisID: 'y' },
        { label: 'Water Rate (Analytical)', color: '#3b82f6', borderWidth: 2, borderDash: [5, 5], yAxisID: 'y' },
        { label: 'Injection Rate', color: '#06b6d4', borderWidth: 2.5, yAxisID: 'y' },
        { label: 'Liquid Rate', color: '#2563eb', borderWidth: 2, yAxisID: 'y', defaultVisible: false },
        { label: 'Oil Rate Error', color: '#15803d', borderWidth: 1.3, borderDash: [2, 4], yAxisID: 'y', defaultVisible: false },
    ];

    const cumulativeCurves: CurveConfig[] = [
        { label: 'Cum Oil', color: '#0f5132', borderWidth: 2.5, yAxisID: 'y' },
        { label: 'Cum Oil (Analytical)', color: '#0f5132', borderWidth: 2, borderDash: [8, 4], yAxisID: 'y' },
        { label: 'Cum Injection', color: '#06b6d4', borderWidth: 2, yAxisID: 'y' },
        { label: 'Cum Water', color: '#1e3a8a', borderWidth: 2, yAxisID: 'y' },
        { label: 'Recovery Factor', color: '#22c55e', borderWidth: 2, yAxisID: 'y1' },
    ];

    const diagnosticsCurves: CurveConfig[] = [
        { label: 'Avg Pressure', color: '#dc2626', borderWidth: 2, yAxisID: 'y' },
        { label: 'Avg Pressure (Analytical)', color: '#f97316', borderWidth: 2, borderDash: [5, 5], yAxisID: 'y' },
        { label: 'VRR', color: '#7c3aed', borderWidth: 2.5, yAxisID: 'y1', defaultVisible: false },
        { label: 'WOR (Sim)', color: '#d97706', borderWidth: 2.3, yAxisID: 'y1', defaultVisible: false },
        { label: 'WOR (Analytical)', color: '#d97706', borderWidth: 2, borderDash: [5, 5], yAxisID: 'y1', defaultVisible: false },
        { label: 'Avg Water Sat', color: '#1d4ed8', borderWidth: 2, yAxisID: 'y1' },
        { label: 'Water Cut (Sim)', color: '#2563eb', borderWidth: 2.3, yAxisID: 'y1', defaultVisible: false },
        { label: 'Water Cut (Analytical)', color: '#1d4ed8', borderWidth: 2, borderDash: [6, 4], yAxisID: 'y1', defaultVisible: false },
        { label: 'MB Error', color: '#ef4444', borderWidth: 1.5, borderDash: [3, 3], yAxisID: 'y2', defaultVisible: false },
    ];

    // --- Build XY series for each panel ---
    let ratesSeries = $derived([
        toXYSeries(xValues, oilProd),
        toXYSeries(xValues, analyticalOilProd as Array<number | null>),
        toXYSeries(xValues, waterProd),
        toXYSeries(xValues, analyticalWaterRate as Array<number | null>),
        toXYSeries(xValues, injection),
        toXYSeries(xValues, liquidProd),
        toXYSeries(xValues, oilRateAbsError as Array<number | null>),
    ]);

    let cumulativeSeries = $derived([
        toXYSeries(xValues, cumulatives.cumOil),
        toXYSeries(xValues, analyticalCumOil as Array<number | null>),
        toXYSeries(xValues, cumulatives.cumInj),
        toXYSeries(xValues, cumulatives.cumWater),
        toXYSeries(xValues, recoveryFactor as Array<number | null>),
    ]);

    let diagnosticsSeries = $derived([
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

    // --- Scale configs for each panel ---
    const ratesScales = {
        y: { type: 'linear', display: true, position: 'left', min: 0, alignToPixels: true,
            title: { display: true, text: 'Rate (m³/day)' }, ticks: { count: 6 } },
    };
    const cumulativeScales = {
        y: { type: 'linear', display: true, position: 'left', min: 0, alignToPixels: true,
            title: { display: true, text: 'Cumulative (m³)' }, ticks: { count: 6 } },
        y1: { type: 'linear', display: true, position: 'right', min: 0, max: 1, alignToPixels: true,
            title: { display: true, text: 'Recovery Factor' },
            grid: { drawOnChartArea: false }, ticks: { count: 6 }, _fraction: true },
    };
    const diagnosticsScales = {
        y: { type: 'linear', display: true, position: 'left', alignToPixels: true,
            title: { display: true, text: 'Pressure (bar)' }, ticks: { count: 6 }, _auto: true },
        y1: { type: 'linear', display: true, position: 'right', min: 0, alignToPixels: true,
            title: { display: true, text: 'Fraction' },
            grid: { drawOnChartArea: false }, ticks: { count: 6 },
            _dynamicTitle: (labels: string[]) => {
                const parts: string[] = [];
                if (labels.some(l => l.includes('VRR'))) parts.push('VRR');
                if (labels.some(l => l.includes('WOR'))) parts.push('WOR');
                if (labels.some(l => l.includes('Sat'))) parts.push('Saturation');
                if (labels.some(l => l.includes('Cut'))) parts.push('Water Cut');
                return parts.length > 0 ? parts.join(' / ') : 'Fraction';
            } },
        y2: { type: 'linear', display: true, position: 'right', min: 0, alignToPixels: true,
            title: { display: true, text: 'MB Error (m³)' },
            grid: { drawOnChartArea: false }, ticks: { count: 6 } },
    };
</script>

<div class="space-y-1">
    <!-- X-axis controls at top -->
    <div class="flex items-center gap-2 mb-2">
        <span class="text-[11px] uppercase tracking-wide opacity-50 shrink-0">X-axis</span>
        <div class="relative flex items-center bg-base-100 border border-base-content/20 rounded-full shadow-sm hover:border-base-content/40 transition-colors cursor-pointer group overflow-hidden">
            <select
                id="x-axis-select"
                class="select select-xs bg-base-100 rounded-full text-base-content border-none focus:outline-none focus:ring-0 min-w-[180px] pl-3 pr-8 appearance-none cursor-pointer ![background-image:none]"
                bind:value={xAxisMode}
                onchange={(e) => setXAxisMode(e.currentTarget.value as XAxisMode)}
            >
                <option value="time">Time (days)</option>
                <option value="pvi" disabled={!pviAvailable}>PV Injected (PVI)</option>
                <option value="cumLiquid">Cumulative Liquid (m³)</option>
                <option value="cumInjection">Cumulative Injection (m³)</option>
                <option value="logTime">Log Time (Fetkovich)</option>
            </select>
            <div class="absolute right-0 h-full flex items-center justify-center w-6 bg-base-content/5 border-l border-base-content/20 rounded-r-full pointer-events-none">
                <svg class="w-3 h-3 opacity-70" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
                </svg>
            </div>
        </div>
    </div>

    <!-- Rates panel -->
    <ChartSubPanel
        panelId="rates"
        title="Rates"
        bind:expanded={ratesExpanded}
        curves={ratesCurves}
        seriesData={ratesSeries}
        scaleConfigs={ratesScales}
        {theme}
        bind:logScale
        allowLogToggle={true}
        targetLeftGutter={maxLeftGutter}
        targetRightGutter={maxRightGutter}
        onGutterMeasure={(left, right) => { nativeGutters = { ...nativeGutters, rates: { left, right } }; }}
    />

    <!-- Cumulative panel -->
    <ChartSubPanel
        panelId="cumulative"
        title="Cumulative"
        bind:expanded={cumulativeExpanded}
        curves={cumulativeCurves}
        seriesData={cumulativeSeries}
        scaleConfigs={cumulativeScales}
        {theme}
        logScale={false}
        targetLeftGutter={maxLeftGutter}
        targetRightGutter={maxRightGutter}
        onGutterMeasure={(left, right) => { nativeGutters = { ...nativeGutters, cumulative: { left, right } }; }}
    />

    <!-- Diagnostics panel -->
    <ChartSubPanel
        panelId="diagnostics"
        title="Diagnostics"
        bind:expanded={diagnosticsExpanded}
        curves={diagnosticsCurves}
        seriesData={diagnosticsSeries}
        scaleConfigs={diagnosticsScales}
        {theme}
        logScale={false}
        targetLeftGutter={maxLeftGutter}
        targetRightGutter={maxRightGutter}
        onGutterMeasure={(left, right) => { nativeGutters = { ...nativeGutters, diagnostics: { left, right } }; }}
    />

    <!-- Error stats -->
    {#if mismatchSummary.pointsCompared > 0}
    <div class="text-[11px] opacity-60 px-1">
        Analytical: {mismatchSummary.pointsCompared} pts ·
        MAE: {mismatchSummary.mae.toFixed(3)} ·
        RMSE: {mismatchSummary.rmse.toFixed(3)} ·
        MAPE: {mismatchSummary.mape.toFixed(2)}%
    </div>
    {/if}
</div>
