import init, { ReservoirSimulator } from '../ressim/pkg/simulator.js';
import type { SimulatorCreatePayload, WorkerRunPayload } from '../simulator-types';

let wasmReady = false;
let simulator: ReservoirSimulator | null = null;
let isRunning = false;
let stopRequested = false;

function buildRunProfile(batchStart: number, stepMsTotal: number, completedSteps: number, snapshotsSent: number) {
  return {
    batchMs: performance.now() - batchStart,
    avgStepMs: completedSteps > 0 ? stepMsTotal / completedSteps : 0,
    snapshotsSent,
  } as { batchMs: number; avgStepMs: number; snapshotsSent: number };
}

function postStopped(batchStart: number, stepMsTotal: number, completedSteps: number, snapshotsSent: number): void {
  post('stopped', {
    reason: 'user',
    completedSteps,
    profile: buildRunProfile(batchStart, stepMsTotal, completedSteps, snapshotsSent),
  });
}

function formatWorkerError(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  const lower = raw.toLowerCase();

  if (lower.includes('out of bounds') || lower.includes('indices')) {
    return `${raw}. Check grid size and well locations.`;
  }
  if (lower.includes('finite') || lower.includes('nan') || lower.includes('inf')) {
    return `${raw}. One or more inputs are invalid; review controls highlighted in red.`;
  }
  if (lower.includes('permeability') || lower.includes('viscos')) {
    return `${raw}. Ensure permeability and fluid properties are positive and physically reasonable.`;
  }
  if (lower.includes('initialized')) {
    return `${raw}. Reset the model and retry.`;
  }

  return `${raw}. Try reducing timestep or resetting the model after validating inputs.`;
}

function post(type: string, payload: Record<string, any> = {}): void {
  self.postMessage({ type, ...payload });
}

function getStatePayload(recordHistory: boolean, stepIndex: number, profile: Record<string, any> = {}): Record<string, any> {
  if (!simulator) {
    throw new Error('Simulator not initialized');
  }

  const extractStart = performance.now();
  const getSatGas = (simulator as unknown as Record<string, unknown>).getSatGas;
  const grid = {
    pressure: simulator.getPressures(),
    sat_water: simulator.getSatWater(),
    sat_oil: simulator.getSatOil(),
    sat_gas: typeof getSatGas === 'function'
      ? (getSatGas as () => Float64Array).call(simulator)
      : new Float64Array(simulator.getPressures().length),
  };
  const wells = simulator.getWellState();
  const time = simulator.get_time();
  const rateHistory = simulator.getRateHistory();
  const solverWarning = simulator.getLastSolverWarning();
  const extractMs = performance.now() - extractStart;

  return {
    grid,
    wells,
    time,
    rateHistory,
    solverWarning,
    recordHistory,
    stepIndex,
    profile: {
      ...profile,
      extractMs,
    },
  };
}

function configureSimulator(payload: SimulatorCreatePayload) {
  simulator = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));

  if (payload.cellDzPerLayer && payload.cellDzPerLayer.length > 0) {
    const setCellDimensionsPerLayer = (simulator as any).setCellDimensionsPerLayer;
    if (typeof setCellDimensionsPerLayer === 'function') {
      setCellDimensionsPerLayer.call(simulator, Number(payload.cellDx), Number(payload.cellDy), new Float64Array(payload.cellDzPerLayer));
    }
  } else {
    const setCellDimensions = /** @type {any} */ (simulator).setCellDimensions;
    if (typeof setCellDimensions === 'function') {
      setCellDimensions.call(simulator, Number(payload.cellDx), Number(payload.cellDy), Number(payload.cellDz));
    }
  }

  const setFluidProperties = /** @type {any} */ (simulator).setFluidProperties;
  if (typeof setFluidProperties === 'function') {
    setFluidProperties.call(simulator, Number(payload.mu_o), Number(payload.mu_w));
  }

  const setFluidCompressibilities = /** @type {any} */ (simulator).setFluidCompressibilities;
  if (typeof setFluidCompressibilities === 'function') {
    setFluidCompressibilities.call(simulator, Number(payload.c_o), Number(payload.c_w));
  }

  const setPvtTable = (simulator as any).setPvtTable;
  if (payload.pvtMode === 'black-oil' && payload.pvtTable && typeof setPvtTable === 'function') {
    setPvtTable.call(simulator, payload.pvtTable);
  }

  const setRockProperties = /** @type {any} */ (simulator).setRockProperties;
  if (typeof setRockProperties === 'function') {
    setRockProperties.call(
      simulator,
      Number(payload.rock_compressibility),
      Number(payload.depth_reference),
      Number(payload.volume_expansion_o),
      Number(payload.volume_expansion_w)
    );
  }

  const setFluidDensities = /** @type {any} */ (simulator).setFluidDensities;
  if (typeof setFluidDensities === 'function') {
    setFluidDensities.call(simulator, Number(payload.rho_o), Number(payload.rho_w));
  }

  simulator.setInitialPressure(payload.initialPressure);

  // Per-layer initial water saturation takes precedence over scalar
  if (payload.initialSaturationPerLayer && payload.initialSaturationPerLayer.length > 0) {
    const setPerLayer = (simulator as any).setInitialSaturationPerLayer;
    if (typeof setPerLayer === 'function') {
      setPerLayer.call(simulator, new Float64Array(payload.initialSaturationPerLayer));
    }
  } else {
    simulator.setInitialSaturation(payload.initialSaturation);
  }

  const setCapillaryParams = /** @type {any} */ (simulator).setCapillaryParams;
  if (typeof setCapillaryParams === 'function') {
    const pEntry = Boolean(payload.capillaryEnabled) ? Number(payload.capillaryPEntry) : 0;
    setCapillaryParams.call(simulator, pEntry, Number(payload.capillaryLambda));
  }

  const setGravityEnabled = /** @type {any} */ (simulator).setGravityEnabled;
  if (typeof setGravityEnabled === 'function') {
    setGravityEnabled.call(simulator, Boolean(payload.gravityEnabled));
  }
  simulator.setRelPermProps(payload.s_wc, payload.s_or, payload.n_w, payload.n_o, payload.k_rw_max ?? 1.0, payload.k_ro_max ?? 1.0);

  // Three-phase setup (only when enabled)
  if (payload.threePhaseModeEnabled) {
    const call3p = (name: string, ...args: unknown[]) => {
      const fn = (simulator as unknown as Record<string, unknown>)[name];
      if (typeof fn === 'function') (fn as (...a: unknown[]) => unknown).call(simulator, ...args);
    };

    call3p('setThreePhaseModeEnabled', true);
    call3p(
      'setThreePhaseRelPermProps',
      payload.s_wc, payload.s_or,
      payload.s_gc ?? 0.05, payload.s_gr ?? 0.05, payload.s_org ?? 0.15,
      payload.n_w, payload.n_o, payload.n_g ?? 1.5,
      payload.k_rw_max ?? 1.0, payload.k_ro_max ?? 1.0, payload.k_rg_max ?? 1.0,
    );
    if (payload.scalTables) {
      call3p('setThreePhaseScalTables', payload.scalTables);
    }
    call3p(
      'setGasFluidProperties',
      payload.mu_g ?? 0.02, payload.c_g ?? 1e-4, payload.rho_g ?? 10.0,
    );
    if (payload.pcogEnabled) {
      call3p('setGasOilCapillaryParams', payload.pcogPEntry ?? 0, payload.pcogLambda ?? 2);
    }
    call3p('setInjectedFluid', payload.injectedFluid ?? 'gas');
    // Per-layer initial gas saturation takes precedence over scalar
    if (payload.initialGasSaturationPerLayer && payload.initialGasSaturationPerLayer.length > 0) {
      call3p('setInitialGasSaturationPerLayer', new Float64Array(payload.initialGasSaturationPerLayer));
    } else if ((payload.initialGasSaturation ?? 0) > 0) {
      call3p('setInitialGasSaturation', payload.initialGasSaturation);
    }
  }

  simulator.setStabilityParams(
    payload.max_sat_change_per_step,
    payload.max_pressure_change_per_step,
    payload.max_well_rate_change_fraction
  );

  const setWellControlModes = /** @type {any} */ (simulator).setWellControlModes;
  if (typeof setWellControlModes === 'function') {
    setWellControlModes.call(
      simulator,
      String(payload.injectorControlMode ?? 'pressure'),
      String(payload.producerControlMode ?? 'pressure')
    );
  } else {
    const setRateControlledWells = /** @type {any} */ (simulator).setRateControlledWells;
    if (typeof setRateControlledWells === 'function') {
      setRateControlledWells.call(simulator, Boolean(payload.rateControlledWells));
    }
  }

  const setTargetWellRates = /** @type {any} */ (simulator).setTargetWellRates;
  if (typeof setTargetWellRates === 'function') {
    const targetInjectorRate = Number(payload.targetInjectorRate ?? 0);
    const targetProducerRate = Number(payload.targetProducerRate ?? targetInjectorRate);
    setTargetWellRates.call(simulator, targetInjectorRate, targetProducerRate);
  }

  const setTargetWellSurfaceRates = /** @type {any} */ (simulator).setTargetWellSurfaceRates;
  if (typeof setTargetWellSurfaceRates === 'function') {
    setTargetWellSurfaceRates.call(
      simulator,
      Number(payload.targetInjectorSurfaceRate ?? 0),
      Number(payload.targetProducerSurfaceRate ?? 0),
    );
  }

  const setWellBhpLimits = /** @type {any} */ (simulator).setWellBhpLimits;
  if (typeof setWellBhpLimits === 'function') {
    const producerBhp = Number(payload.producerBhp ?? 100);
    const injectorBhp = Number(payload.injectorBhp ?? 500);
    const injIsRate = String(payload.injectorControlMode ?? 'pressure') === 'rate';
    const prodIsRate = String(payload.producerControlMode ?? 'pressure') === 'rate';
    // When rate-controlled, allow wide BHP range so rate targets can be achieved.
    // For BHP-controlled wells, use the specified BHP values as limits.
    const defaultBhpMin = prodIsRate ? 0 : Math.min(producerBhp, injectorBhp);
    const defaultBhpMax = Math.max(producerBhp, injectorBhp);
    const bhpMin = Number(payload.bhpMin ?? defaultBhpMin);
    const bhpMax = Number(payload.bhpMax ?? defaultBhpMax);
    setWellBhpLimits.call(simulator, bhpMin, bhpMax);
  }

  if (payload.permMode === 'random') {
    if (payload.useRandomSeed) {
      const seed = typeof payload.randomSeed === 'bigint' ? payload.randomSeed : BigInt(Math.floor(Number(payload.randomSeed ?? 0)));
      simulator.setPermeabilityRandomSeeded(payload.minPerm, payload.maxPerm, seed);
    } else {
      simulator.setPermeabilityRandom(payload.minPerm, payload.maxPerm);
    }
  } else if (payload.permMode === 'perLayer' || payload.permMode === 'uniform') {
    simulator.setPermeabilityPerLayer(new Float64Array(payload.permsX), new Float64Array(payload.permsY), new Float64Array(payload.permsZ));
  }

  const producerI = Number(payload.producerI ?? (payload.nx - 1));
  const producerJ = Number(payload.producerJ ?? 0);
  const injectorI = Number(payload.injectorI ?? 0);
  const injectorJ = Number(payload.injectorJ ?? 0);
  const producerBhp = Number(payload.producerBhp ?? 100);
  const injectorBhp = Number(payload.injectorBhp ?? 500);

  const producerKLayers: number[] = Array.isArray(payload.producerKLayers)
    ? payload.producerKLayers
    : Array.from({ length: payload.nz }, (_, i) => i);
  const injectorKLayers: number[] = Array.isArray(payload.injectorKLayers)
    ? payload.injectorKLayers
    : Array.from({ length: payload.nz }, (_, i) => i);

  try {
    for (const k of producerKLayers) {
      simulator.add_well(producerI, producerJ, k, producerBhp, payload.well_radius, payload.well_skin, false);
    }
    if (Boolean(payload.injectorEnabled ?? true)) {
      for (const k of injectorKLayers) {
        simulator.add_well(injectorI, injectorJ, k, injectorBhp, payload.well_radius, payload.well_skin, true);
      }
    }
  } catch (err: any) {
    throw new Error(`Failed to configure wells: ${err?.message || err}`);
  }
}


self.onmessage = async (event) => {
  const { type, payload } = event.data ?? {};

  try {
    if (type === 'stop') {
      if (isRunning) {
        stopRequested = true;
        post('warning', { message: 'Stopping simulation after current chunk…' });
      } else {
        post('stopped', { reason: 'idle' });
      }
      return;
    }

    if (type === 'init') {
      if (!wasmReady) {
        await init();
        wasmReady = true;
      }
      post('ready');
      return;
    }

    if (type === 'create') {
      if (!wasmReady) {
        await init();
        wasmReady = true;
      }

      try {
        configureSimulator(payload);
      } catch (error) {
        simulator = null;
        throw error;
      }
      post('state', getStatePayload(false, -1, { batchMs: 0, avgStepMs: 0, snapshotsSent: 0 }));
      return;
    }



    if (type === 'run') {
      if (!simulator) {
        throw new Error('Simulator not initialized');
      }
      if (isRunning) {
        throw new Error('Simulator is already running');
      }

      const runPayload = payload as WorkerRunPayload;

      // If continuing an existing run, load the previous state
      if (runPayload.history && runPayload.history.length > 0) {
        const lastHistory = runPayload.history[runPayload.history.length - 1];
        if (lastHistory && lastHistory.grid && lastHistory.wells) {
          const loadStateFn = /** @type {any} */ (simulator).loadState;
          if (typeof loadStateFn === 'function') {
            const rateHistoryPayload = payload.rateHistory ?? [];
            loadStateFn.call(
              simulator,
              lastHistory.time,
              lastHistory.grid,
              lastHistory.wells,
              rateHistoryPayload
            );
          }
        }
      }

      const steps = Math.max(0, Math.floor(Number(runPayload?.steps ?? 0)));
      const deltaTDays = Number(runPayload?.deltaTDays ?? 0);
      const historyInterval = Math.max(1, Number(runPayload?.historyInterval ?? 1));
      const chunkYieldInterval = Math.max(1, Number(runPayload?.chunkYieldInterval ?? 5));

      if (!Number.isFinite(deltaTDays) || deltaTDays <= 0) {
        throw new Error(`Invalid timestep value: ${deltaTDays}`);
      }

      const batchStart = performance.now();
      let stepMsTotal = 0;
      let snapshotsSent = 0;
      isRunning = true;
      stopRequested = false;
      post('runStarted', { steps, deltaTDays });

      // Emit the fully initialized pre-step state so downstream charts and
      // playback have an actual t=0 snapshot rather than a synthetic origin.
      post('state', getStatePayload(true, -1, {
        batchMs: 0,
        avgStepMs: 0,
        snapshotsSent: 1,
      }));
      snapshotsSent = 1;

      let lastYieldTime = performance.now();

      for (let i = 0; i < steps; i++) {
        if (stopRequested) {
          postStopped(batchStart, stepMsTotal, i, snapshotsSent);
          isRunning = false;
          stopRequested = false;
          return;
        }

        const stepStart = performance.now();
        simulator.step(deltaTDays);
        stepMsTotal += performance.now() - stepStart;

        if (stopRequested) {
          postStopped(batchStart, stepMsTotal, i + 1, snapshotsSent);
          isRunning = false;
          stopRequested = false;
          return;
        }

        const shouldRecord = i % historyInterval === 0 || i === steps - 1;
        if (shouldRecord) {
          snapshotsSent += 1;
          post(
            'state',
            getStatePayload(true, i, {
              batchMs: performance.now() - batchStart,
              avgStepMs: stepMsTotal / (i + 1),
              snapshotsSent,
            })
          );
        }

        const timeSinceLastYield = performance.now() - lastYieldTime;
        if ((i + 1) % chunkYieldInterval === 0 || timeSinceLastYield > 16) {
          await new Promise((resolve) => setTimeout(resolve, 0));
          lastYieldTime = performance.now();

          if (stopRequested) {
            postStopped(batchStart, stepMsTotal, i + 1, snapshotsSent);
            isRunning = false;
            stopRequested = false;
            return;
          }
        }
      }

      if (stopRequested) {
        postStopped(batchStart, stepMsTotal, steps, snapshotsSent);
        isRunning = false;
        stopRequested = false;
        return;
      }

      post('batchComplete', {
        profile: buildRunProfile(batchStart, stepMsTotal, steps, snapshotsSent),
      });
      isRunning = false;
      stopRequested = false;
      return;
    }

    if (type === 'dispose') {
      simulator = null;
      close();
    }
  } catch (error) {
    isRunning = false;
    stopRequested = false;
    post('error', { message: formatWorkerError(error) });
  }
};
