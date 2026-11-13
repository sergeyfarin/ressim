import { readFile } from 'node:fs/promises';
import { beforeAll, expect, it } from 'vitest';

import { comp_spe5_wag } from '../catalog/scenarios/comp_spe5_wag';
import initWasm, { CompositionalSimulator } from '../ressim/pkg/simulator.js';
import { COMPOSITIONAL_CASE_SCHEMA } from './types';
import type { CompositionalCaseConfig, CompositionalCheckpoint, CompositionalSnapshot } from './types';

beforeAll(async () => {
  const bytes = await readFile(new URL('../ressim/pkg/simulator_bg.wasm', import.meta.url));
  await initWasm({ module_or_path: bytes });
});

it('preserves component and surface ledgers through a JSON checkpoint and the next WASM step', () => {
  const base = comp_spe5_wag.params.compositional as CompositionalCaseConfig;
  const config: CompositionalCaseConfig = {
    ...base,
    schema: COMPOSITIONAL_CASE_SCHEMA,
    grid: {
      cells: 2, dx_m: 60, dy_m: 6, dz_m: 6, porosity: 0.1, permeability_md: 100,
      rock_reference_pressure_bar: 68.9476, rock_compressibility_per_bar: 0,
    },
    initial_pressure_bar: 75,
    initial_pressure_gradient_bar_per_m: undefined,
    initial_composition: [0.5, 0.03, 0.07, 0.2, 0.15, 0.05],
    schedule: [],
    gravity_enabled: false,
    wells: [
      {
        id: 'INJ', completions: [{ cell: 0, well_index: 4.58 }],
        control: 'bhp', target_bar: 250, injection_composition: [1, 0, 0, 0, 0, 0],
      },
      {
        id: 'PROD', completions: [{ cell: 1, well_index: 4.58 }],
        control: 'surface-rate', target_m3_per_day: -10, phase: 'liquid', bhp_limit_bar: 50,
      },
    ],
  };
  const original = new CompositionalSimulator(config);
  let restored: CompositionalSimulator | undefined;
  try {
    expect(original.step(0.02).accepted_dt_days).toBe(0.02);
    const exported = original.checkpoint() as CompositionalCheckpoint;
    // A Map survives structured clone but JSON silently turns it into {}, losing every well.
    expect(exported.config.wells[0] instanceof Map).toBe(false);
    const checkpoint = JSON.parse(JSON.stringify(exported)) as CompositionalCheckpoint;
    expect(checkpoint.cumulative_well_moles).toHaveLength(2);
    expect(checkpoint.cumulative_well_moles?.[0]).toHaveLength(7);
    expect(checkpoint.cumulative_source_moles).toHaveLength(7);
    expect(checkpoint.cumulative_well_surface_volumes).toHaveLength(2);
    expect(checkpoint.cumulative_well_surface_production_volumes?.[1].liquid_m3).toBeGreaterThan(0);
    restored = CompositionalSimulator.restore(structuredClone(checkpoint));
    expect(restored.getSnapshot()).toEqual(original.getSnapshot());

    expect(original.step(0.02).accepted_dt_days).toBe(0.02);
    expect(restored.step(0.02).accepted_dt_days).toBe(0.02);
    const snapshot = restored.getSnapshot() as CompositionalSnapshot;
    expect(snapshot).toEqual(original.getSnapshot());
    expect(snapshot.cumulative_well_surface_production_volumes?.[1].liquid_m3)
      .toBeGreaterThan(checkpoint.cumulative_well_surface_production_volumes![1].liquid_m3);
  } finally {
    original.free();
    restored?.free();
  }
});
