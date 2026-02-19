<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import FractionalFlow from './lib/FractionalFlow.svelte';
    import DepletionAnalytical from './lib/DepletionAnalytical.svelte';
    import TopBar from './lib/ui/TopBar.svelte';
    import RunControls from './lib/ui/RunControls.svelte';
    import InputsTab from './lib/ui/InputsTab.svelte';
    import SwProfileChart from './lib/SwProfileChart.svelte';
    import { caseCatalog, findCaseByKey, resolveParams } from './lib/caseCatalog';
    import type { WorkerMessage, SimulatorSnapshot } from './lib';

    let wasmReady = false;
    let simWorker: Worker | null = null;
    let runCompleted = false;
    let workerRunning = false;

    // Navigation State
    let activeCategory = 'depletion';
    let activeCase = '';
    let isCustomMode = false;
    let preRunData: any = null;
    let preRunLoading = false;
    let preRunWarning = '';
    let preRunLoadToken = 0;
    let preRunContinuationAvailable = false;
    let preRunHydrated = false;
    let preRunHydrating = false;
    let preRunContinuationStatus = '';
    let pendingPreRunHydrationId = 0;
    let pendingPreRunHydrationResolve: ((ready: boolean) => void) | null = null;

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

    // Fluid properties
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
    import type { GridCell, WellState } from './lib';

    let gridStateRaw: GridCell[] | null = null;
    let wellStateRaw: WellState | null = null;
    let simTime = 0;
    let rateHistory = [];
    import type { AnalyticalProductionPoint } from './lib';
    let analyticalProductionData: AnalyticalProductionPoint[] = [];
    let analyticalSolutionMode: 'waterflood' | 'depletion' = 'depletion';
    let analyticalDepletionRateScale = 1.0;
    let analyticalMeta: { mode: 'waterflood' | 'depletion'; shapeFactor: number | null; shapeLabel: string } = {
        mode: 'waterflood',
        shapeFactor: null,
        shapeLabel: '',
    };
    let previousAnalyticalSolutionMode: 'waterflood' | 'depletion' = analyticalSolutionMode;
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

    $: if (analyticalSolutionMode !== previousAnalyticalSolutionMode) {
        previousAnalyticalSolutionMode = analyticalSolutionMode;
        analyticalProductionData = [];
        analyticalMeta = {
            mode: analyticalSolutionMode,
            shapeFactor: null,
            shapeLabel: analyticalSolutionMode === 'depletion' ? 'Peaceman PSS' : '',
        };
    }
    let validationWarnings: string[] = [];
    let hasValidationErrors = false;
    let estimatedRunSeconds = 0;
    let longRunEstimate = false;
    let latestInjectionRate = 0;

    $: latestInjectionRate = (() => {
        if (!Array.isArray(rateHistory) || rateHistory.length === 0) return 0;
        for (let i = rateHistory.length - 1; i >= 0; i--) {
            const q = Number(rateHistory[i]?.total_injection);
            if (Number.isFinite(q) && q > 0) return q;
        }
        return 0;
    })();

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

    let theme: 'dark' | 'light' = 'dark';

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
    let baseCaseSignature = '';
    let pendingAutoReinit = false;

    const CUSTOM_SUBCASE_BY_CATEGORY: Record<string, { key: string; label: string }> = {
        depletion: { key: 'depletion_custom_subcase', label: 'Custom Depletion Sub-case' },
        waterflood: { key: 'waterflood_custom_subcase', label: 'Custom Waterflood Sub-case' },
    };

    // Visualization
    let showProperty: 'pressure' | 'saturation_water' | 'saturation_oil' | 'permeability_x' | 'permeability_y' | 'permeability_z' | 'porosity' = 'pressure';
    let legendFixedMin = 0;
    let legendFixedMax = 1;

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
            const loadedFinalGrid = Array.isArray(data.finalGrid) ? data.finalGrid : null;
            const validHistoryEntries = loadedHistory.filter((entry) => Array.isArray(entry?.grid) && entry.grid.length === expectedCellCount);
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

            rateHistory = Array.isArray(data.rateHistory) ? data.rateHistory : [];
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

    type ValidationState = { errors: Record<string, string>; warnings: string[] };

    function validateInputs(): ValidationState {
        const errors: Record<string, string> = {};
        const warnings: string[] = [];

        if (initialSaturation < 0 || initialSaturation > 1) {
            errors.initialSaturation = 'Initial water saturation must be in [0, 1].';
        }
        if (delta_t_days <= 0) errors.deltaT = 'Timestep must be positive.';
        if (well_radius <= 0) errors.wellRadius = 'Well radius must be positive.';
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

    $: validationState = validateInputs();
    $: validationErrors = validationState.errors;
    $: validationWarnings = validationState.warnings;
    $: hasValidationErrors = Object.keys(validationErrors).length > 0;
    $: estimatedRunSeconds = Math.max(0, (Number(profileStats.avgStepMs || 0) * Number(steps || 0)) / 1000);
    $: longRunEstimate = estimatedRunSeconds > 10;

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
            if (Number(message.hydrationId) !== pendingPreRunHydrationId) return;
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
            if (preRunHydrating) {
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

    $: if (typeof document !== 'undefined') {
        document.documentElement.setAttribute('data-theme', theme);
    }
    $: if (typeof localStorage !== 'undefined') {
        localStorage.setItem('ressim-theme', theme);
    }

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

    $: if (wasmReady && simWorker && isCustomMode) {
        const nextSignature = JSON.stringify(buildCreatePayload());
        if (lastCreateSignature && nextSignature !== lastCreateSignature) {
            resetSimulationState({ clearErrors: true, clearWarnings: false, resetProfile: true, bumpViz: true });
            if (workerRunning) {
                pendingAutoReinit = true;
                runtimeWarning = 'Config changed during run. Stopping and reinitializing…';
                stopRun();
            } else {
                const reinitialized = initSimulator({ silent: true });
                runtimeWarning = reinitialized ? 'Config changed. Reservoir reinitialized at step 0.' : runtimeWarning;
            }
        }
    }

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
                chunkYieldInterval: 5,
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

    function applyHistoryIndex(idx) {
        if (idx < 0 || idx >= history.length) return;
        currentIndex = idx;
        const entry = history[idx];
        gridStateRaw = entry.grid;
        wellStateRaw = entry.wells;
        simTime = entry.time;
    }

    $: replayTime = history.length > 0 && currentIndex >= 0 && currentIndex < history.length
        ? history[currentIndex].time
        : null;

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

    function buildAvgPressureSeries(ratePoints, historyEntries): Array<number | null> {
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

    $: avgReservoirPressureSeries = buildAvgPressureSeries(rateHistory, history);

    function buildAvgWaterSaturationSeries(ratePoints, historyEntries): Array<number | null> {
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

    $: avgWaterSaturationSeries = buildAvgWaterSaturationSeries(rateHistory, history);
</script>

<main class="min-h-screen bg-base-200 text-base-content" data-theme={theme}>
    <div class="mx-auto max-w-400 space-y-4 p-4 lg:p-6">

        <!-- Hidden component for analytical calculations -->
        <FractionalFlow
            rockProps={{ s_wc, s_or, n_w, n_o }}
            fluidProps={{ mu_w, mu_o }}
            {initialSaturation}
            timeHistory={rateHistory.map((point) => point.time)}
            injectionRateSeries={rateHistory.map((point) => point.total_injection)}
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
                            <svelte:component
                                this={RateChartComponent}
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
                                <svelte:component
                                    this={ThreeDViewComponent}
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
                                    bind:playing
                                    bind:playSpeed
                                    bind:showDebugState
                                    onApplyHistoryIndex={applyHistoryIndex}
                                    onPrev={prev}
                                    onNext={next}
                                    onTogglePlay={togglePlay}
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