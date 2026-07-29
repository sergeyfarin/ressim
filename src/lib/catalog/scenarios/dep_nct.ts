import { depletionLivePanels } from '../chartPanels/depletionLivePanels';
import type { Scenario } from '../scenarios';
import { depletionDef } from '../analyticalAdapters';

/**
 * "Matched history, different reserves" — the N·c_t ambiguity.
 *
 * Physics basis: the exact lumped-tank decline time constant is
 * τ = V_p·c_t / PI. V_p = length·area·porosity scales
 * with OOIP; c_t = S_o·c_o + S_w·c_w + c_rock is total system compressibility.
 * Holding PI fixed (same permeability/geometry/skin) and scaling porosity
 * and c_o inversely so that porosity·c_t stays constant reproduces the same
 * τ — and therefore bit-identical pressure, rate, and cumulative-oil
 * history — while OOIP (∝ porosity) differs by up to 4x across variants.
 * Recovery factor RF = Np/OOIP therefore differs by the same 4x even though
 * every curve a history match would be judged against is unchanged.
 *
 * This is the classic material-balance non-uniqueness: pressure/rate data
 * alone cannot separate OOIP from total compressibility (Dake 1978, ch. 3;
 * Havlena & Odeh 1963). It is the depletion-scenario analog of Tavassoli,
 * Carter & King (SPE 86883, 2004) "Errors in History Matching" — a good
 * history match does not imply correct reserves.
 *
 * c_o values below are solved exactly (not rounded) so that
 * porosity·(0.9·c_o + 0.1·c_w + c_rock) is held to the base-case constant
 * 2.06e-6; see the regression test in dep_nct.test.ts for the derivation
 * and the verified bit-identical production curves.
 */
export const dep_nct: Scenario = {
    key: 'dep_nct',
    label: 'Same History, Different OOIP (N·cₜ)',
    catalog: {
        group: 'depletion-decline',
        role: 'interpretation',
        caseMode: 'dep',
        parameterSummary: 'Matched pressure and rate response · 4× OOIP range · storage / reserves non-uniqueness',
    },
    description: 'Three exact lumped tanks have the same permeability, geometry, pressure/rate history and cumulative oil, but 4x different oil in place. Porosity (∝ OOIP) and oil compressibility are scaled inversely to hold PV·c_t/PI fixed. Recovery factor therefore ranges from about 0.8% to 3.1%. Pressure and rate history alone cannot distinguish OOIP from compressibility — the classic material-balance non-uniqueness (Dake 1978).',
    analyticalMethodSummary: 'Exact lumped-tank material balance with the simulator\'s Peaceman well connection. Holding PV·c_t and PI fixed makes pressure and rate histories non-unique in OOIP.',
    analyticalMethodReference: 'Dake, L.P. (1978) "Fundamentals of Reservoir Engineering", ch. 3; Havlena, D. & Odeh, A.S. (1963) JPT 15(8); Tavassoli, Carter & King, SPE 86883 (2004).',
    chartLayoutKey: 'fetkovich',
    defaultSensitivityDimensionKey: 'nct_ambiguity',
    // Divide the 24-day run (960 × 0.025 d) at its midpoint: up to day 12 reads
    // as "observed history" that all three variants reproduce identically;
    // beyond it, the same matched history extrapolates to 4x-different remaining
    // reserves — the material-balance non-uniqueness this case exists to show.
    historyWindow: { boundary: 12, axis: 'time', historyLabel: 'Matched history', forecastLabel: 'Reserves view' },
    capabilities: {
        analyticalMethod: 'depletion',
        hasInjector: false,
        default3DScalar: 'pressure',
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES is the default for this deliberately matched oil-depletion response.',
    },
    params: {
        // Fluid — exact single-tank depletion contract
        mu_w: 0.5,
        mu_o: 1.0,
        c_o: 1e-5,
        c_w: 3e-6,
        rock_compressibility: 1e-6,
        depth_reference: 0,
        volume_expansion_o: 1,
        volume_expansion_w: 1,
        rho_w: 1000,
        rho_o: 800,
        // Rock / rel perm
        reservoirPorosity: 0.20,
        s_wc: 0.1,
        s_or: 0.1,
        n_w: 2,
        n_o: 2,
        k_rw_max: 1,
        k_ro_max: 1,
        capillaryEnabled: false,
        capillaryPEntry: 0,
        capillaryLambda: 2,
        // One areally large cell: no unresolved within-reservoir gradient, so
        // simulator and analytical model share the same tank storage and well PI.
        nx: 1,
        ny: 1,
        nz: 1,
        cellDx: 1000,
        cellDy: 1000,
        cellDz: 10,
        permMode: 'uniform',
        uniformPermX: 20,
        uniformPermY: 20,
        uniformPermZ: 2,
        // Initial conditions: high pressure reservoir depleting to low BHP
        initialPressure: 1500,
        initialSaturation: 0.1,
        // Wells: single producer, no injector
        injectorEnabled: false,
        injectorControlMode: 'pressure',
        producerControlMode: 'pressure',
        injectorBhp: 500,
        producerBhp: 50,
        targetInjectorRate: 0,
        targetProducerRate: 0,
        injectorI: 0,
        injectorJ: 0,
        producerI: 0,
        producerJ: 0,
        well_radius: 0.1,
        well_skin: 0,
        analyticalDepletionStartDays: 0,
        analyticalDepletionModel: 'tank',
        // Numerics
        fimEnabled: false,
        delta_t_days: 0.025,
        steps: 960,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    analyticalDef: depletionDef,
    liveChartPanels: depletionLivePanels,
    sensitivities: [
        {
            key: 'nct_ambiguity',
            label: 'OOIP vs Compressibility (N·c_t)',
            description: 'Porosity (proportional to OOIP) and oil compressibility are scaled inversely so the PSS decline time constant τ = V_p·c_t/PI — and therefore the entire pressure/rate/cumulative-oil history — stays the same. Only the recovery-factor ceiling changes, because OOIP itself differs by up to 4x. Watch the Rates, Cumulative, and Diagnostics panels stay identical while Recovery Factor moves. The analytical reference is intentionally shared: by construction it does not change across variants — that unperturbed sameness is the whole teaching point.',
            // affectsAnalytical is false for the non-base variants below despite
            // patching c_o/reservoirPorosity: those inputs ARE consumed by
            // depletionDef, but the patch is solved so the analytical curve is
            // unchanged (verified in dep_nct.test.ts). Per the add-scenario skill
            // contract, affectsAnalytical means "the rendered curve changes," not
            // "an input field differs" — marking these true would fail the
            // affectsAnalytical contract test's intent (a true-flagged variant
            // must visibly perturb the curve) for a case where non-perturbation
            // is deliberate, not a bug.
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'nct_small_reservoir',
                    label: 'φ = 0.10, c_o = 21.4e-6/bar  (small N, high c_t)',
                    description: 'Small OOIP with high compressibility — same history, but every barrel produced is a much larger fraction of a smaller tank. RF ≈ 3.1%.',
                    paramPatch: { reservoirPorosity: 0.10, c_o: 2.1444444444444443e-5 },
                    affectsAnalytical: false,
                },
                {
                    key: 'nct_base',
                    label: 'φ = 0.20, c_o = 10e-6/bar  (base)',
                    description: 'Base exact-tank case.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'nct_large_reservoir',
                    label: 'φ = 0.40, c_o = 4.3e-6/bar  (large N, low c_t)',
                    description: 'Large OOIP with low compressibility — same history, but every barrel produced is a much smaller fraction of a bigger tank. RF ≈ 0.8%.',
                    paramPatch: { reservoirPorosity: 0.40, c_o: 4.277777777777778e-6 },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
