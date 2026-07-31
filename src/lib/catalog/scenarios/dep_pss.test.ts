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

type Sample = { time: number; bhp: number; avgPressure: number; oilRate: number };

function run(params: Record<string, any>): Sample[] {
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
    // bhp_min is the producer floor, matching how buildCreatePayload maps
    // producerBhp in the app. Passing 0 here would let the well produce past
    // the end of its constant-rate period and hide the very transition the
    // scenario is built to show.
    sim.setWellBhpLimits(params.producerBhp, params.injectorBhp);
    sim.add_well(params.producerI, params.producerJ, 0, params.producerBhp, params.well_radius, params.well_skin, false);
    const values: Sample[] = [];
    for (let i = 0; i < params.steps; i++) {
        sim.step(params.delta_t_days);
        const producer = (sim.getWellState() as Array<Record<string, any>>).find((well) => !well.injector);
        const pressures = sim.getPressures();
        const latest = sim.getLatestRatePoint() as Record<string, number> | null;
        values.push({
            time: (i + 1) * params.delta_t_days,
            bhp: Number(producer?.flowing_bhp),
            avgPressure: pressures.reduce((sum: number, pressure: number) => sum + pressure, 0) / pressures.length,
            oilRate: Math.abs(Number(latest?.total_production_oil ?? 0)),
        });
    }
    sim.free();
    return values;
}

function drainageAreaOf(params: Record<string, any>): number {
    return Number(params.nx) * Number(params.cellDx) * Number(params.ny) * Number(params.cellDy);
}

function tabulatedShapeFactor(params: Record<string, any>): number | null {
    return computeShapeFactor({
        nxCells: Number(params.nx),
        nyCells: Number(params.ny),
        aspectRatio: (Number(params.nx) * Number(params.cellDx)) / (Number(params.ny) * Number(params.cellDy)),
        nx: Number(params.nx),
        ny: Number(params.ny),
        producerI: Number(params.producerI),
        producerJ: Number(params.producerJ),
    }).shapeFactor;
}

function inferShapeFactor(params: Record<string, any>, sample: Sample): number | null {
    return dietzShapeFactorFromProductivityIndex({
        productivityIndex: sample.oilRate / (sample.avgPressure - sample.bhp),
        permeabilityMd: Math.sqrt(Number(params.uniformPermX) * Number(params.uniformPermY)),
        thicknessM: Number(params.nz) * Number(params.cellDz),
        mobilityPerCp: 1 / Number(params.mu_o),
        drainageAreaM2: drainageAreaOf(params),
        wellRadiusM: Number(params.well_radius),
        skin: Number(params.well_skin),
    });
}

/**
 * Samples inside the pseudo-steady, still-rate-controlled window — i.e. those
 * where the analytical reference is defined. The reference is null before
 * `analyticalPssStartDays` and again once the producer reaches its BHP floor.
 */
function pssWindow(params: Record<string, any>, samples: Sample[]): Sample[] {
    const reference = computeWellTestOnTimeAxis(params, samples.map((point) => point.time));
    if (!reference) return [];
    return samples.filter((_, index) => reference.pssProductivity[index] != null);
}

/**
 * Flowing BHP must track the analytical curve wherever that curve is defined.
 * Asserted alongside the shape-factor checks rather than in a sweep of its
 * own: each simulation run here is a few hundred timesteps, so re-running the
 * same variants for a second assertion doubles the file's cost for nothing.
 */
function expectFlowingBhpMatchesReference(params: Record<string, any>, samples: Sample[], key: string) {
    const reference = computeWellTestOnTimeAxis(params, samples.map((point) => point.time));
    expect(reference, key).not.toBeNull();
    const errors = samples.flatMap((point, index) => {
        const expected = reference?.flowingBhp[index];
        return expected == null ? [] : [Math.abs(point.bhp - expected)];
    });
    expect(errors.length, key).toBeGreaterThan(20);
    expect(Math.max(...errors), key).toBeLessThan(5);
}

describe('Dietz PSS productivity scenario', () => {
    it('recovers the tabulated shape factor of every geometry and well position', async () => {
        await ensureReady();
        const cases: Array<[string, string]> = [
            ['drainage_shape', 'geom_square'],
            ['drainage_shape', 'geom_2to1'],
            ['drainage_shape', 'geom_4to1'],
            ['drainage_shape', 'geom_5to1'],
            ['drainage_shape', 'geom_4to1_offset'],
            ['well_position', 'pos_quarter'],
            ['well_position', 'pos_quadrant'],
        ];
        for (const [dimension, key] of cases) {
            const params = getScenarioWithVariantParams('dep_pss', dimension, key);
            const expected = tabulatedShapeFactor(params);
            expect(expected, key).not.toBeNull();

            const samples = run(params);
            expectFlowingBhpMatchesReference(params, samples, key);

            const window = pssWindow(params, samples);
            expect(window.length, key).toBeGreaterThan(20);

            const inferred = window.slice(-20)
                .map((sample) => inferShapeFactor(params, sample))
                .filter((value): value is number => value !== null);
            const mean = inferred.reduce((sum, value) => sum + value, 0) / inferred.length;

            // Measured error is ~1% for every geometry. 3% leaves headroom for
            // engine changes while still failing loudly if a geometry resolves
            // to the wrong table entry — the entries are 6.8x apart.
            expect(Math.abs(mean - Number(expected)) / Number(expected), key).toBeLessThan(0.03);

            const analyticalPi = dietzProductivityIndex({
                permeabilityMd: Math.sqrt(Number(params.uniformPermX) * Number(params.uniformPermY)),
                thicknessM: Number(params.nz) * Number(params.cellDz),
                mobilityPerCp: 1 / Number(params.mu_o),
                drainageAreaM2: drainageAreaOf(params),
                shapeFactor: Number(expected),
                wellRadiusM: Number(params.well_radius),
                skin: Number(params.well_skin),
            });
            const final = window.at(-1)!;
            const numericalPi = final.oilRate / (final.avgPressure - final.bhp);
            expect(Math.abs(numericalPi - analyticalPi) / analyticalPi, key).toBeLessThan(0.01);
        }
    }, 300_000);

    it('separates geometry from completion: skin moves productivity but not inferred C_A', async () => {
        await ensureReady();
        const productivity: number[] = [];
        const shapeFactors: number[] = [];
        for (const key of ['skin_stimulated', 'skin_clean', 'skin_damaged']) {
            const params = getScenarioWithVariantParams('dep_pss', 'skin', key);
            const samples = run(params);
            expectFlowingBhpMatchesReference(params, samples, key);
            const final = pssWindow(params, samples).at(-1)!;
            productivity.push(final.oilRate / (final.avgPressure - final.bhp));
            shapeFactors.push(Number(inferShapeFactor(params, final)));
        }
        // Stimulated > clean > damaged in productivity …
        expect(productivity[0]).toBeGreaterThan(productivity[1] * 1.1);
        expect(productivity[1]).toBeGreaterThan(productivity[2] * 1.1);
        // … while the shape factor, which is a property of the geometry alone,
        // stays put. This is the claim the scenario's skin dimension makes.
        const spread = (Math.max(...shapeFactors) - Math.min(...shapeFactors)) / Math.min(...shapeFactors);
        expect(spread).toBeLessThan(0.05);
    }, 180_000);

    it('stops the analytical reference at the end of the constant-rate period', async () => {
        await ensureReady();
        const params = getScenarioWithVariantParams('dep_pss', 'drainage_shape', 'geom_square');
        const numerical = run(params);
        const reference = computeWellTestOnTimeAxis(params, numerical.map((point) => point.time))!;

        const lastReferenced = reference.flowingBhp.reduce(
            (last: number, value, index) => (value == null ? last : index), -1,
        );
        expect(lastReferenced).toBeGreaterThan(0);
        expect(lastReferenced).toBeLessThan(numerical.length - 1);

        // The reference must end because the well leaves rate control, not
        // because the run does: the producer is still on target at the last
        // referenced point and off target by the end of the run.
        const target = Number(params.targetProducerRate);
        expect(numerical[lastReferenced].oilRate).toBeGreaterThan(0.95 * target);
        expect(numerical.at(-1)!.oilRate).toBeLessThan(0.95 * target);
        expect(numerical.at(-1)!.bhp).toBeLessThan(Number(params.producerBhp) + 1);
    }, 120_000);
});
