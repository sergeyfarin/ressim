import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { computeBLRecoveryVsPVI, computeWelgeMetrics } from '../../analytical/fractionalFlow';
import { getScenario, getScenarioWithVariantParams } from '../scenarios';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

const ROCK = { s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2, k_rw_max: 1, k_ro_max: 1 };
const FLUID = { mu_w: 0.5, mu_o: 1.0 };
const BL_RECOVERY_AT_ONE_PVI = computeBLRecoveryVsPVI(ROCK, FLUID, 2, 400).find((p) => p.pvi >= 1)!.rf;

type VariantRun = {
    breakthroughPvi: number;
    recoveryAtOnePvi: number;
    warning: string;
};

/** Runs one variant of the column against the wasm core. */
function runVariant(dimensionKey: string, variantKey: string): VariantRun {
    const params = getScenarioWithVariantParams('wf_gravity_stability', dimensionKey, variantKey);
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const nz = Number(params.nz);
    const porosity = Number(params.reservoirPorosity);
    const sim = new ReservoirSimulator(nx, ny, nz, porosity);

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
    sim.setTargetWellRates(Number(params.targetInjectorRate), Number(params.targetProducerRate));
    sim.setWellBhpLimits(Number(params.producerBhp), Number(params.injectorBhp));
    for (const k of params.injectorKLayers as number[]) {
        sim.add_well(0, 0, k, Number(params.injectorBhp), Number(params.well_radius), Number(params.well_skin), true);
    }
    for (const k of params.producerKLayers as number[]) {
        sim.add_well(0, 0, k, Number(params.producerBhp), Number(params.well_radius), Number(params.well_skin), false);
    }

    const poreVolume = nx * Number(params.cellDx) * ny * Number(params.cellDy)
        * nz * Number(params.cellDz) * porosity;
    const oilInPlace = poreVolume * (1 - Number(params.s_wc));
    const dt = Number(params.delta_t_days);

    let cumulativeOil = 0;
    let cumulativeInjection = 0;
    let breakthroughPvi = Number.NaN;
    let recoveryAtOnePvi = 0;
    for (let step = 0; step < Number(params.steps); step += 1) {
        sim.step(dt);
        const point = sim.getRateHistory().at(-1) as Record<string, number> | undefined;
        const oilRate = Math.abs(Number(point?.total_production_oil ?? 0));
        const liquidRate = Math.abs(Number(point?.total_production_liquid ?? 0));
        const waterRate = Math.max(0, liquidRate - oilRate);
        cumulativeOil += oilRate * dt;
        cumulativeInjection += Math.abs(Number(point?.total_injection_reservoir ?? 0)) * dt;
        const pvi = cumulativeInjection / poreVolume;
        if (!Number.isFinite(breakthroughPvi) && liquidRate > 0 && waterRate / liquidRate > 0.01) {
            breakthroughPvi = pvi;
        }
        if (recoveryAtOnePvi === 0 && pvi >= 1) {
            recoveryAtOnePvi = cumulativeOil / oilInPlace;
        }
    }
    const warning = sim.getLastSolverWarning();
    sim.free();
    return { breakthroughPvi, recoveryAtOnePvi, warning };
}

describe('wf_gravity_stability scenario definition', () => {
    it('keeps every variant on one analytical solution and one pore volume', () => {
        const scenario = getScenario('wf_gravity_stability');
        const analyticalInputs = new Set(['mu_w', 'mu_o', 's_wc', 's_or', 'n_w', 'n_o', 'k_rw_max', 'k_ro_max', 'initialSaturation']);
        const basePv = 1 * 20 * 1 * 20 * 60 * 1 * 0.2;

        for (const dimension of scenario?.sensitivities ?? []) {
            expect(dimension.analyticalOverlayMode, dimension.key).toBe('shared');
            for (const variant of dimension.variants) {
                expect(variant.affectsAnalytical, variant.key).toBe(false);
                expect(Object.keys(variant.paramPatch).filter((n) => analyticalInputs.has(n)), variant.key).toEqual([]);
                // Grid variants rescale cellDz with nz, so PVI means the same
                // thing on every curve in the chart.
                const params = getScenarioWithVariantParams('wf_gravity_stability', dimension.key, variant.key);
                expect(Number(params.nz) * Number(params.cellDz) * 400 * 0.2, variant.key).toBeCloseTo(basePv, 6);
                // Perforations must be at opposite ends of whatever grid the
                // variant declares, or the flood is not end to end.
                const injector = (params.injectorKLayers as number[])[0];
                const producer = (params.producerKLayers as number[])[0];
                expect([injector, producer].sort((a, b) => a - b), variant.key)
                    .toEqual([0, Number(params.nz) - 1]);
            }
        }
    });
});

describe('wf_gravity_stability measured behaviour', () => {
    it('brackets the analytical solution instead of bounding it', async () => {
        await ensureWasmReady();

        const control = runVariant('flood_direction', 'dir_no_gravity');
        const up = runVariant('flood_direction', 'dir_up');
        const down = runVariant('flood_direction', 'dir_down');

        for (const [key, run] of Object.entries({ control, up, down })) {
            expect(run.warning, key).toBe('');
        }

        expect(computeWelgeMetrics(ROCK, FLUID, 0.1).breakthroughPvi).toBeCloseTo(0.5856, 3);

        // Control on the analytical curve; the two directions on opposite sides
        // of it. Measured 0.706 / 0.792 / 0.540 against BL's 0.715.
        expect(Math.abs(control.recoveryAtOnePvi - BL_RECOVERY_AT_ONE_PVI) / BL_RECOVERY_AT_ONE_PVI)
            .toBeLessThan(0.02);
        expect(up.recoveryAtOnePvi).toBeGreaterThan(BL_RECOVERY_AT_ONE_PVI + 0.05);
        expect(down.recoveryAtOnePvi).toBeLessThan(BL_RECOVERY_AT_ONE_PVI - 0.15);
        // Breakthrough later than the analytical shock in the stable direction —
        // the signature that this is not numerical dispersion, which can only
        // ever smear the front forward.
        expect(up.breakthroughPvi).toBeGreaterThan(computeWelgeMetrics(ROCK, FLUID, 0.1).breakthroughPvi);
        expect(down.breakthroughPvi).toBeLessThan(control.breakthroughPvi);
    }, 180_000);

    it('fans symmetrically about the analytical curve as the gravity number rises', async () => {
        await ensureWasmReady();

        const upFast = runVariant('gravity_number', 'g_up_fast');
        const upSlow = runVariant('gravity_number', 'g_up_slow');
        const downFast = runVariant('gravity_number', 'g_down_fast');
        const downSlow = runVariant('gravity_number', 'g_down_slow');

        for (const [key, run] of Object.entries({ upFast, upSlow, downFast, downSlow })) {
            expect(run.warning, key).toBe('');
        }

        // Monotone and opposite: up 0.737 → 0.839, down 0.673 → 0.354 as G goes
        // from 0.33 to 3.3. Slowing the flood helps one direction and ruins the other.
        expect(upSlow.recoveryAtOnePvi).toBeGreaterThan(upFast.recoveryAtOnePvi);
        expect(downSlow.recoveryAtOnePvi).toBeLessThan(downFast.recoveryAtOnePvi);
        expect(upSlow.recoveryAtOnePvi).toBeCloseTo(0.839, 1);
        expect(downSlow.recoveryAtOnePvi).toBeCloseTo(0.354, 1);
        expect(upSlow.recoveryAtOnePvi - downSlow.recoveryAtOnePvi).toBeGreaterThan(0.4);
    }, 180_000);

    it('does not converge onto the analytical curve under refinement', async () => {
        await ensureWasmReady();

        const coarseUp = runVariant('resolution', 'res_15');
        const fineUp = runVariant('resolution', 'res_60');
        const coarseDown = runVariant('resolution', 'res_15_down');
        const fineDown = runVariant('resolution', 'res_60_down');
        const fineControl = runVariant('resolution', 'res_120_off');

        // Numerical dispersion converges away: the gravity-free control closes
        // to within 1 % of BL at 120 cells.
        expect(Math.abs(fineControl.recoveryAtOnePvi - BL_RECOVERY_AT_ONE_PVI) / BL_RECOVERY_AT_ONE_PVI)
            .toBeLessThan(0.015);
        // The gravity answers move by less than 0.03 across a 4x refinement and
        // stay far from BL — measured 0.772 → 0.792 up and 0.542 → 0.540 down.
        expect(Math.abs(fineUp.recoveryAtOnePvi - coarseUp.recoveryAtOnePvi)).toBeLessThan(0.03);
        expect(Math.abs(fineDown.recoveryAtOnePvi - coarseDown.recoveryAtOnePvi)).toBeLessThan(0.03);
        expect(fineUp.recoveryAtOnePvi).toBeGreaterThan(BL_RECOVERY_AT_ONE_PVI + 0.05);
        expect(fineDown.recoveryAtOnePvi).toBeLessThan(BL_RECOVERY_AT_ONE_PVI - 0.15);
    }, 240_000);
});
