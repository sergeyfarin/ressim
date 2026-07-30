import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { depletionDef } from '../analyticalAdapters';
import { getScenarioWithVariantParams } from '../scenarios';

let ready: Promise<unknown> | null = null;
async function ensureReady() {
    ready ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((bytes) => initWasm({ module_or_path: bytes }));
    await ready;
}

function run(params: Record<string, any>) {
    const sim = new ReservoirSimulator(params.nx, params.ny, params.nz, params.reservoirPorosity);
    sim.setCellDimensions(params.cellDx, params.cellDy, params.cellDz);
    sim.setFluidProperties(params.mu_o, params.mu_w);
    sim.setFluidCompressibilities(params.c_o, params.c_w);
    sim.setRockProperties(params.rock_compressibility, params.depth_reference, 1, 1);
    sim.setFluidDensities(params.rho_o, params.rho_w);
    sim.setInitialPressure(params.initialPressure);
    sim.setInitialSaturation(params.initialSaturation);
    sim.setRelPermProps(params.s_wc, params.s_or, params.n_w, params.n_o, params.k_rw_max, params.k_ro_max);
    sim.setPermeabilityPerLayer(new Float64Array([params.uniformPermX]), new Float64Array([params.uniformPermY]), new Float64Array([params.uniformPermZ]));
    sim.setWellControlModes('pressure', 'pressure');
    sim.setWellBhpLimits(params.producerBhp, params.injectorBhp);
    sim.setInjectorEnabled(false);
    sim.add_well(params.producerI, params.producerJ, 0, params.producerBhp, params.well_radius, params.well_skin, false);
    for (let i = 0; i < params.steps; i++) sim.step(params.delta_t_days);
    const history = sim.getRateHistory() as Array<Record<string, number>>;
    sim.free();
    return history;
}

function meanRateError(
    numerical: Array<Record<string, number>>,
    referenceParams: Record<string, any>,
): number {
    const compared = numerical.filter(
        (point) => point.time >= Number(referenceParams.analyticalDepletionStartDays),
    );
    const reference = depletionDef.fn(depletionDef.inputsFromParams({
        analyticalDepletionRateScale: 1,
        ...referenceParams,
    }, compared as any)).production;
    const errors = compared.flatMap((point, index) => {
        const analytical = reference[index]?.oilRate;
        if (
            !Number.isFinite(analytical) || analytical <= reference[0].oilRate * 0.01
        ) return [];
        return [Math.abs(point.total_production_oil - analytical) / analytical];
    });
    return errors.reduce((sum, error) => sum + error, 0) / errors.length;
}

describe('finite-reservoir Fetkovich transition', () => {
    it('tracks the finite-slab reference through transient and boundary-dominated flow', async () => {
        await ensureReady();
        for (const key of ['perm_tight', 'perm_base', 'perm_good']) {
            const params: Record<string, any> = {
                analyticalDepletionRateScale: 1,
                ...getScenarioWithVariantParams('dep_decline', 'permeability', key),
            };
            const numerical = run(params);
            const reference = depletionDef.fn(depletionDef.inputsFromParams(params, numerical as any)).production;
            const compared = numerical.filter((point) => point.time >= params.analyticalDepletionStartDays);
            const samples = compared.map((point, i) => {
                const ref = reference[i].oilRate;
                return { time: point.time, error: Math.abs(point.total_production_oil - ref) / ref, meaningful: point.time >= 3 * params.delta_t_days && ref > reference[0].oilRate * 0.01 };
            });
            const errors = samples.filter((value) => value.meaningful).map((value) => value.error);
            expect(errors.length).toBeGreaterThan(10);
            expect(Math.max(...errors), key).toBeLessThan(0.25);
        }
    }, 30_000);

    it('exposes numerical-resolution error against one fixed physical reference', async () => {
        await ensureReady();
        const base = getScenarioWithVariantParams('dep_decline', 'grid_refinement', 'grid_base');

        const gridErrors = ['grid_coarse', 'grid_base', 'grid_fine'].map((key) => {
            const params = getScenarioWithVariantParams('dep_decline', 'grid_refinement', key);
            return meanRateError(run(params), { ...base, delta_t_days: params.delta_t_days, steps: params.steps });
        });
        const timestepErrors = ['timestep_small', 'timestep_base', 'timestep_large'].map((key) => {
            const params = getScenarioWithVariantParams('dep_decline', 'timestep', key);
            return meanRateError(run(params), params);
        });

        expect(gridErrors[2], JSON.stringify(gridErrors)).toBeLessThan(gridErrors[0]);
        expect(timestepErrors[0], JSON.stringify(timestepErrors)).toBeLessThan(timestepErrors[2]);
    }, 60_000);
});
