import init, { ReservoirSimulator } from './ressim/pkg/simulator.js';

let wasmReady = false;
let simulator = null;

function post(type, payload = {}) {
  self.postMessage({ type, ...payload });
}

function getStatePayload(recordHistory, stepIndex, profile = {}) {
  if (!simulator) {
    throw new Error('Simulator not initialized');
  }

  const extractStart = performance.now();
  const grid = simulator.getGridState();
  const wells = simulator.getWellState();
  const time = simulator.get_time();
  const rateHistory = simulator.getRateHistory();
  const extractMs = performance.now() - extractStart;

  return {
    grid,
    wells,
    time,
    rateHistory,
    recordHistory,
    stepIndex,
    profile: {
      ...profile,
      extractMs,
    },
  };
}

function configureSimulator(payload) {
  simulator = new ReservoirSimulator(payload.nx, payload.ny, payload.nz);

  const setCellDimensions = /** @type {any} */ (simulator).setCellDimensions;
  if (typeof setCellDimensions === 'function') {
    setCellDimensions.call(simulator, Number(payload.cellDx), Number(payload.cellDy), Number(payload.cellDz));
  }

  const setFluidProperties = /** @type {any} */ (simulator).setFluidProperties;
  if (typeof setFluidProperties === 'function') {
    setFluidProperties.call(simulator, Number(payload.mu_o), Number(payload.mu_w));
  }

  const setFluidDensities = /** @type {any} */ (simulator).setFluidDensities;
  if (typeof setFluidDensities === 'function') {
    setFluidDensities.call(simulator, Number(payload.rho_o), Number(payload.rho_w));
  }

  simulator.setInitialPressure(payload.initialPressure);
  simulator.setInitialSaturation(payload.initialSaturation);

  const setCapillaryParams = /** @type {any} */ (simulator).setCapillaryParams;
  if (typeof setCapillaryParams === 'function') {
    const pEntry = Boolean(payload.capillaryEnabled) ? Number(payload.capillaryPEntry) : 0;
    setCapillaryParams.call(simulator, pEntry, Number(payload.capillaryLambda));
  }

  const setGravityEnabled = /** @type {any} */ (simulator).setGravityEnabled;
  if (typeof setGravityEnabled === 'function') {
    setGravityEnabled.call(simulator, Boolean(payload.gravityEnabled));
  }
  simulator.setRelPermProps(payload.s_wc, payload.s_or, payload.n_w, payload.n_o);
  simulator.setStabilityParams(payload.max_sat_change_per_step);

  const setRateControlledWells = /** @type {any} */ (simulator).setRateControlledWells;
  if (typeof setRateControlledWells === 'function') {
    setRateControlledWells.call(simulator, Boolean(payload.rateControlledWells));
  }

  const setTargetWellRates = /** @type {any} */ (simulator).setTargetWellRates;
  if (typeof setTargetWellRates === 'function') {
    const targetInjectorRate = Number(payload.targetInjectorRate ?? 0);
    const targetProducerRate = Number(payload.targetProducerRate ?? targetInjectorRate);
    setTargetWellRates.call(simulator, targetInjectorRate, targetProducerRate);
  }

  if (payload.permMode === 'random') {
    if (payload.useRandomSeed) {
      simulator.setPermeabilityRandomSeeded(payload.minPerm, payload.maxPerm, payload.randomSeed);
    } else {
      simulator.setPermeabilityRandom(payload.minPerm, payload.maxPerm);
    }
  } else if (payload.permMode === 'perLayer') {
    simulator.setPermeabilityPerLayer(payload.permsX, payload.permsY, payload.permsZ);
  }

  const clampIndex = (value, maxExclusive) => Math.max(0, Math.min(maxExclusive - 1, Number(value)));
  const producerI = clampIndex(payload.producerI ?? (payload.nx - 1), payload.nx);
  const producerJ = clampIndex(payload.producerJ ?? 0, payload.ny);
  const injectorI = clampIndex(payload.injectorI ?? 0, payload.nx);
  const injectorJ = clampIndex(payload.injectorJ ?? 0, payload.ny);
  const producerBhp = Number(payload.producerBhp ?? 100);
  const injectorBhp = Number(payload.injectorBhp ?? 400);

  for (let i = 0; i < payload.nz; i++) {
    simulator.add_well(producerI, producerJ, i, producerBhp, payload.well_radius, payload.well_skin, false);
  }
  for (let i = 0; i < payload.nz; i++) {
    simulator.add_well(injectorI, injectorJ, i, injectorBhp, payload.well_radius, payload.well_skin, true);
  }
}

self.onmessage = async (event) => {
  const { type, payload } = event.data ?? {};

  try {
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

      configureSimulator(payload);
      post('state', getStatePayload(false, -1, { batchMs: 0, avgStepMs: 0, snapshotsSent: 0 }));
      return;
    }

    if (type === 'run') {
      if (!simulator) {
        throw new Error('Simulator not initialized');
      }

      const steps = Number(payload?.steps ?? 0);
      const deltaTDays = Number(payload?.deltaTDays ?? 0);
      const historyInterval = Math.max(1, Number(payload?.historyInterval ?? 1));

      const batchStart = performance.now();
      let stepMsTotal = 0;
      let snapshotsSent = 0;

      for (let i = 0; i < steps; i++) {
        const stepStart = performance.now();
        simulator.step(deltaTDays);
        stepMsTotal += performance.now() - stepStart;

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
      }

      post('batchComplete', {
        profile: {
          batchMs: performance.now() - batchStart,
          avgStepMs: steps > 0 ? stepMsTotal / steps : 0,
          snapshotsSent,
        },
      });
      return;
    }

    if (type === 'dispose') {
      simulator = null;
      close();
    }
  } catch (error) {
    post('error', { message: error instanceof Error ? error.message : String(error) });
  }
};
