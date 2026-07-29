import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { computeWellTestOnTimeAxis } from '../../charts/analyticalParamAdapters';
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
    sim.setRockProperties(params.rock_compressibility, 0, 1, 1);
    sim.setFluidDensities(params.rho_o, params.rho_w);
    sim.setInitialPressure(params.initialPressure);
    sim.setInitialSaturation(params.initialSaturation);
    sim.setRelPermProps(params.s_wc, params.s_or, params.n_w, params.n_o, params.k_rw_max, params.k_ro_max);
    sim.setPermeabilityPerLayer(new Float64Array([params.uniformPermX]), new Float64Array([params.uniformPermY]), new Float64Array([params.uniformPermZ]));
    sim.setWellControlModes('pressure', 'rate');
    sim.setTargetWellRates(0, params.targetProducerRate);
    sim.setWellBhpLimits(0, params.injectorBhp);
    sim.add_well(params.producerI, params.producerJ, 0, params.producerBhp, params.well_radius, params.well_skin, false);
    const values: Array<{ time: number; bhp: number }> = [];
    for (let i = 0; i < params.steps; i++) {
        sim.step(params.delta_t_days);
        const producer = (sim.getWellState() as Array<Record<string, any>>).find((well) => !well.injector);
        values.push({ time: (i + 1) * params.delta_t_days, bhp: Number(producer?.flowing_bhp) });
    }
    sim.free();
    return values;
}

describe('Dietz PSS productivity scenario', () => {
    it('matches late-time flowing BHP for rate and skin variants', async () => {
        await ensureReady();
        for (const [dimension, keys] of [
            ['skin', ['skin_stimulated', 'skin_clean', 'skin_damaged']],
            ['production_rate', ['rate_low', 'rate_base', 'rate_high']],
        ] as const) {
            for (const key of keys) {
                const params = getScenarioWithVariantParams('dep_pss', dimension, key);
                const numerical = run(params);
                const reference = computeWellTestOnTimeAxis(params, numerical.map((point) => point.time));
                expect(reference).not.toBeNull();
                const errors = numerical.flatMap((point, i) => {
                    const ref = reference?.flowingBhp[i];
                    return ref == null ? [] : [Math.abs(point.bhp - ref)];
                });
                expect(errors.length).toBeGreaterThan(20);
                expect(Math.max(...errors)).toBeLessThan(5);
            }
        }
    }, 30_000);
});
