import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { computeBLRecoveryVsPVI, computeWelgeMetrics } from '../../analytical/fractionalFlow';
import { getScenario, getScenarioWithVariantParams } from '../scenarios';
import { listOpmFlowArtifacts } from '../opmFlowArtifacts';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

const ROCK = { s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2, k_rw_max: 1, k_ro_max: 1 };
const FLUID = { mu_w: 0.5, mu_o: 1.0 };

/** Analytical breakthrough and recovery at 1 PVI for a given oil Corey exponent. */
function analyticalFor(n_o: number) {
    const rock = { ...ROCK, n_o };
    return {
        breakthroughPvi: computeWelgeMetrics(rock, FLUID, 0.1).breakthroughPvi,
        recoveryAtOnePvi: computeBLRecoveryVsPVI(rock, FLUID, 2, 400).find((p) => p.pvi >= 1)!.rf,
    };
}

type VariantRun = {
    breakthroughPvi: number;
    recoveryAtOnePvi: number;
    warning: string;
};

/** Runs one variant of the column against the wasm core. */
function runVariant(dimensionKey: string, variantKey: string): VariantRun {
    const params = getScenarioWithVariantParams('wf_numerics', dimensionKey, variantKey);
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
        new Float64Array([Number(params.uniformPermX)]),
        new Float64Array([Number(params.uniformPermY)]),
        new Float64Array([Number(params.uniformPermZ)]),
    );
    sim.setStabilityParams(
        Number(params.max_sat_change_per_step),
        Number(params.max_pressure_change_per_step),
        Number(params.max_well_rate_change_fraction),
    );
    sim.setWellControlModes(String(params.injectorControlMode), String(params.producerControlMode));
    sim.setTargetWellRates(Number(params.targetInjectorRate), Number(params.targetProducerRate));
    sim.setWellBhpLimits(Number(params.producerBhp), Number(params.injectorBhp));
    sim.add_well(0, 0, 0, Number(params.injectorBhp), Number(params.well_radius), Number(params.well_skin), true);
    sim.add_well(
        Number(params.producerI), 0, 0,
        Number(params.producerBhp), Number(params.well_radius), Number(params.well_skin), false,
    );

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

describe('wf_numerics scenario definition', () => {
    it('holds pore volume, rate and rock fixed across every grid variant', () => {
        const scenario = getScenario('wf_numerics');
        expect(scenario).not.toBeNull();
        const basePoreVolume = 50 * 10 * 1 * 20 * 1 * 10 * 0.2;

        for (const variant of scenario!.sensitivities.find((d) => d.key === 'grid_refinement')!.variants) {
            const params = getScenarioWithVariantParams('wf_numerics', 'grid_refinement', variant.key);
            expect(Number(params.nx) * Number(params.cellDx) * 20 * 10 * 0.2, variant.key)
                .toBeCloseTo(basePoreVolume, 6);
            // The producer must sit in the last cell of whatever grid the
            // variant declares, or the flood is not end to end.
            expect(Number(params.producerI), variant.key).toBe(Number(params.nx) - 1);
            // Nothing physical may change: one analytical curve serves them all.
            expect(variant.affectsAnalytical, variant.key).toBe(false);
        }
    });

    it('injects the same total volume in every timestep variant', () => {
        const scenario = getScenario('wf_numerics');
        for (const variant of scenario!.sensitivities.find((d) => d.key === 'time_truncation')!.variants) {
            const params = getScenarioWithVariantParams('wf_numerics', 'time_truncation', variant.key);
            const days = Number(params.delta_t_days) * Number(params.steps);
            expect(days, variant.key).toBeCloseTo(260, 0);
        }
    });

    it('marks only the rock-curve variant as changing the analytical solution', () => {
        const scenario = getScenario('wf_numerics');
        const dimension = scenario!.sensitivities.find((d) => d.key === 'dispersion_or_rock')!;
        expect(dimension.analyticalOverlayMode).toBe('per-result');
        expect(dimension.variants.filter((v) => v.affectsAnalytical).map((v) => v.key)).toEqual(['dor_steep']);
    });
});

describe('wf_numerics measured behaviour', () => {
    it('converges onto the analytical solution at first order in cell size', async () => {
        await ensureWasmReady();
        const bl = analyticalFor(2);
        expect(bl.breakthroughPvi).toBeCloseTo(0.5856, 3);
        expect(bl.recoveryAtOnePvi).toBeCloseTo(0.7151, 3);

        const ladder = ['grid_10', 'grid_25', 'grid_50', 'grid_100', 'grid_200', 'grid_400']
            .map((key) => runVariant('grid_refinement', key));

        for (const run of ladder) expect(run.warning).toBe('');

        // Breakthrough and recovery both approach the analytical values from
        // below, monotonically, with no rung out of order.
        for (let i = 1; i < ladder.length; i += 1) {
            expect(ladder[i].breakthroughPvi, `rung ${i}`).toBeGreaterThan(ladder[i - 1].breakthroughPvi - 1e-9);
            expect(ladder[i].recoveryAtOnePvi, `rung ${i}`).toBeGreaterThan(ladder[i - 1].recoveryAtOnePvi - 1e-9);
            expect(ladder[i].breakthroughPvi, `rung ${i}`).toBeLessThan(bl.breakthroughPvi + 1e-9);
            expect(ladder[i].recoveryAtOnePvi, `rung ${i}`).toBeLessThan(bl.recoveryAtOnePvi);
        }

        // First-order convergence: halving the cell size halves the error.
        // Checked on the three rungs above the reporting resolution, where the
        // error is still large compared with one report step (0.005 PVI).
        const errors = ladder.map((run) => bl.breakthroughPvi - run.breakthroughPvi);
        expect(errors[0]).toBeCloseTo(0.131, 2);
        for (let i = 1; i < 4; i += 1) {
            const ratio = errors[i - 1] / errors[i];
            expect(ratio, `error ratio at rung ${i}`).toBeGreaterThan(1.7);
            expect(ratio, `error ratio at rung ${i}`).toBeLessThan(2.5);
        }
        // The finest grid lands on the analytical answer to within one report step.
        expect(errors.at(-1)!).toBeLessThan(0.005);
    }, 300_000);

    it('shows the stability limiter, not the report step, as the load-bearing control', async () => {
        await ensureWasmReady();
        const bl = analyticalFor(2);
        const mobileFraction = (1 - 0.1 - 0.1) / (1 - 0.1);

        const quarter = runVariant('time_truncation', 'dt_quarter');
        const base = runVariant('time_truncation', 'dt_base');
        const four = runVariant('time_truncation', 'dt_four');
        const ten = runVariant('time_truncation', 'dt_ten');

        // A 40x change in report step costs under 3 % of recovery, and every
        // protected run stays below the analytical solution.
        for (const run of [quarter, base, four, ten]) {
            expect(run.recoveryAtOnePvi).toBeLessThan(bl.recoveryAtOnePvi);
            expect(run.recoveryAtOnePvi).toBeGreaterThan(0.68);
        }
        expect(quarter.recoveryAtOnePvi - ten.recoveryAtOnePvi).toBeLessThan(0.03);

        // Relaxing the limiter breaks it in the other direction.
        const half = runVariant('time_truncation', 'dt_limiter_half');
        const off = runVariant('time_truncation', 'dt_limiter_off');
        expect(half.recoveryAtOnePvi).toBeGreaterThan(bl.recoveryAtOnePvi);
        expect(off.recoveryAtOnePvi).toBeGreaterThan(mobileFraction);
        // It used to happen silently; since 2026-08-01 the run says so. The
        // detector is material balance, not the saturations — transport clamps
        // those at the end points, so they look perfectly physical.
        expect(off.warning).toContain('Material balance');
        // The half-relaxed run is wrong without being mass-creating: it lands
        // above the analytical curve, but volumes still balance, so it does not
        // warn and should not. Being above an exact solution is a discretization
        // error; inventing barrels is a different failure.
        expect(half.warning).toBe('');
    }, 300_000);

    it('cannot separate grid smearing from a steeper rock curve on breakthrough alone', async () => {
        await ensureWasmReady();

        const coarse = runVariant('dispersion_or_rock', 'dor_coarse');
        const fine = runVariant('dispersion_or_rock', 'dor_fine');
        const steep = runVariant('dispersion_or_rock', 'dor_steep');
        for (const run of [coarse, fine, steep]) expect(run.warning).toBe('');

        // Breakthrough is the same to within 3 %.
        expect(Math.abs(coarse.breakthroughPvi - steep.breakthroughPvi) / steep.breakthroughPvi)
            .toBeLessThan(0.03);
        // Recovery is not: a tenth of the oil in place apart.
        expect(coarse.recoveryAtOnePvi - steep.recoveryAtOnePvi).toBeGreaterThan(0.08);

        // The steep-curve run is converged against its *own* analytical
        // solution; the coarse run is in error against the shared one.
        const steepBl = analyticalFor(3.5);
        expect(steep.recoveryAtOnePvi).toBeCloseTo(steepBl.recoveryAtOnePvi, 2);
        const baseBl = analyticalFor(2);
        expect(baseBl.recoveryAtOnePvi - coarse.recoveryAtOnePvi).toBeGreaterThan(0.02);
        expect(baseBl.recoveryAtOnePvi - fine.recoveryAtOnePvi).toBeLessThan(0.005);
    }, 300_000);

    it('records that the two solver paths disagree on breakthrough and agree on recovery', async () => {
        await ensureWasmReady();
        const impes = runVariant('solver_vs_opm', 'solver_impes');
        const fim = runVariant('solver_vs_opm', 'solver_fim');
        for (const run of [impes, fim]) expect(run.warning).toBe('');

        expect(Math.abs(impes.recoveryAtOnePvi - fim.recoveryAtOnePvi)).toBeLessThan(0.005);
        expect(impes.breakthroughPvi - fim.breakthroughPvi).toBeGreaterThan(0.01);

        // The claim the OPM reference is carried for: an independent fully
        // implicit simulator on the same deck lands on the FIM answer, not the
        // IMPES one, so the 0.02 PVI gap is the formulation and not a defect.
        const opm = opmMetrics('wf_numerics');
        expect(opm.breakthroughPvi).toBeCloseTo(fim.breakthroughPvi, 2);
        expect(Math.abs(opm.breakthroughPvi - impes.breakthroughPvi)).toBeGreaterThan(0.01);
        expect(opm.recoveryAtOnePvi).toBeCloseTo(fim.recoveryAtOnePvi, 2);
    }, 300_000);

    it('carries an OPM Flow run at both the base and the converged resolution', () => {
        const coarse = opmMetrics('wf_numerics');
        const fine = opmMetrics('wf_numerics_fine');
        const bl = analyticalFor(2);

        // OPM refines towards the same analytical answer from the same side.
        expect(fine.breakthroughPvi).toBeGreaterThan(coarse.breakthroughPvi);
        expect(fine.breakthroughPvi).toBeLessThan(bl.breakthroughPvi);
        expect(fine.recoveryAtOnePvi).toBeGreaterThan(coarse.recoveryAtOnePvi);
        expect(fine.recoveryAtOnePvi).toBeLessThan(bl.recoveryAtOnePvi);

        // Both decks describe this scenario's pore volume, which is what makes
        // their PVI axis the same axis the simulation is plotted on.
        for (const key of ['wf_numerics', 'wf_numerics_fine']) {
            const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === key)!;
            expect(artifact.xAxis?.poreVolumeM3, key).toBe(50 * 10 * 1 * 20 * 1 * 10 * 0.2);
        }
    });
});

/** Breakthrough and recovery read off a bundled OPM Flow artifact. */
function opmMetrics(caseKey: string) {
    const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === caseKey)!;
    expect(artifact.status).toBe('parsed');
    const pviByDay = new Map(artifact.xAxis!.timeDays.map((day, i) => [day, artifact.xAxis!.pvi![i]]));
    const byCurve = (match: string) => artifact.series.find((s) => s.curveKey.includes(match))!;
    const waterCut = byCurve('water-cut').data;
    const cumOil = byCurve('cum-oil').data;

    const breakthroughPvi = pviByDay.get(waterCut.find((p) => p.y > 0.01)!.x)!;
    const oilInPlace = artifact.xAxis!.poreVolumeM3! * (1 - 0.1);
    const atOnePvi = cumOil.find((p) => (pviByDay.get(p.x) ?? 0) >= 1)!;
    return { breakthroughPvi, recoveryAtOnePvi: atOnePvi.y / oilInPlace };
}
