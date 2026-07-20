import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../scenarios';
import { depletionDef } from '../analyticalAdapters';
import type { RateHistoryPoint } from '../../simulator-types';

const TIME_HISTORY: RateHistoryPoint[] = [1, 5, 10, 20, 40, 60].map((time) => ({ time }));

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    if (!wasmReady) {
        wasmReady = readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url)).then(
            (wasmBytes) => initWasm({ module_or_path: wasmBytes }),
        );
    }
    await wasmReady;
}

function runVariantRecoveryFactor(variantKey: string): number {
    const params = getScenarioWithVariantParams('dep_nct', 'nct_ambiguity', variantKey);
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const nz = Number(params.nz);

    const sim = new ReservoirSimulator(nx, ny, nz, Number(params.reservoirPorosity));
    sim.setFimEnabled(Boolean(params.fimEnabled));
    sim.setCellDimensions(Number(params.cellDx), Number(params.cellDy), Number(params.cellDz));
    sim.setRelPermProps(
        Number(params.s_wc), Number(params.s_or),
        Number(params.n_w), Number(params.n_o),
        Number(params.k_rw_max), Number(params.k_ro_max),
    );
    sim.setInitialPressure(Number(params.initialPressure));
    sim.setInitialSaturation(Number(params.initialSaturation));
    sim.setFluidProperties(Number(params.mu_o), Number(params.mu_w));
    sim.setFluidCompressibilities(Number(params.c_o), Number(params.c_w));
    sim.setRockProperties(
        Number(params.rock_compressibility), Number(params.depth_reference),
        Number(params.volume_expansion_o), Number(params.volume_expansion_w),
    );
    sim.setFluidDensities(Number(params.rho_o), Number(params.rho_w));
    sim.setCapillaryParams(
        Boolean(params.capillaryEnabled) ? Number(params.capillaryPEntry) : 0,
        Number(params.capillaryLambda),
    );
    sim.setGravityEnabled(Boolean(params.gravityEnabled));
    sim.setPermeabilityPerLayer(
        new Float64Array(Array.from({ length: nz }, () => Number(params.uniformPermX))),
        new Float64Array(Array.from({ length: nz }, () => Number(params.uniformPermY))),
        new Float64Array(Array.from({ length: nz }, () => Number(params.uniformPermZ))),
    );
    sim.setStabilityParams(
        Number(params.max_sat_change_per_step),
        Number(params.max_pressure_change_per_step),
        Number(params.max_well_rate_change_fraction),
    );
    sim.setWellControlModes(String(params.injectorControlMode), String(params.producerControlMode));
    sim.setTargetWellRates(0, 0);
    const producerBhp = Number(params.producerBhp);
    const injectorBhp = Number(params.injectorBhp);
    sim.setWellBhpLimits(Math.min(producerBhp, injectorBhp), Math.max(producerBhp, injectorBhp));
    sim.add_well(
        Number(params.producerI), 0, 0, producerBhp,
        Number(params.well_radius), Number(params.well_skin), false,
    );

    const dt = Number(params.delta_t_days);
    const steps = Number(params.steps);
    for (let i = 0; i < steps; i++) {
        sim.step(dt);
    }

    // total_production_oil is an instantaneous rate (m3/day), not a
    // cumulative — integrate rate x dt, matching buildRateChartData.ts.
    const history = sim.getRateHistorySince(0) as Array<{ time: number; total_production_oil?: number }>;
    let cumOil = 0;
    for (let i = 0; i < history.length; i++) {
        const stepDt = i > 0 ? history[i].time - history[i - 1].time : history[i].time;
        cumOil += Math.abs(history[i].total_production_oil ?? 0) * stepDt;
    }

    const bulkVolume = nx * Number(params.cellDx) * ny * Number(params.cellDy) * nz * Number(params.cellDz);
    const ooip = bulkVolume * Number(params.reservoirPorosity) * (1 - Number(params.s_wc) - Number(params.s_or));
    return cumOil / ooip;
}

function analyticalOilRates(variantKey: string): number[] {
    // analyticalDepletionRateScale is a runtime UI default (parameterStore.svelte.ts
    // seeds it to 1.0), not part of the scenario's own params — apply the same
    // default the real pipeline uses (see benchmarkRunModel.ts / analyticalParamAdapters.ts)
    // rather than leaving it undefined, which the analytical model treats as "no rate".
    const params = {
        analyticalDepletionRateScale: 1,
        ...getScenarioWithVariantParams('dep_nct', 'nct_ambiguity', variantKey),
    };
    const inputs = depletionDef.inputsFromParams(params, TIME_HISTORY);
    const result = depletionDef.fn(inputs);
    return result.production.map((p) => p.oilRate);
}

function ooipProxy(variantKey: string): number {
    const params = getScenarioWithVariantParams('dep_nct', 'nct_ambiguity', variantKey);
    const length = (params.nx as number) * (params.cellDx as number);
    const area = (params.ny as number) * (params.cellDy as number) * (params.nz as number) * (params.cellDz as number);
    const poreVolume = length * area * (params.reservoirPorosity as number);
    return poreVolume * (1 - (params.s_wc as number));
}

describe('dep_nct — N·c_t ambiguity scenario', () => {
    it('produces the same analytical oil-rate history across all three OOIP/compressibility variants', () => {
        // This is the core claim of the scenario: porosity and c_o are scaled
        // inversely so tau = poreVolume * totalCompressibility / PI is held
        // fixed, which per depletionAnalytical.ts fully determines the rate
        // curve. If this test fails, the compensating c_o values need
        // re-deriving (see the module comment in dep_nct.ts).
        const base = analyticalOilRates('nct_base');
        const small = analyticalOilRates('nct_small_reservoir');
        const large = analyticalOilRates('nct_large_reservoir');

        expect(small.length).toBe(base.length);
        expect(large.length).toBe(base.length);
        for (let i = 0; i < base.length; i++) {
            expect(small[i]).toBeCloseTo(base[i], 6);
            expect(large[i]).toBeCloseTo(base[i], 6);
        }
    });

    it('varies OOIP by roughly 4x between the small- and large-reservoir variants despite the matched history', () => {
        const ooipSmall = ooipProxy('nct_small_reservoir');
        const ooipBase = ooipProxy('nct_base');
        const ooipLarge = ooipProxy('nct_large_reservoir');

        // porosity 0.10 / 0.20 / 0.40 -> OOIP ratios are exactly 0.5x / 1x / 2x
        expect(ooipSmall).toBeCloseTo(ooipBase * 0.5, 6);
        expect(ooipLarge).toBeCloseTo(ooipBase * 2.0, 6);

        // Same cumulative oil (from the matched history) over a 4x OOIP spread
        // means recovery factor spans roughly 4x between the two extremes —
        // the actual number this scenario exists to demonstrate.
        const rfRatio = ooipLarge / ooipSmall;
        expect(rfRatio).toBeCloseTo(4.0, 6);
    });

    it('locks the absolute recovery-factor values shown in the scenario/variant description text', async () => {
        // The scenario description asserts specific RF numbers (small ~3.7%,
        // large ~0.9%); these were previously invented from the 4x ratio
        // without being run through the real simulator and were off by
        // roughly 9x (verified 2026-07-18: real headless wasm recovery
        // factors are ~3.74%/1.87%/0.93%, not the originally-claimed
        // ~33%/~8%). This test locks the real values so the description text
        // can't silently drift out of sync with the engine again.
        await ensureWasmReady();

        const rfSmall = runVariantRecoveryFactor('nct_small_reservoir');
        const rfBase = runVariantRecoveryFactor('nct_base');
        const rfLarge = runVariantRecoveryFactor('nct_large_reservoir');

        expect(rfSmall).toBeCloseTo(0.0374, 3);
        expect(rfBase).toBeCloseTo(0.0187, 3);
        expect(rfLarge).toBeCloseTo(0.0093, 3);
        expect(rfSmall / rfLarge).toBeCloseTo(4.0, 1);
    }, 30000);
});
