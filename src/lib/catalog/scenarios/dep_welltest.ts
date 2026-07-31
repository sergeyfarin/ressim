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
 * index and the timestep — shows up here as an error in a number an engineer
 * would report.
 *
 * Analytical reference: the line-source (Theis) solution, `analytical/wellTest.ts`.
 * The overlay is the full exponential-integral form rather than the semilog
 * approximation, so it stays correct at the earliest times on the log axis
 * where the two differ. The module also exposes `semilogValidFromTime`, which
 * for this scenario's properties is ~1e-6 days. The early part of the test is
 * interpreted in the infinite-acting radial period; the longer tail is kept
 * deliberately so the no-flow box boundary becomes visible.
 *
 * Construction
 * ------------
 * A single producer at the centre of an 820 m x 820 m single-layer square, on
 * rate control at 100 m3/day of reservoir voidage. No injector: a well test is
 * a measurement of the reservoir's response to one well, and anything else
 * flowing would superpose onto the transient being interpreted.
 *
 * Two time scales shape the run:
 *   - The radius of investigation, 2.sqrt(eta.t), must stay inside the
 *     no-flow boundary or the response stops being infinite-acting and the
 *     semilog line bends. With eta ~= 44,400 m2/day, r_inv reaches ~298 m at
 *     0.5 days against a 410 m half-domain. The run continues to 2.4 days so
 *     that departure from the infinite-reservoir reference is visible.
 *   - The 0.02-day report timestep still resolves the early semilog window;
 *     interpretation tests discard startup points and stop before boundary
 *     arrival.
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
    label: 'Transient Radial Flow (Theis)',
    catalog: {
        group: 'flow-regimes-decline',
        role: 'interpretation',
        caseMode: 'dep',
        parameterSummary: '2.4-day constant-rate drawdown · radial-flow interpretation followed by box-boundary arrival',
    },
    description: 'A single well produced at constant rate into a reservoir at rest, which is how permeability and skin are measured in the field. On the log-time axis the early flowing bottomhole pressure follows a straight line whose slope depends on k, h, viscosity and rate. The skin axis moves that line by a constant pressure offset, while the permeability axis changes its slope. The numerical run intentionally continues beyond the infinite-acting period: when the pressure disturbance reaches the closed square boundary, numerical BHP falls below the infinite-reservoir reference and exposes boundary-dominated flow.',
    analyticalMethodSummary: 'Line-source (Theis) solution for infinite-acting radial flow, evaluated at the wellbore with the thin-skin pressure drop. It is the interpretation reference before the radius of investigation reaches the no-flow boundary; the 2.4-day numerical tail deliberately exceeds that validity window to show boundary arrival. The full exponential-integral form is plotted, not the semilog approximation.',
    analyticalMethodReference: 'Theis (1935), Trans. AGU 16; Horner (1951); Matthews and Russell (1967), SPE Monograph 1; Earlougher (1977), SPE Monograph 5; Peaceman (1978), SPEJ 18(3).',
    chartLayoutKey: 'well_test',
    defaultSensitivityDimensionKey: 'skin',
    capabilities: {
        analyticalMethod: 'well-test',
        hasInjector: false,
        // Skin is the default sensitivity, and under rate control it changes
        // flowing BHP without changing the reservoir pressure field. A default
        // pressure view would therefore show three effectively identical
        // spatial results and imply that the sensitivity has no effect.
        default3DScalar: null,
        spatialProfile: { defaultAxis: 'i', wellPathLabel: 'Diagonal' },
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES resolves the pressure equation implicitly and reports rate-controlled BHP from the accepted end-of-step pressure.',
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
        // set well below the expected flowing pressure throughout the test so
        // the rate target is never clipped — a well that hits its
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
        // Numerics — 120 x 0.02 d = 2.4 days. The early samples cover the
        // radial-flow interpretation window and the tail shows boundary arrival.
        fimEnabled: false,
        delta_t_days: 0.02,
        steps: 120,
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
            description: 'Permeability sets the slope of the semilog line: m = ln(10).q.mu/(4.pi.C.k.h), so halving k doubles the slope. Unlike skin, it changes the shape of the response rather than offsetting it. Diffusivity also scales with k, so higher permeability reaches the no-flow boundary earlier and bends away from the infinite-reservoir reference sooner.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'perm_tight',
                    label: 'k = 5 mD  (tight)',
                    description: 'Twice the early radial-flow slope of the base case, with later boundary arrival than the other variants.',
                    paramPatch: { uniformPermX: 5, uniformPermY: 5, uniformPermZ: 5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'perm_base',
                    label: 'k = 10 mD  (base)',
                    description: 'The base case. Its early radial-flow interval is followed by a visible closed-boundary departure.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'perm_good',
                    label: 'k = 40 mD  (good)',
                    description: 'A quarter of the base slope but four times the diffusivity, so the pressure front reaches the boundary earliest. The late bend is boundary-dominated flow, not a bad simulation.',
                    paramPatch: { uniformPermX: 40, uniformPermY: 40, uniformPermZ: 40 },
                    affectsAnalytical: true,
                },
            ],
        },
    ],
};
