import type { Scenario } from '../scenarios';

/**
 * Dietz is a pseudo-steady productivity experiment, not a complete depletion
 * decline law.  This case therefore holds rate constant, waits for boundary
 * effects to establish PSS, and compares flowing BHP with
 *   p_wf = p_bar - q / J_Dietz.
 * Average pressure falls independently by material balance, q t/(PV c_t).
 */
export const dep_pss: Scenario = {
    key: 'dep_pss',
    label: 'Dietz PSS Productivity',
    catalog: {
        group: 'depletion-decline',
        role: 'benchmark',
        caseMode: 'dep',
        parameterSummary: 'Closed square · constant-rate producer · late-time Dietz PI measurement',
    },
    description: 'A constant-rate producer in a closed square reservoir. After the pressure disturbance reaches all boundaries, the gap between average reservoir pressure and flowing BHP becomes the Dietz pseudo-steady drawdown q/J. The analytical BHP is deliberately hidden before that late-time window: Dietz is not an early-time transient solution and is not used here as a full-history decline curve.',
    analyticalMethodSummary: 'Dietz pseudo-steady-state productivity: average pressure declines by closed-tank material balance while flowing BHP is p̄ − q/J_Dietz after the declared PSS onset.',
    analyticalMethodReference: 'Dietz (1965), Determination of Average Reservoir Pressure from Build-Up Surveys; Peaceman (1978), SPEJ 18(3).',
    chartLayoutKey: 'well_test',
    defaultSensitivityDimensionKey: 'skin',
    capabilities: {
        analyticalMethod: 'well-test',
        hasInjector: false,
        default3DScalar: 'pressure',
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
        targetInjectorRate: 0, targetProducerRate: 10,
        injectorI: 0, injectorJ: 0,
        producerI: 10, producerJ: 10,
        well_radius: 0.1, well_skin: 0,
        analyticalPressureModel: 'dietz-pss',
        analyticalPssStartDays: 1,
        fimEnabled: false,
        delta_t_days: 0.05, steps: 200,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 25,
        max_well_rate_change_fraction: 1,
        gravityEnabled: false,
    },
    sensitivities: [
        {
            key: 'skin',
            label: 'Skin Factor s',
            description: 'Skin changes the constant PSS drawdown q/J without changing the closed-reservoir average-pressure decline. This cleanly separates completion loss from reservoir storage.',
            analyticalOverlayMode: 'per-result',
            variants: [
                { key: 'skin_stimulated', label: 's = −1', description: 'Stimulated completion: smaller PSS drawdown.', paramPatch: { well_skin: -1 }, affectsAnalytical: true },
                { key: 'skin_clean', label: 's = 0 (base)', description: 'Clean completion.', paramPatch: {}, affectsAnalytical: true },
                { key: 'skin_damaged', label: 's = +3', description: 'Damaged completion: larger PSS drawdown.', paramPatch: { well_skin: 3 }, affectsAnalytical: true },
            ],
        },
        {
            key: 'production_rate',
            label: 'Controlled Rate q',
            description: 'At PSS, both the material-balance pressure slope and the Dietz drawdown scale linearly with the imposed production rate.',
            analyticalOverlayMode: 'per-result',
            variants: [
                { key: 'rate_low', label: 'q = 5 m³/day', description: 'Half-rate pressure depletion and drawdown.', paramPatch: { targetProducerRate: 5 }, affectsAnalytical: true },
                { key: 'rate_base', label: 'q = 10 m³/day (base)', description: 'Base constant-rate experiment.', paramPatch: {}, affectsAnalytical: true },
                { key: 'rate_high', label: 'q = 15 m³/day', description: 'Higher, but still BHP-unconstrained, constant rate.', paramPatch: { targetProducerRate: 15 }, affectsAnalytical: true },
            ],
        },
        {
            key: 'grid_refinement',
            label: 'Grid Refinement',
            description: 'The physical square and analytical Dietz solution stay fixed; only the Cartesian representation of the centre well changes.',
            analyticalOverlayMode: 'shared',
            variants: [
                { key: 'grid_coarse', label: '15×15', description: 'Coarse 420 m square.', paramPatch: { nx: 15, ny: 15, cellDx: 28, cellDy: 28, producerI: 7, producerJ: 7 }, affectsAnalytical: false },
                { key: 'grid_base', label: '21×21 (base)', description: 'Base grid.', paramPatch: {}, affectsAnalytical: false },
                { key: 'grid_fine', label: '35×35', description: 'Fine 420 m square.', paramPatch: { nx: 35, ny: 35, cellDx: 12, cellDy: 12, producerI: 17, producerJ: 17 }, affectsAnalytical: false },
            ],
        },
    ],
};
