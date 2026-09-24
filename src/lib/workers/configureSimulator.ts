import type { ReservoirSimulator } from '../ressim/pkg/simulator.js';
import type { SimulatorCreatePayload, SimulatorWellDefinition, SimulatorWellSchedule } from '../simulator-types';

function addWellCompletion(
  simulator: ReservoirSimulator,
  well: SimulatorWellDefinition,
  completion: { i: number; j: number; k: number },
): void {
  const addWellWithId = (simulator as any).addWellWithId;
  if (typeof addWellWithId === 'function') {
    addWellWithId.call(
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
    return;
  }

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

function applyWellSchedule(simulator: ReservoirSimulator, well: SimulatorWellDefinition): void {
  const setWellSchedule = (simulator as any).setWellSchedule;
  if (typeof setWellSchedule !== 'function') {
    return;
  }

  const schedule = well.schedule;
  setWellSchedule.call(
    simulator,
    String(well.id),
    String(schedule?.controlMode ?? 'pressure'),
    Number(schedule?.targetRate ?? Number.NaN),
    Number(schedule?.targetSurfaceRate ?? Number.NaN),
    Number(schedule?.bhpLimit ?? Number.NaN),
    schedule?.enabled !== false,
  );
}

function applyWellDatum(simulator: ReservoirSimulator, well: SimulatorWellDefinition): void {
  const setWellDatum = (simulator as any).setWellDatum;
  if (typeof setWellDatum !== 'function') {
    return;
  }
  if (well.datumDepth === undefined && well.wellboreDensity === undefined) {
    return;
  }

  setWellDatum.call(
    simulator,
    String(well.id),
    Number(well.datumDepth ?? Number.NaN),
    Number(well.wellboreDensity ?? Number.NaN),
  );
}

/**
 * Configure a freshly constructed simulator from a create payload.
 *
 * This is the worker's own setup, not a copy of it: `sim.worker.ts` calls it, and so do the
 * tests that check scenario wiring (per-layer geometry, completions, well schedules). They used to
 * carry a hand-written replica that had already drifted from the worker (#12).
 */
export function configureReservoirSimulator(simulator: ReservoirSimulator, payload: SimulatorCreatePayload): void {

  const setFimEnabled = /** @type {any} */ (simulator).setFimEnabled;
  if (typeof setFimEnabled === 'function') {
    setFimEnabled.call(simulator, payload.fimEnabled === true);
  }

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
  // Override initial Rs after PVT table sets the default saturated values
  if (payload.initialRs != null) {
    const setInitialRs = (simulator as any).setInitialRs;
    if (typeof setInitialRs === 'function') {
      setInitialRs.call(simulator, Number(payload.initialRs));
    }
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
    call3p('setGasRedissolutionEnabled', payload.gasRedissolutionEnabled !== false);
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
  } else if (payload.permMode === 'field') {
    simulator.setPermeabilityField(
      new Float64Array(payload.fieldPermX ?? []),
      new Float64Array(payload.fieldPermY ?? []),
      new Float64Array(payload.fieldPermZ ?? []),
    );
  } else if (payload.permMode === 'perLayer' || payload.permMode === 'uniform') {
    simulator.setPermeabilityPerLayer(new Float64Array(payload.permsX), new Float64Array(payload.permsY), new Float64Array(payload.permsZ));
  }

  if (payload.sweepConfig) {
    const setSweepConfig = (simulator as any).setSweepConfig;
    if (typeof setSweepConfig === 'function') {
      setSweepConfig.call(simulator, payload.sweepConfig);
    }
  }

  const producerI = Number(payload.producerI ?? (payload.nx - 1));
  const producerJ = Number(payload.producerJ ?? 0);
  const injectorI = Number(payload.injectorI ?? 0);
  const injectorJ = Number(payload.injectorJ ?? 0);
  const producerBhp = Number(payload.producerBhp ?? 100);
  const injectorBhp = Number(payload.injectorBhp ?? 500);

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
          ).map((k) => ({ i: producerI, j: producerJ, k })),
          schedule: {
            controlMode: payload.producerControlMode === 'rate' ? 'rate' : 'pressure',
            targetRate: payload.targetProducerRate,
            targetSurfaceRate: payload.targetProducerSurfaceRate,
            bhpLimit: payload.bhpMin,
            enabled: true,
          } satisfies SimulatorWellSchedule,
        },
        ...(Boolean(payload.injectorEnabled ?? true)
          ? [{
              id: 'injector-main',
              injector: true,
              bhp: injectorBhp,
              wellRadius: payload.well_radius,
              skin: payload.well_skin,
              completions: (Array.isArray(payload.injectorKLayers)
                ? payload.injectorKLayers
                : Array.from({ length: payload.nz }, (_, i) => i)
              ).map((k) => ({ i: injectorI, j: injectorJ, k })),
              schedule: {
                controlMode: payload.injectorControlMode === 'rate' ? 'rate' : 'pressure',
                targetRate: payload.targetInjectorRate,
                targetSurfaceRate: payload.targetInjectorSurfaceRate,
                bhpLimit: payload.bhpMax,
                enabled: payload.injectorEnabled !== false,
              } satisfies SimulatorWellSchedule,
            }]
          : []),
      ];

  try {
    for (const well of explicitWells) {
      if (well.schedule?.enabled === false) {
        continue;
      }
      for (const completion of well.completions) {
        addWellCompletion(simulator, well, completion);
      }
      applyWellSchedule(simulator, well);
      applyWellDatum(simulator, well);
    }
  } catch (err: any) {
    throw new Error(`Failed to configure wells: ${err?.message || err}`);
  }
}
