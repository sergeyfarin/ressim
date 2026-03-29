/* tslint:disable */
/* eslint-disable */

export class ReservoirSimulator {
    free(): void;
    [Symbol.dispose](): void;
    addWellWithId(i: number, j: number, k: number, bhp: number, well_radius: number, skin: number, injector: boolean, physical_well_id: string): void;
    add_well(i: number, j: number, k: number, bhp: number, well_radius: number, skin: number, injector: boolean): void;
    getDimensions(): any;
    getFimTrace(): string;
    getLastSolverWarning(): string;
    getPressures(): Float64Array;
    getRateHistory(): any;
    getRs(): Float64Array;
    getSatGas(): Float64Array;
    getSatOil(): Float64Array;
    getSatWater(): Float64Array;
    getWellState(): any;
    get_time(): number;
    loadState(time_days: number, grid_state: any, well_state: any, rate_history: any): void;
    /**
     * Create a new reservoir simulator with oil-field units
     * Grid dimensions: nx, ny, nz (number of cells in each direction)
     * All parameters use: Pressure [bar], Distance [m], Time [day], Permeability [mD], Viscosity [cP]
     */
    constructor(nx: number, ny: number, nz: number, porosity_val: number);
    setCapillaryParams(p_entry: number, lambda: number): void;
    setCellDimensions(dx: number, dy: number, dz: number): void;
    setCellDimensionsPerLayer(dx: number, dy: number, dz_per_layer: Float64Array): void;
    setFimEnabled(enabled: boolean): void;
    setFluidCompressibilities(c_o: number, c_w: number): void;
    setFluidDensities(rho_o: number, rho_w: number): void;
    setFluidProperties(mu_o: number, mu_w: number): void;
    setGasFluidProperties(mu_g: number, c_g: number, rho_g: number): void;
    setGasOilCapillaryParams(p_entry: number, lambda: number): void;
    setGasRedissolutionEnabled(enabled: boolean): void;
    setGravityEnabled(enabled: boolean): void;
    setInitialGasSaturation(sat_gas: number): void;
    setInitialGasSaturationPerLayer(sg: Float64Array): void;
    setInitialPressure(pressure: number): void;
    setInitialRs(rs: number): void;
    setInitialSaturation(sat_water: number): void;
    setInitialSaturationPerLayer(sw: Float64Array): void;
    setInjectedFluid(fluid: string): void;
    setInjectorEnabled(enabled: boolean): void;
    setPermeabilityPerLayer(perms_x: Float64Array, perms_y: Float64Array, perms_z: Float64Array): void;
    setPermeabilityRandom(min_perm: number, max_perm: number): void;
    setPermeabilityRandomSeeded(min_perm: number, max_perm: number, seed: bigint): void;
    setPvtTable(table_js: any): void;
    setRateControlledWells(enabled: boolean): void;
    setRelPermProps(s_wc: number, s_or: number, n_w: number, n_o: number, k_rw_max: number, k_ro_max: number): void;
    setRockProperties(c_r: number, depth_reference_m: number, b_o: number, b_w: number): void;
    setStabilityParams(max_sat_change_per_step: number, max_pressure_change_per_step: number, max_well_rate_change_fraction: number): void;
    setTargetWellRates(injector_rate_m3_day: number, producer_rate_m3_day: number): void;
    setTargetWellSurfaceRates(injector_rate_m3_day: number, producer_rate_m3_day: number): void;
    setThreePhaseModeEnabled(enabled: boolean): void;
    setThreePhaseRelPermProps(s_wc: number, s_or: number, s_gc: number, s_gr: number, s_org: number, n_w: number, n_o: number, n_g: number, k_rw_max: number, k_ro_max: number, k_rg_max: number): void;
    setThreePhaseScalTables(table_js: any): void;
    setWellBhpLimits(bhp_min: number, bhp_max: number): void;
    setWellControlModes(injector_mode: string, producer_mode: string): void;
    setWellSchedule(physical_well_id: string, control_mode: string, target_rate_m3_day: number, target_surface_rate_m3_day: number, bhp_limit: number, enabled: boolean): void;
    step(target_dt_days: number): void;
    stepWithDiagnostics(target_dt_days: number): string;
    cumulative_mb_error_m3: number;
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
    readonly set_panic_hook: () => void;
    readonly reservoirsimulator_addWellWithId: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number];
    readonly reservoirsimulator_add_well: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number];
    readonly reservoirsimulator_getDimensions: (a: number) => any;
    readonly reservoirsimulator_getFimTrace: (a: number) => [number, number];
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
    readonly reservoirsimulator_setFimEnabled: (a: number, b: number) => void;
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
    readonly reservoirsimulator_setWellSchedule: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number];
    readonly reservoirsimulator_step: (a: number, b: number) => void;
    readonly reservoirsimulator_stepWithDiagnostics: (a: number, b: number) => [number, number];
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
