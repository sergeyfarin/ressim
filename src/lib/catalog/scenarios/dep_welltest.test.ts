import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../scenarios';
import {
    fitSemilogLine,
    permeabilityFromSemilogSlope,
    radiusOfInvestigation,
    skinFromSemilogIntercept,
} from '../../analytical/wellTest';
import { extractWellTestProps, getWellTestRate } from '../../charts/analyticalParamAdapters';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    if (!wasmReady) {
        wasmReady = readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url)).then(
            (wasmBytes) => initWasm({ module_or_path: wasmBytes }),
        );
    }
    await wasmReady;
}

type Params = Record<string, unknown>;

/**
 * Mirrors the single-producer, rate-controlled subset of `configureSimulator`
 * in `sim.worker.ts`. Note the BHP-limit widening: a rate-controlled producer
 * gets a 0 bar floor there, and getting this wrong silently clips the rate and
 * destroys the constant-rate assumption the whole analysis rests on.
 */
function runDrawdown(params: Params): Array<{ time: number; bhp: number }> {
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
    sim.setCapillaryParams(0, Number(params.capillaryLambda));
    sim.setGravityEnabled(Boolean(params.gravityEnabled));
    sim.setPermeabilityPerLayer(
        new Float64Array([Number(params.uniformPermX)]),
        new Float64Array([Number(params.uniformPermY)]),
        new Float64Array([Number(params.uniformPermZ)]),
    );
    sim.setStabilityParams(
        Number(params.max_sat_change_per_step),
        Number(params.max_pressure_change_per_step),
        Number(params.max_well_rate_change_fraction),
    );
    sim.setWellControlModes('pressure', 'rate');
    sim.setTargetWellRates(0, Number(params.targetProducerRate));
    sim.setWellBhpLimits(0, Number(params.injectorBhp));
    sim.add_well(
        Number(params.producerI), Number(params.producerJ), 0,
        Number(params.producerBhp), Number(params.well_radius), Number(params.well_skin), false,
    );

    const dt = Number(params.delta_t_days);
    const steps = Number(params.steps);
    const out: Array<{ time: number; bhp: number }> = [];
    for (let i = 0; i < steps; i++) {
        sim.step(dt);
        const wells = sim.getWellState() as Array<Record<string, unknown>>;
        const producer = wells.find((w) => w.injector === false) ?? wells[0];
        // `bhp` is the configured limit and never moves under rate control;
        // `flowing_bhp` is the pressure the solver arrived at. Reading the
        // wrong one gives a flat line and a nonsense permeability.
        out.push({ time: (i + 1) * dt, bhp: Number(producer?.flowing_bhp) });
    }
    return out;
}

/**
 * Analyse a simulated drawdown exactly as an engineer would: fit the semilog
 * straight line over the analysable window, take k from the slope and s from
 * the extrapolated one-hour intercept.
 */
function interpret(params: Params, history: Array<{ time: number; bhp: number }>) {
    const props = extractWellTestProps(params);
    const q = getWellTestRate(params);
    const p_i = Number(params.initialPressure);

    // Restrict the fit to the infinite-acting window: drop the first few steps
    // (near-well transient / timestep startup) and stop before the radius of
    // investigation reaches the no-flow boundary.
    const halfDomain = (Number(params.nx) * Number(params.cellDx)) / 2;
    const usable = history.filter((pt, i) => (
        i >= 5 && Number.isFinite(pt.bhp) && radiusOfInvestigation(pt.time, props) < 0.6 * halfDomain
    ));

    const fit = fitSemilogLine(usable.map((pt) => ({ x: pt.time, y: pt.bhp })));
    const tRef = 1 / 24;
    const pAtRef = fit.intercept - fit.slope * Math.log10(tRef);
    return {
        pointsUsed: usable.length,
        k: permeabilityFromSemilogSlope(fit.slope, q, props.h, props.mu),
        skin: skinFromSemilogIntercept(p_i - pAtRef, fit.slope, props, tRef),
        finalBhp: history.at(-1)?.bhp ?? Number.NaN,
    };
}

describe('dep_welltest — the simulator as a measurement instrument', () => {
    /**
     * The claim the scenario makes to the user is that the semilog slope of
     * the simulated flowing pressure carries permeability and its offset
     * carries skin. That is only worth telling them if the round trip through
     * the simulator actually returns the inputs, so this test performs the
     * interpretation on simulated data rather than on the analytical curve.
     *
     * The tolerances are deliberately loose: a coarse Cartesian grid with a
     * Peaceman well index is not expected to reproduce a line-source solution
     * exactly, and pinning tighter numbers here would convert ordinary engine
     * drift into a failing test. What matters is that the recovered values are
     * the right quantity, not noise.
     */
    it('recovers the input permeability from the simulated drawdown slope', async () => {
        await ensureWasmReady();
        const params = getScenarioWithVariantParams('dep_welltest', 'permeability', 'perm_base');
        const result = interpret(params, runDrawdown(params));

        expect(result.pointsUsed).toBeGreaterThan(20);
        // The rate target must never have been clipped by the BHP floor.
        expect(result.finalBhp).toBeGreaterThan(Number(params.producerBhp));
        expect(result.k).toBeGreaterThan(0.75 * 10);
        expect(result.k).toBeLessThan(1.35 * 10);
    }, 120000);

    it('tracks permeability across the k ladder', async () => {
        await ensureWasmReady();
        const recovered: number[] = [];
        for (const [variant, kTrue] of [['perm_tight', 5], ['perm_base', 10], ['perm_good', 40]] as Array<[string, number]>) {
            const params = getScenarioWithVariantParams('dep_welltest', 'permeability', variant);
            const result = interpret(params, runDrawdown(params));
            expect(result.k / kTrue).toBeGreaterThan(0.7);
            expect(result.k / kTrue).toBeLessThan(1.4);
            recovered.push(result.k);
        }
        // Monotone in the true permeability, which is the visible claim.
        expect(recovered[1]).toBeGreaterThan(recovered[0]);
        expect(recovered[2]).toBeGreaterThan(recovered[1]);
    }, 200000);

    it('separates skin from permeability — offset moves, slope does not', async () => {
        await ensureWasmReady();
        const results = (['skin_stimulated', 'skin_clean', 'skin_damaged'] as const).map((variant) => {
            const params = getScenarioWithVariantParams('dep_welltest', 'skin', variant);
            return interpret(params, runDrawdown(params));
        });

        // Permeability is skin-independent: all three must recover the same k.
        const [a, b, c] = results.map((r) => r.k);
        expect(Math.abs(a - b) / b).toBeLessThan(0.05);
        expect(Math.abs(c - b) / b).toBeLessThan(0.05);

        // Recovered skin ranks correctly and lands near the input, but with a
        // systematic negative bias from the coarse near-well grid — measured
        // 2026-07-24 as -2.49 / -0.67 / +3.89 against inputs of -2 / 0 / +5.
        // That bias is a documented property of the case, not a defect, so it
        // is asserted as a bounded offset rather than papered over.
        expect(results[0].skin).toBeLessThan(results[1].skin);
        expect(results[1].skin).toBeLessThan(results[2].skin);
        for (const [recovered, input] of [[results[0].skin, -2], [results[1].skin, 0], [results[2].skin, 5]] as Array<[number, number]>) {
            expect(recovered).toBeLessThan(input + 0.5);
            expect(recovered).toBeGreaterThan(input - 2.0);
        }
    }, 200000);

    it('gives a grid-insensitive answer, which is what the Peaceman index is for', async () => {
        await ensureWasmReady();
        const ks = (['grid_coarse', 'grid_base', 'grid_fine'] as const).map((variant) => {
            const params = getScenarioWithVariantParams('dep_welltest', 'near_well_grid', variant);
            return interpret(params, runDrawdown(params)).k;
        });
        // All three sit within ~15% of the true 10 mD — the Peaceman index is
        // doing most of its job at every resolution.
        for (const k of ks) {
            expect(k).toBeGreaterThan(0.8 * 10);
            expect(k).toBeLessThan(1.1 * 10);
        }
        // And the bias shrinks monotonically with refinement rather than
        // wandering: measured 2026-07-24 as 8.674 / 9.118 / 9.209 mD for
        // 40 / 20 / 10 m cells. This convergence is the dimension's claim.
        expect(ks[1]).toBeGreaterThan(ks[0]);
        expect(ks[2]).toBeGreaterThan(ks[1]);
    }, 300000);
});
