import { describe, expect, it } from 'vitest';

import {
  assertCompositionalPayload,
  buildCompositionalCase,
  fluidModelOf,
  isCompositionalCreate,
  peacemanWellIndex,
} from './createPayload';
import {
  COMPOSITIONAL_CASE_SCHEMA,
  COMPOSITIONAL_SCHEDULE_CASE_SCHEMA,
  describeCompositionalError,
} from './types';
import type { CompositionalCaseInput } from './createPayload';

function input(): CompositionalCaseInput {
  return {
    fluid: 'pinned-ternary',
    cells: 5,
    cellDxM: 60,
    cellDyM: 6,
    cellDzM: 6,
    porosity: 0.1,
    permeabilityMd: 100,
    initialPressureBar: 75,
    initialComposition: [0.1, 0.3, 0.6],
    relperm: { model: 'linear' },
    wells: [
      {
        id: 'INJ',
        completions: [{ cell: 0, well_index: 4.58 }],
        control: 'bhp',
        target_bar: 150,
        injection_composition: [1, 0, 0],
      },
    ],
  };
}

describe('the fluid-model discriminator', () => {
  it('reads an absent discriminator as black-oil, never as compositional', () => {
    // This is the rule the plan states outright, and it protects every scenario serialized before
    // the compositional model existed.
    expect(fluidModelOf({ nx: 10 })).toBe('black-oil');
    expect(fluidModelOf({})).toBe('black-oil');
    expect(isCompositionalCreate({ nx: 10 })).toBe(false);
  });

  it('reads an explicit black-oil discriminator as black-oil', () => {
    expect(fluidModelOf({ fluidModel: 'black-oil', compositional: {} })).toBe('black-oil');
  });

  it('needs the discriminator AND a config', () => {
    expect(isCompositionalCreate({ fluidModel: 'compositional' })).toBe(false);
    expect(isCompositionalCreate({ compositional: buildCompositionalCase(input()) })).toBe(false);
    expect(
      isCompositionalCreate({
        fluidModel: 'compositional',
        compositional: buildCompositionalCase(input()),
      }),
    ).toBe(true);
  });

  it('survives values that are not objects at all', () => {
    for (const value of [null, undefined, 3, 'compositional', []]) {
      expect(isCompositionalCreate(value)).toBe(false);
    }
  });
});

describe('assertCompositionalPayload', () => {
  it('passes a black-oil payload through without complaint', () => {
    expect(assertCompositionalPayload({ nx: 10 })).toBeNull();
  });

  it('explains a compositional payload with no case', () => {
    const message = assertCompositionalPayload({ fluidModel: 'compositional' });
    expect(message).toMatch(/carries no compositional case/i);
  });

  it('explains a case from another schema version', () => {
    const message = assertCompositionalPayload({
      fluidModel: 'compositional',
      compositional: { ...buildCompositionalCase(input()), schema: 'ressim-compositional-case/99' },
    });
    expect(message).toContain('ressim-compositional-case/99');
    expect(message).toContain(COMPOSITIONAL_CASE_SCHEMA);
  });

  it('accepts the scheduled schema as well as the unscheduled one', () => {
    const scheduled = buildCompositionalCase({
      ...input(),
      schedule: [{ time_days: 1, well: 'INJ', control: { control: 'shut-in' } }],
    });
    expect(scheduled.schema).toBe(COMPOSITIONAL_SCHEDULE_CASE_SCHEMA);
    expect(
      assertCompositionalPayload({ fluidModel: 'compositional', compositional: scheduled }),
    ).toBeNull();
  });
});

describe('buildCompositionalCase', () => {
  it('stamps the schema and defaults the rock to incompressible', () => {
    const config = buildCompositionalCase(input());
    expect(config.schema).toBe(COMPOSITIONAL_CASE_SCHEMA);
    expect(config.grid.rock_compressibility_per_bar).toBe(0);
    expect(config.gravity_enabled).toBe(false);
  });
  it('copies the arrays rather than aliasing the caller', () => {
    // A payload that shares an array with a store is how a $state proxy leaks across the worker
    // boundary, which structured cloning rejects.
    const source = input();
    const config = buildCompositionalCase(source);
    source.initialComposition[0] = 0.9;
    source.wells[0].completions[0].cell = 4;
    expect(config.initial_composition[0]).toBe(0.1);
    expect(config.wells[0].completions[0].cell).toBe(0);
  });

  it('produces a payload that survives structured cloning', () => {
    const config = buildCompositionalCase(input());
    expect(() => structuredClone(config)).not.toThrow();
    expect(structuredClone(config)).toEqual(config);
  });

  it('keeps the control discriminator flat, as the engine reads it', () => {
    const config = buildCompositionalCase(input());
    const well = config.wells[0];
    expect(well.control).toBe('bhp');
    expect('target_bar' in well && well.target_bar).toBe(150);
  });

  it('leaves the 1-D payload free of the structured-grid fields', () => {
    const config = buildCompositionalCase(input());
    expect('box_grid' in config.grid).toBe(false);
    expect('initial_pressure_gradient_bar_per_m' in config).toBe(false);
  });

  it('carries a box grid and gradient as copied plain data', () => {
    const source: CompositionalCaseInput = {
      ...input(),
      cells: 8,
      boxGrid: {
        nx: 2,
        ny: 2,
        nz: 2,
        layer_dz_m: [10, 20],
        layer_kx_md: [100, 200],
        layer_ky_md: [110, 210],
        layer_kz_md: [10, 20],
        top_depth_m: 1000,
      },
      initialPressureGradientBarPerM: 0.1,
    };
    const config = buildCompositionalCase(source);
    expect(config.grid.box_grid).toEqual(source.boxGrid);
    expect(config.initial_pressure_gradient_bar_per_m).toBe(0.1);

    // The layer vectors must not alias the caller: a $state array is a Proxy and fails structured
    // cloning at the worker boundary.
    source.boxGrid!.layer_dz_m![0] = 99;
    expect(config.grid.box_grid?.layer_dz_m?.[0]).toBe(10);
    expect(() => structuredClone(config)).not.toThrow();
  });

  it('carries a WAG schedule and water as copied plain data', () => {
    const source: CompositionalCaseInput = {
      ...input(),
      relperm: {
        model: 'stone-ii',
        swof: [
          { sw: 0, krw: 0, krow: 1 },
          { sw: 1, krw: 1, krow: 0 },
        ],
        sgof: [
          { sg: 0, krg: 0, krog: 1 },
          { sg: 1, krg: 1, krog: 0 },
        ],
      },
      water: {
        reference_pressure_bar: 200,
        formation_volume_factor: 1.02,
        compressibility_per_bar: 4.5e-5,
        viscosity_cp: 0.5,
        surface_density_kg_per_m3: 1020,
        initial_saturation: 0.15,
      },
      schedule: [
        {
          time_days: 1,
          well: 'INJ',
          control: { control: 'bhp', target_bar: 250 },
          injection: { injection: 'hydrocarbon', composition: [1, 0, 0] },
        },
        {
          time_days: 2,
          well: 'INJ',
          control: { control: 'shut-in' },
        },
      ],
    };
    const config = buildCompositionalCase(source);
    expect(config.schema).toBe(COMPOSITIONAL_SCHEDULE_CASE_SCHEMA);
    expect(config.water).toEqual(source.water);
    expect(config.schedule).toEqual(source.schedule);
    expect(() => structuredClone(config)).not.toThrow();

    // No array is shared with a caller's (possibly proxied) params object.
    const composition = source.schedule![0].injection;
    if (composition?.injection === 'hydrocarbon') composition.composition[0] = 0;
    expect(
      config.schedule?.[0].injection?.injection === 'hydrocarbon'
        ? config.schedule[0].injection.composition[0]
        : null,
    ).toBe(1);
  });
});

describe('peacemanWellIndex', () => {
  it('reproduces the well index C12 measured against opm-common', () => {
    // opm-common's own Connection::CF() for the 1D_COMP depletion cell converts to 6.806139
    // m3.cP/(day.bar); see docs/COMPOSITIONAL_C12_FORENSICS.md section 2.
    const wi = peacemanWellIndex({
      permeabilityMd: 100,
      dxM: 100,
      dyM: 100,
      dzM: 10,
      wellRadiusM: 0.0151 / 2,
    });
    expect(wi).toBeCloseTo(6.806139, 4);
  });

  it('takes a radius, not a diameter', () => {
    // Reading COMPDAT item 9 as a radius cost C12 a 12-15% error in the well index that grew under
    // grid refinement. Halving the radius must change the answer, and visibly.
    const asRadius = peacemanWellIndex({
      permeabilityMd: 100,
      dxM: 100,
      dyM: 100,
      dzM: 10,
      wellRadiusM: 0.0151,
    });
    const asDiameter = peacemanWellIndex({
      permeabilityMd: 100,
      dxM: 100,
      dyM: 100,
      dzM: 10,
      wellRadiusM: 0.0151 / 2,
    });
    expect(asRadius / asDiameter).toBeGreaterThan(1.05);
  });

  it('applies skin', () => {
    const base = { permeabilityMd: 100, dxM: 60, dyM: 6, dzM: 6, wellRadiusM: 0.00755 };
    expect(peacemanWellIndex({ ...base, skin: 60 })).toBeLessThan(peacemanWellIndex(base));
  });
});

describe('describeCompositionalError', () => {
  it('says what is wrong without implementation jargon', () => {
    const messages = [
      describeCompositionalError({
        kind: 'unsupported_schema',
        found: 'old',
        expected: COMPOSITIONAL_CASE_SCHEMA,
      }),
      describeCompositionalError({
        kind: 'out_of_range',
        field: 'grid.porosity',
        value: 0,
        reason: 'must be in (0, 1)',
      }),
      describeCompositionalError({
        kind: 'composition_mismatch',
        field: 'initial_composition',
        expected: 2,
        found: 3,
      }),
      describeCompositionalError({
        kind: 'unsupported_control',
        well: 'INJ',
        control: 'thp',
      }),
    ];
    for (const message of messages) {
      expect(message.length).toBeGreaterThan(20);
      expect(message).not.toMatch(/serde|JsValue|unwrap|panic/i);
    }
    expect(messages[1]).toContain('grid.porosity');
    expect(messages[2]).toContain('3 components');
  });
});
