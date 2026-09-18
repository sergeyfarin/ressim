/**
 * The compositional engine boundary, as TypeScript.
 *
 * These types mirror `src/lib/ressim/src/compositional/api.rs` exactly, including its `snake_case`
 * field names. That is deliberate: this is a **wire format**, not an idiomatic TS interface, and a
 * renaming layer between the two would be one more place for the two sides to drift apart. The
 * Rust side owns the schema; this file follows it.
 *
 * `CompositionalSimulator`'s generated bindings type every payload as `any`, because they cross as
 * `JsValue`. These declarations are what gives the frontend its types back.
 */

/** The only case schema this build accepts. Must match `api::CASE_SCHEMA`. */
export const COMPOSITIONAL_CASE_SCHEMA = 'ressim-compositional-case/1';
/** The only checkpoint schema this build accepts. Must match `api::CHECKPOINT_SCHEMA`. */
export const COMPOSITIONAL_CHECKPOINT_SCHEMA = 'ressim-compositional-checkpoint/1';

/**
 * The fluid, chosen by name.
 *
 * A payload cannot describe an equation of state field by field, and that is a deliberate
 * restriction rather than an unfinished one: C0 pinned these specifications against an external
 * oracle and C6 measured the pressure and composition range they are valid over. A scenario that
 * could supply its own critical properties could leave that range without anything noticing.
 */
export type CompositionalFluidChoice = 'pinned-ternary' | 'pinned-binary';

/** A uniform 1D column. The engine refuses anything else rather than flattening it. */
export interface CompositionalGridConfig {
  cells: number;
  dx_m: number;
  dy_m: number;
  dz_m: number;
  porosity: number;
  permeability_md: number;
  rock_reference_pressure_bar: number;
  rock_compressibility_per_bar: number;
}

/**
 * Relative permeability, which a case must choose explicitly.
 *
 * There is no default, and that is the point — see `docs/COMPOSITIONAL_VALIDATION.md` §6. `linear`
 * is admissible but is a **verification** assumption, and `validateCompositionalConfig` reports it
 * so a scenario-admission check can refuse to ship on it.
 */
export type CompositionalRelPermConfig =
  | { model: 'linear' }
  | {
      model: 'corey';
      liquid_residual: number;
      vapour_residual: number;
      liquid_exponent: number;
      vapour_exponent: number;
      liquid_endpoint: number;
      vapour_endpoint: number;
    }
  | {
      model: 'tabulated';
      liquid_saturation: number[];
      kr_liquid: number[];
      kr_vapour: number[];
    };

/** Which surface stream a volumetric rate target refers to. */
export type CompositionalSurfacePhase = 'liquid' | 'vapour' | 'total';

/**
 * A well control. The engine flattens the discriminator into the well object, so `control` sits
 * alongside `id` rather than under a nested key.
 */
export type CompositionalWellControl =
  | { control: 'bhp'; target_bar: number }
  | { control: 'molar-rate'; target_moles_per_day: number; bhp_limit_bar: number }
  | {
      control: 'surface-rate';
      target_m3_per_day: number;
      phase: CompositionalSurfacePhase;
      bhp_limit_bar: number;
    };

export interface CompositionalCompletion {
  cell: number;
  well_index: number;
  head_offset_bar?: number;
}

export type CompositionalWellConfig = CompositionalWellControl & {
  id: string;
  completions: CompositionalCompletion[];
  /** Overall mole fractions of the injected stream. Required for an injector. */
  injection_composition?: number[];
};

/** A complete case, as it crosses the worker and WASM boundaries. */
export interface CompositionalCaseConfig {
  schema: typeof COMPOSITIONAL_CASE_SCHEMA;
  fluid: CompositionalFluidChoice;
  grid: CompositionalGridConfig;
  relperm: CompositionalRelPermConfig;
  initial_pressure_bar: number;
  /** All `N` overall mole fractions. They must sum to one; nothing renormalizes them. */
  initial_composition: number[];
  wells: CompositionalWellConfig[];
  gravity_enabled: boolean;
}

/** What validation found that is admissible but worth acting on. */
export interface CompositionalAdvisories {
  /**
   * True when the case runs on `linear` relative permeability, which is a verification assumption
   * and not a physical model. A scenario-admission check should refuse to ship a case on it.
   */
  relperm_is_verification_only: boolean;
}

/** Why the engine refused a payload. `kind` is stable; the other fields depend on it. */
export type CompositionalConfigError =
  | { kind: 'unsupported_schema'; found: string; expected: string }
  | { kind: 'unknown_fluid'; name: string; supported: string[] }
  | { kind: 'out_of_range'; field: string; value: number; reason: string }
  | { kind: 'unsupported_geometry'; reason: string }
  | { kind: 'unsupported_control'; well: string; control: string }
  | { kind: 'composition_mismatch'; field: string; expected: number; found: number }
  | { kind: 'rejected'; field: string; reason: string };

/** Why a step did not succeed. */
export interface CompositionalStepFailure {
  /** `Flash`, `Linear`, `Nonlinear`, `Admissibility` or `Budget`. */
  kind: string;
  message: string;
}

/** What one step did. A failure is reported here, not thrown. */
export interface CompositionalStepOutcome {
  /** `null` when no attempt succeeded; `failure` then says why. */
  accepted_dt_days: number | null;
  time_days: number;
  attempts: number;
  newton_iterations: number;
  failure: CompositionalStepFailure | null;
}

/**
 * The accepted state and its totals.
 *
 * Carries **no derivative arrays** — the Jacobian blocks are per-cell, large and of no use to a
 * chart, and the engine's own test bounds this payload's size so that stays true.
 */
export interface CompositionalSnapshot {
  time_days: number;
  component_ids: string[];
  pressure: number[];
  /** `composition[component][cell]` — component-major, because a chart wants one component across the grid. */
  composition: number[][];
  /** `'two-phase' | 'single-liquid' | 'single-vapour' | 'unresolved'`, per cell. */
  phase_state: string[];
  vapour_saturation: number[];
  /** Total moles per component held by the grid — the conservation diagnostic. */
  inventory: number[];
  /** Per well, cumulative component moles. Positive into the reservoir. */
  cumulative_well_moles: number[][];
}

/** A versioned checkpoint. It carries its own config, because a state without one restores into nothing. */
export interface CompositionalCheckpoint {
  schema: typeof COMPOSITIONAL_CHECKPOINT_SCHEMA;
  config: CompositionalCaseConfig;
  time_days: number;
  pressure: number[];
  composition: number[][];
}

/** Turn an engine rejection into something a person can act on, without implementation jargon. */
export function describeCompositionalError(error: CompositionalConfigError): string {
  switch (error.kind) {
    case 'unsupported_schema':
      return `This case was written for a different version of the simulator (${error.found || 'no version'}); this one reads ${error.expected}.`;
    case 'unknown_fluid':
      return `There is no fluid called "${error.name}". Available: ${error.supported.join(', ')}.`;
    case 'out_of_range':
      return `${error.field} is ${error.value}, which ${error.reason}.`;
    case 'unsupported_geometry':
      return `This grid is not supported: ${error.reason}.`;
    case 'unsupported_control':
      return `Well "${error.well}" asks for a control this simulator does not have: ${error.control}.`;
    case 'composition_mismatch':
      return `${error.field} lists ${error.found} components, but this fluid has ${error.expected}.`;
    case 'rejected':
      return `${error.field} was not accepted: ${error.reason}.`;
  }
}

/** True when a value looks like an engine rejection rather than an ordinary error. */
export function isCompositionalConfigError(value: unknown): value is CompositionalConfigError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    typeof (value as { kind: unknown }).kind === 'string'
  );
}
