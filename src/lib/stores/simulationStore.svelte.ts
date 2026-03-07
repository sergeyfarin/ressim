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
    catalog,
    buildCaseKey,
    composeCaseParams,
    getBenchmarkEntry,
    getDefaultToggles,
    getDisabledOptions,
    stabilizeToggleState,
    type CaseMode,
    type ToggleState
} from '../catalog/caseCatalog';
import { buildCreatePayloadFromState } from '../buildCreatePayload';
import {
    validateInputs as validateSimulationInputs,
    type SimulationInputs,
    type ValidationState as InputValidationState,
    type ValidationWarning,
} from '../validateInputs';
import { buildWarningPolicy } from '../warningPolicy';
import {
    buildBenchmarkCloneProvenance,
    buildBasePresetProfile,
    buildOverrideResetPlan,
    buildParameterOverrides,
    evaluateAnalyticalStatus,
    groupParameterOverrides,
    shouldAllowBenchmarkClone,
    shouldAutoClearModifiedState,
    type AnalyticalStatus,
    type BenchmarkProvenance,
} from './phase2PresetContract';

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

const EMPTY_PROFILE_STATS: ProfileStats = {
    batchMs: 0,
    avgStepMs: 0,
    extractMs: 0,
    renderApplyMs: 0,
    snapshotsSent: 0,
};

const MAX_HISTORY_ENTRIES = 300;

const CUSTOM_SUBCASE_BY_MODE: Partial<Record<CaseMode, { key: string; label: string }>> = {
    dep: { key: 'depletion_custom_subcase', label: 'Custom Depletion Sub-case' },
    wf: { key: 'waterflood_custom_subcase', label: 'Custom Waterflood Sub-case' },
    sim: { key: 'simulation_custom_subcase', label: 'Custom Simulation Sub-case' },
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
    let activeMode = $state<CaseMode>('dep');
    let activeCase = $state('');
    let isModified = $state(false);

    let toggles = $state<ToggleState>(getDefaultToggles('dep'));
    let benchmarkProvenance: BenchmarkProvenance | null = $state(null);

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
    let modelConfigSignature = $state('');
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

    const disabledOptions = $derived(getDisabledOptions(toggles));

    // ===== Validation =====
    function buildValidationInput(): SimulationInputs {
        return {
            nx, ny, nz,
            cellDx, cellDy, cellDz,
            steps,
            initialSaturation,
            delta_t_days,
            well_radius,
            mu_w, mu_o,
            c_o, c_w,
            rock_compressibility,
            volume_expansion_o,
            volume_expansion_w,
            max_sat_change_per_step,
            max_pressure_change_per_step,
            max_well_rate_change_fraction,
            injectorI, injectorJ,
            producerI, producerJ,
            s_wc, s_or,
            minPerm, maxPerm,
            injectorEnabled,
            injectorControlMode,
            producerControlMode,
            injectorBhp,
            producerBhp,
            targetInjectorRate,
            targetProducerRate,
        };
    }

    const validationState: InputValidationState = $derived(
        validateSimulationInputs(buildValidationInput()),
    );
    const validationErrors: Record<string, string> = $derived(validationState.errors);
    const validationWarnings: ValidationWarning[] = $derived(validationState.warnings);
    const hasValidationErrors = $derived(Object.keys(validationErrors).length > 0);
    const estimatedRunSeconds = $derived(
        Math.max(0, (Number(profileStats.avgStepMs || 0) * Number(steps || 0)) / 1000),
    );
    const longRunEstimate = $derived(estimatedRunSeconds > 10);

    const basePreset = $derived.by(() => {
        const benchmarkLabel = toggles.benchmarkId
            ? getBenchmarkEntry(toggles.benchmarkId)?.label ?? null
            : null;

        return buildBasePresetProfile({
            key: activeCase,
            mode: activeMode,
            toggles,
            isModified,
            benchmarkLabel,
        });
    });

    const parameterOverrides = $derived.by(() => {
        return buildParameterOverrides({
            currentParams: buildCurrentParameterSnapshot(),
            baseParams: composeCaseParams(toggles),
        });
    });

    const parameterOverrideGroups = $derived(groupParameterOverrides(parameterOverrides));
    const parameterOverrideCount = $derived(Object.keys(parameterOverrides).length);

    const analyticalStatus = $derived.by(() => {
        return evaluateAnalyticalStatus({
            activeMode,
            analyticalMode: analyticalSolutionMode,
            injectorEnabled,
            gravityEnabled,
            capillaryEnabled,
            permMode,
            toggles,
        });
    });

    const warningPolicy = $derived.by(() => {
        return buildWarningPolicy({
            validationErrors,
            validationWarnings,
            analyticalStatus: analyticalStatus as AnalyticalStatus,
            runtimeWarning,
            solverWarning,
            modelReinitNotice,
            longRunEstimate,
            estimatedRunSeconds,
        });
    });

    $effect(() => {
        if (!shouldAutoClearModifiedState({
            isModified,
            activeMode,
            benchmarkProvenance,
            parameterOverrideCount,
        })) return;

        isModified = false;
        baseCaseSignature = buildCaseSignature();
    });

    // ===== Internal Helpers =====

    function buildCurrentParameterSnapshot(): Record<string, unknown> {
        return {
            nx,
            ny,
            nz,
            cellDx,
            cellDy,
            cellDz,
            initialPressure,
            initialSaturation,
            reservoirPorosity,
            mu_w,
            mu_o,
            c_o,
            c_w,
            rock_compressibility,
            depth_reference,
            volume_expansion_o,
            volume_expansion_w,
            rho_w,
            rho_o,
            permMode,
            uniformPermX,
            uniformPermY,
            uniformPermZ,
            minPerm,
            maxPerm,
            useRandomSeed,
            randomSeed,
            layerPermsX: [...layerPermsX],
            layerPermsY: [...layerPermsY],
            layerPermsZ: [...layerPermsZ],
            s_wc,
            s_or,
            n_w,
            n_o,
            k_rw_max,
            k_ro_max,
            well_radius,
            well_skin,
            injectorBhp,
            producerBhp,
            injectorControlMode,
            producerControlMode,
            injectorEnabled,
            targetInjectorRate,
            targetProducerRate,
            injectorI,
            injectorJ,
            producerI,
            producerJ,
            max_sat_change_per_step,
            max_pressure_change_per_step,
            max_well_rate_change_fraction,
            gravityEnabled,
            capillaryEnabled,
            capillaryPEntry,
            capillaryLambda,
            analyticalSolutionMode,
            analyticalDepletionRateScale,
        };
    }

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
            reservoirPorosity: Number(reservoirPorosity),
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
            new URL('../workers/sim.worker.ts', import.meta.url),
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

    function resolveCustomSubCase(mode: CaseMode | string): { key: string; label: string } | null {
        const raw = String(mode ?? '').toLowerCase();
        const normalizedMode: CaseMode | null =
            raw === 'dep' || raw === 'depletion' ? 'dep'
                : raw === 'wf' || raw === 'waterflood' ? 'wf'
                    : raw === 'sim' || raw === 'simulation' ? 'sim'
                        : raw === 'benchmark' ? 'benchmark'
                            : null;
        if (!normalizedMode) return null;
        return CUSTOM_SUBCASE_BY_MODE[normalizedMode] ?? null;
    }

    function maybeSwitchToCustomSubCaseOnReinit(): boolean {
        if (isModified || !activeCase || !baseCaseSignature) return false;

        const customSubCase = resolveCustomSubCase(activeMode);
        if (!customSubCase) return false;
        const nextSignature = buildCaseSignature();
        if (nextSignature === baseCaseSignature) return false;
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
        // If model inputs changed, create a fresh simulator first, then run this batch.
        if (modelNeedsReinit) {
            const initialized = initSimulator({ silent: true });
            if (!initialized) return;
        }

        if (!simWorker || workerRunning) return;
        if (hasValidationErrors) {
            runtimeError = 'Input validation failed. Resolve highlighted fields before running.';
            return;
        }
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
            },
        });
    }

    function stepOnce() { runSimulationBatch(1, 1); }
    function runSteps() { runSimulationBatch(Number(steps), Number(userHistoryInterval ?? defaultHistoryInterval)); }
    function stopRun() { if (simWorker) simWorker.postMessage({ type: 'stop' }); }

    // ===== Config Diff Detection =====

    function checkConfigDiff() {
        const nextSignature = buildModelResetKey();
        if (!modelConfigSignature) { modelConfigSignature = nextSignature; return; }
        if (nextSignature === modelConfigSignature) return;
        modelConfigSignature = nextSignature;
        if (skipNextAutoModelReset) { skipNextAutoModelReset = false; return; }
        if (wasmReady && simWorker && isModified && lastCreateSignature) {
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
        isModified = false;
        benchmarkProvenance = null;
        activeMode = mode;
        toggles = getDefaultToggles(mode);
        baseCaseSignature = '';

        handleToggleChange();
    }

    function handleToggleChange(dimKey?: string, value?: string) {
        const nextToggles = { ...toggles };
        if (dimKey && value) {
            nextToggles[dimKey] = value;
        }
        toggles = stabilizeToggleState(nextToggles);

        const newKey = buildCaseKey(toggles);
        activeCase = newKey;
        isModified = false;
        benchmarkProvenance = null;

        applyCaseParams(composeCaseParams(toggles));
        baseCaseSignature = buildCaseSignature();
    }

    function handleParamEdit() {
        if (isModified) return;
        isModified = true;
        baseCaseSignature = '';
    }

    function cloneActiveBenchmarkToCustom(): boolean {
        if (!shouldAllowBenchmarkClone({ activeMode, isModified })) return false;

        const benchmarkId = toggles.benchmarkId ?? null;
        const benchmarkLabel = benchmarkId
            ? getBenchmarkEntry(benchmarkId)?.label ?? null
            : null;
        const provenance = buildBenchmarkCloneProvenance({
            benchmarkId,
            sourceCaseKey: activeCase,
            sourceLabel: benchmarkLabel,
        });

        handleParamEdit();
        if (provenance && !benchmarkProvenance) {
            benchmarkProvenance = provenance;
        }

        return true;
    }

    function setBenchmarkProvenance(provenance: BenchmarkProvenance | null) {
        benchmarkProvenance = provenance;
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
        const resolved = { ...catalog.defaults, ...params };
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

        // Sync analyticalSolutionMode using the actual resolved parameters
        if (resolved.analyticalSolutionMode === 'waterflood' || resolved.analyticalSolutionMode === 'depletion') {
            analyticalSolutionMode = resolved.analyticalSolutionMode;
        } else if (resolved.analyticalSolutionMode === 'none' && resolved.mode) {
            analyticalSolutionMode = resolved.mode === 'wf' ? 'waterflood' : 'depletion';
        } else {
            analyticalSolutionMode = injectorEnabled ? 'waterflood' : 'depletion';
        }
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
        modelNeedsReinit = true;
        modelReinitNotice = '';
    }

    // ===== Public API =====
    const scenarioSelection = {
        get activeMode() { return activeMode; },
        get activeCase() { return activeCase; },
        get isModified() { return isModified; },
        get basePreset() { return basePreset; },
        get benchmarkProvenance() { return benchmarkProvenance; },
        get toggles() { return toggles; },
        set toggles(v) { toggles = v; },
        get disabledOptions() { return disabledOptions; },
        handleModeChange,
        handleToggleChange,
        handleParamEdit,
        cloneActiveBenchmarkToCustom,
        resolveCustomSubCase,
        setBenchmarkProvenance,
    };

    const parameterState = {
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
        get s_wc() { return s_wc; }, set s_wc(v) { s_wc = v; },
        get s_or() { return s_or; }, set s_or(v) { s_or = v; },
        get n_w() { return n_w; }, set n_w(v) { n_w = v; },
        get n_o() { return n_o; }, set n_o(v) { n_o = v; },
        get k_rw_max() { return k_rw_max; }, set k_rw_max(v) { k_rw_max = v; },
        get k_ro_max() { return k_ro_max; }, set k_ro_max(v) { k_ro_max = v; },
        get well_radius() { return well_radius; }, set well_radius(v) { well_radius = v; },
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
        get ooipM3() { return ooipM3; },
        get poreVolumeM3() { return poreVolumeM3; },
        get rateControlledWells() { return rateControlledWells; },
        get validationErrors() { return validationErrors; },
        get validationWarnings() { return validationWarnings; },
        get hasValidationErrors() { return hasValidationErrors; },
        get parameterOverrides() { return parameterOverrides; },
        get parameterOverrideGroups() { return parameterOverrideGroups; },
        get parameterOverrideCount() { return parameterOverrideCount; },
        resetOverrideGroupsToBase(groupKeys: string[]) {
            if (!Array.isArray(groupKeys) || groupKeys.length === 0) {
                return { resetCount: 0 };
            }

            const resetPlan = buildOverrideResetPlan({
                groupKeys,
                groupedOverrides: parameterOverrideGroups,
                overrides: parameterOverrides,
            });

            for (const item of resetPlan) {
                const nextValue = Array.isArray(item.base) ? [...item.base] : item.base;
                (parameterState as Record<string, unknown>)[item.key] = nextValue;
            }

            return { resetCount: resetPlan.length };
        },
        handleAnalyticalSolutionModeChange,
        handleNzOrPermModeChange,
    };

    const runtimeState = {
        get wasmReady() { return wasmReady; },
        get workerRunning() { return workerRunning; },
        get runCompleted() { return runCompleted; },
        get currentRunTotalSteps() { return currentRunTotalSteps; },
        get currentRunStepsCompleted() { return currentRunStepsCompleted; },
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
        get currentIndex() { return currentIndex; },
        set currentIndex(v) { currentIndex = v; },
        get playing() { return playing; },
        get playSpeed() { return playSpeed; },
        get profileStats() { return profileStats; },
        get latestInjectionRate() { return latestInjectionRate; },
        get avgReservoirPressureSeries() { return avgReservoirPressureSeries; },
        get avgWaterSaturationSeries() { return avgWaterSaturationSeries; },
        get replayTime() { return replayTime; },
        get estimatedRunSeconds() { return estimatedRunSeconds; },
        get longRunEstimate() { return longRunEstimate; },
        get analyticalStatus() { return analyticalStatus; },
        get warningPolicy() { return warningPolicy; },
        setupWorker,
        dispose,
        initSimulator,
        runSteps,
        stepOnce,
        stopRun,
        checkConfigDiff,
        applyHistoryIndex,
        play,
        stopPlaying,
        togglePlay,
        next,
        prev,
    };

    return {
        scenarioSelection,
        parameterState,
        runtimeState,
    };
}

export type SimulationStore = ReturnType<typeof createSimulationStore>;
