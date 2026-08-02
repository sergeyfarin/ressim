import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../scenarios';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

type SolverRun = { cumulativeOil: number; avgPressure: number; warning: string };

/**
 * Runs one numerical-convergence solver variant end to end.
 */
function runVariant(variantKey: string): SolverRun {
    const params = getScenarioWithVariantParams('wf_numerics', 'solver_formulation', variantKey);
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const nz = Number(params.nz);
    const sim = new ReservoirSimulator(nx, ny, nz, Number(params.reservoirPorosity));

    sim.setFimEnabled(Boolean(params.fimEnabled));
    sim.setCellDimensions(Number(params.cellDx), Number(params.cellDy), Number(params.cellDz));
    sim.setRelPermProps(
        Number(params.s_wc), Number(params.s_or), Number(params.n_w), Number(params.n_o),
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
    sim.setCapillaryParams(0, Number(params.capillaryLambda));
    sim.setGravityEnabled(false);
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
    sim.setTargetWellRates(Number(params.targetInjectorRate), Number(params.targetProducerRate));
    sim.setWellBhpLimits(Number(params.producerBhp), Number(params.injectorBhp));
    sim.add_well(
        Number(params.injectorI), Number(params.injectorJ), 0, Number(params.injectorBhp),
        Number(params.well_radius), Number(params.well_skin), true,
    );
    sim.add_well(
        Number(params.producerI), Number(params.producerJ), 0, Number(params.producerBhp),
        Number(params.well_radius), Number(params.well_skin), false,
    );

    const dt = Number(params.delta_t_days);
    let cumulativeOil = 0;
    for (let step = 0; step < Number(params.steps); step += 1) {
        sim.step(dt);
        const point = sim.getRateHistory().at(-1) as Record<string, number> | undefined;
        cumulativeOil += Math.abs(Number(point?.total_production_oil ?? 0)) * dt;
    }
    const final = sim.getRateHistory().at(-1) as Record<string, number> | undefined;
    const result = {
        cumulativeOil,
        avgPressure: Number(final?.avg_reservoir_pressure),
        warning: sim.getLastSolverWarning(),
    };
    sim.free();
    return result;
}

describe('wf_numerics solver_formulation sensitivity', () => {
    it('varies only the formulation', () => {
        const impes = getScenarioWithVariantParams('wf_numerics', 'solver_formulation', 'solver_impes_base');
        const fim = getScenarioWithVariantParams('wf_numerics', 'solver_formulation', 'solver_fim_base');
        const differing = Object.keys(fim).filter((name) => fim[name] !== impes[name]);
        expect(differing).toEqual(['fimEnabled']);
        expect(impes.fimEnabled).toBe(false);
        expect(fim.fimEnabled).toBe(true);
    });

    it('agrees closely between formulations at the scenario report step', async () => {
        await ensureWasmReady();

        const impes = runVariant('solver_impes_base');
        const fim = runVariant('solver_fim_base');

        for (const [key, run] of Object.entries({ impes, fim })) {
            expect(run.warning, key).toBe('');
            expect(run.cumulativeOil, key).toBeGreaterThan(0);
        }

        // Both formulations should remain close at the base report step.
        const gap = Math.abs(fim.cumulativeOil - impes.cumulativeOil) / impes.cumulativeOil;
        expect(gap).toBeLessThan(0.03);
    }, 180_000);
});
