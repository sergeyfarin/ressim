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
                        label: 'Total Liquid Production (m³/day)',
                        data: [],
                        borderColor: 'blue',
                        yAxisID: 'y',
                    },
                    {
                        label: 'Total Injection (m³/day)',
                        data: [],
                        borderColor: 'red',
                        yAxisID: 'y',
                    },
                    {
                        label: 'Cumulative Oil (m³)',
                        data: [],
                        borderColor: 'purple',
                        yAxisID: 'y1',
                    },
                    {
                        label: 'Voidage Replacement Ratio',
                        data: [],
                        borderColor: 'orange',
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
                        // Place it after y1
                        afterDataLimits: (axis) => {
                            axis.paddingTop = 60;
                        }
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
        chart.data.datasets[2].data = liquidProd;
        chart.data.datasets[3].data = injection;
        chart.data.datasets[4].data = cumulativeOilData;
        chart.data.datasets[5].data = vrrData;
        chart.data.datasets[6].data = absErrorData;
        chart.data.datasets[7].data = percentErrorData;

        chart.update();
    }
</script>

<div class="chart-container" style="position: relative; height:100% ; width:100%;">
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
