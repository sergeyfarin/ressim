import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { calculateDepletionAnalyticalProduction } from '../../analytical/depletionAnalytical';
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

function configureAndRun(params: Record<string, unknown>) {
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const nz = Number(params.nz);
    const sim = new ReservoirSimulator(nx, ny, nz, Number(params.reservoirPorosity));
    sim.setCellDimensions(Number(params.cellDx), Number(params.cellDy), Number(params.cellDz));
    sim.setFluidProperties(Number(params.mu_o), Number(params.mu_w));
    sim.setFluidCompressibilities(Number(params.c_o), Number(params.c_w));
    sim.setRockProperties(
        Number(params.rock_compressibility),
        Number(params.depth_reference),
        Number(params.volume_expansion_o),
        Number(params.volume_expansion_w),
    );
    sim.setFluidDensities(Number(params.rho_o), Number(params.rho_w));
    sim.setInitialPressure(Number(params.initialPressure));
    sim.setInitialSaturation(Number(params.initialSaturation));
    sim.setRelPermProps(
        Number(params.s_wc),
        Number(params.s_or),
        Number(params.n_w),
        Number(params.n_o),
        Number(params.k_rw_max),
        Number(params.k_ro_max),
    );
    sim.setCapillaryParams(0, Number(params.capillaryLambda));
    sim.setGravityEnabled(false);
    sim.setPermeabilityPerLayer(
        new Float64Array(params.layerPermsX as number[]),
        new Float64Array(params.layerPermsY as number[]),
        new Float64Array(params.layerPermsZ as number[]),
    );
    sim.setStabilityParams(
        Number(params.max_sat_change_per_step),
        Number(params.max_pressure_change_per_step),
        Number(params.max_well_rate_change_fraction),
    );
    sim.setWellControlModes('pressure', 'pressure');
    sim.setTargetWellRates(0, 0);
    sim.setWellBhpLimits(Number(params.producerBhp), Number(params.injectorBhp));
    sim.setInjectorEnabled(false);

    for (let k = 0; k < nz; k++) {
        sim.addWellWithId(
            Number(params.producerI),
            Number(params.producerJ),
            k,
            Number(params.producerBhp),
            Number(params.well_radius),
            Number(params.well_skin),
            false,
            'producer-main',
        );
    }

    for (let step = 0; step < Number(params.steps); step++) {
        sim.step(Number(params.delta_t_days));
    }

    const rateHistory = sim.getRateHistory() as Array<Record<string, number>>;
    sim.free();
    return rateHistory;
}

function analyticalFor(params: Record<string, unknown>, times: number[]) {
    return calculateDepletionAnalyticalProduction({
        reservoir: {
            length: Number(params.nx) * Number(params.cellDx),
            area: Number(params.ny) * Number(params.cellDy) * Number(params.nz) * Number(params.cellDz),
            porosity: Number(params.reservoirPorosity),
        },
        timeHistory: times,
        minTimeDays: Number(params.analyticalDepletionStartDays),
        initialSaturation: Number(params.initialSaturation),
        nz: Number(params.nz),
        permMode: String(params.permMode),
        uniformPermX: Number(params.uniformPermX),
        uniformPermY: Number(params.uniformPermY),
        layerPermsX: params.layerPermsX as number[],
        layerPermsY: params.layerPermsY as number[],
        cellDx: Number(params.cellDx),
        cellDy: Number(params.cellDy),
        cellDz: Number(params.cellDz),
        wellRadius: Number(params.well_radius),
        wellSkin: Number(params.well_skin),
        muO: Number(params.mu_o),
        sWc: Number(params.s_wc),
        sOr: Number(params.s_or),
        nO: Number(params.n_o),
        c_o: Number(params.c_o),
        c_w: Number(params.c_w),
        cRock: Number(params.rock_compressibility),
        initialPressure: Number(params.initialPressure),
        producerBhp: Number(params.producerBhp),
        depletionRateScale: 1,
        layeredComposite: true,
        nx: Number(params.nx),
        ny: Number(params.ny),
        producerI: Number(params.producerI),
        producerJ: Number(params.producerJ),
    }).production;
}

describe('layered composite depletion scenario', () => {
    it('keeps the numerical and exact layer-superposition decline in the same regime', async () => {
        await ensureWasmReady();

        for (const variantKey of ['contrast_low', 'contrast_base', 'contrast_high']) {
            const params = getScenarioWithVariantParams('dep_arps', 'layer_contrast', variantKey);
            const numerical = configureAndRun(params);
            const reference = analyticalFor(params, numerical.map((point) => Number(point.time)));
            const numericalAfterTransient = numerical.filter(
                (point) => Number(point.time) >= Number(params.analyticalDepletionStartDays),
            );

            expect(reference).toHaveLength(numericalAfterTransient.length);
            expect(numericalAfterTransient.length).toBeGreaterThan(10);

            const finalNumerical = Number(numericalAfterTransient.at(-1)?.total_production_oil);
            const finalReference = Number(reference.at(-1)?.oilRate);
            const relativeErrors = numericalAfterTransient.map((point, index) => {
                const numericalRate = Number(point.total_production_oil);
                const referenceRate = Number(reference[index]?.oilRate);
                return Math.abs(numericalRate - referenceRate) / Math.max(referenceRate, 1e-12);
            });
            const pressureErrors = numericalAfterTransient.map((point, index) =>
                Math.abs(
                    Number(point.avg_reservoir_pressure) - Number(reference[index]?.avgPressure),
                ),
            );
            expect(finalNumerical).toBeGreaterThan(0);
            expect(finalReference).toBeGreaterThan(0);
            expect(Math.max(...relativeErrors)).toBeLessThan(0.03);
            expect(Math.max(...pressureErrors)).toBeLessThan(30);
            expect(finalNumerical / finalReference).toBeGreaterThan(0.97);
            expect(finalNumerical / finalReference).toBeLessThan(1.03);
        }
    }, 30_000);

    it('shows analytical-model breakdown when vertical crossflow is enabled', async () => {
        await ensureWasmReady();
        const errors: number[] = [];

        for (const variantKey of ['communication_isolated', 'communication_weak', 'communication_strong']) {
            const params = getScenarioWithVariantParams('dep_arps', 'vertical_communication', variantKey);
            const numerical = configureAndRun(params);
            const reference = analyticalFor(params, numerical.map((point) => Number(point.time)));
            const relativeErrors = numerical.map((point, index) => {
                const numericalRate = Number(point.total_production_oil);
                const referenceRate = Number(reference[index]?.oilRate);
                return Math.abs(numericalRate - referenceRate) / Math.max(referenceRate, 1e-12);
            });
            errors.push(Math.max(...relativeErrors));
        }

        expect(errors[0]).toBeLessThan(0.03);
        expect(errors[2]).toBeGreaterThan(errors[0] * 2);
        expect(errors[2]).toBeGreaterThan(0.05);
    }, 30_000);
});
