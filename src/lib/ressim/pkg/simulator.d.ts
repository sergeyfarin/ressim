/* tslint:disable */
/* eslint-disable */

export class ReservoirSimulator {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Add a well to the simulator
     * Parameters in oil-field units:
     * - i, j, k: grid cell indices (must be within grid bounds)
     * - bhp: bottom-hole pressure [bar] (must be finite, typical: -100 to 2000 bar)
     * - well_radius: wellbore radius [m]
     * - skin: skin factor [dimensionless]
     * - injector: true for injector (injects fluid), false for producer (extracts fluid)
     *
     * Returns Ok(()) on success, or Err(message) if parameters are invalid.
     * Invalid parameters include:
     * - Out-of-bounds grid indices
     * - NaN or Inf values in bhp or pi
     * - Negative productivity index
     * - BHP outside reasonable range
     */
    add_well(i: number, j: number, k: number, bhp: number, well_radius: number, skin: number, injector: boolean): void;
    getDimensions(): any;
    /**
     * Get last solver warning message (empty string if no warning)
     */
    getLastSolverWarning(): string;
    getPressures(): Float64Array;
    getRateHistory(): any;
    /**
     * Get dissolved gas ratio array [m3/m3]
     */
    getRs(): Float64Array;
    /**
     * Get gas saturation array (zeros when running in 2-phase mode)
     */
    getSatGas(): Float64Array;
    getSatOil(): Float64Array;
    getSatWater(): Float64Array;
    getWellState(): any;
    get_time(): number;
    /**
     * Load the entire state to continue simulation without re-computing from step 0
     */
    loadState(time_days: number, grid_state: any, well_state: any, rate_history: any): void;
    /**
     * Create a new reservoir simulator with oil-field units
     * Grid dimensions: nx, ny, nz (number of cells in each direction)
     * All parameters use: Pressure [bar], Distance [m], Time [day], Permeability [mD], Viscosity [cP]
     */
    constructor(nx: number, ny: number, nz: number, porosity_val: number);
    setCapillaryParams(p_entry: number, lambda: number): void;
    setCellDimensions(dx: number, dy: number, dz: number): void;
    /**
     * Set cell dimensions with per-layer thickness in the z-direction.
     * `dz_per_layer` must have length equal to nz.
     */
    setCellDimensionsPerLayer(dx: number, dy: number, dz_per_layer: Float64Array): void;
    setFluidCompressibilities(c_o: number, c_w: number): void;
    setFluidDensities(rho_o: number, rho_w: number): void;
    setFluidProperties(mu_o: number, mu_w: number): void;
    /**
     * Set gas fluid properties for three-phase mode
     */
    setGasFluidProperties(mu_g: number, c_g: number, rho_g: number): void;
    /**
     * Set gas-oil capillary pressure parameters (Brooks-Corey form)
     */
    setGasOilCapillaryParams(p_entry: number, lambda: number): void;
    setGasRedissolutionEnabled(enabled: boolean): void;
    setGravityEnabled(enabled: boolean): void;
    setInitialGasSaturation(sat_gas: number): void;
    /**
     * Set initial gas saturation per z-layer (three-phase mode)
     */
    setInitialGasSaturationPerLayer(sg: Float64Array): void;
    /**
     * Set initial pressure for all grid cells
     */
    setInitialPressure(pressure: number): void;
    /**
     * Override the dissolved-gas ratio for all cells with a uniform value.
     * Must be called **after** `setPvtTable` so the Rs array already exists.
     * This is used when the reservoir starts undersaturated (Rs < Rs_sat at initial P),
     * e.g. SPE1 Case 1 whose RSVD table specifies a constant Rs below saturation.
     */
    setInitialRs(rs: number): void;
    /**
     * Set initial water saturation for all grid cells
     */
    setInitialSaturation(sat_water: number): void;
    /**
     * Set initial water saturation per z-layer
     */
    setInitialSaturationPerLayer(sw: Float64Array): void;
    /**
     * Set injected fluid type for three-phase mode: "water" or "gas"
     */
    setInjectedFluid(fluid: string): void;
    setInjectorEnabled(enabled: boolean): void;
    /**
     * Set permeability per layer
     */
    setPermeabilityPerLayer(perms_x: Float64Array, perms_y: Float64Array, perms_z: Float64Array): void;
    /**
     * Set permeability with random distribution
     */
    setPermeabilityRandom(min_perm: number, max_perm: number): void;
    /**
     * Set permeability with deterministic random distribution using a fixed seed
     */
    setPermeabilityRandomSeeded(min_perm: number, max_perm: number, seed: bigint): void;
    /**
     * Set initial gas saturation for all grid cells (three-phase mode)
     */
    setPvtTable(table_js: any): void;
    setRateControlledWells(enabled: boolean): void;
    /**
     * Set relative permeability properties
     */
    setRelPermProps(s_wc: number, s_or: number, n_w: number, n_o: number, k_rw_max: number, k_ro_max: number): void;
    setRockProperties(c_r: number, depth_reference_m: number, b_o: number, b_w: number): void;
    /**
     * Set stability parameters for the simulation
     */
    setStabilityParams(max_sat_change_per_step: number, max_pressure_change_per_step: number, max_well_rate_change_fraction: number): void;
    setTargetWellRates(injector_rate_m3_day: number, producer_rate_m3_day: number): void;
    setTargetWellSurfaceRates(injector_rate_m3_day: number, producer_rate_m3_day: number): void;
    /**
     * Enable or disable the three-phase simulation mode
     */
    setThreePhaseModeEnabled(enabled: boolean): void;
    /**
     * Set three-phase relative permeability parameters (Stone II model).
     * `s_org` is the residual oil saturation in a gas flood (typically ≥ `s_or`).
     * It is distinct from `s_gr` (trapped residual gas) and is used as the terminal
     * oil saturation in `k_ro_gas` and gas-oil capillary pressure.
     */
    setThreePhaseRelPermProps(s_wc: number, s_or: number, s_gc: number, s_gr: number, s_org: number, n_w: number, n_o: number, n_g: number, k_rw_max: number, k_ro_max: number, k_rg_max: number): void;
    setThreePhaseScalTables(table_js: any): void;
    setWellBhpLimits(bhp_min: number, bhp_max: number): void;
    setWellControlModes(injector_mode: string, producer_mode: string): void;
    /**
     * Advance simulator by target timestep [days]
     */
    step(target_dt_days: number): void;
    /**
     * Cumulative water material balance error [m³]
     */
    cumulative_mb_error_m3: number;
    /**
     * Cumulative gas material balance error [Sm³] for total gas inventory
     * (free gas + dissolved gas) in three-phase mode.
     */
    cumulative_mb_gas_error_m3: number;
}

export function set_panic_hook(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_get_reservoirsimulator_cumulative_mb_error_m3: (a: number) => number;
    readonly __wbg_get_reservoirsimulator_cumulative_mb_gas_error_m3: (a: number) => number;
    readonly __wbg_reservoirsimulator_free: (a: number, b: number) => void;
    readonly __wbg_set_reservoirsimulator_cumulative_mb_error_m3: (a: number, b: number) => void;
    readonly __wbg_set_reservoirsimulator_cumulative_mb_gas_error_m3: (a: number, b: number) => void;
    readonly reservoirsimulator_add_well: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number];
    readonly reservoirsimulator_getDimensions: (a: number) => any;
    readonly reservoirsimulator_getLastSolverWarning: (a: number) => [number, number];
    readonly reservoirsimulator_getPressures: (a: number) => [number, number];
    readonly reservoirsimulator_getRateHistory: (a: number) => any;
    readonly reservoirsimulator_getRs: (a: number) => [number, number];
    readonly reservoirsimulator_getSatGas: (a: number) => [number, number];
    readonly reservoirsimulator_getSatOil: (a: number) => [number, number];
    readonly reservoirsimulator_getSatWater: (a: number) => [number, number];
    readonly reservoirsimulator_getWellState: (a: number) => any;
    readonly reservoirsimulator_get_time: (a: number) => number;
    readonly reservoirsimulator_loadState: (a: number, b: number, c: any, d: any, e: any) => [number, number];
    readonly reservoirsimulator_new: (a: number, b: number, c: number, d: number) => number;
    readonly reservoirsimulator_setCapillaryParams: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setCellDimensions: (a: number, b: number, c: number, d: number) => [number, number];
    readonly reservoirsimulator_setCellDimensionsPerLayer: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly reservoirsimulator_setFluidCompressibilities: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setFluidDensities: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setFluidProperties: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setGasFluidProperties: (a: number, b: number, c: number, d: number) => [number, number];
    readonly reservoirsimulator_setGasOilCapillaryParams: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setGasRedissolutionEnabled: (a: number, b: number) => void;
    readonly reservoirsimulator_setGravityEnabled: (a: number, b: number) => void;
    readonly reservoirsimulator_setInitialGasSaturation: (a: number, b: number) => void;
    readonly reservoirsimulator_setInitialGasSaturationPerLayer: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setInitialPressure: (a: number, b: number) => void;
    readonly reservoirsimulator_setInitialRs: (a: number, b: number) => void;
    readonly reservoirsimulator_setInitialSaturation: (a: number, b: number) => void;
    readonly reservoirsimulator_setInitialSaturationPerLayer: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setInjectedFluid: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setInjectorEnabled: (a: number, b: number) => void;
    readonly reservoirsimulator_setPermeabilityPerLayer: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly reservoirsimulator_setPermeabilityRandom: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setPermeabilityRandomSeeded: (a: number, b: number, c: number, d: bigint) => [number, number];
    readonly reservoirsimulator_setPvtTable: (a: number, b: any) => [number, number];
    readonly reservoirsimulator_setRateControlledWells: (a: number, b: number) => void;
    readonly reservoirsimulator_setRelPermProps: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly reservoirsimulator_setRockProperties: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly reservoirsimulator_setStabilityParams: (a: number, b: number, c: number, d: number) => void;
    readonly reservoirsimulator_setTargetWellRates: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setTargetWellSurfaceRates: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setThreePhaseModeEnabled: (a: number, b: number) => void;
    readonly reservoirsimulator_setThreePhaseRelPermProps: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => [number, number];
    readonly reservoirsimulator_setThreePhaseScalTables: (a: number, b: any) => [number, number];
    readonly reservoirsimulator_setWellBhpLimits: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setWellControlModes: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly reservoirsimulator_step: (a: number, b: number) => void;
    readonly set_panic_hook: () => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
