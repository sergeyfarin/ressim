#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import { mkdir, writeFile } from 'node:fs/promises';
import process from 'node:process';
import init, { ReservoirSimulator } from '../src/lib/ressim/pkg/simulator.js';

const PRESETS = {
  'water-pressure': {
    description: 'Two-phase waterflood with pressure-controlled injector and producer',
    defaults: {
      nx: 24,
      ny: 1,
      nz: 1,
      dt: 0.25,
      steps: 1,
      wells: 'both',
      control: 'pressure',
      gravity: null,
      capillary: true,
    },
    configure: configureWaterfloodPressure,
  },
  'water-rate': {
    description: 'Two-phase waterflood with rate-controlled injector and producer',
    defaults: {
      nx: 24,
      ny: 1,
      nz: 1,
      dt: 0.25,
      steps: 1,
      wells: 'both',
      control: 'rate',
      gravity: null,
      capillary: true,
    },
    configure: configureWaterfloodRate,
  },
  'gas-pressure': {
    description: 'Three-phase gas injection with pressure-controlled wells',
    defaults: {
      nx: 24,
      ny: 1,
      nz: 1,
      dt: 0.25,
      steps: 1,
      wells: 'both',
      control: 'pressure',
      gravity: null,
      capillary: true,
    },
    configure: configureGasInjectionPressure,
  },
  'gas-rate': {
    description: 'Three-phase gas injection with rate-controlled wells',
    defaults: {
      nx: 24,
      ny: 1,
      nz: 1,
      dt: 0.25,
      steps: 1,
      wells: 'both',
      control: 'rate',
      gravity: null,
      capillary: true,
    },
    configure: configureGasInjectionRate,
  },
  'sweep-areal': {
    description: 'Pressure-controlled areal sweep style baseline',
    defaults: {
      nx: 21,
      ny: 21,
      nz: 1,
      dt: 0.25,
      steps: 1,
      wells: 'both',
      control: 'pressure',
      gravity: false,
      capillary: true,
    },
    configure: configureArealSweep,
  },
};

function printHelp() {
  console.log(`Usage: node scripts/fim-wasm-diagnostic.mjs [options]

Options:
  --preset <name>           Preset name (${Object.keys(PRESETS).join(', ')})
  --grid <nx>x<ny>x<nz>     Grid dimensions override
  --nx <n>                  Override nx
  --ny <n>                  Override ny
  --nz <n>                  Override nz
  --dt <days>               Outer-step size in days
  --steps <n>               Number of outer steps to run
  --wells <layout>          both | injector-only | producer-only
  --control <mode>          pressure | rate
  --gravity <bool>          true | false
  --capillary <bool>        true | false
  --diagnostic <mode>       quiet | summary | outer | step
  --checkpoint-in <file>    Load simulator state checkpoint before running
  --checkpoint-out <file>   Save simulator state checkpoint after the run
  --checkpoint-every <n>    Save a checkpoint every n outer steps
  --checkpoint-dir <dir>    Directory used with --checkpoint-every
  --json                    Emit final JSON summary to stdout (default true)
  --no-json                 Suppress final JSON summary
  --list                    List presets
  --help                    Show this help

Examples:
  node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 3 --diagnostic step
  node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --wells injector-only --dt 0.1 --steps 5
  node scripts/fim-wasm-diagnostic.mjs --preset water-rate --control rate --grid 24x1x1

Current diagnostic granularity:
  summary = structured outer-step summaries and solver warnings.
  outer = per-outer-step summaries without full Newton traces.
  step = per-step summaries plus captured per-Newton and retry traces from the FIM solver.
`);
}

function printPresets() {
  for (const [name, preset] of Object.entries(PRESETS)) {
    console.log(`${name.padEnd(16)} ${preset.description}`);
  }
}

function parseBool(value, flag) {
  if (value === 'true') return true;
  if (value === 'false') return false;
  throw new Error(`Expected true or false for ${flag}, got ${value}`);
}

function parseGrid(value) {
  const match = /^(\d+)x(\d+)x(\d+)$/i.exec(value);
  if (!match) {
    throw new Error(`Expected grid in nx x ny x nz form, got ${value}`);
  }
  return {
    nx: Number(match[1]),
    ny: Number(match[2]),
    nz: Number(match[3]),
  };
}

function parseArgs(argv) {
  const options = {
    preset: 'water-pressure',
    diagnostic: 'summary',
    emitJson: true,
  };

  if (argv.length === 1 && /^\d+$/.test(argv[0])) {
    options.nx = Number(argv[0]);
    return options;
  }

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    switch (arg) {
      case '--preset':
        options.preset = next;
        index += 1;
        break;
      case '--grid': {
        const grid = parseGrid(next);
        Object.assign(options, grid);
        index += 1;
        break;
      }
      case '--nx':
        options.nx = Number(next);
        index += 1;
        break;
      case '--ny':
        options.ny = Number(next);
        index += 1;
        break;
      case '--nz':
        options.nz = Number(next);
        index += 1;
        break;
      case '--dt':
        options.dt = Number(next);
        index += 1;
        break;
      case '--steps':
        options.steps = Number(next);
        index += 1;
        break;
      case '--wells':
        options.wells = next;
        index += 1;
        break;
      case '--control':
        options.control = next;
        index += 1;
        break;
      case '--gravity':
        options.gravity = parseBool(next, '--gravity');
        index += 1;
        break;
      case '--capillary':
        options.capillary = parseBool(next, '--capillary');
        index += 1;
        break;
      case '--diagnostic':
        options.diagnostic = next;
        index += 1;
        break;
      case '--checkpoint-in':
        options.checkpointIn = next;
        index += 1;
        break;
      case '--checkpoint-out':
        options.checkpointOut = next;
        index += 1;
        break;
      case '--checkpoint-every':
        options.checkpointEvery = Number(next);
        index += 1;
        break;
      case '--checkpoint-dir':
        options.checkpointDir = next;
        index += 1;
        break;
      case '--json':
        options.emitJson = true;
        break;
      case '--no-json':
        options.emitJson = false;
        break;
      case '--list':
        options.list = true;
        break;
      case '--help':
        options.help = true;
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function buildOptions(parsed) {
  const preset = PRESETS[parsed.preset];
  if (!preset) {
    throw new Error(`Unknown preset: ${parsed.preset}`);
  }

  const resolved = { ...preset.defaults, ...parsed };
  if (!Number.isFinite(resolved.nx) || !Number.isFinite(resolved.ny) || !Number.isFinite(resolved.nz)) {
    throw new Error('Grid dimensions must be numeric');
  }
  if (!['both', 'injector-only', 'producer-only'].includes(resolved.wells)) {
    throw new Error(`Unsupported wells layout: ${resolved.wells}`);
  }
  if (!['pressure', 'rate'].includes(resolved.control)) {
    throw new Error(`Unsupported control mode: ${resolved.control}`);
  }
  if (!['quiet', 'summary', 'outer', 'step'].includes(resolved.diagnostic)) {
    throw new Error(`Unsupported diagnostic mode: ${resolved.diagnostic}`);
  }
  if (resolved.checkpointEvery != null) {
    if (!Number.isFinite(resolved.checkpointEvery) || resolved.checkpointEvery <= 0) {
      throw new Error('checkpoint-every must be a positive integer');
    }
    if (!resolved.checkpointDir) {
      throw new Error('checkpoint-dir is required when checkpoint-every is set');
    }
  }
  if (resolved.gravity == null) {
    resolved.gravity = resolved.nz > 1;
  }
  resolved.presetConfig = preset;
  return resolved;
}

async function readCheckpoint(filePath) {
  const raw = await readFile(filePath, 'utf8');
  const checkpoint = JSON.parse(raw);
  if (!checkpoint?.grid || !checkpoint?.wells || !Array.isArray(checkpoint?.rateHistory)) {
    throw new Error(`Invalid checkpoint payload: ${filePath}`);
  }
  return checkpoint;
}

function buildCheckpoint(sim, options, lastRecord) {
  const grid = sim.getGridState();
  return {
    preset: options.preset,
    grid: {
      pressure: Array.from(grid.pressure ?? []),
      sat_water: Array.from(grid.sat_water ?? []),
      sat_oil: Array.from(grid.sat_oil ?? []),
      sat_gas: Array.from(grid.sat_gas ?? []),
      rs: Array.from(sim.getRs()),
    },
    wells: sim.getWellState(),
    rateHistory: sim.getRateHistory(),
    timeDays: sim.get_time(),
    lastRecord,
    options: {
      grid: { nx: options.nx, ny: options.ny, nz: options.nz },
      wells: options.wells,
      control: options.control,
      gravity: options.gravity,
      capillary: options.capillary,
      dt: options.dt,
    },
  };
}

async function writeCheckpoint(filePath, checkpoint) {
  await writeFile(filePath, `${JSON.stringify(checkpoint, null, 2)}\n`, 'utf8');
}

function checkpointPath(dir, outerStep) {
  return `${dir.replace(/\/$/, '')}/step-${String(outerStep).padStart(4, '0')}.json`;
}

function configureCommonTwoPhase(sim, options, overrides = {}) {
  sim.setFimEnabled(true);
  sim.setCellDimensions(overrides.dx ?? 10, overrides.dy ?? 10, overrides.dz ?? 1);
  sim.setRelPermProps(0.1, 0.1, 2.0, 2.0, 1.0, 1.0);
  sim.setInitialPressure(overrides.initialPressure ?? 300);
  sim.setInitialSaturation(overrides.initialSaturation ?? 0.1);
  sim.setFluidProperties(1.0, 0.5);
  sim.setFluidCompressibilities(1e-5, 3e-6);
  sim.setRockProperties(1e-6, 0.0, 1.0, 1.0);
  sim.setFluidDensities(800.0, 1000.0);
  sim.setCapillaryParams(options.capillary ? 0.0 : 0.0, options.capillary ? 2.0 : 1e-6);
  sim.setGravityEnabled(options.gravity);
  sim.setPermeabilityPerLayer(
    new Float64Array(Array.from({ length: options.nz }, () => overrides.kx ?? 2000.0)),
    new Float64Array(Array.from({ length: options.nz }, () => overrides.ky ?? 2000.0)),
    new Float64Array(Array.from({ length: options.nz }, () => overrides.kz ?? 200.0)),
  );
  sim.setStabilityParams(overrides.maxSatChange ?? 0.05, overrides.maxPressureChange ?? 75.0, overrides.maxWellRateChange ?? 0.75);
}

function setWells(sim, options, config) {
  const hasInjector = options.wells === 'both' || options.wells === 'injector-only';
  const hasProducer = options.wells === 'both' || options.wells === 'producer-only';

  sim.setWellControlModes(config.injectorControl, config.producerControl);
  sim.setTargetWellRates(config.injectorTargetRate, config.producerTargetRate);
  sim.setWellBhpLimits(config.bhpMin, config.bhpMax);
  if (config.rateControlled != null) {
    sim.setRateControlledWells(config.rateControlled);
  }
  if (config.injectedFluid) {
    sim.setInjectedFluid(config.injectedFluid);
  }

  if (hasInjector) {
    sim.add_well(0, 0, 0, config.injectorBhp, 0.1, 0.0, true);
  }
  if (hasProducer) {
    sim.add_well(options.nx - 1, options.ny - 1, 0, config.producerBhp, 0.1, 0.0, false);
  }
}

function waterWellConfig(options) {
  if (options.control === 'rate') {
    return {
      injectorControl: 'rate',
      producerControl: 'rate',
      injectorTargetRate: 10.0,
      producerTargetRate: 10.0,
      injectorBhp: 400.0,
      producerBhp: 200.0,
      bhpMin: 100.0,
      bhpMax: 500.0,
      rateControlled: true,
      injectedFluid: 'water',
    };
  }

  return {
    injectorControl: 'pressure',
    producerControl: 'pressure',
    injectorTargetRate: 0.0,
    producerTargetRate: 0.0,
    injectorBhp: 500.0,
    producerBhp: 100.0,
    bhpMin: 100.0,
    bhpMax: 500.0,
    rateControlled: false,
  };
}

function gasWellConfig(options) {
  if (options.control === 'rate') {
    return {
      injectorControl: 'rate',
      producerControl: 'rate',
      injectorTargetRate: 500.0,
      producerTargetRate: 200.0,
      injectorBhp: 350.0,
      producerBhp: 100.0,
      bhpMin: 50.0,
      bhpMax: 400.0,
      rateControlled: true,
      injectedFluid: 'gas',
    };
  }

  return {
    injectorControl: 'pressure',
    producerControl: 'pressure',
    injectorTargetRate: 0.0,
    producerTargetRate: 0.0,
    injectorBhp: 350.0,
    producerBhp: 100.0,
    bhpMin: 50.0,
    bhpMax: 400.0,
    rateControlled: false,
    injectedFluid: 'gas',
  };
}

function configureWaterfloodPressure(sim, options) {
  configureCommonTwoPhase(sim, options);
  setWells(sim, options, waterWellConfig(options));
}

function configureWaterfloodRate(sim, options) {
  configureCommonTwoPhase(sim, options);
  setWells(sim, options, waterWellConfig(options));
}

function configureGasBase(sim, options) {
  sim.setFimEnabled(true);
  sim.setCellDimensions(10, 10, 5);
  sim.setInitialPressure(200.0);
  sim.setInitialSaturation(0.15);
  sim.setInitialGasSaturation(0.0);
  sim.setFluidProperties(0.8, 0.4);
  sim.setFluidCompressibilities(1e-4, 5e-5);
  sim.setRockProperties(4e-5, 2500.0, 1.2, 1.0);
  sim.setFluidDensities(850.0, 1020.0);
  sim.setGasFluidProperties(0.02, 1e-4, 0.8);
  sim.setCapillaryParams(0.0, options.capillary ? 2.0 : 1e-6);
  sim.setGravityEnabled(options.gravity);
  sim.setPermeabilityPerLayer(
    new Float64Array(Array.from({ length: options.nz }, () => 500.0)),
    new Float64Array(Array.from({ length: options.nz }, () => 500.0)),
    new Float64Array(Array.from({ length: options.nz }, () => 50.0)),
  );
  sim.setPvtTable([
    { p_bar: 50.0, rs_m3m3: 20.0, bo_m3m3: 1.1, mu_o_cp: 1.0, bg_m3m3: 0.02, mu_g_cp: 0.015 },
    { p_bar: 150.0, rs_m3m3: 80.0, bo_m3m3: 1.25, mu_o_cp: 0.7, bg_m3m3: 0.008, mu_g_cp: 0.018 },
    { p_bar: 250.0, rs_m3m3: 140.0, bo_m3m3: 1.4, mu_o_cp: 0.5, bg_m3m3: 0.005, mu_g_cp: 0.022 },
    { p_bar: 350.0, rs_m3m3: 200.0, bo_m3m3: 1.55, mu_o_cp: 0.4, bg_m3m3: 0.004, mu_g_cp: 0.025 },
  ]);
  sim.setInitialRs(80.0);
  sim.setThreePhaseRelPermProps(0.15, 0.15, 0.05, 0.05, 0.2, 2.0, 2.0, 2.0, 1e-5, 1.0, 0.95);
  sim.setThreePhaseModeEnabled(true);
  sim.setInjectedFluid('gas');
  sim.setGasRedissolutionEnabled(false);
  sim.setStabilityParams(0.05, 75.0, 0.75);
}

function configureGasInjectionPressure(sim, options) {
  configureGasBase(sim, options);
  setWells(sim, options, gasWellConfig(options));
}

function configureGasInjectionRate(sim, options) {
  configureGasBase(sim, options);
  setWells(sim, options, gasWellConfig(options));
}

function configureArealSweep(sim, options) {
  configureCommonTwoPhase(sim, options, {
    dx: 20.0,
    dy: 20.0,
    dz: 10.0,
    kx: 200.0,
    ky: 200.0,
    kz: 20.0,
    maxSatChange: 0.01,
    maxPressureChange: 50.0,
    maxWellRateChange: 1.0,
  });
  setWells(sim, options, waterWellConfig(options));
}

function computeRange(values) {
  let min = Number.POSITIVE_INFINITY;
  let max = Number.NEGATIVE_INFINITY;
  for (const value of values) {
    if (value < min) min = value;
    if (value > max) max = value;
  }
  return { min, max };
}

function snapshot(sim, outerStep, outerMs, previousHistoryLength) {
  const history = sim.getRateHistory();
  const last = history.at(-1) ?? null;
  const fimStepStats = sim.getLastFimStepStats() ?? null;
  const pressures = sim.getPressures();
  const satWater = sim.getSatWater();
  const satGas = sim.getSatGas();
  const rs = sim.getRs();
  const pressureRange = computeRange(pressures);
  const swRange = computeRange(satWater);
  const sgRange = computeRange(satGas);
  const rsRange = computeRange(rs);

  return {
    outerStep,
    outerMs,
    timeDays: sim.get_time(),
    historyLength: history.length,
    historyDelta: history.length - previousHistoryLength,
    warning: sim.getLastSolverWarning(),
    avgPressure: last?.avg_reservoir_pressure ?? null,
    totalOil: last?.total_production_oil ?? null,
    totalLiquid: last?.total_production_liquid ?? null,
    totalInjection: last?.total_injection ?? null,
    producingGor: last?.producing_gor ?? null,
    producerBhpLimitedFraction: last?.producer_bhp_limited_fraction ?? null,
    injectorBhpLimitedFraction: last?.injector_bhp_limited_fraction ?? null,
    fimAcceptedSubsteps: fimStepStats?.accepted_substeps ?? null,
    fimLinearBadRetries: fimStepStats?.linear_bad_retries ?? null,
    fimNonlinearBadRetries: fimStepStats?.nonlinear_bad_retries ?? null,
    fimMixedRetries: fimStepStats?.mixed_retries ?? null,
    fimMinAcceptedDtDays: fimStepStats?.min_accepted_dt_days ?? null,
    fimMaxAcceptedDtDays: fimStepStats?.max_accepted_dt_days ?? null,
    fimGrowthLimiter: fimStepStats?.growth_limiter ?? null,
    fimLastRetryClass: fimStepStats?.last_retry_class ?? null,
    fimLastRetryDominantFamily: fimStepStats?.last_retry_dominant_family ?? null,
    fimLastRetryDominantRow: fimStepStats?.last_retry_dominant_row ?? null,
    pressureMin: pressureRange.min,
    pressureMax: pressureRange.max,
    swMin: swRange.min,
    swMax: swRange.max,
    sgMin: sgRange.min,
    sgMax: sgRange.max,
    rsMin: rsRange.min,
    rsMax: rsRange.max,
    fimTraceLineCount: sim.getFimTrace().split('\n').filter(Boolean).length,
    cumulativeMbErrorM3: sim.cumulative_mb_error_m3,
    cumulativeMbGasErrorM3: sim.cumulative_mb_gas_error_m3,
  };
}

function printStepSummary(record) {
  console.error(
    [
      `step=${String(record.outerStep).padStart(3, ' ')}`,
      `time=${record.timeDays.toFixed(4)}d`,
      `outer_ms=${record.outerMs.toFixed(1)}`,
      `history+=${record.historyDelta}`,
      `substeps=${record.fimAcceptedSubsteps == null ? 'n/a' : record.fimAcceptedSubsteps}`,
      `retries=${record.fimLinearBadRetries == null ? 'n/a' : `${record.fimLinearBadRetries}/${record.fimNonlinearBadRetries}/${record.fimMixedRetries}`}`,
      `avg_p=${record.avgPressure == null ? 'n/a' : record.avgPressure.toFixed(2)}`,
      `oil=${record.totalOil == null ? 'n/a' : record.totalOil.toFixed(2)}`,
      `inj=${record.totalInjection == null ? 'n/a' : record.totalInjection.toFixed(2)}`,
      `gor=${record.producingGor == null ? 'n/a' : record.producingGor.toFixed(2)}`,
      `dt=[${record.fimMinAcceptedDtDays == null ? 'n/a' : record.fimMinAcceptedDtDays.toExponential(3)},${record.fimMaxAcceptedDtDays == null ? 'n/a' : record.fimMaxAcceptedDtDays.toExponential(3)}]`,
      `growth=${record.fimGrowthLimiter ?? 'n/a'}`,
      `retry_dom=${record.fimLastRetryDominantFamily == null ? 'n/a' : `${record.fimLastRetryClass}:${record.fimLastRetryDominantFamily}@${record.fimLastRetryDominantRow}`}`,
      `p=[${record.pressureMin.toFixed(2)},${record.pressureMax.toFixed(2)}]`,
      `sg=[${record.sgMin.toFixed(4)},${record.sgMax.toFixed(4)}]`,
      record.warning ? `warning=${record.warning}` : 'warning=none',
    ].join(' | '),
  );
}

function printFimTrace(trace) {
  if (!trace.trim()) {
    return;
  }
  for (const line of trace.trimEnd().split('\n')) {
    console.error(`  ${line}`);
  }
}

async function main() {
  const parsed = parseArgs(process.argv.slice(2));
  if (parsed.help) {
    printHelp();
    return;
  }
  if (parsed.list) {
    printPresets();
    return;
  }

  const options = buildOptions(parsed);
  const wasmBytes = await readFile(new URL('../src/lib/ressim/pkg/simulator_bg.wasm', import.meta.url));
  await init({ module_or_path: wasmBytes });

  const sim = new ReservoirSimulator(options.nx, options.ny, options.nz, 0.2);
  options.presetConfig.configure(sim, options);

  if (options.checkpointIn) {
    const checkpoint = await readCheckpoint(options.checkpointIn);
    sim.loadState(
      checkpoint.timeDays,
      checkpoint.grid,
      checkpoint.wells,
      checkpoint.rateHistory,
    );
  }

  if (options.checkpointDir) {
    await mkdir(options.checkpointDir, { recursive: true });
  }

  const stepRecords = [];
  const started = performance.now();
  for (let outerStep = 1; outerStep <= options.steps; outerStep += 1) {
    const historyBefore = sim.getRateHistory().length;
    const stepStarted = performance.now();
    let fimTrace = '';
    if (options.diagnostic === 'step') {
      fimTrace = sim.stepWithDiagnostics(options.dt);
    } else {
      sim.step(options.dt);
    }
    const outerMs = performance.now() - stepStarted;
    const record = snapshot(sim, outerStep, outerMs, historyBefore);
    stepRecords.push(record);
    if (options.diagnostic === 'step') {
      printStepSummary(record);
      printFimTrace(fimTrace);
    } else if (options.diagnostic === 'outer') {
      printStepSummary(record);
    }

    if (
      options.checkpointEvery != null
      && outerStep % options.checkpointEvery === 0
      && options.checkpointDir
    ) {
      await writeCheckpoint(
        checkpointPath(options.checkpointDir, outerStep),
        buildCheckpoint(sim, options, record),
      );
    }

    if (record.warning) {
      break;
    }
  }

  if (options.checkpointOut) {
    await writeCheckpoint(
      options.checkpointOut,
      buildCheckpoint(sim, options, stepRecords.at(-1) ?? null),
    );
  }

  const result = {
    preset: options.preset,
    description: options.presetConfig.description,
    grid: { nx: options.nx, ny: options.ny, nz: options.nz },
    dtDays: options.dt,
    stepsRequested: options.steps,
    wells: options.wells,
    control: options.control,
    gravity: options.gravity,
    capillary: options.capillary,
    diagnostic: options.diagnostic,
    totalMs: performance.now() - started,
    final: stepRecords.at(-1) ?? null,
    stepRecords,
  };

  if (options.diagnostic === 'summary' && result.final) {
    printStepSummary(result.final);
  }

  if (options.emitJson) {
    console.log(JSON.stringify(result, null, 2));
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});