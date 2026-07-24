import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
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

type Params = Record<string, unknown>;

type RunResult = {
    /** Injected pore volumes at which producer water cut first exceeds 1%. */
    breakthroughPvi: number;
    /** Injected pore volumes at which producer water cut first exceeds 80%. */
    latePvi: number;
    /** latePvi - breakthroughPvi: how spread out the front is, in PVI. */
    frontWidthPvi: number;
};

/** Mirrors the oil/water subset of `configureSimulator` in `sim.worker.ts`. */
function buildAndRun(params: Params): RunResult {
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
    // The scenario's whole subject: mirror the worker's gate exactly — a
    // disabled capillary model is passed as entry pressure 0, not skipped.
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
    sim.setTargetWellRates(Number(params.targetInjectorRate ?? 0), Number(params.targetProducerRate ?? 0));
    const producerBhp = Number(params.producerBhp);
    const injectorBhp = Number(params.injectorBhp);
    sim.setWellBhpLimits(Math.min(producerBhp, injectorBhp), Math.max(producerBhp, injectorBhp));

    for (let k = 0; k < nz; k++) {
        sim.add_well(Number(params.injectorI), Number(params.injectorJ), k, injectorBhp, Number(params.well_radius), Number(params.well_skin), true);
        sim.add_well(Number(params.producerI), Number(params.producerJ), k, producerBhp, Number(params.well_radius), Number(params.well_skin), false);
    }

    const dt = Number(params.delta_t_days);
    const steps = Number(params.steps);
    for (let i = 0; i < steps; i++) {
        sim.step(dt);
    }

    const poreVolume = nx * ny * nz
        * Number(params.cellDx) * Number(params.cellDy) * Number(params.cellDz)
        * Number(params.reservoirPorosity);

    // Rates are instantaneous (m3/day); water cut = (liquid - oil)/liquid, the
    // convention used by buildRateChartData.ts.
    const history = sim.getRateHistorySince(0) as Array<{
        time: number;
        total_production_oil?: number;
        total_production_liquid?: number;
        total_injection?: number;
    }>;
    let cumInjected = 0;
    let breakthroughPvi = Number.NaN;
    let latePvi = Number.NaN;
    for (let i = 0; i < history.length; i++) {
        const stepDt = i > 0 ? history[i].time - history[i - 1].time : history[i].time;
        cumInjected += Math.abs(history[i].total_injection ?? 0) * stepDt;
        const oil = Math.abs(history[i].total_production_oil ?? 0);
        const liquid = Math.abs(history[i].total_production_liquid ?? 0);
        if (liquid <= 0) continue;
        const waterCut = (liquid - oil) / liquid;
        if (Number.isNaN(breakthroughPvi) && waterCut > 0.01) breakthroughPvi = cumInjected / poreVolume;
        if (Number.isNaN(latePvi) && waterCut > 0.8) latePvi = cumInjected / poreVolume;
    }

    return { breakthroughPvi, latePvi, frontWidthPvi: latePvi - breakthroughPvi };
}

describe('wf_capillary — capillary smearing vs the Buckley-Leverett shock', () => {
    it('spreads the front monotonically with entry pressure, and brings first water in earlier', async () => {
        await ensureWasmReady();

        const dim = 'capillary_strength';
        const off = buildAndRun(getScenarioWithVariantParams('wf_capillary', dim, 'pc_off'));
        const weak = buildAndRun(getScenarioWithVariantParams('wf_capillary', dim, 'pc_weak'));
        const base = buildAndRun(getScenarioWithVariantParams('wf_capillary', dim, 'pc_base'));
        const strong = buildAndRun(getScenarioWithVariantParams('wf_capillary', dim, 'pc_strong'));

        // Measured 2026-07-24 on this tree (96 cells, 40 bar drawdown, 500 days):
        //   P_e (bar)      0        1        3        8
        //   breakthrough  0.5650   0.5457   0.5157   0.4498   PVI
        //   front width   0.0805   0.1211   0.1776   0.2892   PVI
        // The assertions below are directional rather than pinned to these
        // numbers, so ordinary engine drift does not fail the test while a
        // sign flip or a collapse of the effect still would.
        for (const r of [off, weak, base, strong]) {
            expect(Number.isFinite(r.breakthroughPvi), 'every variant must break through').toBe(true);
            expect(Number.isFinite(r.latePvi), 'every variant must reach 80% water cut').toBe(true);
        }

        // Capillary imbibition runs ahead of the viscous front: first water
        // arrives no later than the zero-capillary case, and earlier for the
        // strong rock.
        expect(strong.breakthroughPvi).toBeLessThan(off.breakthroughPvi);

        // Front width grows monotonically with entry pressure — the core claim.
        expect(weak.frontWidthPvi).toBeGreaterThanOrEqual(off.frontWidthPvi);
        expect(base.frontWidthPvi).toBeGreaterThan(weak.frontWidthPvi);
        expect(strong.frontWidthPvi).toBeGreaterThan(base.frontWidthPvi);
    }, 120000);

    it('distinguishes numerical smearing (converges under refinement) from capillary smearing (does not)', async () => {
        await ensureWasmReady();

        const dim = 'capillary_vs_numerical';
        const coarseNoPc = buildAndRun(getScenarioWithVariantParams('wf_capillary', dim, 'cvn_coarse_nopc'));
        const fineNoPc = buildAndRun(getScenarioWithVariantParams('wf_capillary', dim, 'cvn_fine_nopc'));
        const finePc = buildAndRun(getScenarioWithVariantParams('wf_capillary', dim, 'cvn_fine_pc'));
        const finerPc = buildAndRun(getScenarioWithVariantParams('wf_capillary', dim, 'cvn_finer_pc'));

        // Refining a capillary-free run sharpens the front substantially.
        const numericalSharpening = coarseNoPc.frontWidthPvi - fineNoPc.frontWidthPvi;
        expect(numericalSharpening).toBeGreaterThan(0);

        // Refining a strongly capillary run barely changes it: the front width
        // is set by the rock, not the mesh. This is the discriminator the
        // scenario description tells the user to look for, so it is the claim
        // most worth guarding.
        const capillarySharpening = Math.abs(finePc.frontWidthPvi - finerPc.frontWidthPvi);
        expect(capillarySharpening).toBeLessThan(numericalSharpening);
    }, 120000);
});
