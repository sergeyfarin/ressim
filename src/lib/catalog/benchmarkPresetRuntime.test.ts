import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import init, { ReservoirSimulator } from '../ressim/pkg/simulator.js';
import { computeWelgeMetrics } from '../analytical/fractionalFlow';
import { getBenchmarkEntry, getBenchmarkFamily } from './caseCatalog';

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

function applyPermeability(simulator: ReservoirSimulator, params: BenchmarkParams) {
  const permMode = String(params.permMode ?? 'uniform');
  const nz = Number(params.nz);

  if (permMode === 'random') {
    simulator.setPermeabilityRandomSeeded(
      Number(params.minPerm ?? 100),
      Number(params.maxPerm ?? 100),
      42n,
    );
    return;
  }

  const permsX = permMode === 'perLayer'
    ? Array.from(params.layerPermsX as ArrayLike<number>)
    : Array.from({ length: nz }, () => Number(params.uniformPermX ?? 100));
  const permsY = permMode === 'perLayer'
    ? Array.from(params.layerPermsY as ArrayLike<number>)
    : Array.from({ length: nz }, () => Number(params.uniformPermY ?? params.uniformPermX ?? 100));
  const permsZ = permMode === 'perLayer'
    ? Array.from(params.layerPermsZ as ArrayLike<number>)
    : Array.from({ length: nz }, () => Number(params.uniformPermZ ?? 10));

  simulator.setPermeabilityPerLayer(permsX, permsY, permsZ);
}

function measureBreakthroughPvi(params: BenchmarkParams, watercutThreshold = 0.01) {
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
  applyPermeability(simulator, params);
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

  const poreVolume = nx
    * ny
    * nz
    * Number(params.cellDx ?? 10)
    * Number(params.cellDy ?? 10)
    * Number(params.cellDz ?? 1)
    * Number(params.reservoirPorosity ?? 0.2);

  let cumulativeInjection = 0;
  let previousTime = 0;

  for (let step = 1; step <= maxSteps; step += 1) {
    simulator.step(dtDays);
    const point = simulator.getRateHistory().at(-1);
    if (!point || Number(point.total_production_liquid ?? 0) <= 1e-9) continue;

    const timeDays = Number(point.time ?? step * dtDays);
    const dt = Math.max(0, timeDays - previousTime);
    previousTime = timeDays;
    cumulativeInjection += Math.max(0, Number(point.total_injection ?? 0)) * dt;

    const totalLiquid = Number(point.total_production_liquid ?? 0);
    const waterRate = Math.max(0, totalLiquid - Number(point.total_production_oil ?? 0));
    const watercut = Math.min(1, Math.max(0, waterRate / totalLiquid));
    if (watercut >= watercutThreshold) {
      return {
        step,
        timeDays,
        watercut,
        breakthroughPvi: poreVolume > 0 ? cumulativeInjection / poreVolume : Number.POSITIVE_INFINITY,
      };
    }
  }

  return null;
}

describe('frontend benchmark preset runtime coverage', () => {
  it('keeps refined BL presets aligned with their declared Rust-parity breakthrough PV metric', async () => {
    await ensureWasmReady();

    for (const key of ['bl_case_a_refined', 'bl_case_b_refined']) {
      const entry = getBenchmarkEntry(key);
      const family = getBenchmarkFamily(key);
      expect(entry, `${key} benchmark entry should exist`).toBeDefined();
      expect(family, `${key} benchmark family should exist`).toBeDefined();

      const watercutThreshold = family?.breakthroughCriterion?.value ?? 0.01;
      const breakthrough = measureBreakthroughPvi(entry!.params, watercutThreshold);
      expect(
        breakthrough,
        `${key} should reach ${watercutThreshold * 100}% watercut within ${entry!.params.steps} steps`,
      ).not.toBeNull();
      expect(breakthrough!.step).toBeLessThanOrEqual(Number(entry!.params.steps));

      const reference = computeWelgeMetrics(
        {
          s_wc: Number(entry!.params.s_wc ?? 0.1),
          s_or: Number(entry!.params.s_or ?? 0.1),
          n_w: Number(entry!.params.n_w ?? 2),
          n_o: Number(entry!.params.n_o ?? 2),
          k_rw_max: Number(entry!.params.k_rw_max ?? 1),
          k_ro_max: Number(entry!.params.k_ro_max ?? 1),
        },
        {
          mu_w: Number(entry!.params.mu_w ?? 0.5),
          mu_o: Number(entry!.params.mu_o ?? 1),
        },
        Number(entry!.params.initialSaturation ?? entry!.params.s_wc ?? 0.1),
      );

      const relativeError = Math.abs(
        (breakthrough!.breakthroughPvi - reference.breakthroughPvi) / reference.breakthroughPvi,
      );

      expect(relativeError).toBeLessThanOrEqual(family?.comparisonMetric?.tolerance ?? 0.3);
    }
  });
});