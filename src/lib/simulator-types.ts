/**
 * Types for messages and payloads exchanged with the simulation WebWorker
 * and for the simulator "create" payload constructed by the UI.
 */

export interface PvtRow {
    p_bar: number;
    rs_m3m3: number;
    bo_m3m3: number;
    mu_o_cp: number;
    bg_m3m3: number;
    mu_g_cp: number;
}

export interface SwofRow {
  sw: number;
  krw: number;
  krow: number;
  pcow?: number;
}

export interface SgofRow {
  sg: number;
  krg: number;
  krog: number;
  pcog?: number;
}

export interface ThreePhaseScalTables {
  swof: SwofRow[];
  sgof: SgofRow[];
}

export interface SimulatorWellCompletion {
  i: number;
  j: number;
  k: number;
}

export interface SimulatorWellSchedule {
  controlMode?: 'pressure' | 'rate';
  targetRate?: number;
  targetSurfaceRate?: number;
  bhpLimit?: number;
  enabled?: boolean;
}

export interface SimulatorWellDefinition {
  id: string;
  injector: boolean;
  bhp: number;
  wellRadius: number;
  skin: number;
  completions: SimulatorWellCompletion[];
  schedule?: SimulatorWellSchedule;
}

/** Permutation mode for assigning permeability */
export type PermMode = 'uniform' | 'random' | 'perLayer' | string;

/** Minimal shape for the simulator creation payload (returned by buildCreatePayload). */
export interface SimulatorCreatePayload {
  nx: number;
  ny: number;
  nz: number;

  // cell geometry
  cellDx: number;
  cellDy: number;
  cellDz: number;

  // initial conditions
  initialPressure: number;
  initialSaturation: number;
  porosity: number;

  // fluid properties
  mu_w: number;
  mu_o: number;
  c_o: number;
  c_w: number;
  rho_w: number;
  rho_o: number;

  pvtMode?: 'constant' | 'black-oil';
  pvtTable?: PvtRow[];
  scalTables?: ThreePhaseScalTables;
  gasRedissolutionEnabled?: boolean;
  /** Initial dissolved-gas ratio [Sm³/Sm³]. When set, overrides the saturated-curve
   *  default so oil can start undersaturated (e.g. SPE1 RSVD). */
  initialRs?: number;

  // rock / compressibility
  rock_compressibility: number;
  depth_reference: number;
  volume_expansion_o: number;
  volume_expansion_w: number;

  // relative permeability / misc
  s_wc: number;
  s_or: number;
  n_w: number;
  n_o: number;
  k_rw_max: number;
  k_ro_max: number;

  // stability / timestep controls
  max_sat_change_per_step: number;
  max_pressure_change_per_step: number;
  max_well_rate_change_fraction: number;

  // capillary
  capillaryEnabled: boolean;
  capillaryPEntry: number;
  capillaryLambda: number;

  // gravity
  gravityEnabled: boolean;

  // permeability generation
  permMode: PermMode;
  minPerm: number;
  maxPerm: number;
  useRandomSeed?: boolean;
  /** may be numeric or bigint (wasm binding accepts bigint) */
  randomSeed?: number | bigint;
  permsX: number[];
  permsY: number[];
  permsZ: number[];
  /** Optional overrides for BHP limits */
  bhpMin?: number;
  bhpMax?: number;

  // ── Three-phase SCAL ────────────────────────────────────────────────────────
  s_gc?: number;
  s_gr?: number;
  /** Residual oil saturation in a gas flood (typically ≥ s_or). Distinct from s_gr. */
  s_org?: number;
  n_g?: number;
  k_rg_max?: number;
  // Gas-oil capillary pressure
  pcogEnabled?: boolean;
  pcogPEntry?: number;
  pcogLambda?: number;
  // Gas fluid properties
  mu_g?: number;
  c_g?: number;
  rho_g?: number;
  // Three-phase mode flags
  threePhaseModeEnabled?: boolean;
  injectedFluid?: 'water' | 'gas';
  // Initial gas saturation
  initialGasSaturation?: number;
  // Per-layer initial conditions (optional; override scalar values when present)
  initialSaturationPerLayer?: number[];
  initialGasSaturationPerLayer?: number[];
  // Per-layer cell thickness in z-direction (optional; overrides scalar cellDz)
  cellDzPerLayer?: number[];

  // wells / controls
  well_radius: number;
  well_skin: number;
  injectorBhp: number;
  producerBhp: number;
  rateControlledWells?: boolean;
  injectorControlMode?: 'pressure' | 'rate';
  producerControlMode?: 'pressure' | 'rate';
  injectorEnabled?: boolean;
  targetInjectorRate?: number;
  targetProducerRate?: number;
  targetInjectorSurfaceRate?: number;
  targetProducerSurfaceRate?: number;
  injectorI?: number;
  injectorJ?: number;
  producerI?: number;
  producerJ?: number;
  /** Layer indices for producer completions (default: all layers). */
  producerKLayers?: number[];
  /** Layer indices for injector completions (default: all layers). */
  injectorKLayers?: number[];
  /** Explicit physical wells with stable IDs and grouped completions. */
  wells?: SimulatorWellDefinition[];
  /** Whether the runtime should use the FIM step path for this simulator instance. */
  fimEnabled?: boolean;
  /**
   * When present, the simulator will compute sweep efficiency diagnostics every step.
   * Populated by buildCreatePayload when the scenario has showSweepPanel = true.
   */
  sweepConfig?: {
    /** 'areal', 'vertical', or 'both' */
    geometry: string;
    /** Water saturation threshold above which a cell is counted as swept. */
    swept_threshold: number;
    /** Initial oil saturation (= 1 - Swi) for mobile-oil-recovered calculation. */
    initial_oil_saturation: number;
    /** Residual oil saturation S_or. */
    residual_oil_saturation: number;
  };
}

/** Payload sent with the `run` message */
export interface WorkerRunPayload {
  steps: number;
  deltaTDays: number;
  historyInterval?: number;
  chunkYieldInterval?: number;
  history?: SimulatorSnapshot[];
  rateHistory?: RateHistory[];
}

// --- Simulator runtime / output shapes ---

export interface GridState {
  pressure: Float64Array;
  sat_water: Float64Array;
  sat_oil: Float64Array;
  sat_gas: Float64Array;
}

export interface WellStateEntry {
  physical_well_id?: string;
  schedule?: SimulatorWellSchedule;
  i: number;
  j: number;
  k: number;
  bhp?: number;
  injector?: boolean;
  rate_o?: number;
  rate_w?: number;
  well_radius?: number;
  skin?: number;
  [key: string]: unknown;
}

export type WellState = WellStateEntry[];

export interface RateHistoryPoint {
  time: number;
  total_production_oil?: number;
  total_production_liquid?: number;
  total_injection?: number;
  avg_reservoir_pressure?: number;
  avg_pressure?: number;
  avg_water_saturation?: number;
  total_production_gas?: number;
  avg_gas_saturation?: number;
  /** Producing GOR [Sm³/Sm³]: (free gas + dissolved gas) / oil at surface */
  producing_gor?: number;
  /** Fraction of rate-controlled producer completions clamped by BHP limits */
  producer_bhp_limited_fraction?: number;
  /** Fraction of rate-controlled injector completions clamped by BHP limits */
  injector_bhp_limited_fraction?: number;
  /** Sweep efficiency diagnostics (present when sweep config is set on the simulator) */
  sweep?: {
    /** Areal sweep efficiency [0-1]. Null for 'both' geometry. */
    e_a?: number;
    /** Vertical sweep efficiency [0-1]. Null for 'both' geometry. */
    e_v?: number;
    /** Volumetric sweep efficiency [0-1]. */
    e_vol: number;
    /** Fraction of initial mobile oil recovered [0-1]. Some only for 'both' geometry. */
    mobile_oil_recovered?: number;
  };
  // additional fields produced by the simulator may exist
  [key: string]: unknown;
}

export interface SimulatorSnapshot {
  grid: GridState;
  wells: WellState;
  time: number;
  rateHistory?: RateHistoryPoint[];
  rateHistoryDelta?: RateHistoryPoint[];
  solverWarning?: string | null;
  recordHistory?: boolean;
  stepIndex?: number;
  profile?: RunProfile;
}

/** Lightweight profile reported after run chunks */
export interface RunProfile {
  batchMs: number;
  avgStepMs: number;
  snapshotsSent: number;
  extractMs?: number;
}

export type RateHistory = RateHistoryPoint[];
export type SolverWarning = string | null;

/** Analytical production point used for analytical comparisons in the UI */
export interface AnalyticalProductionPoint {
  time: number;
  oilRate?: number;
  waterRate?: number;
  cumulativeOil?: number;
  cumulativeLiquid?: number;
  avgPressure?: number;
}

// Worker -> UI messages
export interface WorkerReadyMessage { type: 'ready' }
export interface WorkerStateMessage { type: 'state'; data: SimulatorSnapshot }
export interface WorkerRunStartedMessage { type: 'runStarted'; steps?: number; deltaTDays?: number; hydration?: boolean; hydrationId?: number | string }
export interface WorkerStoppedMessage { type: 'stopped'; reason?: string; completedSteps?: number; hydration?: boolean; hydrationId?: number | string }
export interface WorkerHydratedMessage { type: 'hydrated'; hydration: true; hydrationId?: number | string; time?: number; rateHistoryLength?: number }
export interface WorkerBatchCompleteMessage { type: 'batchComplete'; profile: RunProfile }
export interface WorkerErrorMessage { type: 'error'; message: string }
export interface WorkerWarningMessage { type: 'warning'; message: string }

export type WorkerMessage =
  | WorkerReadyMessage
  | WorkerStateMessage
  | WorkerRunStartedMessage
  | WorkerStoppedMessage
  | WorkerHydratedMessage
  | WorkerBatchCompleteMessage
  | WorkerErrorMessage
  | WorkerWarningMessage;
