import type {
    SimulatorCreatePayload,
    PvtRow,
    ThreePhaseScalTables,
} from '../simulator-types';
import { generateBlackOilTable } from '../physics/pvt';
import { buildCreatePayloadFromState } from '../buildCreatePayload';
import { catalog } from '../catalog/caseCatalog';
import {
    validateInputs as validateSimulationInputs,
    type SimulationInputs,
    type ValidationState as InputValidationState,
    type ValidationWarning,
} from '../validateInputs';

// ---------- Helper utilities (pure, no runes) ----------

export function parseLayerValues(value: unknown): number[] {
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

export function normalizeLayerArray(values: number[], fallback: number, length: number): number[] {
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

// ---------- Store ----------

class ParameterStoreImpl {

    // ===== $state: Simulation Input Parameters =====

    nx = $state(15);
    ny = $state(10);
    nz = $state(10);
    cellDx = $state(10);
    cellDy = $state(10);
    cellDz = $state(1);
    cellDzPerLayer: number[] = $state([]);
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
    fimEnabled = $state(true);

    // Analytical solution
    analyticalMode: 'waterflood' | 'depletion' | 'none' = $state('depletion');
    analyticalDepletionRateScale = $state(1.0);
    analyticalArpsB = $state(0.0);
    analyticalDepletionStartDays = $state(0.0);
    pvtTableOverride = $state<PvtRow[] | undefined>(undefined);
    scalTables = $state<ThreePhaseScalTables | undefined>(undefined);

    // History interval override (run-control param, not output)
    userHistoryInterval = $state<number | null>(null);

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

    // ===== $derived: Validation =====

    validationState: InputValidationState = $derived.by(
        () => validateSimulationInputs(this.buildValidationInput()),
    );

    validationErrors: Record<string, string> = $derived(this.validationState.errors);
    validationWarnings: ValidationWarning[] = $derived(this.validationState.warnings);
    hasValidationErrors = $derived(Object.keys(this.validationErrors).length > 0);

    // ===== Accessors =====

    // historyInterval is a special computed+writable property
    get historyInterval() { return this.userHistoryInterval ?? this.defaultHistoryInterval; }
    set historyInterval(v: number | null) { this.userHistoryInterval = v; }

    // ===== Methods =====

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
            cellDzPerLayer: [...this.cellDzPerLayer],
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
            analyticalDepletionStartDays: this.analyticalDepletionStartDays,
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
        const fallbackThickness = Math.max(1e-12, Number(this.cellDz) || 1);
        this.cellDzPerLayer = normalizeLayerArray(this.cellDzPerLayer, fallbackThickness, this.nz);
    }

    /**
     * Apply a params record to all simulation input fields.
     * Pure parameter update — no runtime side-effects (no model reset, no worker stop).
     * Callers are responsible for triggering any needed runtime reset.
     */
    applyParamValues(params: Record<string, any>) {
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
        this.cellDzPerLayer = parseLayerValues(resolved.cellDzPerLayer);
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
        this.fimEnabled = resolved.fimEnabled !== false;

        // Sync analyticalMode from explicit params first, then legacy params, then inferred defaults.
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
        this.analyticalDepletionStartDays = fin(resolved.analyticalDepletionStartDays, 0.0);
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
        this.syncLayerArraysToGrid();
        this.injectorI = fin(resolved.injectorI, 0);
        this.injectorJ = fin(resolved.injectorJ, 0);
        this.producerI = fin(resolved.producerI, this.nx - 1);
        this.producerJ = fin(resolved.producerJ, defaultProducerJForGrid(this.ny));
    }

    buildModelResetKey() {
        return JSON.stringify({
            nx: this.nx, ny: this.ny, nz: this.nz,
            cellDx: this.cellDx, cellDy: this.cellDy, cellDz: this.cellDz,
            cellDzPerLayer: this.cellDzPerLayer,
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
            analyticalDepletionStartDays: this.analyticalDepletionStartDays,
        });
    }

    /**
     * Build the core simulator create payload from current parameter state.
     * The caller is responsible for adding scenario-specific sweep config and
     * termination policy (see RuntimeStore.buildCreatePayload).
     */
    buildCorePayload(): SimulatorCreatePayload {
        return buildCreatePayloadFromState({
            nx: this.nx, ny: this.ny, nz: this.nz,
            cellDx: this.cellDx, cellDy: this.cellDy, cellDz: this.cellDz,
            cellDzPerLayer: this.cellDzPerLayer,
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

    handleNzOrPermModeChange() {
        this.syncLayerArraysToGrid();
    }

    handleAnalyticalModeChange(mode: 'waterflood' | 'depletion') {
        this.analyticalMode = mode;
    }

    // ===== Internal flag used by checkConfigDiff =====

    /** Set by applyParamValues to suppress the next auto model reset. */
    skipNextAutoModelReset = $state(false);
}

// ---------- Factory ----------

export function createParameterStore() {
    return new ParameterStoreImpl();
}

export type ParameterStore = InstanceType<typeof ParameterStoreImpl>;
