<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { Chart, registerables } from "chart.js";
    import {
        safeSetDatasetData,
        externalTooltipHandler,
    } from "./chart-helpers";
    import {
        computeCombinedSweep,
        type SweepPoint,
        type CombinedSweepResult,
    } from "../analytical/sweepEfficiency";
    import type { RockProps, FluidProps } from "../analytical/fractionalFlow";

    let {
        rockProps,
        fluidProps,
        layerPermeabilities = [100],
        layerThickness = 10,
        pviMax = 3.0,
        sourceLabel = "Analytical",
    }: {
        rockProps: RockProps;
        fluidProps: FluidProps;
        layerPermeabilities?: number[];
        layerThickness?: number;
        pviMax?: number;
        sourceLabel?: string;
    } = $props();

    let chartCanvas = $state<HTMLCanvasElement | null>(null);
    let chart: Chart<"line", Array<number | null>, string> | null = null;
    let sweepResult = $state<CombinedSweepResult | null>(null);

    $effect(() => {
        if (rockProps && fluidProps) {
            const perms = layerPermeabilities.length > 0 ? layerPermeabilities : [100];
            sweepResult = computeCombinedSweep(rockProps, fluidProps, perms, layerThickness, pviMax);
        }
    });

    $effect(() => {
        if (sweepResult) {
            updateChart();
        }
    });

    onMount(() => {
        Chart.register(...registerables);
        const ctx = chartCanvas?.getContext("2d");
        if (!ctx) return;

        chart = new Chart(ctx, {
            type: "line",
            data: {
                labels: [],
                datasets: [
                    {
                        label: "Areal (E_A)",
                        data: [],
                        borderColor: "#2563eb",
                        backgroundColor: "rgba(37, 99, 235, 0.08)",
                        borderWidth: 2.0,
                        borderDash: [7, 4],
                        pointRadius: 0,
                        fill: false,
                    },
                    {
                        label: "Vertical (E_V)",
                        data: [],
                        borderColor: "#16a34a",
                        backgroundColor: "rgba(22, 163, 74, 0.08)",
                        borderWidth: 1.6,
                        borderDash: [3, 4],
                        pointRadius: 0,
                        fill: false,
                    },
                    {
                        label: "Combined (E_vol)",
                        data: [],
                        borderColor: "#dc2626",
                        backgroundColor: "rgba(220, 38, 38, 0.08)",
                        borderWidth: 2.4,
                        borderDash: [12, 4],
                        pointRadius: 0,
                        fill: false,
                    },
                ],
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        display: true,
                        labels: {
                            font: {
                                family: "'JetBrains Mono', monospace",
                                size: 11,
                            },
                        },
                    },
                    tooltip: {
                        enabled: false,
                        external: externalTooltipHandler,
                    },
                },
                scales: {
                    x: {
                        title: {
                            display: true,
                            text: "Pore Volumes Injected (PVI)",
                            font: {
                                family: "'JetBrains Mono', monospace",
                                size: 11,
                            },
                        },
                        ticks: {
                            font: {
                                family: "'JetBrains Mono', monospace",
                                size: 10,
                            },
                        },
                    },
                    y: {
                        min: 0,
                        max: 1,
                        title: {
                            display: true,
                            text: "Sweep Efficiency",
                            font: {
                                family: "'JetBrains Mono', monospace",
                                size: 11,
                            },
                        },
                        ticks: {
                            font: {
                                family: "'JetBrains Mono', monospace",
                                size: 10,
                            },
                            callback: (v: string | number) =>
                                typeof v === "number" ? (v * 100).toFixed(0) + "%" : v,
                        },
                    },
                },
            },
        });

        updateChart();
    });

    onDestroy(() => {
        chart?.destroy();
        chart = null;
    });

    function updateChart() {
        if (!chart || !sweepResult) return;

        const labels = sweepResult.arealSweep.curve.map((p) =>
            p.pvi.toFixed(2),
        );
        const arealData = sweepResult.arealSweep.curve.map((p) => p.efficiency);
        const vertData = sweepResult.verticalSweep.curve.map(
            (p) => p.efficiency,
        );
        const combData = sweepResult.combined.map((p) => p.efficiency);

        chart.data.labels = labels;
        safeSetDatasetData(chart, 0, arealData);
        safeSetDatasetData(chart, 1, vertData);
        safeSetDatasetData(chart, 2, combData);
        chart.update();
    }
</script>

<div class="rounded-lg border border-border bg-card shadow-sm">
    <div class="p-4 md:p-5">
        <div class="mb-2">
            <h3 class="text-sm font-semibold">
                Sweep Efficiency vs PVI (Analytical)
            </h3>
            <p class="text-xs opacity-70">
                {sourceLabel} — Areal (Craig five-spot), Vertical (Dykstra-Parsons), Combined (E_A × E_V).
            </p>
        </div>

        <div style="position: relative; height: min(34vh, 280px); width: 100%;">
            <canvas bind:this={chartCanvas}></canvas>
        </div>

        {#if sweepResult}
            <div class="mt-2 flex flex-wrap gap-4 text-xs opacity-80">
                <span>
                    M = {sweepResult.arealSweep.mobilityRatio.toFixed(2)}
                </span>
                <span>
                    E_A(BT) = {(sweepResult.arealSweep.eaAtBreakthrough * 100).toFixed(1)}%
                </span>
                <span>
                    V_DP = {sweepResult.verticalSweep.vdp.toFixed(3)}
                </span>
            </div>
        {/if}
    </div>
</div>
