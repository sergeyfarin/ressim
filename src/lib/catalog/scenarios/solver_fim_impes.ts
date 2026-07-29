import type { UniversalPanelDef } from '../../charts/universalChartTypes';
import type { Scenario } from '../scenarios';

const solverComparisonLivePanels: UniversalPanelDef[] = [
    {
        panelKey: 'rates',
        curves: [
            {
                key: 'oil-rate-sim', label: 'Oil Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#16a34a',
                getData: (ctx) => ctx.sim.oilRate.map((value) => value * ctx.scaleFactor),
            },
            {
                key: 'water-rate-sim', label: 'Water Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#1e3a8a',
                getData: (ctx) => ctx.sim.waterRate.map((value) => value * ctx.scaleFactor),
            },
            {
                key: 'injection-rate', label: 'Injection Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#06b6d4',
                getData: (ctx) => ctx.sim.injectionRate.map((value) => value * ctx.scaleFactor),
            },
        ],
    },
    {
        panelKey: 'diagnostics',
        curves: [
            {
                key: 'avg-pressure-sim', label: 'Average Pressure', curveType: 'simulation',
                yAxisID: 'y', color: '#dc2626',
                getData: (ctx) => ctx.sim.avgPressure,
            },
            {
                key: 'water-cut-sim', label: 'Water Cut', curveType: 'simulation',
                yAxisID: 'y1', color: '#2563eb',
                getData: (ctx) => ctx.sim.waterCutSim,
            },
        ],
    },
];

/**
 * A deliberately coarse, rate-controlled two-phase case that makes the
 * numerical formulation visible. It is an exhibit of current solver behavior,
 * not a claim that either trajectory is an analytical truth reference.
 */
export const solver_fim_impes: Scenario = {
    key: 'solver_fim_impes',
    label: 'FIM vs. IMPES — Coarse Timestep',
    catalog: {
        group: 'other',
        role: 'interpretation',
        caseMode: 'wf',
        parameterSummary: 'Rate-controlled 1D waterflood · 5-day report steps · coupled versus sequential formulation',
    },
    description: 'The same rate-controlled waterflood is solved with FIM and IMPES at deliberately coarse 5-day report steps. Coupling pressure and saturation inside one nonlinear solve changes the pressure and oil-rate trajectory relative to sequential implicit-pressure/explicit-saturation updates. The timestep is intentionally large enough for that numerical choice to remain visible; this is a formulation comparison, not an analytical accuracy benchmark.',
    analyticalMethodSummary: 'Simulation-to-simulation comparison only. No analytical curve is promoted as the oracle because the exhibit isolates timestep and well-coupling behavior rather than a closed-form displacement limit.',
    analyticalMethodReference: 'Aziz & Settari (1979), Petroleum Reservoir Simulation, chapters on IMPES and fully implicit formulations.',
    chartLayoutKey: 'waterflood',
    chartLayoutPatch: {
        rateChart: {
            xAxisMode: 'time',
            xAxisOptions: ['time', 'pvi', 'cumInjection'],
            xAxisRangePolicy: { mode: 'data-extent' },
            panelOrder: ['diagnostics', 'oil_rate', 'rates', 'cumulative', 'volumes', 'recovery'],
            panels: {
                diagnostics: {
                    title: 'Average Reservoir Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    expanded: true,
                },
                oil_rate: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    expanded: true,
                },
                rates: {
                    title: 'Water Cut',
                    curveKeys: ['water-cut-sim'],
                    expanded: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary'],
                    expanded: false,
                },
            },
        },
    },
    defaultSensitivityDimensionKey: 'solver_comparison',
    capabilities: {
        analyticalMethod: 'none',
        hasInjector: true,
        default3DScalar: 'saturation_water',
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES remains the base formulation; the scenario-owned sensitivity explicitly runs the identical case with FIM.',
    },
    params: {
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
        nx: 24,
        ny: 1,
        nz: 1,
        cellDx: 10,
        cellDy: 10,
        cellDz: 1,
        permMode: 'uniform',
        uniformPermX: 2000,
        uniformPermY: 2000,
        uniformPermZ: 200,
        initialPressure: 300,
        initialSaturation: 0.1,
        injectorEnabled: true,
        injectorControlMode: 'rate',
        producerControlMode: 'rate',
        targetInjectorRate: 10,
        targetProducerRate: 10,
        injectorBhp: 400,
        producerBhp: 200,
        bhpMin: 100,
        bhpMax: 500,
        injectorI: 0,
        injectorJ: 0,
        producerI: 23,
        producerJ: 0,
        well_radius: 0.1,
        well_skin: 0,
        fimEnabled: false,
        delta_t_days: 5,
        steps: 20,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    liveChartPanels: solverComparisonLivePanels,
    sensitivities: [
        {
            key: 'solver_comparison',
            label: 'Numerical Formulation',
            description: 'Hold the grid, fluids, wells, controls, timestep, and run horizon fixed; change only whether pressure and saturation are solved sequentially or as one coupled nonlinear system.',
            variesSolver: true,
            variants: [
                {
                    key: 'solver_impes',
                    label: 'IMPES',
                    description: 'Implicit pressure followed by explicit saturation transport, with stability-driven internal subdivision.',
                    paramPatch: { fimEnabled: false },
                    affectsAnalytical: false,
                },
                {
                    key: 'solver_fim',
                    label: 'FIM',
                    description: 'Fully implicit coupled pressure/saturation Newton solve at the same 5-day report timestep.',
                    paramPatch: { fimEnabled: true },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
