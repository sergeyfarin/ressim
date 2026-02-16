<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import FractionalFlow from './lib/FractionalFlow.svelte';
    import StaticPropertiesPanel from './lib/ui/StaticPropertiesPanel.svelte';
    import TimestepControlsPanel from './lib/ui/TimestepControlsPanel.svelte';
    import ReservoirPropertiesPanel from './lib/ui/ReservoirPropertiesPanel.svelte';
    import RelativeCapillaryPanel from './lib/ui/RelativeCapillaryPanel.svelte';
    import WellPropertiesPanel from './lib/ui/WellPropertiesPanel.svelte';
    import DynamicControlsPanel from './lib/ui/DynamicControlsPanel.svelte';
    import VisualizationReplayPanel from './lib/ui/VisualizationReplayPanel.svelte';
    import BenchmarkResultsCard from './lib/ui/BenchmarkResultsCard.svelte';

    let wasmReady = false;
    let simWorker: Worker | null = null;
    let runCompleted = false;
    let workerRunning = false;

    // UI inputs
    let nx = 15;
    let ny = 10;
    let nz = 10;
    let cellDx = 10;
    let cellDy = 10;
    let cellDz = 1;
    let delta_t_days = 0.25;
    let steps = 20;

    // Initial Conditions
    let initialPressure = 300.0;
    let initialSaturation = 0.3;
    const reservoirPorosity = 0.2;

    // Fluid properties (single source of truth for simulator + analytical)
    let mu_w = 0.5;
    let mu_o = 1.0;
    let rho_w = 1000.0;
    let rho_o = 800.0;
    let ooipM3 = 0;
    $: ooipM3 = nx * ny * nz * cellDx * cellDy * cellDz * reservoirPorosity * Math.max(0, 1 - initialSaturation);

    // Permeability
    let permMode: 'uniform' | 'random' | 'perLayer' = 'uniform';
    let uniformPermX = 100.0;
    let uniformPermY = 100.0;
    let uniformPermZ = 10.0;
    let minPerm = 50.0;
    let maxPerm = 200.0;
    let useRandomSeed = true;
    let randomSeed = 12345;
    let layerPermsX: number[] = [100, 150, 50, 200, 120, 1000, 90, 110, 130, 70];
    let layerPermsY: number[] = [100, 150, 50, 200, 120, 1000, 90, 110, 130, 70];
    let layerPermsZ: number[] = [10, 15, 5, 20, 12, 8, 9, 11, 13, 7];
    let scenarioPreset = 'custom';

    // Relative Permeability / Capillary
    let s_wc = 0.1;
    let s_or = 0.1;
    let n_w = 2.0;
    let n_o = 2.0;

    // Well inputs
    let well_radius = 0.1;
    let well_skin = 0.0;
    let injectorBhp = 400.0;
    let producerBhp = 100.0;
    let rateControlledWells = false;
    let targetInjectorRate = 350.0;
    let targetProducerRate = 350.0;
    let injectorI = 0;
    let injectorJ = 0;
    let producerI = nx - 1;
    let producerJ = 0;

    // Stability
    let max_sat_change_per_step = 0.1;
    let gravityEnabled = false;
    let capillaryEnabled = true;
    let capillaryPEntry = 5.0;
    let capillaryLambda = 2.0;

    // Display data
    let gridStateRaw = null;
    let wellStateRaw = null;
    let simTime = 0;
    let rateHistory = [];
    let analyticalProductionData = [];

    type BenchmarkRow = {
        name: string;
        pvBtSim: number;
        pvBtRef: number;
        relError: number;
        tolerance: number;
    };

    type BenchmarkMode = 'baseline' | 'refined';
    type BenchmarkModes = Record<BenchmarkMode, BenchmarkRow[]>;

    type WorkerProfile = {
        batchMs: number;
        avgStepMs: number;
        simStepMs: number;
        extractMs: number;
        snapshotsSent: number;
    };

    type ProfileStats = {
        batchMs: number;
        avgStepMs: number;
        extractMs: number;
        renderApplyMs: number;
        snapshotsSent: number;
    };

    type BenchmarkArtifact = {
        generatedAt?: string;
        source?: string;
        command?: string;
        defaultMode?: BenchmarkMode;
        modes?: Partial<BenchmarkModes>;
        cases?: BenchmarkRow[];
    };

    const fallbackBenchmarkModes: BenchmarkModes = {
        baseline: [
            {
                name: 'BL-Case-A',
                pvBtSim: 0.5239,
                pvBtRef: 0.5860,
                relError: 0.106,
                tolerance: 0.25,
            },
            {
                name: 'BL-Case-B',
                pvBtSim: 0.4768,
                pvBtRef: 0.5074,
                relError: 0.060,
                tolerance: 0.30,
            },
        ],
        refined: [
            {
                name: 'BL-Case-A-Refined',
                pvBtSim: 0.5649,
                pvBtRef: 0.5860,
                relError: 0.036,
                tolerance: 0.25,
            },
            {
                name: 'BL-Case-B-Refined',
                pvBtSim: 0.4907,
                pvBtRef: 0.5074,
                relError: 0.033,
                tolerance: 0.30,
            },
        ],
    };

    let benchmarkModes: BenchmarkModes = {
        baseline: [...fallbackBenchmarkModes.baseline],
        refined: [...fallbackBenchmarkModes.refined],
    };
    let selectedBenchmarkMode: BenchmarkMode = 'baseline';
    let benchmarkSource = 'fallback';
    let benchmarkGeneratedAt = '';
    let theme: 'dark' | 'light' = 'dark';

    const benchmarkModeLabel: Record<BenchmarkMode, string> = {
        baseline: 'Baseline (nx=24, dt=0.5 day)',
        refined: 'Refined (nx=96, dt=0.125 day)',
    };

    $: benchmarkRows = benchmarkModes[selectedBenchmarkMode] ?? [];

    $: baselineRelErrByCase = new Map(
        (benchmarkModes.baseline ?? []).map((row) => [row.name.replace('-Refined', ''), row.relError])
    );

    $: benchmarkRowsWithStatus = benchmarkRows.map((row) => ({
        ...row,
        passes: row.relError <= row.tolerance,
        improvementVsBaselinePp: (() => {
            const baseKey = row.name.replace('-Refined', '');
            const baselineRelErr = baselineRelErrByCase.get(baseKey);
            if (!Number.isFinite(baselineRelErr)) return null;
            return (baselineRelErr - row.relError) * 100.0;
        })(),
    }));

    $: allBenchmarksPass = benchmarkRowsWithStatus.every((row) => row.passes);

    // History / replay
    let history = [];
    let avgReservoirPressureSeries: Array<number | null> = [];
    let avgWaterSaturationSeries: Array<number | null> = [];
    let currentIndex = -1;
    let playing = false;
    let playSpeed = 2;
    let playTimer = null;
    const HISTORY_RECORD_INTERVAL = 2;
    const MAX_HISTORY_ENTRIES = 300;
    let showDebugState = false;
    let profileStats: ProfileStats = {
        batchMs: 0,
        avgStepMs: 0,
        extractMs: 0,
        renderApplyMs: 0,
        snapshotsSent: 0,
    };
    let ThreeDViewComponent = null;
    let RateChartComponent = null;
    let loadingThreeDView = false;

    // Visualization
    let showProperty: 'pressure' | 'saturation_water' | 'saturation_oil' | 'permeability_x' | 'permeability_y' | 'permeability_z' | 'porosity' = 'pressure';
    let legendRangeMode: 'fixed' | 'percentile' = 'percentile';
    let legendPercentileLow = 5;
    let legendPercentileHigh = 95;
    let legendFixedMin = 0;
    let legendFixedMax = 1;

    const scenarioPresets = {
        custom: null,
        bl_case_a_refined: {
            nx: 96,
            ny: 1,
            nz: 1,
            cellDx: 10,
            cellDy: 10,
            cellDz: 1,
            delta_t_days: 0.125,
            steps: 240,
            max_sat_change_per_step: 0.05,
            initialPressure: 300,
            initialSaturation: 0.1,
            mu_w: 0.5,
            mu_o: 1.0,
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.0,
            n_o: 2.0,
            gravityEnabled: false,
            capillaryEnabled: false,
            capillaryPEntry: 0.0,
            capillaryLambda: 2.0,
            permMode: 'uniform',
            uniformPermX: 2000,
            uniformPermY: 2000,
            uniformPermZ: 2000,
            injectorBhp: 500,
            producerBhp: 100,
            rateControlledWells: true,
            targetInjectorRate: 350,
            targetProducerRate: 350,
            injectorI: 0,
            injectorJ: 0,
            producerI: 95,
            producerJ: 0,
        },
        bl_case_b_refined: {
            nx: 96,
            ny: 1,
            nz: 1,
            cellDx: 10,
            cellDy: 10,
            cellDz: 1,
            delta_t_days: 0.125,
            steps: 240,
            max_sat_change_per_step: 0.05,
            initialPressure: 300,
            initialSaturation: 0.15,
            mu_w: 0.6,
            mu_o: 1.4,
            s_wc: 0.15,
            s_or: 0.15,
            n_w: 2.2,
            n_o: 2.0,
            gravityEnabled: false,
            capillaryEnabled: false,
            capillaryPEntry: 0.0,
            capillaryLambda: 2.0,
            permMode: 'uniform',
            uniformPermX: 2000,
            uniformPermY: 2000,
            uniformPermZ: 2000,
            injectorBhp: 500,
            producerBhp: 100,
            rateControlledWells: true,
            targetInjectorRate: 350,
            targetProducerRate: 350,
            injectorI: 0,
            injectorJ: 0,
            producerI: 95,
            producerJ: 0,
        },
        bl_aligned_homogeneous: {
            nx: 48,
            ny: 1,
            nz: 1,
            cellDx: 5,
            cellDy: 10,
            cellDz: 10,
            delta_t_days: 0.1,
            steps: 120,
            max_sat_change_per_step: 0.05,
            initialPressure: 300,
            initialSaturation: 0.2,
            mu_w: 0.5,
            mu_o: 1.0,
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.0,
            n_o: 2.0,
            gravityEnabled: false,
            capillaryEnabled: false,
            capillaryPEntry: 0.0,
            capillaryLambda: 2.0,
            permMode: 'uniform',
            uniformPermX: 150,
            uniformPermY: 150,
            uniformPermZ: 150,
            rateControlledWells: true,
            targetInjectorRate: 250,
            targetProducerRate: 250,
            injectorI: 0,
            injectorJ: 0,
            producerI: 47,
            producerJ: 0,
        },
        bl_aligned_mild_capillary: {
            nx: 48,
            ny: 1,
            nz: 1,
            cellDx: 5,
            cellDy: 10,
            cellDz: 10,
            delta_t_days: 0.1,
            steps: 120,
            max_sat_change_per_step: 0.05,
            initialPressure: 300,
            initialSaturation: 0.2,
            mu_w: 0.5,
            mu_o: 1.0,
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.0,
            n_o: 2.0,
            gravityEnabled: false,
            capillaryEnabled: true,
            capillaryPEntry: 0.75,
            capillaryLambda: 3.2,
            permMode: 'uniform',
            uniformPermX: 150,
            uniformPermY: 150,
            uniformPermZ: 150,
            rateControlledWells: true,
            targetInjectorRate: 250,
            targetProducerRate: 250,
            injectorI: 0,
            injectorJ: 0,
            producerI: 47,
            producerJ: 0,
        },
        bl_aligned_mobility_balanced: {
            nx: 48,
            ny: 1,
            nz: 1,
            cellDx: 5,
            cellDy: 10,
            cellDz: 10,
            delta_t_days: 0.1,
            steps: 120,
            max_sat_change_per_step: 0.05,
            initialPressure: 300,
            initialSaturation: 0.2,
            mu_w: 0.8,
            mu_o: 1.0,
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.1,
            n_o: 1.9,
            gravityEnabled: false,
            capillaryEnabled: false,
            capillaryPEntry: 0.0,
            capillaryLambda: 2.0,
            permMode: 'uniform',
            uniformPermX: 150,
            uniformPermY: 150,
            uniformPermZ: 150,
            rateControlledWells: true,
            targetInjectorRate: 250,
            targetProducerRate: 250,
            injectorI: 0,
            injectorJ: 0,
            producerI: 47,
            producerJ: 0,
        },
        baseline_waterflood: {
            initialPressure: 300,
            initialSaturation: 0.3,
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.0,
            n_o: 2.0,
            capillaryEnabled: true,
            capillaryPEntry: 5.0,
            capillaryLambda: 2.0,
            permMode: 'random',
            minPerm: 50,
            maxPerm: 200,
            useRandomSeed: true,
            randomSeed: 12345,
            injectorI: 0,
            injectorJ: 0,
            producerI: nx - 1,
            producerJ: 0,
        },
        high_contrast_layers: {
            initialPressure: 320,
            initialSaturation: 0.25,
            s_wc: 0.12,
            s_or: 0.12,
            n_w: 2.2,
            n_o: 2.2,
            capillaryEnabled: true,
            capillaryPEntry: 8.0,
            capillaryLambda: 2.5,
            permMode: 'perLayer',
            layerPermsXStr: '30, 40, 60, 90, 150, 400, 150, 90, 60, 40',
            layerPermsYStr: '30, 40, 60, 90, 150, 400, 150, 90, 60, 40',
            layerPermsZStr: '3, 4, 6, 9, 15, 40, 15, 9, 6, 4',
            injectorI: 0,
            injectorJ: 0,
            producerI: nx - 1,
            producerJ: 0,
        },
        viscous_fingering_risk: {
            initialPressure: 280,
            initialSaturation: 0.2,
            s_wc: 0.08,
            s_or: 0.15,
            n_w: 1.6,
            n_o: 2.4,
            capillaryEnabled: true,
            capillaryPEntry: 3.0,
            capillaryLambda: 1.6,
            permMode: 'random',
            minPerm: 20,
            maxPerm: 500,
            useRandomSeed: true,
            randomSeed: 987654,
            injectorI: 0,
            injectorJ: 0,
            producerI: nx - 1,
            producerJ: 0,
        },
    };

    const scenarioPresetOptions = [
        { value: 'custom', label: 'Custom' },
        { value: 'bl_case_a_refined', label: 'BL Case A — Refined (benchmark-like)' },
        { value: 'bl_case_b_refined', label: 'BL Case B — Refined (benchmark-like)' },
        { value: 'bl_aligned_homogeneous', label: 'BL Aligned — Homogeneous 1D-like' },
        { value: 'bl_aligned_mild_capillary', label: 'BL Aligned — Mild Capillary' },
        { value: 'bl_aligned_mobility_balanced', label: 'BL Aligned — Mobility Balanced' },
        { value: 'baseline_waterflood', label: 'Baseline Waterflood' },
        { value: 'high_contrast_layers', label: 'High Contrast Layers' },
        { value: 'viscous_fingering_risk', label: 'Viscous Fingering Risk' },
    ];

    function parseLayerValues(value: unknown): number[] {
        if (Array.isArray(value)) {
            return value.map((v) => Number(v)).filter((v) => Number.isFinite(v) && v > 0);
        }
        return String(value)
            .split(',')
            .map((v) => Number(v.trim()))
            .filter((v) => Number.isFinite(v) && v > 0);
    }

    function normalizeLayerArray(values: number[], fallback: number, length: number): number[] {
        return Array.from({ length }, (_, i) => {
            const value = Number(values[i]);
            return Number.isFinite(value) && value > 0 ? value : fallback;
        });
    }

    function syncLayerArraysToGrid() {
        layerPermsX = normalizeLayerArray(layerPermsX, uniformPermX, nz);
        layerPermsY = normalizeLayerArray(layerPermsY, uniformPermY, nz);
        layerPermsZ = normalizeLayerArray(layerPermsZ, uniformPermZ, nz);
    }

    function isPermMode(value: string): value is 'uniform' | 'random' | 'perLayer' {
        return value === 'uniform' || value === 'random' || value === 'perLayer';
    }

    $: if (layerPermsX.length !== nz || layerPermsY.length !== nz || layerPermsZ.length !== nz) {
        syncLayerArraysToGrid();
    }

    $: producerI = Math.max(0, Math.min(nx - 1, Number(producerI)));
    $: producerJ = Math.max(0, Math.min(ny - 1, Number(producerJ)));
    $: injectorI = Math.max(0, Math.min(nx - 1, Number(injectorI)));
    $: injectorJ = Math.max(0, Math.min(ny - 1, Number(injectorJ)));

    const scenarioPresetSetters: Record<string, (value: unknown) => void> = {
        nx: (value) => nx = Math.max(1, Math.round(Number(value) || 1)),
        ny: (value) => ny = Math.max(1, Math.round(Number(value) || 1)),
        nz: (value) => nz = Math.max(1, Math.round(Number(value) || 1)),
        cellDx: (value) => cellDx = Number(value),
        cellDy: (value) => cellDy = Number(value),
        cellDz: (value) => cellDz = Number(value),
        delta_t_days: (value) => delta_t_days = Number(value),
        steps: (value) => steps = Math.max(1, Math.round(Number(value) || 1)),
        initialPressure: (value) => initialPressure = Number(value),
        initialSaturation: (value) => initialSaturation = Number(value),
        mu_w: (value) => mu_w = Number(value),
        mu_o: (value) => mu_o = Number(value),
        s_wc: (value) => s_wc = Number(value),
        s_or: (value) => s_or = Number(value),
        n_w: (value) => n_w = Number(value),
        n_o: (value) => n_o = Number(value),
        max_sat_change_per_step: (value) => max_sat_change_per_step = Number(value),
        gravityEnabled: (value) => gravityEnabled = Boolean(value),
        capillaryEnabled: (value) => capillaryEnabled = Boolean(value),
        capillaryPEntry: (value) => capillaryPEntry = Number(value),
        capillaryLambda: (value) => capillaryLambda = Number(value),
        permMode: (value) => {
            const candidate = String(value);
            if (isPermMode(candidate)) {
                permMode = candidate;
            }
        },
        uniformPermX: (value) => uniformPermX = Number(value),
        uniformPermY: (value) => uniformPermY = Number(value),
        uniformPermZ: (value) => uniformPermZ = Number(value),
        minPerm: (value) => minPerm = Number(value),
        maxPerm: (value) => maxPerm = Number(value),
        useRandomSeed: (value) => useRandomSeed = Boolean(value),
        randomSeed: (value) => randomSeed = Number(value),
        layerPermsXStr: (value) => layerPermsX = parseLayerValues(value),
        layerPermsYStr: (value) => layerPermsY = parseLayerValues(value),
        layerPermsZStr: (value) => layerPermsZ = parseLayerValues(value),
        injectorBhp: (value) => injectorBhp = Number(value),
        producerBhp: (value) => producerBhp = Number(value),
        rateControlledWells: (value) => rateControlledWells = Boolean(value),
        targetInjectorRate: (value) => targetInjectorRate = Number(value),
        targetProducerRate: (value) => targetProducerRate = Number(value),
        injectorI: (value) => injectorI = Number(value),
        injectorJ: (value) => injectorJ = Number(value),
        producerI: (value) => producerI = Number(value),
        producerJ: (value) => producerJ = Number(value),
    };

    $: cellDx = Math.max(0.1, Number(cellDx) || 0.1);
    $: cellDy = Math.max(0.1, Number(cellDy) || 0.1);
    $: cellDz = Math.max(0.1, Number(cellDz) || 0.1);
    $: steps = Math.max(1, Math.round(Number(steps) || 1));
    $: targetInjectorRate = Math.max(0, Number(targetInjectorRate) || 0);
    $: targetProducerRate = Math.max(0, Number(targetProducerRate) || 0);

    function applyScenarioPreset() {
        const preset = scenarioPresets[scenarioPreset] as Record<string, unknown> | null;
        if (!preset) return;

        for (const [key, value] of Object.entries(preset)) {
            if (value === undefined) continue;
            scenarioPresetSetters[key]?.(value);
        }
    }

    function pushHistoryEntry(entry) {
        history = [...history, entry];
        if (history.length > MAX_HISTORY_ENTRIES) {
            const overflow = history.length - MAX_HISTORY_ENTRIES;
            history = history.slice(overflow);
            currentIndex = Math.max(0, currentIndex - overflow);
        }
        currentIndex = history.length - 1;
    }

    function updateProfileStats(profile: Partial<WorkerProfile> = {}, renderApplyMs = 0) {
        profileStats = {
            batchMs: Number(profile.batchMs ?? profileStats.batchMs ?? 0),
            avgStepMs: Number(profile.avgStepMs ?? profile.simStepMs ?? profileStats.avgStepMs ?? 0),
            extractMs: Number(profile.extractMs ?? profileStats.extractMs ?? 0),
            renderApplyMs,
            snapshotsSent: Number(profile.snapshotsSent ?? profileStats.snapshotsSent ?? 0),
        };
    }

    function applyWorkerState(message) {
        const renderStart = performance.now();
        gridStateRaw = message.grid;
        wellStateRaw = message.wells;
        simTime = message.time;
        rateHistory = message.rateHistory;

        if (message.recordHistory) {
            pushHistoryEntry({
                time: message.time,
                grid: message.grid,
                wells: message.wells,
            });
        }

        updateProfileStats(message.profile, performance.now() - renderStart);
    }

    function handleWorkerMessage(event) {
        const { type, ...message } = event.data ?? {};
        if (type === 'ready') {
            wasmReady = true;
            initSimulator();
            return;
        }

        if (type === 'state') {
            applyWorkerState(message);
            return;
        }

        if (type === 'batchComplete') {
            workerRunning = false;
            runCompleted = true;
            updateProfileStats(message.profile, profileStats.renderApplyMs);
            applyHistoryIndex(history.length - 1);
            return;
        }

        if (type === 'error') {
            workerRunning = false;
            console.error('Simulation worker error:', message.message);
            alert(`Simulation error: ${message.message}`);
        }
    }

    function setupWorker() {
        simWorker = new Worker(new URL('./lib/sim.worker.js', import.meta.url), { type: 'module' });
        simWorker.onmessage = handleWorkerMessage;
        simWorker.postMessage({ type: 'init' });
    }

    async function loadRateChartModule() {
        try {
            const rateChartModule = await import('./lib/RateChart.svelte');
            RateChartComponent = rateChartModule.default;
        } catch (error) {
            console.error('Failed to load rate chart module:', error);
        }
    }

    async function loadThreeDViewModule() {
        if (ThreeDViewComponent || loadingThreeDView) return;
        loadingThreeDView = true;
        try {
            const threeDModule = await import('./lib/3dview.svelte');
            ThreeDViewComponent = threeDModule.default;
        } catch (error) {
            console.error('Failed to load 3D view module:', error);
        } finally {
            loadingThreeDView = false;
        }
    }

    function toggleTheme() {
        theme = theme === 'dark' ? 'light' : 'dark';
    }

    onMount(() => {
        const savedTheme = localStorage.getItem('ressim-theme');
        if (savedTheme === 'light' || savedTheme === 'dark') {
            theme = savedTheme;
        }
        document.documentElement.setAttribute('data-theme', theme);

        setupWorker();
        loadRateChartModule();
        loadBenchmarkResults();
    });

    $: if (typeof document !== 'undefined') {
        document.documentElement.setAttribute('data-theme', theme);
    }

    $: if (typeof localStorage !== 'undefined') {
        localStorage.setItem('ressim-theme', theme);
    }

    $: if (!ThreeDViewComponent && (gridStateRaw || history.length > 0)) {
        loadThreeDViewModule();
    }

    onDestroy(() => {
        stopPlaying();
        if (simWorker) {
            simWorker.postMessage({ type: 'dispose' });
            simWorker.terminate();
            simWorker = null;
        }
    });

    function initSimulator() {
        if (!wasmReady || !simWorker) {
            alert('WASM not ready yet');
            return;
        }

        const validWellLocations =
            Number.isInteger(injectorI) && Number.isInteger(injectorJ) &&
            Number.isInteger(producerI) && Number.isInteger(producerJ) &&
            injectorI >= 0 && injectorI < nx && injectorJ >= 0 && injectorJ < ny &&
            producerI >= 0 && producerI < nx && producerJ >= 0 && producerJ < ny;

        if (!validWellLocations) {
            alert('Invalid well location. Ensure i/j are within grid bounds.');
            return;
        }

        history = [];
        currentIndex = -1;
        runCompleted = false;

        simWorker.postMessage({
            type: 'create',
            payload: buildCreatePayload(),
        });
        runSteps();
    }

    function buildCreatePayload() {
        const useUniformPerm = permMode === 'uniform';
        const permsX = useUniformPerm ? Array.from({ length: nz }, () => Number(uniformPermX)) : layerPermsX.map(Number);
        const permsY = useUniformPerm ? Array.from({ length: nz }, () => Number(uniformPermY)) : layerPermsY.map(Number);
        const permsZ = useUniformPerm ? Array.from({ length: nz }, () => Number(uniformPermZ)) : layerPermsZ.map(Number);

        return {
            nx: Number(nx),
            ny: Number(ny),
            nz: Number(nz),
            cellDx: Number(cellDx),
            cellDy: Number(cellDy),
            cellDz: Number(cellDz),
            initialPressure: Number(initialPressure),
            initialSaturation: Number(initialSaturation),
            mu_w: Number(mu_w),
            mu_o: Number(mu_o),
            rho_w: Number(rho_w),
            rho_o: Number(rho_o),
            s_wc: Number(s_wc),
            s_or: Number(s_or),
            n_w: Number(n_w),
            n_o: Number(n_o),
            max_sat_change_per_step: Number(max_sat_change_per_step),
            capillaryEnabled: Boolean(capillaryEnabled),
            capillaryPEntry: Number(capillaryPEntry),
            capillaryLambda: Number(capillaryLambda),
            gravityEnabled: Boolean(gravityEnabled),
            permMode: useUniformPerm ? 'perLayer' : permMode,
            minPerm: Number(minPerm),
            maxPerm: Number(maxPerm),
            useRandomSeed: Boolean(useRandomSeed),
            randomSeed: Number(randomSeed),
            permsX,
            permsY,
            permsZ,
            well_radius: Number(well_radius),
            well_skin: Number(well_skin),
            injectorBhp: Number(injectorBhp),
            producerBhp: Number(producerBhp),
            rateControlledWells: Boolean(rateControlledWells),
            targetInjectorRate: Number(targetInjectorRate),
            targetProducerRate: Number(targetProducerRate),
            injectorI: Number(injectorI),
            injectorJ: Number(injectorJ),
            producerI: Number(producerI),
            producerJ: Number(producerJ),
        };
    }

    async function loadBenchmarkResults() {
        try {
            const artifactUrl = `${import.meta.env.BASE_URL}benchmark-results.json`;
            const response = await fetch(artifactUrl, { cache: 'no-store' });
            if (!response.ok) return;

            const artifact = (await response.json()) as BenchmarkArtifact;
            const normalizeRows = (rows: BenchmarkRow[] | undefined): BenchmarkRow[] => (rows ?? [])
                .map((row) => ({
                    name: String(row.name),
                    pvBtSim: Number(row.pvBtSim),
                    pvBtRef: Number(row.pvBtRef),
                    relError: Number(row.relError),
                    tolerance: Number(row.tolerance),
                }))
                .filter((row) =>
                    row.name.length > 0 &&
                    Number.isFinite(row.pvBtSim) &&
                    Number.isFinite(row.pvBtRef) &&
                    Number.isFinite(row.relError) &&
                    Number.isFinite(row.tolerance)
                );

            const normalizedBaseline = normalizeRows(artifact.modes?.baseline ?? artifact.cases);
            const normalizedRefined = normalizeRows(artifact.modes?.refined);

            if (normalizedBaseline.length > 0) {
                benchmarkModes = {
                    baseline: normalizedBaseline,
                    refined: normalizedRefined.length > 0 ? normalizedRefined : benchmarkModes.refined,
                };
                selectedBenchmarkMode = artifact.defaultMode ?? 'baseline';
                benchmarkSource = artifact.source ? String(artifact.source) : 'artifact';
                benchmarkGeneratedAt = artifact.generatedAt ? String(artifact.generatedAt) : '';
            }
        } catch (error) {
            console.warn('Failed to load benchmark-results artifact, using fallback values.', error);
        }
    }

    function stepOnce() {
        runSimulationBatch(1, 1);
    }

    function runSteps() {
        runSimulationBatch(Number(steps), HISTORY_RECORD_INTERVAL);
    }

    function runSimulationBatch(batchSteps: number, historyInterval: number) {
        if (!simWorker || workerRunning) return;
        workerRunning = true;
        simWorker.postMessage({
            type: 'run',
            payload: {
                steps: batchSteps,
                deltaTDays: Number(delta_t_days),
                historyInterval,
            }
        });
    }



    /* Playback controls */
    function play() {
        if (history.length === 0) return;
        playing = true;
        stopPlaying();
        playTimer = setInterval(() => {
            next();
            if (currentIndex >= history.length - 1) {
                stopPlaying();
            }
        }, 1000 / playSpeed);
    }

    function stopPlaying() {
        playing = false;
        if (playTimer) {
            clearInterval(playTimer);
            playTimer = null;
        }
    }

    function togglePlay() {
        if (playing) stopPlaying(); else play();
    }

    function next() {
        if (history.length === 0) return;
        currentIndex = Math.min(history.length - 1, currentIndex + 1);
        applyHistoryIndex(currentIndex);
    }

    function prev() {
        if (history.length === 0) return;
        currentIndex = Math.max(0, currentIndex - 1);
        applyHistoryIndex(currentIndex);
    }

    function applyHistoryIndex(idx) {
        if (idx < 0 || idx >= history.length) return;
        const entry = history[idx];
        gridStateRaw = entry.grid;
        wellStateRaw = entry.wells;
        simTime = entry.time;
        // We don't update rateHistory here, as it's cumulative
    }

    $: replayTime = history.length > 0 && currentIndex >= 0 && currentIndex < history.length
        ? history[currentIndex].time
        : null;

    function computeAveragePressure(grid: Array<{ pressure?: number }>): number {
        if (!Array.isArray(grid) || grid.length === 0) return 0;
        let sum = 0;
        let count = 0;
        for (const cell of grid) {
            const value = Number(cell?.pressure);
            if (Number.isFinite(value)) {
                sum += value;
                count += 1;
            }
        }
        return count > 0 ? sum / count : 0;
    }

    function computeAverageWaterSaturation(
        grid: Array<{ sat_water?: number; satWater?: number; sw?: number }>
    ): number {
        if (!Array.isArray(grid) || grid.length === 0) return 0;
        let sum = 0;
        let count = 0;
        for (const cell of grid) {
            const value = Number(cell?.sat_water ?? cell?.satWater ?? cell?.sw);
            if (Number.isFinite(value)) {
                sum += value;
                count += 1;
            }
        }
        return count > 0 ? sum / count : 0;
    }

    function buildAvgPressureSeries(
        ratePoints: Array<{ time?: number }>,
        historyEntries: Array<{ time?: number; grid?: Array<{ pressure?: number }> }>
    ): Array<number | null> {
        if (!Array.isArray(ratePoints) || ratePoints.length === 0) return [];

        const snapshots = historyEntries
            .filter((entry) => Number.isFinite(Number(entry?.time)) && Array.isArray(entry?.grid))
            .map((entry) => ({
                time: Number(entry.time),
                avgPressure: computeAveragePressure(entry.grid ?? []),
            }))
            .sort((a, b) => a.time - b.time);

        if (snapshots.length === 0) {
            return ratePoints.map(() => null);
        }

        let snapIdx = 0;
        let currentAvg = snapshots[0].avgPressure;
        const aligned = [];

        for (const point of ratePoints) {
            const t = Number(point?.time ?? 0);
            while (snapIdx + 1 < snapshots.length && snapshots[snapIdx + 1].time <= t) {
                snapIdx += 1;
                currentAvg = snapshots[snapIdx].avgPressure;
            }
            aligned.push(currentAvg);
        }

        return aligned;
    }

    $: avgReservoirPressureSeries = buildAvgPressureSeries(rateHistory, history);

    function buildAvgWaterSaturationSeries(
        ratePoints: Array<{ time?: number }>,
        historyEntries: Array<{ time?: number; grid?: Array<{ sat_water?: number; satWater?: number; sw?: number }> }>
    ): Array<number | null> {
        if (!Array.isArray(ratePoints) || ratePoints.length === 0) return [];

        const snapshots = historyEntries
            .filter((entry) => Number.isFinite(Number(entry?.time)) && Array.isArray(entry?.grid))
            .map((entry) => ({
                time: Number(entry.time),
                avgSw: computeAverageWaterSaturation(entry.grid ?? []),
            }))
            .sort((a, b) => a.time - b.time);

        if (snapshots.length === 0) {
            return ratePoints.map(() => null);
        }

        let snapIdx = 0;
        let currentAvg = snapshots[0].avgSw;
        const aligned = [];

        for (const point of ratePoints) {
            const t = Number(point?.time ?? 0);
            while (snapIdx + 1 < snapshots.length && snapshots[snapIdx + 1].time <= t) {
                snapIdx += 1;
                currentAvg = snapshots[snapIdx].avgSw;
            }
            aligned.push(currentAvg);
        }

        return aligned;
    }

    $: avgWaterSaturationSeries = buildAvgWaterSaturationSeries(rateHistory, history);

    
</script>

<main class="min-h-screen bg-base-200 text-base-content" data-theme={theme}>
    <div class="mx-auto max-w-[1600px] space-y-4 p-4 lg:p-6">
        <FractionalFlow
            rockProps={{ s_wc, s_or, n_w, n_o }}
            fluidProps={{ mu_w, mu_o }}
            {initialSaturation}
            timeHistory={rateHistory.map((point) => point.time)}
            injectionRate={rateHistory.find(r => r.total_injection > 0)?.total_injection ?? 0}
            reservoir={{ length: nx * cellDx, area: ny * cellDy * nz * cellDz, porosity: reservoirPorosity }}
            on:analyticalData={(e) => analyticalProductionData = e.detail.production}
        />

        <header class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div>
                <h1 class="text-2xl font-bold lg:text-3xl">Simplified Reservoir Simulation Model</h1>
                <p class="text-sm opacity-80">Interactive two-phase simulation with 3D visualisation fully in browser.</p>
            </div>
            <button class="btn btn-sm btn-outline" on:click={toggleTheme}>
                {theme === 'dark' ? 'Switch to Light' : 'Switch to Dark'}
            </button>
        </header>

        <div class="grid grid-cols-1 gap-4 md:grid-cols-12">
            <aside class="space-y-3 md:col-span-3 md:max-w-[280px]">
                <div class="card border border-base-300 bg-base-100 shadow-sm">
                    <div class="card-body p-3">
                        <label class="form-control">
                            <span class="label-text text-xs">Scenario Preset</span>
                            <select class="select select-bordered select-sm w-full" bind:value={scenarioPreset} on:change={applyScenarioPreset}>
                                {#each scenarioPresetOptions as option}
                                    <option value={option.value}>{option.label}</option>
                                {/each}
                            </select>
                        </label>
                    </div>
                </div>
                <DynamicControlsPanel
                    bind:steps
                    {wasmReady}
                    {workerRunning}
                    {runCompleted}
                    {simTime}
                    historyLength={history.length}
                    {profileStats}
                    onRunSteps={runSteps}
                    onStepOnce={stepOnce}
                    onInitSimulator={initSimulator}
                />
                <StaticPropertiesPanel
                    bind:nx
                    bind:ny
                    bind:nz
                    bind:cellDx
                    bind:cellDy
                    bind:cellDz
                />

                <ReservoirPropertiesPanel
                    bind:initialPressure
                    bind:initialSaturation
                    bind:gravityEnabled
                    bind:permMode
                    bind:uniformPermX
                    bind:uniformPermY
                    bind:uniformPermZ
                    bind:useRandomSeed
                    bind:randomSeed
                    bind:minPerm
                    bind:maxPerm
                    bind:nz
                    bind:layerPermsX
                    bind:layerPermsY
                    bind:layerPermsZ
                />

                <RelativeCapillaryPanel
                    bind:s_wc
                    bind:s_or
                    bind:n_w
                    bind:n_o
                    bind:capillaryEnabled
                    bind:capillaryPEntry
                    bind:capillaryLambda
                />

                <WellPropertiesPanel
                    bind:well_radius
                    bind:well_skin
                    bind:nx
                    bind:ny
                    bind:injectorI
                    bind:injectorJ
                    bind:producerI
                    bind:producerJ
                />

                <TimestepControlsPanel
                    bind:delta_t_days
                    bind:max_sat_change_per_step
                />

                

            </aside>

            <section class="space-y-3 md:col-span-9">
                <div class="card border border-base-300 bg-base-100 shadow-sm">
                    <div class="card-body p-4 md:p-5">
                        {#if RateChartComponent}
                            <svelte:component
                                this={RateChartComponent}
                                {rateHistory}
                                {analyticalProductionData}
                                {avgReservoirPressureSeries}
                                {avgWaterSaturationSeries}
                                {ooipM3}
                                {theme}
                            />
                        {:else}
                            <div class="text-sm opacity-70">Loading rate chart…</div>
                        {/if}
                    </div>
                </div>

                <div class="card border border-base-300 bg-base-100 shadow-sm">
                    <div class="card-body p-4 md:p-5">
                        {#if ThreeDViewComponent}
                            <svelte:component
                                this={ThreeDViewComponent}
                                nx={nx}
                                ny={ny}
                                nz={nz}
                                {theme}
                                gridState={gridStateRaw}
                                showProperty={showProperty}
                                legendRangeMode={legendRangeMode}
                                legendPercentileLow={legendPercentileLow}
                                legendPercentileHigh={legendPercentileHigh}
                                legendFixedMin={legendFixedMin}
                                legendFixedMax={legendFixedMax}
                                history={history}
                                currentIndex={currentIndex}
                                wellState={wellStateRaw}
                            />
                        {:else}
                            <div class="flex items-center justify-center rounded border border-base-300 bg-base-200" style="height: clamp(240px, 35vh, 420px);">
                                <button class="btn btn-sm" on:click={loadThreeDViewModule}>Load 3D view</button>
                            </div>
                        {/if}

                        <div class="mt-4 border-t border-base-300 pt-4">
                            <VisualizationReplayPanel
                                bind:showProperty
                                bind:legendRangeMode
                                bind:legendPercentileLow
                                bind:legendPercentileHigh
                                bind:legendFixedMin
                                bind:legendFixedMax
                                historyLength={history.length}
                                bind:currentIndex
                                replayTime={replayTime}
                                bind:playing
                                bind:playSpeed
                                bind:showDebugState
                                onApplyHistoryIndex={applyHistoryIndex}
                                onPrev={prev}
                                onNext={next}
                                onTogglePlay={togglePlay}
                            />
                        </div>
                    </div>
                </div>

                <!-- <BenchmarkResultsCard
                    {benchmarkSource}
                    {benchmarkGeneratedAt}
                    {selectedBenchmarkMode}
                    {benchmarkModeLabel}
                    {benchmarkRowsWithStatus}
                    {allBenchmarksPass}
                    onSelectMode={(mode) => selectedBenchmarkMode = mode}
                /> -->

                {#if showDebugState}
                    <div class="card border border-base-300 bg-base-100 shadow-sm">
                        <div class="card-body grid gap-4 p-4 lg:grid-cols-2">
                            <div>
                                <h4 class="mb-2 text-sm font-semibold">Grid State (current)</h4>
                                <pre class="max-h-[420px] overflow-auto rounded border border-base-300 bg-base-200 p-2 text-xs">{JSON.stringify(gridStateRaw, null, 2)}</pre>
                            </div>
                            <div>
                                <h4 class="mb-2 text-sm font-semibold">Well State (current)</h4>
                                <pre class="max-h-[420px] overflow-auto rounded border border-base-300 bg-base-200 p-2 text-xs">{JSON.stringify(wellStateRaw, null, 2)}</pre>
                            </div>
                        </div>
                    </div>
                {/if}
            </section>
        </div>
    </div>
</main>