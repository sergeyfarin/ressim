import initWasm, { CompositionalSimulator, ReservoirSimulator, set_panic_hook } from '../ressim/pkg/simulator.js';
import type { SimulatorCreatePayload, WorkerRunPayload } from '../simulator-types';
import type {
  CompositionalCaseConfig,
  CompositionalCheckpoint,
} from '../compositional/types';
import { isCompositionalCreate } from '../compositional/createPayload';
import {
  CompositionalSession,
  describeCompositionalStop,
  type CompositionalEngine,
  type CompositionalEngineFactory,
} from './compositionalSession';
import { evaluateTerminationPolicy } from './terminationPolicy';
import { configureReservoirSimulator } from './configureSimulator';

let wasmReady = false;
let simulator: ReservoirSimulator | null = null;
let isRunning = false;
let stopRequested = false;
let lastRateHistoryLen = 0;
let wasmInitPromise: Promise<void> | null = null;
let activeCreatePayload: SimulatorCreatePayload | null = null;

/**
 * The compositional run, when there is one.
 *
 * Deliberately a separate handle from `simulator`, not a union: the black-oil `run` loop owns rate
 * history, the termination policy and `loadState`, none of which mean the same thing for a
 * compositional case, and C13's exit criterion is that black-oil scenarios are unchanged. The two
 * never both exist — `create` disposes whichever was there.
 */
let compositional: CompositionalSession | null = null;

/** The generated bindings type every payload as `any`; this is where those types come back. */
const compositionalFactory: CompositionalEngineFactory = {
  create: (config: CompositionalCaseConfig) =>
    new CompositionalSimulator(config) as unknown as CompositionalEngine,
  restore: (checkpoint: CompositionalCheckpoint) =>
    CompositionalSimulator.restore(checkpoint) as unknown as CompositionalEngine,
};

function disposeRuns(): void {
  compositional?.dispose();
  compositional = null;
  simulator = null;
}

async function ensureWasmReady(): Promise<void> {
  if (wasmReady) {
    return;
  }

  if (!wasmInitPromise) {
    wasmInitPromise = (async () => {
      await initWasm();
      set_panic_hook();
      wasmReady = true;
    })().catch((error) => {
      wasmInitPromise = null;
      throw error;
    });
  }

  await wasmInitPromise;
}

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
  const grid = simulator.getGridState();
  const wells = simulator.getWellState();
  const time = simulator.get_time();
  const rateHistoryDelta = simulator.getRateHistorySince(lastRateHistoryLen) as Array<Record<string, unknown>>;
  lastRateHistoryLen += rateHistoryDelta.length;
  const solverWarning = simulator.getLastSolverWarning();
  const extractMs = performance.now() - extractStart;

  return {
    grid,
    wells,
    time,
    rateHistoryDelta,
    solverWarning,
    recordHistory,
    stepIndex,
    profile: {
      ...profile,
      extractMs,
    },
  };
}

function peekLatestRatePoint(): Record<string, unknown> | null {
  if (!simulator) {
    throw new Error('Simulator not initialized');
  }

  // Only the latest point is needed here, so avoid marshalling the whole
  // undelivered rate-history tail across the wasm boundary every step.
  return (simulator.getLatestRatePoint() as Record<string, unknown> | null) ?? null;
}

function configureSimulator(payload: SimulatorCreatePayload) {
  simulator = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));
  activeCreatePayload = payload;
  lastRateHistoryLen = 0;
  configureReservoirSimulator(simulator, payload);
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
      await ensureWasmReady();
      post('ready');
      return;
    }

    if (type === 'create') {
      await ensureWasmReady();

      // The fluid-model discriminator. An absent one is black-oil, which is what every payload
      // serialized before the compositional model existed looks like.
      if (isCompositionalCreate(payload)) {
        disposeRuns();
        try {
          compositional = CompositionalSession.create(compositionalFactory, payload.compositional);
        } catch (error) {
          compositional = null;
          throw error;
        }
        post('compositionalState', {
          data: compositional.snapshot(),
          config: compositional.config,
        });
        return;
      }

      disposeRuns();
      try {
        configureSimulator(payload);
      } catch (error) {
        simulator = null;
        throw error;
      }
      post('state', getStatePayload(false, -1, { batchMs: 0, avgStepMs: 0, snapshotsSent: 0 }));
      return;
    }

    if (type === 'compositionalRun') {
      if (!compositional) {
        throw new Error('No compositional case has been created');
      }
      const { steps = 1, dtDays = 1, snapshotEvery } = payload ?? {};
      const batchStart = performance.now();
      let snapshotsSent = 0;
      const stop = compositional.run({ steps, dtDays, snapshotEvery }, (data, stepIndex) => {
        snapshotsSent += 1;
        post('compositionalState', { data, stepIndex });
      });
      post('compositionalStopped', {
        reason: stop.reason,
        completedSteps: stop.completedSteps,
        message: describeCompositionalStop(stop),
        failure: stop.reason === 'failed' ? stop.failure : undefined,
        profile: buildRunProfile(batchStart, 0, stop.completedSteps, snapshotsSent),
      });
      return;
    }

    if (type === 'compositionalStop') {
      compositional?.requestStop();
      return;
    }

    if (type === 'compositionalCheckpoint') {
      if (!compositional) {
        throw new Error('No compositional case has been created');
      }
      post('compositionalCheckpoint', { checkpoint: compositional.checkpoint() });
      return;
    }

    if (type === 'compositionalRestore') {
      await ensureWasmReady();
      disposeRuns();
      try {
        compositional = CompositionalSession.restore(compositionalFactory, payload?.checkpoint);
      } catch (error) {
        compositional = null;
        throw error;
      }
      post('compositionalState', {
        data: compositional.snapshot(),
        config: compositional.config,
      });
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
            lastRateHistoryLen = rateHistoryPayload.length;
          }
        }
      }

      const steps = Math.max(0, Math.floor(Number(runPayload?.steps ?? 0)));
      const deltaTDays = Number(runPayload?.deltaTDays ?? 0);
      const historyInterval = Math.max(1, Number(runPayload?.historyInterval ?? 1));
      const chunkYieldInterval = Math.max(1, Number(runPayload?.chunkYieldInterval ?? 5));
      const terminationPolicy = activeCreatePayload?.terminationPolicy;

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

        const terminationMatch = evaluateTerminationPolicy(
          terminationPolicy,
          peekLatestRatePoint() as any,
          activeCreatePayload ?? { injectedFluid: 'water' } as SimulatorCreatePayload,
        );

        if (stopRequested) {
          postStopped(batchStart, stepMsTotal, i + 1, snapshotsSent);
          isRunning = false;
          stopRequested = false;
          return;
        }

        const shouldRecord = Boolean(terminationMatch) || i % historyInterval === 0 || i === steps - 1;
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

        if (terminationMatch) {
          post('batchComplete', {
            profile: buildRunProfile(batchStart, stepMsTotal, i + 1, snapshotsSent),
            completedSteps: i + 1,
            terminationSummary: `Simulation completed early after ${i + 1} step(s): ${terminationMatch.summary}`,
          });
          isRunning = false;
          stopRequested = false;
          return;
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
      activeCreatePayload = null;
      lastRateHistoryLen = 0;
      close();
    }
  } catch (error) {
    isRunning = false;
    stopRequested = false;
    post('error', { message: formatWorkerError(error) });
  }
};
