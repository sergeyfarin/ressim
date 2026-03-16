/**
 * scenarios.ts — single source of truth for all predefined simulation scenarios.
 *
 * Replaces the combination of presetCases.ts + benchmarkCases.ts + caseCatalog.ts + caseLibrary.ts.
 * Each scenario is self-contained: full param set + chart preset + optional sensitivity definition.
 */

import type { RateChartLayoutConfig } from '../charts/rateChartLayoutConfig';

// ─── Types ────────────────────────────────────────────────────────────────────

export type ScenarioClass = 'waterflood' | 'depletion';

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
                description: 'Coarse 24-cell 1D grid. Large numerical diffusion expected — early watercut and smeared front.',
                paramPatch: { nx: 24, producerI: 23, cellDx: 40 },
                affectsAnalytical: false,
            },
            {
                key: 'grid_48',
                label: '48 cells (medium)',
                description: 'Intermediate 48-cell 1D grid. Reduced numerical diffusion compared to 24 cells.',
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
        description: 'Buckley-Leverett 1D waterflood with favorable mobility ratio (M ≈ 2, mu_o=1 cp). Compare numerical solution to the BL analytical shock.',
        scenarioClass: 'waterflood',
        chartPreset: 'waterflood',
        params: { ...BL_A_BASE_PARAMS },
        sensitivity: buildGridRefinementSensitivity(),
    },

    {
        key: 'wf_bl_case_b',
        label: 'BL Case B',
        description: 'Buckley-Leverett 1D waterflood with mild unfavorable mobility (mu_o=1.4, mu_w=0.6). Earlier breakthrough than Case A.',
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
        description: 'Waterflood with three oil viscosity values. Both the simulation and the analytical fractional flow solution change across variants — see all three curves on one chart.',
        scenarioClass: 'waterflood',
        chartPreset: 'waterflood',
        params: { ...BL_A_BASE_PARAMS },
        sensitivity: {
            key: 'mobility',
            label: 'Oil viscosity (mu_o)',
            description: 'Vary oil viscosity to explore how mobility ratio affects breakthrough, watercut shape, and ultimate recovery. The analytical BL solution updates for each variant.',
            variants: [
                {
                    key: 'mu_favorable',
                    label: 'mu_o = 0.5 cp (M ≈ 1)',
                    description: 'Near-unit mobility ratio — sharp, piston-like displacement. Both simulation and analytical update.',
                    paramPatch: { mu_o: 0.5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'mu_unit',
                    label: 'mu_o = 1.0 cp (M ≈ 2)',
                    description: 'Standard BL Case A mobility — matches the base case exactly.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'mu_unfavorable',
                    label: 'mu_o = 5.0 cp (M ≈ 10)',
                    description: 'Strongly unfavorable — very early breakthrough, poor recovery. Both simulation and analytical update.',
                    paramPatch: { mu_o: 5.0 },
                    affectsAnalytical: true,
                },
            ],
        },
    },

    // ── Depletion ────────────────────────────────────────────────────────────

    {
        key: 'dep_dietz_center',
        label: 'Dietz Center',
        description: 'Pressure depletion in a 21×21 square reservoir with central producer. Shape factor C_A ≈ 30.88 (Dietz, 1965).',
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
        description: 'Pressure depletion in a 21×21 square reservoir with corner producer. Shape factor C_A ≈ 0.56 (Dietz, 1965).',
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
        description: 'Constant BHP exponential decline in a 1D reservoir with high initial pressure. Rate vs time follows the Fetkovich exponential type curve.',
        scenarioClass: 'depletion',
        chartPreset: 'fetkovich',
        params: {
            ...DIETZ_BASE_PARAMS,
            // Override grid for 1D Fetkovich geometry
            nx: 48,
            ny: 1,
            nz: 1,
            cellDx: 10,
            cellDy: 10,
            cellDz: 10,
            // Low permeability and high initial pressure drive exponential decline
            uniformPermX: 20,
            uniformPermY: 20,
            uniformPermZ: 2,
            producerBhp: 50,
            initialPressure: 1500,
            steps: 100,
            delta_t_days: 5,
            // Corner producer in 1D
            producerI: 47,
            producerJ: 0,
        },
    },

    // TODO: additional scenarios to add in future iterations
    // - wf_bl_case_a_capillary: BL Case A with mild capillary pressure sensitivity
    // - wf_bl_case_a_timestep: BL Case A with timestep refinement (dt = 0.25 / 0.5 / 1.0 days)
    // - wf_bl_layered: BL Case A with mild vs strong layered heterogeneity
    // - dep_dietz_center_perforations: Dietz center with partial perforation sensitivity
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
