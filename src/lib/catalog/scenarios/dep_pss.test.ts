import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { computeWellTestOnTimeAxis } from '../../charts/analyticalParamAdapters';
import {
    computeShapeFactor,
    dietzProductivityIndex,
    dietzShapeFactorFromProductivityIndex,
} from '../../analytical/depletionAnalytical';
import { getScenarioWithVariantParams } from '../scenarios';

let ready: Promise<unknown> | null = null;
async function ensureReady() {
    ready ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((bytes) => initWasm({ module_or_path: bytes }));
    await ready;
}

function run(params: Record<string, any>, collectDiagnostics = false) {
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
    const values: Array<{ time: number; bhp: number; avgPressure: number; oilRate: number }> = [];
    for (let i = 0; i < params.steps; i++) {
        sim.step(params.delta_t_days);
        const producer = (sim.getWellState() as Array<Record<string, any>>).find((well) => !well.injector);
        const pressures = collectDiagnostics ? sim.getPressures() : null;
        const avgPressure = pressures
            ? pressures.reduce((sum, pressure) => sum + pressure, 0) / pressures.length
            : Number.NaN;
        const latest = collectDiagnostics ? sim.getLatestRatePoint() as Record<string, number> | null : null;
        values.push({
            time: (i + 1) * params.delta_t_days,
            bhp: Number(producer?.flowing_bhp),
            avgPressure,
            oilRate: Math.abs(Number(latest?.total_production_oil ?? 0)),
        });
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

    it('converges toward the centered-square Dietz shape factor with grid refinement', async () => {
        await ensureReady();
        const errors: number[] = [];

        for (const key of ['grid_coarse', 'grid_base', 'grid_fine']) {
            const params = getScenarioWithVariantParams('dep_pss', 'grid_refinement', key);
            const numerical = run(params, true);
            const late = numerical.filter((point) => point.time >= Number(params.analyticalPssStartDays));
            const inferred = late.slice(-20).map((point) => {
                const productivityIndex = point.oilRate / (point.avgPressure - point.bhp);
                return dietzShapeFactorFromProductivityIndex({
                    productivityIndex,
                    permeabilityMd: Math.sqrt(Number(params.uniformPermX) * Number(params.uniformPermY)),
                    thicknessM: Number(params.nz) * Number(params.cellDz),
                    mobilityPerCp: 1 / Number(params.mu_o),
                    drainageAreaM2: Number(params.nx) * Number(params.cellDx) * Number(params.ny) * Number(params.cellDy),
                    wellRadiusM: Number(params.well_radius),
                    skin: Number(params.well_skin),
                });
            }).filter((value): value is number => value !== null);
            const meanShapeFactor = inferred.reduce((sum, value) => sum + value, 0) / inferred.length;
            const expected = computeShapeFactor({
                nxCells: Number(params.nx), nyCells: Number(params.ny), aspectRatio: 1,
                nx: Number(params.nx), ny: Number(params.ny),
                producerI: Number(params.producerI), producerJ: Number(params.producerJ),
            }).shapeFactor;
            expect(expected).not.toBeNull();
            errors.push(Math.abs(meanShapeFactor - Number(expected)) / Number(expected));

            const analyticalPi = dietzProductivityIndex({
                permeabilityMd: Number(params.uniformPermX),
                thicknessM: Number(params.cellDz),
                mobilityPerCp: 1 / Number(params.mu_o),
                drainageAreaM2: 420 * 420,
                shapeFactor: Number(expected),
                wellRadiusM: Number(params.well_radius),
                skin: Number(params.well_skin),
            });
            const final = late.at(-1)!;
            const numericalPi = final.oilRate / (final.avgPressure - final.bhp);
            expect(Math.abs(numericalPi - analyticalPi) / analyticalPi, key).toBeLessThan(0.2);
        }

        expect(errors[2]).toBeLessThan(errors[0]);
        expect(errors[2]).toBeLessThan(0.15);
    }, 60_000);
});
