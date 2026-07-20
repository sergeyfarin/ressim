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

function buildAndRun(params: Params): { displacementEfficiency: number } {
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
    sim.setTargetWellRates(0, 0);
    const producerBhp = Number(params.producerBhp);
    const injectorBhp = Number(params.injectorBhp);
    sim.setWellBhpLimits(Math.min(producerBhp, injectorBhp), Math.max(producerBhp, injectorBhp));

    const producerI = Number(params.producerI);
    const injectorI = Number(params.injectorI);
    for (let k = 0; k < nz; k++) {
        sim.add_well(injectorI, 0, k, injectorBhp, Number(params.well_radius), Number(params.well_skin), true);
        sim.add_well(producerI, 0, k, producerBhp, Number(params.well_radius), Number(params.well_skin), false);
    }

    const dt = Number(params.delta_t_days);
    const steps = Number(params.steps);
    for (let i = 0; i < steps; i++) {
        sim.step(dt);
    }

    // Both wells run under pressure (BHP) control, not rate control, so a
    // fixed-time cumulative recovery factor is not a valid cross-variant
    // metric here: better vertical communication (high kv) simply moves more
    // total fluid in the same 800 days, which *raises* fixed-time recovery
    // for every variant regardless of how much oil is bypassed. Verified
    // 2026-07-18: cumulative RF for "both" is actually ~4pp *higher* than
    // base at day 800 (and at every earlier checkpoint back to day 100),
    // the opposite of the intended bypassed-oil story.
    //
    // Displacement/sweep efficiency — oil recovered per unit water injected
    // — is the metric that actually isolates bypassing: it is monotonically
    // worse for the interaction case at every checkpoint, independent of how
    // much total throughput the pressure-controlled wells achieve.
    // `total_production_oil`/`total_injection` on each history point are
    // instantaneous rates (m3/day), not cumulatives — integrate rate x dt
    // across the full history, matching the convention in buildRateChartData.ts.
    const history = sim.getRateHistorySince(0) as Array<{
        time: number;
        total_production_oil?: number;
        total_injection?: number;
    }>;
    let cumOil = 0;
    let cumInjected = 0;
    for (let i = 0; i < history.length; i++) {
        const stepDt = i > 0 ? history[i].time - history[i - 1].time : history[i].time;
        cumOil += Math.abs(history[i].total_production_oil ?? 0) * stepDt;
        cumInjected += Math.abs(history[i].total_injection ?? 0) * stepDt;
    }

    return { displacementEfficiency: cumOil / cumInjected };
}

describe('wf_tornado — kv x density-contrast interaction', () => {
    it('shows the combined-variant displacement-efficiency drop is much larger than either single change alone', async () => {
        await ensureWasmReady();

        const base = buildAndRun(getScenarioWithVariantParams('wf_tornado', 'interaction', 'interaction_base'));
        const kvOnly = buildAndRun(getScenarioWithVariantParams('wf_tornado', 'interaction', 'interaction_kv_only'));
        const rhoOnly = buildAndRun(getScenarioWithVariantParams('wf_tornado', 'interaction', 'interaction_rho_only'));
        const both = buildAndRun(getScenarioWithVariantParams('wf_tornado', 'interaction', 'interaction_both'));

        const kvOnlyDrop = 1 - kvOnly.displacementEfficiency / base.displacementEfficiency;
        const rhoOnlyDrop = 1 - rhoOnly.displacementEfficiency / base.displacementEfficiency;
        const bothDrop = 1 - both.displacementEfficiency / base.displacementEfficiency;

        // Individually, both single-parameter changes should be small.
        expect(kvOnlyDrop).toBeGreaterThan(0);
        expect(kvOnlyDrop).toBeLessThan(0.08);
        expect(Math.abs(rhoOnlyDrop)).toBeLessThan(0.02);

        // Combined, the drop should be substantially larger than either
        // individual drop — the actual claim this scenario exists to make.
        // Verified headless (2026-07-18) at ~7.5% combined vs ~2.3%/~0.02%
        // individually; assert a conservative margin so the test tolerates
        // minor engine drift without losing the qualitative signal.
        expect(bothDrop).toBeGreaterThan(0.05);
        expect(bothDrop).toBeGreaterThan(2.5 * Math.max(kvOnlyDrop, Math.abs(rhoOnlyDrop)));
    }, 30000);
});
