import type { Scenario } from '../scenarios';
import { waterfloodBLDef } from '../analyticalAdapters';

/**
 * The case where no Buckley-Leverett assumption is broken.
 *
 * `wf_capillary` and `wf_gravity_stability` each switch on a physical term the
 * fractional-flow solution does not carry, and measure the gap that opens.
 * This case switches nothing on: one dimension, incompressible-scale
 * compressibilities, no capillarity, no gravity, constant total rate. The
 * analytical solution supplies the incompressible reference. The small storage
 * terms are also checked against matched Flow inputs; numerical refinement
 * must be distinguished from that physical-model approximation.
 *
 * The grid ladder checks first-order convergence of breakthrough and recovery.
 * #63's conservative component closure changes the former measured error
 * coefficient; the test retains its accuracy band with a further validation
 * refinement beyond the offered grid ladder.
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
    description: 'Numerical dispersion and convergence for a 1D waterflood. Grid, timestep and solver changes are compared with one fixed Buckley–Leverett solution.',
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
    sensitivities: [
        {
            key: 'grid_refinement',
            label: 'Grid Resolution',
            description: 'One physical problem, six grids. Cell size falls from 50 m to 1.25 m while the column length, pore volume, rate and rock stay fixed.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'grid_10',
                    label: '10 cells  (Δx = 50 m)',
                    description: 'Deliberately coarse: water arrives early and the front spreads over much of the column. Compare refinement to distinguish this numerical smearing from the rock response.',
                    paramPatch: { nx: 10, cellDx: 50, producerI: 9 },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_25',
                    label: '25 cells  (Δx = 20 m)',
                    description: 'Smaller cells reduce the early-breakthrough error while preserving the physical model.',
                    paramPatch: { nx: 25, cellDx: 20, producerI: 24 },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_50',
                    label: '50 cells  (Δx = 10 m, base)',
                    description: 'The shipped resolution still brings water through early and underpredicts recovery relative to the analytical solution.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_100',
                    label: '100 cells  (Δx = 5 m)',
                    description: 'Refines the front while keeping the same physical domain and rock curve.',
                    paramPatch: { nx: 100, cellDx: 5, producerI: 99 },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_200',
                    label: '200 cells  (Δx = 2.5 m)',
                    description: 'A fine-grid control. The breakthrough front approaches the analytical prediction as the grid is refined.',
                    paramPatch: { nx: 200, cellDx: 2.5, producerI: 199 },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_400',
                    label: '400 cells  (Δx = 1.25 m)',
                    description: 'The finest displayed grid approaches the analytical front more closely. Off by default because the smaller cells require many more transport steps.',
                    paramPatch: { nx: 400, cellDx: 1.25, producerI: 399 },
                    affectsAnalytical: false,
                    enabledByDefault: false,
                },
            ],
        },
        {
            key: 'time_truncation',
            label: 'Timestep & the Stability Limiter',
            description: 'The same 10 m grid stepped four different ways, and then twice more with its safety net removed.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'dt_quarter',
                    label: 'Δt = 0.25 d',
                    description: 'Four times the reporting resolution of the base case gives similar recovery. Grid refinement has the larger effect here.',
                    paramPatch: { delta_t_days: 0.25, steps: 1040 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_base',
                    label: 'Δt = 1 d  (base)',
                    description: 'The base report cadence, with stability-controlled internal steps.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_four',
                    label: 'Δt = 4 d',
                    description: 'The saturation limiter takes smaller internal steps, keeping recovery close to the base case despite the coarser report cadence.',
                    paramPatch: { delta_t_days: 4, steps: 65 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_ten',
                    label: 'Δt = 10 d',
                    description: 'The coarse report cadence can hide front arrival between displayed points. The engine still uses accepted substeps to control saturation changes.',
                    paramPatch: { delta_t_days: 10, steps: 26 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_limiter_half',
                    label: 'Δt = 4 d, limiter relaxed to ΔS ≤ 0.5',
                    description: 'Relaxing the saturation limit overpredicts recovery against the analytical reference even though component inventories remain balanced.',
                    paramPatch: { delta_t_days: 4, steps: 65, max_sat_change_per_step: 0.5 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_limiter_off',
                    label: 'Δt = 4 d, limiter effectively off  (ΔS ≤ 1.0)',
                    description: 'The conservative component closure preserves the oil inventory, but the relaxed saturation limit can still overpredict recovery against the analytical solution. Mass balance and numerical accuracy are different checks.',
                    paramPatch: { delta_t_days: 4, steps: 65, max_sat_change_per_step: 1.0 },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'dispersion_or_rock',
            label: 'Smeared by the Grid or by the Rock?',
            description: 'Two ways to make water arrive early, and the reason breakthrough timing alone cannot choose between them.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'dor_coarse',
                    label: 'Δx = 50 m, n_o = 2  (grid error)',
                    description: 'The coarse grid on the benign rock curve brings water through early and underpredicts recovery against its analytical solution.',
                    paramPatch: { nx: 10, cellDx: 50, producerI: 9 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dor_fine',
                    label: 'Δx = 2.5 m, n_o = 2  (converged control)',
                    description: 'The same rock on a fine grid approaches its analytical breakthrough and recovery. This is the control for the coarse-grid run.',
                    paramPatch: { nx: 200, cellDx: 2.5, producerI: 199 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dor_steep',
                    label: 'Δx = 2.5 m, n_o = 3.5  (rock)',
                    description: 'A fine grid with a steeper oil curve brings water through at nearly the same time as the coarse run, but recovers substantially less oil. Its own analytical solution confirms that the rock curve causes this change.',
                    paramPatch: { nx: 200, cellDx: 2.5, producerI: 199, n_o: 3.5 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'solver_formulation',
            label: 'Solver (FIM vs IMPES)',
            variesSolver: true,
            description: 'Compare IMPES and FIM at fine and coarse report steps. The analytical solution is unchanged.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'solver_impes_base',
                    label: 'IMPES 1-day steps (base)',
                    description: 'IMPES with stability-controlled internal substeps.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'solver_fim_base',
                    label: 'FIM 1-day steps',
                    description: 'Fully implicit pressure and saturation at the base report step.',
                    paramPatch: { fimEnabled: true },
                    affectsAnalytical: false,
                },
                {
                    key: 'solver_impes_coarse',
                    label: 'IMPES 10-day steps',
                    description: 'IMPES with a coarse requested report step and internal subdivision.',
                    paramPatch: { delta_t_days: 10, steps: 26 },
                    affectsAnalytical: false,
                },
                {
                    key: 'solver_fim_coarse',
                    label: 'FIM 10-day steps',
                    description: 'FIM with a coarse requested report step and nonlinear timestep control.',
                    paramPatch: { fimEnabled: true, delta_t_days: 10, steps: 26 },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
