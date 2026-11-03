import { describe, expect, it } from 'vitest';
import {
  evaluateCompositionalTerminationPolicy,
  evaluateTerminationPolicy,
} from './terminationPolicy';
import type { CompositionalSnapshot } from '../compositional/types';
import type { RateHistoryPoint, SimulatorCreatePayload } from '../simulator-types';

function makePayload(overrides: Partial<SimulatorCreatePayload> = {}): SimulatorCreatePayload {
  return {
    nx: 1,
    ny: 1,
    nz: 1,
    cellDx: 1,
    cellDy: 1,
    cellDz: 1,
    initialPressure: 300,
    initialSaturation: 0.1,
    porosity: 0.2,
    mu_w: 0.5,
    mu_o: 1,
    c_o: 1e-5,
    c_w: 3e-6,
    rho_w: 1000,
    rho_o: 800,
    rock_compressibility: 1e-6,
    depth_reference: 0,
    volume_expansion_o: 1,
    volume_expansion_w: 1,
    s_wc: 0.1,
    s_or: 0.1,
    n_w: 2,
    n_o: 2,
    k_rw_max: 1,
    k_ro_max: 1,
    max_sat_change_per_step: 0.1,
    max_pressure_change_per_step: 50,
    max_well_rate_change_fraction: 1,
    capillaryEnabled: false,
    capillaryPEntry: 0,
    capillaryLambda: 2,
    gravityEnabled: false,
    permMode: 'uniform',
    minPerm: 100,
    maxPerm: 100,
    permsX: [100],
    permsY: [100],
    permsZ: [10],
    well_radius: 0.1,
    well_skin: 0,
    injectorBhp: 500,
    producerBhp: 100,
    injectorEnabled: true,
    injectorControlMode: 'pressure',
    producerControlMode: 'pressure',
    targetInjectorRate: 0,
    targetProducerRate: 0,
    injectorI: 0,
    injectorJ: 0,
    producerI: 0,
    producerJ: 0,
    ...overrides,
  };
}

describe('evaluateTerminationPolicy', () => {
  it('matches watercut threshold at fractional values', () => {
    const point: RateHistoryPoint = {
      time: 10,
      total_production_oil: 90,
      total_production_liquid: 100,
    };

    const result = evaluateTerminationPolicy({
      mode: 'any',
      conditions: [{ kind: 'watercut-threshold', value: 0.1, scope: 'producer' }],
    }, point, makePayload());

    expect(result?.summary).toContain('Watercut threshold reached');
  });

  it('matches producer oil rate falling to zero with lte relation', () => {
    const point: RateHistoryPoint = {
      time: 10,
      total_production_oil: 0,
      total_production_liquid: 5,
    };

    const result = evaluateTerminationPolicy({
      mode: 'any',
      conditions: [{ kind: 'phase-rate-threshold', phase: 'oil', relation: 'lte', value: 0, scope: 'producer' }],
    }, point, makePayload());

    expect(result).not.toBeNull();
  });

  it('matches GOR exceedance with gte relation', () => {
    const point: RateHistoryPoint = {
      time: 10,
      producing_gor: 250,
      total_production_oil: 50,
      total_production_liquid: 60,
    };

    const result = evaluateTerminationPolicy({
      mode: 'any',
      conditions: [{ kind: 'gor-threshold', relation: 'gte', value: 200, scope: 'producer' }],
    }, point, makePayload());

    expect(result).not.toBeNull();
  });
});

function compositionalSnapshot(
  rates: CompositionalSnapshot['well_surface_rates'],
): CompositionalSnapshot {
  return {
    time_days: 10,
    component_ids: ['CO2', 'C1', 'C10'],
    pressure: [100],
    composition: [[0.3], [0.3], [0.4]],
    phase_state: ['two-phase'],
    vapour_saturation: [0.3],
    inventory: [1, 3, 6],
    cumulative_well_moles: [[0, 0, 0]],
    ...(rates === undefined ? {} : { well_surface_rates: rates }),
  };
}

describe('evaluateCompositionalTerminationPolicy', () => {
  it('matches the SPE5 GOR shut-in from separator vapour and liquid rates', () => {
    // 2500 m³/m³ is above 10 Mscf/STB (1781.076 m³/m³).
    const result = evaluateCompositionalTerminationPolicy({
      mode: 'any',
      conditions: [{ kind: 'gor-threshold', relation: 'gte', value: 1781.0760667903526, scope: 'producer' }],
    }, compositionalSnapshot([
      { liquid_m3_per_day: -100, vapour_m3_per_day: -250_000 },
    ]));

    expect(result?.summary).toContain('GOR threshold reached');
    expect(result?.actualValue).toBeCloseTo(2500, 9);
  });

  it('does not match before the GOR threshold is reached', () => {
    const result = evaluateCompositionalTerminationPolicy({
      mode: 'any',
      conditions: [{ kind: 'gor-threshold', relation: 'gte', value: 1781.0760667903526, scope: 'producer' }],
    }, compositionalSnapshot([
      { liquid_m3_per_day: -100, vapour_m3_per_day: -100_000 },
    ]));

    expect(result).toBeNull();
  });

  it('matches the SPE5 WOR shut-in as the equivalent surface water cut', () => {
    // WOR 5 is water cut 5/6; 600 water against 100 liquid is WOR 6.
    const result = evaluateCompositionalTerminationPolicy({
      mode: 'any',
      conditions: [{ kind: 'watercut-threshold', value: 5 / 6, scope: 'producer' }],
    }, compositionalSnapshot([
      {
        liquid_m3_per_day: -100,
        vapour_m3_per_day: 0,
        water_m3_per_day: -600,
      },
    ]));

    expect(result?.summary).toContain('Watercut threshold reached');
    expect(result?.actualValue).toBeCloseTo(600 / 700, 12);
  });

  it('evaluates a producer phase-rate condition', () => {
    const result = evaluateCompositionalTerminationPolicy({
      mode: 'any',
      conditions: [{ kind: 'phase-rate-threshold', phase: 'gas', relation: 'gte', value: 200, scope: 'producer' }],
    }, compositionalSnapshot([
      { liquid_m3_per_day: -10, vapour_m3_per_day: -250 },
    ]));

    expect(result).not.toBeNull();
  });

  it('treats unreported surface rates as not evaluated, not as zero', () => {
    const policy = {
      mode: 'any' as const,
      conditions: [{ kind: 'gor-threshold' as const, relation: 'lte' as const, value: 0, scope: 'producer' as const }],
    };
    expect(evaluateCompositionalTerminationPolicy(policy, compositionalSnapshot(undefined))).toBeNull();
  });
});