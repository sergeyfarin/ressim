import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../ressim/pkg/simulator.js';
import { computeWelgeMetrics } from '@ressim/analytical/fractionalFlow';
import { buildBenchmarkCreatePayload, buildBenchmarkRunResult, buildBenchmarkRunSpecs } from '../benchmarkRunModel';
import { buildReferenceComparisonModel } from '@ressim/charts/buildChartData';
import { getBenchmarkEntry, getBenchmarkFamily, getBenchmarkVariantsForFamily } from './benchmarkCases';
import { getScenario, getScenarioWithVariantParams } from './scenarios';
import type { SimulatorCreatePayload } from '../simulator-types';
import { configureReservoirSimulator } from '../workers/configureSimulator';

type BenchmarkParams = Record<string, unknown>;

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
  if (!wasmReady) {
    wasmReady = readFile(new URL('../ressim/pkg/simulator_bg.wasm', import.meta.url)).then(
      (wasmBytes) => initWasm({ module_or_path: wasmBytes }),
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

  simulator.setPermeabilityPerLayer(new Float64Array(permsX), new Float64Array(permsY), new Float64Array(permsZ));
}

/** Build a simulator the way the worker does: its own setup function, not a replica (#12). */
function configureSimulatorFromPayload(payload: SimulatorCreatePayload): ReservoirSimulator {
  const simulator = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));
  configureReservoirSimulator(simulator, payload);
  return simulator;
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

function runBenchmarkSpec(spec: ReturnType<typeof buildBenchmarkRunSpecs>[number]) {
  const params = spec.params;
  const nx = Number(params.nx);
  const ny = Number(params.ny);
  const nz = Number(params.nz);
  const dtDays = Number(spec.deltaTDays);
  const steps = Number(spec.steps);

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
  simulator.setFluidCompressibilities(Number(params.c_o ?? 1e-5), Number(params.c_w ?? 3e-6));
  simulator.setRockProperties(
    Number(params.rock_compressibility ?? 1e-6),
    Number(params.depth_reference ?? 0),
    Number(params.volume_expansion_o ?? 1),
    Number(params.volume_expansion_w ?? 1),
  );
  simulator.setFluidDensities(Number(params.rho_o ?? 800), Number(params.rho_w ?? 1000));
  simulator.setCapillaryParams(
    Number(params.capillaryEnabled ?? false) ? Number(params.capillaryPEntry ?? 0) : 0,
    Number(params.capillaryLambda ?? 2),
  );
  simulator.setGravityEnabled(Boolean(params.gravityEnabled ?? false));
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
  simulator.setWellBhpLimits(
    Math.min(Number(params.producerBhp ?? 100), Number(params.injectorBhp ?? 500)),
    Math.max(Number(params.producerBhp ?? 100), Number(params.injectorBhp ?? 500)),
  );

  for (let k = 0; k < nz; k += 1) {
    simulator.add_well(
      Number(params.injectorI ?? 0),
      Number(params.injectorJ ?? 0),
      k,
      Number(params.injectorBhp ?? 500),
      Number(params.well_radius ?? 0.1),
      Number(params.well_skin ?? 0),
      true,
    );
    simulator.add_well(
      Number(params.producerI ?? nx - 1),
      Number(params.producerJ ?? 0),
      k,
      Number(params.producerBhp ?? 100),
      Number(params.well_radius ?? 0.1),
      Number(params.well_skin ?? 0),
      false,
    );
  }

  for (let step = 0; step < steps; step += 1) {
    simulator.step(dtDays);
  }

  const result = buildBenchmarkRunResult({
    spec,
    rateHistory: simulator.getRateHistory(),
  });
  simulator.free();
  return result;
}

describe('frontend benchmark preset runtime coverage', () => {
  it.skip('keeps refined BL presets aligned with their declared Rust-parity breakthrough PV metric' + ' [ARCHIVED FIXTURE — see .archive/README.md, TODO.md]', async () => {
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
  }, 30_000);

  it.skip('builds distinct BL Case A comparison series for selected sensitivity variants' + ' [ARCHIVED FIXTURE — see .archive/README.md, TODO.md]', async () => {
    await ensureWasmReady();

    const family = getBenchmarkFamily('bl_case_a_refined');
    const variants = getBenchmarkVariantsForFamily('bl_case_a_refined');
    const selectedVariants = [
      variants.find((variant) => variant.variantKey === 'grid_24'),
      variants.find((variant) => variant.variantKey === 'grid_48'),
    ].filter((variant): variant is NonNullable<typeof variant> => Boolean(variant));

    expect(family).not.toBeNull();
    expect(selectedVariants).toHaveLength(2);

    const results = buildBenchmarkRunSpecs(family!, selectedVariants).map(runBenchmarkSpec);
    const model = buildReferenceComparisonModel({
      family,
      results,
      xAxisMode: 'pvi',
    });

    const waterCutSeries = model.panels.rates.curves
      .map((curve, index) => ({ curve, series: model.panels.rates.series[index] ?? [] }))
      .filter((entry) => entry.curve.curveKey === 'water-cut-sim');

    expect(waterCutSeries).toHaveLength(3);

    const distinctSeries = new Set(
      waterCutSeries.map((entry) => JSON.stringify(entry.series.map((point) => [point.x, point.y]))),
    );

    expect(distinctSeries.size).toBe(3);
  }, 30_000);

  it('preserves the Well-Test reservoir-rate target through the worker-style payload', async () => {
    await ensureWasmReady();

    const flowingBhps = ['skin_stimulated', 'skin_clean', 'skin_damaged'].map((variantKey) => {
      const params = getScenarioWithVariantParams('dep_welltest', 'skin', variantKey);
      const payload = buildBenchmarkCreatePayload(params);
      const simulator = configureSimulatorFromPayload(payload);

      simulator.step(Number(params.delta_t_days));
      const producer = (simulator.getWellState() as Array<Record<string, unknown>>)
        .find((well) => well.injector === false);
      const flowingBhp = Number(producer?.flowing_bhp);
      simulator.free();

      expect(flowingBhp).toBeLessThan(Number(params.initialPressure) - 1);
      return flowingBhp;
    });

    expect(flowingBhps[0]).toBeGreaterThan(flowingBhps[1]);
    expect(flowingBhps[1]).toBeGreaterThan(flowingBhps[2]);
  });

  it('WASM bindings create free gas for a minimal three-phase gas-injection case', async () => {
    await ensureWasmReady();

    const simulator = new ReservoirSimulator(5, 1, 1, 0.2);
    simulator.setThreePhaseRelPermProps(0.1, 0.1, 0.05, 0.05, 0.1, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7);
    simulator.setGasFluidProperties(0.02, 1e-4, 10.0);
    simulator.setThreePhaseModeEnabled(true);
    simulator.setInjectedFluid('gas');
    simulator.setInitialPressure(200);
    simulator.setInitialSaturation(0.1);
    simulator.add_well(0, 0, 0, 400, 0.1, 0, true);
    simulator.add_well(4, 0, 0, 100, 0.1, 0, false);

    for (let step = 0; step < 20; step += 1) {
      simulator.step(2.0);
    }

    const maxSg = Math.max(...Array.from(simulator.getSatGas()));
    simulator.free();

    expect(maxSg).toBeGreaterThan(1e-6);
  });

  it('keeps the coarse SPE1 grid advancing to producer gas breakthrough in the WASM runtime path', async () => {
    await ensureWasmReady();

    const params: BenchmarkParams = {
      ...getScenario('spe1_gas_injection')!.params,
      nx: 5, ny: 5, cellDx: 609.6, cellDy: 609.6,
      producerI: 4, producerJ: 4,
    };
    const payload = buildBenchmarkCreatePayload(params);
    const simulator = configureSimulatorFromPayload(payload);
    const producerIndex = ((2 * payload.ny) + Number(payload.producerJ ?? 0)) * payload.nx + Number(payload.producerI ?? (payload.nx - 1));
    const injectorIndex = Number(payload.injectorI ?? 0);
    const initialRs = Number(simulator.getRs()[producerIndex] ?? Number.NaN);

    let breakthroughTime: number | null = null;
    let lastProducerSg = 0;
    let lastGor = 0;
    let lastInjection = 0;
    let maxSg = 0;
    let injectorSg = 0;

    for (let step = 0; step < Number(params.steps ?? 120); step += 1) {
      simulator.step(Number(params.delta_t_days ?? 30));

      const satGas = simulator.getSatGas();
      const producerSg = Number(satGas[producerIndex] ?? 0);
      const latestRate = simulator.getRateHistory().at(-1);
      lastProducerSg = producerSg;
      lastGor = Number(latestRate?.producing_gor ?? 0);
      lastInjection = Number(latestRate?.total_injection ?? 0);
      injectorSg = Number(satGas[injectorIndex] ?? 0);
      maxSg = Math.max(maxSg, ...Array.from(satGas));

      if (producerSg > 1e-4 || lastGor > 50) {
        breakthroughTime = simulator.get_time();
        break;
      }
    }

    const finalWarning = simulator.getLastSolverWarning();
    simulator.free();

    expect(initialRs).toBeCloseTo(226.197, 3);
    expect(lastInjection).toBeGreaterThan(1);
    expect(maxSg).toBeGreaterThan(1e-6);
    expect(injectorSg).toBeGreaterThan(1e-6);
    expect({ breakthroughTime, finalWarning, lastProducerSg, lastGor }).toEqual(
      expect.objectContaining({
        breakthroughTime: expect.any(Number),
        finalWarning: '',
      }),
    );
  });
});
