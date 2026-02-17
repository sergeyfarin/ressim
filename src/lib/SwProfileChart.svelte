<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Chart, registerables } from 'chart.js';

    export let gridState: Array<Record<string, unknown>> = [];
    export let nx = 1;
    export let ny = 1;
    export let nz = 1;
    export let cellDx = 10;
    export let cellDy = 10;
    export let cellDz = 1;
    export let simTime = 0;
    export let producerJ = 0;
    export let initialSaturation = 0.3;
    export let reservoirPorosity = 0.2;
    export let injectionRate = 0;
    export let scenarioMode: 'waterflood' | 'depletion' = 'waterflood';
    export let rockProps: { s_wc: number; s_or: number; n_w: number; n_o: number };
    export let fluidProps: { mu_w: number; mu_o: number };

    let selectedRow = Number.NaN;
    let chartCanvas: HTMLCanvasElement;
    let chart: Chart<'line', Array<number | null>, string> | null = null;
    let frontCellIndex: number | null = null;

    $: selectedRow = Math.max(0, Math.min(ny - 1, Number.isFinite(selectedRow) ? selectedRow : producerJ));

    $: updateProfileChart();

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
                        label: 'Simulated Sw Profile',
                        data: [],
                        borderColor: '#1d4ed8',
                        borderWidth: 2.4,
                        pointRadius: 0,
                    },
                    {
                        label: 'Analytical Front Profile',
                        data: [],
                        borderColor: '#16a34a',
                        borderWidth: 2,
                        borderDash: [6, 4],
                        pointRadius: 0,
                    },
                ],
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        display: true,
                    },
                },
                scales: {
                    x: {
                        title: {
                            display: true,
                            text: 'Cell Index (i)',
                        },
                    },
                    y: {
                        min: 0,
                        max: 1,
                        title: {
                            display: true,
                            text: 'Water Saturation Sw',
                        },
                    },
                },
            },
        });

        updateProfileChart();
    });

    onDestroy(() => {
        chart?.destroy();
        chart = null;
    });

    function cellIndex(i: number, j: number, k: number) {
        return (k * nx * ny) + (j * nx) + i;
    }

    function k_rw(s_w: number) {
        const { s_wc, s_or, n_w } = rockProps;
        const s_eff = Math.max(0, Math.min(1, (s_w - s_wc) / (1 - s_wc - s_or)));
        return Math.pow(s_eff, n_w);
    }

    function k_ro(s_w: number) {
        const { s_wc, s_or, n_o } = rockProps;
        const s_eff = Math.max(0, Math.min(1, (1 - s_w - s_or) / (1 - s_wc - s_or)));
        return Math.pow(s_eff, n_o);
    }

    function fractionalFlow(s_w: number) {
        const { mu_w, mu_o } = fluidProps;
        const krw = k_rw(s_w);
        const kro = k_ro(s_w);
        const numerator = krw / mu_w;
        const denominator = numerator + (kro / mu_o);
        if (denominator === 0) return 0;
        return numerator / denominator;
    }

    function computeShockSw() {
        const { s_wc, s_or } = rockProps;
        const sMin = s_wc;
        const sMax = 1 - s_or;
        const swi = Math.max(sMin, Math.min(sMax, initialSaturation));

        let swShock = swi;
        let maxSlope = 0;
        for (let s = swi + 5e-4; s <= sMax; s += 5e-4) {
            const fw = fractionalFlow(s);
            const slope = fw / Math.max(1e-12, s - swi);
            if (slope > maxSlope && Number.isFinite(slope)) {
                maxSlope = slope;
                swShock = s;
            }
        }

        return { swi, swShock, dfwShock: maxSlope };
    }

    function buildSimulatedProfile() {
        if (!Array.isArray(gridState) || gridState.length === 0 || nx <= 0 || ny <= 0 || nz <= 0) {
            return Array.from({ length: Math.max(1, nx) }, () => null);
        }

        const row = Math.max(0, Math.min(ny - 1, selectedRow));
        const values: Array<number | null> = [];

        for (let i = 0; i < nx; i++) {
            const id = cellIndex(i, row, 0);
            const cell = gridState[id] ?? {};
            const sw = Number((cell as any).sat_water ?? (cell as any).satWater ?? (cell as any).sw);
            values.push(Number.isFinite(sw) ? Math.max(0, Math.min(1, sw)) : null);
        }

        return values;
    }

    function buildAnalyticalProfile() {
        if (scenarioMode !== 'waterflood' || injectionRate <= 0 || nx <= 0) {
            frontCellIndex = null;
            return Array.from({ length: Math.max(1, nx) }, () => null);
        }

        const { swi, swShock, dfwShock } = computeShockSw();
        const area = Math.max(1e-9, ny * cellDy * nz * cellDz);
        const vShock = (injectionRate / (area * Math.max(1e-9, reservoirPorosity))) * Math.max(0, dfwShock);
        const xFront = Math.max(0, Math.min(nx * cellDx, vShock * Math.max(0, simTime)));
        frontCellIndex = Math.max(0, Math.min(nx - 1, Math.floor(xFront / Math.max(1e-9, cellDx))));

        const profile = [];
        for (let i = 0; i < nx; i++) {
            const xCenter = (i + 0.5) * cellDx;
            profile.push(xCenter <= xFront ? swShock : swi);
        }
        return profile;
    }

    function updateProfileChart() {
        if (!chart) return;

        const labels = Array.from({ length: Math.max(1, nx) }, (_, idx) => `${idx}`);
        const simulated = buildSimulatedProfile();
        const analytical = buildAnalyticalProfile();

        chart.data.labels = labels;
        chart.data.datasets[0].data = simulated;
        chart.data.datasets[1].data = analytical;
        chart.update();
    }
</script>

<div class="card border border-base-300 bg-base-100 shadow-sm">
    <div class="card-body p-4 md:p-5">
        <div class="mb-2 flex flex-wrap items-end gap-2">
            <div>
                <h3 class="text-sm font-semibold">Sw Profile Along Injector-Producer Axis</h3>
                <p class="text-xs opacity-70">Current snapshot vs analytical flood-front profile (k = 0 plane).</p>
            </div>
            <label class="form-control ml-auto w-44">
                <span class="label-text text-xs">Row (j)</span>
                <input
                    type="number"
                    min="0"
                    max={Math.max(0, ny - 1)}
                    step="1"
                    class="input input-bordered input-xs"
                    bind:value={selectedRow}
                />
            </label>
        </div>

        <div style="height: min(34vh, 280px); width: 100%;">
            <canvas bind:this={chartCanvas}></canvas>
        </div>

        {#if frontCellIndex !== null}
            <div class="mt-2 text-xs opacity-80">Analytical front is near cell i ≈ {frontCellIndex} at t = {simTime.toFixed(2)} days.</div>
        {/if}
    </div>
</div>
