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
