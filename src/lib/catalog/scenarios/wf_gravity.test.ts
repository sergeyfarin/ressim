import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { computeBLRecoveryVsPVI, computeWelgeMetrics } from '../../analytical/fractionalFlow';
import { getScenario, getScenarioWithVariantParams } from '../scenarios';
import { listDeclaredOpmFlowArtifactKeys, resolveScenarioReferenceSeries } from '../opmFlowArtifacts';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

const ROCK = { s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2, k_rw_max: 1, k_ro_max: 1 };
const FLUID = { mu_w: 0.5, mu_o: 1.0 };

type VariantRun = {
    /** PVI at which produced water cut first exceeds 1 %. */
    breakthroughPvi: number;
    /** Recovery of oil in place at one pore volume injected. */
    recoveryAtOnePvi: number;
    warning: string;
    /** Per-completion wellbore head offsets [bar], in the order wells were added. */
    headOffsets: number[];
};

/** Completion overrides, for exercising fully perforated wells under gravity. */
type CompletionOverride = {
    injectorKLayers: number[];
    producerKLayers: number[];
    /** Depth [m] to quote both wells' BHP at; omit for the shallowest completion. */
    datumDepth?: number;
    /** Scenario params to replace before the run (rates, BHPs, step count). */
    params?: Record<string, number | boolean>;
};

/**
 * Runs one variant end to end against the wasm core, mirroring what the worker
 * builds from the same params. Completions come from the scenario unless
 * `override` replaces them.
 */
function runVariant(
    dimensionKey: string,
    variantKey: string,
    override?: CompletionOverride,
): VariantRun {
    const params = { ...getScenarioWithVariantParams('wf_gravity', dimensionKey, variantKey), ...override?.params };
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
    const injectorLayers = override?.injectorKLayers ?? (params.injectorKLayers as number[]);
    const producerLayers = override?.producerKLayers ?? (params.producerKLayers as number[]);
    if (override) {
        // Identified completions, so the datum can be attached to the physical well.
        for (const k of injectorLayers) {
            sim.addWellWithId(
                Number(params.injectorI), Number(params.injectorJ), k, Number(params.injectorBhp),
                Number(params.well_radius), Number(params.well_skin), true, 'injector-main',
            );
        }
        for (const k of producerLayers) {
            sim.addWellWithId(
                Number(params.producerI), Number(params.producerJ), k, Number(params.producerBhp),
                Number(params.well_radius), Number(params.well_skin), false, 'producer-main',
            );
        }
        if (override.datumDepth !== undefined) {
            sim.setWellDatum('injector-main', override.datumDepth, Number.NaN);
            sim.setWellDatum('producer-main', override.datumDepth, Number.NaN);
        }
    } else {
        for (const k of injectorLayers) {
            sim.add_well(
                Number(params.injectorI), Number(params.injectorJ), k, Number(params.injectorBhp),
                Number(params.well_radius), Number(params.well_skin), true,
            );
        }
        for (const k of producerLayers) {
            sim.add_well(
                Number(params.producerI), Number(params.producerJ), k, Number(params.producerBhp),
                Number(params.well_radius), Number(params.well_skin), false,
            );
        }
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
    const headOffsets = (sim.getWellState() as Array<Record<string, number>>)
        .map((well) => Number(well.head_offset_bar ?? 0));
    sim.free();
    return { breakthroughPvi, recoveryAtOnePvi, warning, headOffsets };
}

describe('wf_gravity scenario definition', () => {
    it('perforates one layer per well so the completion dimension has something to move', () => {
        const scenario = getScenario('wf_gravity');

        expect(scenario?.params.gravityEnabled).toBe(true);
        // One perforation per well is what lets `completion_strategy` move the
        // producer from the base of the section to the top. (Multi-layer
        // completions are no longer barred under gravity — the engine references
        // each completion's BHP to the well's datum — but they would collapse
        // that dimension.)
        expect(scenario?.params.injectorKLayers).toEqual([19]);
        expect(scenario?.params.producerKLayers).toEqual([19]);
        // Isotropic permeability: the gravity-off control then differs from BL
        // only by discretisation, not by suppressed vertical flow.
        expect(scenario?.params.uniformPermZ).toBe(scenario?.params.uniformPermX);
    });

    it('leaves the analytical solution untouched in every variant', () => {
        const scenario = getScenario('wf_gravity');
        const analyticalInputs = new Set(['mu_w', 'mu_o', 's_wc', 's_or', 'n_w', 'n_o', 'k_rw_max', 'k_ro_max', 'initialSaturation']);

        for (const dimension of scenario?.sensitivities ?? []) {
            expect(dimension.analyticalOverlayMode, dimension.key).toBe('shared');
            for (const variant of dimension.variants) {
                expect(variant.affectsAnalytical, variant.key).toBe(false);
                const touched = Object.keys(variant.paramPatch).filter((name) => analyticalInputs.has(name));
                expect(touched, variant.key).toEqual([]);
            }
        }
    });
});

describe('wf_gravity OPM Flow cross-check', () => {
    it('declares the bundled artifact and keeps the deck aligned with the base case', () => {
        const scenario = getScenario('wf_gravity');

        expect(listDeclaredOpmFlowArtifactKeys(scenario?.referenceSources)).toEqual(['wf_gravity']);
        const series = resolveScenarioReferenceSeries(scenario?.referenceSources);
        expect(series.map((entry) => entry.curveKey)).toEqual([
            'opm-water-cut', 'opm-oil-rate', 'opm-cum-oil', 'opm-avg-pressure',
        ]);
        // The deck's schedule is 210 × 2 days; the artifact must cover the same
        // horizon or the two curves would be compared over different floods.
        const lastDay = series[0].data.at(-1)!.x;
        expect(lastDay).toBe(Number(scenario?.params.steps) * Number(scenario?.params.delta_t_days));
    });

    it('agrees with the OPM Flow run of the identical model', async () => {
        await ensureWasmReady();

        const scenario = getScenario('wf_gravity');
        const series = resolveScenarioReferenceSeries(scenario?.referenceSources);
        const cumOil = series.find((entry) => entry.curveKey === 'opm-cum-oil')!.data;
        const waterCut = series.find((entry) => entry.curveKey === 'opm-water-cut')!.data;

        const poreVolume = 30 * 10 * 1 * 20 * 20 * 2 * 0.2;
        const oilInPlace = poreVolume * 0.9;
        const rate = Number(scenario?.params.targetInjectorRate);
        const dayAtOnePvi = poreVolume / rate;
        const opmRecoveryAtOnePvi = cumOil.find((point) => point.x >= dayAtOnePvi)!.y / oilInPlace;
        const opmBreakthroughPvi = (waterCut.find((point) => point.y > 0.01)!.x * rate) / poreVolume;

        const ressim = runVariant('opm_cross_check', 'opm_step_2d');

        expect(ressim.warning).toBe('');
        // Measured: OPM 0.583 / 0.227, ResSim 0.585 / 0.253. Two independent
        // simulators reproducing the gravity tongue is the evidence that the
        // 18 % shortfall against Buckley-Leverett is physics, not an IMPES defect.
        expect(opmRecoveryAtOnePvi).toBeCloseTo(0.583, 2);
        expect(Math.abs(ressim.recoveryAtOnePvi - opmRecoveryAtOnePvi) / opmRecoveryAtOnePvi)
            .toBeLessThan(0.02);
        // Breakthrough timing is the more demanding comparison — measured 0.253
        // against OPM's 0.227, 12 % apart, where BL's 0.586 is 158 % late.
        expect(Math.abs(ressim.breakthroughPvi - opmBreakthroughPvi) / opmBreakthroughPvi)
            .toBeLessThan(0.15);
    }, 180_000);
});

describe('wf_gravity measured behaviour', () => {
    it('returns to Buckley-Leverett with gravity off and departs from it with gravity on', async () => {
        await ensureWasmReady();

        const welge = computeWelgeMetrics(ROCK, FLUID, 0.1);
        const blCurve = computeBLRecoveryVsPVI(ROCK, FLUID, 2, 400);
        const blRecoveryAtOnePvi = blCurve.find((point) => point.pvi >= 1)!.rf;
        expect(welge.breakthroughPvi).toBeCloseTo(0.5856, 3);
        expect(blRecoveryAtOnePvi).toBeCloseTo(0.715, 2);

        const control = runVariant('gravity_number', 'ng_off');
        const base = runVariant('gravity_number', 'ng_base');

        expect(control.warning).toBe('');
        expect(base.warning).toBe('');

        // The control is the claim that the departure below is physics: with
        // gravity off the section recovers within 3 % of the BL solution.
        expect(Math.abs(control.recoveryAtOnePvi - blRecoveryAtOnePvi) / blRecoveryAtOnePvi)
            .toBeLessThan(0.03);
        expect(control.breakthroughPvi).toBeGreaterThan(0.45);

        // Gravity on, everything else identical: the tongue costs ~0.11 of
        // recovery and halves the time to first water.
        expect(control.recoveryAtOnePvi - base.recoveryAtOnePvi).toBeGreaterThan(0.08);
        expect(base.breakthroughPvi).toBeLessThan(0.6 * control.breakthroughPvi);
    }, 180_000);

    it('loses more recovery as the gravity number rises', async () => {
        await ensureWasmReady();

        const viscous = runVariant('gravity_number', 'ng_viscous');
        const base = runVariant('gravity_number', 'ng_base');
        const gravityDominated = runVariant('gravity_number', 'ng_gravity');

        for (const [key, run] of Object.entries({ viscous, base, gravityDominated })) {
            expect(run.warning, key).toBe('');
        }

        // Monotone in N_g: 0.14 → 0.56 → 2.23 measured 0.689 → 0.585 → 0.380.
        expect(viscous.recoveryAtOnePvi).toBeGreaterThan(base.recoveryAtOnePvi);
        expect(base.recoveryAtOnePvi).toBeGreaterThan(gravityDominated.recoveryAtOnePvi);
        expect(viscous.recoveryAtOnePvi).toBeCloseTo(0.689, 1);
        expect(gravityDominated.recoveryAtOnePvi).toBeCloseTo(0.380, 1);

        // The scenario text claims BL over-predicts by ~87 % at the slow rung.
        const blCurve = computeBLRecoveryVsPVI(ROCK, FLUID, 2, 400);
        const blRecoveryAtOnePvi = blCurve.find((point) => point.pvi >= 1)!.rf;
        expect(blRecoveryAtOnePvi / gravityDominated.recoveryAtOnePvi).toBeGreaterThan(1.7);
    }, 180_000);

    it('attributes the effect to the density difference, not to gravity being enabled', async () => {
        await ensureWasmReady();

        const equalDensity = runVariant('density_contrast', 'drho_zero');
        const strongContrast = runVariant('density_contrast', 'drho_strong');
        const control = runVariant('gravity_number', 'ng_off');

        expect(equalDensity.warning).toBe('');
        // Gravity enabled but Δρ = 0 reproduces the gravity-off control.
        // Measured 0.704 against the control's 0.699 — a 0.7 % gap that comes
        // from the hydrostatic pressure field, not from any displacement effect.
        expect(Math.abs(equalDensity.recoveryAtOnePvi - control.recoveryAtOnePvi) / control.recoveryAtOnePvi)
            .toBeLessThan(0.02);
        expect(strongContrast.recoveryAtOnePvi).toBeLessThan(equalDensity.recoveryAtOnePvi - 0.15);
    }, 180_000);

    it('flips the sign of the gravity penalty when the producer is completed at the top', async () => {
        await ensureWasmReady();

        const bottomGravity = runVariant('completion_strategy', 'comp_bottom_gravity');
        const topGravity = runVariant('completion_strategy', 'comp_top_gravity');
        const topNoGravity = runVariant('completion_strategy', 'comp_top_nogravity');

        for (const [key, run] of Object.entries({ bottomGravity, topGravity, topNoGravity })) {
            expect(run.warning, key).toBe('');
        }

        // With the producer at the base gravity is a loss; move it to the top of
        // the same section and the identical buoyancy becomes a gain.
        expect(topGravity.recoveryAtOnePvi).toBeGreaterThan(topNoGravity.recoveryAtOnePvi);
        expect(topGravity.recoveryAtOnePvi - bottomGravity.recoveryAtOnePvi).toBeGreaterThan(0.1);
    }, 180_000);

    it('references a fully perforated well\'s BHP to its datum instead of biasing by depth', async () => {
        await ensureWasmReady();

        const scenario = getScenario('wf_gravity');
        const nz = Number(scenario?.params.nz);
        const cellDz = Number(scenario?.params.cellDz);
        const allLayers = Array.from({ length: nz }, (_, k) => k);
        // Short horizon: what this test measures is the head profile the wasm
        // API produces, not a recovery number. Note that this configuration
        // still trips IMPES's pressure recovery at t≈0 — that failure is
        // reproducible on the pre-datum engine and is tracked separately in
        // TODO.md, so the warning is deliberately not asserted here.
        const fullyPerforatedParams = { steps: 20 };

        const fullyPerforated = runVariant('gravity_number', 'ng_base', {
            injectorKLayers: allLayers,
            producerKLayers: allLayers,
            params: fullyPerforatedParams,
        });

        // Default datum is the shallowest completion, so the head runs from zero
        // at the top of the section to ρ·g·H at its base. With water standing in
        // the wellbore that is ~1000 × 9.80665 × 38 m ≈ 3.7 bar.
        const injectorHeads = fullyPerforated.headOffsets.slice(0, nz);
        expect(injectorHeads[0]).toBeCloseTo(0, 10);
        for (let k = 1; k < nz; k += 1) {
            expect(injectorHeads[k]).toBeGreaterThan(injectorHeads[k - 1]);
        }
        const sectionHeight = (nz - 1) * cellDz;
        expect(injectorHeads[nz - 1]).toBeGreaterThan(0.9e-5 * 1000 * 9.80665 * sectionHeight);
        expect(injectorHeads[nz - 1]).toBeLessThan(1.1e-5 * 1000 * 9.80665 * sectionHeight);

        // Quoting the same well at the base of the section shifts every head by
        // one constant, leaving the depth *differences* untouched. The two runs
        // are not the same flood — the same BHP number now means a deeper
        // pressure — so the derived column densities drift apart by a few ppm.
        const datumAtBase = runVariant('gravity_number', 'ng_base', {
            injectorKLayers: allLayers,
            producerKLayers: allLayers,
            datumDepth: (nz - 0.5) * cellDz,
            params: fullyPerforatedParams,
        });
        const shiftedHeads = datumAtBase.headOffsets.slice(0, nz);
        for (let k = 1; k < nz; k += 1) {
            expect(shiftedHeads[k] - shiftedHeads[k - 1])
                .toBeCloseTo(injectorHeads[k] - injectorHeads[k - 1], 3);
        }
        expect(shiftedHeads[0]).toBeLessThan(0);

        // Gravity off leaves every completion on the well's single BHP, exactly
        // as before the datum existed.
        const noGravity = runVariant('gravity_number', 'ng_base', {
            injectorKLayers: allLayers,
            producerKLayers: allLayers,
            params: { ...fullyPerforatedParams, gravityEnabled: false },
        });
        expect(noGravity.headOffsets.every((head) => head === 0)).toBe(true);
    }, 240_000);

    it('needs vertical communication before gravity can segregate the fluids', async () => {
        await ensureWasmReady();

        const tightGravity = runVariant('vertical_communication', 'kv_tight_gravity');
        const tightNoGravity = runVariant('vertical_communication', 'kv_tight_nogravity');
        const isoGravity = runVariant('vertical_communication', 'kv_iso_gravity');
        const isoNoGravity = runVariant('vertical_communication', 'kv_iso_nogravity');

        const tightPenalty = tightNoGravity.recoveryAtOnePvi - tightGravity.recoveryAtOnePvi;
        const isoPenalty = isoNoGravity.recoveryAtOnePvi - isoGravity.recoveryAtOnePvi;

        // Measured 0.033 against 0.114: suppressing k_z suppresses the tongue.
        expect(tightPenalty).toBeGreaterThan(0);
        expect(tightPenalty).toBeLessThan(0.5 * isoPenalty);
        // …while the low-k_z pair sits far below BL for a reason that is not gravity.
        expect(tightNoGravity.recoveryAtOnePvi).toBeLessThan(isoNoGravity.recoveryAtOnePvi - 0.2);
    }, 240_000);
});
