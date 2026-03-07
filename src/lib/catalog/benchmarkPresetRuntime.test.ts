import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import init, { ReservoirSimulator } from '../ressim/pkg/simulator.js';
import { getBenchmarkEntry } from './caseCatalog';

type BenchmarkParams = Record<string, unknown>;

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
  if (!wasmReady) {
    wasmReady = readFile(new URL('../ressim/pkg/simulator_bg.wasm', import.meta.url)).then(
      (wasmBytes) => init({ module_or_path: wasmBytes }),
    );
  }
  await wasmReady;
}

function measureBreakthroughStep(params: BenchmarkParams, watercutThreshold = 0.01) {
  const nx = Number(params.nx);
  const ny = Number(params.ny);
  const nz = Number(params.nz);
  const dtDays = Number(params.delta_t_days);
  const maxSteps = Number(params.steps);

  const simulator = new ReservoirSimulator(nx, ny, nz, Number(params.reservoirPorosity ?? 0.2));
  simulator.setCellDimensions(
    Number(params.cellDx ?? 10),
    Number(params.cellDy ?? 10),
    Number(params.cellDz ?? 1),
  );
  simulator.setRelPermProps(
    Number(params.s_wc ?? 0.1),
    Number(params.s_or ?? 0.1),
    Number(params.n_w ?? 2),
    Number(params.n_o ?? 2),
    Number(params.k_rw_max ?? 1),
    Number(params.k_ro_max ?? 1),
  );
  simulator.setInitialPressure(Number(params.initialPressure ?? 300));
  simulator.setInitialSaturation(Number(params.initialSaturation ?? 0.3));
  simulator.setFluidProperties(Number(params.mu_o ?? 1), Number(params.mu_w ?? 0.5));
  simulator.setCapillaryParams(
    Number(params.capillaryEnabled ?? false) ? Number(params.capillaryPEntry ?? 0) : 0,
    Number(params.capillaryLambda ?? 2),
  );
  simulator.setPermeabilityRandomSeeded(
    Number(params.uniformPermX ?? 2000),
    Number(params.uniformPermX ?? 2000),
    42n,
  );
  simulator.setStabilityParams(
    Number(params.max_sat_change_per_step ?? 0.05),
    Number(params.max_pressure_change_per_step ?? 75),
    Number(params.max_well_rate_change_fraction ?? 0.75),
  );
  simulator.setWellControlModes(
    String(params.injectorControlMode ?? 'pressure'),
    String(params.producerControlMode ?? 'pressure'),
  );
  simulator.setTargetWellRates(
    Number(params.targetInjectorRate ?? 0),
    Number(params.targetProducerRate ?? 0),
  );

  simulator.add_well(
    Number(params.injectorI ?? 0),
    Number(params.injectorJ ?? 0),
    0,
    Number(params.injectorBhp ?? 500),
    Number(params.well_radius ?? 0.1),
    Number(params.well_skin ?? 0),
    true,
  );
  simulator.add_well(
    Number(params.producerI ?? nx - 1),
    Number(params.producerJ ?? 0),
    0,
    Number(params.producerBhp ?? 100),
    Number(params.well_radius ?? 0.1),
    Number(params.well_skin ?? 0),
    false,
  );

  for (let step = 1; step <= maxSteps; step += 1) {
    simulator.step(dtDays);
    const point = simulator.getRateHistory().at(-1);
    if (!point || Number(point.total_production_liquid ?? 0) <= 1e-9) continue;

    const totalLiquid = Number(point.total_production_liquid ?? 0);
    const waterRate = Math.max(0, totalLiquid - Number(point.total_production_oil ?? 0));
    const watercut = Math.min(1, Math.max(0, waterRate / totalLiquid));
    if (watercut >= watercutThreshold) {
      return {
        step,
        timeDays: Number(point.time ?? step * dtDays),
        watercut,
      };
    }
  }

  return null;
}

describe('frontend benchmark preset runtime coverage', () => {
  it('reaches breakthrough within the declared run horizon for refined BL presets', async () => {
    await ensureWasmReady();

    for (const key of ['bl_case_a_refined', 'bl_case_b_refined']) {
      const entry = getBenchmarkEntry(key);
      expect(entry, `${key} benchmark entry should exist`).toBeDefined();

      const breakthrough = measureBreakthroughStep(entry!.params);
      expect(
        breakthrough,
        `${key} should reach 1% watercut within ${entry!.params.steps} steps`,
      ).not.toBeNull();
      expect(breakthrough!.step).toBeLessThanOrEqual(Number(entry!.params.steps));
    }
  });
});