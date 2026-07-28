import type { Scenario } from '../scenarios';

/**
 * Pressure-transient test — constant-rate drawdown in an infinite-acting
 * radial system.
 *
 * The classical well test, and the first scenario in the catalog whose
 * question is "what can you *measure*?" rather than "how much oil do you
 * get?". A well is produced at a fixed rate from a reservoir at rest; the
 * flowing bottomhole pressure falls along a straight line on a semilog plot,
 * and the slope of that line gives permeability while its one-hour intercept
 * gives skin. Everything a simulator does near a well — the Peaceman well
 * index, the grid resolution around the completion, the timestep — shows up
 * here as an error in a number an engineer would report.
 *
 * Analytical reference: the line-source (Theis) solution, `analytical/wellTest.ts`.
 * The overlay is the full exponential-integral form rather than the semilog
 * approximation, so it stays correct at the earliest times on the log axis
 * where the two differ. The module also exposes `semilogValidFromTime`, which
 * for this scenario's properties is ~1e-6 days — the whole test is inside the
 * infinite-acting radial period, by design.
 *
 * Construction
 * ------------
 * A single producer at the centre of an 820 m x 820 m single-layer square, on
 * rate control at 100 m3/day of reservoir voidage. No injector: a well test is
 * a measurement of the reservoir's response to one well, and anything else
 * flowing would superpose onto the transient being interpreted.
 *
 * Two bounds set the run length, and both are checked:
 *   - The radius of investigation, 2.sqrt(eta.t), must stay inside the
 *     no-flow boundary or the response stops being infinite-acting and the
 *     semilog line bends. With eta ~= 44,400 m2/day, r_inv reaches ~298 m at
 *     0.5 days against a 410 m half-domain, so the 12-hour test stays radial.
 *   - The timestep has to resolve the early-time decade: at dt = 0.005 days
 *     the test spans two full log cycles, which is what a slope needs.
 *
 * The base permeability is deliberately low (10 mD). At the catalog's more
 * typical few-hundred mD the diffusivity is so high that the pressure front
 * reaches the boundary in minutes and there is no infinite-acting period left
 * to analyse.
 *
 * Rate control is not optional here. `computeWellTestOnTimeAxis` returns null
 * for a BHP-controlled case rather than inventing a rate, so a variant that
 * switched to pressure control would silently lose its overlay.
 *
 * What the interpretation actually returns (measured 2026-07-24)
 * ------------------------------------------------------------
 * `dep_welltest.test.ts` runs the simulated drawdown through the same analysis
 * a user would apply — fit the semilog line, take k from the slope and s from
 * the one-hour intercept — and the round trip does not come back exact:
 *
 *   grid          40 m       20 m       10 m      (true k = 10 mD, s = 0)
 *   recovered k   8.674      9.118      9.209  mD
 *   recovered s  -0.997     -0.665     -0.597
 *
 * So the simulator, read as an instrument, reports permeability ~9 % low on
 * the base grid and a slightly stimulated well that is not stimulated. The
 * bias shrinks monotonically with refinement but is still there at 10 m cells.
 * Across the skin ladder the recovered permeability is *identical to three
 * decimals* for s = -2, 0 and +5, which is the slope/offset separation the
 * scenario claims, holding exactly.
 *
 * This is a property of the case worth showing rather than tuning away: it is
 * what a coarse Cartesian grid plus a Peaceman well index does to a number an
 * engineer would put in a report.
 *
 * Engine dependency: this scenario needs `Well::flowing_bhp`, added 2026-07-24.
 * The pre-existing `bhp` field is the well's configured target-or-limit and
 * does not move under rate control, so before that change the whole case
 * plotted a flat line.
 *
 * References: Theis (1935) Trans. AGU 16; Horner (1951) 3rd World Pet.
 * Congress; Matthews & Russell (1967) SPE Monograph 1; Earlougher (1977) SPE
 * Monograph 5; Peaceman (1978) SPEJ 18(3) for the well-index model this case
 * exercises; Dake (1978) ch. 7.
 */
export const dep_welltest: Scenario = {
    key: 'dep_welltest',
    label: 'Well-Test Drawdown',
    catalog: {
        group: 'pressure-transient',
        role: 'interpretation',
        caseMode: 'dep',
        parameterSummary: '12-hour constant-rate drawdown · flowing BHP on log time · permeability and skin interpretation',
    },
    description: 'A single well produced at constant rate into a reservoir at rest, which is how permeability and skin are actually measured in the field. On the log-time axis the flowing bottomhole pressure falls along a straight line whose slope depends only on k, h, viscosity and rate — not on anything about the grid — so any gap between the simulated and analytical curves is the simulator\'s near-well model rather than the physics. The skin axis shows the constant pressure offset a damaged or stimulated completion adds; the permeability axis changes the slope itself; and the grid axis asks the question the other two set up — does the Peaceman well index recover the right transient when the well sits in a 40 m cell?',
    analyticalMethodSummary: 'Line-source (Theis) solution for infinite-acting radial flow, evaluated at the wellbore with the thin-skin pressure drop. Valid only while the radius of investigation stays inside the no-flow boundary — about 0.5 days for these properties, which is why the test is short. The full exponential-integral form is plotted, not the semilog approximation.',
    analyticalMethodReference: 'Theis (1935), Trans. AGU 16; Horner (1951); Matthews and Russell (1967), SPE Monograph 1; Earlougher (1977), SPE Monograph 5; Peaceman (1978), SPEJ 18(3).',
    chartLayoutKey: 'well_test',
    defaultSensitivityDimensionKey: 'skin',
    capabilities: {
        analyticalMethod: 'well-test',
        hasInjector: false,
        // No displacement front to look at — the 3D view has nothing to add
        // that the pressure transient does not say better.
        default3DScalar: null,
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES is the interactive default for this short constant-rate transient; the solver axis exposes near-well formulation differences against one line-source reference.',
        comparisonSensitivityAvailable: true,
    },
    params: {
        // Fluid
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
        // Rock / relative permeability. Water is immobile at connate, so this
        // is single-phase oil flow — the condition the line-source solution
        // assumes.
        reservoirPorosity: 0.2,
        s_wc: 0.2,
        s_or: 0.1,
        n_w: 2,
        n_o: 2,
        k_rw_max: 1,
        k_ro_max: 1,
        capillaryEnabled: false,
        capillaryPEntry: 0,
        capillaryLambda: 2,
        // Grid: 41 x 41 x 1, 820 m x 820 m x 20 m, producer at the centre so
        // the four no-flow boundaries are equidistant.
        nx: 41,
        ny: 41,
        nz: 1,
        cellDx: 20,
        cellDy: 20,
        cellDz: 20,
        permMode: 'uniform',
        uniformPermX: 10,
        uniformPermY: 10,
        uniformPermZ: 10,
        // Initial conditions — reservoir at rest at 300 bar, water at connate
        initialPressure: 300,
        initialSaturation: 0.2,
        // Wells: one rate-controlled producer, no injector. The BHP floor is
        // set well below the expected flowing pressure (~228 bar at the end of
        // the test) so the rate target is never clipped — a well that hits its
        // BHP limit is no longer a constant-rate test and the analysis breaks.
        injectorEnabled: false,
        injectorControlMode: 'pressure',
        producerControlMode: 'rate',
        injectorBhp: 500,
        producerBhp: 50,
        targetInjectorRate: 0,
        targetProducerRate: 100,
        injectorI: 0,
        injectorJ: 0,
        producerI: 20,
        producerJ: 20,
        well_radius: 0.1,
        well_skin: 0,
        // Numerics — 100 x 0.005 d = 0.5 days (12 hours), two log cycles.
        fimEnabled: false,
        delta_t_days: 0.005,
        steps: 100,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 1.0,
        gravityEnabled: false,
    },
    sensitivities: [
        {
            key: 'skin',
            label: 'Skin Factor  s',
            description: 'Skin is a pressure drop concentrated at the wellbore, so it moves the whole semilog line up or down by 2.s times the pressure group without changing its slope. That separation is the reason a well test can report permeability and completion damage as two independent numbers from one measurement: the slope carries k, the offset carries s. Each variant gets its own analytical curve, so a mismatch is the simulator, not the reference.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'skin_stimulated',
                    label: 's = -2  (stimulated)',
                    description: 'An acidised or fractured completion flows at a higher bottomhole pressure than an undamaged one at the same rate.',
                    paramPatch: { well_skin: -2 },
                    affectsAnalytical: true,
                },
                {
                    key: 'skin_clean',
                    label: 's = 0  (clean, base)',
                    description: 'No completion damage. The straight line sits exactly where permeability alone puts it.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'skin_damaged',
                    label: 's = +5  (damaged)',
                    description: 'Drilling or completion damage. The extra drawdown is large and constant — parallel to the clean line, not steeper. Reading it as low permeability instead of skin is the classic misinterpretation this axis exists to prevent.',
                    paramPatch: { well_skin: 5 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'permeability',
            label: 'Permeability  k',
            description: 'Permeability sets the slope of the semilog line: m = ln(10).q.mu/(4.pi.C.k.h), so halving k doubles the slope. Unlike skin, it changes the shape of the response rather than offsetting it — which is exactly why the two can be told apart. Note that the diffusivity also scales with k, so the higher-permeability variants push the radius of investigation towards the no-flow boundary sooner; at 40 mD the late part of the test is approaching the limit of the infinite-acting assumption.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'perm_tight',
                    label: 'k = 5 mD  (tight)',
                    description: 'Twice the slope of the base case, and the transient stays radial for the whole test.',
                    paramPatch: { uniformPermX: 5, uniformPermY: 5, uniformPermZ: 5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'perm_base',
                    label: 'k = 10 mD  (base)',
                    description: 'The base case. Radius of investigation reaches roughly 300 m by the end of the 12-hour test, comfortably inside the 410 m half-domain.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'perm_good',
                    label: 'k = 40 mD  (good)',
                    description: 'A quarter of the base slope, but four times the diffusivity: the pressure front reaches the boundary within the test window, so watch the late-time data bend away from the straight line. That bend is boundary-dominated flow, not a bad simulation.',
                    paramPatch: { uniformPermX: 40, uniformPermY: 40, uniformPermZ: 40 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'near_well_grid',
            label: 'Near-Well Grid Resolution',
            description: 'The question the other two axes set up. The analytical solution knows about a 0.1 m wellbore; the coarse grid represents that well as a source term in a 40 m block, and the Peaceman well index is what reconciles the two. The reference curve is identical for all three variants — permeability, thickness, rate and skin are unchanged — so any spread here is purely the near-well numerics. The domain is held at ~820 m and the producer stays at the grid centre in every variant. Measured on this build: interpreting the simulated drawdown returns 8.67, 9.12 and 9.21 mD at 40, 20 and 10 m cells against a true 10 mD, and an apparent skin of -1.00, -0.67 and -0.60 against a true zero. The bias converges the right way but is still ~8 % at 10 m — worth knowing before trusting a simulated pressure transient to the second digit.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'grid_coarse',
                    label: '21 x 21  (40 m cells)',
                    description: '840 m domain in 40 m blocks — the well lives in a block 400 times wider than its own diameter.',
                    paramPatch: { nx: 21, ny: 21, cellDx: 40, cellDy: 40, producerI: 10, producerJ: 10 },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_base',
                    label: '41 x 41  (20 m cells, base)',
                    description: 'The base grid.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_fine',
                    label: '81 x 81  (10 m cells)',
                    description: 'Four times as many cells for the same domain. If the Peaceman index is doing its job, this should not move the flowing-pressure curve much — that is the result worth checking, not assuming.',
                    paramPatch: { nx: 81, ny: 81, cellDx: 10, cellDy: 10, producerI: 40, producerJ: 40 },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
