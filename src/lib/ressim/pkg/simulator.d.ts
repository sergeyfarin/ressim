/* tslint:disable */
/* eslint-disable */

export class ReservoirSimulator {
    free(): void;
    [Symbol.dispose](): void;
    addWellWithId(i: number, j: number, k: number, bhp: number, well_radius: number, skin: number, injector: boolean, physical_well_id: string): void;
    add_well(i: number, j: number, k: number, bhp: number, well_radius: number, skin: number, injector: boolean): void;
    getDimensions(): any;
    getFimTrace(): string;
    getGridState(): any;
    getLastSolverWarning(): string;
    getPressures(): Float64Array;
    getRateHistory(): any;
    getRateHistorySince(start_index: number): any;
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
    /**
     * Configure sweep efficiency diagnostics to be computed every step.
     * Accepts a JSON object matching `SweepConfig`: `{ geometry, swept_threshold,
     * initial_oil_saturation, residual_oil_saturation }`.
     * Pass `null`/`undefined` to disable sweep computation.
     */
    setSweepConfig(config_js: any): void;
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
