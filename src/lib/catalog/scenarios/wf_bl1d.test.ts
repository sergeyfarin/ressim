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
 * Runs one solver_formulation variant end to end. Ported from the deleted
 * `solver_fim_impes` scenario test when that case was folded into this one as
 * a sensitivity dimension (2026-07-31), so the formulation comparison keeps
 * its regression coverage.
 */
function runVariant(variantKey: string): SolverRun {
    const params = getScenarioWithVariantParams('wf_bl1d', 'solver_formulation', variantKey);
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

describe('wf_bl1d solver_formulation sensitivity', () => {
    it('varies only the formulation and the report timestep', () => {
        const base = getScenarioWithVariantParams('wf_bl1d', 'solver_formulation', 'solver_impes_base');
        const variants = ['solver_fim_base', 'solver_impes_coarse', 'solver_fim_coarse'];
        for (const key of variants) {
            const params = getScenarioWithVariantParams('wf_bl1d', 'solver_formulation', key);
            const differing = Object.keys(params).filter((name) => params[name] !== base[name]);
            expect(differing.sort(), key).toEqual(
                key === 'solver_fim_base'
                    ? ['fimEnabled']
                    : key === 'solver_impes_coarse'
                        ? ['delta_t_days', 'steps']
                        : ['delta_t_days', 'fimEnabled', 'steps'],
            );
            // Same 50-day horizon in every variant, so cumulative oil is
            // comparable across all four.
            expect(Number(params.delta_t_days) * Number(params.steps), key).toBeCloseTo(50, 9);
        }
    });

    it('agrees between formulations at the base step and diverges at coarse steps', async () => {
        await ensureWasmReady();

        const impesBase = runVariant('solver_impes_base');
        const fimBase = runVariant('solver_fim_base');
        const impesCoarse = runVariant('solver_impes_coarse');
        const fimCoarse = runVariant('solver_fim_coarse');

        for (const [key, run] of Object.entries({ impesBase, fimBase, impesCoarse, fimCoarse })) {
            expect(run.warning, key).toBe('');
            expect(run.cumulativeOil, key).toBeGreaterThan(0);
        }

        // At the base step the formulation barely matters: measured 0.97%.
        const baseGap = Math.abs(fimBase.cumulativeOil - impesBase.cumulativeOil) / impesBase.cumulativeOil;
        expect(baseGap).toBeLessThan(0.03);

        // Coarsening to 5-day steps costs IMPES far more recovery than FIM —
        // measured 8.8% against 3.1% of each solver's own fine-step run. That
        // asymmetry, not the raw divergence, is what the dimension teaches.
        const impesLoss = 1 - impesCoarse.cumulativeOil / impesBase.cumulativeOil;
        const fimLoss = 1 - fimCoarse.cumulativeOil / fimBase.cumulativeOil;
        expect(impesLoss).toBeGreaterThan(0.05);
        expect(fimLoss).toBeLessThan(0.05);
        expect(impesLoss).toBeGreaterThan(fimLoss * 2);

        // …so the two formulations, indistinguishable at the base step, are
        // clearly apart at the coarse one: measured 7.4%.
        const coarseGap = Math.abs(fimCoarse.cumulativeOil - impesCoarse.cumulativeOil) / impesCoarse.cumulativeOil;
        expect(coarseGap).toBeGreaterThan(0.04);
        expect(coarseGap).toBeGreaterThan(baseGap * 3);
    }, 180_000);
});
