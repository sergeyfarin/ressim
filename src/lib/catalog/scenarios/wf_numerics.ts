import { waterfloodLivePanels } from '../chartPanels/waterfloodLivePanels';
import type { Scenario } from '../scenarios';
import { waterfloodBLDef } from '../analyticalAdapters';

/**
 * The case where no Buckley-Leverett assumption is broken.
 *
 * `wf_capillary` and `wf_gravity_stability` each switch on a physical term the
 * fractional-flow solution does not carry, and measure the gap that opens.
 * This case switches nothing on: one dimension, incompressible-scale
 * compressibilities, no capillarity, no gravity, constant total rate. The
 * analytical solution is then exact for the equations being solved, so every
 * difference on the chart is the discretization — and it can be driven to zero
 * by refining, which is what makes it identifiable as such.
 *
 * The measurements are in the dimension descriptions. The headline is that
 * breakthrough error against BL halves each time the cell size halves
 * (0.131 / 0.061 / 0.031 / 0.016 / 0.006 / 0.001 PVI over 50 m to 1.25 m
 * cells), the first-order convergence that single-point upstream weighting
 * predicts.
 */
export const wf_numerics: Scenario = {
    key: 'wf_numerics',
    label: 'Numerical Dispersion & Convergence',
    catalog: {
        group: 'buckley-leverett-displacement',
        role: 'benchmark',
        caseMode: 'wf',
        parameterSummary: '1D waterflood · grid, timestep and solver as forecast variables · convergence against an exact reference',
    },
    description: 'The same 500 m column at six resolutions, from 50 m cells down to 1.25 m. Nothing physical changes between the runs: same rock, same fluids, same rate, same pore volume, no capillarity and no gravity, so the Buckley-Leverett solution is exact for the equations the simulator is solving and the whole gap between the curves is the grid. On 50 m cells water arrives at 0.455 PVI against the analytical 0.586 and recovery at one pore volume injected is 0.686 against 0.715; on 1.25 m cells breakthrough is 0.585 and recovery 0.713. The error in breakthrough halves every time the cell size halves — 0.131, 0.061, 0.031, 0.016, 0.006, 0.001 — which is first-order convergence, the rate single-point upstream weighting is expected to deliver and the signature that separates a discretization error from a missing physical term. The practical consequence is the third dimension here: a coarse grid smears the front in a way that looks very much like a steeper oil relative-permeability curve, and breakthrough timing alone cannot tell the two apart. OPM Flow runs of the same deck, at the base and the converged resolution, are bundled as reference curves — a second simulator taking its own route to the same analytical answer, and the evidence that the IMPES/FIM disagreement in the last dimension here is the formulation rather than a defect.',
    analyticalMethodSummary: 'Buckley-Leverett with Welge shock construction. Every variant shares one analytical curve because none of them changes the physics — the reference is the exact solution of the same equations, and the simulation converges onto it.',
    analyticalMethodReference: 'Buckley and Leverett (1942); Welge (1952); Lantz (1971), SPEJ 11(3) — "Quantitative Evaluation of Numerical Diffusion (Truncation Error)"; Aziz and Settari (1979), Petroleum Reservoir Simulation, ch. 5; Todd, O\'Dell and Hirasaki (1972), JPT 24(11).',
    // The same column, the same rock curves and the same reservoir-volume
    // injection rate run through OPM Flow at both the base and the converged
    // resolution — a second simulator's own convergence path onto the same
    // analytical answer. See the "solver_vs_opm" dimension for what the pair settles.
    referenceSources: [{ kind: 'opm-flow', artifactKeys: ['wf_numerics', 'wf_numerics_fine'] }],
    chartLayoutKey: 'waterflood',
    defaultSensitivityDimensionKey: 'grid_refinement',
    capabilities: {
        analyticalMethod: 'buckley-leverett',
        hasInjector: true,
        default3DScalar: 'saturation_water',
        spatialProfile: { defaultAxis: 'i' },
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES is the validated default for this two-phase column, and the case measures its truncation behaviour directly; the FIM comparison is a sensitivity variant rather than a different default.',
    },
    params: {
        // Fluid — identical to wf_bl1d, wf_capillary and wf_gravity_stability,
        // so all four cases are judged against the same analytical solution.
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
        // Rock / relative permeability
        reservoirPorosity: 0.2,
        s_wc: 0.1,
        s_or: 0.1,
        n_w: 2,
        n_o: 2,
        k_rw_max: 1,
        k_ro_max: 1,
        capillaryEnabled: false,
        capillaryPEntry: 0,
        capillaryLambda: 2,
        // Grid: 500 m x 20 m x 10 m slab at 10 m cells — 20,000 m3 pore volume.
        // Every grid variant rescales cellDx with nx so the pore volume, and
        // therefore the PVI axis, is identical on every curve in the chart.
        nx: 50,
        ny: 1,
        nz: 1,
        cellDx: 10,
        cellDy: 20,
        cellDz: 10,
        permMode: 'uniform',
        uniformPermX: 500,
        uniformPermY: 500,
        uniformPermZ: 500,
        // Initial conditions
        initialPressure: 300,
        initialSaturation: 0.1,
        // Wells — rate control on the injector holds the total rate constant,
        // which is the condition Buckley-Leverett assumes. 100 m3/day into a
        // 20,000 m3 pore volume is 200 days per pore volume injected.
        injectorEnabled: true,
        injectorControlMode: 'rate',
        producerControlMode: 'pressure',
        injectorBhp: 700,
        producerBhp: 200,
        targetInjectorRate: 100,
        targetProducerRate: 0,
        injectorI: 0,
        injectorJ: 0,
        producerI: 49,
        producerJ: 0,
        well_radius: 0.1,
        well_skin: 0,
        // 260 days at 100 m3/day is 1.3 pore volumes injected.
        fimEnabled: false,
        delta_t_days: 1,
        steps: 260,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    analyticalDef: waterfloodBLDef,
    liveChartPanels: waterfloodLivePanels,
    sensitivities: [
        {
            key: 'grid_refinement',
            label: 'Grid Refinement',
            description: 'One physical problem, six grids. Cell size falls from 50 m to 1.25 m while the column length, pore volume, rate and rock stay fixed, so the PVI axis means exactly the same thing on every curve and the analytical solution is the same single line. Breakthrough moves 0.455 → 0.525 → 0.555 → 0.570 → 0.580 → 0.585 PVI against the analytical 0.586, and recovery at one pore volume injected moves 0.686 → 0.699 → 0.707 → 0.711 → 0.712 → 0.713 against 0.715. Read the breakthrough error instead of the breakthrough: 0.131, 0.061, 0.031, 0.016, 0.006, 0.001 — it halves each time the cell size halves. That is first-order convergence in Δx, which is what a single-point upstream-weighted saturation update is supposed to give, and it is the test that identifies a gap as numerical: a physical term the reference is missing would not shrink under refinement at all.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'grid_10',
                    label: '10 cells  (Δx = 50 m)',
                    description: 'Deliberately too coarse. Water arrives at 0.455 PVI, 22 % early, and the front is spread over most of the column. Nothing is wrong with the physics — this is what a field-scale cell does to a shock.',
                    paramPatch: { nx: 10, cellDx: 50, producerI: 9 },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_25',
                    label: '25 cells  (Δx = 20 m)',
                    description: 'Breakthrough 0.525 PVI, error 0.061 — a little under half the 50 m error, for a little under half the cell size.',
                    paramPatch: { nx: 25, cellDx: 20, producerI: 24 },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_50',
                    label: '50 cells  (Δx = 10 m, base)',
                    description: 'The shipped resolution and a realistic one: breakthrough 0.555 PVI, recovery 0.707 at one pore volume injected. Still 5 % early on breakthrough, and 1 % low on recovery.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_100',
                    label: '100 cells  (Δx = 5 m)',
                    description: 'Breakthrough 0.570 PVI, error 0.016.',
                    paramPatch: { nx: 100, cellDx: 5, producerI: 99 },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_200',
                    label: '200 cells  (Δx = 2.5 m)',
                    description: 'Breakthrough 0.580 PVI, error 0.006 — within the 0.005 PVI that one report step resolves. Below this the measurement is limited by how often the run is sampled, not by the grid.',
                    paramPatch: { nx: 200, cellDx: 2.5, producerI: 199 },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_400',
                    label: '400 cells  (Δx = 1.25 m)',
                    description: 'The convergence endpoint: breakthrough 0.585 against the analytical 0.586, recovery 0.713 against 0.715. Off by default because it is by far the slowest run here — the cost of the last 0.005 PVI of accuracy is roughly 15x the base case.',
                    paramPatch: { nx: 400, cellDx: 1.25, producerI: 399 },
                    affectsAnalytical: false,
                    enabledByDefault: false,
                },
            ],
        },
        {
            key: 'time_truncation',
            label: 'Timestep & the Stability Limiter',
            description: 'The same 10 m grid stepped four different ways, and then twice more with its safety net removed. The report step itself barely matters — 0.25, 1 and 4 days give recovery 0.709, 0.707 and 0.698 at one pore volume injected — because IMPES subdivides internally whenever a cell\'s saturation would move more than `max_sat_change_per_step`, so asking for a longer step mostly buys more substeps rather than more error. Raise that limit and the protection goes away: at 0.5 the run recovers 0.733, above the analytical 0.715, and at 1.0 it reports 0.936 — more oil than the column contains, since only 0.889 of the oil in place is mobile at all. That is the explicit saturation update going unstable, and it is the reason the limiter exists. The two failures are not the same failure, and the chart can tell them apart. At 0.5 the run is merely wrong: it lands above an exact solution, which no correct answer can do, but volumes still balance to 1e-12 of pore volume, so this is discretization error with the sign flipped. At 1.0 it is inventing barrels — material balance drifts 8.9 % of pore volume and the run now carries a warning saying so. Neither one looks broken otherwise: no crash, no NaN, and the saturations stay inside the end points because transport clamps them there. An IMPES answer above the analytical curve in a case with no gravity and no capillarity is not a better answer, and material balance is the only thing on this page that can say which kind of not-better it is.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'dt_quarter',
                    label: 'Δt = 0.25 d',
                    description: 'Four times the reporting resolution of the base case and essentially the same answer: breakthrough 0.550 PVI, recovery 0.709. Time truncation is not what limits this run.',
                    paramPatch: { delta_t_days: 0.25, steps: 1040 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_base',
                    label: 'Δt = 1 d  (base)',
                    description: 'Breakthrough 0.555 PVI, recovery 0.707.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_four',
                    label: 'Δt = 4 d',
                    description: 'Recovery 0.698 — 1 % below the base case, for a quarter of the run time. The saturation limiter is absorbing most of the requested step.',
                    paramPatch: { delta_t_days: 4, steps: 65 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_ten',
                    label: 'Δt = 10 d',
                    description: 'Recovery 0.691. Also the point where the report cadence itself becomes the measurement limit: one step is 0.05 PVI, so breakthrough can no longer be located more precisely than that.',
                    paramPatch: { delta_t_days: 10, steps: 26 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_limiter_half',
                    label: 'Δt = 4 d, limiter relaxed to ΔS ≤ 0.5',
                    description: 'The first wrong result: recovery 0.733 at one pore volume injected, above the analytical 0.715 in a case that has no mechanism for beating it. Volumes still balance to 1e-12 of pore volume, so nothing has been created — the front is simply in the wrong place, far enough that the error changed sign. No warning is raised, and none should be.',
                    paramPatch: { delta_t_days: 4, steps: 65, max_sat_change_per_step: 0.5 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_limiter_off',
                    label: 'Δt = 4 d, limiter effectively off  (ΔS ≤ 1.0)',
                    description: 'Recovery 0.936 of the oil in place, when only 0.889 of it is mobile — the run produces oil that is not there and reports breakthrough at 0.860 PVI. Unlike the rung above it, this one is not merely mis-placing the front: material balance drifts 8.9 % of pore volume, and that is what raises the warning. Kept in the case because a stability limit is much easier to respect once you have seen what it looks like when it is violated, and because it is the regression test for the warning.',
                    paramPatch: { delta_t_days: 4, steps: 65, max_sat_change_per_step: 1.0 },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'dispersion_or_rock',
            label: 'Smeared by the Grid or by the Rock?',
            description: 'Two ways to make water arrive early, and the reason breakthrough timing alone cannot choose between them. A 50 m grid with a benign oil relative permeability (n_o = 2) breaks through at 0.455 PVI; a 2.5 m grid with a steeper one (n_o = 3.5) breaks through at 0.465 — a 2 % difference, well inside what a history match would call agreement. Everything after breakthrough separates them: at one pore volume injected the coarse run has recovered 0.686 and the steep-curve run 0.586, a tenth of the oil in place apart. The analytical curves say the same thing from the other side — the coarse run is a numerical error against the n_o = 2 solution and converges onto it under refinement, while the steep-curve run *is* its own analytical solution, matched to within 0.003. Calibrating a relative-permeability curve on a coarse grid transfers the grid\'s error into the rock properties, where refining the model will never remove it.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'dor_coarse',
                    label: 'Δx = 50 m, n_o = 2  (grid error)',
                    description: 'The coarse grid on the benign rock curve: breakthrough 0.455 PVI, recovery 0.686 at one pore volume injected, against its own analytical 0.586 and 0.715.',
                    paramPatch: { nx: 10, cellDx: 50, producerI: 9 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dor_fine',
                    label: 'Δx = 2.5 m, n_o = 2  (converged control)',
                    description: 'The same rock on a converged grid: breakthrough 0.580 against 0.586, recovery 0.712 against 0.715. This is what the coarse run is trying to be.',
                    paramPatch: { nx: 200, cellDx: 2.5, producerI: 199 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dor_steep',
                    label: 'Δx = 2.5 m, n_o = 3.5  (rock)',
                    description: 'A converged grid with a steeper oil curve. Breakthrough 0.465 PVI — within 0.01 of the coarse run — but recovery 0.586, a tenth of the oil in place below it. Its own analytical solution predicts 0.468 and 0.589, so this run is not in error at all; it is simply a different reservoir.',
                    paramPatch: { nx: 200, cellDx: 2.5, producerI: 199, n_o: 3.5 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'solver_vs_opm',
            label: 'Solver Choice, Arbitrated by OPM Flow',
            // `wf_bl1d` already carries a timestep x formulation matrix; this
            // dimension exists for the thing that one cannot do, which is put a
            // third, independent implicit simulator beside the pair.
            variesSolver: true,
            description: 'The same grid, the same timestep, the same inputs, run through the two formulations the engine ships. IMPES breaks through at 0.555 PVI and FIM at 0.535 — a 0.02 PVI disagreement, four days of a 260-day run — while both recover 0.707 at one pore volume injected and the analytical solution says 0.586 and 0.715. The OPM Flow reference curves settle which of those is the formulation talking. OPM is fully implicit, and on the identical deck it breaks through at 0.535 PVI and recovers 0.7066 — the same answer as ResSim\'s FIM path to three decimals, and the same 0.02 PVI away from ResSim\'s IMPES path. On the converged grid the pair moves together again (OPM 0.560 and 0.7127 against ResSim 0.580 and 0.712). So the disagreement is not a defect in either code: it is what implicit and explicit saturation updates do to a shock, reproduced independently. Solver choice belongs on the same list as grid and timestep — it moves the forecast, and it is a choice, not a property of the reservoir.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'solver_impes',
                    label: 'IMPES  (base)',
                    description: 'Implicit pressure, explicit saturation. Breakthrough 0.555 PVI, recovery 0.707.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'solver_fim',
                    label: 'Fully implicit  (FIM)',
                    description: 'Breakthrough 0.535 PVI, recovery 0.707 — earlier water for the same recovery, and roughly an order of magnitude more work per step. On this case the extra stability buys nothing, because the explicit scheme was never near its limit.',
                    paramPatch: { fimEnabled: true },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
