import { readFile } from 'node:fs/promises';
import init, { ReservoirSimulator } from '../src/lib/ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../src/lib/catalog/scenarios';
import { buildBenchmarkCreatePayload } from '../src/lib/benchmarkRunModel';
import type { SimulatorCreatePayload, SimulatorWellDefinition, SimulatorWellSchedule } from '../src/lib/simulator-types';

function configureSimulator(payload: SimulatorCreatePayload): ReservoirSimulator {
  const simulator = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));
  const call = (name: string, ...args: unknown[]) => {
    const fn = (simulator as unknown as Record<string, unknown>)[name];
    if (typeof fn === 'function') {
      return (fn as (...inner: unknown[]) => unknown).call(simulator, ...args);
    }
    return undefined;
  };

  call('setFimEnabled', payload.fimEnabled !== false);
  if (payload.cellDzPerLayer && payload.cellDzPerLayer.length > 0) {
    call('setCellDimensionsPerLayer', Number(payload.cellDx), Number(payload.cellDy), new Float64Array(payload.cellDzPerLayer));
  } else {
    call('setCellDimensions', Number(payload.cellDx), Number(payload.cellDy), Number(payload.cellDz));
  }
  call('setFluidProperties', Number(payload.mu_o), Number(payload.mu_w));
  call('setFluidCompressibilities', Number(payload.c_o), Number(payload.c_w));
  if (payload.pvtMode === 'black-oil' && payload.pvtTable) {
    call('setPvtTable', payload.pvtTable);
  }
  if (payload.initialRs != null) {
    call('setInitialRs', Number(payload.initialRs));
  }
  call(
    'setRockProperties',
    Number(payload.rock_compressibility),
    Number(payload.depth_reference),
    Number(payload.volume_expansion_o),
    Number(payload.volume_expansion_w),
  );
  call('setFluidDensities', Number(payload.rho_o), Number(payload.rho_w));
  simulator.setInitialPressure(payload.initialPressure);
  if (payload.initialSaturationPerLayer && payload.initialSaturationPerLayer.length > 0) {
    call('setInitialSaturationPerLayer', new Float64Array(payload.initialSaturationPerLayer));
  } else {
    simulator.setInitialSaturation(payload.initialSaturation);
  }
  call('setCapillaryParams', Boolean(payload.capillaryEnabled) ? Number(payload.capillaryPEntry) : 0, Number(payload.capillaryLambda));
  call('setGravityEnabled', Boolean(payload.gravityEnabled));
  simulator.setRelPermProps(payload.s_wc, payload.s_or, payload.n_w, payload.n_o, payload.k_rw_max ?? 1.0, payload.k_ro_max ?? 1.0);

  if (payload.threePhaseModeEnabled) {
    call('setThreePhaseModeEnabled', true);
    call(
      'setThreePhaseRelPermProps',
      payload.s_wc,
      payload.s_or,
      payload.s_gc ?? 0.05,
      payload.s_gr ?? 0.05,
      payload.s_org ?? 0.15,
      payload.n_w,
      payload.n_o,
      payload.n_g ?? 1.5,
      payload.k_rw_max ?? 1.0,
      payload.k_ro_max ?? 1.0,
      payload.k_rg_max ?? 1.0,
    );
    if (payload.scalTables) {
      call('setThreePhaseScalTables', payload.scalTables);
    }
    call('setGasFluidProperties', payload.mu_g ?? 0.02, payload.c_g ?? 1e-4, payload.rho_g ?? 10.0);
    call('setGasRedissolutionEnabled', payload.gasRedissolutionEnabled !== false);
    if (payload.pcogEnabled) {
      call('setGasOilCapillaryParams', payload.pcogPEntry ?? 0, payload.pcogLambda ?? 2);
    }
    call('setInjectedFluid', payload.injectedFluid ?? 'gas');
    if (payload.initialGasSaturationPerLayer && payload.initialGasSaturationPerLayer.length > 0) {
      call('setInitialGasSaturationPerLayer', new Float64Array(payload.initialGasSaturationPerLayer));
    } else if ((payload.initialGasSaturation ?? 0) > 0) {
      call('setInitialGasSaturation', payload.initialGasSaturation);
    }
  }

  call(
    'setStabilityParams',
    payload.max_sat_change_per_step,
    payload.max_pressure_change_per_step,
    payload.max_well_rate_change_fraction,
  );
  call('setWellControlModes', String(payload.injectorControlMode ?? 'pressure'), String(payload.producerControlMode ?? 'pressure'));
  call('setTargetWellRates', Number(payload.targetInjectorRate ?? 0), Number(payload.targetProducerRate ?? 0));
  call('setTargetWellSurfaceRates', Number(payload.targetInjectorSurfaceRate ?? 0), Number(payload.targetProducerSurfaceRate ?? 0));

  const producerBhp = Number(payload.producerBhp ?? 100);
  const injectorBhp = Number(payload.injectorBhp ?? 500);
  const prodIsRate = String(payload.producerControlMode ?? 'pressure') === 'rate';
  const bhpMin = Number(payload.bhpMin ?? (prodIsRate ? 0 : Math.min(producerBhp, injectorBhp)));
  const bhpMax = Number(payload.bhpMax ?? Math.max(producerBhp, injectorBhp));
  call('setWellBhpLimits', bhpMin, bhpMax);
  simulator.setPermeabilityPerLayer(new Float64Array(payload.permsX), new Float64Array(payload.permsY), new Float64Array(payload.permsZ));

  const explicitWells: SimulatorWellDefinition[] = Array.isArray(payload.wells) && payload.wells.length > 0
    ? payload.wells
    : [
        {
          id: 'producer-main',
          injector: false,
          bhp: producerBhp,
          wellRadius: payload.well_radius,
          skin: payload.well_skin,
          completions: (Array.isArray(payload.producerKLayers)
            ? payload.producerKLayers
            : Array.from({ length: payload.nz }, (_, i) => i)
          ).map((k) => ({ i: Number(payload.producerI ?? (payload.nx - 1)), j: Number(payload.producerJ ?? 0), k })),
          schedule: {
            controlMode: payload.producerControlMode === 'rate' ? 'rate' : 'pressure',
            targetRate: payload.targetProducerRate,
            targetSurfaceRate: payload.targetProducerSurfaceRate,
            bhpLimit: payload.bhpMin,
            enabled: true as boolean,
          } satisfies SimulatorWellSchedule,
        },
        ...(payload.injectorEnabled === false ? [] : [{
          id: 'injector-main',
          injector: true,
          bhp: injectorBhp,
          wellRadius: payload.well_radius,
          skin: payload.well_skin,
          completions: (Array.isArray(payload.injectorKLayers)
            ? payload.injectorKLayers
            : Array.from({ length: payload.nz }, (_, i) => i)
          ).map((k) => ({ i: Number(payload.injectorI ?? 0), j: Number(payload.injectorJ ?? 0), k })),
          schedule: {
            controlMode: payload.injectorControlMode === 'rate' ? 'rate' : 'pressure',
            targetRate: payload.targetInjectorRate,
            targetSurfaceRate: payload.targetInjectorSurfaceRate,
            bhpLimit: payload.bhpMax,
            enabled: true as boolean,
          } satisfies SimulatorWellSchedule,
        }]),
      ];

  for (const well of explicitWells) {
    if (well.schedule?.enabled === false) continue;
    for (const completion of well.completions) {
      const addWellWithId = (simulator as unknown as Record<string, unknown>).addWellWithId;
      if (typeof addWellWithId === 'function') {
        (addWellWithId as (...args: unknown[]) => unknown).call(
          simulator,
          completion.i,
          completion.j,
          completion.k,
          Number(well.bhp),
          Number(well.wellRadius),
          Number(well.skin),
          Boolean(well.injector),
          String(well.id),
        );
      } else {
        simulator.add_well(
          completion.i,
          completion.j,
          completion.k,
          Number(well.bhp),
          Number(well.wellRadius),
          Number(well.skin),
          Boolean(well.injector),
        );
      }
    }
    const setWellSchedule = (simulator as unknown as Record<string, unknown>).setWellSchedule;
    if (typeof setWellSchedule === 'function') {
      (setWellSchedule as (...args: unknown[]) => unknown).call(
        simulator,
        String(well.id),
        String(well.schedule?.controlMode ?? 'pressure'),
        Number(well.schedule?.targetRate ?? Number.NaN),
        Number(well.schedule?.targetSurfaceRate ?? Number.NaN),
        Number(well.schedule?.bhpLimit ?? Number.NaN),
        well.schedule?.enabled !== false as boolean,
      );
    }
  }

  return simulator;
}

const wasmBytes = await readFile(new URL('../src/lib/ressim/pkg/simulator_bg.wasm', import.meta.url));
await init({ module_or_path: wasmBytes });

const params = getScenarioWithVariantParams('spe1_gas_injection', 'grid', 'grid_5');
const payload = buildBenchmarkCreatePayload(params);
const simulator = configureSimulator(payload);
const producerIndex = ((2 * payload.ny) + Number(payload.producerJ ?? 0)) * payload.nx + Number(payload.producerI ?? (payload.nx - 1));

let lastSig = '';
let repeatCount = 0;
for (let step = 0; step < Number(params.steps ?? 120); step++) {
  simulator.step(Number(params.delta_t_days ?? 30));
  const pressure = simulator.getPressures();
  const sw = simulator.getSatWater();
  const so = simulator.getSatOil();
  const sg = simulator.getSatGas();
  const ratePoint = simulator.getRateHistory().at(-1);
  const sig = [pressure[producerIndex], sw[producerIndex], so[producerIndex], sg[producerIndex], ratePoint?.producing_gor ?? -1]
    .map((value) => Number(value).toFixed(8))
    .join('|');
  repeatCount = sig === lastSig ? repeatCount + 1 : 0;
  lastSig = sig;

  if ((step + 1) % 10 === 0 || repeatCount >= 3 || step + 1 === Number(params.steps ?? 120)) {
    const maxSg = Array.from(sg).reduce((max, value) => Math.max(max, value), 0);
    const avgSg = Array.from(sg).reduce((sum, value) => sum + value, 0) / sg.length;
    console.log(JSON.stringify({
      step: step + 1,
      time: simulator.get_time(),
      producerPressure: pressure[producerIndex],
      producerSw: sw[producerIndex],
      producerSo: so[producerIndex],
      producerSg: sg[producerIndex],
      producingGor: ratePoint?.producing_gor ?? null,
      gasRate: ratePoint?.total_production_gas ?? null,
      oilRate: ratePoint?.total_production_oil ?? null,
      maxSg,
      avgSg,
      warning: simulator.getLastSolverWarning(),
      repeatCount,
    }));
  }
}

simulator.free();
