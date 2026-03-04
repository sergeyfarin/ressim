import type {
    WorkerMessage,
    SimulatorSnapshot,
    RateHistoryPoint,
    SimulatorCreatePayload,
    AnalyticalProductionPoint,
    GridState,
    WellState,
} from '../simulator-types';
import {
    caseCatalog,
    findCaseByKey,
    findMatchingCases,
    resolveParams,
    FOCUS_OPTIONS,
    type CaseMode,
    type CaseEntry,
} from '../caseCatalog';
import { buildCreatePayloadFromState } from '../buildCreatePayload';

// ---------- Helper utilities (pure, no runes) ----------

function parseLayerValues(value: unknown): number[] {
    if (Array.isArray(value)) {
        return value
            .map((v) => Number(v))
            .filter((v) => Number.isFinite(v) && v > 0);
    }
    return String(value)
        .split(',')
        .map((v) => Number(v.trim()))
        .filter((v) => Number.isFinite(v) && v > 0);
}

function normalizeRateHistory(raw: unknown): RateHistoryPoint[] {
    if (!Array.isArray(raw)) return [];
    return raw.map((entry: Record<string, unknown>, idx: number) => {
        const point = entry && typeof entry === 'object' ? entry : {};
        const fallbackTime = idx > 0 ? idx : 0;
        const numeric = (value: unknown, fallback = 0) => {
            const next = Number(value);
            return Number.isFinite(next) ? next : fallback;
        };
        return {
            ...point,
            time: numeric(point.time, fallbackTime),
            total_production_oil: numeric(point.total_production_oil),
            total_production_liquid: numeric(point.total_production_liquid),
            total_production_liquid_reservoir: numeric(point.total_production_liquid_reservoir),
            total_injection: numeric(point.total_injection),
            total_injection_reservoir: numeric(point.total_injection_reservoir),
            material_balance_error_m3: numeric(point.material_balance_error_m3),
            avg_reservoir_pressure: numeric(point.avg_reservoir_pressure),
            avg_pressure: numeric(point.avg_pressure),
            avg_water_saturation: numeric(point.avg_water_saturation),
        };
    });
}

function normalizeLayerArray(values: number[], fallback: number, length: number): number[] {
    return Array.from({ length }, (_, i) => {
        const value = Number(values[i]);
        return Number.isFinite(value) && value > 0 ? value : fallback;
    });
}

function isPermMode(value: string): value is 'uniform' | 'random' | 'perLayer' {
    return value === 'uniform' || value === 'random' || value === 'perLayer';
}

// ---------- Types ----------

type HistoryEntry = SimulatorSnapshot;

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

type ValidationState = {
    errors: Record<string, string>;
    warnings: string[];
};

const EMPTY_PROFILE_STATS: ProfileStats = {
    batchMs: 0,
    avgStepMs: 0,
    extractMs: 0,
    renderApplyMs: 0,
    snapshotsSent: 0,
};

const MAX_HISTORY_ENTRIES = 300;

const CUSTOM_SUBCASE_BY_MODE: Record<string, { key: string; label: string }> = {
    depletion: { key: 'depletion_custom_subcase', label: 'Custom Depletion Sub-case' },
    waterflood: { key: 'waterflood_custom_subcase', label: 'Custom Waterflood Sub-case' },
    simulation: { key: 'simulation_custom_subcase', label: 'Custom Simulation Sub-case' },
};

// ---------- Store ----------

export function createSimulationStore() {
    // ===== Simulation Input State =====
    let nx = $state(15);
    let ny = $state(10);
    let nz = $state(10);
    let cellDx = $state(10);
    let cellDy = $state(10);
    let cellDz = $state(1);
    let delta_t_days = $state(0.25);
    let steps = $state(20);

    // Initial Conditions
    let initialPressure = $state(300.0);
    let initialSaturation = $state(0.3);
    let reservoirPorosity = $state(0.2);

    // Fluid properties
    let mu_w = $state(0.5);
    let mu_o = $state(1.0);
    let c_o = $state(1e-5);
    let c_w = $state(3e-6);
    let rock_compressibility = $state(1e-6);
    let depth_reference = $state(0.0);
    let volume_expansion_o = $state(1.0);
    let volume_expansion_w = $state(1.0);
    let rho_w = $state(1000.0);
    let rho_o = $state(800.0);

    // Permeability
    let permMode: 'uniform' | 'random' | 'perLayer' = $state('uniform');
    let uniformPermX = $state(100.0);
    let uniformPermY = $state(100.0);
    let uniformPermZ = $state(10.0);
    let minPerm = $state(50.0);
    let maxPerm = $state(200.0);
    let useRandomSeed = $state(true);
    let randomSeed = $state(12345);
    let layerPermsX: number[] = $state([100, 150, 50, 200, 120, 1000, 90, 110, 130, 70]);
    let layerPermsY: number[] = $state([100, 150, 50, 200, 120, 1000, 90, 110, 130, 70]);
    let layerPermsZ: number[] = $state([10, 15, 5, 20, 12, 8, 9, 11, 13, 7]);

    // Relative Permeability / Capillary
    let s_wc = $state(0.1);
    let s_or = $state(0.1);
    let n_w = $state(2.0);
    let n_o = $state(2.0);
    let k_rw_max = $state(1.0);
    let k_ro_max = $state(1.0);

    // Well inputs
    let well_radius = $state(0.1);
    let well_skin = $state(0.0);
    let injectorBhp = $state(500.0);
    let producerBhp = $state(100.0);
    let injectorControlMode: 'rate' | 'pressure' = $state('pressure');
    let producerControlMode: 'rate' | 'pressure' = $state('pressure');
    let injectorEnabled = $state(true);
    let targetInjectorRate = $state(350.0);
    let targetProducerRate = $state(350.0);
    let injectorI = $state(0);
    let injectorJ = $state(0);
    let producerI = $state(14);
    let producerJ = $state(0);

    // Stability
    let max_sat_change_per_step = $state(0.1);
    let max_pressure_change_per_step = $state(75.0);
    let max_well_rate_change_fraction = $state(0.75);
    let gravityEnabled = $state(false);
    let capillaryEnabled = $state(true);
    let capillaryPEntry = $state(5.0);
    let capillaryLambda = $state(2.0);

    // ===== Simulation Output / Runtime State =====
    let wasmReady = $state(false);
    let simWorker: Worker | null = $state(null);
    let runCompleted = $state(false);
    let workerRunning = $state(false);
    let currentRunTotalSteps = $state(0);
    let currentRunStepsCompleted = $state(0);

    // Navigation State
    let activeMode = $state<CaseMode>('depletion');
    let activeCase = $state('');
    let isCustomMode = $state(false);
    let preRunData: any = $state(null);
    let preRunLoading = $state(false);
    let preRunWarning = $state('');
    let preRunLoadToken = $state(0);
    let preRunContinuationAvailable = $state(false);

    let toggles = $state({
        geometry: '1D' as string,
        wellPosition: 'end-to-end' as string,
        permeability: 'uniform' as string,
        gravity: false,
        capillary: false,
        fluids: 'standard' as string,
        focus: 'shape-factor' as string,
    });

    // Display data
    let gridStateRaw: GridState | null = $state(null);
    let wellStateRaw: WellState | null = $state(null);
    let simTime = $state(0);
    let rateHistory = $state<RateHistoryPoint[]>([]);
    let analyticalProductionData: AnalyticalProductionPoint[] = $state([]);
    let analyticalSolutionMode: 'waterflood' | 'depletion' = $state('depletion');
    let analyticalDepletionRateScale = $state(1.0);
    let analyticalMeta: {
        mode: 'waterflood' | 'depletion';
        shapeFactor: number | null;
        shapeLabel: string;
    } = $state({ mode: 'waterflood', shapeFactor: null, shapeLabel: '' });
    let runtimeWarning = $state('');
    let solverWarning = $state('');
    let runtimeError = $state('');
    let vizRevision = $state(0);
    let modelReinitNotice = $state('');
    let modelNeedsReinit = $state(false);
    let configDiffSignature = $state('');
    let skipNextAutoModelReset = $state(false);

    // History / replay
    let history = $state<HistoryEntry[]>([]);
    let currentIndex = $state(-1);
    let playing = $state(false);
    let playSpeed = $state(2);
    let playTimer: ReturnType<typeof setInterval> | null = $state(null);
    let userHistoryInterval = $state<number | null>(null);
    const defaultHistoryInterval = $derived(Math.max(1, Math.ceil(steps / 50)));

    // Profile
    let profileStats: ProfileStats = $state({ ...EMPTY_PROFILE_STATS });
    let lastCreateSignature = $state('');
    let baseCaseSignature = $state('');
    let pendingAutoReinit = $state(false);

    // ===== Derived Values =====
    const ooipM3 = $derived(
        nx * ny * nz * cellDx * cellDy * cellDz * reservoirPorosity * Math.max(0, 1 - initialSaturation),
    );
    const poreVolumeM3 = $derived(nx * ny * nz * cellDx * cellDy * cellDz * reservoirPorosity);
    const rateControlledWells = $derived.by(
        () => injectorControlMode === 'rate' && producerControlMode === 'rate',
    );
    const latestInjectionRate = $derived.by(() => {
        if (!Array.isArray(rateHistory) || rateHistory.length === 0) return 0;
        for (let i = rateHistory.length - 1; i >= 0; i--) {
            const q = Number(rateHistory[i]?.total_injection);
            if (Number.isFinite(q) && q > 0) return q;
        }
        return 0;
    });
    const avgReservoirPressureSeries = $derived(
        rateHistory.map((point) => point.avg_reservoir_pressure ?? null),
    );
    const avgWaterSaturationSeries = $derived(
        rateHistory.map((point) => point.avg_water_saturation ?? null),
    );
    const replayTime = $derived(
        history.length > 0 && currentIndex >= 0 && currentIndex < history.length
            ? history[currentIndex].time
            : null,
    );

    const matchingCases = $derived(
        caseCatalog.filter(c => {
            if (c.facets.mode !== activeMode) return false;
            if (c.facets.geometry !== toggles.geometry) return false;
            if (c.facets.wellPosition !== toggles.wellPosition) return false;
            if (c.facets.permeability !== toggles.permeability) return false;
            if (c.facets.gravity !== toggles.gravity) return false;
            if (c.facets.capillary !== toggles.capillary) return false;
            if (c.facets.fluids !== toggles.fluids) return false;
            if (c.facets.focus !== toggles.focus) return false;
            return true;
        })
    );

    // ===== Validation =====
    function validateInputs(): ValidationState {
        const errors: Record<string, string> = {};
        const warnings: string[] = [];
        const numeric = (value: unknown) => Number(value);
        const isFiniteNumber = (value: unknown) => Number.isFinite(numeric(value));

        if (!Number.isInteger(numeric(nx)) || numeric(nx) < 1) errors.nx = 'Nx must be an integer ≥ 1.';
        if (!Number.isInteger(numeric(ny)) || numeric(ny) < 1) errors.ny = 'Ny must be an integer ≥ 1.';
        if (!Number.isInteger(numeric(nz)) || numeric(nz) < 1) errors.nz = 'Nz must be an integer ≥ 1.';
        if (!isFiniteNumber(cellDx) || numeric(cellDx) <= 0) errors.cellDx = 'Cell Δx must be positive.';
        if (!isFiniteNumber(cellDy) || numeric(cellDy) <= 0) errors.cellDy = 'Cell Δy must be positive.';
        if (!isFiniteNumber(cellDz) || numeric(cellDz) <= 0) errors.cellDz = 'Cell Δz must be positive.';
        if (!Number.isInteger(numeric(steps)) || numeric(steps) < 1) errors.steps = 'Steps must be an integer ≥ 1.';
        if (initialSaturation < 0 || initialSaturation > 1) errors.initialSaturation = 'Initial water saturation must be in [0, 1].';
        if (!isFiniteNumber(delta_t_days) || numeric(delta_t_days) <= 0) errors.deltaT = 'Timestep must be positive.';
        if (!isFiniteNumber(well_radius) || numeric(well_radius) <= 0) errors.wellRadius = 'Well radius must be positive.';
        if (!isFiniteNumber(mu_w) || numeric(mu_w) <= 0) errors.mu_w = 'Water viscosity must be positive.';
        if (!isFiniteNumber(mu_o) || numeric(mu_o) <= 0) errors.mu_o = 'Oil viscosity must be positive.';
        if (!isFiniteNumber(c_o) || numeric(c_o) < 0) errors.c_o = 'Oil compressibility must be ≥ 0.';
        if (!isFiniteNumber(c_w) || numeric(c_w) < 0) errors.c_w = 'Water compressibility must be ≥ 0.';
        if (!isFiniteNumber(rock_compressibility) || numeric(rock_compressibility) < 0) errors.rock_compressibility = 'Rock compressibility must be ≥ 0.';
        if (!isFiniteNumber(volume_expansion_o) || numeric(volume_expansion_o) <= 0) errors.volume_expansion_o = 'Oil formation volume factor must be positive.';
        if (!isFiniteNumber(volume_expansion_w) || numeric(volume_expansion_w) <= 0) errors.volume_expansion_w = 'Water formation volume factor must be positive.';
        if (!isFiniteNumber(max_sat_change_per_step) || numeric(max_sat_change_per_step) <= 0 || numeric(max_sat_change_per_step) > 1) errors.max_sat_change_per_step = 'Max ΔSw per step must be in (0, 1].';
        if (!isFiniteNumber(max_pressure_change_per_step) || numeric(max_pressure_change_per_step) <= 0) errors.max_pressure_change_per_step = 'Max ΔP per step must be positive.';
        if (!isFiniteNumber(max_well_rate_change_fraction) || numeric(max_well_rate_change_fraction) <= 0) errors.max_well_rate_change_fraction = 'Max well-rate change fraction must be positive.';

        if (
            !Number.isInteger(numeric(injectorI)) ||
            !Number.isInteger(numeric(injectorJ)) ||
            !Number.isInteger(numeric(producerI)) ||
            !Number.isInteger(numeric(producerJ))
        ) {
            errors.wellIndexType = 'Well indices must be integers.';
        } else if (
            numeric(injectorI) < 0 || numeric(injectorI) >= numeric(nx) ||
            numeric(injectorJ) < 0 || numeric(injectorJ) >= numeric(ny) ||
            numeric(producerI) < 0 || numeric(producerI) >= numeric(nx) ||
            numeric(producerJ) < 0 || numeric(producerJ) >= numeric(ny)
        ) {
            errors.wellIndexRange = 'Well indices must lie within the grid bounds.';
        }

        if (s_wc + s_or >= 1) errors.saturationEndpoints = 'S_wc + S_or must be < 1.';
        if (minPerm > maxPerm) errors.permBounds = 'Min perm must not exceed max perm.';
        if (injectorEnabled && injectorI === producerI && injectorJ === producerJ) {
            errors.wellOverlap = 'Injector and producer cannot share the same i/j location.';
        }
        if (injectorControlMode === 'pressure' && producerControlMode === 'pressure' && injectorBhp <= producerBhp) {
            errors.wellPressureOrder = 'Injector BHP should be greater than producer BHP.';
        }
        if (injectorControlMode === 'rate' && targetInjectorRate <= 0 && injectorEnabled) {
            errors.injectorRate = 'Injector rate must be positive when enabled and rate-controlled.';
        }
        if (producerControlMode === 'rate' && targetProducerRate <= 0) {
            errors.producerRate = 'Producer rate must be positive when rate-controlled.';
        }
        if (delta_t_days * steps > 3650) {
            warnings.push('Requested run covers more than 10 years; results may require tighter timestep limits.');
        }
        if (max_pressure_change_per_step > 250) {
            warnings.push('Large max ΔP per step may reduce numerical robustness.');
        }
        return { errors, warnings };
    }

    const validationState: ValidationState = $derived(validateInputs());
    const validationErrors: Record<string, string> = $derived(validationState.errors);
    const validationWarnings: string[] = $derived(validationState.warnings);
    const hasValidationErrors = $derived(Object.keys(validationErrors).length > 0);
    const estimatedRunSeconds = $derived(
        Math.max(0, (Number(profileStats.avgStepMs || 0) * Number(steps || 0)) / 1000),
    );
    const longRunEstimate = $derived(estimatedRunSeconds > 10);

    // ===== Internal Helpers =====

    function syncLayerArraysToGrid() {
        layerPermsX = normalizeLayerArray(layerPermsX, uniformPermX, nz);
        layerPermsY = normalizeLayerArray(layerPermsY, uniformPermY, nz);
        layerPermsZ = normalizeLayerArray(layerPermsZ, uniformPermZ, nz);
    }

    function buildCreatePayload(): SimulatorCreatePayload {
        return buildCreatePayloadFromState({
            nx, ny, nz, cellDx, cellDy, cellDz,
            initialPressure, initialSaturation, porosity: reservoirPorosity,
            mu_w, mu_o, c_o, c_w, rho_w, rho_o,
            rock_compressibility, depth_reference, volume_expansion_o, volume_expansion_w,
            s_wc, s_or, n_w, n_o, k_rw_max, k_ro_max,
            max_sat_change_per_step, max_pressure_change_per_step, max_well_rate_change_fraction,
            capillaryEnabled, capillaryPEntry, capillaryLambda, gravityEnabled,
            permMode, minPerm, maxPerm, useRandomSeed, randomSeed,
            permsX: layerPermsX, permsY: layerPermsY, permsZ: layerPermsZ,
            well_radius, well_skin, injectorBhp, producerBhp,
            rateControlledWells, injectorControlMode, producerControlMode,
            injectorEnabled, targetInjectorRate, targetProducerRate,
            injectorI, injectorJ, producerI, producerJ,
            uniformPermX, uniformPermY, uniformPermZ,
        });
    }

    function buildModelResetKey() {
        return JSON.stringify({
            nx: Number(nx), ny: Number(ny), nz: Number(nz),
            cellDx: Number(cellDx), cellDy: Number(cellDy), cellDz: Number(cellDz),
            initialPressure: Number(initialPressure), initialSaturation: Number(initialSaturation),
            mu_w: Number(mu_w), mu_o: Number(mu_o), c_o: Number(c_o), c_w: Number(c_w),
            rock_compressibility: Number(rock_compressibility), depth_reference: Number(depth_reference),
            volume_expansion_o: Number(volume_expansion_o), volume_expansion_w: Number(volume_expansion_w),
            rho_w: Number(rho_w), rho_o: Number(rho_o),
            s_wc: Number(s_wc), s_or: Number(s_or), n_w: Number(n_w), n_o: Number(n_o),
            k_rw_max: Number(k_rw_max), k_ro_max: Number(k_ro_max),
            max_sat_change_per_step: Number(max_sat_change_per_step),
            max_pressure_change_per_step: Number(max_pressure_change_per_step),
            max_well_rate_change_fraction: Number(max_well_rate_change_fraction),
            gravityEnabled: Boolean(gravityEnabled), capillaryEnabled: Boolean(capillaryEnabled),
            capillaryPEntry: Number(capillaryPEntry), capillaryLambda: Number(capillaryLambda),
            permMode, uniformPermX: Number(uniformPermX), uniformPermY: Number(uniformPermY),
            uniformPermZ: Number(uniformPermZ), minPerm: Number(minPerm), maxPerm: Number(maxPerm),
            useRandomSeed: Boolean(useRandomSeed), randomSeed: Number(randomSeed),
            layerPermsX: layerPermsX.map(Number), layerPermsY: layerPermsY.map(Number), layerPermsZ: layerPermsZ.map(Number),
            well_radius: Number(well_radius), well_skin: Number(well_skin),
            injectorBhp: Number(injectorBhp), producerBhp: Number(producerBhp),
            injectorControlMode, producerControlMode,
            injectorEnabled: Boolean(injectorEnabled),
            targetInjectorRate: Number(targetInjectorRate), targetProducerRate: Number(targetProducerRate),
            injectorI: Number(injectorI), injectorJ: Number(injectorJ),
            producerI: Number(producerI), producerJ: Number(producerJ),
        });
    }

    function buildCaseSignature(): string {
        return JSON.stringify({
            model: buildModelResetKey(),
            delta_t_days: Number(delta_t_days),
            steps: Number(steps),
            analyticalSolutionMode,
            analyticalDepletionRateScale: Number(analyticalDepletionRateScale),
        });
    }

    // ===== Playback Controls =====

    function stopPlaying() {
        playing = false;
        if (playTimer) {
            clearInterval(playTimer);
            playTimer = null;
        }
    }

    function applyHistoryIndex(idx: number) {
        if (idx < 0 || idx >= history.length) return;
        currentIndex = idx;
        const entry = history[idx];
        gridStateRaw = entry.grid;
        wellStateRaw = entry.wells;
        simTime = entry.time;
    }

    function play() {
        if (history.length === 0) return;
        if (playTimer) { clearInterval(playTimer); playTimer = null; }
        playing = true;
        playTimer = setInterval(() => {
            next();
            if (currentIndex >= history.length - 1) stopPlaying();
        }, 1000 / playSpeed);
    }

    function togglePlay() {
        if (playing) stopPlaying();
        else play();
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

    // ===== Simulation State Management =====

    function resetSimulationState(options: {
        clearErrors?: boolean; clearWarnings?: boolean; resetProfile?: boolean; bumpViz?: boolean;
    } = {}) {
        const { clearErrors = false, clearWarnings = false, resetProfile = false, bumpViz = false } = options;
        stopPlaying();
        history = [];
        currentIndex = -1;
        gridStateRaw = null;
        wellStateRaw = null;
        simTime = 0;
        rateHistory = [];
        runCompleted = false;
        if (resetProfile) profileStats = { ...EMPTY_PROFILE_STATS };
        if (clearErrors) runtimeError = '';
        if (clearWarnings) runtimeWarning = '';
        if (bumpViz) vizRevision += 1;
    }

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

    function pushHistoryEntry(entry: HistoryEntry) {
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

    function applyWorkerState(message: SimulatorSnapshot) {
        const renderStart = performance.now();
        gridStateRaw = message.grid;
        wellStateRaw = message.wells;
        simTime = message.time;
        rateHistory = message.rateHistory ?? [];
        solverWarning = message.solverWarning || '';
        if (message.recordHistory) {
            pushHistoryEntry({ time: message.time, grid: message.grid, wells: message.wells });
        }
        if (message.stepIndex !== undefined && currentRunTotalSteps > 0) {
            currentRunStepsCompleted = message.stepIndex + 1;
        }
        updateProfileStats(message.profile, performance.now() - renderStart);
    }

    // ===== Worker Communication =====

    function handleWorkerMessage(event: MessageEvent<WorkerMessage>) {
        const message = event.data;
        if (!message) return;

        if (message.type === 'ready') {
            wasmReady = true;
            initSimulator({ silent: true });
            return;
        }
        if (message.type === 'runStarted') {
            runtimeError = '';
            workerRunning = true;
            return;
        }
        if (message.type === 'state') {
            applyWorkerState(message as unknown as SimulatorSnapshot);
            return;
        }
        if (message.type === 'batchComplete') {
            workerRunning = false;
            runCompleted = true;
            currentRunTotalSteps = 0;
            currentRunStepsCompleted = 0;
            updateProfileStats(message.profile, profileStats.renderApplyMs);
            applyHistoryIndex(history.length - 1);
            if (pendingAutoReinit) {
                pendingAutoReinit = false;
                const reinitialized = initSimulator({ silent: true });
                runtimeWarning = reinitialized
                    ? 'Config changed. Reservoir reinitialized at step 0.'
                    : runtimeWarning;
            }
            return;
        }
        if (message.type === 'stopped') {
            workerRunning = false;
            runCompleted = true;
            if (pendingAutoReinit) {
                pendingAutoReinit = false;
                const reinitialized = initSimulator({ silent: true });
                runtimeWarning = reinitialized
                    ? 'Config changed during run. Reinitialized at step 0.'
                    : runtimeWarning;
                return;
            }
            runtimeWarning = message.reason === 'user'
                ? `Simulation stopped after ${Number(message.completedSteps ?? 0)} step(s).`
                : 'No running simulation to stop.';
            currentRunTotalSteps = 0;
            currentRunStepsCompleted = 0;
            if ('profile' in message && message.profile) {
                updateProfileStats(message.profile, profileStats.renderApplyMs);
            }
            applyHistoryIndex(history.length - 1);
            return;
        }
        if (message.type === 'warning') {
            runtimeWarning = String(message.message ?? 'Simulation warning');
            return;
        }
        if (message.type === 'error') {
            workerRunning = false;
            console.error('Simulation worker error:', message.message);
            runtimeError = String(message.message ?? 'Simulation error');
            if (pendingAutoReinit) pendingAutoReinit = false;
        }
    }

    function setupWorker() {
        simWorker = new Worker(
            new URL('../sim.worker.ts', import.meta.url),
            { type: 'module' },
        );
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

    function dispose() {
        stopPlaying();
        if (simWorker) {
            simWorker.postMessage({ type: 'dispose' });
            simWorker.terminate();
            simWorker = null;
        }
    }

    // ===== Simulator Init / Run =====

    function resolveCustomSubCase(mode: string): { key: string; label: string } | null {
        return CUSTOM_SUBCASE_BY_MODE[String(mode ?? '').toLowerCase()] ?? null;
    }

    function maybeSwitchToCustomSubCaseOnReinit(): boolean {
        if (isCustomMode || !activeCase || !baseCaseSignature) return false;
        if (!findCaseByKey(activeCase)) return false;
        const customSubCase = resolveCustomSubCase(activeMode);
        if (!customSubCase) return false;
        const nextSignature = buildCaseSignature();
        if (nextSignature === baseCaseSignature) return false;
        isCustomMode = false;
        preRunLoadToken += 1;
        preRunData = null;
        preRunWarning = '';
        activeCase = customSubCase.key;
        baseCaseSignature = nextSignature;
        return true;
    }

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
            if (!silent) runtimeError = 'Invalid well location.';
            return false;
        }
        if (hasValidationErrors) {
            if (!silent) runtimeError = 'Input validation failed.';
            return false;
        }
        const switchedToCustomSubCase = maybeSwitchToCustomSubCaseOnReinit();
        history = [];
        currentIndex = -1;
        runCompleted = false;
        modelNeedsReinit = false;
        modelReinitNotice = '';
        runtimeError = '';
        runtimeWarning = '';
        if (switchedToCustomSubCase) {
            runtimeWarning = 'Preset modified. Reinitialized as a custom sub-case.';
        }
        vizRevision += 1;
        const payload = buildCreatePayload();
        simWorker.postMessage({ type: 'create', payload });
        lastCreateSignature = JSON.stringify(payload);
        pendingAutoReinit = false;
        if (runAfterInit) runSteps();
        return true;
    }

    function runSimulationBatch(batchSteps: number, batchHistoryInterval: number) {
        if (modelNeedsReinit) { initSimulator(); return; }
        if (!simWorker || workerRunning || hasValidationErrors) return;
        workerRunning = true;
        currentRunTotalSteps = batchSteps;
        currentRunStepsCompleted = 0;
        runtimeError = '';
        runtimeWarning = longRunEstimate
            ? `Estimated run: ${estimatedRunSeconds.toFixed(1)}s. You can stop at any time.`
            : runtimeWarning;
        simWorker.postMessage({
            type: 'run',
            payload: {
                steps: batchSteps,
                deltaTDays: Number(delta_t_days),
                historyInterval: batchHistoryInterval,
                chunkYieldInterval: 1,
                history,
                rateHistory,
            },
        });
    }

    function stepOnce() { runSimulationBatch(1, 1); }
    function runSteps() { runSimulationBatch(Number(steps), Number(userHistoryInterval ?? defaultHistoryInterval)); }
    function stopRun() { if (simWorker) simWorker.postMessage({ type: 'stop' }); }

    // ===== Config Diff Detection =====

    function checkConfigDiff() {
        const nextSignature = JSON.stringify(buildCreatePayload());
        if (!configDiffSignature) { configDiffSignature = nextSignature; return; }
        if (nextSignature === configDiffSignature) return;
        configDiffSignature = nextSignature;
        if (skipNextAutoModelReset) { skipNextAutoModelReset = false; return; }
        if (wasmReady && simWorker && isCustomMode && lastCreateSignature) {
            resetSimulationState({ clearErrors: true, clearWarnings: false, resetProfile: true, bumpViz: true });
            if (workerRunning) {
                pendingAutoReinit = true;
                runtimeWarning = 'Config changed during run. Stopping and reinitializing…';
                stopRun();
            } else {
                const reinitialized = initSimulator({ silent: true });
                runtimeWarning = reinitialized ? 'Config changed. Reservoir reinitialized at step 0.' : runtimeWarning;
            }
            return;
        }
        resetModelAndVisualizationState(true, true);
    }

    // ===== Case Navigation =====

    function handleModeChange(mode: CaseMode) {
        preRunLoadToken += 1;
        isCustomMode = false;
        activeMode = mode;
        activeCase = '';
        baseCaseSignature = '';
        preRunData = null;
        preRunWarning = '';
        preRunLoading = false;
        // Reset toggles to mode defaults
        const focusOpts = FOCUS_OPTIONS[mode];
        toggles.geometry = '1D';
        toggles.wellPosition = 'end-to-end';
        toggles.permeability = 'uniform';
        toggles.gravity = false;
        toggles.capillary = false;
        toggles.fluids = 'standard';
        toggles.focus = focusOpts?.[0]?.value ?? 'shape-factor';

        const cases = caseCatalog.filter(c => c.facets.mode === mode);
        if (cases.length > 0) {
            handleCaseChange(cases[0].key);
        }
    }

    function handleCaseChange(key: string) {
        activeCase = key;
        preRunWarning = '';
        const found = findCaseByKey(key);
        if (found) {
            applyCaseParams(found.params);
            baseCaseSignature = buildCaseSignature();
            loadPreRunCase(key);
        }
    }

    function handleCustomMode() {
        if (isCustomMode) return;
        preRunLoadToken += 1;
        isCustomMode = true;
        activeCase = '';
        baseCaseSignature = '';
        preRunData = null;
        preRunWarning = '';
    }

    function handleAnalyticalSolutionModeChange(mode: 'waterflood' | 'depletion') {
        analyticalSolutionMode = mode;
        analyticalProductionData = [];
        analyticalMeta = {
            mode,
            shapeFactor: null,
            shapeLabel: mode === 'depletion' ? 'Peaceman PSS' : '',
        };
    }

    function handleNzOrPermModeChange() {
        syncLayerArraysToGrid();
    }

    function applyCaseParams(params: Record<string, any>) {
        const resolved = resolveParams(params);
        skipNextAutoModelReset = true;
        userHistoryInterval = null;
        nx = Math.max(1, Math.round(Number(resolved.nx) || 1));
        ny = Math.max(1, Math.round(Number(resolved.ny) || 1));
        nz = Math.max(1, Math.round(Number(resolved.nz) || 1));
        cellDx = Number(resolved.cellDx) || 10;
        cellDy = Number(resolved.cellDy) || 10;
        cellDz = Number(resolved.cellDz) || 1;
        delta_t_days = Number(resolved.delta_t_days) || 0.25;
        steps = Math.max(1, Math.round(Number(resolved.steps) || 20));
        max_sat_change_per_step = Number(resolved.max_sat_change_per_step) || 0.1;
        initialPressure = Number(resolved.initialPressure) || 300;
        initialSaturation = Number(resolved.initialSaturation) || 0.3;
        mu_w = Number(resolved.mu_w) || 0.5;
        mu_o = Number(resolved.mu_o) || 1.0;
        c_o = Number(resolved.c_o) || 1e-5;
        c_w = Number(resolved.c_w) || 3e-6;
        rock_compressibility = Number(resolved.rock_compressibility) || 1e-6;
        depth_reference = Number(resolved.depth_reference) || 0;
        volume_expansion_o = Number(resolved.volume_expansion_o) || 1.0;
        volume_expansion_w = Number(resolved.volume_expansion_w) || 1.0;
        rho_w = Number(resolved.rho_w) || 1000;
        rho_o = Number(resolved.rho_o) || 800;
        s_wc = Number(resolved.s_wc) || 0.1;
        s_or = Number(resolved.s_or) || 0.1;
        n_w = Number(resolved.n_w) || 2.0;
        n_o = Number(resolved.n_o) || 2.0;
        k_rw_max = Number(resolved.k_rw_max) || 1.0;
        k_ro_max = Number(resolved.k_ro_max) || 1.0;
        well_radius = Number(resolved.well_radius) || 0.1;
        well_skin = Number(resolved.well_skin) || 0;
        max_pressure_change_per_step = Number(resolved.max_pressure_change_per_step) || 75;
        max_well_rate_change_fraction = Number(resolved.max_well_rate_change_fraction) || 0.75;
        gravityEnabled = Boolean(resolved.gravityEnabled);
        capillaryEnabled = resolved.capillaryEnabled !== false;
        capillaryPEntry = Number(resolved.capillaryPEntry) || 0;
        capillaryLambda = Number(resolved.capillaryLambda) || 2;
        injectorEnabled = resolved.injectorEnabled !== false;
        analyticalSolutionMode = injectorEnabled ? 'waterflood' : 'depletion';
        injectorControlMode = resolved.injectorControlMode === 'rate' ? 'rate' : 'pressure';
        producerControlMode = resolved.producerControlMode === 'rate' ? 'rate' : 'pressure';
        injectorBhp = Number(resolved.injectorBhp) || 400;
        producerBhp = Number(resolved.producerBhp) || 100;
        targetInjectorRate = Number(resolved.targetInjectorRate) || 350;
        targetProducerRate = Number(resolved.targetProducerRate) || 350;
        if (resolved.permMode && isPermMode(resolved.permMode)) { permMode = resolved.permMode; }
        if (resolved.uniformPermX !== undefined) uniformPermX = Number(resolved.uniformPermX);
        if (resolved.uniformPermY !== undefined) uniformPermY = Number(resolved.uniformPermY);
        if (resolved.uniformPermZ !== undefined) uniformPermZ = Number(resolved.uniformPermZ);
        if (resolved.minPerm !== undefined) minPerm = Number(resolved.minPerm);
        if (resolved.maxPerm !== undefined) maxPerm = Number(resolved.maxPerm);
        if (resolved.useRandomSeed !== undefined) useRandomSeed = Boolean(resolved.useRandomSeed);
        if (resolved.randomSeed !== undefined) randomSeed = Number(resolved.randomSeed);
        if (resolved.layerPermsX) layerPermsX = parseLayerValues(resolved.layerPermsX);
        if (resolved.layerPermsY) layerPermsY = parseLayerValues(resolved.layerPermsY);
        if (resolved.layerPermsZ) layerPermsZ = parseLayerValues(resolved.layerPermsZ);
        handleNzOrPermModeChange();
        injectorI = Number(resolved.injectorI) || 0;
        injectorJ = Number(resolved.injectorJ) || 0;
        producerI = Number(resolved.producerI) || nx - 1;
        producerJ = Number(resolved.producerJ) || 0;
        resetModelAndVisualizationState(true, false);
    }

    async function loadPreRunCase(key: string) {
        const requestToken = ++preRunLoadToken;
        preRunLoading = true;
        preRunData = null;
        preRunWarning = '';
        try {
            const url = `${import.meta.env.BASE_URL}cases/${key}.json`;
            const resp = await fetch(url, { cache: 'no-store' });
            if (requestToken !== preRunLoadToken || key !== activeCase || isCustomMode) return;
            if (!resp.ok) { preRunData = null; return; }
            const data = await resp.json();
            if (requestToken !== preRunLoadToken || key !== activeCase || isCustomMode) return;
            preRunData = data;

            const expectedCellCount = Math.max(1, Number(nx) * Number(ny) * Number(nz));
            const loadedHistory = Array.isArray(data.history) ? data.history : [];
            const loadedFinalGrid: GridState | null = data.finalGrid ? (data.finalGrid as GridState) : null;
            const validHistoryEntries: HistoryEntry[] = [];
            let historyHasMismatches = false;
            for (let i = 0; i < loadedHistory.length; i++) {
                const entry = loadedHistory[i];
                if (entry?.grid?.pressure && entry.grid.pressure.length === expectedCellCount) {
                    validHistoryEntries.push({
                        time: Number(entry.time ?? 0),
                        grid: entry.grid,
                        wells: Array.isArray(entry.wells) ? entry.wells : [],
                        rateHistory: [],
                        solverWarning: '',
                        recordHistory: false,
                    } as HistoryEntry);
                } else {
                    historyHasMismatches = true;
                }
            }
            const finalGridMatches = Boolean(
                loadedFinalGrid && loadedFinalGrid.pressure && loadedFinalGrid.pressure.length === expectedCellCount,
            );

            history = validHistoryEntries;
            currentIndex = validHistoryEntries.length - 1;

            const selectedHistoryEntry = currentIndex >= 0 ? validHistoryEntries[currentIndex] : null;
            const selectedHistoryGrid = selectedHistoryEntry?.grid ?? null;

            if (selectedHistoryGrid && selectedHistoryGrid.pressure && selectedHistoryGrid.pressure.length === expectedCellCount) {
                gridStateRaw = selectedHistoryGrid;
                wellStateRaw = selectedHistoryEntry?.wells ?? data.finalWells ?? null;
                simTime = Number(selectedHistoryEntry?.time ?? data.simTime ?? 0);
            } else if (finalGridMatches) {
                gridStateRaw = loadedFinalGrid;
                wellStateRaw = data.finalWells ?? null;
                simTime = Number(data.simTime ?? 0);
            } else {
                gridStateRaw = null;
                wellStateRaw = data.finalWells ?? null;
                simTime = Number(data.simTime ?? 0);
                preRunWarning = `Pre-run data grid size mismatch for selected case (${nx}×${ny}×${nz}, expected ${expectedCellCount} cells). Re-export case data.`;
            }

            if (!preRunWarning && historyHasMismatches) {
                preRunWarning = `Some pre-run history snapshots do not match selected grid size (${nx}×${ny}×${nz}); only valid snapshots were loaded.`;
            }

            rateHistory = normalizeRateHistory(data.rateHistory);
            rateHistory.forEach((point) => {
                if (point.total_production_liquid_reservoir === undefined)
                    point.total_production_liquid_reservoir = point.total_production_liquid || 0;
                if (point.total_injection_reservoir === undefined)
                    point.total_injection_reservoir = point.total_injection || 0;
                if (point.material_balance_error_m3 === undefined)
                    point.material_balance_error_m3 = 0;
            });
            runCompleted = true;
            preRunContinuationAvailable = true;
            runtimeWarning = 'Pre-run case loaded. Click Run to continue from the saved endpoint.';
            vizRevision += 1;
        } catch (e) {
            console.error('Failed to load pre-run data:', e);
            if (requestToken === preRunLoadToken) {
                preRunData = null;
                preRunWarning = 'Failed to load pre-run data for this case.';
            }
        } finally {
            if (requestToken === preRunLoadToken) {
                preRunLoading = false;
            }
        }
    }

    // ===== Public API =====
    // Using getters/setters for $state variables that need bind: in the template
    return {
        // ----- Simulation inputs (read-write for bind:) -----
        get nx() { return nx; }, set nx(v) { nx = v; },
        get ny() { return ny; }, set ny(v) { ny = v; },
        get nz() { return nz; }, set nz(v) { nz = v; },
        get cellDx() { return cellDx; }, set cellDx(v) { cellDx = v; },
        get cellDy() { return cellDy; }, set cellDy(v) { cellDy = v; },
        get cellDz() { return cellDz; }, set cellDz(v) { cellDz = v; },
        get delta_t_days() { return delta_t_days; }, set delta_t_days(v) { delta_t_days = v; },
        get steps() { return steps; }, set steps(v) { steps = v; },
        get initialPressure() { return initialPressure; }, set initialPressure(v) { initialPressure = v; },
        get initialSaturation() { return initialSaturation; }, set initialSaturation(v) { initialSaturation = v; },
        get reservoirPorosity() { return reservoirPorosity; }, set reservoirPorosity(v) { reservoirPorosity = v; },
        get mu_w() { return mu_w; }, set mu_w(v) { mu_w = v; },
        get mu_o() { return mu_o; }, set mu_o(v) { mu_o = v; },
        get c_o() { return c_o; }, set c_o(v) { c_o = v; },
        get c_w() { return c_w; }, set c_w(v) { c_w = v; },
        get rock_compressibility() { return rock_compressibility; }, set rock_compressibility(v) { rock_compressibility = v; },
        get depth_reference() { return depth_reference; }, set depth_reference(v) { depth_reference = v; },
        get volume_expansion_o() { return volume_expansion_o; }, set volume_expansion_o(v) { volume_expansion_o = v; },
        get volume_expansion_w() { return volume_expansion_w; }, set volume_expansion_w(v) { volume_expansion_w = v; },
        get rho_w() { return rho_w; }, set rho_w(v) { rho_w = v; },
        get rho_o() { return rho_o; }, set rho_o(v) { rho_o = v; },
        get permMode() { return permMode; }, set permMode(v) { permMode = v; },
        get uniformPermX() { return uniformPermX; }, set uniformPermX(v) { uniformPermX = v; },
        get uniformPermY() { return uniformPermY; }, set uniformPermY(v) { uniformPermY = v; },
        get uniformPermZ() { return uniformPermZ; }, set uniformPermZ(v) { uniformPermZ = v; },
        get minPerm() { return minPerm; }, set minPerm(v) { minPerm = v; },
        get maxPerm() { return maxPerm; }, set maxPerm(v) { maxPerm = v; },
        get useRandomSeed() { return useRandomSeed; }, set useRandomSeed(v) { useRandomSeed = v; },
        get randomSeed() { return randomSeed; }, set randomSeed(v) { randomSeed = v; },
        get layerPermsX() { return layerPermsX; }, set layerPermsX(v) { layerPermsX = v; },
        get layerPermsY() { return layerPermsY; }, set layerPermsY(v) { layerPermsY = v; },
        get layerPermsZ() { return layerPermsZ; }, set layerPermsZ(v) { layerPermsZ = v; },
        get s_wc() { return s_wc; }, set s_wc(v: number) { s_wc = v; },
        get s_or() { return s_or; }, set s_or(v: number) { s_or = v; },
        get n_w() { return n_w; }, set n_w(v: number) { n_w = v; },
        get n_o() { return n_o; }, set n_o(v: number) { n_o = v; },
        get k_rw_max() { return k_rw_max; }, set k_rw_max(v: number) { k_rw_max = v; },
        get k_ro_max() { return k_ro_max; }, set k_ro_max(v: number) { k_ro_max = v; },
        get well_radius() { return well_radius; }, set well_radius(v: number) { well_radius = v; },
        get well_skin() { return well_skin; }, set well_skin(v) { well_skin = v; },
        get injectorBhp() { return injectorBhp; }, set injectorBhp(v) { injectorBhp = v; },
        get producerBhp() { return producerBhp; }, set producerBhp(v) { producerBhp = v; },
        get injectorControlMode() { return injectorControlMode; }, set injectorControlMode(v) { injectorControlMode = v; },
        get producerControlMode() { return producerControlMode; }, set producerControlMode(v) { producerControlMode = v; },
        get injectorEnabled() { return injectorEnabled; }, set injectorEnabled(v) { injectorEnabled = v; },
        get targetInjectorRate() { return targetInjectorRate; }, set targetInjectorRate(v) { targetInjectorRate = v; },
        get targetProducerRate() { return targetProducerRate; }, set targetProducerRate(v) { targetProducerRate = v; },
        get injectorI() { return injectorI; }, set injectorI(v) { injectorI = v; },
        get injectorJ() { return injectorJ; }, set injectorJ(v) { injectorJ = v; },
        get producerI() { return producerI; }, set producerI(v) { producerI = v; },
        get producerJ() { return producerJ; }, set producerJ(v) { producerJ = v; },
        get max_sat_change_per_step() { return max_sat_change_per_step; }, set max_sat_change_per_step(v) { max_sat_change_per_step = v; },
        get max_pressure_change_per_step() { return max_pressure_change_per_step; }, set max_pressure_change_per_step(v) { max_pressure_change_per_step = v; },
        get max_well_rate_change_fraction() { return max_well_rate_change_fraction; }, set max_well_rate_change_fraction(v) { max_well_rate_change_fraction = v; },
        get gravityEnabled() { return gravityEnabled; }, set gravityEnabled(v) { gravityEnabled = v; },
        get capillaryEnabled() { return capillaryEnabled; }, set capillaryEnabled(v) { capillaryEnabled = v; },
        get capillaryPEntry() { return capillaryPEntry; }, set capillaryPEntry(v) { capillaryPEntry = v; },
        get capillaryLambda() { return capillaryLambda; }, set capillaryLambda(v) { capillaryLambda = v; },
        get analyticalSolutionMode() { return analyticalSolutionMode; }, set analyticalSolutionMode(v) { analyticalSolutionMode = v; },
        get analyticalDepletionRateScale() { return analyticalDepletionRateScale; }, set analyticalDepletionRateScale(v) { analyticalDepletionRateScale = v; },
        get historyInterval() { return userHistoryInterval ?? defaultHistoryInterval; }, set historyInterval(v) { userHistoryInterval = v; },
        get currentIndex() { return currentIndex; }, set currentIndex(v) { currentIndex = v; },

        // ----- Read-only state -----
        get wasmReady() { return wasmReady; },
        get workerRunning() { return workerRunning; },
        get runCompleted() { return runCompleted; },
        get currentRunTotalSteps() { return currentRunTotalSteps; },
        get currentRunStepsCompleted() { return currentRunStepsCompleted; },
        get activeMode() { return activeMode; },
        get activeCase() { return activeCase; },
        get isCustomMode() { return isCustomMode; },
        get toggles() { return toggles; },
        set toggles(v) { toggles = v; },
        get matchingCases() { return matchingCases; },
        get preRunData() { return preRunData; },
        get preRunLoading() { return preRunLoading; },
        get preRunWarning() { return preRunWarning; },
        get preRunContinuationAvailable() { return preRunContinuationAvailable; },
        get gridStateRaw() { return gridStateRaw; },
        get wellStateRaw() { return wellStateRaw; },
        get simTime() { return simTime; },
        get rateHistory() { return rateHistory; },
        get analyticalProductionData() { return analyticalProductionData; },
        set analyticalProductionData(v) { analyticalProductionData = v; },
        get analyticalMeta() { return analyticalMeta; },
        set analyticalMeta(v) { analyticalMeta = v; },
        get runtimeWarning() { return runtimeWarning; },
        get runtimeError() { return runtimeError; },
        get solverWarning() { return solverWarning; },
        get vizRevision() { return vizRevision; },
        get modelNeedsReinit() { return modelNeedsReinit; },
        get modelReinitNotice() { return modelReinitNotice; },
        get history() { return history; },
        get playing() { return playing; },
        get playSpeed() { return playSpeed; },
        get profileStats() { return profileStats; },

        // ----- Derived -----
        get ooipM3() { return ooipM3; },
        get poreVolumeM3() { return poreVolumeM3; },
        get rateControlledWells() { return rateControlledWells; },
        get latestInjectionRate() { return latestInjectionRate; },
        get avgReservoirPressureSeries() { return avgReservoirPressureSeries; },
        get avgWaterSaturationSeries() { return avgWaterSaturationSeries; },
        get replayTime() { return replayTime; },
        get validationErrors() { return validationErrors; },
        get validationWarnings() { return validationWarnings; },
        get hasValidationErrors() { return hasValidationErrors; },
        get estimatedRunSeconds() { return estimatedRunSeconds; },
        get longRunEstimate() { return longRunEstimate; },

        // ----- Actions -----
        setupWorker,
        dispose,
        initSimulator,
        runSteps,
        stepOnce,
        stopRun,
        checkConfigDiff,
        handleModeChange,
        handleCaseChange,
        handleCustomMode,
        handleAnalyticalSolutionModeChange,
        handleNzOrPermModeChange,
        resolveCustomSubCase,
        applyHistoryIndex,
        play,
        stopPlaying,
        togglePlay,
        next,
        prev,
    };
}

export type SimulationStore = ReturnType<typeof createSimulationStore>;
