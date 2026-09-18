/**
 * The fluid-model discriminator on a create payload, and how a compositional case is built.
 *
 * The plan's rule, and the reason this is a guard rather than a cast: **an absent discriminator is
 * black-oil.** Every payload serialized before the compositional model existed lacks the field, and
 * reading those as compositional would silently reinterpret every stored scenario. The guard
 * therefore requires the discriminator *and* a config to be present; either alone is not enough.
 */

import type { SimulatorCreatePayload } from '../simulator-types';
import {
  COMPOSITIONAL_CASE_SCHEMA,
  type CompositionalCaseConfig,
  type CompositionalFluidChoice,
  type CompositionalRelPermConfig,
  type CompositionalWellConfig,
} from './types';

/** Which engine a payload is asking for. */
export type FluidModelKind = 'black-oil' | 'compositional';

/** A create payload that carries a compositional case. */
export interface CompositionalCreatePayload {
  fluidModel: 'compositional';
  compositional: CompositionalCaseConfig;
}

/**
 * True when this payload asks for the compositional engine.
 *
 * Both the discriminator and the config must be present. A payload with `fluidModel:
 * 'compositional'` and nothing to run is a caller bug, and failing the guard turns it into a
 * black-oil create that will complain about missing fields — which is a worse error than the one
 * `assertCompositionalPayload` gives.
 */
export function isCompositionalCreate(
  payload: unknown,
): payload is SimulatorCreatePayload & CompositionalCreatePayload {
  if (typeof payload !== 'object' || payload === null) {
    return false;
  }
  const candidate = payload as Partial<CompositionalCreatePayload>;
  return (
    candidate.fluidModel === 'compositional' &&
    typeof candidate.compositional === 'object' &&
    candidate.compositional !== null
  );
}

/** The engine a payload will run on. Absent discriminator means black-oil, by design. */
export function fluidModelOf(payload: unknown): FluidModelKind {
  return isCompositionalCreate(payload) ? 'compositional' : 'black-oil';
}

/**
 * Explain why a payload that claims to be compositional is not usable.
 *
 * Returns `null` when it is fine. This catches the shape errors worth catching on this side — a
 * missing config, a schema from another version — and leaves the physical validation to the
 * engine, which owns it and reports the field.
 */
export function assertCompositionalPayload(payload: unknown): string | null {
  if (typeof payload !== 'object' || payload === null) {
    return 'The create payload is not an object.';
  }
  const candidate = payload as Partial<CompositionalCreatePayload>;
  if (candidate.fluidModel !== 'compositional') {
    return null;
  }
  if (typeof candidate.compositional !== 'object' || candidate.compositional === null) {
    return 'This scenario asks for the compositional model but carries no compositional case.';
  }
  const schema = (candidate.compositional as { schema?: unknown }).schema;
  if (schema !== COMPOSITIONAL_CASE_SCHEMA) {
    return `This compositional case was written for ${String(schema) || 'no version'}; this build reads ${COMPOSITIONAL_CASE_SCHEMA}.`;
  }
  return null;
}

/** What a caller has to supply to describe a 1D compositional column. */
export interface CompositionalCaseInput {
  fluid: CompositionalFluidChoice;
  cells: number;
  cellDxM: number;
  cellDyM: number;
  cellDzM: number;
  porosity: number;
  permeabilityMd: number;
  initialPressureBar: number;
  initialComposition: number[];
  relperm: CompositionalRelPermConfig;
  wells: CompositionalWellConfig[];
  /** Rock compressibility; both default to the incompressible rock the validated cases use. */
  rockReferencePressureBar?: number;
  rockCompressibilityPerBar?: number;
  gravityEnabled?: boolean;
}

/**
 * Build a case config.
 *
 * This only assembles the payload — it does not validate the physics. The engine does that, before
 * it allocates anything, and it names the field; duplicating those checks here would create a
 * second place for the rules to live and drift.
 */
export function buildCompositionalCase(input: CompositionalCaseInput): CompositionalCaseConfig {
  return {
    schema: COMPOSITIONAL_CASE_SCHEMA,
    fluid: input.fluid,
    grid: {
      cells: input.cells,
      dx_m: input.cellDxM,
      dy_m: input.cellDyM,
      dz_m: input.cellDzM,
      porosity: input.porosity,
      permeability_md: input.permeabilityMd,
      rock_reference_pressure_bar: input.rockReferencePressureBar ?? 1.0,
      rock_compressibility_per_bar: input.rockCompressibilityPerBar ?? 0,
    },
    relperm: input.relperm,
    initial_pressure_bar: input.initialPressureBar,
    initial_composition: [...input.initialComposition],
    wells: input.wells.map((well) => ({
      ...well,
      completions: well.completions.map((completion) => ({ ...completion })),
      ...(well.injection_composition
        ? { injection_composition: [...well.injection_composition] }
        : {}),
    })),
    gravity_enabled: input.gravityEnabled ?? false,
  };
}

/**
 * Peaceman's geometric well index for a vertical well in an isotropic rectangular cell, in
 * `m³·cP/(day·bar)`.
 *
 * The same expression the black-oil path uses, with the same Darcy constant. Exposed here because a
 * compositional payload carries the well index rather than the completion geometry: the engine's
 * connection law takes `WI` and factors mobility out of it, so the geometry has to be turned into a
 * number somewhere, and doing it here keeps the engine free of well-completion conventions.
 *
 * `wellRadiusM` is a **radius**. `COMPDAT` item 9 is a diameter, and reading one as the other cost
 * C12 a 12–15% error in the well index that grew under grid refinement — see
 * `docs/COMPOSITIONAL_C12_FORENSICS.md`.
 */
export function peacemanWellIndex(options: {
  permeabilityMd: number;
  dxM: number;
  dyM: number;
  dzM: number;
  wellRadiusM: number;
  skin?: number;
}): number {
  const DARCY_METRIC_FACTOR = 8.5269888e-3;
  const { permeabilityMd, dxM, dyM, dzM, wellRadiusM } = options;
  const equivalentRadius = (0.28 * Math.sqrt(dxM * dxM + dyM * dyM)) / 2;
  const denominator = Math.log(equivalentRadius / wellRadiusM) + (options.skin ?? 0);
  return (DARCY_METRIC_FACTOR * 2 * Math.PI * permeabilityMd * dzM) / denominator;
}
