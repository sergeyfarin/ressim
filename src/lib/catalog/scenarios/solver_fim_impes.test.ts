import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../scenarios';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    if (!wasmReady) {
        wasmReady = readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url)).then(
            (wasmBytes) => initWasm({ module_or_path: wasmBytes }),
        );
    }
    await wasmReady;
}

function runVariant(variantKey: string) {
    const params = getScenarioWithVariantParams('solver_fim_impes', 'solver_comparison', variantKey);
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
    sim.setWellBhpLimits(Number(params.bhpMin), Number(params.bhpMax));
    sim.add_well(
        Number(params.injectorI), Number(params.injectorJ), 0, Number(params.injectorBhp),
        Number(params.well_radius), Number(params.well_skin), true,
    );
    sim.add_well(
        Number(params.producerI), Number(params.producerJ), 0, Number(params.producerBhp),
        Number(params.well_radius), Number(params.well_skin), false,
    );

    for (let step = 0; step < Number(params.steps); step += 1) {
        sim.step(Number(params.delta_t_days));
    }

    const final = sim.getRateHistory().at(-1);
    return {
        time: sim.get_time(),
        warning: sim.getLastSolverWarning(),
        avgPressure: Number(final?.avg_reservoir_pressure),
        oilRate: Number(final?.total_production_oil),
    };
}

describe('solver_fim_impes scenario', () => {
    it('changes only the solver between its declared variants and makes the formulation visible', async () => {
        await ensureWasmReady();

        const impesParams = getScenarioWithVariantParams('solver_fim_impes', 'solver_comparison', 'solver_impes');
        const fimParams = getScenarioWithVariantParams('solver_fim_impes', 'solver_comparison', 'solver_fim');
        expect({ ...impesParams, fimEnabled: undefined }).toEqual({ ...fimParams, fimEnabled: undefined });

        const impes = runVariant('solver_impes');
        const fim = runVariant('solver_fim');
        expect(impes.warning).toBe('');
        expect(fim.warning).toBe('');
        expect(impes.time).toBeCloseTo(100, 9);
        expect(fim.time).toBeCloseTo(100, 9);
        expect(Math.abs(fim.avgPressure - impes.avgPressure)).toBeGreaterThan(10);
        expect(Math.abs(fim.oilRate - impes.oilRate) / impes.oilRate).toBeGreaterThan(0.2);
    });
});
