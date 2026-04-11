import type {
    WorkerMessage,
    SimulatorSnapshot,
    RateHistoryPoint,
    GridState,
    WellState,
} from '../simulator-types';
import {
    getBenchmarkFamily,
    getBenchmarkVariantsForFamily,
    type BenchmarkSensitivityAxisKey,
} from '../catalog/caseCatalog';
import {
    buildBenchmarkCreatePayload,
    buildBenchmarkRunResult,
    buildBenchmarkRunSpecs,
    resolveBenchmarkReferenceComparisons,
    type BenchmarkRunResult as ReferenceRunResult,
    type BenchmarkRunSpec as ReferenceRunSpec,
} from '../benchmarkRunModel';
import { cloneTerminationPolicy } from '../workers/terminationPolicy';
import { buildWarningPolicy, type AnalyticalStatus } from '../warningPolicy';
import { getScenario, getScenarioWithVariantParams, resolveCapabilities } from '../catalog/scenarios';
import type { ParameterStore } from './parameterStore.svelte';

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

// ---------- Navigation interface (breaks circular dep) ----------

/** Minimal navigation surface RuntimeStore needs. Set via connectNavigation(). */
interface NavRef {
    maybySwitchToCustomSubCaseOnReinit(): boolean;
    readonly activeScenarioObject: ReturnType<typeof getScenario>;
    readonly isCustomMode: boolean;
    readonly isModified: boolean;
    readonly activeReferenceFamily: ReturnType<typeof getBenchmarkFamily>;
    restoreActiveReferenceBaseDisplay(): void;
    readonly analyticalStatus: AnalyticalStatus;
}

// ---------- Store ----------

class RuntimeStoreImpl {
    readonly #params!: ParameterStore;
    #nav: NavRef | null = null;

    constructor(params: ParameterStore) {
        this.#params = params;
    }

    connectNavigation(nav: NavRef): void {
        this.#nav = nav;
    }

    // ===== $state: Simulation Output / Runtime =====

    wasmReady = $state(false);
    simWorker: Worker | null = null;
    runCompleted = $state(false);
    workerRunning = $state(false);
    stopPending = $state(false);
    currentRunTotalSteps = $state(0);
    currentRunStepsCompleted = $state(0);
    referenceSweepRunning = $state(false);
    referenceSweepError = $state('');
    referenceTotalRuns = $state(0);
    referenceRunsCompleted = $state(0);
    referenceRunQueue = $state<ReferenceRunSpec[]>([]);
    activeReferenceRunSpec: ReferenceRunSpec | null = $state(null);
    referenceRunResults = $state<ReferenceRunResult[]>([]);

    // Display data
    gridStateRaw: GridState | null = $state(null);
    wellStateRaw: WellState | null = $state(null);
    simTime = $state(0);
    rateHistory = $state<RateHistoryPoint[]>([]);
    runtimeWarning = $state('');
    solverWarning = $state('');
    runtimeError = $state('');
    convergenceHitCount = $state(0);
    convergenceHitCaseName = $state('');
    latestStepSolverWarning = $state('');
    referenceConvergenceWarnings = $state<string[]>([]);
    vizRevision = $state(0);
    modelReinitNotice = $state('');
    modelNeedsReinit = $state(false);
    modelConfigSignature = $state('');

    // History / replay
    history = $state<HistoryEntry[]>([]);
    currentIndex = $state(-1);
    playing = $state(false);
    playSpeed = $state(2);
    playTimer: ReturnType<typeof setInterval> | null = null;

    // Profile
    profileStats: ProfileStats = $state({ ...EMPTY_PROFILE_STATS });
    lastCreateSignature = $state('');
    pendingAutoReinit = $state(false);

    // ===== $derived =====

    latestInjectionRate = $derived.by(() => {
        if (!Array.isArray(this.rateHistory) || this.rateHistory.length === 0) return 0;
        for (let i = this.rateHistory.length - 1; i >= 0; i--) {
            const q = Number(this.rateHistory[i]?.total_injection);
            if (Number.isFinite(q) && q > 0) return q;
        }
        return 0;
    });

    avgReservoirPressureSeries = $derived(
        this.rateHistory.map((point) => point.avg_reservoir_pressure ?? null),
    );

    avgWaterSaturationSeries = $derived(
        this.rateHistory.map((point) => point.avg_water_saturation ?? null),
    );

    replayTime = $derived(
        this.history.length > 0 && this.currentIndex >= 0 && this.currentIndex < this.history.length
            ? this.history[this.currentIndex].time
            : null,
    );

    referenceSweepProgressLabel = $derived.by(() => {
        if (this.referenceTotalRuns <= 0) return '';
        if (this.activeReferenceRunSpec) {
            const stepPart = this.currentRunTotalSteps > 0
                ? ` — ${this.currentRunStepsCompleted}/${this.currentRunTotalSteps} steps`
                : '';
            return `Case ${this.referenceRunsCompleted + 1}/${this.referenceTotalRuns}${stepPart}`;
        }
        return `${this.referenceRunsCompleted}/${this.referenceTotalRuns} done`;
    });

    estimatedRunSeconds = $derived(
        Math.max(0, (Number(this.profileStats.avgStepMs || 0) * Number(this.#params.steps || 0)) / 1000),
    );

    longRunEstimate = $derived(this.estimatedRunSeconds > 10);

    warningPolicy = $derived.by(() => {
        return buildWarningPolicy({
            validationErrors: this.#params.validationErrors,
            validationWarnings: this.#params.validationWarnings,
            analyticalStatus: (this.#nav?.analyticalStatus ?? { level: 'ok', reason: '' }) as AnalyticalStatus,
            runtimeWarning: this.runtimeWarning,
            solverWarning: this.solverWarning,
            modelReinitNotice: this.modelReinitNotice,
            longRunEstimate: this.longRunEstimate,
            estimatedRunSeconds: this.estimatedRunSeconds,
        });
    });

    // ===== Internal Helpers =====

    formatConvergenceCaseWarning(label: string, count: number, partial = false): string {
        return `"${label}" (${count}×${partial ? ', partial' : ''})`;
    }

    buildConvergenceSummary(caseWarnings: string[]): string {
        return `Convergence issues — check charts for anomalies. Cases affected: ${caseWarnings.join(', ')}.`;
    }

    refreshConvergenceWarningDisplay(options: { includeActiveReferenceCase?: boolean } = {}) {
        const { includeActiveReferenceCase = false } = options;

        if (
            this.referenceSweepRunning ||
            this.activeReferenceRunSpec ||
            this.referenceConvergenceWarnings.length > 0
        ) {
            const warnings = [...this.referenceConvergenceWarnings];
            if (
                includeActiveReferenceCase &&
                this.activeReferenceRunSpec &&
                this.convergenceHitCount > 0
            ) {
                warnings.push(
                    this.formatConvergenceCaseWarning(
                        this.convergenceHitCaseName || this.activeReferenceRunSpec.label,
                        this.convergenceHitCount,
                        true,
                    ),
                );
            }

            this.solverWarning = warnings.length > 0
                ? this.buildConvergenceSummary(warnings)
                : '';
            return;
        }

        this.solverWarning = this.convergenceHitCount > 0
            ? `Convergence issue hit ${this.convergenceHitCount} time${this.convergenceHitCount === 1 ? '' : 's'} during the run — check charts for anomalies.`
            : '';
    }

    clearReferenceRunnerState(clearResults = true) {
        this.referenceSweepRunning = false;
        this.referenceSweepError = '';
        this.referenceTotalRuns = 0;
        this.referenceRunsCompleted = 0;
        this.referenceRunQueue = [];
        this.activeReferenceRunSpec = null;
        if (clearResults) {
            this.referenceRunResults = [];
        }
    }

    captureCurrentFinalSnapshot(): SimulatorSnapshot | null {
        if (this.gridStateRaw && this.wellStateRaw) {
            return {
                time: this.simTime,
                grid: this.gridStateRaw,
                wells: this.wellStateRaw,
            };
        }

        return this.history.length > 0 ? this.history[this.history.length - 1] : null;
    }

    hydrateRuntimeFromReferenceResult(result: ReferenceRunResult) {
        this.stopPlaying();
        this.history = result.history ?? [];
        this.currentIndex = this.history.length > 0 ? this.history.length - 1 : -1;
        this.gridStateRaw = result.finalSnapshot?.grid ?? null;
        this.wellStateRaw = result.finalSnapshot?.wells ?? null;
        this.simTime = result.finalSnapshot?.time ?? Number(result.rateHistory.at(-1)?.time ?? 0);
        this.rateHistory = result.rateHistory ?? [];
        this.runCompleted = true;
        this.currentRunTotalSteps = 0;
        this.currentRunStepsCompleted = 0;
        if (this.history.length > 0) {
            this.applyHistoryIndex(this.history.length - 1);
        }
    }

    finalizeActiveReferenceRun() {
        if (!this.activeReferenceRunSpec) return null;

        const result = buildBenchmarkRunResult({
            spec: this.activeReferenceRunSpec,
            rateHistory: this.rateHistory,
            history: this.history,
            finalSnapshot: this.captureCurrentFinalSnapshot(),
        });

        this.referenceRunResults = resolveBenchmarkReferenceComparisons([
            ...this.referenceRunResults,
            result,
        ]);
        this.referenceRunsCompleted += 1;
        this.activeReferenceRunSpec = null;
        return result;
    }

    startQueuedReferenceRun(): boolean {
        if (!this.simWorker) return false;

        const [nextSpec, ...remaining] = this.referenceRunQueue;
        if (!nextSpec) return false;

        this.referenceRunQueue = remaining;
        this.activeReferenceRunSpec = nextSpec;
        this.referenceSweepError = '';

        const savedSteps = this.#params.steps;
        const savedDeltaTDays = this.#params.delta_t_days;
        // Apply case params to parameter store only (no runtime reset needed — we create fresh below).
        this.#params.applyParamValues(nextSpec.params as Record<string, any>);
        this.#params.steps = savedSteps;
        this.#params.delta_t_days = savedDeltaTDays;

        // Reset display state for the new reference run.
        this.stopPlaying();
        this.history = [];
        this.currentIndex = -1;
        this.gridStateRaw = null;
        this.wellStateRaw = null;
        this.rateHistory = [];
        this.runCompleted = false;
        this.simTime = 0;
        this.modelNeedsReinit = false;
        this.modelReinitNotice = '';
        this.pendingAutoReinit = false;
        this.runtimeError = '';
        this.convergenceHitCount = 0;
        this.convergenceHitCaseName = nextSpec.label;
        this.latestStepSolverWarning = '';
        this.refreshConvergenceWarningDisplay();
        this.currentRunTotalSteps = nextSpec.steps;
        this.currentRunStepsCompleted = 0;

        // $state.snapshot() strips Svelte 5 reactive proxies so the payload
        // can be structured-cloned by postMessage (pvtTable, cellDzPerLayer, etc.).
        const rawParams = $state.snapshot(nextSpec.params) as Record<string, any>;
        const payload = buildBenchmarkCreatePayload({
            ...rawParams,
            terminationPolicy: nextSpec.terminationPolicy ?? undefined,
        });
        this.lastCreateSignature = JSON.stringify(payload);
        this.simWorker.postMessage({ type: 'create', payload });
        this.simWorker.postMessage({
            type: 'run',
            payload: {
                steps: nextSpec.steps,
                deltaTDays: nextSpec.deltaTDays,
                historyInterval: nextSpec.historyInterval,
                chunkYieldInterval: 1,
            },
        });

        return true;
    }

    runReferenceSpecs(specs: ReferenceRunSpec[]): boolean {
        if (!this.#nav?.activeReferenceFamily) {
            this.runtimeError = 'Reference runs are only available when a library reference case is active.';
            return false;
        }
        if (!this.wasmReady || !this.simWorker) {
            this.runtimeError = 'WASM not ready yet.';
            return false;
        }
        if (this.workerRunning || this.referenceSweepRunning) return false;
        if (!Array.isArray(specs) || specs.length === 0) {
            this.runtimeError = 'No reference runs are available for the selected family.';
            return false;
        }

        this.clearReferenceRunnerState(true);
        this.referenceSweepRunning = true;
        this.referenceTotalRuns = specs.length;
        this.referenceRunQueue = [...specs];
        this.runtimeError = '';
        this.runtimeWarning = '';
        this.referenceConvergenceWarnings = [];
        return this.startQueuedReferenceRun();
    }

    runActiveReferenceBase(): boolean {
        const family = this.#nav?.activeReferenceFamily ?? null;
        if (!family) {
            this.runtimeError = 'Select a library reference before running the reference set.';
            return false;
        }
        return this.runReferenceSpecs(buildBenchmarkRunSpecs(family));
    }

    runActiveReferenceSensitivityAxis(axis: BenchmarkSensitivityAxisKey): boolean {
        const family = this.#nav?.activeReferenceFamily ?? null;
        if (!family) {
            this.runtimeError = 'Select a library reference before running a sensitivity set.';
            return false;
        }

        const variants = getBenchmarkVariantsForFamily(family.key).filter(
            (variant) => variant.axis === axis,
        );
        if (variants.length === 0) {
            this.runtimeError = `No ${axis} variants are available for the selected reference family.`;
            return false;
        }

        return this.runReferenceSpecs(buildBenchmarkRunSpecs(family, variants));
    }

    runActiveReferenceSelection(variantKeys: string[] = []): boolean {
        const family = this.#nav?.activeReferenceFamily ?? null;
        if (!family) {
            this.runtimeError = 'Select a library reference before running the reference set.';
            return false;
        }

        if (!Array.isArray(variantKeys) || variantKeys.length === 0) {
            return this.runActiveReferenceBase();
        }

        const variantMap = new Map(
            getBenchmarkVariantsForFamily(family.key).map((variant) => [variant.variantKey, variant]),
        );
        const selectedVariants = variantKeys
            .map((variantKey) => variantMap.get(variantKey) ?? null)
            .filter((variant): variant is NonNullable<typeof variant> => Boolean(variant));

        if (selectedVariants.length === 0) {
            this.runtimeError = 'Select at least one sensitivity variant before running the reference set.';
            return false;
        }

        return this.runReferenceSpecs(buildBenchmarkRunSpecs(family, selectedVariants));
    }

    // ===== Playback Controls =====

    stopPlaying() {
        this.playing = false;
        if (this.playTimer) {
            clearInterval(this.playTimer);
            this.playTimer = null;
        }
    }

    applyHistoryIndex(idx: number) {
        if (idx < 0 || idx >= this.history.length) return;
        this.currentIndex = idx;
        const entry = this.history[idx];
        this.gridStateRaw = entry.grid;
        this.wellStateRaw = entry.wells;
        this.simTime = entry.time;
    }

    play() {
        if (this.history.length === 0) return;
        if (this.playTimer) { clearInterval(this.playTimer); this.playTimer = null; }
        this.playing = true;
        this.playTimer = setInterval(() => {
            this.next();
            if (this.currentIndex >= this.history.length - 1) this.stopPlaying();
        }, 1000 / this.playSpeed);
    }

    togglePlay() {
        if (this.playing) this.stopPlaying();
        else this.play();
    }

    next() {
        if (this.history.length === 0) return;
        this.currentIndex = Math.min(this.history.length - 1, this.currentIndex + 1);
        this.applyHistoryIndex(this.currentIndex);
    }

    prev() {
        if (this.history.length === 0) return;
        this.currentIndex = Math.max(0, this.currentIndex - 1);
        this.applyHistoryIndex(this.currentIndex);
    }

    // ===== Simulation State Management =====

    resetSimulationState(options: {
        clearErrors?: boolean; clearWarnings?: boolean; resetProfile?: boolean; bumpViz?: boolean;
    } = {}) {
        const { clearErrors = false, clearWarnings = false, resetProfile = false, bumpViz = false } = options;
        this.stopPlaying();
        this.history = [];
        this.currentIndex = -1;
        this.gridStateRaw = null;
        this.wellStateRaw = null;
        this.simTime = 0;
        this.rateHistory = [];
        this.runCompleted = false;
        if (resetProfile) this.profileStats = { ...EMPTY_PROFILE_STATS };
        if (clearErrors) this.runtimeError = '';
        if (clearWarnings) this.runtimeWarning = '';
        if (bumpViz) this.vizRevision += 1;
    }

    resetModelAndVisualizationState(stopWorker = true, showReinitNotice = false) {
        this.stopPlaying();
        if (stopWorker && this.simWorker && this.workerRunning) {
            this.simWorker.postMessage({ type: 'stop' });
        }
        this.history = [];
        this.currentIndex = -1;
        this.gridStateRaw = null;
        this.wellStateRaw = null;
        this.rateHistory = [];
        this.runCompleted = false;
        this.simTime = 0;
        this.runtimeWarning = '';
        this.runtimeError = '';
        if (showReinitNotice) {
            this.modelNeedsReinit = true;
            this.modelReinitNotice = 'Model reset required after input changes.';
        }
        this.vizRevision += 1;
    }

    pushHistoryEntry(entry: HistoryEntry) {
        this.history = [...this.history, entry];
        if (this.history.length > MAX_HISTORY_ENTRIES) {
            const overflow = this.history.length - MAX_HISTORY_ENTRIES;
            this.history = this.history.slice(overflow);
            this.currentIndex = Math.max(0, this.currentIndex - overflow);
        }
        this.currentIndex = this.history.length - 1;
    }

    updateProfileStats(profile: Partial<WorkerProfile> = {}, renderApplyMs = 0) {
        this.profileStats = {
            batchMs: Number(profile.batchMs ?? this.profileStats.batchMs ?? 0),
            avgStepMs: Number(profile.avgStepMs ?? profile.simStepMs ?? this.profileStats.avgStepMs ?? 0),
            extractMs: Number(profile.extractMs ?? this.profileStats.extractMs ?? 0),
            renderApplyMs,
            snapshotsSent: Number(profile.snapshotsSent ?? this.profileStats.snapshotsSent ?? 0),
        };
    }

    applyWorkerState(message: SimulatorSnapshot) {
        const renderStart = performance.now();
        this.gridStateRaw = message.grid;
        this.wellStateRaw = message.wells;
        this.simTime = message.time;
        if (Array.isArray(message.rateHistory)) {
            this.rateHistory = message.rateHistory;
        } else if (Array.isArray(message.rateHistoryDelta) && message.rateHistoryDelta.length > 0) {
            this.rateHistory = [...this.rateHistory, ...message.rateHistoryDelta];
        }
        const newSolverWarning = message.solverWarning || '';
        if (newSolverWarning && !this.latestStepSolverWarning) {
            // new convergence hit (rising edge)
            this.convergenceHitCount += 1;
        }
        this.latestStepSolverWarning = newSolverWarning;
        this.refreshConvergenceWarningDisplay({ includeActiveReferenceCase: true });
        if (message.recordHistory) {
            this.pushHistoryEntry({ time: message.time, grid: message.grid, wells: message.wells });
        }
        if (message.stepIndex !== undefined && this.currentRunTotalSteps > 0) {
            this.currentRunStepsCompleted = message.stepIndex + 1;
        }
        this.updateProfileStats(message.profile, performance.now() - renderStart);
    }

    // ===== Worker Communication =====

    handleWorkerMessage(event: MessageEvent<WorkerMessage>) {
        const message = event.data;
        if (!message) return;

        if (message.type === 'ready') {
            this.wasmReady = true;
            this.initSimulator({ silent: true });
            return;
        }
        if (message.type === 'runStarted') {
            this.runtimeError = '';
            this.workerRunning = true;
            return;
        }
        if (message.type === 'state') {
            this.applyWorkerState(message as unknown as SimulatorSnapshot);
            return;
        }
        if (message.type === 'batchComplete') {
            this.workerRunning = false;
            this.stopPending = false;
            this.runCompleted = true;
            this.currentRunTotalSteps = 0;
            this.currentRunStepsCompleted = 0;
            this.updateProfileStats(message.profile, this.profileStats.renderApplyMs);
            this.applyHistoryIndex(this.history.length - 1);
            if (this.referenceSweepRunning && this.activeReferenceRunSpec) {
                if (this.convergenceHitCount > 0) {
                    this.referenceConvergenceWarnings = [
                        ...this.referenceConvergenceWarnings,
                        this.formatConvergenceCaseWarning(this.convergenceHitCaseName, this.convergenceHitCount),
                    ];
                }
                this.latestStepSolverWarning = '';
                this.finalizeActiveReferenceRun();
                if (this.referenceRunQueue.length > 0) {
                    this.refreshConvergenceWarningDisplay();
                    this.startQueuedReferenceRun();
                } else {
                    this.referenceSweepRunning = false;
                    this.#nav?.restoreActiveReferenceBaseDisplay();
                    this.refreshConvergenceWarningDisplay();
                    this.runtimeWarning = '';
                }
                return;
            }
            if (this.pendingAutoReinit) {
                this.pendingAutoReinit = false;
                const reinitialized = this.initSimulator({ silent: true });
                this.latestStepSolverWarning = '';
                this.solverWarning = '';
                this.runtimeWarning = reinitialized
                    ? 'Inputs changed. Model reset to step 0.'
                    : this.runtimeWarning;
            } else {
                this.latestStepSolverWarning = '';
                this.refreshConvergenceWarningDisplay();
                if (message.terminationSummary) {
                    this.runtimeWarning = String(message.terminationSummary);
                }
            }
            return;
        }
        if (message.type === 'stopped') {
            this.workerRunning = false;
            this.stopPending = false;
            this.runCompleted = true;
            if (this.referenceSweepRunning || this.activeReferenceRunSpec) {
                this.referenceSweepRunning = false;
                this.referenceRunQueue = [];
                this.activeReferenceRunSpec = null;
                this.currentRunTotalSteps = 0;
                this.currentRunStepsCompleted = 0;
                if ('profile' in message && message.profile) {
                    this.updateProfileStats(message.profile, this.profileStats.renderApplyMs);
                }
                this.#nav?.restoreActiveReferenceBaseDisplay();
                if (this.convergenceHitCount > 0) {
                    this.referenceConvergenceWarnings = [
                        ...this.referenceConvergenceWarnings,
                        this.formatConvergenceCaseWarning(this.convergenceHitCaseName, this.convergenceHitCount, true),
                    ];
                }
                this.latestStepSolverWarning = '';
                this.refreshConvergenceWarningDisplay();
                this.runtimeWarning = `Reference run set stopped after ${this.referenceRunsCompleted} completed run(s).`;
                return;
            }
            if (this.pendingAutoReinit) {
                this.pendingAutoReinit = false;
                const reinitialized = this.initSimulator({ silent: true });
                this.latestStepSolverWarning = '';
                this.solverWarning = '';
                this.runtimeWarning = reinitialized
                    ? 'Inputs changed during the run. Model reset to step 0.'
                    : this.runtimeWarning;
                return;
            }
            this.latestStepSolverWarning = '';
            this.refreshConvergenceWarningDisplay();
            this.runtimeWarning = message.reason === 'user'
                ? `Simulation stopped after ${Number(message.completedSteps ?? 0)} step(s).`
                : 'No running simulation to stop.';
            this.currentRunTotalSteps = 0;
            this.currentRunStepsCompleted = 0;
            if ('profile' in message && message.profile) {
                this.updateProfileStats(message.profile, this.profileStats.renderApplyMs);
            }
            this.applyHistoryIndex(this.history.length - 1);
            return;
        }
        if (message.type === 'warning') {
            this.runtimeWarning = String(message.message ?? 'Simulation warning');
            return;
        }
        if (message.type === 'error') {
            this.workerRunning = false;
            this.stopPending = false;
            console.error('Simulation worker error:', message.message);
            this.runtimeError = String(message.message ?? 'Simulation error');
            if (this.referenceSweepRunning || this.activeReferenceRunSpec) {
                this.referenceSweepError = this.runtimeError;
                this.referenceSweepRunning = false;
                this.referenceRunQueue = [];
                this.activeReferenceRunSpec = null;
                this.#nav?.restoreActiveReferenceBaseDisplay();
            }
            if (this.pendingAutoReinit) this.pendingAutoReinit = false;
        }
    }

    setupWorker() {
        this.simWorker = new Worker(
            new URL('../workers/sim.worker.ts', import.meta.url),
            { type: 'module' },
        );
        this.simWorker.onmessage = (event) => this.handleWorkerMessage(event);
        this.simWorker.onerror = (event) => {
            this.workerRunning = false;
            this.runtimeError = `Worker error: ${event.message || 'Unknown worker failure'}`;
        };
        this.simWorker.onmessageerror = () => {
            this.workerRunning = false;
            this.runtimeError = 'Worker message deserialization failed. Reset the model and retry.';
        };
        this.simWorker.postMessage({ type: 'init' });
    }

    dispose() {
        this.stopPlaying();
        if (this.simWorker) {
            this.simWorker.postMessage({ type: 'dispose' });
            this.simWorker.terminate();
            this.simWorker = null;
        }
    }

    // ===== Simulator Init / Run =====

    /**
     * Build the full simulator create payload: core params + scenario-specific
     * sweep config and termination policy from the active scenario.
     */
    buildCreatePayload() {
        const payload = this.#params.buildCorePayload();
        const sc = this.#nav?.activeScenarioObject ?? null;
        const isCustomMode = this.#nav?.isCustomMode ?? false;
        if (sc && !isCustomMode) {
            const resolved = resolveCapabilities(sc.capabilities);
            if (resolved.showSweepPanel && resolved.sweepGeometry) {
                const sw0 = payload.initialSaturation;
                const movable = Math.max(0, 1 - payload.s_wc - payload.s_or);
                payload.sweepConfig = {
                    geometry: resolved.sweepGeometry,
                    // 20% of movable range above connate — swept cells, not artefacts
                    swept_threshold: payload.s_wc + 0.2 * movable,
                    initial_oil_saturation: Math.max(0, 1 - sw0),
                    residual_oil_saturation: payload.s_or,
                };
            }
            payload.terminationPolicy = cloneTerminationPolicy(sc.terminationPolicy);
        }
        return payload;
    }

    initSimulator(options: { runAfterInit?: boolean; silent?: boolean } = {}): boolean {
        const { runAfterInit = false, silent = false } = options;
        if (!this.wasmReady || !this.simWorker) {
            if (!silent) this.runtimeError = 'WASM not ready yet.';
            return false;
        }
        const validWellLocations =
            Number.isInteger(this.#params.injectorI) && Number.isInteger(this.#params.injectorJ) &&
            Number.isInteger(this.#params.producerI) && Number.isInteger(this.#params.producerJ) &&
            this.#params.injectorI >= 0 && this.#params.injectorI < this.#params.nx &&
            this.#params.injectorJ >= 0 && this.#params.injectorJ < this.#params.ny &&
            this.#params.producerI >= 0 && this.#params.producerI < this.#params.nx &&
            this.#params.producerJ >= 0 && this.#params.producerJ < this.#params.ny;
        if (!validWellLocations) {
            if (!silent) this.runtimeError = 'Invalid well location.';
            return false;
        }
        if (this.#params.hasValidationErrors) {
            if (!silent) this.runtimeError = 'Input validation failed.';
            return false;
        }
        const switchedToCustomSubCase = this.#nav?.maybySwitchToCustomSubCaseOnReinit() ?? false;
        this.history = [];
        this.currentIndex = -1;
        this.runCompleted = false;
        this.modelNeedsReinit = false;
        this.modelReinitNotice = '';
        this.runtimeError = '';
        this.runtimeWarning = '';
        if (switchedToCustomSubCase) {
            this.runtimeWarning = 'Library case modified. Reset as a custom case.';
        }
        this.vizRevision += 1;
        const payload = this.buildCreatePayload();
        this.simWorker.postMessage({ type: 'create', payload });
        this.lastCreateSignature = JSON.stringify(payload);
        this.pendingAutoReinit = false;
        if (runAfterInit) this.runSteps();
        return true;
    }

    runSimulationBatch(batchSteps: number, batchHistoryInterval: number) {
        if (this.modelNeedsReinit) {
            const initialized = this.initSimulator({ silent: true });
            if (!initialized) return;
        }

        if (!this.simWorker || this.workerRunning) return;
        if (this.#params.hasValidationErrors) {
            this.runtimeError = 'Input validation failed. Resolve highlighted fields before running.';
            return;
        }
        this.workerRunning = true;
        this.currentRunTotalSteps = batchSteps;
        this.currentRunStepsCompleted = 0;
        this.runtimeError = '';
        this.referenceConvergenceWarnings = [];
        this.convergenceHitCount = 0;
        this.convergenceHitCaseName = '';
        this.latestStepSolverWarning = '';
        this.refreshConvergenceWarningDisplay();
        this.runtimeWarning = this.longRunEstimate
            ? `Estimated run: ${this.estimatedRunSeconds.toFixed(1)}s. You can stop at any time.`
            : this.runtimeWarning;
        this.simWorker.postMessage({
            type: 'run',
            payload: {
                steps: batchSteps,
                deltaTDays: this.#params.delta_t_days,
                historyInterval: batchHistoryInterval,
                chunkYieldInterval: 1,
            },
        });
    }

    stepOnce() { this.runSimulationBatch(1, 1); }

    runSteps() {
        this.runSimulationBatch(
            this.#params.steps,
            this.#params.userHistoryInterval ?? this.#params.defaultHistoryInterval,
        );
    }

    stopRun() {
        if (this.simWorker) {
            this.stopPending = true;
            this.simWorker.postMessage({ type: 'stop' });
        }
    }

    // ===== Config Diff Detection =====

    checkConfigDiff() {
        const nextSignature = this.#params.buildModelResetKey();
        if (!this.modelConfigSignature) { this.modelConfigSignature = nextSignature; return; }
        if (nextSignature === this.modelConfigSignature) return;
        this.modelConfigSignature = nextSignature;
        if (this.#params.skipNextAutoModelReset) { this.#params.skipNextAutoModelReset = false; return; }
        if (this.wasmReady && this.simWorker && (this.#nav?.isModified ?? false) && this.lastCreateSignature) {
            this.resetSimulationState({ clearErrors: true, clearWarnings: false, resetProfile: true, bumpViz: true });
            if (this.workerRunning) {
                this.pendingAutoReinit = true;
                this.runtimeWarning = 'Inputs changed during the run. Stopping and resetting…';
                this.stopRun();
            } else {
                const reinitialized = this.initSimulator({ silent: true });
                this.runtimeWarning = reinitialized ? 'Inputs changed. Model reset to step 0.' : this.runtimeWarning;
            }
            return;
        }
        this.resetModelAndVisualizationState(true, true);
    }

    // ===== Scenario Sweep =====

    buildScenarioSweepSpecs(
        scenarioKey: string,
        dimensionKey: string,
        variantKeys: string[],
    ): import('../benchmarkRunModel').BenchmarkRunSpec[] {
        const scenario = getScenario(scenarioKey);
        if (!scenario) return [];

        const dimension = scenario.sensitivities.find((d) => d.key === dimensionKey);
        if (!dimension) {
            if (import.meta.env.DEV) {
                console.warn(`[store] buildScenarioSweepSpecs: unknown dimensionKey "${dimensionKey}" for scenario "${scenarioKey}"`);
            }
            return [];
        }

        const analyticalMethod = scenario.capabilities.analyticalMethod;
        const baseParams = scenario.params;
        const analyticalRef = {
            kind: 'analytical' as const,
            source: analyticalMethod === 'digitized-reference' ? `${scenarioKey}:digitized-reference` : `${scenarioKey}:analytical`,
        };

        const specs: import('../benchmarkRunModel').BenchmarkRunSpec[] = [];

        for (const variantKey of variantKeys) {
            const variant = dimension.variants.find((v) => v.key === variantKey);
            if (!variant) {
                if (import.meta.env.DEV) {
                    console.warn(`[store] buildScenarioSweepSpecs: unknown variantKey "${variantKey}" in dimension "${dimensionKey}"`);
                }
                continue;
            }
            const variantParams = getScenarioWithVariantParams(scenarioKey, dimensionKey, variantKey);
            const runSteps = this.#params.hasUserStepsOverride
                ? Math.max(1, Math.round(Number(this.#params.steps ?? baseParams.steps ?? 240)))
                : Math.max(1, Math.round(Number(variantParams.steps ?? baseParams.steps ?? 240)));
            const runDeltaTDays = this.#params.hasUserDeltaTDaysOverride
                ? Number(this.#params.delta_t_days ?? baseParams.delta_t_days ?? 0.125)
                : Number(variantParams.delta_t_days ?? baseParams.delta_t_days ?? 0.125);
            specs.push({
                key: `${scenarioKey}__${dimensionKey}__${variantKey}`,
                caseKey: scenarioKey,
                familyKey: scenarioKey,
                analyticalMethod,
                variantKey,
                variantLabel: variant.label,
                label: `${scenario.label} — ${variant.label}`,
                description: variant.description,
                params: variantParams,
                steps: runSteps,
                deltaTDays: runDeltaTDays,
                historyInterval: Math.max(1, Math.ceil(runSteps / 25)),
                reference: analyticalRef,
                comparisonMetric: null,
                breakthroughCriterion: null,
                terminationPolicy: cloneTerminationPolicy(scenario.terminationPolicy) ?? null,
                comparisonMeaning: variant.description,
            });
        }

        return specs;
    }

    runScenarioSweep(scenarioKey: string, dimensionKey: string, variantKeys: string[]): boolean {
        if (!this.wasmReady || !this.simWorker) {
            this.runtimeError = 'WASM not ready yet.';
            return false;
        }
        if (this.workerRunning || this.referenceSweepRunning) return false;

        const specs = this.buildScenarioSweepSpecs(scenarioKey, dimensionKey, variantKeys);
        if (specs.length === 0) return false;

        this.clearReferenceRunnerState(true);
        this.referenceSweepRunning = true;
        this.referenceTotalRuns = specs.length;
        this.referenceRunQueue = [...specs];
        this.runtimeError = '';
        this.runtimeWarning = '';
        return this.startQueuedReferenceRun();
    }
}

// ---------- Factory ----------

export function createRuntimeStore(params: ParameterStore) {
    return new RuntimeStoreImpl(params);
}

export type RuntimeStore = InstanceType<typeof RuntimeStoreImpl>;
