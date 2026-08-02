import type { Scenario } from '../scenarios';
import { waterfloodBLDef } from '../analyticalAdapters';

/**
 * Vertical displacement, where gravity acts along the flow direction rather
 * than across it. In one dimension there is no room for a tongue to form, so
 * the entire effect is the gravity term in the fractional flow — which changes
 * sign with the direction of flow, and therefore so does the error in the
 * viscous-only Buckley-Leverett reference.
 */
export const wf_gravity_stability: Scenario = {
    key: 'wf_gravity_stability',
    label: 'Gravity-Stable vs Unstable Displacement',
    catalog: {
        group: 'buckley-leverett-displacement',
        role: 'simulation',
        caseMode: 'wf',
        parameterSummary: '1D vertical column · upward vs downward flood · signed departure from the viscous BL curve',
    },
    description: 'A 60 m vertical column with one perforation at each end, flooded upward or downward at the same rate.',
    analyticalMethodSummary: 'Buckley-Leverett with Welge shock construction, computed from the viscous fractional flow only. It carries no gravity term and no dependence on flow direction, so one curve serves every variant — the simulation moves above it in the stable direction and below it in the unstable one.',
    analyticalMethodReference: 'Buckley and Leverett (1942); Welge (1952); Dake (1978), Fundamentals of Reservoir Engineering, §10.5 (gravity-modified fractional flow); Hagoort (1980), SPEJ 20(3) — gravity drainage; Dietz (1953).',
    chartLayoutKey: 'waterflood',
    defaultSensitivityDimensionKey: 'flood_direction',
    capabilities: {
        analyticalMethod: 'buckley-leverett',
        hasInjector: true,
        default3DScalar: 'saturation_water',
        // The column is one cell in i and j; the saturation profile lives in k.
        spatialProfile: { defaultAxis: 'k' },
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES is the validated default for this two-phase column; every variant runs in a couple of seconds at the shipped resolution.',
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
        // Rock / relative permeability — identical to wf_bl1d and wf_gravity, so
        // all three cases are judged against the same analytical solution.
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
        // Grid: a single 20 m × 20 m column, 60 cells of 1 m — 4,800 m³ pore volume
        nx: 1,
        ny: 1,
        nz: 60,
        cellDx: 20,
        cellDy: 20,
        cellDz: 1,
        permMode: 'uniform',
        uniformPermX: 2000,
        uniformPermY: 2000,
        uniformPermZ: 2000,
        // Initial conditions
        initialPressure: 300,
        initialSaturation: 0.1,
        // Wells — one perforation each, at opposite ends of the column. Base case
        // injects at the bottom (k = 59) and produces at the top: gravity-stable.
        // Single perforations are also what makes gravity honest here, since the
        // engine shares one BHP across a well's completions without a wellbore
        // hydrostatic correction.
        injectorEnabled: true,
        injectorControlMode: 'rate',
        producerControlMode: 'pressure',
        injectorBhp: 700,
        producerBhp: 200,
        // 100 reservoir m³/day through the 400 m² column puts the base case at
        // G ≈ 1.3 — buoyancy and viscous drive of the same order.
        targetInjectorRate: 100,
        targetProducerRate: 0,
        injectorI: 0,
        injectorJ: 0,
        injectorKLayers: [59],
        producerI: 0,
        producerJ: 0,
        producerKLayers: [0],
        well_radius: 0.1,
        well_skin: 0,
        // 4,800 m³ pore volume at 100 m³/day: 67 days is 1.4 PVI.
        fimEnabled: false,
        delta_t_days: 0.5,
        steps: 134,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: true,
    },
    analyticalDef: waterfloodBLDef,
    sensitivities: [
        {
            key: 'flood_direction',
            label: 'Flood Direction',
            description: 'Three runs of the same column at the same rate: gravity off, flooded upward, flooded downward.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'dir_no_gravity',
                    label: 'Gravity off  (control)',
                    description: 'The same column with buoyancy disabled — a plain 1D Buckley-Leverett tube.',
                    paramPatch: { gravityEnabled: false },
                    affectsAnalytical: false,
                },
                {
                    key: 'dir_up',
                    label: 'Upward  (inject at base — stable, base case)',
                    description: 'Water enters at the base and pushes oil upward. Buoyancy pulls the denser water back down against the flow, suppressing any tendency of water to run ahead: the front is sharper than the analytical shock.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'dir_down',
                    label: 'Downward  (inject at top — unstable)',
                    description: 'The same column flooded the other way. Now buoyancy drives water in the same direction as the flow, water outruns the front, and breakthrough arrives at 0.406 PVI with recovery 0.540.',
                    paramPatch: { injectorKLayers: [0], producerKLayers: [59] },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'gravity_number',
            label: 'Gravity Number  G  (both directions)',
            description: 'The rate ladder in both directions at once, fanning symmetrically about the fixed analytical curve.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'g_up_fast',
                    label: 'Up, G ≈ 0.33  (400 m³/d)',
                    description: 'Fast enough that viscous forces dominate: recovery 0.737, only 3 % above the analytical curve.',
                    paramPatch: { targetInjectorRate: 400, delta_t_days: 0.1, steps: 170 },
                    affectsAnalytical: false,
                },
                {
                    key: 'g_up_base',
                    label: 'Up, G ≈ 1.3  (100 m³/d, base)',
                    description: 'Buoyancy and viscous drive comparable. Recovery 0.792.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'g_up_slow',
                    label: 'Up, G ≈ 3.3  (40 m³/d)',
                    description: 'Gravity-dominated and stable: the displacement approaches a piston. Recovery 0.839, breakthrough 0.717 PVI — later than the analytical shock, which is not something a viscous-only solution can produce.',
                    paramPatch: { targetInjectorRate: 40, delta_t_days: 1, steps: 168 },
                    affectsAnalytical: false,
                },
                {
                    key: 'g_down_fast',
                    label: 'Down, G ≈ 0.33  (400 m³/d)',
                    description: 'The mirror rung: recovery 0.673, 6 % below the analytical curve.',
                    paramPatch: {
                        injectorKLayers: [0], producerKLayers: [59],
                        targetInjectorRate: 400, delta_t_days: 0.1, steps: 170,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'g_down_base',
                    label: 'Down, G ≈ 1.3  (100 m³/d)',
                    description: 'Recovery 0.540 — the same rate that gives 0.792 upward.',
                    paramPatch: { injectorKLayers: [0], producerKLayers: [59] },
                    affectsAnalytical: false,
                },
                {
                    key: 'g_down_slow',
                    label: 'Down, G ≈ 3.3  (40 m³/d)',
                    description: 'Gravity-dominated and unstable: water falls through the oil ahead of the front. Recovery 0.354, half the analytical forecast, and breakthrough at 0.292 PVI. Producing slowly is a good idea in one direction and a bad one in the other.',
                    paramPatch: {
                        injectorKLayers: [0], producerKLayers: [59],
                        targetInjectorRate: 40, delta_t_days: 1, steps: 168,
                    },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'resolution',
            label: 'Physics or Truncation Error?',
            description: 'The same question `wf_capillary` asks about capillary smearing, asked about gravity: refine the grid and watch which curve moves.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'res_15',
                    label: '15 cells, up',
                    description: 'Coarse column, upward flood: recovery 0.772 — already far above BL and moving the wrong way for a truncation error.',
                    paramPatch: { nz: 15, cellDz: 4, injectorKLayers: [14], producerKLayers: [0] },
                    affectsAnalytical: false,
                },
                {
                    key: 'res_60',
                    label: '60 cells, up  (base)',
                    description: 'The shipped resolution: recovery 0.792.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'res_15_down',
                    label: '15 cells, down',
                    description: 'Coarse column, downward flood: recovery 0.542.',
                    paramPatch: { nz: 15, cellDz: 4, injectorKLayers: [0], producerKLayers: [14] },
                    affectsAnalytical: false,
                },
                {
                    key: 'res_60_down',
                    label: '60 cells, down',
                    description: 'Recovery 0.540 — two of a four-point ladder (15/30/60/120) that moves by 0.002 in total. The unstable answer is converged, and it is not the analytical one.',
                    paramPatch: { injectorKLayers: [0], producerKLayers: [59] },
                    affectsAnalytical: false,
                },
                {
                    key: 'res_120_off',
                    label: '120 cells, gravity off',
                    description: 'The convergence control: with no gravity term the refined column recovers 0.710 against the analytical 0.715, closing most of the coarse-grid gap. Off by default because it is the slowest run here.',
                    paramPatch: {
                        nz: 120, cellDz: 0.5, gravityEnabled: false,
                        injectorKLayers: [119], producerKLayers: [0],
                    },
                    affectsAnalytical: false,
                    enabledByDefault: false,
                },
            ],
        },
    ],
};
