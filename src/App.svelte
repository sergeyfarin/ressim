<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import FractionalFlow from './lib/FractionalFlow.svelte';
    import DepletionAnalytical from './lib/DepletionAnalytical.svelte';
    import TopBar from './lib/ui/TopBar.svelte';
    import RunControls from './lib/ui/RunControls.svelte';
    import InputsTab from './lib/ui/InputsTab.svelte';
    import SwProfileChart from './lib/SwProfileChart.svelte';
    import { caseCatalog, findCaseByKey, resolveParams } from './lib/caseCatalog';
    import type { WorkerMessage, SimulatorSnapshot, RateHistoryPoint } from './lib';

    let wasmReady = $state(false);
    let simWorker: Worker | null = $state(null);
    let runCompleted = $state(false);
    let workerRunning = $state(false);

    // Navigation State
    let activeCategory = $state('depletion');
    let activeCase = $state('');
    let isCustomMode = $state(false);
    let preRunData: any = $state(null);
    let preRunLoading = $state(false);
    let preRunWarning = $state('');
    let preRunLoadToken = $state(0);
    let preRunContinuationAvailable = $state(false);
    let preRunHydrated = $state(false);
    let preRunHydrating = $state(false);
    let preRunContinuationStatus = $state('');
    let pendingPreRunHydrationId = $state(0);
    let pendingPreRunHydrationResolve: ((ready: boolean) => void) | null = $state(null);
    let pendingPreRunHydrationTimeout: ReturnType<typeof setTimeout> | null = $state(null);

    // UI inputs
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
    const reservoirPorosity = 0.2;

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
    const ooipM3 = $derived(nx * ny * nz * cellDx * cellDy * cellDz * reservoirPorosity * Math.max(0, 1 - initialSaturation));
    const poreVolumeM3 = $derived(nx * ny * nz * cellDx * cellDy * cellDz * reservoirPorosity);

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

    // Well inputs
    let well_radius = $state(0.1);
    let well_skin = $state(0.0);
    let injectorBhp = $state(400.0);
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

    // Display data
    import type { GridCell, WellState } from './lib';

    let gridStateRaw: GridCell[] | null = $state(null);
    let wellStateRaw: WellState | null = $state(null);
    let simTime = $state(0);
    let rateHistory = $state<RateHistoryPoint[]>([]);
    import type { AnalyticalProductionPoint } from './lib';
    let analyticalProductionData: AnalyticalProductionPoint[] = $state([]);
    let analyticalSolutionMode: 'waterflood' | 'depletion' = $state('depletion');
    let analyticalDepletionRateScale = $state(1.0);
    let analyticalMeta: { mode: 'waterflood' | 'depletion'; shapeFactor: number | null; shapeLabel: string } = $state({
        mode: 'waterflood',
        shapeFactor: null,
        shapeLabel: '',
    });
    let runtimeWarning = $state('');
    let solverWarning = $state('');
    let runtimeError = $state('');
    let vizRevision = $state(0);
    let modelReinitNotice = $state('');
    let modelNeedsReinit = $state(false);
    let configDiffSignature = $state('');
    let skipNextAutoModelReset = $state(false);
    const latestInjectionRate = $derived.by(() => {
        if (!Array.isArray(rateHistory) || rateHistory.length === 0) return 0;
        for (let i = rateHistory.length - 1; i >= 0; i--) {
            const q = Number(rateHistory[i]?.total_injection);
            if (Number.isFinite(q) && q > 0) return q;
        }
        return 0;
    });

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
        batchMs: 0, avgStepMs: 0, extractMs: 0, renderApplyMs: 0, snapshotsSent: 0,
    };

    let theme: 'dark' | 'light' = $state('dark');

    // History / replay
    type HistoryEntry = SimulatorSnapshot;
    type ThreeDViewComponentType = typeof import('./lib/3dview.svelte').default;
    type RateChartComponentType = typeof import('./lib/RateChart.svelte').default;
    let history = $state<HistoryEntry[]>([]);
    const avgReservoirPressureSeries: Array<number | null> = $derived(buildAvgPressureSeries(rateHistory, history));
    const avgWaterSaturationSeries: Array<number | null> = $derived(buildAvgWaterSaturationSeries(rateHistory, history));
    let currentIndex = $state(-1);
    let playing = $state(false);
    let playSpeed = $state(2);
    let playTimer: ReturnType<typeof setInterval> | null = $state(null);
    const HISTORY_RECORD_INTERVAL = 1;
    const MAX_HISTORY_ENTRIES = 300;
    let showDebugState = $state(false);
    let profileStats: ProfileStats = $state({ ...EMPTY_PROFILE_STATS });
    let ThreeDViewComponent = $state<ThreeDViewComponentType | null>(null);
    let RateChartComponent = $state<RateChartComponentType | null>(null);
    let loadingThreeDView = $state(false);
    let lastCreateSignature = $state('');
    let baseCaseSignature = $state('');
    let pendingAutoReinit = $state(false);

    const CUSTOM_SUBCASE_BY_CATEGORY: Record<string, { key: string; label: string }> = {
        depletion: { key: 'depletion_custom_subcase', label: 'Custom Depletion Sub-case' },
        waterflood: { key: 'waterflood_custom_subcase', label: 'Custom Waterflood Sub-case' },
    };
    const PRE_RUN_HYDRATION_TIMEOUT_MS = 15000;

    // Visualization
    let showProperty: 'pressure' | 'saturation_water' | 'saturation_oil' | 'permeability_x' | 'permeability_y' | 'permeability_z' | 'porosity' = $state('pressure');
    let legendFixedMin = $state(0);
    let legendFixedMax = $state(1);

    // ---------- Helper utilities ----------

    function parseLayerValues(value: unknown): number[] {
        if (Array.isArray(value)) {
            return value.map((v) => Number(v)).filter((v) => Number.isFinite(v) && v > 0);
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

    function syncLayerArraysToGrid() {
        layerPermsX = normalizeLayerArray(layerPermsX, uniformPermX, nz);
        layerPermsY = normalizeLayerArray(layerPermsY, uniformPermY, nz);
        layerPermsZ = normalizeLayerArray(layerPermsZ, uniformPermZ, nz);
    }

    function isPermMode(value: string): value is 'uniform' | 'random' | 'perLayer' {
        return value === 'uniform' || value === 'random' || value === 'perLayer';
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
    const rateControlledWells = $derived.by(() => injectorControlMode === 'rate' && producerControlMode === 'rate');

    // ---------- Category / case navigation ----------

    function handleCategoryChange(cat: string) {
        preRunLoadToken += 1;
        resetPreRunContinuationState();
        isCustomMode = false;
        activeCategory = cat;
        activeCase = '';
        baseCaseSignature = '';
        preRunData = null;
        preRunWarning = '';
        preRunLoading = false;
        // Auto-select first case
        const cases = caseCatalog[cat]?.cases;
        if (cases && cases.length > 0) {
            handleCaseChange(cases[0].key);
        }
    }

    function handleCaseChange(key: string) {
        resetPreRunContinuationState();
        activeCase = key;
        isCustomMode = false;
        baseCaseSignature = '';
        preRunWarning = '';
        const found = findCaseByKey(key);
        if (found) {
            applyCaseParams(found.case.params);
            baseCaseSignature = buildCaseSignature();
            loadPreRunCase(key);
        }
    }

    function handleCustomMode() {
        preRunLoadToken += 1;
        resetPreRunContinuationState();
        isCustomMode = true;
        activeCase = '';
        baseCaseSignature = '';
        preRunData = null;
        preRunWarning = '';
        preRunLoading = false;
        resetModelAndVisualizationState(true, false);
    }

    function resolveCustomSubCase(category: string): { key: string; label: string } | null {
        return CUSTOM_SUBCASE_BY_CATEGORY[String(category ?? '').toLowerCase()] ?? null;
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

    function maybeSwitchToCustomSubCaseOnReinit(): boolean {
        if (isCustomMode || !activeCase || !baseCaseSignature) return false;
        if (!findCaseByKey(activeCase)) return false;

        const customSubCase = resolveCustomSubCase(activeCategory);
        if (!customSubCase) return false;

        const nextSignature = buildCaseSignature();
        if (nextSignature === baseCaseSignature) return false;

        preRunLoadToken += 1;
        resetPreRunContinuationState();
        preRunData = null;
        preRunWarning = '';
        preRunLoading = false;
        activeCase = customSubCase.key;
        baseCaseSignature = nextSignature;
        return true;
    }

    // Apply a case's params to the local state
    function applyCaseParams(params: Record<string, any>) {
        const resolved = resolveParams(params);
        skipNextAutoModelReset = true;

        // Grid dimensions first
        nx = Math.max(1, Math.round(Number(resolved.nx) || 1));
        ny = Math.max(1, Math.round(Number(resolved.ny) || 1));
        nz = Math.max(1, Math.round(Number(resolved.nz) || 1));
        cellDx = Number(resolved.cellDx) || 10;
        cellDy = Number(resolved.cellDy) || 10;
        cellDz = Number(resolved.cellDz) || 1;

        // Then everything else
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
        well_radius = Number(resolved.well_radius) || 0.1;
        well_skin = Number(resolved.well_skin) || 0;
        max_pressure_change_per_step = Number(resolved.max_pressure_change_per_step) || 75;
        max_well_rate_change_fraction = Number(resolved.max_well_rate_change_fraction) || 0.75;

        if (resolved.permMode && isPermMode(resolved.permMode)) {
            permMode = resolved.permMode;
        }
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
        producerI = Number(resolved.producerI) || (nx - 1);
        producerJ = Number(resolved.producerJ) || 0;

        resetModelAndVisualizationState(true, false);
    }

    // Load pre-run case data
    async function loadPreRunCase(key: string) {
        const requestToken = ++preRunLoadToken;
        resetPreRunContinuationState();
        preRunLoading = true;
        preRunData = null;
        preRunWarning = '';
        try {
            const url = `${import.meta.env.BASE_URL}cases/${key}.json`;
            const resp = await fetch(url, { cache: 'no-store' });
            if (requestToken !== preRunLoadToken || key !== activeCase || isCustomMode) return;
            if (!resp.ok) {
                preRunData = null;
                return;
            }
            const data = await resp.json();
            if (requestToken !== preRunLoadToken || key !== activeCase || isCustomMode) return;
            preRunData = data;

            const expectedCellCount = Math.max(1, Number(nx) * Number(ny) * Number(nz));
            const loadedHistory = Array.isArray(data.history) ? data.history : [];
            const loadedFinalGrid: GridCell[] | null = Array.isArray(data.finalGrid) ? data.finalGrid as GridCell[] : null;
            const validHistoryEntries: HistoryEntry[] = loadedHistory
                .filter((entry: any) => Array.isArray(entry?.grid) && entry.grid.length === expectedCellCount)
                .map((entry: any) => ({
                    time: Number(entry?.time ?? 0),
                    grid: entry.grid,
                    wells: Array.isArray(entry?.wells) ? entry.wells : [],
                    rateHistory: Array.isArray(entry?.rateHistory) ? normalizeRateHistory(entry.rateHistory) : [],
                    solverWarning: typeof entry?.solverWarning === 'string' ? entry.solverWarning : '',
                    recordHistory: Boolean(entry?.recordHistory),
                }));
            const historyHasMismatches = loadedHistory.length > 0 && validHistoryEntries.length !== loadedHistory.length;
            const finalGridMatches = Boolean(loadedFinalGrid && loadedFinalGrid.length === expectedCellCount);

            history = validHistoryEntries;
            currentIndex = validHistoryEntries.length - 1;

            const selectedHistoryEntry = currentIndex >= 0 ? validHistoryEntries[currentIndex] : null;
            const selectedHistoryGrid = Array.isArray(selectedHistoryEntry?.grid) ? selectedHistoryEntry.grid : null;

            if (selectedHistoryGrid && selectedHistoryGrid.length === expectedCellCount) {
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
            runCompleted = true;
            preRunContinuationAvailable = true;
            preRunHydrated = false;
            runtimeWarning = 'Pre-run case loaded. Click Run to continue from the saved endpoint.';
            vizRevision += 1;
        } catch {
            if (requestToken === preRunLoadToken) {
                preRunData = null;
                preRunWarning = 'Failed to load pre-run data for this case.';
                resetPreRunContinuationState();
            }
        } finally {
            if (requestToken === preRunLoadToken) {
                preRunLoading = false;
            }
        }
    }

    function resolvePendingPreRunHydration(ready: boolean) {
        if (pendingPreRunHydrationTimeout) {
            clearTimeout(pendingPreRunHydrationTimeout);
            pendingPreRunHydrationTimeout = null;
        }
        if (pendingPreRunHydrationResolve) {
            pendingPreRunHydrationResolve(ready);
            pendingPreRunHydrationResolve = null;
        }
    }

    function resetPreRunContinuationState() {
        preRunContinuationAvailable = false;
        preRunHydrated = false;
        preRunHydrating = false;
        preRunContinuationStatus = '';
        pendingPreRunHydrationId = 0;
        resolvePendingPreRunHydration(false);
    }

    async function ensurePreRunContinuationReady(): Promise<boolean> {
        if (isCustomMode || !activeCase) return true;
        if (!preRunContinuationAvailable) {
            if (preRunLoading) {
                runtimeWarning = 'Pre-run case data is still loading.';
                return false;
            }

            const nextCreateSignature = JSON.stringify(buildCreatePayload());
            if (lastCreateSignature && lastCreateSignature === nextCreateSignature && !modelNeedsReinit) {
                return true;
            }

            const initialized = initSimulator({ silent: true });
            if (initialized) {
                runtimeWarning = 'Pre-run continuation unavailable. Simulation started from step 0.';
            }
            return initialized;
        }
        if (preRunHydrated) return true;
        if (!wasmReady || !simWorker) {
            runtimeError = 'WASM not ready yet.';
            return false;
        }

        if (preRunHydrating && pendingPreRunHydrationResolve) {
            return new Promise<boolean>((resolve) => {
                const previousResolver = pendingPreRunHydrationResolve;
                pendingPreRunHydrationResolve = (ready: boolean) => {
                    previousResolver?.(ready);
                    resolve(ready);
                };
            });
        }

        const hydrateSteps = Math.max(1, Math.floor(Number(preRunData?.steps ?? steps ?? 1)));
        const hydrateDeltaT = Number(preRunData?.params?.delta_t_days ?? delta_t_days);
        if (!Number.isFinite(hydrateDeltaT) || hydrateDeltaT <= 0) {
            runtimeError = 'Cannot continue pre-run case: invalid timestep in case data.';
            return false;
        }

        preRunHydrating = true;
        preRunContinuationStatus = 'Preparing continuation…';
        workerRunning = true;
        runtimeError = '';

        const hydrationId = ++pendingPreRunHydrationId;
        const waitForHydration = new Promise<boolean>((resolve) => {
            pendingPreRunHydrationResolve = resolve;

            pendingPreRunHydrationTimeout = setTimeout(() => {
                if (pendingPreRunHydrationId === hydrationId && preRunHydrating) {
                    runtimeError = `Hydration timed out after ${Math.round(PRE_RUN_HYDRATION_TIMEOUT_MS / 1000)}s.`;
                    preRunHydrating = false;
                    preRunHydrated = false;
                    preRunContinuationStatus = '';
                    workerRunning = false;
                    simWorker?.postMessage({ type: 'stop' });
                    resolvePendingPreRunHydration(false);
                }
            }, PRE_RUN_HYDRATION_TIMEOUT_MS);
        });

        simWorker.postMessage({
            type: 'hydratePreRun',
            payload: {
                hydrationId,
                createPayload: buildCreatePayload(),
                steps: hydrateSteps,
                deltaTDays: hydrateDeltaT,
            },
        });

        return waitForHydration;
    }

    // ---------- Model reset / validation ----------

    function resetModelAndVisualizationState(stopWorker = true, showReinitNotice = false) {
        stopPlaying();
        if (!isCustomMode && activeCase) {
            resetPreRunContinuationState();
        }

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
            nx: Number(nx), ny: Number(ny), nz: Number(nz),
            cellDx: Number(cellDx), cellDy: Number(cellDy), cellDz: Number(cellDz),
            initialPressure: Number(initialPressure),
            initialSaturation: Number(initialSaturation),
            mu_w: Number(mu_w), mu_o: Number(mu_o),
            c_o: Number(c_o), c_w: Number(c_w),
            rock_compressibility: Number(rock_compressibility),
            depth_reference: Number(depth_reference),
            volume_expansion_o: Number(volume_expansion_o),
            volume_expansion_w: Number(volume_expansion_w),
            rho_w: Number(rho_w), rho_o: Number(rho_o),
            s_wc: Number(s_wc), s_or: Number(s_or),
            n_w: Number(n_w), n_o: Number(n_o),
            max_sat_change_per_step: Number(max_sat_change_per_step),
            max_pressure_change_per_step: Number(max_pressure_change_per_step),
            max_well_rate_change_fraction: Number(max_well_rate_change_fraction),
            gravityEnabled: Boolean(gravityEnabled),
            capillaryEnabled: Boolean(capillaryEnabled),
            capillaryPEntry: Number(capillaryPEntry),
            capillaryLambda: Number(capillaryLambda),
            permMode,
            uniformPermX: Number(uniformPermX), uniformPermY: Number(uniformPermY), uniformPermZ: Number(uniformPermZ),
            minPerm: Number(minPerm), maxPerm: Number(maxPerm),
            useRandomSeed: Boolean(useRandomSeed), randomSeed: Number(randomSeed),
            layerPermsX: layerPermsX.map(Number),
            layerPermsY: layerPermsY.map(Number),
            layerPermsZ: layerPermsZ.map(Number),
            well_radius: Number(well_radius), well_skin: Number(well_skin),
            injectorBhp: Number(injectorBhp), producerBhp: Number(producerBhp),
            injectorControlMode, producerControlMode,
            injectorEnabled: Boolean(injectorEnabled),
            targetInjectorRate: Number(targetInjectorRate),
            targetProducerRate: Number(targetProducerRate),
            injectorI: Number(injectorI), injectorJ: Number(injectorJ),
            producerI: Number(producerI), producerJ: Number(producerJ),
        });
    }

    type ValidationState = { errors: Record<string, string>; warnings: string[] };

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

        if (initialSaturation < 0 || initialSaturation > 1) {
            errors.initialSaturation = 'Initial water saturation must be in [0, 1].';
        }
        if (!isFiniteNumber(delta_t_days) || numeric(delta_t_days) <= 0) errors.deltaT = 'Timestep must be positive.';
        if (!isFiniteNumber(well_radius) || numeric(well_radius) <= 0) errors.wellRadius = 'Well radius must be positive.';

        if (!isFiniteNumber(mu_w) || numeric(mu_w) <= 0) errors.mu_w = 'Water viscosity must be positive.';
        if (!isFiniteNumber(mu_o) || numeric(mu_o) <= 0) errors.mu_o = 'Oil viscosity must be positive.';
        if (!isFiniteNumber(c_o) || numeric(c_o) < 0) errors.c_o = 'Oil compressibility must be ≥ 0.';
        if (!isFiniteNumber(c_w) || numeric(c_w) < 0) errors.c_w = 'Water compressibility must be ≥ 0.';
        if (!isFiniteNumber(rock_compressibility) || numeric(rock_compressibility) < 0) {
            errors.rock_compressibility = 'Rock compressibility must be ≥ 0.';
        }
        if (!isFiniteNumber(volume_expansion_o) || numeric(volume_expansion_o) <= 0) {
            errors.volume_expansion_o = 'Oil formation volume factor must be positive.';
        }
        if (!isFiniteNumber(volume_expansion_w) || numeric(volume_expansion_w) <= 0) {
            errors.volume_expansion_w = 'Water formation volume factor must be positive.';
        }

        if (!isFiniteNumber(max_sat_change_per_step) || numeric(max_sat_change_per_step) <= 0 || numeric(max_sat_change_per_step) > 1) {
            errors.max_sat_change_per_step = 'Max ΔSw per step must be in (0, 1].';
        }
        if (!isFiniteNumber(max_pressure_change_per_step) || numeric(max_pressure_change_per_step) <= 0) {
            errors.max_pressure_change_per_step = 'Max ΔP per step must be positive.';
        }
        if (!isFiniteNumber(max_well_rate_change_fraction) || numeric(max_well_rate_change_fraction) <= 0) {
            errors.max_well_rate_change_fraction = 'Max well-rate change fraction must be positive.';
        }

        if (!Number.isInteger(numeric(injectorI)) || !Number.isInteger(numeric(injectorJ)) ||
            !Number.isInteger(numeric(producerI)) || !Number.isInteger(numeric(producerJ))) {
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
    const estimatedRunSeconds = $derived(Math.max(0, (Number(profileStats.avgStepMs || 0) * Number(steps || 0)) / 1000));
    const longRunEstimate = $derived(estimatedRunSeconds > 10);

    // ---------- Simulation state management ----------

    function resetSimulationState(options: { clearErrors?: boolean; clearWarnings?: boolean; resetProfile?: boolean; bumpViz?: boolean } = {}) {
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
            pushHistoryEntry({
                time: message.time,
                grid: message.grid,
                wells: message.wells,
            });
        }
        updateProfileStats(message.profile, performance.now() - renderStart);
    }

    function handleWorkerMessage(event: MessageEvent<WorkerMessage>) {
        const message = event.data;
        if (!message) return;

        if (message.type === 'ready') {
            wasmReady = true;
            initSimulator({ silent: true });
            return;
        }
        if (message.type === 'runStarted') { runtimeError = ''; workerRunning = true; return; }
        if (message.type === 'state') { applyWorkerState(message.data); return; }
        if (message.type === 'hydrated') {
            if (!preRunHydrating) return;
            if (Number(message.hydrationId ?? 0) !== pendingPreRunHydrationId) return;
            preRunHydrating = false;
            preRunHydrated = true;
            preRunContinuationStatus = '';
            workerRunning = false;
            resolvePendingPreRunHydration(true);
            return;
        }

        if (message.type === 'batchComplete') {
            workerRunning = false;
            runCompleted = true;
            updateProfileStats(message.profile, profileStats.renderApplyMs);
            applyHistoryIndex(history.length - 1);

            if (pendingAutoReinit) {
                pendingAutoReinit = false;
                const reinitialized = initSimulator({ silent: true });
                runtimeWarning = reinitialized ? 'Config changed. Reservoir reinitialized at step 0.' : runtimeWarning;
            }
            return;
        }

        if (message.type === 'stopped') {
            workerRunning = false;
            if (preRunHydrating && message.hydration) {
                const hydrationId = Number(message.hydrationId ?? pendingPreRunHydrationId);
                if (hydrationId !== pendingPreRunHydrationId) return;
                preRunHydrating = false;
                preRunHydrated = false;
                preRunContinuationStatus = '';
                runtimeWarning = 'Pre-run continuation cancelled before completion.';
                resolvePendingPreRunHydration(false);
                return;
            }
            runCompleted = true;
            if (pendingAutoReinit) {
                pendingAutoReinit = false;
                const reinitialized = initSimulator({ silent: true });
                runtimeWarning = reinitialized ? 'Config changed during run. Reinitialized at step 0.' : runtimeWarning;
                return;
            }
            runtimeWarning = message.reason === 'user'
                ? `Simulation stopped after ${Number(message.completedSteps ?? 0)} step(s).`
                : 'No running simulation to stop.';
            if ('profile' in message && message.profile) updateProfileStats(message.profile, profileStats.renderApplyMs);
            applyHistoryIndex(history.length - 1);
            return;
        }

        if (message.type === 'warning') { runtimeWarning = String(message.message ?? 'Simulation warning'); return; }
        if (message.type === 'error') {
            workerRunning = false;
            console.error('Simulation worker error:', message.message);
            runtimeError = String(message.message ?? 'Simulation error');
            if (pendingAutoReinit) pendingAutoReinit = false;
            if (preRunHydrating) {
                preRunHydrating = false;
                preRunHydrated = false;
                preRunContinuationStatus = '';
                resolvePendingPreRunHydration(false);
            }
        }
    }

    function setupWorker() {
        simWorker = new Worker(new URL('./lib/sim.worker.ts', import.meta.url), { type: 'module' });
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

    // ---------- Lazy module loading ----------

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

    // ---------- Theme ----------

    function toggleTheme() {
        theme = theme === 'dark' ? 'light' : 'dark';
    }

    // ---------- Lifecycle ----------

    onMount(() => {
        const savedTheme = localStorage.getItem('ressim-theme');
        if (savedTheme === 'light' || savedTheme === 'dark') theme = savedTheme;
        document.documentElement.setAttribute('data-theme', theme);
        setupWorker();
        loadRateChartModule();
        loadThreeDViewModule();
        // Auto-select first category and case
        handleCategoryChange('depletion');
    });

    $effect(() => {
        if (typeof document === 'undefined') return;
        document.documentElement.setAttribute('data-theme', theme);
    });
    $effect(() => {
        if (typeof localStorage === 'undefined') return;
        localStorage.setItem('ressim-theme', theme);
    });

    onDestroy(() => {
        stopPlaying();
        if (simWorker) {
            simWorker.postMessage({ type: 'dispose' });
            simWorker.terminate();
            simWorker = null;
        }
    });

    // ---------- Simulator init / run ----------

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
        preRunHydrated = false;
        preRunHydrating = false;
        preRunContinuationStatus = '';
        resolvePendingPreRunHydration(false);
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

    import type { SimulatorCreatePayload } from './lib';
    import { buildCreatePayloadFromState } from './lib/buildCreatePayload';

    function buildCreatePayload(): SimulatorCreatePayload {
        return buildCreatePayloadFromState({
            nx, ny, nz,
            cellDx, cellDy, cellDz,
            initialPressure, initialSaturation,
            mu_w, mu_o, c_o, c_w, rho_w, rho_o,
            rock_compressibility, depth_reference, volume_expansion_o, volume_expansion_w,
            s_wc, s_or, n_w, n_o,
            max_sat_change_per_step, max_pressure_change_per_step, max_well_rate_change_fraction,
            capillaryEnabled, capillaryPEntry, capillaryLambda,
            gravityEnabled,
            permMode, minPerm, maxPerm, useRandomSeed, randomSeed,
            permsX: layerPermsX, permsY: layerPermsY, permsZ: layerPermsZ,
            well_radius, well_skin, injectorBhp, producerBhp,
            rateControlledWells, injectorControlMode, producerControlMode, injectorEnabled,
            targetInjectorRate, targetProducerRate,
            injectorI, injectorJ, producerI, producerJ,
            uniformPermX, uniformPermY, uniformPermZ,
        });
    }

    // Use the typed helper and shared `SimulatorCreatePayload` from `src/lib` for compile-time checks.

    function checkConfigDiff() {
        const nextSignature = JSON.stringify(buildCreatePayload());
        if (!configDiffSignature) {
            configDiffSignature = nextSignature;
            return;
        }
        if (nextSignature === configDiffSignature) return;

        configDiffSignature = nextSignature;

        if (skipNextAutoModelReset) {
            skipNextAutoModelReset = false;
            return;
        }

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

    $effect(() => {
        checkConfigDiff();
    });

    function stepOnce() { runSimulationBatch(1, 1); }
    function runSteps() { runSimulationBatch(Number(steps), HISTORY_RECORD_INTERVAL); }

    async function runSimulationBatch(batchSteps: number, historyInterval: number) {
        if (modelNeedsReinit) { initSimulator(); return; }
        const continuationReady = await ensurePreRunContinuationReady();
        if (!continuationReady) return;
        if (!simWorker || workerRunning || hasValidationErrors) return;
        workerRunning = true;
        runtimeError = '';
        runtimeWarning = longRunEstimate
            ? `Estimated run: ${estimatedRunSeconds.toFixed(1)}s. You can stop at any time.`
            : runtimeWarning;
        simWorker.postMessage({
            type: 'run',
            payload: {
                steps: batchSteps,
                deltaTDays: Number(delta_t_days),
                historyInterval,
                chunkYieldInterval: 1,
            }
        });
    }

    function stopRun() {
        if (!simWorker) return;
        simWorker.postMessage({ type: 'stop' });
    }

    // ---------- Playback controls ----------

    function play() {
        if (history.length === 0) return;
        if (playTimer) { clearInterval(playTimer); playTimer = null; }
        playing = true;
        playTimer = setInterval(() => {
            next();
            if (currentIndex >= history.length - 1) stopPlaying();
        }, 1000 / playSpeed);
    }

    function stopPlaying() {
        playing = false;
        if (playTimer) { clearInterval(playTimer); playTimer = null; }
    }

    function togglePlay() { if (playing) stopPlaying(); else play(); }

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

    function applyHistoryIndex(idx: number) {
        if (idx < 0 || idx >= history.length) return;
        currentIndex = idx;
        const entry = history[idx];
        gridStateRaw = entry.grid;
        wellStateRaw = entry.wells;
        simTime = entry.time;
    }

    const replayTime = $derived(history.length > 0 && currentIndex >= 0 && currentIndex < history.length
        ? history[currentIndex].time
        : null);

    // ---------- Derived series for charts ----------

    function computeAveragePressure(grid: Array<{ pressure?: number }>): number {
        if (!Array.isArray(grid) || grid.length === 0) return 0;
        let sum = 0, count = 0;
        for (const cell of grid) {
            const value = Number(cell?.pressure);
            if (Number.isFinite(value)) { sum += value; count += 1; }
        }
        return count > 0 ? sum / count : 0;
    }

    function computeAverageWaterSaturation(grid: Array<{ sat_water?: number; satWater?: number; sw?: number }>): number {
        if (!Array.isArray(grid) || grid.length === 0) return 0;
        let sum = 0, count = 0;
        for (const cell of grid) {
            const value = Number(cell?.sat_water ?? cell?.satWater ?? cell?.sw);
            if (Number.isFinite(value)) { sum += value; count += 1; }
        }
        return count > 0 ? sum / count : 0;
    }

    function buildAvgPressureSeries(ratePoints: RateHistoryPoint[], historyEntries: HistoryEntry[]): Array<number | null> {
        if (!Array.isArray(ratePoints) || ratePoints.length === 0) return [];
        const snapshots = historyEntries
            .filter((entry) => Number.isFinite(Number(entry?.time)) && Array.isArray(entry?.grid))
            .map((entry) => ({ time: Number(entry.time), avgPressure: computeAveragePressure(entry.grid ?? []) }))
            .sort((a, b) => a.time - b.time);
        if (snapshots.length === 0) return ratePoints.map(() => null);
        let snapIdx = 0, currentAvg = snapshots[0].avgPressure;
        const aligned = [];
        for (const point of ratePoints) {
            const t = Number(point?.time ?? 0);
            while (snapIdx + 1 < snapshots.length && snapshots[snapIdx + 1].time <= t) {
                snapIdx += 1; currentAvg = snapshots[snapIdx].avgPressure;
            }
            aligned.push(currentAvg);
        }
        return aligned;
    }

    function buildAvgWaterSaturationSeries(ratePoints: RateHistoryPoint[], historyEntries: HistoryEntry[]): Array<number | null> {
        if (!Array.isArray(ratePoints) || ratePoints.length === 0) return [];
        const snapshots = historyEntries
            .filter((entry) => Number.isFinite(Number(entry?.time)) && Array.isArray(entry?.grid))
            .map((entry) => ({ time: Number(entry.time), avgSw: computeAverageWaterSaturation(entry.grid ?? []) }))
            .sort((a, b) => a.time - b.time);
        if (snapshots.length === 0) return ratePoints.map(() => null);
        let snapIdx = 0, currentAvg = snapshots[0].avgSw;
        const aligned = [];
        for (const point of ratePoints) {
            const t = Number(point?.time ?? 0);
            while (snapIdx + 1 < snapshots.length && snapshots[snapIdx + 1].time <= t) {
                snapIdx += 1; currentAvg = snapshots[snapIdx].avgSw;
            }
            aligned.push(currentAvg);
        }
        return aligned;
    }

</script>

<main class="min-h-screen bg-base-200 text-base-content" data-theme={theme}>
    <div class="mx-auto max-w-400 space-y-4 p-4 lg:p-6">

        <!-- Hidden component for analytical calculations -->
        <FractionalFlow
            rockProps={{ s_wc, s_or, n_w, n_o }}
            fluidProps={{ mu_w, mu_o }}
            {initialSaturation}
            timeHistory={rateHistory.map((point) => point.time)}
            injectionRateSeries={rateHistory.map((point) => Number(point.total_injection ?? 0))}
            reservoir={{ length: nx * cellDx, area: ny * cellDy * nz * cellDz, porosity: reservoirPorosity }}
            scenarioMode={analyticalSolutionMode}
            onAnalyticalData={(detail) => {
                if (analyticalSolutionMode === 'waterflood') {
                    analyticalProductionData = detail.production;
                }
            }}
            onAnalyticalMeta={(detail) => {
                if (analyticalSolutionMode === 'waterflood') {
                    analyticalMeta = detail;
                }
            }}
        />

        <DepletionAnalytical
            enabled={analyticalSolutionMode === 'depletion'}
            timeHistory={rateHistory.map((point) => point.time)}
            reservoir={{ length: nx * cellDx, area: ny * cellDy * nz * cellDz, porosity: reservoirPorosity }}
            {initialSaturation}
            permX={uniformPermX}
            permY={uniformPermY}
            {cellDx}
            {cellDy}
            wellboreDz={nz * cellDz}
            wellRadius={well_radius}
            wellSkin={well_skin}
            muO={mu_o}
            sWc={s_wc}
            sOr={s_or}
            nO={n_o}
            c_o={c_o}
            c_w={c_w}
            cRock={rock_compressibility}
            {initialPressure}
            producerBhp={producerBhp}
            depletionRateScale={analyticalDepletionRateScale}
            onAnalyticalData={(detail) => {
                if (analyticalSolutionMode === 'depletion') {
                    analyticalProductionData = detail.production;
                }
            }}
            onAnalyticalMeta={(detail) => {
                if (analyticalSolutionMode === 'depletion') {
                    analyticalMeta = detail;
                }
            }}
        />

        <!-- Header -->
        <header class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div>
                <h1 class="text-2xl font-bold lg:text-3xl">Simplified Reservoir Simulation Model</h1>
                <p class="text-sm opacity-80">Interactive two-phase simulation with 3D visualisation fully in browser.</p>
            </div>
            <button class="btn btn-sm btn-outline" onclick={toggleTheme}>
                {theme === 'dark' ? '☀ Light' : '🌙 Dark'}
            </button>
        </header>

        <!-- Top Bar: category buttons + case selector -->
        <TopBar
            {activeCategory}
            {activeCase}
            {isCustomMode}
            customSubCase={resolveCustomSubCase(activeCategory)}
            onCategoryChange={handleCategoryChange}
            onCaseChange={handleCaseChange}
            onCustomMode={handleCustomMode}
        />

        <!-- Run Controls -->
        <RunControls
            {wasmReady}
            {workerRunning}
            {runCompleted}
            {simTime}
            historyLength={history.length}
            {estimatedRunSeconds}
            {longRunEstimate}
            canStop={workerRunning}
            hasValidationErrors={hasValidationErrors}
            {solverWarning}
            modelReinitNotice={modelReinitNotice}
            continuationStatus={preRunContinuationStatus}
            inputsAnchorHref="#inputs-section"
            bind:steps
            onRunSteps={runSteps}
            onStepOnce={stepOnce}
            onInitSimulator={initSimulator}
            onStopRun={stopRun}
        />

        <!-- Error / Warning banners -->
        {#if runtimeWarning}
            <div class="rounded-md border border-warning bg-base-100 p-2 text-xs text-warning">{runtimeWarning}</div>
        {/if}
        {#if preRunWarning}
            <div class="rounded-md border border-warning bg-base-100 p-2 text-xs text-warning">{preRunWarning}</div>
        {/if}
        {#if runtimeError}
            <div class="rounded-md border border-error bg-base-100 p-2 text-xs text-error">{runtimeError}</div>
        {/if}
        {#if preRunLoading}
            <div class="text-xs opacity-60 text-center">Loading pre-run case data…</div>
        {/if}

        <div class="grid grid-cols-1 gap-3 xl:grid-cols-2 xl:items-start">
            <div class="space-y-3">
                <div class="card border border-base-300 bg-base-100 shadow-sm">
                    <div class="card-body p-4 md:p-5">
                        {#if RateChartComponent}
                            <RateChartComponent
                                {rateHistory}
                                {analyticalProductionData}
                                {avgReservoirPressureSeries}
                                {avgWaterSaturationSeries}
                                {ooipM3}
                                {poreVolumeM3}
                                {activeCategory}
                                {activeCase}
                                {theme}
                            />
                        {:else}
                            <div class="text-sm opacity-70">Loading rate chart…</div>
                        {/if}
                    </div>
                </div>

                <SwProfileChart
                    gridState={gridStateRaw ?? []}
                    {nx} {ny} {nz}
                    {cellDx} {cellDy} {cellDz}
                    simTime={simTime}
                    producerJ={producerJ}
                    {initialSaturation}
                    reservoirPorosity={reservoirPorosity}
                    injectionRate={latestInjectionRate}
                    scenarioMode={analyticalSolutionMode}
                    rockProps={{ s_wc, s_or, n_w, n_o }}
                    fluidProps={{ mu_w, mu_o }}
                />

                {#if analyticalMeta.mode === 'depletion'}
                    <div class="rounded-md border border-base-300 bg-base-100 p-3 text-xs">
                        <div class="font-semibold">Depletion Analytical Mode</div>
                        <div class="opacity-80">Model: {analyticalMeta.shapeLabel || 'PSS'} — q(t)&nbsp;=&nbsp;J·ΔP·e<sup>−t/τ</sup>, τ&nbsp;=&nbsp;V<sub>p</sub>·c<sub>t</sub>/J</div>
                    </div>
                {/if}
            </div>

            <div class="space-y-3">
                <div class="card border border-base-300 bg-base-100 shadow-sm">
                    <div class="card-body p-4 md:p-5">
                        {#if ThreeDViewComponent}
                            {#key `${nx}-${ny}-${nz}-${vizRevision}`}
                                <ThreeDViewComponent
                                    nx={nx} ny={ny} nz={nz}
                                    cellDx={cellDx} cellDy={cellDy} cellDz={cellDz}
                                    {theme}
                                    gridState={gridStateRaw}
                                    bind:showProperty
                                    bind:legendFixedMin
                                    bind:legendFixedMax
                                    {s_wc}
                                    {s_or}
                                    bind:currentIndex
                                    {replayTime}
                                    onApplyHistoryIndex={applyHistoryIndex}
                                    history={history}
                                    wellState={wellStateRaw}
                                />
                            {/key}
                        {:else}
                            <div class="flex items-center justify-center rounded border border-base-300 bg-base-200" style="height: clamp(240px, 35vh, 420px);">
                                {#if loadingThreeDView}
                                    <span class="loading loading-spinner loading-md"></span>
                                {:else}
                                    <button class="btn btn-sm" onclick={loadThreeDViewModule}>Load 3D view</button>
                                {/if}
                            </div>
                        {/if}
                    </div>
                </div>
            </div>
        </div>

        <div id="inputs-section">
            <InputsTab
                bind:nx bind:ny bind:nz
                bind:cellDx bind:cellDy bind:cellDz
                bind:initialPressure bind:initialSaturation
                bind:mu_w bind:mu_o bind:c_o bind:c_w
                bind:rho_w bind:rho_o
                bind:rock_compressibility bind:depth_reference
                bind:volume_expansion_o bind:volume_expansion_w
                bind:gravityEnabled
                bind:permMode
                bind:uniformPermX bind:uniformPermY bind:uniformPermZ
                bind:useRandomSeed bind:randomSeed
                bind:minPerm bind:maxPerm
                bind:layerPermsX bind:layerPermsY bind:layerPermsZ
                bind:s_wc bind:s_or bind:n_w bind:n_o
                bind:capillaryEnabled bind:capillaryPEntry bind:capillaryLambda
                bind:well_radius bind:well_skin
                bind:injectorEnabled
                bind:injectorControlMode bind:producerControlMode
                bind:injectorBhp bind:producerBhp
                bind:targetInjectorRate bind:targetProducerRate
                bind:injectorI bind:injectorJ bind:producerI bind:producerJ
                bind:delta_t_days
                bind:max_sat_change_per_step
                bind:max_pressure_change_per_step
                bind:max_well_rate_change_fraction
                bind:analyticalSolutionMode
                bind:analyticalDepletionRateScale
                onAnalyticalSolutionModeChange={handleAnalyticalSolutionModeChange}
                onNzOrPermModeChange={handleNzOrPermModeChange}
                {validationErrors}
                {validationWarnings}
                readOnly={!isCustomMode && activeCase !== ''}
            />
        </div>

        <!-- Debug State -->
        {#if showDebugState}
            <div class="card border border-base-300 bg-base-100 shadow-sm">
                <div class="card-body grid gap-4 p-4 lg:grid-cols-2">
                    <div>
                        <h4 class="mb-2 text-sm font-semibold">Grid State (current)</h4>
                        <pre class="max-h-105 overflow-auto rounded border border-base-300 bg-base-200 p-2 text-xs">{JSON.stringify(gridStateRaw, null, 2)}</pre>
                    </div>
                    <div>
                        <h4 class="mb-2 text-sm font-semibold">Well State (current)</h4>
                        <pre class="max-h-105 overflow-auto rounded border border-base-300 bg-base-200 p-2 text-xs">{JSON.stringify(wellStateRaw, null, 2)}</pre>
                    </div>
                </div>
            </div>
        {/if}
    </div>
</main>