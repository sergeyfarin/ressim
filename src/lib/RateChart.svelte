<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Chart, registerables, type ChartDataset, type PointStyle } from 'chart.js';

    export let rateHistory = [];
    export let analyticalProductionData = [];
    export let avgReservoirPressureSeries: Array<number | null> = [];
    export let avgWaterSaturationSeries: Array<number | null> = [];
    export let ooipM3 = 0;
    export let poreVolumeM3 = 0;
    export let theme: 'dark' | 'light' = 'dark';

    type MismatchSummary = {
        pointsCompared: number;
        mae: number;
        rmse: number;
        mape: number;
    };

    type ChartTab = 'oil' | 'water' | 'voidage' | 'validation' | 'pvi' | 'custom';
    type LineDataset = ChartDataset<'line', Array<number | null>>;
    type AxisScaleConfig = {
        display?: boolean;
        min?: number;
        title?: { text?: string };
        grid?: { color?: string };
    };
    type ChartScalesMap = Record<string, AxisScaleConfig | undefined>;

    let chartCanvas: HTMLCanvasElement;
    let chart: Chart<'line', Array<number | null>, string>;
    let activeTab: ChartTab = 'oil';
    let selectedDatasetIndexes: number[] = [];
    let lineSelectorExpanded = false;
    // Reserved for future 3-phase extension: use red for gas-related series.
    const GAS_COLOR = '#ef4444';
    const OIL_COLOR = '#16a34a';
    const OIL_COLOR_DARK = '#15803d';
    const OIL_CUM_COLOR = '#0f5132';
    const WATER_PROD_COLOR = '#1e3a8a';
    const WATER_INJ_COLOR = '#06b6d4';
    const WATER_BALANCE_COLOR = '#2563eb';
    const PRESSURE_COLOR = '#dc2626';
    const VOIDAGE_COLOR = '#7c3aed';
    const SATURATION_COLOR = '#1d4ed8';
    const ERROR_GREEN_COLOR = '#15803d';
    const DATASET_INDEX = {
        OIL_RATE: 0,
        ANALYTICAL_OIL_RATE: 1,
        WATER_PROD: 2,
        WATER_INJ: 3,
        CUM_OIL: 4,
        ANALYTICAL_CUM_OIL: 5,
        RECOVERY_FACTOR: 6,
        LIQUID_PROD: 7,
        VRR: 8,
        AVG_PRESSURE: 9,
        AVG_WATER_SAT: 10,
        OIL_RATE_ABS_ERROR: 11,
        RF_VS_PVI: 12,
        WATERCUT_SIM_VS_PVI: 13,
        WATERCUT_ANALYTICAL_VS_PVI: 14,
    } as const;
    let mismatchSummary: MismatchSummary = {
        pointsCompared: 0,
        mae: 0,
        rmse: 0,
        mape: 0,
    };

    $: if (rateHistory && chart) {
        updateChart();
    }

    $: if (analyticalProductionData && chart) {
        updateChart();
    }

    $: if (avgReservoirPressureSeries && chart) {
        updateChart();
    }

    $: if (avgWaterSaturationSeries && chart) {
        updateChart();
    }

    $: if (chart && activeTab) {
        applyActiveTab();
    }

    $: if (activeTab === 'custom') {
        lineSelectorExpanded = true;
    }

    $: if (chart && theme) {
        applyThemeStyles();
    }

    const tabLabels: Record<ChartTab, string> = {
        oil: 'Oil Rate + Cumulative',
        water: 'Water Injection + Production',
        voidage: 'Voidage / Liquid Balance',
        validation: 'Model vs Analytical',
        pvi: 'RF + Water Cut vs PVI',
        custom: 'Custom',
    };

    const presetSelections: Record<'oil' | 'water' | 'voidage' | 'validation' | 'pvi', number[]> = {
        oil: [DATASET_INDEX.OIL_RATE, DATASET_INDEX.CUM_OIL, DATASET_INDEX.RECOVERY_FACTOR],
        water: [DATASET_INDEX.WATER_PROD, DATASET_INDEX.WATER_INJ, DATASET_INDEX.AVG_WATER_SAT],
        voidage: [DATASET_INDEX.AVG_PRESSURE, DATASET_INDEX.VRR, DATASET_INDEX.LIQUID_PROD],
        validation: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.ANALYTICAL_OIL_RATE,
            DATASET_INDEX.CUM_OIL,
            DATASET_INDEX.ANALYTICAL_CUM_OIL,
            DATASET_INDEX.OIL_RATE_ABS_ERROR,
        ],
        pvi: [
            DATASET_INDEX.RF_VS_PVI,
            DATASET_INDEX.WATERCUT_SIM_VS_PVI,
            DATASET_INDEX.WATERCUT_ANALYTICAL_VS_PVI,
        ],
    };

    function legendAxisPriority(axisId: string | undefined): number {
        if (!axisId) return 99;
        if (axisId === 'y') return 0;
        if (axisId === 'y1' || axisId === 'y2') return 1;
        if (axisId === 'y3' || axisId === 'y4') return 2;
        return 99;
    }

    function buildLegendLineSwatch(color: string, width: number, dash: number[]): PointStyle {
        if (typeof document === 'undefined') return 'rect';
        const canvas = document.createElement('canvas');
        canvas.width = 36;
        canvas.height = 10;
        const ctx = canvas.getContext('2d');
        if (!ctx) return 'rect';

        ctx.clearRect(0, 0, canvas.width, canvas.height);
        ctx.strokeStyle = color;
        ctx.lineWidth = Math.max(1, width);
        ctx.setLineDash(Array.isArray(dash) ? dash : []);
        ctx.lineCap = 'butt';
        const y = Math.floor(canvas.height / 2) + 0.5;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(canvas.width, y);
        ctx.stroke();

        return canvas;
    }

    function getLineDataset(datasetIndex: number): LineDataset | undefined {
        return chart?.data.datasets?.[datasetIndex] as LineDataset | undefined;
    }

    function getScalesMap(): ChartScalesMap {
        return (chart.options.scales ?? {}) as ChartScalesMap;
    }

    onMount(() => {
        Chart.register(...registerables);
        const ctx = chartCanvas.getContext('2d');
        chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [
                    {
                        label: 'Oil Production (m³/day)',
                        data: [],
                        borderColor: OIL_COLOR,
                        borderWidth: 2.5,
                        yAxisID: 'y',
                    },
                    {
                        label: 'Analytical Oil Production (m³/day)',
                        data: [],
                        borderColor: OIL_COLOR_DARK,
                        borderWidth: 2,
                        borderDash: [5, 5],
                        yAxisID: 'y',
                    },
                    {
                        label: 'Water Production (m³/day)',
                        data: [],
                        borderColor: WATER_PROD_COLOR,
                        borderWidth: 2.5,
                        yAxisID: 'y',
                    },
                    {
                        label: 'Water Injection (m³/day)',
                        data: [],
                        borderColor: WATER_INJ_COLOR,
                        borderWidth: 2.5,
                        yAxisID: 'y',
                    },
                    {
                        label: 'Cumulative Oil (m³)',
                        data: [],
                        borderColor: OIL_CUM_COLOR,
                        borderWidth: 2.5,
                        yAxisID: 'y1',
                    },
                    {
                        label: 'Analytical Cumulative Oil (m³)',
                        data: [],
                        borderColor: OIL_CUM_COLOR,
                        borderWidth: 2,
                        borderDash: [8, 4],
                        yAxisID: 'y1',
                    },
                    {
                        label: 'Recovery Factor',
                        data: [],
                        borderColor: '#22c55e',
                        borderWidth: 2,
                        yAxisID: 'y5',
                    },
                    {
                        label: 'Liquid Production (m³/day)',
                        data: [],
                        borderColor: WATER_BALANCE_COLOR,
                        borderWidth: 2,
                        yAxisID: 'y4',
                    },
                    {
                        label: 'Cumulative Voidage Replacement Ratio',
                        data: [],
                        borderColor: VOIDAGE_COLOR,
                        borderWidth: 2.5,
                        tension: 0,
                        cubicInterpolationMode: 'monotone',
                        yAxisID: 'y2',
                    },
                    {
                        label: 'Average Reservoir Pressure (bar)',
                        data: [],
                        borderColor: PRESSURE_COLOR,
                        borderWidth: 2,
                        yAxisID: 'y',
                    },
                    {
                        label: 'Average Water Saturation',
                        data: [],
                        borderColor: SATURATION_COLOR,
                        borderWidth: 2,
                        yAxisID: 'y5',
                    },
                    {
                        label: 'Oil Rate Abs Error (m³/day)',
                        data: [],
                        borderColor: ERROR_GREEN_COLOR,
                        borderWidth: 1.3,
                        borderDash: [2, 4],
                        yAxisID: 'y',
                    },
                    {
                        label: 'Recovery Factor vs PVI',
                        data: [],
                        borderColor: '#16a34a',
                        borderWidth: 2.3,
                        yAxisID: 'y5',
                    },
                    {
                        label: 'Water Cut (Sim) vs PVI',
                        data: [],
                        borderColor: '#2563eb',
                        borderWidth: 2.3,
                        yAxisID: 'y5',
                    },
                    {
                        label: 'Water Cut (Analytical) vs PVI',
                        data: [],
                        borderColor: '#1d4ed8',
                        borderWidth: 2,
                        borderDash: [6, 4],
                        yAxisID: 'y5',
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        labels: {
                            filter: (legendItem) => {
                                return !legendItem.hidden;
                            },
                            generateLabels: (chartRef) => {
                                const defaultLabels = Chart.defaults.plugins.legend.labels.generateLabels(chartRef);
                                return defaultLabels.map((label) => {
                                    const datasetIndex = label.datasetIndex ?? -1;
                                    const dataset = getLineDataset(datasetIndex);
                                    const borderWidth = Number(dataset?.borderWidth ?? label.lineWidth ?? 2);
                                    const borderColor = Array.isArray(dataset?.borderColor)
                                        ? String(dataset.borderColor[0] ?? label.strokeStyle)
                                        : String(dataset?.borderColor ?? label.strokeStyle);
                                    const borderDash = Array.isArray(dataset?.borderDash)
                                        ? dataset.borderDash.map((segment) => Number(segment))
                                        : (Array.isArray(label.lineDash) ? label.lineDash : []);

                                    return {
                                        ...label,
                                        fillStyle: 'rgba(0,0,0,0)',
                                        strokeStyle: borderColor,
                                        lineWidth: Math.max(1, borderWidth),
                                        pointStyle: buildLegendLineSwatch(borderColor, borderWidth, borderDash),
                                    };
                                });
                            },
                            sort: (a, b, data) => {
                                const aIndex = a.datasetIndex ?? Number.MAX_SAFE_INTEGER;
                                const bIndex = b.datasetIndex ?? Number.MAX_SAFE_INTEGER;
                                const aAxisId = (data.datasets?.[aIndex] as LineDataset | undefined)?.yAxisID;
                                const bAxisId = (data.datasets?.[bIndex] as LineDataset | undefined)?.yAxisID;
                                const priorityDelta = legendAxisPriority(aAxisId) - legendAxisPriority(bAxisId);
                                if (priorityDelta !== 0) return priorityDelta;
                                return aIndex - bIndex;
                            },
                            boxWidth: 36,
                            boxHeight: 1,
                            usePointStyle: true,
                            pointStyleWidth: 36,
                        }
                    },
                    tooltip: {
                        callbacks: {
                            label: (context) => {
                                const datasetLabel = context.dataset.label ?? '';
                                const rawValue = context.parsed?.y;
                                if (!Number.isFinite(rawValue)) {
                                    return datasetLabel;
                                }

                                if (context.datasetIndex === DATASET_INDEX.VRR) {
                                    return `${datasetLabel}: ${Number(rawValue).toFixed(4)}`;
                                }

                                return `${datasetLabel}: ${Number(rawValue).toFixed(3)}`;
                            },
                        },
                    }
                },
                scales: {
                    x: {
                        title: {
                            display: true,
                            text: 'Time (days)'
                        }
                    },
                    y: {
                        type: 'linear',
                        display: true,
                        position: 'left',
                        min: 0,
                        alignToPixels: true,
                        title: {
                            display: true,
                            text: 'Rate (m³/day)'
                        },
                        ticks: {
                            count: 6,
                        },
                    },
                    y1: {
                        type: 'linear',
                        display: true,
                        position: 'right',
                        min: 0,
                        title: {
                            display: true,
                            text: 'Cumulative Oil (m³)'
                        },
                        grid: {
                            drawOnChartArea: false, // only draw grid for first Y axis
                        },
                    },
                    y2: {
                        type: 'linear',
                        display: true,
                        position: 'right',
                        alignToPixels: true,
                        title: {
                            display: true,
                            text: 'Voidage Replacement Ratio'
                        },
                        grid: {
                            drawOnChartArea: false,
                        },
                        min: 0,
                        ticks: {
                            count: 6,
                        },
                    },
                    y4: {
                        type: 'linear',
                        display: true,
                        position: 'right',
                        offset: true,
                        min: 0,
                        alignToPixels: true,
                        title: {
                            display: true,
                            text: 'Liquid Production (m³/day)'
                        },
                        grid: {
                            drawOnChartArea: false,
                        },
                        ticks: {
                            count: 6,
                        }
                    },
                    y5: {
                        type: 'linear',
                        display: true,
                        position: 'right',
                        offset: true,
                        min: 0,
                        max: 1,
                        alignToPixels: true,
                        title: {
                            display: true,
                            text: 'Fraction'
                        },
                        grid: {
                            drawOnChartArea: false,
                        },
                        ticks: {
                            count: 6,
                        }
                    }
                }
            }
        });

        selectedDatasetIndexes = [...presetSelections.oil];
        applyActiveTab();
    });

    onDestroy(() => {
        chart?.destroy();
    });

    function updateChart() {
        if (!chart || !rateHistory || rateHistory.length === 0) return;

        const timeLabels = rateHistory.map(p => p.time.toFixed(2));
        const oilProd = rateHistory.map(p => p.total_production_oil);
        const liquidProd = rateHistory.map(p => p.total_production_liquid);
        const injection = rateHistory.map(p => p.total_injection);
        const waterProd = liquidProd.map((qL, idx) => Math.max(0, qL - oilProd[idx]));

        const analyticalOilProd = rateHistory.map((_, idx) => {
            const value = analyticalProductionData[idx]?.oilRate;
            return Number.isFinite(value) ? value : null;
        });

        const analyticalCumulativeOilData = rateHistory.map((point, idx) => {
            const explicitValue = analyticalProductionData[idx]?.cumulativeOil;
            if (Number.isFinite(explicitValue)) return explicitValue;

            const analyticalOil = analyticalOilProd[idx];
            if (!Number.isFinite(analyticalOil)) return null;

            let runningCum = 0;
            for (let i = 0; i <= idx; i++) {
                const oi = analyticalOilProd[i];
                if (!Number.isFinite(oi)) return null;
                const dt = i > 0 ? Math.max(0, rateHistory[i].time - rateHistory[i - 1].time) : Math.max(0, rateHistory[i].time);
                runningCum += (oi as number) * dt;
            }
            return runningCum;
        });

        const avgReservoirPressure = rateHistory.map((point, idx) => {
            const seriesValue = avgReservoirPressureSeries?.[idx];
            const value = seriesValue ?? point.avg_reservoir_pressure ?? point.average_reservoir_pressure ?? point.avg_pressure;
            return Number.isFinite(value) ? value : null;
        });

        const avgWaterSat = rateHistory.map((_, idx) => {
            const value = avgWaterSaturationSeries?.[idx];
            return Number.isFinite(value) ? value : null;
        });
        
        // Calculate cumulative oil production
        let cumulativeOil = 0;
        const cumulativeOilData = [];
        let cumulativeInjection = 0;
        const pviData = [];
        for (let i = 0; i < rateHistory.length; i++) {
            const dt = i > 0 ? rateHistory[i].time - rateHistory[i-1].time : rateHistory[i].time;
            cumulativeOil += oilProd[i] * dt;
            cumulativeOilData.push(cumulativeOil);
            cumulativeInjection += Math.max(0, injection[i]) * dt;
            if (!Number.isFinite(poreVolumeM3) || poreVolumeM3 <= 1e-12) {
                pviData.push(0);
            } else {
                pviData.push(cumulativeInjection / poreVolumeM3);
            }
        }

        const recoveryFactorData = cumulativeOilData.map((cumOil) => {
            if (!Number.isFinite(ooipM3) || ooipM3 <= 1e-12) return null;
            return Math.max(0, Math.min(1, cumOil / ooipM3));
        });

        const waterCutSimVsPvi = rateHistory.map((point, idx) => {
            const liquid = Number(point.total_production_liquid);
            if (!Number.isFinite(liquid) || liquid <= 1e-12) return 0;
            const waterCut = Math.max(0, Math.min(1, waterProd[idx] / liquid));
            return waterCut;
        });

        const waterCutAnalyticalVsPvi = rateHistory.map((_, idx) => {
            const oil = Number(analyticalProductionData[idx]?.oilRate);
            const water = Number(analyticalProductionData[idx]?.waterRate);
            const total = oil + water;
            if (!Number.isFinite(total) || total <= 1e-12) return null;
            return Math.max(0, Math.min(1, water / total));
        });

        // Calculate cumulative VRR using reservoir-condition rates (aligns with pressure support physics)
        let cumulativeInjReservoir = 0;
        let cumulativeProdReservoir = 0;
        const vrrData = rateHistory.map((p, idx) => {
            const dt = idx > 0 ? Math.max(0, Number(rateHistory[idx].time) - Number(rateHistory[idx - 1].time)) : Math.max(0, Number(rateHistory[idx].time));
            const injectedReservoir = Number(p.total_injection_reservoir ?? p.total_injection);
            const producedReservoir = Number(p.total_production_liquid_reservoir ?? p.total_production_liquid);

            if (dt > 0 && Number.isFinite(injectedReservoir) && Number.isFinite(producedReservoir)) {
                cumulativeInjReservoir += Math.max(0, injectedReservoir) * dt;
                cumulativeProdReservoir += Math.max(0, producedReservoir) * dt;
            }

            if (cumulativeProdReservoir <= 1e-12) return null;
            const rawVrr = cumulativeInjReservoir / cumulativeProdReservoir;
            return Math.abs(rawVrr - 1.0) < 1e-9 ? 1.0 : rawVrr;
        });

        const absErrorData = oilProd.map((simValue, idx) => {
            const analyticalValue = analyticalOilProd[idx];
            if (analyticalValue === null || !Number.isFinite(analyticalValue)) return null;
            return Math.abs(simValue - analyticalValue);
        });

        const percentErrorData = oilProd.map((simValue, idx) => {
            const analyticalValue = analyticalOilProd[idx];
            if (analyticalValue === null || !Number.isFinite(analyticalValue)) return null;
            const denominator = Math.max(Math.abs(analyticalValue), 1e-9);
            return (Math.abs(simValue - analyticalValue) / denominator) * 100.0;
        });

        const validAbsErrors = absErrorData.filter(v => v !== null && Number.isFinite(v)) as number[];
        const validPercentErrors = percentErrorData.filter(v => v !== null && Number.isFinite(v)) as number[];

        if (validAbsErrors.length > 0) {
            const mae = validAbsErrors.reduce((acc, v) => acc + v, 0) / validAbsErrors.length;
            const rmse = Math.sqrt(validAbsErrors.reduce((acc, v) => acc + (v * v), 0) / validAbsErrors.length);
            const mape = validPercentErrors.length > 0
                ? validPercentErrors.reduce((acc, v) => acc + v, 0) / validPercentErrors.length
                : 0;

            mismatchSummary = {
                pointsCompared: validAbsErrors.length,
                mae,
                rmse,
                mape,
            };
        } else {
            mismatchSummary = {
                pointsCompared: 0,
                mae: 0,
                rmse: 0,
                mape: 0,
            };
        }

        const pviLabels = pviData.map((value) => value.toFixed(3));
        chart.data.labels = activeTab === 'pvi' ? pviLabels : timeLabels;
        chart.data.datasets[0].data = oilProd;
        chart.data.datasets[1].data = analyticalOilProd;
        chart.data.datasets[2].data = waterProd;
        chart.data.datasets[3].data = injection;
        chart.data.datasets[4].data = cumulativeOilData;
        chart.data.datasets[5].data = analyticalCumulativeOilData;
        chart.data.datasets[6].data = recoveryFactorData;
        chart.data.datasets[7].data = liquidProd;
        chart.data.datasets[8].data = vrrData;
        chart.data.datasets[9].data = avgReservoirPressure;
        chart.data.datasets[10].data = avgWaterSat;
        chart.data.datasets[11].data = absErrorData;
        chart.data.datasets[12].data = recoveryFactorData;
        chart.data.datasets[13].data = waterCutSimVsPvi;
        chart.data.datasets[14].data = waterCutAnalyticalVsPvi;

        const showPoints = timeLabels.length <= 20;
        for (const dataset of chart.data.datasets) {
            const lineDataset = dataset as LineDataset;
            lineDataset.pointRadius = showPoints ? 2 : 0;
            lineDataset.pointHoverRadius = showPoints ? 3 : 4;
            lineDataset.pointHitRadius = 8;
        }

        chart.update();
    }

    function setDatasetVisibility(visibleIndexes: number[]) {
        if (!chart) return;
        chart.data.datasets.forEach((_, idx) => {
            const show = visibleIndexes.includes(idx);
            chart.setDatasetVisibility(idx, show);
        });
    }

    function applyThemeStyles() {
        if (!chart) return;
        const scales = getScalesMap();
        const gridColor = theme === 'dark' ? 'rgba(203, 213, 225, 0.07)' : 'rgba(15, 23, 42, 0.10)';

        if (scales.x?.grid) scales.x.grid.color = gridColor;
        if (scales.y?.grid) scales.y.grid.color = gridColor;
        if (scales.y1?.grid) scales.y1.grid.color = gridColor;
        if (scales.y2?.grid) scales.y2.grid.color = gridColor;
        if (scales.y4?.grid) scales.y4.grid.color = gridColor;
        if (scales.y5?.grid) scales.y5.grid.color = gridColor;

        chart.update();
    }

    function applySelection(visibleIndexes: number[]) {
        if (!chart) return;
        setDatasetVisibility(visibleIndexes);

        const scales = getScalesMap();
        const activeAxisIds = new Set(
            visibleIndexes
                .map((idx) => (chart.data.datasets[idx] as LineDataset | undefined)?.yAxisID)
                .filter((axisId): axisId is string => Boolean(axisId))
        );

        if (scales.y) scales.y.display = activeAxisIds.has('y');
        if (scales.y1) scales.y1.display = activeAxisIds.has('y1');
        if (scales.y2) scales.y2.display = activeAxisIds.has('y2');
        if (scales.y4) scales.y4.display = activeAxisIds.has('y4');
        if (scales.y5) scales.y5.display = activeAxisIds.has('y5');

        if (scales.y?.title) {
            if (visibleIndexes.includes(DATASET_INDEX.AVG_PRESSURE)) {
                scales.y.title.text = 'Average Reservoir Pressure (bar)';
                if (scales.y) delete scales.y.min;
            } else {
                scales.y.title.text = 'Rate (m³/day)';
                if (scales.y) scales.y.min = 0;
            }
        }

        if (scales.y5?.title) {
            if (visibleIndexes.includes(DATASET_INDEX.AVG_WATER_SAT)) {
                scales.y5.title.text = 'Average Water Saturation';
            } else if (
                visibleIndexes.includes(DATASET_INDEX.WATERCUT_SIM_VS_PVI)
                || visibleIndexes.includes(DATASET_INDEX.WATERCUT_ANALYTICAL_VS_PVI)
            ) {
                scales.y5.title.text = 'Fraction (RF / Water Cut)';
            } else {
                scales.y5.title.text = 'Recovery Factor';
            }
        }

        if (scales.x?.title) {
            scales.x.title.text = activeTab === 'pvi' ? 'PV Injected (PVI)' : 'Time (days)';
        }

        chart.update();
    }

    function applyActiveTab() {
        if (!chart) return;

        if (activeTab !== 'custom') {
            selectedDatasetIndexes = [...presetSelections[activeTab]];
        }
        updateChart();
        applySelection(selectedDatasetIndexes);
    }

    function toggleDataset(datasetIndex: number) {
        const next = new Set(selectedDatasetIndexes);
        if (next.has(datasetIndex)) {
            next.delete(datasetIndex);
        } else {
            next.add(datasetIndex);
        }

        selectedDatasetIndexes = [...next].sort((a, b) => a - b);
        activeTab = 'custom';
        applySelection(selectedDatasetIndexes);
    }

    function toggleLineSelector() {
        lineSelectorExpanded = !lineSelectorExpanded;
    }
</script>

<div class="mb-2 flex flex-wrap gap-2">
    {#each Object.entries(tabLabels) as [key, label]}
        <button
            type="button"
            class={`btn btn-xs sm:btn-sm ${activeTab === key ? 'btn-primary' : 'btn-outline'}`}
            on:click={() => activeTab = key as typeof activeTab}
        >
            {label}
        </button>
    {/each}
</div>

<div class="mb-2">
    <button
        type="button"
        class="btn btn-xs btn-outline"
        on:click={toggleLineSelector}
    >
        {lineSelectorExpanded ? 'Hide line selection' : 'Show line selection'}
    </button>
</div>

{#if lineSelectorExpanded}
    <div class="mb-2 flex flex-wrap gap-1">
        {#if chart}
            {#each chart.data.datasets as dataset, idx}
                <button
                    type="button"
                    class={`btn btn-xs ${selectedDatasetIndexes.includes(idx) ? 'btn-secondary' : 'btn-ghost'}`}
                    on:click={() => toggleDataset(idx)}
                >
                    {dataset.label}
                </button>
            {/each}
        {/if}
    </div>
{/if}

<div class="chart-container" style="position: relative; height: min(52vh, 440px); width:100%;">
    <canvas bind:this={chartCanvas}></canvas>
</div>

<div style="margin-top: 0.5rem; font-size: 12px; color: #555; text-align: left;">
    <div>Analytical points compared: {mismatchSummary.pointsCompared}</div>
    <div>
        MAE: {mismatchSummary.mae.toFixed(3)} m³/day ·
        RMSE: {mismatchSummary.rmse.toFixed(3)} m³/day ·
        MAPE: {mismatchSummary.mape.toFixed(2)}%
    </div>
</div>
