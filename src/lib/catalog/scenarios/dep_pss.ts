import type { Scenario } from '../scenarios';

/**
 * Dietz shape factor — how drainage geometry sets pseudo-steady productivity.
 *
 * A constant-rate producer in a closed reservoir. Once the pressure
 * disturbance has reached every boundary the whole reservoir depletes in
 * lockstep: dp/dt is uniform, and the gap between average and flowing
 * pressure stops moving. That constant gap is the case's subject.
 *
 *   p_bar - p_wf = q / J,   J = 2.pi.C.k.h.lambda / [ 0.5 ln(4A / (gamma.C_A.r_w^2)) + s ]
 *
 * Everything in that expression except C_A is a property of the rock, the
 * fluid or the completion. C_A is the *shape* term: it carries where the
 * boundaries are and where the well sits inside them. Hold area, rate,
 * permeability and skin fixed, change only the shape of the drainage volume,
 * and the drawdown changes — that is the exhibit.
 *
 * Why the geometry sweep is the primary dimension
 * -----------------------------------------------
 * A single centred square makes the shape factor an input, not a result: the
 * analytical curve is a horizontal line at the constant the reference was
 * handed, and the productivity index is constant in time by construction, so
 * nothing on the chart moves. The four shipped geometries all enclose the
 * same 176,400 m2 and produce at the same rate, so the only thing separating
 * them is C_A, which spans 30.88 down to 4.51 — a factor of 6.8.
 *
 * That factor compresses hard through the logarithm: a 6.8x change in C_A is
 * only a ~13% change in productivity. This is the honest lesson of the Dietz
 * equation and the reason the inferred-C_A panel is kept alongside the
 * productivity panel rather than instead of it. The C_A panel is the
 * *sensitive* view — it exponentially amplifies a small productivity error —
 * while the drawdown panel is the *physical* one.
 *
 * Geometries (all 176,400 m2, all single-layer, all 40 m3/day):
 *
 *   grid      cell        well        A-ratio   C_A tabulated   C_A measured
 *   21 x 21   20 x 20     centred      1:1        30.8828          31.14
 *   27 x 11   22 x 27     centred      2:1        21.8369          22.02
 *   35 x  7   24 x 30     centred      4:1         5.379             5.41
 *   22 x 22   19.09 sq.   quadrant     1:1         4.5132            4.55
 *
 * Measured values are from constant-rate runs on this scenario's own
 * parameters, sampled at the end of the constant-rate period. Every geometry
 * recovers its tabulated C_A within 1% and its tabulated productivity index
 * within 0.1%. The small residual bias is positive in every case: it is
 * well-block discretisation error amplified by the inversion, which is what
 * the grid-refinement dimension isolates.
 *
 * The quadrant well sits on a 22 x 22 grid rather than the base 21 x 21 so
 * that a cell centre falls exactly on the quarter point (5.5/22 = 0.25).
 * Position accuracy matters more than it looks: placing the same well on the
 * 21 x 21 grid puts it 4.8% off the quarter point, which is a 29% error in
 * inferred C_A.
 *
 * What actually moves in time
 * ---------------------------
 * Two things, and both are plotted:
 *
 *   - The drawdown rises from zero to its Dietz asymptote. This is the
 *     approach to pseudo-steady state, and it is the only transient in the
 *     case. It is also geometry-dependent: measured at 0.25 days for the
 *     square, 0.40 for the 2:1, and 0.85 for both the 4:1 and the
 *     quadrant-well square. Elongated drainage and off-centre wells take
 *     longer to feel all their boundaries. The panel therefore plots from
 *     t = 0 while the productivity and C_A panels stay blank until
 *     `analyticalPssStartDays`, because a productivity index computed during
 *     the transient is not a pseudo-steady quantity.
 *
 *   - Average pressure falls linearly by material balance, q.t/(PV.c_t), and
 *     flowing pressure falls parallel to it.
 *
 * End of the constant-rate period
 * -------------------------------
 * The run deliberately outlives its own premise. Average pressure declines
 * until p_wf reaches the producer's 50 bar floor at about day 19 — sooner with
 * a damaged completion, later with a stimulated one — after which
 * the well is BHP-limited and no longer producing the imposed rate. The
 * analytical reference stops exactly there rather than extrapolating a
 * constant-rate solution into a regime it cannot describe, so the end of the
 * period is a visible, predicted event instead of a silent divergence. What
 * happens after it — boundary-dominated decline at constant BHP — is
 * `dep_decline`'s subject, not this one's.
 *
 * Scope
 * -----
 * Dietz is a pseudo-steady productivity statement, not a decline law. The
 * infinite-acting period before boundaries are felt is `dep_welltest`; the
 * transient-to-boundary-dominated transition at constant BHP is
 * `dep_decline`; layered superposition is `dep_arps`.
 *
 * References: Dietz (1965), Determination of Average Reservoir Pressure from
 * Build-Up Surveys, JPT 17(8); Earlougher (1977), Advances in Well Test
 * Analysis, SPE Monograph 5, Table C.1; Dake (1978), Fundamentals of
 * Reservoir Engineering, ch. 7; Peaceman (1978), SPEJ 18(3) for the well
 * index whose error the refinement dimension exposes.
 */
export const dep_pss: Scenario = {
    key: 'dep_pss',
    label: 'Dietz Shape Factor — PSS Productivity',
    catalog: {
        group: 'depletion-decline',
        role: 'benchmark',
        caseMode: 'dep',
        parameterSummary: 'Equal-area closed geometries · constant-rate producer · C_A from measured drawdown',
    },
    description: 'A constant-rate producer in a closed reservoir, run across four drainage geometries of identical area: a centred square, 2:1 and 4:1 rectangles, and a square with the well at a quadrant centre. Once boundaries are felt, the steady gap p̄−p_wf gives the numerical productivity J=q/(p̄−p_wf), and inverting the Dietz equation recovers an effective shape factor to compare with the tabulated C_A of 30.8828, 21.8369, 5.379 and 4.5132. The drawdown panel plots from t=0 so the approach to pseudo-steady state — later for elongated and off-centre geometries — is visible; the productivity and C_A panels stay blank through the transient, where they have no meaning. The run continues until the producer reaches its BHP floor, and the analytical curves stop there rather than outliving the constant-rate premise.',
    analyticalMethodSummary: 'Dietz pseudo-steady-state productivity and its inverse: average pressure declines by material balance, p_wf=p̄−q/J_Dietz with J_Dietz from the tabulated C_A for each geometry, and the numerical J recovers an effective C_A for comparison. Curves terminate at the analytically predicted end of the constant-rate period.',
    analyticalMethodReference: 'Dietz (1965), JPT 17(8); Earlougher (1977), Advances in Well Test Analysis, SPE Monograph 5, Table C.1; Dake (1978), Fundamentals of Reservoir Engineering, ch. 7; Peaceman (1978), SPEJ 18(3).',
    chartLayoutKey: 'well_test',
    chartLayoutPatch: {
        rateChart: {
            xAxisMode: 'logTime',
            xAxisOptions: ['logTime', 'time'],
            panelOrder: [
                'pss_drawdown', 'pss_shape_factor', 'pss_productivity',
                'producer_bhp', 'diagnostics', 'oil_rate', 'control_limits',
            ],
            panels: {
                pss_drawdown: {
                    title: 'Drawdown p̄ − p_wf — Approach to PSS',
                    curveKeys: ['pss-drawdown-sim', 'pss-drawdown-reference'],
                    scalePreset: 'pressure',
                    visible: true,
                    expanded: true,
                },
                pss_shape_factor: {
                    title: 'Inferred Dietz Shape Factor C_A',
                    curveKeys: ['pss-shape-factor-sim', 'pss-shape-factor-reference'],
                    scalePreset: 'shape_factor',
                    visible: true,
                    expanded: true,
                },
                pss_productivity: {
                    title: 'PSS Productivity Index',
                    curveKeys: ['pss-productivity-sim', 'pss-productivity-reference'],
                    scalePreset: 'productivity',
                    visible: true,
                    expanded: false,
                },
                producer_bhp: {
                    title: 'Flowing BHP (material balance)',
                    expanded: false,
                },
                diagnostics: {
                    title: 'Average Reservoir Pressure',
                    visible: true,
                    expanded: false,
                },
                oil_rate: {
                    title: 'Oil Rate (end of constant-rate period)',
                    expanded: false,
                },
            },
        },
    },
    defaultSensitivityDimensionKey: 'drainage_shape',
    capabilities: {
        analyticalMethod: 'well-test',
        hasInjector: false,
        default3DScalar: 'pressure',
        spatialProfile: { defaultAxis: 'i', wellPathLabel: 'Diagonal' },
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES is the interactive default for this single-phase, constant-rate PSS experiment.',
    },
    params: {
        mu_w: 0.5, mu_o: 1,
        c_o: 1e-5, c_w: 3e-6, rock_compressibility: 1e-6,
        depth_reference: 0,
        volume_expansion_o: 1, volume_expansion_w: 1,
        rho_w: 1000, rho_o: 800,
        reservoirPorosity: 0.2,
        s_wc: 0.2, s_or: 0.1, n_w: 2, n_o: 2,
        k_rw_max: 1, k_ro_max: 1,
        capillaryEnabled: false, capillaryPEntry: 0, capillaryLambda: 2,
        nx: 21, ny: 21, nz: 1,
        cellDx: 20, cellDy: 20, cellDz: 10,
        permMode: 'uniform',
        uniformPermX: 20, uniformPermY: 20, uniformPermZ: 2,
        initialPressure: 300,
        initialSaturation: 0.2,
        injectorEnabled: false,
        injectorControlMode: 'pressure',
        producerControlMode: 'rate',
        injectorBhp: 500, producerBhp: 50,
        // 40 m3/day depletes the 176,400 m2 x 10 m volume to the 50 bar floor
        // in ~19 days, so the end of the constant-rate period lands inside a
        // run whose 0.05-day step still resolves a 0.25-day PSS onset.
        targetInjectorRate: 0, targetProducerRate: 40,
        injectorI: 0, injectorJ: 0,
        producerI: 10, producerJ: 10,
        well_radius: 0.1, well_skin: 0,
        analyticalPressureModel: 'dietz-pss',
        // Later than the slowest geometry's measured 0.80-day onset, so the
        // productivity and C_A panels never report a transient value.
        analyticalPssStartDays: 1,
        fimEnabled: false,
        delta_t_days: 0.05, steps: 400,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 25,
        max_well_rate_change_fraction: 1,
        gravityEnabled: false,
    },
    sensitivities: [
        {
            key: 'drainage_shape',
            label: 'Drainage Geometry',
            description: 'Four closed geometries of identical 176,400 m² area produced at the same rate through the same completion. Only the shape term C_A differs — 30.8828, 21.8369, 5.379, 4.5132 — so the separation between the drawdown curves is purely geometric. Elongated drainage and off-centre wells also reach pseudo-steady state later, visible as a delayed approach on the drawdown panel.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'geom_square',
                    label: 'Square, centred (base)',
                    description: '420 m × 420 m with the well at the centre. C_A = 30.8828, the most productive closed geometry in the table.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'geom_2to1',
                    label: '2:1 rectangle, centred',
                    description: '594 m × 297 m, same area and same centred well. C_A = 21.8369: a longer drainage path costs productivity even before the well moves.',
                    paramPatch: { nx: 27, ny: 11, cellDx: 22, cellDy: 27, producerI: 13, producerJ: 5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'geom_4to1',
                    label: '4:1 rectangle, centred',
                    description: '840 m × 210 m, same area. C_A = 5.379 — a 5.7× drop from the square, and a visibly later approach to pseudo-steady state.',
                    paramPatch: { nx: 35, ny: 7, cellDx: 24, cellDy: 30, producerI: 17, producerJ: 3 },
                    affectsAnalytical: true,
                },
                {
                    key: 'geom_quadrant',
                    label: 'Square, well at quadrant centre',
                    description: 'The base square with the well moved to the centre of one quadrant. C_A = 4.5132: an off-centre well in a square is as costly as a 4:1 rectangle. The 22×22 grid places a cell centre exactly on the quarter point.',
                    paramPatch: { nx: 22, ny: 22, cellDx: 420 / 22, cellDy: 420 / 22, producerI: 5, producerJ: 5 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'skin',
            label: 'Skin Factor s',
            description: 'Skin adds a wellbore pressure drop that changes the PSS drawdown without touching the closed-reservoir pressure decline. It is the control for the geometry sweep: skin moves the drawdown and productivity panels but leaves the inferred C_A panel flat, because the inversion divides skin back out. Geometry and completion are separable, and this dimension is what shows it.',
            analyticalOverlayMode: 'per-result',
            variants: [
                { key: 'skin_stimulated', label: 's = −1', description: 'Stimulated completion: smaller PSS drawdown, same C_A.', paramPatch: { well_skin: -1 }, affectsAnalytical: true },
                { key: 'skin_clean', label: 's = 0 (base)', description: 'Clean completion.', paramPatch: {}, affectsAnalytical: true },
                { key: 'skin_damaged', label: 's = +3', description: 'Damaged completion: larger PSS drawdown, same C_A, and the BHP floor is reached sooner.', paramPatch: { well_skin: 3 }, affectsAnalytical: true },
            ],
        },
        {
            key: 'grid_refinement',
            label: 'Grid Refinement',
            description: 'The physical 420 m square, centred well and analytical C_A = 30.8828 stay fixed, so every separation between the curves is numerical well-block and spatial-discretisation error. The inferred C_A is biased high by about 0.9% on the coarse grid and 0.8% on the fine one: a sub-percent productivity error, amplified by the inversion and shrinking monotonically with refinement. It is a small effect here by design — the geometries above are resolved well enough that shape, not the grid, sets the answer.',
            analyticalOverlayMode: 'shared',
            variants: [
                { key: 'grid_coarse', label: '15×15', description: 'Coarse 420 m square.', paramPatch: { nx: 15, ny: 15, cellDx: 28, cellDy: 28, producerI: 7, producerJ: 7 }, affectsAnalytical: false },
                { key: 'grid_base', label: '21×21 (base)', description: 'Base grid.', paramPatch: {}, affectsAnalytical: false },
                { key: 'grid_fine', label: '35×35', description: 'Fine 420 m square.', paramPatch: { nx: 35, ny: 35, cellDx: 12, cellDy: 12, producerI: 17, producerJ: 17 }, affectsAnalytical: false },
            ],
        },
    ],
};
