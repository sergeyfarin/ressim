<script lang="ts">
    /**
     * SpatialProfileChart — a 1D cross-section of the property the 3D view is
     * showing, along a chosen grid line, at the currently selected timestep.
     *
     * Lives in the 3D group rather than the run-results charts because it shows
     * one number per cell at one instant, not one number per report step across
     * the run. It reads the same `showProperty` and the same snapshot the 3D
     * view renders, so scrubbing the timestep or switching the property moves
     * both together.
     *
     * Replaces the dormant `charts/SwProfileChart.svelte`, which was fixed to
     * water saturation, fixed to the k = 0 plane, indexed by cell number, and
     * carried its own copies of the fractional-flow relations.
     */
    import { onMount, onDestroy } from "svelte";
    import { Chart, registerables } from "chart.js";
    import {
        applyThemeToChart,
        externalTooltipHandler,
    } from "../charts/chart-helpers";
    import {
        axisLength,
        buildFloodFrontOverlay,
        buildSpatialProfile,
        type SpatialProfileAxis,
        type SpatialProfileGrid,
        type SpatialProfileProperty,
    } from "./spatialProfileModel";
    import type { GridState } from "../simulator-types";
    import type { FluidProps, RockProps } from "../analytical/fractionalFlow";

    let {
        gridState = null,
        grid,
        property = "saturation_water",
        simTime = 0,
        sourceLabel = "Live runtime",
        theme = "dark",
        rockProps,
        fluidProps,
        initialSaturation = 0.2,
        injectionRate = 0,
        defaultJ = 0,
    }: {
        gridState?: GridState | null;
        grid: SpatialProfileGrid;
        property?: SpatialProfileProperty;
        simTime?: number;
        sourceLabel?: string;
        theme?: "dark" | "light";
        rockProps: RockProps;
        fluidProps: FluidProps;
        initialSaturation?: number;
        injectionRate?: number;
        /** Producer row — the most useful default line through a pattern flood. */
        defaultJ?: number;
    } = $props();

    const SERIES_COLORS: Record<string, string> = {
        pressure: "#7c3aed",
        saturation_water: "#1d4ed8",
        saturation_oil: "#16a34a",
        saturation_gas: "#ea580c",
    };
    const FRONT_COLOR = "#94a3b8";

    let canvas = $state<HTMLCanvasElement | null>(null);
    let chart = $state<Chart<"line", Array<number | null>, string> | null>(null);

    let axis = $state<SpatialProfileAxis>("i");
    let userI = $state<number | null>(null);
    let userJ = $state<number | null>(null);
    let userK = $state<number | null>(null);

    // Selected line, clamped to the live grid. Falls back to the producer row
    // on J and the top layer on K, which is the section most cases want first.
    const fixedI = $derived(
        Math.max(0, Math.min(grid.nx - 1, userI ?? Math.floor(grid.nx / 2))),
    );
    const fixedJ = $derived(
        Math.max(0, Math.min(grid.ny - 1, userJ ?? defaultJ)),
    );
    const fixedK = $derived(Math.max(0, Math.min(grid.nz - 1, userK ?? 0)));

    const profile = $derived(
        buildSpatialProfile({
            gridState,
            grid,
            axis,
            fixedI,
            fixedJ,
            fixedK,
            property,
        }),
    );

    const frontOverlay = $derived(
        buildFloodFrontOverlay({
            grid,
            axis,
            property,
            rock: rockProps,
            fluid: fluidProps,
            initialSaturation,
            injectionRate,
            simTime,
        }),
    );

    /** The two indices held constant, as editable controls. */
    const heldAxes = $derived(
        (["i", "j", "k"] as const)
            .filter((candidate) => candidate !== axis)
            .map((candidate) => ({
                axis: candidate,
                label: candidate.toUpperCase(),
                max: Math.max(0, axisLength(grid, candidate) - 1),
                value: candidate === "i" ? fixedI : candidate === "j" ? fixedJ : fixedK,
            })),
    );

    function setHeldIndex(which: SpatialProfileAxis, value: number) {
        if (which === "i") userI = value;
        else if (which === "j") userJ = value;
        else userK = value;
    }

    onMount(() => {
        Chart.register(...registerables);
        const context = canvas?.getContext("2d");
        if (!context) return;
        chart = new Chart(context, {
            type: "line",
            data: { labels: [], datasets: [] },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                animation: false,
                interaction: { mode: "index", intersect: false },
                plugins: {
                    legend: {
                        display: true,
                        labels: {
                            boxWidth: 10,
                            font: { family: "'JetBrains Mono', monospace", size: 10 },
                        },
                    },
                    tooltip: { enabled: false, external: externalTooltipHandler },
                },
                scales: {
                    x: {
                        type: "linear",
                        title: { display: true, text: "", font: { family: "'JetBrains Mono', monospace", size: 11 } },
                        ticks: { font: { family: "'JetBrains Mono', monospace", size: 10 } },
                    },
                    y: {
                        title: { display: true, text: "", font: { family: "'JetBrains Mono', monospace", size: 11 } },
                        ticks: { font: { family: "'JetBrains Mono', monospace", size: 10 } },
                    },
                },
            },
        });
        redraw();
    });

    onDestroy(() => {
        chart?.destroy();
        chart = null;
    });

    function redraw() {
        if (!chart) return;

        const datasets = profile.series.map((series) => ({
            label: series.label,
            data: series.values,
            borderColor: SERIES_COLORS[series.key] ?? "#64748b",
            borderWidth: 2.4,
            pointRadius: 0,
            spanGaps: false,
        }));

        if (frontOverlay) {
            datasets.push({
                label: "Reference Front Profile",
                data: frontOverlay.values,
                borderColor: FRONT_COLOR,
                borderWidth: 2,
                pointRadius: 0,
                spanGaps: false,
                // @ts-expect-error Chart.js accepts borderDash on line datasets
                borderDash: [6, 4],
            });
        }

        chart.data.labels = profile.distances.map((d) => d.toFixed(1));
        // Assigned wholesale rather than patched per index: the series set
        // changes whenever the property changes (1 series ⇄ 3 for ternary), the
        // count is tiny, and animation is off, so there is nothing to gain from
        // in-place mutation — and indexing `chart.data.datasets` is disallowed
        // outside chart-helpers (no-direct-chart-datasets-access.test.ts).
        chart.data.datasets = datasets;

        const scales = chart.options.scales as Record<string, any>;
        scales.x.title.text = profile.axisLabel;
        scales.y.title.text = profile.valueLabel;
        scales.y.min = profile.valueRange?.[0];
        scales.y.max = profile.valueRange?.[1];

        applyThemeToChart(chart, theme);
        chart.update("none");
    }

    $effect(() => {
        // Redraw on any input that changes the rendered profile.
        void [profile, frontOverlay, theme];
        redraw();
    });
</script>

<div class="mt-3 rounded-lg border border-border bg-card/40">
    <div class="p-3 md:p-4">
        <div class="mb-2 flex flex-wrap items-end gap-3">
            <div class="min-w-0">
                <h4 class="text-xs font-semibold">{profile.valueLabel} Profile Along Grid Line</h4>
                <p class="text-[11px] opacity-70">
                    {sourceLabel} snapshot at t = {simTime.toFixed(2)} d — follows the 3D
                    property and timestep selectors.
                </p>
            </div>

            <div class="ml-auto flex flex-wrap items-end gap-2">
                <label class="flex flex-col gap-0.5">
                    <span class="text-[10px] uppercase tracking-wide opacity-70">Axis</span>
                    <select
                        class="h-7 rounded-md border border-input bg-transparent px-2 text-xs"
                        bind:value={axis}
                    >
                        <option value="i">I</option>
                        <option value="j">J</option>
                        <option value="k">K</option>
                    </select>
                </label>

                {#each heldAxes as held (held.axis)}
                    <label class="flex flex-col gap-0.5">
                        <span class="text-[10px] uppercase tracking-wide opacity-70">
                            {held.label} = {held.value}
                        </span>
                        <input
                            type="range"
                            min="0"
                            max={held.max}
                            step="1"
                            class="h-7 w-24"
                            disabled={held.max === 0}
                            value={held.value}
                            oninput={(event) =>
                                setHeldIndex(held.axis, Number(event.currentTarget.value))}
                        />
                    </label>
                {/each}
            </div>
        </div>

        <div style="position: relative; height: min(28vh, 240px); width: 100%;">
            <canvas bind:this={canvas}></canvas>
        </div>

        {#if frontOverlay}
            <div class="mt-2 text-[11px] opacity-80">
                Reference flood front at {frontOverlay.frontDistance.toFixed(1)} m
                (Sw {frontOverlay.initialSw.toFixed(2)} → {frontOverlay.shockSw.toFixed(2)}).
            </div>
        {/if}
    </div>
</div>
