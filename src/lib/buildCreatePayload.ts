import type { SimulatorCreatePayload } from './simulator-types'

/**
 * Build a SimulatorCreatePayload from plain UI state. Kept pure so it can be
 * unit-tested and type-checked independently of the Svelte component.
 */
export function buildCreatePayloadFromState(state: Partial<SimulatorCreatePayload> & {
  // UI helpers referenced by the original implementation
  permMode?: string;
  uniformPermX?: number;
  uniformPermY?: number;
  uniformPermZ?: number;
  layerPermsX?: number[];
  layerPermsY?: number[];
  layerPermsZ?: number[];
  nz?: number;
}): SimulatorCreatePayload {
  const useUniformPerm = state.permMode === 'uniform'
  const nz = Number(state.nz ?? 1)
  const permsX = useUniformPerm ? Array.from({ length: nz }, () => Number(state.uniformPermX ?? 0)) : (state.layerPermsX ?? []).map(Number)
  const permsY = useUniformPerm ? Array.from({ length: nz }, () => Number(state.uniformPermY ?? 0)) : (state.layerPermsY ?? []).map(Number)
  const permsZ = useUniformPerm ? Array.from({ length: nz }, () => Number(state.uniformPermZ ?? 0)) : (state.layerPermsZ ?? []).map(Number)

  return {
    nx: Number(state.nx ?? 1),
    ny: Number(state.ny ?? 1),
    nz: nz,

    cellDx: Number(state.cellDx ?? 1),
    cellDy: Number(state.cellDy ?? 1),
    cellDz: Number(state.cellDz ?? 1),

    initialPressure: Number(state.initialPressure ?? 0),
    initialSaturation: Number(state.initialSaturation ?? 0),

    mu_w: Number(state.mu_w ?? 1),
    mu_o: Number(state.mu_o ?? 1),
    c_o: Number(state.c_o ?? 0),
    c_w: Number(state.c_w ?? 0),
    rho_w: Number(state.rho_w ?? 1000),
    rho_o: Number(state.rho_o ?? 800),

    rock_compressibility: Number(state.rock_compressibility ?? 0),
    depth_reference: Number(state.depth_reference ?? 0),
    volume_expansion_o: Number(state.volume_expansion_o ?? 1),
    volume_expansion_w: Number(state.volume_expansion_w ?? 1),

    s_wc: Number(state.s_wc ?? 0),
    s_or: Number(state.s_or ?? 0),
    n_w: Number(state.n_w ?? 2),
    n_o: Number(state.n_o ?? 2),

    max_sat_change_per_step: Number(state.max_sat_change_per_step ?? 1),
    max_pressure_change_per_step: Number(state.max_pressure_change_per_step ?? 1),
    max_well_rate_change_fraction: Number(state.max_well_rate_change_fraction ?? 1),

    capillaryEnabled: Boolean(state.capillaryEnabled ?? false),
    capillaryPEntry: Number(state.capillaryPEntry ?? 0),
    capillaryLambda: Number(state.capillaryLambda ?? 0),

    gravityEnabled: Boolean(state.gravityEnabled ?? false),

    permMode: String(state.permMode ?? 'uniform'),
    minPerm: Number(state.minPerm ?? 0),
    maxPerm: Number(state.maxPerm ?? 0),
    useRandomSeed: Boolean(state.useRandomSeed ?? false),
    randomSeed: state.randomSeed ?? 0,
    permsX,
    permsY,
    permsZ,

    well_radius: Number(state.well_radius ?? 0),
    well_skin: Number(state.well_skin ?? 0),
    injectorBhp: Number(state.injectorBhp ?? 100),
    producerBhp: Number(state.producerBhp ?? 100),

    rateControlledWells: Boolean(state.rateControlledWells ?? false),
    injectorControlMode: state.injectorControlMode ?? 'pressure',
    producerControlMode: state.producerControlMode ?? 'pressure',
    injectorEnabled: Boolean(state.injectorEnabled ?? true),
    targetInjectorRate: Number(state.targetInjectorRate ?? 0),
    targetProducerRate: Number(state.targetProducerRate ?? 0),
    injectorI: Number(state.injectorI ?? 0),
    injectorJ: Number(state.injectorJ ?? 0),
    producerI: Number(state.producerI ?? 0),
    producerJ: Number(state.producerJ ?? 0),
  }
}
