import type {
  SimulatorCreatePayload,
  SimulatorWellCompletion,
  SimulatorWellDefinition,
  SimulatorWellSchedule,
  ThreePhaseScalTables,
} from './simulator-types'

function toFiniteNumber(value: unknown, fallback: number): number {
  const numeric = Number(value)
  return Number.isFinite(numeric) ? numeric : fallback
}

function toMin(value: unknown, min: number, fallback: number): number {
  return Math.max(min, toFiniteNumber(value, fallback))
}

function toClamped(value: unknown, min: number, max: number, fallback: number): number {
  return Math.min(max, Math.max(min, toFiniteNumber(value, fallback)))
}

function toIntMin(value: unknown, min: number, fallback: number): number {
  return Math.max(min, Math.round(toFiniteNumber(value, fallback)))
}

function toIntRange(value: unknown, min: number, max: number, fallback: number): number {
  return Math.min(max, Math.max(min, Math.round(toFiniteNumber(value, fallback))))
}

function defaultProducerJForGrid(ny: number): number {
  return Math.max(0, ny - 1)
}

function normalizeLayerArray(values: unknown, fallback: number, length: number): number[] {
  if (!Array.isArray(values)) {
    return Array.from({ length }, () => fallback)
  }
  return Array.from({ length }, (_, index) => {
    const value = toFiniteNumber(values[index], fallback)
    return value > 0 ? value : fallback
  })
}

function cloneScalTables(value: unknown): ThreePhaseScalTables | undefined {
  if (!value || typeof value !== 'object') {
    return undefined
  }
  const tables = value as ThreePhaseScalTables
  if (!Array.isArray(tables.swof) || !Array.isArray(tables.sgof)) {
    return undefined
  }
  return {
    swof: tables.swof.map((row) => ({ ...row })),
    sgof: tables.sgof.map((row) => ({ ...row })),
  }
}

function normalizeCompletionLayers(values: unknown, nz: number): number[] {
  if (!Array.isArray(values) || values.length === 0) {
    return Array.from({ length: nz }, (_, index) => index)
  }

  const seen = new Set<number>()
  const normalized: number[] = []
  for (const raw of values) {
    const layer = toIntRange(raw, 0, nz - 1, 0)
    if (seen.has(layer)) continue
    seen.add(layer)
    normalized.push(layer)
  }
  return normalized
}

function buildLegacyWellDefinitions(input: {
  nx: number;
  ny: number;
  nz: number;
  injectorI: number;
  injectorJ: number;
  producerI: number;
  producerJ: number;
  producerKLayers?: unknown;
  injectorKLayers?: unknown;
  well_radius: number;
  well_skin: number;
  producerBhp: number;
  injectorBhp: number;
  injectorEnabled: boolean;
  injectorControlMode: 'pressure' | 'rate';
  producerControlMode: 'pressure' | 'rate';
  targetInjectorRate: number;
  targetProducerRate: number;
  targetInjectorSurfaceRate: number;
  targetProducerSurfaceRate: number;
  bhpMin?: number;
  bhpMax?: number;
}): SimulatorWellDefinition[] {
  const producerCompletions: SimulatorWellCompletion[] = normalizeCompletionLayers(
    input.producerKLayers,
    input.nz,
  ).map((k) => ({ i: input.producerI, j: input.producerJ, k }))

  const wells: SimulatorWellDefinition[] = [
    {
      id: 'producer-main',
      injector: false,
      bhp: input.producerBhp,
      wellRadius: input.well_radius,
      skin: input.well_skin,
      completions: producerCompletions,
      schedule: {
        controlMode: input.producerControlMode,
        targetRate: input.targetProducerRate,
        targetSurfaceRate: input.targetProducerSurfaceRate,
        bhpLimit: input.bhpMin,
        enabled: true,
      } satisfies SimulatorWellSchedule,
    },
  ]

  if (input.injectorEnabled) {
    const injectorCompletions: SimulatorWellCompletion[] = normalizeCompletionLayers(
      input.injectorKLayers,
      input.nz,
    ).map((k) => ({ i: input.injectorI, j: input.injectorJ, k }))

    wells.push({
      id: 'injector-main',
      injector: true,
      bhp: input.injectorBhp,
      wellRadius: input.well_radius,
      skin: input.well_skin,
      completions: injectorCompletions,
      schedule: {
        controlMode: input.injectorControlMode,
        targetRate: input.targetInjectorRate,
        targetSurfaceRate: input.targetInjectorSurfaceRate,
        bhpLimit: input.bhpMax,
        enabled: input.injectorEnabled,
      } satisfies SimulatorWellSchedule,
    })
  }

  return wells
}

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
  const nx = toIntMin(state.nx, 1, 1)
  const ny = toIntMin(state.ny, 1, 1)
  const nz = toIntMin(state.nz, 1, 1)
  const useUniformPerm = state.permMode === 'uniform'

  const uniformPermX = toMin(state.uniformPermX, 0.001, 100)
  const uniformPermY = toMin(state.uniformPermY, 0.001, 100)
  const uniformPermZ = toMin(state.uniformPermZ, 0.001, 10)

  const permsX = useUniformPerm
    ? Array.from({ length: nz }, () => uniformPermX)
    : normalizeLayerArray(state.layerPermsX, uniformPermX, nz)
  const permsY = useUniformPerm
    ? Array.from({ length: nz }, () => uniformPermY)
    : normalizeLayerArray(state.layerPermsY, uniformPermY, nz)
  const permsZ = useUniformPerm
    ? Array.from({ length: nz }, () => uniformPermZ)
    : normalizeLayerArray(state.layerPermsZ, uniformPermZ, nz)

  const cellDx = toMin(state.cellDx, 0.1, 10)
  const cellDy = toMin(state.cellDy, 0.1, 10)
  const cellDz = toMin(state.cellDz, 0.1, 1)

  const injectorControlMode = state.injectorControlMode === 'rate' ? 'rate' : 'pressure'
  const producerControlMode = state.producerControlMode === 'rate' ? 'rate' : 'pressure'

  const minPerm = toMin(state.minPerm, 0, 50)
  const maxPerm = toMin(state.maxPerm, minPerm, 200)

  const injectorI = toIntRange(state.injectorI, 0, nx - 1, 0)
  const injectorJ = toIntRange(state.injectorJ, 0, ny - 1, 0)
  const producerI = toIntRange(state.producerI, 0, nx - 1, nx - 1)
  const producerJ = toIntRange(state.producerJ, 0, ny - 1, defaultProducerJForGrid(ny))
  const well_radius = toMin(state.well_radius, 0.0001, 0.1)
  const well_skin = toFiniteNumber(state.well_skin, 0)
  const injectorBhp = toMin(state.injectorBhp, 0.1, 500)
  const producerBhp = toMin(state.producerBhp, 0.1, 100)
  const targetInjectorRate = toMin(state.targetInjectorRate, 0, 350)
  const targetProducerRate = toMin(state.targetProducerRate, 0, 350)
  const targetInjectorSurfaceRate = toMin(state.targetInjectorSurfaceRate, 0, 0)
  const targetProducerSurfaceRate = toMin(state.targetProducerSurfaceRate, 0, 0)
  const injectorEnabled = Boolean(state.injectorEnabled ?? true)
  const wells = Array.isArray(state.wells)
    ? state.wells.map((well) => ({
      id: String(well.id),
      injector: Boolean(well.injector),
      bhp: toFiniteNumber(well.bhp, Boolean(well.injector) ? injectorBhp : producerBhp),
      wellRadius: toMin(well.wellRadius, 0.0001, well_radius),
      skin: toFiniteNumber(well.skin, well_skin),
      completions: Array.isArray(well.completions)
        ? well.completions.map((completion) => ({
          i: toIntRange(completion.i, 0, nx - 1, 0),
          j: toIntRange(completion.j, 0, ny - 1, 0),
          k: toIntRange(completion.k, 0, nz - 1, 0),
        }))
        : [],
      schedule: well.schedule
        ? {
          controlMode: well.schedule.controlMode === 'rate' ? 'rate' : 'pressure',
          targetRate: well.schedule.targetRate != null ? toMin(well.schedule.targetRate, 0, 0) : undefined,
          targetSurfaceRate: well.schedule.targetSurfaceRate != null
            ? toMin(well.schedule.targetSurfaceRate, 0, 0)
            : undefined,
          bhpLimit: well.schedule.bhpLimit != null ? toFiniteNumber(well.schedule.bhpLimit, 0) : undefined,
          enabled: well.schedule.enabled !== false,
        }
        : undefined,
    }))
    : buildLegacyWellDefinitions({
      nx,
      ny,
      nz,
      injectorI,
      injectorJ,
      producerI,
      producerJ,
      producerKLayers: state.producerKLayers,
      injectorKLayers: state.injectorKLayers,
      well_radius,
      well_skin,
      producerBhp,
      injectorBhp,
      injectorEnabled,
      injectorControlMode,
      producerControlMode,
      targetInjectorRate,
      targetProducerRate,
      targetInjectorSurfaceRate,
      targetProducerSurfaceRate,
      bhpMin: state.bhpMin != null ? toFiniteNumber(state.bhpMin, producerBhp) : undefined,
      bhpMax: state.bhpMax != null ? toFiniteNumber(state.bhpMax, injectorBhp) : undefined,
    })

  return {
    nx,
    ny,
    nz: nz,

    cellDx,
    cellDy,
    cellDz,

    initialPressure: toFiniteNumber(state.initialPressure, 300),
    initialSaturation: toClamped(state.initialSaturation, 0, 1, 0.3),
    porosity: toClamped(state.porosity, 0.01, 1, 0.2),

    mu_w: toMin(state.mu_w, 0.01, 0.5),
    mu_o: toMin(state.mu_o, 0.01, 1.0),
    c_o: toMin(state.c_o, 0, 1e-5),
    c_w: toMin(state.c_w, 0, 3e-6),
    rho_w: toMin(state.rho_w, 1, 1000),
    rho_o: toMin(state.rho_o, 1, 800),

    rock_compressibility: toMin(state.rock_compressibility, 0, 1e-6),
    depth_reference: toFiniteNumber(state.depth_reference, 0),
    volume_expansion_o: toMin(state.volume_expansion_o, 0.01, 1.0),
    volume_expansion_w: toMin(state.volume_expansion_w, 0.01, 1.0),

    s_wc: toClamped(state.s_wc, 0, 1, 0.1),
    s_or: toClamped(state.s_or, 0, 1, 0.1),
    n_w: toMin(state.n_w, 0.01, 2),
    n_o: toMin(state.n_o, 0.01, 2),
    k_rw_max: toClamped(state.k_rw_max, 0, 1, 1.0),
    k_ro_max: toClamped(state.k_ro_max, 0.01, 1, 1.0),

    max_sat_change_per_step: toClamped(state.max_sat_change_per_step, 0.01, 1, 0.1),
    max_pressure_change_per_step: toMin(state.max_pressure_change_per_step, 1, 75),
    max_well_rate_change_fraction: toMin(state.max_well_rate_change_fraction, 0.01, 0.75),

    capillaryEnabled: Boolean(state.capillaryEnabled ?? false),
    capillaryPEntry: toMin(state.capillaryPEntry, 0, 0),
    capillaryLambda: toMin(state.capillaryLambda, 0, 2),

    gravityEnabled: Boolean(state.gravityEnabled ?? false),

    permMode: String(state.permMode ?? 'uniform'),
    minPerm,
    maxPerm,
    useRandomSeed: Boolean(state.useRandomSeed ?? false),
    randomSeed: toFiniteNumber(state.randomSeed, 0),
    permsX,
    permsY,
    permsZ,

    well_radius,
    well_skin,
    injectorBhp,
    producerBhp,

    rateControlledWells: Boolean(state.rateControlledWells ?? false),
    injectorControlMode,
    producerControlMode,
    injectorEnabled,
    targetInjectorRate,
    targetProducerRate,
    targetInjectorSurfaceRate,
    targetProducerSurfaceRate,
    injectorI,
    injectorJ,
    producerI,
    producerJ,

    // Three-phase
    s_gc:    toClamped(state.s_gc,    0, 1,    0.05),
    s_gr:    toClamped(state.s_gr,    0, 1,    0.05),
    s_org:   toClamped(state.s_org,   0, 1,    0.15),
    n_g:     toMin(state.n_g,         0.01,    1.5),
    k_rg_max: toClamped(state.k_rg_max, 0.01, 1, 1.0),
    pcogEnabled: Boolean(state.pcogEnabled ?? false),
    pcogPEntry:  toMin(state.pcogPEntry, 0, 0),
    pcogLambda:  toMin(state.pcogLambda, 0, 2),
    mu_g:  toMin(state.mu_g,  0.001, 0.02),
    c_g:   toMin(state.c_g,   0,     1e-4),
    rho_g: toMin(state.rho_g, 0.1,   10.0),
    threePhaseModeEnabled: Boolean(state.threePhaseModeEnabled ?? false),
    injectedFluid: (state.injectedFluid ?? 'gas') as 'water' | 'gas',
    initialGasSaturation: toClamped(state.initialGasSaturation, 0, 1, 0),

    // Per-layer overrides — spread into fresh arrays to strip Svelte 5 reactive
    // proxies that cannot be structured-cloned by postMessage.
    initialSaturationPerLayer: Array.isArray(state.initialSaturationPerLayer) ? [...state.initialSaturationPerLayer] : undefined,
    initialGasSaturationPerLayer: Array.isArray(state.initialGasSaturationPerLayer) ? [...state.initialGasSaturationPerLayer] : undefined,
    cellDzPerLayer: Array.isArray(state.cellDzPerLayer) ? [...state.cellDzPerLayer] : undefined,
    // Per-layer well completions
    producerKLayers: Array.isArray(state.producerKLayers) ? [...state.producerKLayers] : undefined,
    injectorKLayers: Array.isArray(state.injectorKLayers) ? [...state.injectorKLayers] : undefined,
    wells,

    pvtMode: state.pvtMode === 'black-oil' ? 'black-oil' : 'constant',
    pvtTable: state.pvtMode === 'black-oil' && Array.isArray(state.pvtTable)
      ? state.pvtTable.map((row: any) => ({ ...row }))
      : undefined,
    scalTables: cloneScalTables(state.scalTables),
    gasRedissolutionEnabled: state.gasRedissolutionEnabled !== false,
    initialRs: typeof state.initialRs === 'number' ? state.initialRs : undefined,
  }
}
