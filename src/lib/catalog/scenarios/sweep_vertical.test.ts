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

/**
 * Mirrors the oil/water subset of `configureSimulator` in `sim.worker.ts`,
 * including the per-layer permeability path this scenario relies on.
 */
function runAndTrack(params: Params) {
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

    const perLayer = (key: string, uniformKey: string) => (
        String(params.permMode) === 'perLayer' && Array.isArray(params[key])
            ? (params[key] as number[]).map(Number)
            : Array.from({ length: nz }, () => Number(params[uniformKey]))
    );
    sim.setPermeabilityPerLayer(
        new Float64Array(perLayer('layerPermsX', 'uniformPermX')),
        new Float64Array(perLayer('layerPermsY', 'uniformPermY')),
        new Float64Array(perLayer('layerPermsZ', 'uniformPermZ')),
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
    for (let k = 0; k < nz; k++) {
        sim.add_well(Number(params.injectorI), 0, k, injectorBhp, Number(params.well_radius), Number(params.well_skin), true);
        sim.add_well(Number(params.producerI), 0, k, producerBhp, Number(params.well_radius), Number(params.well_skin), false);
    }

    const dt = Number(params.delta_t_days);
    const steps = Number(params.steps);
    for (let i = 0; i < steps; i++) sim.step(dt);

    const poreVolume = nx * ny * nz
        * Number(params.cellDx) * Number(params.cellDy) * Number(params.cellDz)
        * Number(params.reservoirPorosity);
    const mobileOil = poreVolume * (1 - Number(params.initialSaturation) - Number(params.s_or));

    const history = sim.getRateHistorySince(0) as Array<{
        time: number; total_production_oil?: number; total_injection?: number;
    }>;
    let cumOil = 0;
    let cumInjected = 0;
    const track: Array<{ pvi: number; recovery: number }> = [{ pvi: 0, recovery: 0 }];
    for (let i = 0; i < history.length; i++) {
        const stepDt = i > 0 ? history[i].time - history[i - 1].time : history[i].time;
        cumOil += Math.abs(history[i].total_production_oil ?? 0) * stepDt;
        cumInjected += Math.abs(history[i].total_injection ?? 0) * stepDt;
        track.push({ pvi: cumInjected / poreVolume, recovery: cumOil / mobileOil });
    }

    return {
        endPvi: cumInjected / poreVolume,
        /**
         * Recovery of mobile oil sampled against injected pore volumes. These
         * variants have different total kh, so under BHP control they reach
         * different PVI in the same 125 days — comparing at equal *time* would
         * compare different points of the flood, which is the trap documented
         * in the T7.11 negative result.
         */
        recoveryAtPvi(target: number): number {
            for (let i = 1; i < track.length; i++) {
                if (track[i].pvi >= target) {
                    const a = track[i - 1];
                    const b = track[i];
                    const span = b.pvi - a.pvi;
                    if (span <= 0) return b.recovery;
                    return a.recovery + ((target - a.pvi) / span) * (b.recovery - a.recovery);
                }
            }
            return Number.NaN;
        },
    };
}

describe('sweep_vertical / endpoints_vs_geology — rock curves vs geology (T7.18)', () => {
    /**
     * Two claims the dimension makes to the user, both worth guarding:
     *
     *  1. Each change alone reduces recovery — otherwise there is no dilemma.
     *  2. The two are strongly *sub*-additive: combined, they cost much less
     *     than the sum of their individual effects. That direction was
     *     measured, not assumed — the first draft of this dimension asserted
     *     the opposite (amplification, as in wf_tornado) and the simulator
     *     said no. Measured 2026-07-24 at 0.625 PVI:
     *       base 0.7257 | curve 0.5786 | layers 0.3794 | both 0.3573
     *     individual sum -0.4935 vs actual combined -0.3684.
     */
    it('shows both mechanisms hurting recovery, and the two masking each other when combined', async () => {
        await ensureWasmReady();

        const dim = 'endpoints_vs_geology';
        const runs = {
            base: runAndTrack(getScenarioWithVariantParams('sweep_vertical', dim, 'evg_base')),
            curve: runAndTrack(getScenarioWithVariantParams('sweep_vertical', dim, 'evg_curve_only')),
            layers: runAndTrack(getScenarioWithVariantParams('sweep_vertical', dim, 'evg_layers_only')),
            both: runAndTrack(getScenarioWithVariantParams('sweep_vertical', dim, 'evg_both')),
        };

        // Compare every variant at the same injected pore volume.
        const commonPvi = Math.min(...Object.values(runs).map((r) => r.endPvi)) * 0.95;
        expect(commonPvi, 'runs must overlap in PVI').toBeGreaterThan(0.3);

        const rf = Object.fromEntries(
            Object.entries(runs).map(([k, r]) => [k, r.recoveryAtPvi(commonPvi)]),
        ) as Record<keyof typeof runs, number>;

        for (const value of Object.values(rf)) {
            expect(Number.isFinite(value)).toBe(true);
            expect(value).toBeGreaterThan(0);
        }

        // 1. Each mechanism alone costs recovery.
        expect(rf.curve).toBeLessThan(rf.base);
        expect(rf.layers).toBeLessThan(rf.base);
        expect(rf.both).toBeLessThan(rf.curve);
        expect(rf.both).toBeLessThan(rf.layers);

        // 2. The interaction, and its direction: the Corey change costs far
        //    less against an already-layered background than against uniform
        //    rock, so the combined penalty is well short of additive.
        const curveCostUniform = rf.base - rf.curve;
        const curveCostLayered = rf.layers - rf.both;
        expect(curveCostLayered).toBeLessThan(0.5 * curveCostUniform);

        const additivePrediction = curveCostUniform + (rf.base - rf.layers);
        const actualCombined = rf.base - rf.both;
        expect(actualCombined).toBeLessThan(0.9 * additivePrediction);
    }, 300000);
});
