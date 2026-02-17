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
    getGridState(): any;
    getRateHistory(): any;
    getWellState(): any;
    get_time(): number;
    /**
     * Create a new reservoir simulator with oil-field units
     * Grid dimensions: nx, ny, nz (number of cells in each direction)
     * All parameters use: Pressure [bar], Distance [m], Time [day], Permeability [mD], Viscosity [cP]
     */
    constructor(nx: number, ny: number, nz: number);
    setCapillaryParams(p_entry: number, lambda: number): void;
    setCellDimensions(dx: number, dy: number, dz: number): void;
    setFluidCompressibilities(c_o: number, c_w: number): void;
    setFluidDensities(rho_o: number, rho_w: number): void;
    setFluidProperties(mu_o: number, mu_w: number): void;
    setGravityEnabled(enabled: boolean): void;
    /**
     * Set initial pressure for all grid cells
     */
    setInitialPressure(pressure: number): void;
    /**
     * Set initial water saturation for all grid cells
     */
    setInitialSaturation(sat_water: number): void;
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
    setRateControlledWells(enabled: boolean): void;
    /**
     * Set relative permeability properties
     */
    setRelPermProps(s_wc: number, s_or: number, n_w: number, n_o: number): void;
    setRockProperties(c_r: number, depth_reference_m: number, b_o: number, b_w: number): void;
    /**
     * Set stability parameters for the simulation
     */
    setStabilityParams(max_sat_change_per_step: number, max_pressure_change_per_step: number, max_well_rate_change_fraction: number): void;
    setTargetWellRates(injector_rate_m3_day: number, producer_rate_m3_day: number): void;
    setWellControlModes(injector_mode: string, producer_mode: string): void;
    step(target_dt_days: number): void;
}

export function set_panic_hook(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_reservoirsimulator_free: (a: number, b: number) => void;
    readonly reservoirsimulator_add_well: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number];
    readonly reservoirsimulator_getDimensions: (a: number) => any;
    readonly reservoirsimulator_getGridState: (a: number) => any;
    readonly reservoirsimulator_getRateHistory: (a: number) => any;
    readonly reservoirsimulator_getWellState: (a: number) => any;
    readonly reservoirsimulator_get_time: (a: number) => number;
    readonly reservoirsimulator_new: (a: number, b: number, c: number) => number;
    readonly reservoirsimulator_setCapillaryParams: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setCellDimensions: (a: number, b: number, c: number, d: number) => [number, number];
    readonly reservoirsimulator_setFluidCompressibilities: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setFluidDensities: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setFluidProperties: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setGravityEnabled: (a: number, b: number) => void;
    readonly reservoirsimulator_setInitialPressure: (a: number, b: number) => void;
    readonly reservoirsimulator_setInitialSaturation: (a: number, b: number) => void;
    readonly reservoirsimulator_setInjectorEnabled: (a: number, b: number) => void;
    readonly reservoirsimulator_setPermeabilityPerLayer: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly reservoirsimulator_setPermeabilityRandom: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setPermeabilityRandomSeeded: (a: number, b: number, c: number, d: bigint) => [number, number];
    readonly reservoirsimulator_setRateControlledWells: (a: number, b: number) => void;
    readonly reservoirsimulator_setRelPermProps: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly reservoirsimulator_setRockProperties: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly reservoirsimulator_setStabilityParams: (a: number, b: number, c: number, d: number) => void;
    readonly reservoirsimulator_setTargetWellRates: (a: number, b: number, c: number) => [number, number];
    readonly reservoirsimulator_setWellControlModes: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly reservoirsimulator_step: (a: number, b: number) => void;
    readonly set_panic_hook: () => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
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
