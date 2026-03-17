/**
 * scenarios.ts — single source of truth for all predefined simulation scenarios.
 *
 * Replaces the combination of presetCases.ts + benchmarkCases.ts + caseCatalog.ts + caseLibrary.ts.
 * Each scenario is self-contained: full param set + chart preset + optional sensitivity definition.
 */

import type { RateChartLayoutConfig } from '../charts/rateChartLayoutConfig';

// ─── Types ────────────────────────────────────────────────────────────────────

export type ScenarioClass = 'waterflood' | 'depletion' | '3phase';

export type SensitivityVariant = {
    key: string;
    label: string;
    description: string;
    /** Parameters to merge on top of the scenario base params for this variant. */
    paramPatch: Record<string, unknown>;
    /**
     * True  → this variant changes a parameter that feeds the analytical solution
     *         (e.g. mu_o changes fractional flow → both sim and analytical update).
     * False → the analytical solution is grid/timestep-independent; only the
     *         simulation result changes (e.g. grid refinement).
     */
    affectsAnalytical: boolean;
};

export type ScenarioSensitivity = {
    key: string;
    label: string;
    description: string;
    variants: SensitivityVariant[];
};

export type Scenario = {
    key: string;
    label: string;
    description: string;
    scenarioClass: ScenarioClass;
    /** Complete, self-contained simulator parameter set. */
    params: Record<string, unknown>;
    /** Key into CHART_PRESETS — controls default x-axis, panels, and curve selection. */
    chartPreset: string;
    sensitivity?: ScenarioSensitivity;
};

// ─── Shared param bases ───────────────────────────────────────────────────────

/**
 * Full param set for Buckley-Leverett waterflood cases.
 * Sourced from bl_case_a_refined benchmark (96-cell, pressure-controlled, mu_o=1, mu_w=0.5).
 */
const BL_A_BASE_PARAMS: Record<string, unknown> = {
    analyticalSolutionMode: 'waterflood',
    mu_w: 0.5,
    mu_o: 1,
    c_o: 0.00001,
    c_w: 0.000003,
    rock_compressibility: 0.000001,
    depth_reference: 0,
    volume_expansion_o: 1,
    volume_expansion_w: 1,
    rho_w: 1000,
    rho_o: 800,
    well_radius: 0.1,
    well_skin: 0,
    max_pressure_change_per_step: 75,
    max_well_rate_change_fraction: 0.75,
    injectorEnabled: true,
    injectorControlMode: 'pressure',
    producerControlMode: 'pressure',
    injectorBhp: 500,
    producerBhp: 100,
    targetInjectorRate: 0,
    targetProducerRate: 0,
    reservoirPorosity: 0.2,
    nx: 96,
    ny: 1,
    nz: 1,
    cellDx: 10,
    cellDy: 10,
    cellDz: 1,
    delta_t_days: 0.125,
    steps: 240,
    max_sat_change_per_step: 0.05,
    initialPressure: 300,
    initialSaturation: 0.1,
    injectorI: 0,
    injectorJ: 0,
    producerI: 95,
    producerJ: 0,
    permMode: 'uniform',
    uniformPermX: 2000,
    uniformPermY: 2000,
    uniformPermZ: 200,
    s_wc: 0.1,
    s_or: 0.1,
    n_w: 2,
    n_o: 2,
    k_rw_max: 1,
    k_ro_max: 1,
    gravityEnabled: false,
    capillaryEnabled: false,
    capillaryPEntry: 0,
    capillaryLambda: 2,
    rateControlledWells: false,
};

/**
 * Full param set for Dietz-style depletion cases.
 * Grid and producer position are overridden per scenario below.
 */
const DIETZ_BASE_PARAMS: Record<string, unknown> = {
    analyticalSolutionMode: 'depletion',
    mu_w: 0.5,
    mu_o: 1,
    c_o: 0.00001,
    c_w: 0.000003,
    rock_compressibility: 0.000001,
    depth_reference: 0,
    volume_expansion_o: 1,
    volume_expansion_w: 1,
    rho_w: 1000,
    rho_o: 800,
    well_radius: 0.1,
    well_skin: 0,
    max_pressure_change_per_step: 75,
    max_well_rate_change_fraction: 0.75,
    injectorEnabled: false,
    injectorControlMode: 'pressure',
    producerControlMode: 'pressure',
    injectorBhp: 500,
    producerBhp: 100,
    targetInjectorRate: 0,
    targetProducerRate: 0,
    reservoirPorosity: 0.2,
    nx: 21,
    ny: 21,
    nz: 1,
    cellDx: 20,
    cellDy: 20,
    cellDz: 10,
    delta_t_days: 1,
    steps: 50,
    max_sat_change_per_step: 0.05,
    initialPressure: 300,
    initialSaturation: 0.1,
    injectorI: 0,
    injectorJ: 0,
    producerI: 10,   // overridden per case
    producerJ: 10,   // overridden per case
    permMode: 'uniform',
    uniformPermX: 50,
    uniformPermY: 50,
    uniformPermZ: 5,
    s_wc: 0.1,
    s_or: 0.1,
    n_w: 2,
    n_o: 2,
    k_rw_max: 1,
    k_ro_max: 1,
    gravityEnabled: false,
    capillaryEnabled: false,
    capillaryPEntry: 0,
    capillaryLambda: 2,
};

/**
 * Full param set for Fetkovich exponential decline cases.
 * 1D slab geometry, high initial pressure, low permeability — drives exponential decline.
 * Peaceman r_eq ≈ 1.98 m for this grid (cellDx=cellDy=10 m), so skin = -2 is the safe minimum
 * (ln(r_eq/r_w) ≈ 2.99; denom = 2.99 + skin must remain positive for the simulator).
 */
const FETKOVICH_BASE_PARAMS: Record<string, unknown> = {
    ...DIETZ_BASE_PARAMS,
    nx: 48,
    ny: 1,
    nz: 1,
    cellDx: 10,
    cellDy: 10,
    cellDz: 10,
    uniformPermX: 20,
    uniformPermY: 20,
    uniformPermZ: 2,
    producerBhp: 50,
    initialPressure: 1500,
    steps: 100,
    delta_t_days: 5,
    producerI: 47,
    producerJ: 0,
};

// ─── Chart presets ────────────────────────────────────────────────────────────

export const CHART_PRESETS: Record<string, RateChartLayoutConfig> = {
    /**
     * Standard waterflood view: PVI x-axis, breakthrough watercut + recovery + pressure panels.
     * Analytical overlay shown alongside simulation curves.
     */
    waterflood: {
        rateChart: {
            xAxisMode: 'pvi',
            xAxisOptions: ['pvi', 'time', 'cumInjection'],
            allowLogScale: false,
            logScale: false,
            ratesExpanded: true,
            cumulativeExpanded: true,
            diagnosticsExpanded: false,
            panels: {
                rates: {
                    title: 'Breakthrough',
                    curveKeys: ['water-cut-sim', 'water-cut-reference', 'avg-water-sat'],
                    curveLabels: ['Water Cut (Sim)', 'Water Cut (Ref)', 'Avg Water Sat'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                },
                cumulative: {
                    title: 'Recovery',
                    curveKeys: ['recovery-factor', 'cum-oil-sim', 'cum-oil-reference', 'cum-injection'],
                    curveLabels: ['Recovery Factor', 'Cum Oil', 'Cum Oil (Ref)', 'Cum Injection'],
                    scalePreset: 'cumulative',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    curveLabels: ['Avg Pressure'],
                    scalePreset: 'pressure',
                },
            },
        },
    },

    /**
     * Standard depletion view: time x-axis, oil rate + cumulative oil + pressure panels.
     * Analytical reference solution shown alongside simulation.
     */
    depletion: {
        rateChart: {
            xAxisMode: 'time',
            xAxisOptions: ['time', 'tD', 'logTime'],
            allowLogScale: true,
            logScale: false,
            ratesExpanded: true,
            cumulativeExpanded: true,
            diagnosticsExpanded: true,
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference', 'oil-rate-error'],
                    curveLabels: ['Oil Rate', 'Oil Rate (Ref)', 'Oil Rate Error'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                cumulative: {
                    title: 'Cumulative Oil / Recovery',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference', 'recovery-factor'],
                    curveLabels: ['Cum Oil', 'Cum Oil (Ref)', 'Recovery Factor'],
                    scalePreset: 'cumulative',
                },
                diagnostics: {
                    title: 'Pressure / Decline',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
                    curveLabels: ['Avg Pressure', 'Avg Pressure (Ref)'],
                    scalePreset: 'pressure',
                },
            },
        },
    },

    /**
     * Fetkovich-specific: log-time x-axis by default, log-scale rates.
     * Exponential decline is most visible on log-time axes.
     */
    fetkovich: {
        rateChart: {
            xAxisMode: 'logTime',
            xAxisOptions: ['logTime', 'time', 'tD'],
            allowLogScale: true,
            logScale: true,
            ratesExpanded: true,
            cumulativeExpanded: true,
            diagnosticsExpanded: true,
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference', 'oil-rate-error'],
                    curveLabels: ['Oil Rate', 'Oil Rate (Ref)', 'Oil Rate Error'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                cumulative: {
                    title: 'Cumulative Oil / Recovery',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference', 'recovery-factor'],
                    curveLabels: ['Cum Oil', 'Cum Oil (Ref)', 'Recovery Factor'],
                    scalePreset: 'cumulative',
                },
                diagnostics: {
                    title: 'Pressure / Decline',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
                    curveLabels: ['Avg Pressure', 'Avg Pressure (Ref)'],
                    scalePreset: 'pressure',
                },
            },
        },
    },
};

// ─── Grid refinement sensitivity (shared by BL Case A and B) ─────────────────

/**
 * Three-level grid refinement sensitivity.
 * The Buckley-Leverett analytical solution is grid-independent — only the
 * simulation result changes, making this a pure numerical convergence study.
 */
function buildGridRefinementSensitivity(): ScenarioSensitivity {
    return {
        key: 'grid-refinement',
        label: 'Grid refinement',
        description: 'Compare numerical convergence across three grid resolutions. The analytical reference solution is grid-independent.',
        variants: [
            {
                key: 'grid_24',
                label: '24 cells (coarse)',
                description: 'Coarse 24-cell grid — large numerical diffusion, smeared front.',
                paramPatch: { nx: 24, producerI: 23, cellDx: 40 },
                affectsAnalytical: false,
            },
            {
                key: 'grid_48',
                label: '48 cells (medium)',
                description: 'Intermediate 48-cell grid — reduced numerical diffusion.',
                paramPatch: { nx: 48, producerI: 47, cellDx: 20 },
                affectsAnalytical: false,
            },
            {
                key: 'grid_96',
                label: '96 cells (fine)',
                description: 'Fine 96-cell reference grid — matches the base case exactly.',
                paramPatch: {},
                affectsAnalytical: false,
            },
        ],
    };
}

// ─── Scenarios ────────────────────────────────────────────────────────────────

export const SCENARIOS: Scenario[] = [
    // ── Waterflood ──────────────────────────────────────────────────────────

    {
        key: 'wf_bl_case_a',
        label: 'BL Case A',
        description: '1D waterflood, M ≈ 2 (μ_o = 1 cp). Numerical solution converges to the BL shock as grid is refined. — Buckley & Leverett (1942)',
        scenarioClass: 'waterflood',
        chartPreset: 'waterflood',
        params: { ...BL_A_BASE_PARAMS },
        sensitivity: buildGridRefinementSensitivity(),
    },

    {
        key: 'wf_bl_case_b',
        label: 'BL Case B',
        description: '1D waterflood, mild unfavorable mobility (μ_o = 1.4 cp). Earlier breakthrough than Case A. — Buckley & Leverett (1942)',
        scenarioClass: 'waterflood',
        chartPreset: 'waterflood',
        params: {
            ...BL_A_BASE_PARAMS,
            mu_w: 0.6,
            mu_o: 1.4,
            initialSaturation: 0.15,
            s_wc: 0.15,
            s_or: 0.15,
            n_w: 2.2,
        },
        sensitivity: buildGridRefinementSensitivity(),
    },

    {
        key: 'wf_mobility_study',
        label: 'Mobility Study',
        description: 'Mobility ratio M = μ_w/μ_o shifts the fractional flow curve, controlling breakthrough timing and recovery. — Craig (1971)',
        scenarioClass: 'waterflood',
        chartPreset: 'waterflood',
        params: { ...BL_A_BASE_PARAMS },
        sensitivity: {
            key: 'mobility',
            label: 'Oil viscosity (μ_o)',
            description: 'Vary oil viscosity to explore how mobility ratio affects breakthrough, watercut shape, and recovery. Both simulation and analytical update.',
            variants: [
                {
                    key: 'mu_favorable',
                    label: 'μ_o = 0.5 cp (M ≈ 1)',
                    description: 'Near-unit mobility ratio — sharp, piston-like displacement.',
                    paramPatch: { mu_o: 0.5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'mu_unit',
                    label: 'μ_o = 1.0 cp (M ≈ 2)',
                    description: 'Standard BL Case A mobility — matches the base case exactly.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'mu_unfavorable',
                    label: 'μ_o = 5.0 cp (M ≈ 10)',
                    description: 'Strongly unfavorable — very early breakthrough, poor recovery.',
                    paramPatch: { mu_o: 5.0 },
                    affectsAnalytical: true,
                },
            ],
        },
    },

    {
        key: 'wf_corey_exponent',
        label: 'Corey n_o',
        description: 'The Corey exponent controls the curvature of k_ro, shifting both breakthrough timing and ultimate recovery. — Corey (1954)',
        scenarioClass: 'waterflood',
        chartPreset: 'waterflood',
        params: { ...BL_A_BASE_PARAMS },
        sensitivity: {
            key: 'corey_no',
            label: 'Oil Corey exponent (n_o)',
            description: 'n_o determines the shape of the oil relative permeability curve. Both simulation and analytical solution update.',
            variants: [
                {
                    key: 'no_15',
                    label: 'n_o = 1.5 (near-linear)',
                    description: 'Near-linear k_ro — oil transmits readily at intermediate saturations.',
                    paramPatch: { n_o: 1.5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'no_20',
                    label: 'n_o = 2.0 (base)',
                    description: 'Standard Corey exponent — matches the base case exactly.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'no_35',
                    label: 'n_o = 3.5 (convex)',
                    description: 'Strongly convex k_ro — oil permeability collapses quickly, earlier breakthrough.',
                    paramPatch: { n_o: 3.5 },
                    affectsAnalytical: true,
                },
            ],
        },
    },

    {
        key: 'wf_residual_oil',
        label: 'Residual Oil',
        description: 'S_or sets the ceiling on waterflood recovery. "The residual oil saturation is the most important variable affecting recovery efficiency." — Craig (1971)',
        scenarioClass: 'waterflood',
        chartPreset: 'waterflood',
        params: { ...BL_A_BASE_PARAMS, s_or: 0.15 },
        sensitivity: {
            key: 'sor',
            label: 'Residual oil saturation (S_or)',
            description: 'S_or directly sets the maximum recovery factor RF = (1 − S_wc − S_or) / (1 − S_wc). Both simulation and analytical update.',
            variants: [
                {
                    key: 'sor_low',
                    label: 'S_or = 0.05 (low trapping)',
                    description: 'Low residual trapping — high recovery ceiling.',
                    paramPatch: { s_or: 0.05 },
                    affectsAnalytical: true,
                },
                {
                    key: 'sor_mid',
                    label: 'S_or = 0.15 (base)',
                    description: 'Moderate residual oil — base case.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'sor_high',
                    label: 'S_or = 0.30 (high trapping)',
                    description: 'High residual trapping — poor recovery ceiling, typical of tight carbonates.',
                    paramPatch: { s_or: 0.30 },
                    affectsAnalytical: true,
                },
            ],
        },
    },

    {
        key: 'wf_capillary',
        label: 'Capillary',
        description: 'Capillary entry pressure diffuses the sharp BL shock front. Analytical reference stays sharp; deviation shows capillary spreading. — Rapoport & Leas (1953)',
        scenarioClass: 'waterflood',
        chartPreset: 'waterflood',
        params: { ...BL_A_BASE_PARAMS },
        sensitivity: {
            key: 'capillary',
            label: 'Capillary entry pressure (P_e)',
            description: 'Capillary pressure smears the saturation shock into a transition zone. The analytical BL reference remains sharp — deviation quantifies capillary dispersion.',
            variants: [
                {
                    key: 'cap_off',
                    label: 'P_e = 0 (disabled)',
                    description: 'No capillary pressure — sharp BL shock, analytical and simulation agree.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'cap_mild',
                    label: 'P_e = 0.3 bar (mild)',
                    description: 'Mild capillary entry pressure — slight front spreading.',
                    paramPatch: { capillaryEnabled: true, capillaryPEntry: 0.3, capillaryLambda: 2 },
                    affectsAnalytical: false,
                },
                {
                    key: 'cap_strong',
                    label: 'P_e = 1.5 bar (strong)',
                    description: 'Stronger capillary entry pressure — broad transition zone, front significantly smeared.',
                    paramPatch: { capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2 },
                    affectsAnalytical: false,
                },
            ],
        },
    },

    // ── Depletion ────────────────────────────────────────────────────────────

    {
        key: 'dep_dietz_center',
        label: 'Dietz Center',
        description: 'Square reservoir, central producer, C_A ≈ 30.88. "Well location and drainage area shape determine the shape factor." — Dietz (1965)',
        scenarioClass: 'depletion',
        chartPreset: 'depletion',
        params: {
            ...DIETZ_BASE_PARAMS,
            producerI: 10,
            producerJ: 10,
        },
    },

    {
        key: 'dep_dietz_corner',
        label: 'Dietz Corner',
        description: 'Square reservoir, corner producer, C_A ≈ 0.56. "Well location and drainage area shape determine the shape factor." — Dietz (1965)',
        scenarioClass: 'depletion',
        chartPreset: 'depletion',
        params: {
            ...DIETZ_BASE_PARAMS,
            producerI: 0,
            producerJ: 0,
        },
    },

    {
        key: 'dep_fetkovich',
        label: 'Fetkovich Decline',
        description: 'Constant-BHP exponential decline. "At long producing times, all finite-reservoir wells exhibit exponential decline." — Fetkovich (1971)',
        scenarioClass: 'depletion',
        chartPreset: 'fetkovich',
        params: { ...FETKOVICH_BASE_PARAMS },
    },

    {
        key: 'dep_skin',
        label: 'Skin',
        description: 'Skin modifies the near-wellbore pressure drop, scaling the productivity index and the decline rate. — Hawkins (1956)',
        scenarioClass: 'depletion',
        chartPreset: 'fetkovich',
        params: { ...FETKOVICH_BASE_PARAMS },
        sensitivity: {
            key: 'skin',
            label: 'Skin factor (s)',
            description: 'Positive skin represents formation damage; negative skin represents stimulation. Both analytical and simulation update.',
            variants: [
                {
                    key: 'skin_neg',
                    label: 's = −2 (stimulated)',
                    description: 'Mild stimulation (e.g. acid job) — PI ≈ 3× base, faster decline.',
                    paramPatch: { well_skin: -2 },
                    affectsAnalytical: true,
                },
                {
                    key: 'skin_zero',
                    label: 's = 0 (clean)',
                    description: 'No damage or stimulation — base case.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'skin_pos',
                    label: 's = +5 (damaged)',
                    description: 'Moderate formation damage (fines, mud filtrate) — reduced PI, slower decline.',
                    paramPatch: { well_skin: 5 },
                    affectsAnalytical: true,
                },
            ],
        },
    },

    {
        key: 'dep_permeability',
        label: 'Permeability',
        description: 'Permeability controls the productivity index and hence the decline time constant τ = V_p · c_t / PI. — Fetkovich (1971)',
        scenarioClass: 'depletion',
        chartPreset: 'fetkovich',
        params: { ...FETKOVICH_BASE_PARAMS },
        sensitivity: {
            key: 'permeability',
            label: 'Absolute permeability (k)',
            description: 'PI is proportional to k; τ is inversely proportional. Higher k → faster, steeper decline. Both analytical and simulation update.',
            variants: [
                {
                    key: 'perm_tight',
                    label: 'k = 5 mD (tight)',
                    description: 'Tight reservoir — low PI, slow decline, long-lived production.',
                    paramPatch: { uniformPermX: 5, uniformPermY: 5, uniformPermZ: 0.5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'perm_base',
                    label: 'k = 20 mD (base)',
                    description: 'Base Fetkovich permeability — matches the reference decline curve.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'perm_good',
                    label: 'k = 100 mD (good)',
                    description: 'Good reservoir — high PI, rapid initial rate, steep decline.',
                    paramPatch: { uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10 },
                    affectsAnalytical: true,
                },
            ],
        },
    },

    {
        key: 'dep_compressibility',
        label: 'Compressibility',
        description: 'Total compressibility governs reservoir storage — higher c_t extends the decline time constant τ = V_p · c_t / PI. — Craft & Hawkins (1959)',
        scenarioClass: 'depletion',
        chartPreset: 'fetkovich',
        params: { ...FETKOVICH_BASE_PARAMS },
        sensitivity: {
            key: 'compressibility',
            label: 'Oil compressibility (c_o)',
            description: 'c_o dominates total compressibility c_t for undersaturated oil. Higher c_t stores more energy and lengthens the decline. Both analytical and simulation update.',
            variants: [
                {
                    key: 'ct_low',
                    label: 'c_o = 5×10⁻⁶ bar⁻¹ (stiff)',
                    description: 'Stiff undersaturated oil — low storage, fast decline.',
                    paramPatch: { c_o: 5e-6 },
                    affectsAnalytical: true,
                },
                {
                    key: 'ct_base',
                    label: 'c_o = 1×10⁻⁵ bar⁻¹ (base)',
                    description: 'Typical black oil compressibility — base case.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'ct_high',
                    label: 'c_o = 5×10⁻⁵ bar⁻¹ (volatile)',
                    description: 'High compressibility (near bubble point or volatile oil) — large storage, extended decline.',
                    paramPatch: { c_o: 5e-5 },
                    affectsAnalytical: true,
                },
            ],
        },
    },

    // ─── Three-Phase Scenarios ─────────────────────────────────────────────────
    {
        key: '3p_gas_injection',
        label: 'Gas Injection',
        scenarioClass: '3phase',
        description: 'Gas injector displacing oil in a 1D homogeneous reservoir. No initial free gas.',
        chartPreset: 'depletion',
        params: {
            nx: 20, ny: 1, nz: 1,
            cellDx: 50, cellDy: 50, cellDz: 10,
            initialPressure: 250,
            initialSaturation: 0.2,
            initialGasSaturation: 0,
            reservoirPorosity: 0.2,
            uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10,
            permMode: 'uniform',
            s_wc: 0.2, s_or: 0.15,
            s_gc: 0.05, s_gr: 0.05,
            n_w: 2.0, n_o: 2.0, n_g: 1.5,
            k_rw_max: 0.4, k_ro_max: 1.0, k_rg_max: 0.8,
            mu_w: 0.5, mu_o: 2.0, mu_g: 0.02,
            c_w: 3e-6, c_o: 1e-5, c_g: 1e-4,
            rho_w: 1000, rho_o: 800, rho_g: 10,
            depth_reference: 0,
            volume_expansion_o: 1.0, volume_expansion_w: 1.0,
            rock_compressibility: 1e-6,
            injectorEnabled: true,
            injectorControlMode: 'pressure',
            producerControlMode: 'pressure',
            injectorBhp: 350, producerBhp: 100,
            injectorI: 0, injectorJ: 0,
            producerI: 19, producerJ: 0,
            well_radius: 0.1, well_skin: 0,
            capillaryEnabled: false,
            capillaryPEntry: 0, capillaryLambda: 2,
            gravityEnabled: false,
            threePhaseModeEnabled: true,
            injectedFluid: 'gas',
            pcogEnabled: false, pcogPEntry: 3, pcogLambda: 2,
            delta_t_days: 5,
            max_sat_change_per_step: 0.1,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
            analyticalSolutionMode: 'waterflood',
        },
    },
    {
        key: '3p_solution_gas_drive',
        label: 'Solution Gas Drive',
        scenarioClass: '3phase',
        description: 'Pressure depletion with initial free gas, no injector. Models a reservoir below bubble point.',
        chartPreset: 'depletion',
        params: {
            nx: 20, ny: 1, nz: 1,
            cellDx: 50, cellDy: 50, cellDz: 10,
            initialPressure: 200,
            initialSaturation: 0.2,
            initialGasSaturation: 0.08,
            reservoirPorosity: 0.2,
            uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10,
            permMode: 'uniform',
            s_wc: 0.2, s_or: 0.15,
            s_gc: 0.05, s_gr: 0.05,
            n_w: 2.0, n_o: 2.0, n_g: 1.5,
            k_rw_max: 0.4, k_ro_max: 1.0, k_rg_max: 0.8,
            mu_w: 0.5, mu_o: 2.0, mu_g: 0.02,
            c_w: 3e-6, c_o: 1e-5, c_g: 1e-4,
            rho_w: 1000, rho_o: 800, rho_g: 10,
            depth_reference: 0,
            volume_expansion_o: 1.0, volume_expansion_w: 1.0,
            rock_compressibility: 1e-6,
            injectorEnabled: false,
            injectorControlMode: 'pressure',
            producerControlMode: 'pressure',
            injectorBhp: 350, producerBhp: 100,
            injectorI: 0, injectorJ: 0,
            producerI: 19, producerJ: 0,
            well_radius: 0.1, well_skin: 0,
            capillaryEnabled: false,
            capillaryPEntry: 0, capillaryLambda: 2,
            gravityEnabled: false,
            threePhaseModeEnabled: true,
            injectedFluid: 'gas',
            pcogEnabled: false, pcogPEntry: 3, pcogLambda: 2,
            delta_t_days: 10,
            max_sat_change_per_step: 0.1,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
            analyticalSolutionMode: 'depletion',
        },
    },
];

// ─── Lookup helpers ───────────────────────────────────────────────────────────

const scenarioMap = new Map(SCENARIOS.map((s) => [s.key, s]));

export function getScenario(key: string | null | undefined): Scenario | null {
    if (!key) return null;
    return scenarioMap.get(key) ?? null;
}

/** Returns the full base params for a scenario, or {} if not found. */
export function getScenarioParams(key: string | null | undefined): Record<string, unknown> {
    return getScenario(key)?.params ?? {};
}

/**
 * Returns the full params for a scenario + variant combination.
 * Merges the variant's paramPatch on top of the scenario base params.
 * If variantKey is null/undefined, returns the base scenario params.
 */
export function getScenarioWithVariantParams(
    scenarioKey: string,
    variantKey: string | null | undefined,
): Record<string, unknown> {
    const scenario = getScenario(scenarioKey);
    if (!scenario) return {};
    if (!variantKey) return scenario.params;
    const variant = scenario.sensitivity?.variants.find((v) => v.key === variantKey);
    if (!variant) return scenario.params;
    return { ...scenario.params, ...variant.paramPatch };
}

/** Returns the chart layout config for a preset key, or {} if not found. */
export function getChartPreset(presetKey: string | null | undefined): RateChartLayoutConfig {
    return CHART_PRESETS[presetKey ?? ''] ?? {};
}

export function listScenarios(): Scenario[] {
    return SCENARIOS;
}
