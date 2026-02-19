/**
 * Types for messages and payloads exchanged with the simulation WebWorker
 * and for the simulator "create" payload constructed by the UI.
 */

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

  // fluid properties
  mu_w: number;
  mu_o: number;
  c_o: number;
  c_w: number;
  rho_w: number;
  rho_o: number;

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

  // wells / controls
  well_radius: number;
  well_skin: number;
  injectorBhp: number;
  producerBhp: number;
  rateControlledWells?: boolean;
  injectorControlMode?: string;
  producerControlMode?: string;
  injectorEnabled?: boolean;
  targetInjectorRate?: number;
  targetProducerRate?: number;
  injectorI?: number;
  injectorJ?: number;
  producerI?: number;
  producerJ?: number;
}

/** Payload sent with the `run` message */
export interface WorkerRunPayload {
  steps: number;
  deltaTDays: number;
  historyInterval?: number;
  chunkYieldInterval?: number;
}

/** Payload for pre-run hydration */
export interface HydratePreRunPayload {
  createPayload: SimulatorCreatePayload;
  steps: number;
  deltaTDays: number;
  hydrationId?: number | string;
}

// --- Simulator runtime / output shapes ---

export interface GridCell {
  // primary values used by UI
  pressure?: number;
  sat_water?: number; // wasm -> snake_case
  satWater?: number; // JS/camelCase aliases
  sw?: number; // sometimes 'sw' is used
  permeability_x?: number;
  permeability_y?: number;
  permeability_z?: number;
  porosity?: number;

  // optional cell indices (some outputs include i/j/k)
  i?: number;
  j?: number;
  k?: number;

  // catch-all for additional simulator-provided properties
  [key: string]: unknown;
}

export interface WellStateEntry {
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
  // additional fields produced by the simulator may exist
  [key: string]: unknown;
}

export interface SimulatorSnapshot {
  grid: GridCell[];
  wells: WellState;
  time: number;
  rateHistory?: RateHistoryPoint[];
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
}

// Worker -> UI messages
export interface WorkerReadyMessage { type: 'ready' }
export interface WorkerStateMessage { type: 'state'; data: SimulatorSnapshot }
export interface WorkerRunStartedMessage { type: 'runStarted'; steps?: number; deltaTDays?: number; hydration?: boolean }
export interface WorkerStoppedMessage { type: 'stopped'; reason?: string; completedSteps?: number; hydration?: boolean }
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
