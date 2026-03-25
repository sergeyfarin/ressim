import type {
    WorkerMessage,
    SimulatorSnapshot,
    RateHistoryPoint,
    SimulatorCreatePayload,
    AnalyticalProductionPoint,
    GridState,
    WellState,
    PvtRow,
    ThreePhaseScalTables,
} from '../simulator-types';
import {
    catalog,
    buildCaseKey,
    composeCaseParams,
    getCaseLibraryEntry,
    resolveCaseLibraryEntryFromScenario,
    getBenchmarkEntry,
    getBenchmarkFamily,
    getBenchmarkVariantsForFamily,
    type BenchmarkSensitivityAxisKey,
    getDefaultToggles,
    getDisabledOptions,
    stabilizeToggleState,
    type CaseMode,
    type ToggleState
} from '../catalog/caseCatalog';
import { buildCreatePayloadFromState } from '../buildCreatePayload';
import { generateBlackOilTable } from '../physics/pvt';
import {
    buildBenchmarkCreatePayload,
    buildBenchmarkRunResult,
    buildBenchmarkRunSpecs,
    resolveBenchmarkReferenceComparisons,
    type BenchmarkRunResult as ReferenceRunResult,
    type BenchmarkRunSpec as ReferenceRunSpec,
} from '../benchmarkRunModel';
import {
    validateInputs as validateSimulationInputs,
    type SimulationInputs,
    type ValidationState as InputValidationState,
    type ValidationWarning,
} from '../validateInputs';
import { buildWarningPolicy, evaluateAnalyticalStatus, type AnalyticalStatus } from '../warningPolicy';
import { getDefaultScenarioAnalyticalMode, getScenario, getScenarioChartLayout, getScenarioWithVariantParams, getDefaultVariantKeys, resolveCapabilities, suppressesPrimaryAnalyticalOverlays, type ScenarioAnalyticalOption } from '../catalog/scenarios';
import {
    buildReferenceCloneProvenance,
    buildBasePresetProfile,
    buildComparisonSelection,
    buildOverrideResetPlan,
    buildParameterOverrides,
    groupParameterOverrides,
    shouldAllowReferenceClone,
    shouldAutoClearModifiedState,
    resolveProductFamily,
    resolveScenarioSource,
    buildScenarioEditabilityPolicy,
    type ScenarioNavigationState,
    type ReferenceProvenance,
    type ComparisonSelection,
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

function defaultProducerJForGrid(ny: number): number {
    return Math.max(0, ny - 1);
}

function clonePvtTable(rows: unknown): PvtRow[] | undefined {
    if (!Array.isArray(rows)) return undefined;
    return rows.map((row) => ({ ...(row as PvtRow) }));
}

function cloneScalTables(value: unknown): ThreePhaseScalTables | undefined {
    if (!value || typeof value !== 'object') return undefined;
    const tables = value as ThreePhaseScalTables;
    if (!Array.isArray(tables.swof) || !Array.isArray(tables.sgof)) return undefined;
    return {
        swof: tables.swof.map((row) => ({ ...row })),
        sgof: tables.sgof.map((row) => ({ ...row })),
    };
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

class SimulationStoreImpl {

    // ===== $state: Simulation Input Parameters =====

    nx = $state(15);
    ny = $state(10);
    nz = $state(10);
    cellDx = $state(10);
    cellDy = $state(10);
    cellDz = $state(1);
    delta_t_days = $state(0.25);
    steps = $state(20);
    hasUserDeltaTDaysOverride = $state(false);
    hasUserStepsOverride = $state(false);

    // Initial Conditions
    initialPressure = $state(300.0);
    initialSaturation = $state(0.3);
    reservoirPorosity = $state(0.2);

    // Fluid properties
    pvtMode: 'constant' | 'black-oil' = $state('constant');
    apiGravity = $state(30.0);
    gasSpecificGravity = $state(0.7);
    reservoirTemperature = $state(80.0);
    bubblePoint = $state(150.0);

    mu_w = $state(0.5);
    mu_o = $state(1.0);
    c_o = $state(1e-5);
    c_w = $state(3e-6);
    rock_compressibility = $state(1e-6);
    depth_reference = $state(0.0);
    volume_expansion_o = $state(1.0);
    volume_expansion_w = $state(1.0);
    rho_w = $state(1000.0);
    rho_o = $state(800.0);

    // Permeability
    permMode: 'uniform' | 'random' | 'perLayer' = $state('uniform');
    uniformPermX = $state(100.0);
    uniformPermY = $state(100.0);
    uniformPermZ = $state(10.0);
    minPerm = $state(50.0);
    maxPerm = $state(200.0);
    useRandomSeed = $state(true);
    randomSeed = $state(12345);
    layerPermsX: number[] = $state([100, 150, 50, 200, 120, 1000, 90, 110, 130, 70]);
    layerPermsY: number[] = $state([100, 150, 50, 200, 120, 1000, 90, 110, 130, 70]);
    layerPermsZ: number[] = $state([10, 15, 5, 20, 12, 8, 9, 11, 13, 7]);

    // Relative Permeability / Capillary
    s_wc = $state(0.1);
    s_or = $state(0.1);
    n_w = $state(2.0);
    n_o = $state(2.0);
    k_rw_max = $state(1.0);
    k_ro_max = $state(1.0);

    // Well inputs
    well_radius = $state(0.1);
    well_skin = $state(0.0);
    injectorBhp = $state(500.0);
    producerBhp = $state(100.0);
    injectorControlMode: 'rate' | 'pressure' = $state('pressure');
    producerControlMode: 'rate' | 'pressure' = $state('pressure');
    injectorEnabled = $state(true);
    targetInjectorRate = $state(350.0);
    targetProducerRate = $state(350.0);
    targetInjectorSurfaceRate = $state<number | null>(null);
    targetProducerSurfaceRate = $state<number | null>(null);
    injectorI = $state(0);
    injectorJ = $state(0);
    producerI = $state(14);
    producerJ = $state(9);

    // Stability
    max_sat_change_per_step = $state(0.1);
    max_pressure_change_per_step = $state(75.0);
    max_well_rate_change_fraction = $state(0.75);
    gravityEnabled = $state(false);
    capillaryEnabled = $state(true);
    capillaryPEntry = $state(5.0);
    capillaryLambda = $state(2.0);

    // Three-phase
    s_gc = $state(0.05);
    s_gr = $state(0.05);
    s_org = $state(0.15);
    n_g = $state(1.5);
    k_rg_max = $state(1.0);
    pcogEnabled = $state(false);
    pcogPEntry = $state(3.0);
    pcogLambda = $state(2.0);
    mu_g = $state(0.02);
    c_g = $state(1e-4);
    rho_g = $state(10.0);
    threePhaseModeEnabled = $state(false);
    injectedFluid = $state<'water' | 'gas'>('gas');
    initialGasSaturation = $state(0.0);
    gasRedissolutionEnabled = $state(true);
    /** Initial dissolved-gas ratio override [Sm³/Sm³]; undefined = use saturated curve */
    initialRs = $state<number | undefined>(undefined);

    // Analytical solution
    analyticalMode: 'waterflood' | 'depletion' | 'none' = $state('depletion');
    analyticalDepletionRateScale = $state(1.0);
    analyticalArpsB = $state(0.0);
    pvtTableOverride = $state<PvtRow[] | undefined>(undefined);
    scalTables = $state<ThreePhaseScalTables | undefined>(undefined);

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
    analyticalProductionData: AnalyticalProductionPoint[] = $state([]);
    analyticalMeta: {
        mode: 'waterflood' | 'depletion' | 'none';
        shapeFactor: number | null;
        shapeLabel: string;
    } = $state({ mode: 'waterflood', shapeFactor: null, shapeLabel: '' });
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
    skipNextAutoModelReset = $state(false);

    // History / replay
    history = $state<HistoryEntry[]>([]);
    currentIndex = $state(-1);
    playing = $state(false);
    playSpeed = $state(2);
    playTimer: ReturnType<typeof setInterval> | null = null;
    userHistoryInterval = $state<number | null>(null);

    // Profile
    profileStats: ProfileStats = $state({ ...EMPTY_PROFILE_STATS });
    lastCreateSignature = $state('');
    baseCaseSignature = $state('');
    pendingAutoReinit = $state(false);

    // ===== $state: Navigation =====

    activeMode = $state<CaseMode>('dep');
    activeCase = $state('');
    isModified = $state(false);
    toggles = $state<ToggleState>(getDefaultToggles('dep'));
    referenceProvenance: ReferenceProvenance | null = $state(null);
    activeComparisonSelection = $state<ComparisonSelection>(buildComparisonSelection());
    explicitLibraryEntryKey: string | null = $state(null);

    // Scenario-picker state
    activeScenarioKey: string | null = $state(null);
    activeSensitivityDimensionKey: string | null = $state(null);
    activeAnalyticalOptionKey: string | null = $state(null);
    activeVariantKeys: string[] = $state([]);
    isCustomMode = $state(false);

    // ===== $derived =====

    pvtTable = $derived.by(() => {
        if (this.pvtMode === 'black-oil') {
            if (this.pvtTableOverride?.length) {
                return this.pvtTableOverride.map((row) => ({ ...row }));
            }
            const pMax = Math.max(this.initialPressure * 1.5, Math.max(this.injectorBhp, this.producerBhp) * 1.5, 500);
            return generateBlackOilTable(this.apiGravity, this.gasSpecificGravity, this.reservoirTemperature, this.bubblePoint, pMax);
        }
        return undefined;
    });

    defaultHistoryInterval = $derived(Math.max(1, Math.ceil(this.steps / 25)));

    ooipM3 = $derived(
        this.nx * this.ny * this.nz * this.cellDx * this.cellDy * this.cellDz *
        this.reservoirPorosity * Math.max(0, 1 - this.initialSaturation),
    );

    poreVolumeM3 = $derived(
        this.nx * this.ny * this.nz * this.cellDx * this.cellDy * this.cellDz * this.reservoirPorosity,
    );

    rateControlledWells = $derived.by(
        () => this.injectorControlMode === 'rate' && this.producerControlMode === 'rate',
    );

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

    disabledOptions = $derived(getDisabledOptions(this.toggles));

    // ===== $derived: Validation =====

    validationState: InputValidationState = $derived.by(
        () => validateSimulationInputs(this.buildValidationInput()),
    );

    validationErrors: Record<string, string> = $derived(this.validationState.errors);
    validationWarnings: ValidationWarning[] = $derived(this.validationState.warnings);
    hasValidationErrors = $derived(Object.keys(this.validationErrors).length > 0);

    estimatedRunSeconds = $derived(
        Math.max(0, (Number(this.profileStats.avgStepMs || 0) * Number(this.steps || 0)) / 1000),
    );
    longRunEstimate = $derived(this.estimatedRunSeconds > 10);

    // ===== $derived: Navigation / Library =====

    activeLibraryEntry = $derived.by(() => {
        if (this.isModified) return null;

        if (this.explicitLibraryEntryKey) {
            return getCaseLibraryEntry(this.explicitLibraryEntryKey);
        }

        return resolveCaseLibraryEntryFromScenario({
            activeMode: this.activeMode,
            benchmarkId: this.toggles.benchmarkId ?? null,
            scenarioParams: composeCaseParams(this.toggles),
        });
    });

    activeReferenceFamily = $derived.by(() => {
        const benchmarkFamilyKey = this.activeLibraryEntry?.benchmarkFamilyKey ?? null;
        return benchmarkFamilyKey ? getBenchmarkFamily(benchmarkFamilyKey) : null;
    });

    activeNavigationLibraryEntry = $derived.by(() => {
        if (this.activeLibraryEntry) return this.activeLibraryEntry;
        if (this.referenceProvenance?.sourceCaseKey) {
            return getCaseLibraryEntry(this.referenceProvenance.sourceCaseKey);
        }
        return null;
    });

    basePreset = $derived.by(() => {
        const benchmarkId = this.activeReferenceFamily?.key ?? null;
        const benchmarkLabel = this.activeLibraryEntry?.label
            ?? (benchmarkId ? getBenchmarkEntry(benchmarkId)?.label ?? null : null);

        return buildBasePresetProfile({
            key: this.activeCase,
            mode: this.activeMode,
            toggles: this.toggles,
            isModified: this.isModified,
            benchmarkId,
            benchmarkLabel,
            benchmarkAnalyticalMethod: this.activeReferenceFamily?.analyticalMethod ?? null,
            activeLibraryCaseKey: this.activeLibraryEntry?.key ?? null,
            activeLibraryGroup: this.activeLibraryEntry?.group ?? null,
        });
    });

    navigationState = $derived.by((): ScenarioNavigationState => {
        const benchmarkId = this.activeReferenceFamily?.key ?? null;
        const activeSource = resolveScenarioSource({ isModified: this.isModified });
        const activeLibraryGroup = activeSource === 'custom' ? null : (this.activeLibraryEntry?.group ?? null);

        return {
            activeFamily: resolveProductFamily({
                activeMode: this.activeMode,
                activeLibraryFamily: this.activeNavigationLibraryEntry?.family ?? null,
                benchmarkAnalyticalMethod: this.activeReferenceFamily?.analyticalMethod ?? null,
                benchmarkId,
            }),
            activeSource,
            activeLibraryCaseKey: activeSource === 'custom' ? null : (this.activeLibraryEntry?.key ?? null),
            activeLibraryGroup,
            sourceLabel: activeSource === 'custom' ? null : (this.activeLibraryEntry?.sourceLabel ?? null),
            referenceSourceLabel: activeSource === 'custom' ? null : (this.activeLibraryEntry?.referenceSourceLabel ?? null),
            provenanceSummary: activeSource === 'custom' ? null : (this.activeLibraryEntry?.provenanceSummary ?? null),
            activeComparisonSelection: buildComparisonSelection(this.activeComparisonSelection),
            editabilityPolicy: buildScenarioEditabilityPolicy({
                caseSource: activeSource,
                activeLibraryGroup,
            }),
        };
    });

    parameterOverrides = $derived.by(() => {
        return buildParameterOverrides({
            currentParams: this.buildCurrentParameterSnapshot(),
            baseParams: composeCaseParams(this.toggles),
        });
    });

    parameterOverrideGroups = $derived(groupParameterOverrides(this.parameterOverrides));
    parameterOverrideCount = $derived(Object.keys(this.parameterOverrides).length);

    analyticalStatus = $derived.by(() => {
        return evaluateAnalyticalStatus({
            activeMode: this.activeMode,
            analyticalMode: this.analyticalMode,
            injectorEnabled: this.injectorEnabled,
            gravityEnabled: this.gravityEnabled,
            capillaryEnabled: this.capillaryEnabled,
            permMode: this.permMode,
            toggles: this.toggles,
        });
    });

    activeScenarioObject = $derived(getScenario(this.activeScenarioKey));
    activeAnalyticalOption = $derived.by((): ScenarioAnalyticalOption | null => {
        const scenario = this.activeScenarioObject;
        const options = scenario?.analyticalOptions ?? [];
        if (options.length === 0) return null;
        const selected = options.find((option) => option.key === this.activeAnalyticalOptionKey);
        if (selected) return selected;
        return options.find((option) => option.default) ?? options[0] ?? null;
    });

    activeScenarioAsFamily = $derived.by((): import('../catalog/benchmarkCases').BenchmarkFamily | null => {
        const sc = this.activeScenarioObject;
        if (!sc || this.isCustomMode) return null;
        const resolved = resolveCapabilities(sc.capabilities);
        const activeDimension = sc.sensitivities.find((dimension) => dimension.key === this.activeSensitivityDimensionKey) ?? null;
        const chartLayout = getScenarioChartLayout(sc, this.activeSensitivityDimensionKey);

        const xAxis = resolved.analyticalNativeXAxis as import('../catalog/benchmarkCases').BenchmarkXAxisKey;
        const panels = (resolved.primaryRateCurve === 'oil-rate'
            ? ['oil-rate', 'cumulative-oil', 'decline-diagnostics']
            : ['watercut-breakthrough', 'recovery', 'pressure']
        ) as import('../catalog/benchmarkCases').BenchmarkPanelKey[];

        return {
            key: sc.key,
            baseCaseKey: sc.key,
            analyticalMethod: resolved.analyticalMethod,
            sensitivityAxes: [],
            reference: {
                kind: 'analytical' as const,
                source: resolved.analyticalMethod === 'digitized-reference' ? `${sc.key}:digitized-reference` : `${sc.key}:analytical`,
            },
            displayDefaults: { xAxis, panels },
            stylePolicy: { colorBy: 'case' as const, lineStyleBy: 'quantity-or-reference' as const, separatePressurePanel: true },
            runPolicy: 'compare-to-reference' as const,
            label: sc.label,
            description: sc.description,
            baseCase: { key: sc.key, label: sc.label, description: sc.description, params: sc.params },
            suppressPrimaryAnalyticalOverlays: suppressesPrimaryAnalyticalOverlays(chartLayout),
            showSweepPanel: resolved.showSweepPanel,
            sweepGeometry: resolved.sweepGeometry,
            sweepAnalyticalMethod: this.activeAnalyticalOption?.sweepMethod,
            analyticalOverlayMode: activeDimension?.analyticalOverlayMode ?? 'auto',
            publishedReferenceSeries: sc.publishedReferenceSeries,
        };
    });

    warningPolicy = $derived.by(() => {
        return buildWarningPolicy({
            validationErrors: this.validationErrors,
            validationWarnings: this.validationWarnings,
            analyticalStatus: this.analyticalStatus as AnalyticalStatus,
            runtimeWarning: this.runtimeWarning,
            solverWarning: this.solverWarning,
            modelReinitNotice: this.modelReinitNotice,
            longRunEstimate: this.longRunEstimate,
            estimatedRunSeconds: this.estimatedRunSeconds,
        });
    });

    // ===== Constructor: effects =====

    constructor() {
        $effect(() => {
            if (!shouldAutoClearModifiedState({
                isModified: this.isModified,
                referenceProvenance: this.referenceProvenance,
                parameterOverrideCount: this.parameterOverrideCount,
            })) return;

            this.isModified = false;
            this.baseCaseSignature = this.buildCaseSignature();
        });
    }

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

    buildValidationInput(): SimulationInputs {
        return {
            nx: this.nx, ny: this.ny, nz: this.nz,
            cellDx: this.cellDx, cellDy: this.cellDy, cellDz: this.cellDz,
            steps: this.steps,
            initialSaturation: this.initialSaturation,
            delta_t_days: this.delta_t_days,
            well_radius: this.well_radius,
            mu_w: this.mu_w, mu_o: this.mu_o,
            c_o: this.c_o, c_w: this.c_w,
            rock_compressibility: this.rock_compressibility,
            volume_expansion_o: this.volume_expansion_o,
            volume_expansion_w: this.volume_expansion_w,
            max_sat_change_per_step: this.max_sat_change_per_step,
            max_pressure_change_per_step: this.max_pressure_change_per_step,
            max_well_rate_change_fraction: this.max_well_rate_change_fraction,
            injectorI: this.injectorI, injectorJ: this.injectorJ,
            producerI: this.producerI, producerJ: this.producerJ,
            s_wc: this.s_wc, s_or: this.s_or,
            s_gc: this.s_gc, s_gr: this.s_gr, s_org: this.s_org, n_g: this.n_g, mu_g: this.mu_g, c_g: this.c_g,
            threePhaseModeEnabled: this.threePhaseModeEnabled,
            uniformPermX: this.uniformPermX,
            reservoirPorosity: this.reservoirPorosity,
            minPerm: this.minPerm, maxPerm: this.maxPerm,
            injectorEnabled: this.injectorEnabled,
            injectorControlMode: this.injectorControlMode,
            producerControlMode: this.producerControlMode,
            injectorBhp: this.injectorBhp,
            producerBhp: this.producerBhp,
            targetInjectorRate: this.targetInjectorRate,
            targetProducerRate: this.targetProducerRate,
            targetInjectorSurfaceRate: this.targetInjectorSurfaceRate,
            targetProducerSurfaceRate: this.targetProducerSurfaceRate,
        };
    }

    buildCurrentParameterSnapshot(): Record<string, unknown> {
        return {
            nx: this.nx,
            ny: this.ny,
            nz: this.nz,
            cellDx: this.cellDx,
            cellDy: this.cellDy,
            cellDz: this.cellDz,
            initialPressure: this.initialPressure,
            initialSaturation: this.initialSaturation,
            reservoirPorosity: this.reservoirPorosity,
            pvtMode: this.pvtMode,
            pvtTable: this.pvtTable,
            scalTables: this.scalTables,
            apiGravity: this.apiGravity,
            gasSpecificGravity: this.gasSpecificGravity,
            reservoirTemperature: this.reservoirTemperature,
            bubblePoint: this.bubblePoint,
            mu_w: this.mu_w,
            mu_o: this.mu_o,
            c_o: this.c_o,
            c_w: this.c_w,
            rock_compressibility: this.rock_compressibility,
            depth_reference: this.depth_reference,
            volume_expansion_o: this.volume_expansion_o,
            volume_expansion_w: this.volume_expansion_w,
            rho_w: this.rho_w,
            rho_o: this.rho_o,
            permMode: this.permMode,
            uniformPermX: this.uniformPermX,
            uniformPermY: this.uniformPermY,
            uniformPermZ: this.uniformPermZ,
            minPerm: this.minPerm,
            maxPerm: this.maxPerm,
            useRandomSeed: this.useRandomSeed,
            randomSeed: this.randomSeed,
            layerPermsX: [...this.layerPermsX],
            layerPermsY: [...this.layerPermsY],
            layerPermsZ: [...this.layerPermsZ],
            s_wc: this.s_wc,
            s_or: this.s_or,
            n_w: this.n_w,
            n_o: this.n_o,
            k_rw_max: this.k_rw_max,
            k_ro_max: this.k_ro_max,
            well_radius: this.well_radius,
            well_skin: this.well_skin,
            injectorBhp: this.injectorBhp,
            producerBhp: this.producerBhp,
            injectorControlMode: this.injectorControlMode,
            producerControlMode: this.producerControlMode,
            injectorEnabled: this.injectorEnabled,
            targetInjectorRate: this.targetInjectorRate,
            targetProducerRate: this.targetProducerRate,
            targetInjectorSurfaceRate: this.targetInjectorSurfaceRate,
            targetProducerSurfaceRate: this.targetProducerSurfaceRate,
            injectorI: this.injectorI,
            injectorJ: this.injectorJ,
            producerI: this.producerI,
            producerJ: this.producerJ,
            max_sat_change_per_step: this.max_sat_change_per_step,
            max_pressure_change_per_step: this.max_pressure_change_per_step,
            max_well_rate_change_fraction: this.max_well_rate_change_fraction,
            gravityEnabled: this.gravityEnabled,
            capillaryEnabled: this.capillaryEnabled,
            capillaryPEntry: this.capillaryPEntry,
            capillaryLambda: this.capillaryLambda,
            analyticalMode: this.analyticalMode,
            analyticalDepletionRateScale: this.analyticalDepletionRateScale,
            analyticalArpsB: this.analyticalArpsB,
            s_gc: this.s_gc,
            s_gr: this.s_gr,
            s_org: this.s_org,
            n_g: this.n_g,
            k_rg_max: this.k_rg_max,
            pcogEnabled: this.pcogEnabled,
            pcogPEntry: this.pcogPEntry,
            pcogLambda: this.pcogLambda,
            mu_g: this.mu_g,
            c_g: this.c_g,
            rho_g: this.rho_g,
            threePhaseModeEnabled: this.threePhaseModeEnabled,
            injectedFluid: this.injectedFluid,
            initialGasSaturation: this.initialGasSaturation,
            gasRedissolutionEnabled: this.gasRedissolutionEnabled,
            initialRs: this.initialRs,
        };
    }

    syncLayerArraysToGrid() {
        this.layerPermsX = normalizeLayerArray(this.layerPermsX, this.uniformPermX, this.nz);
        this.layerPermsY = normalizeLayerArray(this.layerPermsY, this.uniformPermY, this.nz);
        this.layerPermsZ = normalizeLayerArray(this.layerPermsZ, this.uniformPermZ, this.nz);
    }

    buildCreatePayload(): SimulatorCreatePayload {
        return buildCreatePayloadFromState({
            nx: this.nx, ny: this.ny, nz: this.nz,
            cellDx: this.cellDx, cellDy: this.cellDy, cellDz: this.cellDz,
            initialPressure: this.initialPressure, initialSaturation: this.initialSaturation,
            porosity: this.reservoirPorosity,
            pvtMode: this.pvtMode,
            pvtTable: this.pvtTable,
            scalTables: this.scalTables,
            mu_w: this.mu_w, mu_o: this.mu_o, c_o: this.c_o, c_w: this.c_w,
            rho_w: this.rho_w, rho_o: this.rho_o,
            rock_compressibility: this.rock_compressibility,
            depth_reference: this.depth_reference,
            volume_expansion_o: this.volume_expansion_o,
            volume_expansion_w: this.volume_expansion_w,
            s_wc: this.s_wc, s_or: this.s_or,
            n_w: this.n_w, n_o: this.n_o,
            k_rw_max: this.k_rw_max, k_ro_max: this.k_ro_max,
            max_sat_change_per_step: this.max_sat_change_per_step,
            max_pressure_change_per_step: this.max_pressure_change_per_step,
            max_well_rate_change_fraction: this.max_well_rate_change_fraction,
            capillaryEnabled: this.capillaryEnabled,
            capillaryPEntry: this.capillaryPEntry,
            capillaryLambda: this.capillaryLambda,
            gravityEnabled: this.gravityEnabled,
            permMode: this.permMode,
            minPerm: this.minPerm, maxPerm: this.maxPerm,
            useRandomSeed: this.useRandomSeed, randomSeed: this.randomSeed,
            permsX: this.layerPermsX, permsY: this.layerPermsY, permsZ: this.layerPermsZ,
            well_radius: this.well_radius, well_skin: this.well_skin,
            injectorBhp: this.injectorBhp, producerBhp: this.producerBhp,
            rateControlledWells: this.rateControlledWells,
            injectorControlMode: this.injectorControlMode,
            producerControlMode: this.producerControlMode,
            injectorEnabled: this.injectorEnabled,
            targetInjectorRate: this.targetInjectorRate,
            targetProducerRate: this.targetProducerRate,
            targetInjectorSurfaceRate: this.targetInjectorSurfaceRate ?? undefined,
            targetProducerSurfaceRate: this.targetProducerSurfaceRate ?? undefined,
            injectorI: this.injectorI, injectorJ: this.injectorJ,
            producerI: this.producerI, producerJ: this.producerJ,
            uniformPermX: this.uniformPermX,
            uniformPermY: this.uniformPermY,
            uniformPermZ: this.uniformPermZ,
            s_gc: this.s_gc, s_gr: this.s_gr, s_org: this.s_org, n_g: this.n_g, k_rg_max: this.k_rg_max,
            pcogEnabled: this.pcogEnabled, pcogPEntry: this.pcogPEntry, pcogLambda: this.pcogLambda,
            mu_g: this.mu_g, c_g: this.c_g, rho_g: this.rho_g,
            threePhaseModeEnabled: this.threePhaseModeEnabled,
            injectedFluid: this.injectedFluid,
            initialGasSaturation: this.initialGasSaturation,
            gasRedissolutionEnabled: this.gasRedissolutionEnabled,
            initialRs: this.initialRs,
        });
    }

    buildModelResetKey() {
        return JSON.stringify({
            nx: this.nx, ny: this.ny, nz: this.nz,
            cellDx: this.cellDx, cellDy: this.cellDy, cellDz: this.cellDz,
            initialPressure: this.initialPressure, initialSaturation: this.initialSaturation,
            reservoirPorosity: this.reservoirPorosity,
            pvtTable: this.pvtTable,
            scalTables: this.scalTables,
            pvtMode: this.pvtMode, apiGravity: this.apiGravity, gasSpecificGravity: this.gasSpecificGravity,
            reservoirTemperature: this.reservoirTemperature, bubblePoint: this.bubblePoint,
            mu_w: this.mu_w, mu_o: this.mu_o, c_o: this.c_o, c_w: this.c_w,
            rock_compressibility: this.rock_compressibility, depth_reference: this.depth_reference,
            volume_expansion_o: this.volume_expansion_o, volume_expansion_w: this.volume_expansion_w,
            rho_w: this.rho_w, rho_o: this.rho_o,
            s_wc: this.s_wc, s_or: this.s_or, n_w: this.n_w, n_o: this.n_o,
            k_rw_max: this.k_rw_max, k_ro_max: this.k_ro_max,
            max_sat_change_per_step: this.max_sat_change_per_step,
            max_pressure_change_per_step: this.max_pressure_change_per_step,
            max_well_rate_change_fraction: this.max_well_rate_change_fraction,
            gravityEnabled: this.gravityEnabled, capillaryEnabled: this.capillaryEnabled,
            capillaryPEntry: this.capillaryPEntry, capillaryLambda: this.capillaryLambda,
            permMode: this.permMode,
            uniformPermX: this.uniformPermX, uniformPermY: this.uniformPermY,
            uniformPermZ: this.uniformPermZ,
            minPerm: this.minPerm, maxPerm: this.maxPerm,
            useRandomSeed: this.useRandomSeed, randomSeed: this.randomSeed,
            layerPermsX: this.layerPermsX, layerPermsY: this.layerPermsY,
            layerPermsZ: this.layerPermsZ,
            well_radius: this.well_radius, well_skin: this.well_skin,
            injectorBhp: this.injectorBhp, producerBhp: this.producerBhp,
            injectorControlMode: this.injectorControlMode,
            producerControlMode: this.producerControlMode,
            injectorEnabled: this.injectorEnabled,
            targetInjectorRate: this.targetInjectorRate,
            targetProducerRate: this.targetProducerRate,
            targetInjectorSurfaceRate: this.targetInjectorSurfaceRate,
            targetProducerSurfaceRate: this.targetProducerSurfaceRate,
            injectorI: this.injectorI, injectorJ: this.injectorJ,
            producerI: this.producerI, producerJ: this.producerJ,
            threePhaseModeEnabled: this.threePhaseModeEnabled,
            s_gc: this.s_gc, s_gr: this.s_gr, s_org: this.s_org, n_g: this.n_g, k_rg_max: this.k_rg_max,
            mu_g: this.mu_g, c_g: this.c_g, rho_g: this.rho_g,
            pcogEnabled: this.pcogEnabled, pcogPEntry: this.pcogPEntry,
            pcogLambda: this.pcogLambda,
            injectedFluid: this.injectedFluid,
            initialGasSaturation: this.initialGasSaturation,
            gasRedissolutionEnabled: this.gasRedissolutionEnabled,
            initialRs: this.initialRs,
        });
    }

    buildCaseSignature(): string {
        return JSON.stringify({
            model: this.buildModelResetKey(),
            delta_t_days: this.delta_t_days,
            steps: this.steps,
            analyticalMode: this.analyticalMode,
            analyticalDepletionRateScale: this.analyticalDepletionRateScale,
            analyticalArpsB: this.analyticalArpsB,
        });
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

    restoreActiveReferenceBaseDisplay() {
        const family = this.activeReferenceFamily;
        if (!family) return;

        this.applyCaseParams(family.baseCase.params);

        const baseResult = this.referenceRunResults.find(
            (result) => result.familyKey === family.key && result.variantKey === null,
        );
        if (baseResult) {
            this.hydrateRuntimeFromReferenceResult(baseResult);
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

        const savedSteps = this.steps; // preserve user's run-control override
        const savedDeltaTDays = this.delta_t_days;
        this.applyCaseParams(nextSpec.params);
        this.steps = savedSteps;
        this.delta_t_days = savedDeltaTDays;
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
        this.runCompleted = false;

        // $state.snapshot() strips Svelte 5 reactive proxies so the payload
        // can be structured-cloned by postMessage (pvtTable, cellDzPerLayer, etc.).
        const rawParams = $state.snapshot(nextSpec.params) as Record<string, any>;
        const payload = buildBenchmarkCreatePayload(rawParams);
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
        if (!this.activeReferenceFamily) {
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
        const family = this.activeReferenceFamily;
        if (!family) {
            this.runtimeError = 'Select a library reference before running the reference set.';
            return false;
        }
        return this.runReferenceSpecs(buildBenchmarkRunSpecs(family));
    }

    runActiveReferenceSensitivityAxis(axis: BenchmarkSensitivityAxisKey): boolean {
        const family = this.activeReferenceFamily;
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
        const family = this.activeReferenceFamily;
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
        this.rateHistory = message.rateHistory ?? [];
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
                    this.restoreActiveReferenceBaseDisplay();
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
                this.restoreActiveReferenceBaseDisplay();
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
                this.restoreActiveReferenceBaseDisplay();
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

    resolveCustomSubCase(mode: CaseMode | string): { key: string; label: string } | null {
        const raw = String(mode ?? '').toLowerCase();
        const normalizedMode: CaseMode | null =
            raw === 'dep' || raw === 'depletion' ? 'dep'
                : raw === 'wf' || raw === 'waterflood' ? 'wf'
                    : raw === 'sim' || raw === 'simulation' ? 'sim'
                        : null;
        if (!normalizedMode) return null;
        return CUSTOM_SUBCASE_BY_MODE[normalizedMode] ?? null;
    }

    maybeSwitchToCustomSubCaseOnReinit(): boolean {
        if (this.isModified || !this.activeCase || !this.baseCaseSignature) return false;

        const customSubCase = this.resolveCustomSubCase(this.activeMode);
        if (!customSubCase) return false;
        const nextSignature = this.buildCaseSignature();
        if (nextSignature === this.baseCaseSignature) return false;
        this.activeCase = customSubCase.key;
        this.baseCaseSignature = nextSignature;
        return true;
    }

    initSimulator(options: { runAfterInit?: boolean; silent?: boolean } = {}): boolean {
        const { runAfterInit = false, silent = false } = options;
        if (!this.wasmReady || !this.simWorker) {
            if (!silent) this.runtimeError = 'WASM not ready yet.';
            return false;
        }
        const validWellLocations =
            Number.isInteger(this.injectorI) && Number.isInteger(this.injectorJ) &&
            Number.isInteger(this.producerI) && Number.isInteger(this.producerJ) &&
            this.injectorI >= 0 && this.injectorI < this.nx &&
            this.injectorJ >= 0 && this.injectorJ < this.ny &&
            this.producerI >= 0 && this.producerI < this.nx &&
            this.producerJ >= 0 && this.producerJ < this.ny;
        if (!validWellLocations) {
            if (!silent) this.runtimeError = 'Invalid well location.';
            return false;
        }
        if (this.hasValidationErrors) {
            if (!silent) this.runtimeError = 'Input validation failed.';
            return false;
        }
        const switchedToCustomSubCase = this.maybeSwitchToCustomSubCaseOnReinit();
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
        if (this.hasValidationErrors) {
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
                deltaTDays: this.delta_t_days,
                historyInterval: batchHistoryInterval,
                chunkYieldInterval: 1,
            },
        });
    }

    stepOnce() { this.runSimulationBatch(1, 1); }
    runSteps() {
        this.runSimulationBatch(
            this.steps,
            this.userHistoryInterval ?? this.defaultHistoryInterval,
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
        const nextSignature = this.buildModelResetKey();
        if (!this.modelConfigSignature) { this.modelConfigSignature = nextSignature; return; }
        if (nextSignature === this.modelConfigSignature) return;
        this.modelConfigSignature = nextSignature;
        if (this.skipNextAutoModelReset) { this.skipNextAutoModelReset = false; return; }
        if (this.wasmReady && this.simWorker && this.isModified && this.lastCreateSignature) {
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

    // ===== Case Navigation =====

    handleModeChange(mode: CaseMode) {
        if (this.referenceSweepRunning || this.activeReferenceRunSpec) {
            this.runtimeWarning = 'Stop reference runs before switching families.';
            return;
        }

        this.isModified = false;
        this.referenceProvenance = null;
        this.activeMode = mode;
        this.toggles = getDefaultToggles(mode);
        this.explicitLibraryEntryKey = null;
        this.activeComparisonSelection = buildComparisonSelection();
        this.baseCaseSignature = '';
        this.clearReferenceRunnerState(true);

        this.handleToggleChange();
    }

    handleToggleChange(dimKey?: string, value?: string) {
        if (this.referenceSweepRunning || this.activeReferenceRunSpec) {
            this.runtimeWarning = 'Stop reference runs before changing the active case.';
            return;
        }

        const nextToggles = { ...this.toggles };
        if (dimKey && value) {
            nextToggles[dimKey] = value;
        }
        this.toggles = stabilizeToggleState(nextToggles);

        const newKey = buildCaseKey(this.toggles);
        this.activeCase = newKey;
        this.explicitLibraryEntryKey = null;
        this.isModified = false;
        this.referenceProvenance = null;
        this.activeComparisonSelection = buildComparisonSelection();
        this.clearReferenceRunnerState(true);
        this.clearRuntimeOverrides();

        this.applyCaseParams(composeCaseParams(this.toggles));
        this.baseCaseSignature = this.buildCaseSignature();
    }

    handleParamEdit() {
        if (this.isModified) return;
        this.isModified = true;
        this.baseCaseSignature = '';
    }

    clearRuntimeOverrides() {
        this.hasUserDeltaTDaysOverride = false;
        this.hasUserStepsOverride = false;
    }

    markDeltaTDaysOverride() {
        this.hasUserDeltaTDaysOverride = true;
    }

    markStepsOverride() {
        this.hasUserStepsOverride = true;
    }

    resolveOwningModeForLibraryEntry(entryKey: string): CaseMode | null {
        const entry = getCaseLibraryEntry(entryKey);
        if (!entry) return null;

        if (entry.entryKind === 'preset') {
            return entry.activation.activeMode;
        }

        if (entry.family === 'waterflood') return 'wf';
        if (entry.family === 'scenario-builder') return 'sim';
        return 'dep';
    }

    activateLibraryEntry(entryKey: string): boolean {
        const entry = getCaseLibraryEntry(entryKey);
        if (!entry) {
            this.runtimeError = 'Selected library case could not be resolved.';
            return false;
        }
        if (this.referenceSweepRunning || this.activeReferenceRunSpec) {
            this.runtimeWarning = 'Stop reference runs before changing the active library case.';
            return false;
        }

        const nextMode = this.resolveOwningModeForLibraryEntry(entryKey);
        if (!nextMode) {
            this.runtimeError = 'Selected library case could not be mapped to a scenario mode.';
            return false;
        }

        this.isModified = false;
        this.referenceProvenance = null;
        this.activeMode = nextMode;
        this.toggles = getDefaultToggles(nextMode);
        this.explicitLibraryEntryKey = entry.key;
        this.activeCase = entry.key;
        this.activeComparisonSelection = buildComparisonSelection();
        this.baseCaseSignature = '';
        this.clearReferenceRunnerState(true);
        this.clearRuntimeOverrides();

        this.applyCaseParams(entry.params);
        this.baseCaseSignature = this.buildCaseSignature();
        return true;
    }

    cloneActiveReferenceToCustom(): boolean {
        if (!shouldAllowReferenceClone({
            isModified: this.isModified,
            hasReferenceLibraryCase: Boolean(this.activeNavigationLibraryEntry),
        })) return false;

        const benchmarkId = this.activeReferenceFamily?.key ?? this.toggles.benchmarkId ?? null;
        const benchmarkLabel = this.activeNavigationLibraryEntry?.label
            ?? (benchmarkId ? getBenchmarkEntry(benchmarkId)?.label ?? null : null);
        const provenance = buildReferenceCloneProvenance({
            benchmarkId,
            sourceCaseKey: this.activeNavigationLibraryEntry?.key ?? this.activeCase,
            sourceLabel: benchmarkLabel,
        });

        this.handleParamEdit();
        if (provenance && !this.referenceProvenance) {
            this.referenceProvenance = provenance;
        }

        return true;
    }

    setReferenceProvenance(provenance: ReferenceProvenance | null) {
        this.referenceProvenance = provenance;
    }

    handleAnalyticalModeChange(mode: 'waterflood' | 'depletion') {
        this.analyticalMode = mode;
        this.analyticalProductionData = [];
        this.analyticalMeta = {
            mode,
            shapeFactor: null,
            shapeLabel: mode === 'depletion' ? 'Peaceman PSS' : '',
        };
    }

    handleNzOrPermModeChange() {
        this.syncLayerArraysToGrid();
    }

    applyCaseParams(params: Record<string, any>) {
        const resolved = { ...catalog.defaults, ...params };
        /** Return `v` when it's a real number, otherwise `fallback`. */
        const fin = (v: unknown, fallback: number): number => {
            const n = Number(v);
            return Number.isFinite(n) ? n : fallback;
        };
        this.skipNextAutoModelReset = true;
        this.userHistoryInterval = null;
        this.nx = Math.max(1, Math.round(fin(resolved.nx, 1)));
        this.ny = Math.max(1, Math.round(fin(resolved.ny, 1)));
        this.nz = Math.max(1, Math.round(fin(resolved.nz, 1)));
        this.cellDx = fin(resolved.cellDx, 10);
        this.cellDy = fin(resolved.cellDy, 10);
        this.cellDz = fin(resolved.cellDz, 1);
        this.delta_t_days = fin(resolved.delta_t_days, 0.25);
        this.steps = Math.max(1, Math.round(fin(resolved.steps, 20)));
        this.max_sat_change_per_step = fin(resolved.max_sat_change_per_step, 0.1);
        this.initialPressure = fin(resolved.initialPressure, 300);
        this.initialSaturation = fin(resolved.initialSaturation, 0.3);
        
        if (resolved.pvtMode === 'black-oil' || resolved.pvtMode === 'constant') {
            this.pvtMode = resolved.pvtMode;
        } else {
            this.pvtMode = 'constant';
        }
        this.pvtTableOverride = clonePvtTable(resolved.pvtTable);
        this.scalTables = cloneScalTables(resolved.scalTables);
        this.apiGravity = fin(resolved.apiGravity, 30.0);
        this.gasSpecificGravity = fin(resolved.gasSpecificGravity, 0.7);
        this.reservoirTemperature = fin(resolved.reservoirTemperature, 80.0);
        this.bubblePoint = fin(resolved.bubblePoint, 150.0);

        this.mu_w = fin(resolved.mu_w, 0.5);
        this.mu_o = fin(resolved.mu_o, 1.0);
        this.c_o = fin(resolved.c_o, 1e-5);
        this.c_w = fin(resolved.c_w, 3e-6);
        this.rock_compressibility = fin(resolved.rock_compressibility, 1e-6);
        this.depth_reference = fin(resolved.depth_reference, 0);
        this.volume_expansion_o = fin(resolved.volume_expansion_o, 1.0);
        this.volume_expansion_w = fin(resolved.volume_expansion_w, 1.0);
        this.rho_w = fin(resolved.rho_w, 1000);
        this.rho_o = fin(resolved.rho_o, 800);
        this.s_wc = fin(resolved.s_wc, 0.1);
        this.s_or = fin(resolved.s_or, 0.1);
        this.n_w = fin(resolved.n_w, 2.0);
        this.n_o = fin(resolved.n_o, 2.0);
        this.k_rw_max = fin(resolved.k_rw_max, 1.0);
        this.k_ro_max = fin(resolved.k_ro_max, 1.0);
        this.well_radius = fin(resolved.well_radius, 0.1);
        this.well_skin = fin(resolved.well_skin, 0);
        this.max_pressure_change_per_step = fin(resolved.max_pressure_change_per_step, 75);
        this.max_well_rate_change_fraction = fin(resolved.max_well_rate_change_fraction, 0.75);
        this.gravityEnabled = Boolean(resolved.gravityEnabled);
        this.capillaryEnabled = resolved.capillaryEnabled !== false;
        this.capillaryPEntry = fin(resolved.capillaryPEntry, 0);
        this.capillaryLambda = fin(resolved.capillaryLambda, 2);
        this.injectorEnabled = resolved.injectorEnabled !== false;
        // Three-phase
        this.threePhaseModeEnabled = Boolean(resolved.threePhaseModeEnabled);
        if (resolved.s_gc !== undefined) this.s_gc = Number(resolved.s_gc);
        if (resolved.s_gr !== undefined) this.s_gr = Number(resolved.s_gr);
        if (resolved.s_org !== undefined) this.s_org = Number(resolved.s_org);
        if (resolved.n_g !== undefined) this.n_g = Number(resolved.n_g);
        if (resolved.k_rg_max !== undefined) this.k_rg_max = Number(resolved.k_rg_max);
        if (resolved.mu_g !== undefined) this.mu_g = Number(resolved.mu_g);
        if (resolved.c_g !== undefined) this.c_g = Number(resolved.c_g);
        if (resolved.rho_g !== undefined) this.rho_g = Number(resolved.rho_g);
        if (resolved.pcogEnabled !== undefined) this.pcogEnabled = Boolean(resolved.pcogEnabled);
        if (resolved.pcogPEntry !== undefined) this.pcogPEntry = Number(resolved.pcogPEntry);
        if (resolved.pcogLambda !== undefined) this.pcogLambda = Number(resolved.pcogLambda);
        if (resolved.injectedFluid === 'water' || resolved.injectedFluid === 'gas') {
            this.injectedFluid = resolved.injectedFluid;
        }
        if (resolved.initialGasSaturation !== undefined) {
            this.initialGasSaturation = Number(resolved.initialGasSaturation);
        }
        if (resolved.gasRedissolutionEnabled !== undefined) {
            this.gasRedissolutionEnabled = Boolean(resolved.gasRedissolutionEnabled);
        }
        if (resolved.initialRs !== undefined) {
            this.initialRs = Number(resolved.initialRs);
        }

        // Sync analyticalMode from explicit params first, then legacy params, then inferred defaults.
        // When resolvedAnalyticalMode is undefined (not specified in params), keep the current value
        // — selectScenario already set it correctly from capabilities.
        const resolvedAnalyticalMode = resolved.analyticalMode ?? resolved.analyticalSolutionMode;
        if (resolvedAnalyticalMode === 'waterflood' || resolvedAnalyticalMode === 'depletion') {
            this.analyticalMode = resolvedAnalyticalMode;
        } else if (resolvedAnalyticalMode === 'none') {
            this.analyticalMode = 'none';
        } else if (resolvedAnalyticalMode !== undefined) {
            this.analyticalMode = this.injectorEnabled ? 'waterflood' : 'depletion';
        }
        this.analyticalDepletionRateScale = fin(resolved.analyticalDepletionRateScale, 1.0);
        this.analyticalArpsB = fin(resolved.analyticalArpsB, 0.0);
        this.injectorControlMode = resolved.injectorControlMode === 'rate' ? 'rate' : 'pressure';
        this.producerControlMode = resolved.producerControlMode === 'rate' ? 'rate' : 'pressure';
        this.injectorBhp = fin(resolved.injectorBhp, 400);
        this.producerBhp = fin(resolved.producerBhp, 100);
        this.targetInjectorRate = fin(resolved.targetInjectorRate, 350);
        this.targetProducerRate = fin(resolved.targetProducerRate, 350);
        this.targetInjectorSurfaceRate = resolved.targetInjectorSurfaceRate === undefined ? null : fin(resolved.targetInjectorSurfaceRate, 0);
        this.targetProducerSurfaceRate = resolved.targetProducerSurfaceRate === undefined ? null : fin(resolved.targetProducerSurfaceRate, 0);
        if (resolved.permMode && isPermMode(resolved.permMode)) { this.permMode = resolved.permMode; }
        if (resolved.uniformPermX !== undefined) this.uniformPermX = Number(resolved.uniformPermX);
        if (resolved.uniformPermY !== undefined) this.uniformPermY = Number(resolved.uniformPermY);
        if (resolved.uniformPermZ !== undefined) this.uniformPermZ = Number(resolved.uniformPermZ);
        if (resolved.minPerm !== undefined) this.minPerm = Number(resolved.minPerm);
        if (resolved.maxPerm !== undefined) this.maxPerm = Number(resolved.maxPerm);
        if (resolved.useRandomSeed !== undefined) this.useRandomSeed = Boolean(resolved.useRandomSeed);
        if (resolved.randomSeed !== undefined) this.randomSeed = Number(resolved.randomSeed);
        if (resolved.layerPermsX) this.layerPermsX = parseLayerValues(resolved.layerPermsX);
        if (resolved.layerPermsY) this.layerPermsY = parseLayerValues(resolved.layerPermsY);
        if (resolved.layerPermsZ) this.layerPermsZ = parseLayerValues(resolved.layerPermsZ);
        this.handleNzOrPermModeChange();
        this.injectorI = fin(resolved.injectorI, 0);
        this.injectorJ = fin(resolved.injectorJ, 0);
        this.producerI = fin(resolved.producerI, this.nx - 1);
        this.producerJ = fin(resolved.producerJ, defaultProducerJForGrid(this.ny));
        this.resetModelAndVisualizationState(true, false);
        this.modelNeedsReinit = true;
        this.modelReinitNotice = '';
    }

    // ===== Scenario-Picker Actions =====

    selectScenario(key: string) {
        const scenario = getScenario(key);
        if (!scenario) return;
        if (this.referenceSweepRunning || this.activeReferenceRunSpec) return;

        this.activeScenarioKey = key;
        this.isCustomMode = false;
        this.isModified = false;
        this.referenceProvenance = null;
        this.activeComparisonSelection = buildComparisonSelection();
        this.clearReferenceRunnerState(true);

        // Initialise sensitivity dimension and pre-select enabled variants.
        const defaultDimKey = scenario.defaultSensitivityDimensionKey ?? scenario.sensitivities[0]?.key ?? null;
        this.activeSensitivityDimensionKey = defaultDimKey;
        const defaultDim = scenario.sensitivities.find((d) => d.key === defaultDimKey) ?? null;
        this.activeVariantKeys = defaultDim ? getDefaultVariantKeys(defaultDim) : [];
        this.activeAnalyticalOptionKey = scenario.analyticalOptions?.find((option) => option.default)?.key
            ?? scenario.analyticalOptions?.[0]?.key
            ?? null;

        // Derive CaseMode from scenario capabilities.
        const nextMode: CaseMode = scenario.capabilities.requiresThreePhaseMode ? '3p'
            : scenario.capabilities.analyticalMethod === 'buckley-leverett' ? 'wf' : 'dep';
        this.activeMode = nextMode;
        this.toggles = getDefaultToggles(nextMode);
        this.explicitLibraryEntryKey = null;
        this.activeCase = key;
        this.clearRuntimeOverrides();
        this.analyticalMode = getDefaultScenarioAnalyticalMode(scenario.capabilities);

        this.applyCaseParams(scenario.params);
        this.baseCaseSignature = this.buildCaseSignature();
    }

    /**
     * Switch the active sensitivity dimension for the current scenario.
     * Resets activeVariantKeys to the new dimension's default-enabled variants.
     */
    selectSensitivityDimension(dimensionKey: string) {
        const scenario = this.activeScenarioObject;
        if (!scenario) return;
        if (this.referenceSweepRunning || this.activeReferenceRunSpec) return;
        const dimension = scenario.sensitivities.find((d) => d.key === dimensionKey);
        if (!dimension) {
            if (import.meta.env.DEV) {
                console.warn(`[store] selectSensitivityDimension: unknown key "${dimensionKey}" for scenario "${scenario.key}"`);
            }
            return;
        }
        if (dimensionKey === this.activeSensitivityDimensionKey) return;

        this.activeComparisonSelection = buildComparisonSelection();
        this.clearReferenceRunnerState(true);
        this.activeSensitivityDimensionKey = dimensionKey;
        this.activeVariantKeys = getDefaultVariantKeys(dimension);
    }

    selectAnalyticalOption(optionKey: string) {
        const scenario = this.activeScenarioObject;
        if (!scenario) return;
        if (this.referenceSweepRunning || this.activeReferenceRunSpec) return;
        if (!(scenario.analyticalOptions ?? []).some((option) => option.key === optionKey)) return;
        if (optionKey === this.activeAnalyticalOptionKey) return;

        this.activeComparisonSelection = buildComparisonSelection();
        this.clearReferenceRunnerState(true);
        this.activeAnalyticalOptionKey = optionKey;
    }

    toggleScenarioVariant(variantKey: string) {
        if (this.referenceSweepRunning || this.activeReferenceRunSpec) return;

        this.activeComparisonSelection = buildComparisonSelection();
        this.clearReferenceRunnerState(true);
        this.activeVariantKeys = this.activeVariantKeys.includes(variantKey)
            ? this.activeVariantKeys.filter((k) => k !== variantKey)
            : [...this.activeVariantKeys, variantKey];
    }

    enterCustomMode() {
        this.isCustomMode = true;
        this.activeAnalyticalOptionKey = null;
        this.handleParamEdit();
    }

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
            const runSteps = this.hasUserStepsOverride
                ? Math.max(1, Math.round(Number(this.steps ?? baseParams.steps ?? 240)))
                : Math.max(1, Math.round(Number(variantParams.steps ?? baseParams.steps ?? 240)));
            const runDeltaTDays = this.hasUserDeltaTDaysOverride
                ? Number(this.delta_t_days ?? baseParams.delta_t_days ?? 0.125)
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

    resetOverrideGroupsToBase(groupKeys: string[]): { resetCount: number } {
        if (!Array.isArray(groupKeys) || groupKeys.length === 0) {
            return { resetCount: 0 };
        }

        const resetPlan = buildOverrideResetPlan({
            groupKeys,
            groupedOverrides: this.parameterOverrideGroups,
            overrides: this.parameterOverrides,
        });

        for (const item of resetPlan) {
            const nextValue = Array.isArray(item.base) ? [...item.base] : item.base;
            (this as Record<string, unknown>)[item.key] = nextValue;
        }

        return { resetCount: resetPlan.length };
    }

    setComparisonSelection(selection: Partial<ComparisonSelection>) {
        this.activeComparisonSelection = buildComparisonSelection(selection);
    }

    // ===== Public Sub-namespace Views =====
    // Each returns `this` so consumers can use the familiar `params.nx`, `runtime.wasmReady` etc.
    // All state and methods live on the single class instance — no boilerplate re-export needed.

    get parameterState() { return this; }
    get scenarioSelection() { return this; }
    get runtimeState() { return this; }

    // historyInterval is a special computed+writable property
    get historyInterval() { return this.userHistoryInterval ?? this.defaultHistoryInterval; }
    set historyInterval(v: number | null) { this.userHistoryInterval = v; }


    // Navigation state delegation getters — flatten navigationState properties for direct access
    get activeFamily() { return this.navigationState.activeFamily; }
    get activeSource() { return this.navigationState.activeSource; }
    get activeLibraryCaseKey() { return this.navigationState.activeLibraryCaseKey; }
    get activeLibraryGroup() { return this.navigationState.activeLibraryGroup; }
    get sourceLabel() { return this.navigationState.sourceLabel; }
    get referenceSourceLabel() { return this.navigationState.referenceSourceLabel; }
    get provenanceSummary() { return this.navigationState.provenanceSummary; }
    get editabilityPolicy() { return this.navigationState.editabilityPolicy; }
}

// ---------- Factory ----------

export function createSimulationStore() {
    return new SimulationStoreImpl();
}

export type SimulationStore = ReturnType<typeof createSimulationStore>;
