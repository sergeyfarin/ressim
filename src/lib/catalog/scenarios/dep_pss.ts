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
 * nothing on the chart moves. The shipped geometries all enclose the same
 * 176,400 m2 and produce at the same rate, so the only thing separating them
 * is C_A — and across the two geometry dimensions it spans 30.8828 down to
 * 0.2318, a factor of 133.
 *
 * That range compresses hard through the logarithm: 133x in C_A is a 35%
 * change in productivity, and the 6.8x between a centred and a quadrant well
 * is only 13%. This is the honest lesson of the Dietz equation, and the
 * reason the inferred-C_A panel is kept alongside the productivity panel
 * rather than instead of it. The C_A panel is the *sensitive* view — it
 * exponentially amplifies a small productivity error, and separates the cases
 * by two orders of magnitude where the drawdown panel separates them by 35% —
 * while the drawdown panel is the *physical* one.
 *
 * Geometries (all 176,400 m2, all single-layer, all 40 m3/day). Measured
 * values are constant-rate runs on this scenario's own parameters:
 *
 *   grid      cell           well           A-ratio   C_A tab.   C_A meas.
 *   21 x 21   20    sq.      centred         1:1      30.8828      31.7
 *   22 x 11   27    sq.      centred         2:1      21.8369      22.1
 *   22 x 21   19.09 x 20     quarter length  1:1      12.9851      13.3
 *   28 x  7   30    sq.      centred         4:1       5.379        5.41
 *   22 x 22   19.09 sq.      quadrant        1:1       4.5132       4.65
 *   35 x  7   26.83 sq.      centred         5:1       2.36         2.41
 *   22 x 11   38.18 x 19.09  quarter length  4:1       0.2318       0.236
 *
 * Every entry lands within 3% of its tabulated value, against neighbouring
 * table entries 1.4x to 23x away, so the match is unambiguous. The bias is
 * positive in every case and does not shrink with refinement (7x7 to 35x35 on
 * the square moves it only from +3.3% to +2.6%): it is not grid error but the
 * pressure dependence of the fluid properties over a depleting run, which is
 * why this case ships no grid-refinement dimension. A candidate 4:1 geometry
 * with the well at 1/8 of the length was measured at C_A = 0.0047, matched no
 * tabulated entry, and was dropped rather than assigned to the nearest one.
 *
 * Off-centre wells sit on grids sized so that a cell centre falls exactly on
 * the quarter point (5.5/22 = 0.25). Position accuracy matters more than it
 * looks: placing the same well on the base 21 x 21 grid puts it 4.8% off the
 * quarter point, which is a 29% error in inferred C_A.
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
 * index this case depends on.
 */
export const dep_pss: Scenario = {
    key: 'dep_pss',
    label: 'Drainage Geometry & Productivity (Dietz)',
    catalog: {
        group: 'flow-regimes-decline',
        role: 'benchmark',
        caseMode: 'dep',
        parameterSummary: 'Equal-area closed geometries · constant-rate producer · C_A from measured drawdown',
    },
    description: 'A constant-rate producer in a closed reservoir, run across drainage geometries of identical area: squares and 2:1, 4:1 and 5:1 rectangles, with the well centred, at a quarter length, or at a quadrant centre.',
    analyticalMethodSummary: 'Dietz pseudo-steady-state productivity and its inverse: average pressure declines by material balance, p_wf=p̄−q/J_Dietz with J_Dietz from the tabulated C_A for each geometry, and the numerical J recovers an effective C_A for comparison. Curves terminate at the analytically predicted end of the constant-rate period.',
    analyticalMethodReference: 'Dietz (1965), JPT 17(8); Earlougher (1977), Advances in Well Test Analysis, SPE Monograph 5, Table C.1; Dake (1978), Fundamentals of Reservoir Engineering, ch. 7; Peaceman (1978), SPEJ 18(3).',
    chartLayoutKey: 'well_test',
    chartLayoutPatch: {
        chart: {
            xAxisMode: 'logTime',
            xAxisOptions: ['logTime', 'time'],
            allowLogScale: true,
            // Physical panels first — the drawdown that defines the case, then
            // the pressures it is built from. The two inverted diagnostics sit
            // at the bottom, collapsed: they are how you would *report* the
            // result, not how you would read it.
            panelOrder: [
                'pss_drawdown', 'producer_bhp', 'diagnostics', 'oil_rate',
                'pss_productivity', 'pss_shape_factor',
            ],
            panels: {
                pss_drawdown: {
                    title: 'Drawdown p̄ − p_wf — Approach to PSS',
                    curveKeys: ['pss-drawdown-sim', 'pss-drawdown-reference'],
                    scalePreset: 'pressure',
                    visible: true,
                    expanded: true,
                },
                producer_bhp: {
                    title: 'Flowing BHP',
                    visible: true,
                    expanded: true,
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
                pss_productivity: {
                    title: 'PSS Productivity Index (diagnostic)',
                    curveKeys: ['pss-productivity-sim', 'pss-productivity-reference'],
                    scalePreset: 'productivity',
                    visible: true,
                    expanded: false,
                },
                pss_shape_factor: {
                    title: 'Inferred Dietz Shape Factor C_A (diagnostic)',
                    curveKeys: ['pss-shape-factor-sim', 'pss-shape-factor-reference'],
                    scalePreset: 'shape_factor',
                    // C_A spans 30.8828 to 0.2318 across the shipped geometries.
                    // On a linear axis from zero the four least productive cases
                    // collapse onto the baseline; the quantity lives inside a
                    // logarithm, so a log axis is its natural presentation.
                    logScale: true,
                    allowLogToggle: true,
                    // Collapsed by default: an effective shape factor recovered
                    // by inverting a productivity measurement needs explaining
                    // before it means anything. The drawdown and BHP panels
                    // above carry the same information directly.
                    visible: true,
                    expanded: false,
                },
                // The producer only leaves rate control in the last day of the
                // run, so a control-limit fraction is zero across essentially
                // the whole chart. The oil-rate panel shows the same handover.
                control_limits: {
                    visible: false,
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
            description: 'Five closed geometries of identical 176,400 m² area, produced at the same rate through the same completion.',
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
                    label: '2:1 rectangle',
                    description: '594 m × 297 m, same area, well still centred. C_A = 21.8369: a longer drainage path costs productivity before the well moves at all.',
                    paramPatch: { nx: 22, ny: 11, cellDx: 27, cellDy: 27, producerI: 10, producerJ: 5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'geom_4to1',
                    label: '4:1 rectangle',
                    description: '840 m × 210 m, same area. C_A = 5.379 — a 5.7× drop from the square, and a visibly later approach to pseudo-steady state.',
                    paramPatch: { nx: 28, ny: 7, cellDx: 30, cellDy: 30, producerI: 13, producerJ: 3 },
                    affectsAnalytical: true,
                },
                {
                    key: 'geom_5to1',
                    label: '5:1 rectangle',
                    description: '939 m × 188 m, same area. C_A = 2.36. The trend with elongation is steep and shows no sign of flattening.',
                    paramPatch: { nx: 35, ny: 7, cellDx: 26.8328, cellDy: 26.8328, producerI: 17, producerJ: 3 },
                    affectsAnalytical: true,
                },
                {
                    key: 'geom_4to1_offset',
                    label: '4:1 rectangle, well off-centre',
                    description: 'The same 840 m × 210 m rectangle with the well moved to a quarter of its length.',
                    paramPatch: { nx: 22, ny: 11, cellDx: 840 / 22, cellDy: 210 / 11, producerI: 5, producerJ: 5 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'well_position',
            label: 'Well Position',
            description: 'One 420 m square, one rate, one completion — only the well moves.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'pos_centred',
                    label: 'Centred (base)',
                    description: 'Well at the centre of the square. C_A = 30.8828.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'pos_quarter',
                    label: 'Quarter length, centred across',
                    description: 'Well at a quarter of one side, still centred on the other. C_A = 12.9851.',
                    paramPatch: { nx: 22, ny: 21, cellDx: 420 / 22, cellDy: 20, producerI: 5, producerJ: 10 },
                    affectsAnalytical: true,
                },
                {
                    key: 'pos_quadrant',
                    label: 'Quadrant centre',
                    description: 'Well at the centre of one quadrant — off-centre in both directions. C_A = 4.5132, as costly as a 4:1 rectangle with a centred well.',
                    paramPatch: { nx: 22, ny: 22, cellDx: 420 / 22, cellDy: 420 / 22, producerI: 5, producerJ: 5 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'skin',
            label: 'Skin Factor s',
            description: 'Skin adds a wellbore pressure drop that changes the PSS drawdown without touching the closed-reservoir pressure decline.',
            analyticalOverlayMode: 'per-result',
            variants: [
                { key: 'skin_stimulated', label: 's = −1', description: 'Stimulated completion: smaller PSS drawdown, same C_A.', paramPatch: { well_skin: -1 }, affectsAnalytical: true },
                { key: 'skin_clean', label: 's = 0 (base)', description: 'Clean completion.', paramPatch: {}, affectsAnalytical: true },
                { key: 'skin_damaged', label: 's = +3', description: 'Damaged completion: larger PSS drawdown, same C_A, and the BHP floor is reached sooner.', paramPatch: { well_skin: 3 }, affectsAnalytical: true },
            ],
        },
    ],
};
