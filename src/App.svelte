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
    import SwProfileChart from './lib/SwProfileChart.svelte';

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
    let c_o = 1e-5;
    let c_w = 3e-6;
    let rock_compressibility = 1e-6;
    let depth_reference = 0.0;
    let volume_expansion_o = 1.0;
    let volume_expansion_w = 1.0;
    let rho_w = 1000.0;
    let rho_o = 800.0;
    let ooipM3 = 0;
    let poreVolumeM3 = 0;
    $: ooipM3 = nx * ny * nz * cellDx * cellDy * cellDz * reservoirPorosity * Math.max(0, 1 - initialSaturation);
    $: poreVolumeM3 = nx * ny * nz * cellDx * cellDy * cellDz * reservoirPorosity;

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
    let injectorControlMode: 'rate' | 'pressure' = 'pressure';
    let producerControlMode: 'rate' | 'pressure' = 'pressure';
    let injectorEnabled = true;
    let rateControlledWells = false;
    let targetInjectorRate = 350.0;
    let targetProducerRate = 350.0;
    let injectorI = 0;
    let injectorJ = 0;
    let producerI = nx - 1;
    let producerJ = 0;

    // Stability
    let max_sat_change_per_step = 0.1;
    let max_pressure_change_per_step = 75.0;
    let max_well_rate_change_fraction = 0.75;
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
    let analyticalMeta: { mode: 'waterflood' | 'depletion'; shapeFactor: number | null; shapeLabel: string } = {
        mode: 'waterflood',
        shapeFactor: null,
        shapeLabel: '',
    };
    let runtimeWarning = '';
    let solverWarning = '';
    let runtimeError = '';
    let vizRevision = 0;
    let modelReinitNotice = '';
    let modelNeedsReinit = false;
    let modelResetKey = '';
    let skipNextAutoModelReset = false;
    let validationState: { errors: Record<string, string>; warnings: string[] } = { errors: {}, warnings: [] };
    let validationErrors: Record<string, string> = {};
    let validationWarnings: string[] = [];
    let hasValidationErrors = false;
    let estimatedRunSeconds = 0;
    let longRunEstimate = false;

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

    const EMPTY_PROFILE_STATS: ProfileStats = {
        batchMs: 0,
        avgStepMs: 0,
        extractMs: 0,
        renderApplyMs: 0,
        snapshotsSent: 0,
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
    let latestInjectionRate = 0;

    $: latestInjectionRate = (() => {
        if (!Array.isArray(rateHistory) || rateHistory.length === 0) return 0;
        for (let i = rateHistory.length - 1; i >= 0; i--) {
            const q = Number(rateHistory[i]?.total_injection);
            if (Number.isFinite(q) && q > 0) return q;
        }
        return 0;
    })();

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
    const HISTORY_RECORD_INTERVAL = 1;
    const MAX_HISTORY_ENTRIES = 300;
    let showDebugState = false;
    let profileStats: ProfileStats = { ...EMPTY_PROFILE_STATS };
    let ThreeDViewComponent = null;
    let RateChartComponent = null;
    let loadingThreeDView = false;
    let lastCreateSignature = '';
    let pendingAutoReinit = false;

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
        depletion_corner_producer: {
            nx: 48,
            ny: 1,
            nz: 1,
            cellDx: 10,
            cellDy: 10,
            cellDz: 5,
            delta_t_days: 0.1,
            steps: 180,
            initialPressure: 300,
            initialSaturation: 0.25,
            injectorEnabled: false,
            producerControlMode: 'pressure',
            producerBhp: 80,
            injectorI: 0,
            injectorJ: 0,
            producerI: 0,
            producerJ: 0,
        },
        depletion_center_producer: {
            nx: 49,
            ny: 49,
            nz: 1,
            cellDx: 10,
            cellDy: 10,
            cellDz: 5,
            delta_t_days: 0.1,
            steps: 220,
            initialPressure: 300,
            initialSaturation: 0.25,
            injectorEnabled: false,
            producerControlMode: 'pressure',
            producerBhp: 80,
            injectorI: 0,
            injectorJ: 0,
            producerI: 24,
            producerJ: 24,
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
        { value: 'depletion_corner_producer', label: 'Depletion — Corner Producer' },
        { value: 'depletion_center_producer', label: 'Depletion — Center Producer' },
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
        c_o: (value) => c_o = Number(value),
        c_w: (value) => c_w = Number(value),
        rock_compressibility: (value) => rock_compressibility = Number(value),
        depth_reference: (value) => depth_reference = Number(value),
        volume_expansion_o: (value) => volume_expansion_o = Number(value),
        volume_expansion_w: (value) => volume_expansion_w = Number(value),
        s_wc: (value) => s_wc = Number(value),
        s_or: (value) => s_or = Number(value),
        n_w: (value) => n_w = Number(value),
        n_o: (value) => n_o = Number(value),
        max_sat_change_per_step: (value) => max_sat_change_per_step = Number(value),
        max_pressure_change_per_step: (value) => max_pressure_change_per_step = Number(value),
        max_well_rate_change_fraction: (value) => max_well_rate_change_fraction = Number(value),
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
        rateControlledWells: (value) => {
            rateControlledWells = Boolean(value);
            if (rateControlledWells) {
                injectorControlMode = 'rate';
                producerControlMode = 'rate';
            }
        },
        injectorControlMode: (value) => injectorControlMode = String(value) === 'rate' ? 'rate' : 'pressure',
        producerControlMode: (value) => producerControlMode = String(value) === 'rate' ? 'rate' : 'pressure',
        injectorEnabled: (value) => injectorEnabled = Boolean(value),
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
    $: delta_t_days = Math.max(0.001, Number(delta_t_days) || 0.001);
    $: mu_w = Math.max(0.01, Number(mu_w) || 0.01);
    $: mu_o = Math.max(0.01, Number(mu_o) || 0.01);
    $: c_o = Math.max(0, Number(c_o) || 0);
    $: c_w = Math.max(0, Number(c_w) || 0);
    $: rock_compressibility = Math.max(0, Number(rock_compressibility) || 0);
    $: volume_expansion_o = Math.max(0.01, Number(volume_expansion_o) || 0.01);
    $: volume_expansion_w = Math.max(0.01, Number(volume_expansion_w) || 0.01);
    $: max_sat_change_per_step = Math.max(0.01, Math.min(1, Number(max_sat_change_per_step) || 0.01));
    $: max_pressure_change_per_step = Math.max(1, Number(max_pressure_change_per_step) || 1);
    $: max_well_rate_change_fraction = Math.max(0.01, Number(max_well_rate_change_fraction) || 0.01);
    $: targetInjectorRate = Math.max(0, Number(targetInjectorRate) || 0);
    $: targetProducerRate = Math.max(0, Number(targetProducerRate) || 0);
    $: rateControlledWells = injectorControlMode === 'rate' && producerControlMode === 'rate';

    function resetModelAndVisualizationState(stopWorker = true, showReinitNotice = false) {
        stopPlaying();

        if (stopWorker && simWorker && workerRunning) {
            simWorker.postMessage({ type: 'stop' });
        }

        history = [];
        currentIndex = -1;
        gridStateRaw = null;
        wellStateRaw = null;
        rateHistory = [];
        runCompleted = false;
        simTime = 0;
        runtimeWarning = '';
        runtimeError = '';
        if (showReinitNotice) {
            modelNeedsReinit = true;
            modelReinitNotice = 'Model reinit required due to input changes';
        }
        vizRevision += 1;
    }

    function buildModelResetKey() {
        return JSON.stringify({
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
            c_o: Number(c_o),
            c_w: Number(c_w),
            rock_compressibility: Number(rock_compressibility),
            depth_reference: Number(depth_reference),
            volume_expansion_o: Number(volume_expansion_o),
            volume_expansion_w: Number(volume_expansion_w),
            rho_w: Number(rho_w),
            rho_o: Number(rho_o),
            s_wc: Number(s_wc),
            s_or: Number(s_or),
            n_w: Number(n_w),
            n_o: Number(n_o),
            max_sat_change_per_step: Number(max_sat_change_per_step),
            max_pressure_change_per_step: Number(max_pressure_change_per_step),
            max_well_rate_change_fraction: Number(max_well_rate_change_fraction),
            gravityEnabled: Boolean(gravityEnabled),
            capillaryEnabled: Boolean(capillaryEnabled),
            capillaryPEntry: Number(capillaryPEntry),
            capillaryLambda: Number(capillaryLambda),
            permMode,
            uniformPermX: Number(uniformPermX),
            uniformPermY: Number(uniformPermY),
            uniformPermZ: Number(uniformPermZ),
            minPerm: Number(minPerm),
            maxPerm: Number(maxPerm),
            useRandomSeed: Boolean(useRandomSeed),
            randomSeed: Number(randomSeed),
            layerPermsX: layerPermsX.map(Number),
            layerPermsY: layerPermsY.map(Number),
            layerPermsZ: layerPermsZ.map(Number),
            well_radius: Number(well_radius),
            well_skin: Number(well_skin),
            injectorBhp: Number(injectorBhp),
            producerBhp: Number(producerBhp),
            injectorControlMode,
            producerControlMode,
            injectorEnabled: Boolean(injectorEnabled),
            targetInjectorRate: Number(targetInjectorRate),
            targetProducerRate: Number(targetProducerRate),
            injectorI: Number(injectorI),
            injectorJ: Number(injectorJ),
            producerI: Number(producerI),
            producerJ: Number(producerJ),
        });
    }

    $: {
        const nextKey = buildModelResetKey();
        if (!modelResetKey) {
            modelResetKey = nextKey;
        } else if (nextKey !== modelResetKey) {
            modelResetKey = nextKey;
            if (skipNextAutoModelReset) {
                skipNextAutoModelReset = false;
            } else {
                resetModelAndVisualizationState(true, true);
            }
        }
    }

    function applyScenarioPreset() {
        const preset = scenarioPresets[scenarioPreset] as Record<string, unknown> | null;
        if (!preset) return;

        skipNextAutoModelReset = true;

        // Set grid dimensions first to ensure well positions are clamped correctly
        const priorityKeys = ['nx', 'ny', 'nz', 'cellDx', 'cellDy', 'cellDz'];
        for (const key of priorityKeys) {
            if (preset[key] !== undefined) {
                scenarioPresetSetters[key]?.(preset[key]);
            }
        }

        // Then set the rest
        for (const [key, value] of Object.entries(preset)) {
            if (value === undefined || priorityKeys.includes(key)) continue;
            scenarioPresetSetters[key]?.(value);
        }

        resetModelAndVisualizationState(true, true);
    }

    type ValidationState = {
        errors: Record<string, string>;
        warnings: string[];
    };

    function validateInputs(): ValidationState {
        const errors: Record<string, string> = {};
        const warnings: string[] = [];

        if (initialSaturation < 0 || initialSaturation > 1) {
            errors.initialSaturation = 'Initial water saturation must be in [0, 1].';
        }
        if (delta_t_days <= 0) {
            errors.deltaT = 'Timestep must be positive.';
        }
        if (well_radius <= 0) {
            errors.wellRadius = 'Well radius must be positive.';
        }
        if (s_wc + s_or >= 1) {
            errors.saturationEndpoints = 'S_wc + S_or must be < 1.';
        }
        if (minPerm > maxPerm) {
            errors.permBounds = 'Min permeability must not exceed max permeability.';
        }
        if (injectorEnabled && injectorI === producerI && injectorJ === producerJ) {
            errors.wellOverlap = 'Injector and producer cannot share the same i/j location.';
        }
        if (injectorControlMode === 'pressure' && producerControlMode === 'pressure' && injectorBhp <= producerBhp) {
            errors.wellPressureOrder = 'Injector BHP should be greater than producer BHP for pressure-driven displacement.';
        }
        if (injectorControlMode === 'rate' && targetInjectorRate <= 0 && injectorEnabled) {
            errors.injectorRate = 'Injector rate must be positive when injector is enabled and rate-controlled.';
        }
        if (producerControlMode === 'rate' && targetProducerRate <= 0) {
            errors.producerRate = 'Producer rate must be positive when rate-controlled.';
        }
        if (delta_t_days * steps > 3650) {
            warnings.push('Requested run covers more than 10 years of simulated time; results may require tighter timestep limits.');
        }
        if (max_pressure_change_per_step > 250) {
            warnings.push('Large max ΔP per step may reduce numerical robustness.');
        }

        return { errors, warnings };
    }

    $: validationState = validateInputs();
    $: validationErrors = validationState.errors;
    $: validationWarnings = validationState.warnings;
    $: hasValidationErrors = Object.keys(validationErrors).length > 0;
    $: estimatedRunSeconds = Math.max(0, (Number(profileStats.avgStepMs || 0) * Number(steps || 0)) / 1000);
    $: longRunEstimate = estimatedRunSeconds > 10;

    function resetSimulationState(options: { clearErrors?: boolean; clearWarnings?: boolean; resetProfile?: boolean; bumpViz?: boolean } = {}) {
        const {
            clearErrors = false,
            clearWarnings = false,
            resetProfile = false,
            bumpViz = false,
        } = options;

        stopPlaying();
        history = [];
        currentIndex = -1;
        gridStateRaw = null;
        wellStateRaw = null;
        simTime = 0;
        rateHistory = [];
        runCompleted = false;

        if (resetProfile) {
            profileStats = { ...EMPTY_PROFILE_STATS };
        }
        if (clearErrors) {
            runtimeError = '';
        }
        if (clearWarnings) {
            runtimeWarning = '';
        }
        if (bumpViz) {
            vizRevision += 1;
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

        // Capture solver warning from worker state
        solverWarning = message.solverWarning || '';

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
            initSimulator({ silent: true });
            return;
        }

        if (type === 'runStarted') {
            runtimeError = '';
            workerRunning = true;
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

            if (pendingAutoReinit) {
                pendingAutoReinit = false;
                const reinitialized = initSimulator({ silent: true });
                runtimeWarning = reinitialized
                    ? 'Configuration changed. Reservoir reset and reinitialized at step 0.'
                    : runtimeWarning;
            }
            return;
        }

        if (type === 'stopped') {
            workerRunning = false;
            runCompleted = true;

            if (pendingAutoReinit) {
                pendingAutoReinit = false;
                const reinitialized = initSimulator({ silent: true });
                runtimeWarning = reinitialized
                    ? 'Configuration changed during run. Reservoir reinitialized at step 0.'
                    : runtimeWarning;
                return;
            }

            runtimeWarning = message.reason === 'user'
                ? `Simulation stopped by user after ${Number(message.completedSteps ?? 0)} step(s).`
                : 'No running simulation to stop.';
            updateProfileStats(message.profile, profileStats.renderApplyMs);
            applyHistoryIndex(history.length - 1);
            return;
        }

        if (type === 'warning') {
            runtimeWarning = String(message.message ?? 'Simulation warning');
            return;
        }

        if (type === 'error') {
            workerRunning = false;
            console.error('Simulation worker error:', message.message);
            runtimeError = String(message.message ?? 'Simulation error');

            if (pendingAutoReinit) {
                pendingAutoReinit = false;
            }
        }
    }

    function setupWorker() {
        simWorker = new Worker(new URL('./lib/sim.worker.js', import.meta.url), { type: 'module' });
        simWorker.onmessage = handleWorkerMessage;
        simWorker.onerror = (event) => {
            workerRunning = false;
            runtimeError = `Worker error: ${event.message || 'Unknown worker failure'}`;
        };
        simWorker.onmessageerror = () => {
            workerRunning = false;
            runtimeError = 'Worker message deserialization failed. Reinitialize and retry.';
        };
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

    function initSimulator(options: { runAfterInit?: boolean; silent?: boolean } = {}): boolean {
        const { runAfterInit = false, silent = false } = options;

        if (!wasmReady || !simWorker) {
            if (!silent) runtimeError = 'WASM not ready yet.';
            return false;
        }

        const validWellLocations =
            Number.isInteger(injectorI) && Number.isInteger(injectorJ) &&
            Number.isInteger(producerI) && Number.isInteger(producerJ) &&
            injectorI >= 0 && injectorI < nx && injectorJ >= 0 && injectorJ < ny &&
            producerI >= 0 && producerI < nx && producerJ >= 0 && producerJ < ny;

        if (!validWellLocations) {
            if (!silent) runtimeError = 'Invalid well location. Ensure i/j are within grid bounds.';
            return false;
        }

        if (hasValidationErrors) {
            if (!silent) runtimeError = 'Input validation failed. Review highlighted controls.';
            return false;
        }

        history = [];
        currentIndex = -1;
        runCompleted = false;
        modelNeedsReinit = false;
        modelReinitNotice = '';
        runtimeError = '';
        runtimeWarning = '';
        vizRevision += 1;

        const payload = buildCreatePayload();
        simWorker.postMessage({
            type: 'create',
            payload,
        });
        lastCreateSignature = JSON.stringify(payload);
        pendingAutoReinit = false;

        if (runAfterInit) {
            runSteps();
        }

        return true;
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
            c_o: Number(c_o),
            c_w: Number(c_w),
            rock_compressibility: Number(rock_compressibility),
            depth_reference: Number(depth_reference),
            volume_expansion_o: Number(volume_expansion_o),
            volume_expansion_w: Number(volume_expansion_w),
            rho_w: Number(rho_w),
            rho_o: Number(rho_o),
            s_wc: Number(s_wc),
            s_or: Number(s_or),
            n_w: Number(n_w),
            n_o: Number(n_o),
            max_sat_change_per_step: Number(max_sat_change_per_step),
            max_pressure_change_per_step: Number(max_pressure_change_per_step),
            max_well_rate_change_fraction: Number(max_well_rate_change_fraction),
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
            injectorControlMode,
            producerControlMode,
            injectorEnabled: Boolean(injectorEnabled),
            targetInjectorRate: Number(targetInjectorRate),
            targetProducerRate: Number(targetProducerRate),
            injectorI: Number(injectorI),
            injectorJ: Number(injectorJ),
            producerI: Number(producerI),
            producerJ: Number(producerJ),
        };
    }

    $: if (wasmReady && simWorker) {
        const nextSignature = JSON.stringify(buildCreatePayload());

        if (lastCreateSignature && nextSignature !== lastCreateSignature) {
            resetSimulationState({
                clearErrors: true,
                clearWarnings: false,
                resetProfile: true,
                bumpViz: true,
            });

            if (workerRunning) {
                pendingAutoReinit = true;
                runtimeWarning = 'Configuration changed during run. Stopping and reinitializing at step 0…';
                stopRun();
            } else {
                const reinitialized = initSimulator({ silent: true });
                runtimeWarning = reinitialized
                    ? 'Configuration changed. Reservoir reset and reinitialized at step 0.'
                    : runtimeWarning;
            }
        }
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
        if (modelNeedsReinit) {
            initSimulator();
            return;
        }
        if (!simWorker || workerRunning || hasValidationErrors) return;
        workerRunning = true;
        runtimeError = '';
        runtimeWarning = longRunEstimate
            ? `Estimated run duration is ${estimatedRunSeconds.toFixed(1)}s. You can stop at any time.`
            : runtimeWarning;
        simWorker.postMessage({
            type: 'run',
            payload: {
                steps: batchSteps,
                deltaTDays: Number(delta_t_days),
                historyInterval,
                chunkYieldInterval: 5,
            }
        });
    }

    function stopRun() {
        if (!simWorker) return;
        simWorker.postMessage({ type: 'stop' });
    }



    /* Playback controls */
    function play() {
        if (history.length === 0) return;
        if (playTimer) {
            clearInterval(playTimer);
            playTimer = null;
        }
        playing = true;
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
        currentIndex = idx;
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
            injectionRateSeries={rateHistory.map((point) => point.total_injection)}
            reservoir={{ length: nx * cellDx, area: ny * cellDy * nz * cellDz, porosity: reservoirPorosity }}
            scenarioMode={injectorEnabled ? 'waterflood' : 'depletion'}
            producerLocation={{ i: producerI, j: producerJ, nx, ny }}
            on:analyticalData={(e) => analyticalProductionData = e.detail.production}
            on:analyticalMeta={(e) => analyticalMeta = e.detail}
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
                    {modelReinitNotice}
                    {simTime}
                    {estimatedRunSeconds}
                    {longRunEstimate}
                    canStop={workerRunning}
                    hasValidationErrors={hasValidationErrors}
                    historyLength={history.length}
                    {profileStats}
                    {solverWarning}
                    onRunSteps={runSteps}
                    onStepOnce={stepOnce}
                    onInitSimulator={initSimulator}
                    onStopRun={stopRun}
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
                    bind:mu_w
                    bind:mu_o
                    bind:c_o
                    bind:c_w
                    bind:rho_w
                    bind:rho_o
                    bind:rock_compressibility
                    bind:depth_reference
                    bind:volume_expansion_o
                    bind:volume_expansion_w
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
                    fieldErrors={validationErrors}
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
                    bind:injectorEnabled
                    bind:injectorControlMode
                    bind:producerControlMode
                    bind:injectorBhp
                    bind:producerBhp
                    bind:targetInjectorRate
                    bind:targetProducerRate
                    bind:injectorI
                    bind:injectorJ
                    bind:producerI
                    bind:producerJ
                    fieldErrors={validationErrors}
                />

                <TimestepControlsPanel
                    bind:delta_t_days
                    bind:max_sat_change_per_step
                    bind:max_pressure_change_per_step
                    bind:max_well_rate_change_fraction
                    fieldErrors={validationErrors}
                />

                {#if validationWarnings.length > 0}
                    <div class="card border border-warning bg-base-100 shadow-sm">
                        <div class="card-body p-3 text-xs">
                            {#each validationWarnings as warning}
                                <div class="text-warning">⚠ {warning}</div>
                            {/each}
                        </div>
                    </div>
                {/if}

                {#if runtimeWarning}
                    <div class="rounded-md border border-warning bg-base-100 p-2 text-xs text-warning">{runtimeWarning}</div>
                {/if}
                {#if runtimeError}
                    <div class="rounded-md border border-error bg-base-100 p-2 text-xs text-error">{runtimeError}</div>
                {/if}

                

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
                                {poreVolumeM3}
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
                            {#key `${nx}-${ny}-${nz}-${vizRevision}`}
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
                            {/key}
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

                <SwProfileChart
                    gridState={gridStateRaw ?? []}
                    {nx}
                    {ny}
                    {nz}
                    {cellDx}
                    {cellDy}
                    {cellDz}
                    simTime={simTime}
                    producerJ={producerJ}
                    {initialSaturation}
                    reservoirPorosity={reservoirPorosity}
                    injectionRate={latestInjectionRate}
                    scenarioMode={injectorEnabled ? 'waterflood' : 'depletion'}
                    rockProps={{ s_wc, s_or, n_w, n_o }}
                    fluidProps={{ mu_w, mu_o }}
                />

                {#if analyticalMeta.mode === 'depletion'}
                    <div class="rounded-md border border-base-300 bg-base-100 p-3 text-xs">
                        <div class="font-semibold">Depletion Analytical Mode</div>
                        <div class="opacity-80">Dietz shape factor: {analyticalMeta.shapeFactor ?? 'n/a'} ({analyticalMeta.shapeLabel || 'unspecified location'})</div>
                    </div>
                {/if}

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