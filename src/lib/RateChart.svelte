<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Chart, registerables, type ChartDataset, type PointStyle } from 'chart.js';
    import { getLineDataset, getDatasetLabel, safeSetDatasetData, applyThemeToChart } from './chart-helpers';
    import type { RateHistoryPoint, AnalyticalProductionPoint } from './simulator-types';

    export let rateHistory: RateHistoryPoint[] = [];

    export let analyticalProductionData: AnalyticalProductionPoint[] = [];
    export let avgReservoirPressureSeries: Array<number | null> = [];
    export let avgWaterSaturationSeries: Array<number | null> = [];
    export let ooipM3: number = 0;
    export let poreVolumeM3: number = 0;
    export let activeCategory: string = '';
    export let activeCase: string = '';
    export let theme: 'dark' | 'light' = 'dark';

    type MismatchSummary = {
        pointsCompared: number;
        mae: number;
        rmse: number;
        mape: number;
    };

    type XAxisMode = 'time' | 'pvi' | 'cumLiquid' | 'cumInjection';
    type XYPoint = { x: number; y: number | null };

    type LineDataset = ChartDataset<'line', Array<number | null | XYPoint>>;
    type AxisScaleConfig = {
        display?: boolean;
        min?: number;
        max?: number;
        title?: { text?: string };
        grid?: { color?: string };
    };
    type ChartScalesMap = Record<string, AxisScaleConfig | undefined>;

    let chartCanvas: HTMLCanvasElement;
    let chart: Chart<'line', XYPoint[], number>;
    let selectedDatasetIndexes: number[] = [];
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

    let xAxisMode: XAxisMode = 'time';
    let pviAvailable = false;
    let pendingScenarioXAxisMode: XAxisMode | null = null;
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

    $: if (chart && theme) {
        applyThemeToChart(chart, theme);
    }

    const scenarioSelectionByCategory: Record<string, number[]> = {
        depletion: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.ANALYTICAL_OIL_RATE,
            DATASET_INDEX.CUM_OIL,
            DATASET_INDEX.ANALYTICAL_CUM_OIL,
            DATASET_INDEX.OIL_RATE_ABS_ERROR,
            DATASET_INDEX.AVG_PRESSURE,
        ],
        waterflood: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.WATER_PROD,
            DATASET_INDEX.WATER_INJ,
            DATASET_INDEX.CUM_OIL,
            DATASET_INDEX.RECOVERY_FACTOR,
            DATASET_INDEX.AVG_WATER_SAT,
        ],
        exploration: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.WATER_PROD,
            DATASET_INDEX.WATER_INJ,
            DATASET_INDEX.LIQUID_PROD,
            DATASET_INDEX.VRR,
            DATASET_INDEX.AVG_PRESSURE,
        ],
    };

    const scenarioSelectionByCase: Record<string, number[]> = {
        depletion_corner_producer: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.ANALYTICAL_OIL_RATE,
            DATASET_INDEX.CUM_OIL,
            DATASET_INDEX.ANALYTICAL_CUM_OIL,
            DATASET_INDEX.OIL_RATE_ABS_ERROR,
            DATASET_INDEX.AVG_PRESSURE,
        ],
        depletion_center_producer: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.ANALYTICAL_OIL_RATE,
            DATASET_INDEX.CUM_OIL,
            DATASET_INDEX.ANALYTICAL_CUM_OIL,
            DATASET_INDEX.OIL_RATE_ABS_ERROR,
            DATASET_INDEX.AVG_PRESSURE,
        ],
        depletion_custom_subcase: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.ANALYTICAL_OIL_RATE,
            DATASET_INDEX.CUM_OIL,
            DATASET_INDEX.ANALYTICAL_CUM_OIL,
            DATASET_INDEX.OIL_RATE_ABS_ERROR,
            DATASET_INDEX.AVG_PRESSURE,
        ],
        bl_case_a_refined: [
            DATASET_INDEX.RF_VS_PVI,
            DATASET_INDEX.WATERCUT_SIM_VS_PVI,
            DATASET_INDEX.WATERCUT_ANALYTICAL_VS_PVI,
        ],
        bl_case_b_refined: [
            DATASET_INDEX.RF_VS_PVI,
            DATASET_INDEX.WATERCUT_SIM_VS_PVI,
            DATASET_INDEX.WATERCUT_ANALYTICAL_VS_PVI,
        ],
        waterflood_custom_subcase: [
            DATASET_INDEX.RF_VS_PVI,
            DATASET_INDEX.WATERCUT_SIM_VS_PVI,
            DATASET_INDEX.WATERCUT_ANALYTICAL_VS_PVI,
        ],
        bl_aligned_homogeneous: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.WATER_PROD,
            DATASET_INDEX.WATER_INJ,
            DATASET_INDEX.CUM_OIL,
            DATASET_INDEX.RECOVERY_FACTOR,
            DATASET_INDEX.AVG_WATER_SAT,
        ],
        bl_aligned_mild_capillary: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.WATER_PROD,
            DATASET_INDEX.WATER_INJ,
            DATASET_INDEX.AVG_WATER_SAT,
            DATASET_INDEX.AVG_PRESSURE,
            DATASET_INDEX.VRR,
        ],
        bl_aligned_mobility_balanced: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.WATER_PROD,
            DATASET_INDEX.WATER_INJ,
            DATASET_INDEX.VRR,
            DATASET_INDEX.RECOVERY_FACTOR,
            DATASET_INDEX.AVG_WATER_SAT,
        ],
        baseline_waterflood: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.WATER_PROD,
            DATASET_INDEX.WATER_INJ,
            DATASET_INDEX.LIQUID_PROD,
            DATASET_INDEX.VRR,
            DATASET_INDEX.AVG_PRESSURE,
        ],
        high_contrast_layers: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.WATER_PROD,
            DATASET_INDEX.WATER_INJ,
            DATASET_INDEX.AVG_PRESSURE,
            DATASET_INDEX.AVG_WATER_SAT,
            DATASET_INDEX.VRR,
        ],
        viscous_fingering_risk: [
            DATASET_INDEX.OIL_RATE,
            DATASET_INDEX.WATER_PROD,
            DATASET_INDEX.WATER_INJ,
            DATASET_INDEX.LIQUID_PROD,
            DATASET_INDEX.AVG_WATER_SAT,
            DATASET_INDEX.VRR,
        ],
    };

    const fallbackSelection: number[] = [
        DATASET_INDEX.OIL_RATE,
        DATASET_INDEX.WATER_PROD,
        DATASET_INDEX.WATER_INJ,
        DATASET_INDEX.CUM_OIL,
        DATASET_INDEX.AVG_PRESSURE,
    ];

    let lastScenarioSelectionKey = '';

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



    function getScalesMap(): ChartScalesMap {
        return (chart.options.scales ?? {}) as ChartScalesMap;
    }

    function getXAxisTitle(mode: XAxisMode): string {
        if (mode === 'pvi') return 'PV Injected (PVI)';
        if (mode === 'cumLiquid') return 'Cumulative Liquid Production (m³)';
        if (mode === 'cumInjection') return 'Cumulative Injection (m³)';
        return 'Time (days)';
    }

    function setXAxisMode(mode: XAxisMode) {
        pendingScenarioXAxisMode = null;
        if (mode === 'pvi' && !pviAvailable) return;
        xAxisMode = mode;
        updateChart();
    }

    function resolveScenarioDefaultXAxis(caseKey: string): XAxisMode {
        const normalizedCase = String(caseKey ?? '').toLowerCase();
        if (normalizedCase.startsWith('bl_') || normalizedCase === 'waterflood_custom_subcase') {
            return 'pvi';
        }
        return 'time';
    }

    function resolveScenarioSelection(category: string, caseKey: string): number[] {
        const normalizedCategory = String(category ?? '').toLowerCase();
        const normalizedCase = String(caseKey ?? '').toLowerCase();

        const byCase = scenarioSelectionByCase[normalizedCase];
        if (Array.isArray(byCase) && byCase.length > 0) {
            return [...byCase];
        }

        const byCategory = scenarioSelectionByCategory[normalizedCategory];
        if (Array.isArray(byCategory) && byCategory.length > 0) {
            return [...byCategory];
        }

        return [...fallbackSelection];
    }

    function applyScenarioSelection() {
        if (!chart) return;
        const selection = resolveScenarioSelection(activeCategory, activeCase)
            .filter((idx) => idx >= 0 && idx < chart.data.datasets.length);
        selectedDatasetIndexes = [...new Set(selection)].sort((a, b) => a - b);
        pendingScenarioXAxisMode = resolveScenarioDefaultXAxis(activeCase);
        if (pendingScenarioXAxisMode !== 'pvi') {
            xAxisMode = pendingScenarioXAxisMode;
            pendingScenarioXAxisMode = null;
        }
        updateChart();
    }

    onMount(() => {
        Chart.register(...registerables);
        const ctx = chartCanvas?.getContext('2d');
        if (!ctx) return;
        chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [
                    {
                        label: 'Oil Rate',
                        data: [],
                        borderColor: OIL_COLOR,
                        borderWidth: 2.5,
                        yAxisID: 'y',
                    },
                    {
                        label: 'Oil Rate (Analytical)',
                        data: [],
                        borderColor: OIL_COLOR_DARK,
                        borderWidth: 2,
                        borderDash: [5, 5],
                        yAxisID: 'y',
                    },
                    {
                        label: 'Water Rate',
                        data: [],
                        borderColor: WATER_PROD_COLOR,
                        borderWidth: 2.5,
                        yAxisID: 'y',
                    },
                    {
                        label: 'Water Injection Rate',
                        data: [],
                        borderColor: WATER_INJ_COLOR,
                        borderWidth: 2.5,
                        yAxisID: 'y',
                    },
                    {
                        label: 'Cumulative Oil',
                        data: [],
                        borderColor: OIL_CUM_COLOR,
                        borderWidth: 2.5,
                        yAxisID: 'y1',
                    },
                    {
                        label: 'Cumulative Oil (Rate)',
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
                        label: 'Liquid Production',
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
                        label: 'Average Reservoir Pressure',
                        data: [],
                        borderColor: PRESSURE_COLOR,
                        borderWidth: 2,
                        yAxisID: 'y3',
                    },
                    {
                        label: 'Average Water Saturation',
                        data: [],
                        borderColor: SATURATION_COLOR,
                        borderWidth: 2,
                        yAxisID: 'y5',
                    },
                    {
                        label: 'Oil Rate (Difference vs. Analytical)',
                        data: [],
                        borderColor: ERROR_GREEN_COLOR,
                        borderWidth: 1.3,
                        borderDash: [2, 4],
                        yAxisID: 'y',
                    },
                    {
                        label: 'Recovery Factor',
                        data: [],
                        borderColor: '#16a34a',
                        borderWidth: 2.3,
                        yAxisID: 'y5',
                    },
                    {
                        label: 'Water Cut',
                        data: [],
                        borderColor: '#2563eb',
                        borderWidth: 2.3,
                        yAxisID: 'y5',
                    },
                    {
                        label: 'Water Cut (Analytical)',
                        data: [],
                        borderColor: '#1d4ed8',
                        borderWidth: 2,
                        borderDash: [6, 4],
                        yAxisID: 'y5',
                    }
                ]
            },
            options: {
                animation: false,
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        display: false
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
                        type: 'linear',
                        title: {
                            display: false,
                            text: 'Time (days)'
                        },
                        ticks: {
                            callback: (value) => {
                                const numeric = Number(value);
                                if (!Number.isFinite(numeric)) return '';
                                if (xAxisMode === 'pvi') return numeric.toFixed(3);
                                return numeric.toFixed(1);
                            },
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
                    y3: {
                        type: 'linear',
                        display: true,
                        position: 'right',
                        offset: true,
                        alignToPixels: true,
                        title: {
                            display: true,
                            text: 'Average Reservoir Pressure (bar)'
                        },
                        grid: {
                            drawOnChartArea: false,
                        },
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

        applyScenarioSelection();
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
        const waterProd = liquidProd.map((qL, idx) => Math.max(0, Number(qL ?? 0) - Number(oilProd[idx] ?? 0)));

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

        const avgWaterSat = rateHistory.map((point, idx) => {
            const seriesValue = avgWaterSaturationSeries?.[idx];
            const value = seriesValue ?? point.avg_water_saturation;
            return Number.isFinite(value) ? value : null;
        });
        
        // Calculate cumulative oil production
        let cumulativeOil = 0;
        const cumulativeOilData = [];
        let cumulativeInjection = 0;
        let cumulativeLiquid = 0;
        const pviData = [];
        const cumulativeLiquidData = [];
        const cumulativeInjectionData = [];
        for (let i = 0; i < rateHistory.length; i++) {
            const dt = i > 0 ? rateHistory[i].time - rateHistory[i-1].time : rateHistory[i].time;
            cumulativeOil += Number(oilProd[i] ?? 0) * dt;
            cumulativeOilData.push(cumulativeOil);
            cumulativeInjection += Math.max(0, Number(injection[i] ?? 0)) * dt;
            cumulativeLiquid += Math.max(0, Number(liquidProd[i] ?? 0)) * dt;
            cumulativeLiquidData.push(cumulativeLiquid);
            cumulativeInjectionData.push(cumulativeInjection);
            if (!Number.isFinite(poreVolumeM3) || poreVolumeM3 <= 1e-12) {
                pviData.push(0);
            } else {
                pviData.push(cumulativeInjection / poreVolumeM3);
            }
        }

        pviAvailable = cumulativeInjection > 1e-12;
        if (pendingScenarioXAxisMode === 'pvi' && pviAvailable) {
            xAxisMode = 'pvi';
            pendingScenarioXAxisMode = null;
        }
        if (!pviAvailable && xAxisMode === 'pvi') {
            xAxisMode = 'time';
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

        const absErrorData = oilProd.map((simValueOpt, idx) => {
            const analyticalValue = Number(analyticalOilProd[idx]);
            const simValue = Number(simValueOpt);
            if (!Number.isFinite(analyticalValue) || !Number.isFinite(simValue)) return null;
            return Math.abs(simValue - analyticalValue);
        });

        const percentErrorData = oilProd.map((simValueOpt, idx) => {
            const analyticalValue = Number(analyticalOilProd[idx]);
            const simValue = Number(simValueOpt);
            if (!Number.isFinite(analyticalValue) || !Number.isFinite(simValue)) return null;
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

        const xValues = xAxisMode === 'pvi'
            ? pviData
            : xAxisMode === 'cumLiquid'
                ? cumulativeLiquidData
                : xAxisMode === 'cumInjection'
                    ? cumulativeInjectionData
                    : rateHistory.map((p) => Number(p.time));

        const toXYSeries = (yValues: Array<number | null | undefined>): XYPoint[] => {
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
        };

        chart.data.labels = [];
        safeSetDatasetData(chart, 0, toXYSeries(oilProd));
        safeSetDatasetData(chart, 1, toXYSeries(analyticalOilProd as Array<number | null>));
        safeSetDatasetData(chart, 2, toXYSeries(waterProd));
        safeSetDatasetData(chart, 3, toXYSeries(injection));
        safeSetDatasetData(chart, 4, toXYSeries(cumulativeOilData));
        safeSetDatasetData(chart, 5, toXYSeries(analyticalCumulativeOilData as Array<number | null>));
        safeSetDatasetData(chart, 6, toXYSeries(recoveryFactorData as Array<number | null>));
        safeSetDatasetData(chart, 7, toXYSeries(liquidProd));
        safeSetDatasetData(chart, 8, toXYSeries(vrrData as Array<number | null>));
        safeSetDatasetData(chart, 9, toXYSeries(avgReservoirPressure as Array<number | null>));
        safeSetDatasetData(chart, 10, toXYSeries(avgWaterSat as Array<number | null>));
        safeSetDatasetData(chart, 11, toXYSeries(absErrorData as Array<number | null>));
        safeSetDatasetData(chart, 12, toXYSeries(recoveryFactorData as Array<number | null>));
        safeSetDatasetData(chart, 13, toXYSeries(waterCutSimVsPvi as Array<number | null>));
        safeSetDatasetData(chart, 14, toXYSeries(waterCutAnalyticalVsPvi as Array<number | null>));

        const showPoints = timeLabels.length <= 20;
        for (let idx = 0; idx < (chart.data.datasets?.length ?? 0); idx++) {
            const lineDataset = getLineDataset(chart, idx);
            if (!lineDataset) continue;
            lineDataset.pointRadius = showPoints ? 2 : 0;
            lineDataset.pointHoverRadius = showPoints ? 3 : 4;
            lineDataset.pointHitRadius = 8;
        }

        const effectiveSelection = selectedDatasetIndexes.length > 0
            ? selectedDatasetIndexes
            : chart.data.datasets.map((_, idx) => idx);
        applySelection(effectiveSelection);
    }

    function setDatasetVisibility(visibleIndexes: number[]) {
        if (!chart) return;
        for (let idx = 0; idx < (chart.data.datasets?.length ?? 0); idx++) {
            const show = visibleIndexes.includes(idx);
            chart.setDatasetVisibility(idx, show);
        }
    }

    function collectAxisValues(axisId: string, visibleIndexes: number[]): number[] {
        if (!chart) return [];
        const values: number[] = [];

        for (const idx of visibleIndexes) {
            const dataset = getLineDataset(chart, idx);
            if (!dataset || dataset.yAxisID !== axisId || !Array.isArray(dataset.data)) continue;

            for (const point of dataset.data as Array<XYPoint | number | null | undefined>) {
                if (typeof point === 'number') {
                    if (Number.isFinite(point)) values.push(point);
                    continue;
                }
                if (!point || typeof point !== 'object') continue;
                const yValue = Number((point as XYPoint).y);
                if (Number.isFinite(yValue)) values.push(yValue);
            }
        }

        return values;
    }

    function clearAxisBounds(axis: AxisScaleConfig | undefined) {
        if (!axis) return;
        delete axis.min;
        delete axis.max;
    }

    function niceUpperBound(value: number): number {
        if (!Number.isFinite(value) || value <= 0) return 1;
        const exponent = Math.floor(Math.log10(value));
        const magnitude = 10 ** exponent;
        const fraction = value / magnitude;
        let niceFraction = 1;
        if (fraction <= 1) niceFraction = 1;
        else if (fraction <= 2) niceFraction = 2;
        else if (fraction <= 5) niceFraction = 5;
        else niceFraction = 10;
        return niceFraction * magnitude;
    }

    function applyPositiveAxisBounds(axis: AxisScaleConfig | undefined, values: number[], fallbackMax = 1) {
        if (!axis) return;
        if (!values.length) {
            clearAxisBounds(axis);
            return;
        }
        const maxValue = Math.max(0, ...values);
        axis.min = 0;
        axis.max = niceUpperBound(Math.max(maxValue * 1.05, fallbackMax));
    }

    function applyFractionAxisBounds(axis: AxisScaleConfig | undefined, values: number[]) {
        if (!axis) return;
        if (!values.length) {
            clearAxisBounds(axis);
            return;
        }
        const maxValue = Math.max(0, ...values);
        axis.min = 0;
        const targetMax = Math.max(maxValue * 1.1, 0.05);
        axis.max = Math.min(1, niceUpperBound(targetMax));
    }

    function applyAutoAxisBounds(axis: AxisScaleConfig | undefined, values: number[]) {
        if (!axis) return;
        if (!values.length) {
            clearAxisBounds(axis);
            return;
        }

        const minValue = Math.min(...values);
        const maxValue = Math.max(...values);

        if (Math.abs(maxValue - minValue) < 1e-9) {
            const pad = Math.max(Math.abs(maxValue) * 0.05, 1);
            axis.min = minValue - pad;
            axis.max = maxValue + pad;
            return;
        }

        const span = maxValue - minValue;
        const pad = span * 0.1;
        axis.min = minValue - pad;
        axis.max = maxValue + pad;
    }

    function applySelection(visibleIndexes: number[]) {
        if (!chart) return;
        setDatasetVisibility(visibleIndexes);

        const scales = getScalesMap();
        const activeAxisIds = new Set(
            visibleIndexes
                .map((idx) => getLineDataset(chart, idx)?.yAxisID)
                .filter((axisId): axisId is string => Boolean(axisId))
        );

        if (scales.y) scales.y.display = activeAxisIds.has('y');
        if (scales.y1) scales.y1.display = activeAxisIds.has('y1');
        if (scales.y2) scales.y2.display = activeAxisIds.has('y2');
        if (scales.y3) scales.y3.display = activeAxisIds.has('y3');
        if (scales.y4) scales.y4.display = activeAxisIds.has('y4');
        if (scales.y5) scales.y5.display = activeAxisIds.has('y5');

        if (scales.y?.title) scales.y.title.text = 'Rate (m³/day)';
        if (scales.y3?.title) scales.y3.title.text = 'Average Reservoir Pressure (bar)';

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
            scales.x.title.text = getXAxisTitle(xAxisMode);
        }

        applyPositiveAxisBounds(scales.y, collectAxisValues('y', visibleIndexes));
        applyPositiveAxisBounds(scales.y1, collectAxisValues('y1', visibleIndexes));
        applyPositiveAxisBounds(scales.y2, collectAxisValues('y2', visibleIndexes));
        applyAutoAxisBounds(scales.y3, collectAxisValues('y3', visibleIndexes));
        applyPositiveAxisBounds(scales.y4, collectAxisValues('y4', visibleIndexes));
        applyFractionAxisBounds(scales.y5, collectAxisValues('y5', visibleIndexes));

        chart.update();
    }

    function toggleDataset(datasetIndex: number) {
        const next = new Set<number>();
        for (const idx of selectedDatasetIndexes) {
            next.add(idx);
        }
        if (next.has(datasetIndex)) {
            next.delete(datasetIndex);
        } else {
            next.add(datasetIndex);
        }

        selectedDatasetIndexes = [...next].sort((a, b) => a - b);
        updateChart();
    }

    function getDatasetColor(idx: number): string {
        const ds = getLineDataset(chart, idx);
        if (!ds) return '#888';
        if (Array.isArray(ds.borderColor)) return String(ds.borderColor[0]);
        return String(ds.borderColor);
    }
    
    function getDatasetDashArray(idx: number): string {
        const ds = getLineDataset(chart, idx);
        if (!ds) return '';
        if (Array.isArray(ds.borderDash)) return ds.borderDash.join(', ');
        return '';
    }

    function getDatasetBorderWidth(idx: number): number {
        const ds = getLineDataset(chart, idx);
        if (!ds) return 2;
        if (typeof ds.borderWidth === 'number') return ds.borderWidth;
        return 2;
    }

    $: if (chart) {
        const nextKey = `${activeCategory}::${activeCase}`;
        if (nextKey !== lastScenarioSelectionKey) {
            lastScenarioSelectionKey = nextKey;
            applyScenarioSelection();
        }
    }
</script>

<div class="mb-2 space-y-2">
    {#if chart}


        <div class="mt-2">
            <div class="mb-1 text-[11px] uppercase tracking-wide opacity-65">Curves</div>
            <div class="flex flex-wrap gap-2 items-center">
                {#each selectedDatasetIndexes as idx}
                    <div class="inline-flex items-center gap-1.5 px-2 py-1 bg-base-200 border border-base-300 rounded-full text-xs shadow-sm shadow-base-300/20">
                        <svg width="16" height="4" class="overflow-visible shrink-0 ml-0.5" viewBox="0 0 16 4">
                            <line x1="0" y1="2" x2="16" y2="2" 
                                stroke={getDatasetColor(idx)} 
                                stroke-width={getDatasetBorderWidth(idx)} 
                                stroke-dasharray={getDatasetDashArray(idx)} 
                            />
                        </svg>
                        <span class="opacity-90">{getDatasetLabel(chart, idx)}</span>
                        <button 
                            type="button" 
                            class="btn btn-ghost btn-xs h-4 min-h-4 w-4 px-0 rounded-full opacity-60 hover:opacity-100 hover:bg-base-300 mr-[-2px] ml-0.5 pb-[1px]" 
                            on:click={() => toggleDataset(idx)}
                            aria-label="Remove curve"
                            title="Remove curve"
                        >
                            ✕
                        </button>
                    </div>
                {/each}

                {#if selectedDatasetIndexes.length < chart.data.datasets.length}
                    <select 
                        class="select select-bordered select-xs bg-base-100 max-w-[200px] rounded-full focus:outline-none focus:ring-1 focus:ring-base-content/20 font-normal shadow-sm"
                        on:change={(e) => {
                            if (e.currentTarget.value) {
                                toggleDataset(Number(e.currentTarget.value));
                                e.currentTarget.value = '';
                            }
                        }}
                    >
                        <option value="" disabled selected>+ Select curve...</option>
                        {#each chart.data.datasets as _, idx}
                            {#if !selectedDatasetIndexes.includes(idx)}
                                <option value={idx}>{getDatasetLabel(chart, idx)}</option>
                            {/if}
                        {/each}
                    </select>
                {/if}
            </div>
        </div>
    {/if}
</div>

<div class="chart-container" style="position: relative; height: min(52vh, 440px); width:100%;">
    <canvas bind:this={chartCanvas}></canvas>
</div>

<div class="mt-3 flex items-center justify-between">
    <div class="flex items-center gap-2">
        <label for="x-axis-select" class="text-xs uppercase tracking-wide opacity-70 font-medium">X-Axis:</label>
        <select 
            id="x-axis-select"
            class="select select-bordered select-sm bg-base-100 shadow-sm min-w-[160px]"
            bind:value={xAxisMode}
            on:change={(e) => setXAxisMode(e.currentTarget.value as XAxisMode)}
        >
            <option value="time">Time (days)</option>
            <option value="pvi" disabled={!pviAvailable}>PV Injected (PVI)</option>
            <option value="cumLiquid">Cumulative Liquid (m³)</option>
            <option value="cumInjection">Cumulative Injection (m³)</option>
        </select>
    </div>
</div>

<div style="margin-top: 0.5rem; font-size: 12px; color: #555; text-align: left;">
    <div>Analytical points compared: {mismatchSummary.pointsCompared}</div>
    <div>
        MAE: {mismatchSummary.mae.toFixed(3)} m³/day ·
        RMSE: {mismatchSummary.rmse.toFixed(3)} m³/day ·
        MAPE: {mismatchSummary.mape.toFixed(2)}%
    </div>
</div>
