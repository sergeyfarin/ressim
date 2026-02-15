<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Chart, registerables } from 'chart.js';

    export let rateHistory = [];
    export let analyticalProductionData = [];

    type MismatchSummary = {
        pointsCompared: number;
        mae: number;
        rmse: number;
        mape: number;
    };

    let chartCanvas: HTMLCanvasElement;
    let chart: Chart;
    let activeTab: 'oil' | 'water' | 'voidage' | 'validation' = 'oil';
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

    $: if (chart) {
        applyActiveTab();
    }

    const tabLabels: Record<typeof activeTab, string> = {
        oil: 'Oil Rate + Cumulative',
        water: 'Water Injection + Production',
        voidage: 'Voidage / Liquid Balance',
        validation: 'Model vs Analytical',
    };

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
                        borderColor: 'green',
                        yAxisID: 'y',
                    },
                    {
                        label: 'Analytical Oil Production (m³/day)',
                        data: [],
                        borderColor: 'green',
                        borderDash: [5, 5],
                        yAxisID: 'y',
                    },
                    {
                        label: 'Water Production (m³/day)',
                        data: [],
                        borderColor: '#0ea5e9',
                        yAxisID: 'y',
                    },
                    {
                        label: 'Water Injection (m³/day)',
                        data: [],
                        borderColor: '#ef4444',
                        yAxisID: 'y',
                    },
                    {
                        label: 'Cumulative Oil (m³)',
                        data: [],
                        borderColor: 'purple',
                        yAxisID: 'y1',
                    },
                    {
                        label: 'Liquid Production (m³/day)',
                        data: [],
                        borderColor: '#2563eb',
                        borderDash: [4, 4],
                        yAxisID: 'y',
                    },
                    {
                        label: 'Voidage Replacement Ratio',
                        data: [],
                        borderColor: '#f59e0b',
                        yAxisID: 'y2',
                    },
                    {
                        label: 'Oil Rate Abs Error (m³/day)',
                        data: [],
                        borderColor: 'black',
                        borderDash: [6, 4],
                        yAxisID: 'y',
                    },
                    {
                        label: 'Oil Rate Error (%)',
                        data: [],
                        borderColor: '#8b5cf6',
                        borderDash: [3, 3],
                        yAxisID: 'y3',
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
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
                        title: {
                            display: true,
                            text: 'Rate (m³/day)'
                        },
                    },
                    y1: {
                        type: 'linear',
                        display: true,
                        position: 'right',
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
                        title: {
                            display: true,
                            text: 'Voidage Replacement Ratio'
                        },
                        grid: {
                            drawOnChartArea: false,
                        },
                        min: 0,
                    },
                    y3: {
                        type: 'linear',
                        display: true,
                        position: 'right',
                        offset: true,
                        title: {
                            display: true,
                            text: 'Oil Rate Error (%)'
                        },
                        grid: {
                            drawOnChartArea: false,
                        }
                    }
                }
            }
        });

        applyActiveTab();
    });

    onDestroy(() => {
        chart?.destroy();
    });

    function updateChart() {
        if (!chart || !rateHistory || rateHistory.length === 0) return;

        const labels = rateHistory.map(p => p.time.toFixed(2));
        const oilProd = rateHistory.map(p => p.total_production_oil);
        const liquidProd = rateHistory.map(p => p.total_production_liquid);
        const injection = rateHistory.map(p => p.total_injection);
        const waterProd = liquidProd.map((qL, idx) => Math.max(0, qL - oilProd[idx]));

        const analyticalOilProd = rateHistory.map((_, idx) => {
            const value = analyticalProductionData[idx]?.oilRate;
            return Number.isFinite(value) ? value : null;
        });
        
        // Calculate cumulative oil production
        let cumulativeOil = 0;
        const cumulativeOilData = [];
        for (let i = 0; i < rateHistory.length; i++) {
            const dt = i > 0 ? rateHistory[i].time - rateHistory[i-1].time : rateHistory[i].time;
            cumulativeOil += oilProd[i] * dt;
            cumulativeOilData.push(cumulativeOil);
        }

        // Calculate Voidage Replacement Ratio
        const vrrData = rateHistory.map(p => {
            if (p.total_production_liquid > 0) {
                return p.total_injection / p.total_production_liquid;
            }
            return 0; // Avoid division by zero
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

        chart.data.labels = labels;
        chart.data.datasets[0].data = oilProd;
        chart.data.datasets[1].data = analyticalOilProd;
        chart.data.datasets[2].data = waterProd;
        chart.data.datasets[3].data = injection;
        chart.data.datasets[4].data = cumulativeOilData;
        chart.data.datasets[5].data = liquidProd;
        chart.data.datasets[6].data = vrrData;
        chart.data.datasets[7].data = absErrorData;
        chart.data.datasets[8].data = percentErrorData;

        chart.update();
    }

    function setDatasetVisibility(visibleIndexes: number[]) {
        if (!chart) return;
        chart.data.datasets.forEach((_, idx) => {
            const show = visibleIndexes.includes(idx);
            chart.setDatasetVisibility(idx, show);
        });
    }

    function applyAxisVisibility(config: { y: boolean; y1: boolean; y2: boolean; y3: boolean }) {
        if (!chart) return;
        const scales = chart.options.scales ?? {};
        if (scales.y) scales.y.display = config.y;
        if (scales.y1) scales.y1.display = config.y1;
        if (scales.y2) scales.y2.display = config.y2;
        if (scales.y3) scales.y3.display = config.y3;
    }

    function applyActiveTab() {
        if (!chart) return;

        if (activeTab === 'oil') {
            setDatasetVisibility([0, 1, 4]);
            applyAxisVisibility({ y: true, y1: true, y2: false, y3: false });
        } else if (activeTab === 'water') {
            setDatasetVisibility([2, 3]);
            applyAxisVisibility({ y: true, y1: false, y2: false, y3: false });
        } else if (activeTab === 'voidage') {
            setDatasetVisibility([3, 5, 6]);
            applyAxisVisibility({ y: true, y1: false, y2: true, y3: false });
        } else {
            setDatasetVisibility([0, 1, 7, 8]);
            applyAxisVisibility({ y: true, y1: false, y2: false, y3: true });
        }

        chart.update();
    }
</script>

<div class="mb-2 flex flex-wrap gap-2">
    {#each Object.entries(tabLabels) as [key, label]}
        <button
            class={`btn btn-xs sm:btn-sm ${activeTab === key ? 'btn-primary' : 'btn-outline'}`}
            on:click={() => activeTab = key as typeof activeTab}
        >
            {label}
        </button>
    {/each}
</div>

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
