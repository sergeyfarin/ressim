<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Chart, registerables } from 'chart.js';

    export let rateHistory = [];
    export let analyticalProductionData = [];

    let chartCanvas: HTMLCanvasElement;
    let chart: Chart;

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

        const analyticalOilProd = analyticalProductionData.map(p => p.oilRate);
        
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

        chart.data.labels = labels;
        chart.data.datasets[0].data = oilProd;
        chart.data.datasets[1].data = analyticalOilProd;
        chart.data.datasets[2].data = liquidProd;
        chart.data.datasets[3].data = injection;
        chart.data.datasets[4].data = cumulativeOilData;
        chart.data.datasets[5].data = vrrData;

        chart.update();
    }
</script>

<div class="chart-container" style="position: relative; height:40vh; width:80vw">
    <canvas bind:this={chartCanvas}></canvas>
</div>
