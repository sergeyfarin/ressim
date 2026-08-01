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
    import { onMount, onDestroy, untrack } from "svelte";
    import { Chart, registerables } from "chart.js";
    import { ANALYTICAL_DASH } from "../charts/curveStylePolicy";
    import {
        applyThemeToChart,
        externalTooltipHandler,
    } from "../charts/chart-helpers";
    import {
        axisLength,
        buildFloodFrontOverlay,
        buildSpatialProfile,
        buildSweepDiagonalOverlay,
        cumulativeInjectedVolume,
        defaultSpatialProfileAxis,
        type SpatialProfileAxis,
        type SpatialProfileGrid,
        type SpatialProfileLayerSelection,
        type SpatialProfileProperty,
        type SpatialProfileReference,
    } from "./spatialProfileModel";
    import type { GridState, RateHistoryPoint } from "../simulator-types";
    import type { FluidProps, RockProps } from "../analytical/fractionalFlow";
    import type { PressureDisplayRange } from "./spatialViewModel";

    let {
        gridState = null,
        grid,
        property = "saturation_water",
        reference = null,
        defaultAxis = null,
        wellPathLabel = "Diagonal",
        simTime = 0,
        sourceLabel = "Live runtime",
        theme = "dark",
        rockProps,
        fluidProps,
        initialSaturation = 0.2,
        porosity = 0,
        rateHistory = [],
        injectorI = 0,
        injectorJ = 0,
        injectorKLayers = [],
        producerKLayers = [],
        producerI = grid.nx - 1,
        producerJ = 0,
        pressureDisplayRange,
    }: {
        gridState?: GridState | null;
        grid: SpatialProfileGrid;
        property?: SpatialProfileProperty;
        reference?: SpatialProfileReference | null;
        defaultAxis?: SpatialProfileAxis | null;
        wellPathLabel?: string;
        simTime?: number;
        sourceLabel?: string;
        theme?: "dark" | "light";
        rockProps: RockProps;
        fluidProps: FluidProps;
        initialSaturation?: number;
        porosity?: number;
        rateHistory?: RateHistoryPoint[];
        injectorI?: number;
        injectorKLayers?: number[];
        producerKLayers?: number[];
        injectorJ?: number;
        producerI?: number;
        producerJ?: number;
        pressureDisplayRange?: PressureDisplayRange;
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

    const isOneDimensional = $derived(
        [grid.nx, grid.ny, grid.nz].filter((count) => count > 1).length <= 1,
    );
    const hasDiagonalWellPath = $derived(
        grid.nx > 1 && grid.ny > 1 && injectorI !== producerI && injectorJ !== producerJ,
    );
    let axis = $state<SpatialProfileAxis>(untrack(() => defaultSpatialProfileAxis(grid, defaultAxis)));
    let userI = $state<number | null>(null);
    let userJ = $state<number | null>(null);
    let userK = $state<number | null>(null);
    let layerSelection = $state<SpatialProfileLayerSelection>(
        untrack(() => grid.nz > 1 ? "average" : 0),
    );

    // Ordinary areal profiles pass through the producer. Corner-to-corner
    // floods use the explicit well path instead.
    const fixedI = $derived(
        Math.max(0, Math.min(grid.nx - 1, userI ?? producerI)),
    );
    const fixedJ = $derived(
        Math.max(0, Math.min(grid.ny - 1, userJ ?? producerJ)),
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
            layerSelection,
            injectorI,
            injectorJ,
            producerI,
            producerJ,
            property,
        }),
    );

    const frontOverlay = $derived.by(() => {
        const injectedVolume = cumulativeInjectedVolume(rateHistory, simTime);
        if (reference?.kind === "sweep") {
            return buildSweepDiagonalOverlay({
                grid,
                axis,
                property,
                layerSelection,
                geometry: reference.geometry,
                rock: rockProps,
                fluid: fluidProps,
                initialSaturation,
                porosity,
                injectedVolume,
                layerPermeabilities: reference.layerPermeabilities,
                injectorI,
                injectorJ,
                producerI,
                producerJ,
            });
        }
        if (reference?.kind !== "buckley-leverett") return null;
        return buildFloodFrontOverlay({
            grid,
            axis,
            property,
            rock: rockProps,
            fluid: fluidProps,
            initialSaturation,
            porosity,
            injectedVolume,
            // The flood path itself: which axis the wells are separated along,
            // and which end the water enters from. A column flooded from its
            // base runs towards index 0, and the overlay mirrors.
            wells: {
                injector: { i: injectorI, j: injectorJ, k: injectorKLayers[0] ?? 0 },
                producer: {
                    i: producerI,
                    j: producerJ,
                    k: producerKLayers[0] ?? Math.max(0, grid.nz - 1),
                },
            },
        });
    });

    /** The two indices held constant, as editable controls. */
    const heldAxes = $derived(
        (axis === "well-path" ? [] : (["i", "j", "k"] as const))
            .filter((candidate) => candidate !== axis)
            .filter((candidate) => candidate !== "k")
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
                        display: false,
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
                label: frontOverlay.label,
                data: frontOverlay.values,
                borderColor: FRONT_COLOR,
                borderWidth: 2,
                pointRadius: 0,
                spanGaps: false,
                // The front position is an analytical (Buckley-Leverett) marker,
                // so it carries the analytical dash like every other reference.
                // @ts-expect-error Chart.js accepts borderDash on line datasets
                borderDash: ANALYTICAL_DASH,
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
        const valueRange = property === "pressure" && pressureDisplayRange
            ? [pressureDisplayRange.min, pressureDisplayRange.max]
            : profile.valueRange;
        scales.y.min = valueRange?.[0];
        scales.y.max = valueRange?.[1];

        applyThemeToChart(chart, theme);
        chart.update("none");
    }

    $effect(() => {
        // Redraw on any input that changes the rendered profile.
        void [profile, frontOverlay, theme, pressureDisplayRange];
        redraw();
    });
</script>

<div class="mt-3 rounded-lg border border-border bg-card/40">
    <div class="p-3 md:p-4">
        <div class="mb-2 flex flex-wrap items-end gap-3">
            <div class="min-w-0">
                <h4 class="text-xs font-semibold">{profile.valueLabel} Profile</h4>
                <p class="text-[11px] opacity-70">
                    {sourceLabel} snapshot at t = {simTime.toFixed(2)} d — follows the 3D
                    property and timestep selectors.
                </p>
            </div>

            {#if !isOneDimensional}
            <div class="ml-auto flex flex-wrap items-end gap-2">
                <label class="flex flex-col gap-0.5">
                    <span class="text-[10px] uppercase tracking-wide opacity-70">Axis</span>
                    <select
                        class="h-7 rounded-md border border-input bg-transparent px-2 text-xs"
                        bind:value={axis}
                    >
                        {#if grid.nx > 1}<option value="i">I</option>{/if}
                        {#if grid.ny > 1}<option value="j">J</option>{/if}
                        {#if grid.nz > 1}<option value="k">K</option>{/if}
                        {#if hasDiagonalWellPath}
                            <option value="well-path">{wellPathLabel}</option>
                        {/if}
                    </select>
                </label>

                {#if grid.nz > 1 && axis !== "k"}
                    <label class="flex flex-col gap-0.5">
                        <span class="text-[10px] uppercase tracking-wide opacity-70">Layers</span>
                        <select
                            class="h-7 rounded-md border border-input bg-transparent px-2 text-xs"
                            bind:value={layerSelection}
                        >
                            <option value="average">Column average</option>
                            {#each Array.from({ length: grid.nz }, (_, k) => k) as k}
                                <option value={k}>K = {k}</option>
                            {/each}
                        </select>
                    </label>
                {/if}

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
            {/if}
        </div>

        <div style="position: relative; height: min(28vh, 240px); width: 100%;">
            <canvas bind:this={canvas}></canvas>
        </div>

        {#if frontOverlay}
            <div class="mt-2 text-[11px] opacity-80">
                {frontOverlay.label} front at {frontOverlay.frontDistance.toFixed(1)} m
                (Sw {frontOverlay.initialSw.toFixed(2)} → {frontOverlay.shockSw.toFixed(2)}).
                {#if reference?.kind === "sweep"}
                    Craig E_A is mapped to the diagonal; Buckley–Leverett describes displacement
                    within the contacted region.
                {/if}
            </div>
        {/if}
    </div>
</div>
