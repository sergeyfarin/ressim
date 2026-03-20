/**
 * scenarios.ts — single source of truth for all predefined simulation scenarios.
 *
 * Replaces the combination of presetCases.ts + benchmarkCases.ts + caseCatalog.ts + caseLibrary.ts.
 *
 * Each scenario is self-contained: explicit full param set (no shared mutable base objects),
 * chart preset, and an array of sensitivity dimensions. Each dimension holds named variants
 * that can be toggled on/off for a sensitivity sweep.
 *
 * Key naming convention:
 *   {domain}_{physics_descriptor}
 *   e.g. wf_bl1d, sweep_areal, dep_pss, dep_decline
 *
 * Sensitivity dimension keys: lower_snake of parameter name (mobility, corey_no, sor, …)
 * Sensitivity variant keys:   {dim_abbrev}_{value_tag} (e.g. mob_favorable, sor_low)
 */

import type { RateChartLayoutConfig } from '../charts/rateChartLayoutConfig';

// ─── Types ────────────────────────────────────────────────────────────────────

/** Physical classification — drives analytical solution routing and mode toggles. */
export type ScenarioClass = 'waterflood' | 'depletion' | '3phase';

/**
 * UI domain — controls which tab a scenario appears under in ScenarioPicker.
 * Separate from ScenarioClass so that 'sweep' scenarios can share the 'waterflood'
 * analytical path while displaying under their own domain tab.
 */
export type ScenarioDomain = 'waterflood' | 'sweep' | 'depletion' | 'gas';

export type SensitivityVariant = {
    key: string;
    label: string;
    description: string;
    /** Parameters merged on top of the scenario base params for this variant. */
    paramPatch: Record<string, unknown>;
    /**
     * True  → this variant changes a parameter that feeds the analytical solution
     *         (e.g. mu_o changes fractional flow → both sim and analytical update).
     * False → the analytical solution is independent of this parameter; only the
     *         simulation result changes (e.g. grid refinement, layer heterogeneity).
     */
    affectsAnalytical: boolean;
    /**
     * Whether this variant is pre-selected when the dimension loads.
     * Omit or set true for the normal case; set false to make a variant available
     * but not run by default (useful for extreme or slow cases in large dimensions).
     */
    enabledByDefault?: boolean;
};

export type SensitivityDimension = {
    key: string;
    label: string;
    description: string;
    variants: SensitivityVariant[];
    /**
     * Override the scenario's chartPreset when this dimension is active.
     * Useful when a dimension (e.g. grid convergence) benefits from a different
     * default view than the scenario's primary chart.
     */
    chartPresetOverride?: string;
};

export type Scenario = {
    key: string;
    label: string;
    description: string;
    analyticalMethodSummary: string;
    analyticalMethodReference: string;
    /** Physical classification — drives analytical solution routing. */
    scenarioClass: ScenarioClass;
    /** UI domain — controls which tab this scenario appears under. */
    domain: ScenarioDomain;
    /** Complete, self-contained simulator parameter set. No shared base objects. */
    params: Record<string, unknown>;
    /** Key into CHART_PRESETS — controls default x-axis, panels, and curve selection. */
    chartPreset: string;
    /**
     * Sensitivity dimensions available for this scenario.
     * Empty array = no sensitivity study defined.
     * First element is the default dimension shown on load unless
     * defaultSensitivityDimensionKey is set.
     */
    sensitivities: SensitivityDimension[];
    /**
     * Key of the dimension to activate when the scenario is first selected.
     * Defaults to sensitivities[0].key if omitted.
     */
    defaultSensitivityDimensionKey?: string;
};

// ─── Chart presets ────────────────────────────────────────────────────────────

export const CHART_PRESETS: Record<string, RateChartLayoutConfig> = {
    /**
     * 1D Buckley-Leverett waterflood: PVI x-axis, one variable per panel.
     * Rates → water cut  |  Recovery → RF  |  Cum Oil (collapsed)  |  Pressure (collapsed)
     */
    waterflood: {
        rateChart: {
            xAxisMode: 'pvi',
            xAxisOptions: ['pvi', 'time', 'cumInjection'],
            allowLogScale: false,
            logScale: false,
            ratesExpanded: true,
            recoveryExpanded: true,
            cumulativeExpanded: false,
            diagnosticsExpanded: false,
            panels: {
                rates: {
                    title: 'Breakthrough',
                    curveKeys: ['water-cut-sim', 'water-cut-reference'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                },
                volumes: {
                    title: 'Cum Injection',
                    curveKeys: ['cum-injection'],
                    scalePreset: 'cumulative_volumes',
                },
                oil_rate: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    scalePreset: 'rates',
                },
            },
        },
    },

    /**
     * Sweep efficiency scenarios: all standard panels collapsed — the sweep-specific panels
     * (RF sweep, E_A, E_V, E_vol) are the primary display when showSweepPanel is active.
     */
    sweep: {
        rateChart: {
            xAxisMode: 'pvi',
            xAxisOptions: ['pvi', 'time'],
            allowLogScale: false,
            logScale: false,
            ratesExpanded: false,
            recoveryExpanded: false,
            cumulativeExpanded: false,
            diagnosticsExpanded: false,
            panels: {
                rates: {
                    title: 'Breakthrough (1D BL)',
                    curveKeys: ['water-cut-sim', 'water-cut-reference'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                },
            },
        },
    },

    /**
     * Oil depletion (PSS/transient): time x-axis, oil rate + RF + pressure panels.
     * Analytical reference solution shown alongside simulation.
     */
    oil_depletion: {
        rateChart: {
            xAxisMode: 'time',
            xAxisOptions: ['time', 'tD', 'logTime'],
            allowLogScale: true,
            logScale: false,
            ratesExpanded: true,
            recoveryExpanded: true,
            cumulativeExpanded: false,
            diagnosticsExpanded: true,
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
                    scalePreset: 'pressure',
                },
            },
        },
    },

    /**
     * Fetkovich exponential decline: log-time x-axis, log-scale rates.
     * Same panel structure as oil_depletion.
     */
    fetkovich: {
        rateChart: {
            xAxisMode: 'logTime',
            xAxisOptions: ['logTime', 'time', 'tD'],
            allowLogScale: true,
            logScale: true,
            ratesExpanded: true,
            recoveryExpanded: true,
            cumulativeExpanded: false,
            diagnosticsExpanded: true,
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference'],
                    scalePreset: 'pressure',
                },
            },
        },
    },

    /**
     * Gas-domain scenarios: no validated analytical reference yet.
     * Same panel structure as oil_depletion but without analytical reference curves.
     */
    gas: {
        rateChart: {
            xAxisMode: 'time',
            xAxisOptions: ['time', 'logTime'],
            allowLogScale: true,
            logScale: false,
            ratesExpanded: true,
            recoveryExpanded: true,
            cumulativeExpanded: false,
            diagnosticsExpanded: true,
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                },
            },
        },
    },
};

// ─── Scenarios ────────────────────────────────────────────────────────────────
//
// Each scenario uses an explicit, self-contained params object.
// No shared mutable base objects — mutations to one scenario cannot affect another.
//
// Params objects are frozen at the bottom of this file to catch accidental
// runtime mutation early (see Object.freeze calls near SCENARIOS definition).

export const SCENARIOS: Scenario[] = [

    // ════════════════════════════════════════════════════════════════════════
    // WATERFLOOD DOMAIN — 1D Buckley-Leverett
    // ════════════════════════════════════════════════════════════════════════

    {
        key: 'wf_bl1d',
        label: '1D Waterflood',
        description: 'Viscous-dominated 1D displacement — no gravity or capillary pressure. The numerical shock front sharpens toward the analytical solution as grid resolution increases.',
        analyticalMethodSummary: 'Fractional-flow solution with Welge shock construction — predicts breakthrough timing and post-breakthrough recovery, independent of grid resolution.',
        analyticalMethodReference: 'Buckley and Leverett (1942); Welge (1952).',
        scenarioClass: 'waterflood',
        domain: 'waterflood',
        chartPreset: 'waterflood',
        defaultSensitivityDimensionKey: 'mobility',
        params: {
            analyticalSolutionMode: 'waterflood',
            // Fluid properties
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
            // Capillary pressure (off by default)
            capillaryEnabled: false,
            capillaryPEntry: 0,
            capillaryLambda: 2,
            // Grid: 96-cell 1D slab, 960 m total length
            nx: 96,
            ny: 1,
            nz: 1,
            cellDx: 10,
            cellDy: 10,
            cellDz: 1,
            permMode: 'uniform',
            uniformPermX: 2000,
            uniformPermY: 2000,
            uniformPermZ: 200,
            // Initial conditions
            initialPressure: 300,
            initialSaturation: 0.1,
            // Wells: pressure-controlled injector + producer
            injectorEnabled: true,
            injectorControlMode: 'pressure',
            producerControlMode: 'pressure',
            injectorBhp: 500,
            producerBhp: 100,
            targetInjectorRate: 0,
            targetProducerRate: 0,
            injectorI: 0,
            injectorJ: 0,
            producerI: 95,
            producerJ: 0,
            well_radius: 0.1,
            well_skin: 0,
            // Numerics
            delta_t_days: 0.125,
            steps: 240,
            max_sat_change_per_step: 0.05,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
            // Physics toggles
            gravityEnabled: false,
        },
        sensitivities: [
            {
                key: 'mobility',
                label: 'Mobility Ratio',
                description: 'Vary oil viscosity to explore how the end-point mobility ratio M = (k_rw/μ_w)/(k_ro/μ_o) controls breakthrough timing, watercut shape, and recovery. Both simulation and analytical solution update.',
                variants: [
                    {
                        key: 'mob_favorable',
                        label: 'M ≈ 1  (μ_o = 0.5 cp)',
                        description: 'Near-unit mobility ratio — sharp, piston-like displacement. Best waterflood recovery.',
                        paramPatch: { mu_o: 0.5 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'mob_base',
                        label: 'M ≈ 2  (μ_o = 1.0 cp)',
                        description: 'Base case — moderate unfavourable mobility. BL shock at S_w ≈ 0.55.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'mob_case_b',
                        label: 'M ≈ 2.3  (BL Case B)',
                        description: 'BL Case B — adverse mobility with tighter rock (μ_o=1.4, μ_w=0.6, n_w=2.2, s_wc=s_or=0.15). Earlier breakthrough than base.',
                        paramPatch: { mu_w: 0.6, mu_o: 1.4, initialSaturation: 0.15, s_wc: 0.15, s_or: 0.15, n_w: 2.2 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'mob_unfavorable',
                        label: 'M ≈ 10  (μ_o = 5.0 cp)',
                        description: 'Strongly unfavourable — very early breakthrough, poor recovery. Classic viscous fingering regime.',
                        paramPatch: { mu_o: 5.0, delta_t_days: 0.25 },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'corey_no',
                label: 'Corey Exponent n_o',
                description: 'The Corey exponent controls the curvature of k_ro(S_w). Higher n_o → more convex k_ro → oil transmits less readily at intermediate saturations. Both simulation and analytical update.',
                variants: [
                    {
                        key: 'no_15',
                        label: 'n_o = 1.5  (near-linear)',
                        description: 'Near-linear k_ro — oil transmits readily at intermediate saturations, slower BL shock.',
                        paramPatch: { n_o: 1.5 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'no_20',
                        label: 'n_o = 2.0  (base)',
                        description: 'Standard Corey exponent — base case.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'no_35',
                        label: 'n_o = 3.5  (convex)',
                        description: 'Strongly convex k_ro — oil permeability collapses quickly near residual, earlier breakthrough.',
                        paramPatch: { n_o: 3.5 },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'sor',
                label: 'Residual Oil  S_or',
                description: 'S_or sets the ceiling on waterflood recovery: RF_max = (1 − S_wc − S_or)/(1 − S_wc). Also shifts the fractional flow curve endpoint. Both simulation and analytical update.',
                variants: [
                    {
                        key: 'sor_low',
                        label: 'S_or = 0.05  (low trapping)',
                        description: 'Low residual trapping — high recovery ceiling, typical of clean sandstone.',
                        paramPatch: { s_or: 0.05 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'sor_mid',
                        label: 'S_or = 0.15  (base)',
                        description: 'Moderate residual oil — base case.',
                        paramPatch: { s_or: 0.15 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'sor_high',
                        label: 'S_or = 0.30  (high trapping)',
                        description: 'High residual trapping — poor recovery ceiling, typical of carbonates or tight rock.',
                        paramPatch: { s_or: 0.30 },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'capillary',
                label: 'Capillary Pressure',
                description: 'Capillary pressure diffuses the sharp BL shock front. The analytical solution stays sharp — the gap between them quantifies capillary spreading. The analytical solution does not include capillary effects.',
                variants: [
                    {
                        key: 'cap_off',
                        label: 'P_e = 0  (disabled)',
                        description: 'No capillary pressure — sharp BL shock; simulation and analytical agree.',
                        paramPatch: {},
                        affectsAnalytical: false,
                    },
                    {
                        key: 'cap_mild',
                        label: 'P_e = 0.3 bar  (mild)',
                        description: 'Mild capillary entry pressure — slight front spreading. — Rapoport & Leas (1953)',
                        paramPatch: { capillaryEnabled: true, capillaryPEntry: 0.3, capillaryLambda: 2 },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'cap_strong',
                        label: 'P_e = 1.5 bar  (strong)',
                        description: 'Strong capillary pressure — broad transition zone, front significantly smeared.',
                        paramPatch: { capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2 },
                        affectsAnalytical: false,
                    },
                ],
            },
            {
                key: 'grid',
                label: 'Grid Resolution',
                description: 'Numerical convergence study. The Buckley-Leverett analytical solution is grid-independent — only the simulation changes as the grid is refined.',
                chartPresetOverride: 'waterflood',
                variants: [
                    {
                        key: 'grid_3',
                        label: '3 cells  (coarse)',
                        description: 'Coarse 3-cell grid — large numerical diffusion, smeared front.',
                        paramPatch: { nx: 3, producerI: 2, cellDx: 320 },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'grid_6',
                        label: '6 cells  (coarse)',
                        description: 'Coarse 6-cell grid — large numerical diffusion, smeared front.',
                        paramPatch: { nx: 6, producerI: 5, cellDx: 160 },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'grid_24',
                        label: '24 cells  (coarse)',
                        description: 'Coarse 24-cell grid — large numerical diffusion, smeared front.',
                        paramPatch: { nx: 24, producerI: 23, cellDx: 40 },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'grid_48',
                        label: '48 cells  (medium)',
                        description: 'Intermediate 48-cell grid — reduced numerical diffusion.',
                        paramPatch: { nx: 48, producerI: 47, cellDx: 20 },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'grid_96',
                        label: '96 cells  (fine)',
                        description: 'Fine 96-cell reference grid — matches the base case exactly.',
                        paramPatch: {},
                        affectsAnalytical: false,
                    },
                ],
            },
        ],
    },

    // ════════════════════════════════════════════════════════════════════════
    // SWEEP DOMAIN — Areal (2D XY), Vertical (2D XZ), Combined (3D)
    // ════════════════════════════════════════════════════════════════════════

    {
        key: 'sweep_areal',
        label: 'Areal Sweep (XY)',
        description: 'Five-spot pattern flood in 2D (XY). Areal sweep E_A at breakthrough is strongly controlled by end-point mobility ratio: E_A(BT) ≈ 0.70 at M = 1, dropping sharply for unfavourable M > 1.',
        analyticalMethodSummary: 'Craig five-spot correlation — predicts E_A vs PVI for a homogeneous pattern. Heterogeneous variants show the additional sweep penalty on top of this baseline.',
        analyticalMethodReference: 'Craig (1971); Dyes, Caudle, and Erickson (1954).',
        scenarioClass: 'waterflood',
        domain: 'sweep',
        chartPreset: 'sweep',
        defaultSensitivityDimensionKey: 'mobility',
        params: {
            analyticalSolutionMode: 'waterflood',
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
            // Rock / rel perm
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
            // Grid: 21×21×1 five-spot, 420 m × 420 m × 10 m
            nx: 21,
            ny: 21,
            nz: 1,
            cellDx: 20,
            cellDy: 20,
            cellDz: 10,
            permMode: 'uniform',
            uniformPermX: 200,
            uniformPermY: 200,
            uniformPermZ: 20,
            // Initial conditions
            initialPressure: 300,
            initialSaturation: 0.1,
            // Wells: injector at origin, producer at far corner
            injectorEnabled: true,
            injectorControlMode: 'pressure',
            producerControlMode: 'pressure',
            injectorBhp: 500,
            producerBhp: 100,
            targetInjectorRate: 0,
            targetProducerRate: 0,
            injectorI: 0,
            injectorJ: 0,
            producerI: 20,
            producerJ: 20,
            well_radius: 0.1,
            well_skin: 0,
            // Numerics
            delta_t_days: 0.5,
            steps: 120,
            max_sat_change_per_step: 0.05,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
            gravityEnabled: false,
        },
        sensitivities: [
            {
                key: 'mobility',
                label: 'Mobility Ratio',
                description: 'End-point mobility ratio M = (k_rw_max/μ_w)/(k_ro_max/μ_o) is the primary control on five-spot areal sweep efficiency. Both simulation and analytical (Craig polynomial) update.',
                variants: [
                    {
                        key: 'mob_favorable',
                        label: 'M ≈ 0.5  (μ_o = 0.25 cp)',
                        description: 'Favourable mobility — high areal sweep, late breakthrough, piston-like.',
                        paramPatch: { mu_o: 0.25 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'mob_unit',
                        label: 'M ≈ 2  (μ_o = 1.0 cp)',
                        description: 'Moderate mobility ratio — base case. E_A(BT) ≈ 0.58.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'mob_unfavorable',
                        label: 'M ≈ 10  (μ_o = 5.0 cp)',
                        description: 'Strongly unfavourable — viscous channelling, early breakthrough, poor areal sweep.',
                        paramPatch: { mu_o: 5.0 },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'areal_heterogeneity',
                label: 'Areal Heterogeneity',
                description: 'Introduce seeded within-layer permeability variation to test areal sweep sensitivity beyond the Craig five-spot correlation. Simulation changes, while the analytical areal sweep curve remains shared context because the current model does not resolve within-layer randomness.',
                variants: [
                    {
                        key: 'areal_uniform',
                        label: 'Uniform  (baseline pattern)',
                        description: 'Uniform permeability in the XY plane — clean Craig-style five-spot baseline.',
                        paramPatch: {},
                        affectsAnalytical: false,
                    },
                    {
                        key: 'areal_mild_random',
                        label: 'Mild random  (seeded)',
                        description: 'Moderate within-layer permeability variation — mild channelling and pattern distortion.',
                        paramPatch: {
                            permMode: 'random',
                            minPerm: 120,
                            maxPerm: 280,
                            useRandomSeed: true,
                            randomSeed: 4201,
                        },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'areal_strong_random',
                        label: 'Strong random  (seeded)',
                        description: 'Strong within-layer variation — pronounced areal bypassing and early preferential flow paths.',
                        paramPatch: {
                            permMode: 'random',
                            minPerm: 40,
                            maxPerm: 500,
                            useRandomSeed: true,
                            randomSeed: 4202,
                        },
                        affectsAnalytical: false,
                    },
                ],
            },
            {
                key: 'sor',
                label: 'Residual Oil  S_or',
                description: 'S_or affects the endpoint mobility ratio (k_ro endpoint shifts with S_or), influencing both displacement efficiency and areal sweep. Both simulation and analytical update.',
                variants: [
                    {
                        key: 'sor_low',
                        label: 'S_or = 0.05',
                        description: 'Very low trapping — nearly complete displacement, high recovery ceiling.',
                        paramPatch: { s_or: 0.05 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'sor_base',
                        label: 'S_or = 0.10  (base)',
                        description: 'Moderate residual — standard endpoint.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'sor_high',
                        label: 'S_or = 0.25',
                        description: 'High trapping — tight carbonate-like, reduced displacement efficiency.',
                        paramPatch: { s_or: 0.25 },
                        affectsAnalytical: true,
                    },
                ],
            },
        ],
    },

    {
        key: 'sweep_vertical',
        label: 'Vertical Sweep (XZ)',
        description: 'Non-communicating layered reservoir (XZ). Higher permeability contrast V_DP causes earlier breakthrough in faster layers, reducing vertical sweep E_V.',
        analyticalMethodSummary: 'Dykstra-Parsons layered model with BL displacement — predicts per-layer breakthrough and combined E_V for the active layer permeabilities.',
        analyticalMethodReference: 'Dykstra and Parsons (1950); Buckley and Leverett (1942); Welge (1952).',
        scenarioClass: 'waterflood',
        domain: 'sweep',
        chartPreset: 'sweep',
        defaultSensitivityDimensionKey: 'heterogeneity',
        params: {
            analyticalSolutionMode: 'waterflood',
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
            // Rock / rel perm
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
            // Grid: 5-layer 1D slab, 48 cells × 5 layers, 960 m × 20 m × 20 m
            // Base case: moderate heterogeneity V_DP ≈ 0.5
            nx: 48,
            ny: 1,
            nz: 5,
            cellDx: 20,
            cellDy: 20,
            cellDz: 4,
            permMode: 'perLayer',
            uniformPermX: 100,
            uniformPermY: 100,
            uniformPermZ: 10,
            layerPermsX: [200, 150, 100, 60, 40],
            layerPermsY: [200, 150, 100, 60, 40],
            layerPermsZ: [20, 15, 10, 6, 4],
            // Initial conditions
            initialPressure: 300,
            initialSaturation: 0.1,
            // Wells
            injectorEnabled: true,
            injectorControlMode: 'pressure',
            producerControlMode: 'pressure',
            injectorBhp: 500,
            producerBhp: 100,
            targetInjectorRate: 0,
            targetProducerRate: 0,
            injectorI: 0,
            injectorJ: 0,
            producerI: 47,
            producerJ: 0,
            well_radius: 0.1,
            well_skin: 0,
            // Numerics
            delta_t_days: 0.25,
            steps: 200,
            max_sat_change_per_step: 0.05,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
            gravityEnabled: false,
        },
        sensitivities: [
            {
                key: 'heterogeneity',
                label: 'Layer Heterogeneity  V_DP',
                description: 'Compare uniform, moderate, and extreme permeability contrasts across layers. The Dykstra-Parsons analytical model uses the active layer permeabilities, so both simulation and analytical sweep overlays update.',
                variants: [
                    {
                        key: 'vdp_uniform',
                        label: 'V_DP ≈ 0  (uniform)',
                        description: 'All layers equal permeability — all fronts advance simultaneously. E_V = 1 at breakthrough.',
                        paramPatch: {
                            permMode: 'uniform',
                            uniformPermX: 100,
                            uniformPermY: 100,
                            uniformPermZ: 10,
                        },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'vdp_moderate',
                        label: 'V_DP ≈ 0.5  (moderate)',
                        description: '5:1 permeability range — typical clastic reservoir. Base case.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'vdp_extreme',
                        label: 'V_DP ≈ 0.8  (extreme)',
                        description: '50:1 permeability range — thief zone dominates; very early breakthrough in the fastest layer.',
                        paramPatch: {
                            layerPermsX: [500, 150, 50, 20, 10],
                            layerPermsY: [500, 150, 50, 20, 10],
                            layerPermsZ: [50, 15, 5, 2, 1],
                        },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'mobility',
                label: 'Mobility Ratio',
                description: 'Mobility ratio also affects vertical sweep efficiency — unfavourable mobility worsens E_V in layered reservoirs. Both simulation and analytical update.',
                variants: [
                    {
                        key: 'mob_favorable',
                        label: 'M ≈ 1  (μ_o = 0.5 cp)',
                        description: 'Near-unit mobility — best possible sweep for a given heterogeneity.',
                        paramPatch: { mu_o: 0.5 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'mob_base',
                        label: 'M ≈ 2  (μ_o = 1.0 cp)',
                        description: 'Moderate mobility ratio — base case.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'mob_unfavorable',
                        label: 'M ≈ 10  (μ_o = 5.0 cp)',
                        description: 'Strongly unfavourable — compounds the heterogeneity penalty on sweep.',
                        paramPatch: { mu_o: 5.0 },
                        affectsAnalytical: true,
                    },
                ],
            },
        ],
    },

    {
        key: 'sweep_combined',
        label: 'Combined Sweep (3D)',
        description: 'Volumetric sweep E_vol = E_A × E_V in a 3D five-spot-over-layers flood. Two axes: a 2×2 interaction matrix of mobility vs. layering, and a progressive sweep from ideal to fully-degraded conditions.',
        analyticalMethodSummary: 'Factorized sweep model: Craig areal × Dykstra-Parsons vertical × BL displacement, linked via the local-PVI approximation.',
        analyticalMethodReference: 'Craig (1971); Dykstra and Parsons (1950); Buckley and Leverett (1942); Welge (1952).',
        scenarioClass: 'waterflood',
        domain: 'sweep',
        chartPreset: 'sweep',
        defaultSensitivityDimensionKey: 'interaction_core',
        params: {
            analyticalSolutionMode: 'waterflood',
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
            // Rock / rel perm
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
            // Grid: 21×21×5 five-spot + 5 layers, 420 m × 420 m × 20 m
            // Base: moderate heterogeneity (V_DP ≈ 0.5) + moderate mobility (M ≈ 2)
            nx: 21,
            ny: 21,
            nz: 5,
            cellDx: 20,
            cellDy: 20,
            cellDz: 4,
            permMode: 'perLayer',
            uniformPermX: 100,
            uniformPermY: 100,
            uniformPermZ: 10,
            layerPermsX: [200, 150, 100, 60, 40],
            layerPermsY: [200, 150, 100, 60, 40],
            layerPermsZ: [20, 15, 10, 6, 4],
            // Initial conditions
            initialPressure: 300,
            initialSaturation: 0.1,
            // Wells
            injectorEnabled: true,
            injectorControlMode: 'pressure',
            producerControlMode: 'pressure',
            injectorBhp: 500,
            producerBhp: 100,
            targetInjectorRate: 0,
            targetProducerRate: 0,
            injectorI: 0,
            injectorJ: 0,
            producerI: 20,
            producerJ: 20,
            well_radius: 0.1,
            well_skin: 0,
            // Numerics
            delta_t_days: 0.5,
            steps: 120,
            max_sat_change_per_step: 0.05,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
            gravityEnabled: false,
        },
        sensitivities: [
            {
                key: 'interaction_core',
                label: 'Mobility × Vertical Heterogeneity',
                description: 'Minimal 2 × 2 interaction map for 3D sweep. Separate mobility-only, layering-only, and compounded penalties without introducing areal randomness.',
                variants: [
                    {
                        key: 'interaction_favorable_uniform',
                        label: 'Favorable + uniform',
                        description: 'Near-piston 3D baseline: favorable mobility and no vertical heterogeneity.',
                        paramPatch: {
                            mu_o: 0.5,
                            permMode: 'uniform',
                            uniformPermX: 100,
                            uniformPermY: 100,
                            uniformPermZ: 10,
                        },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'interaction_unfavorable_uniform',
                        label: 'Unfavorable + uniform',
                        description: 'Mobility penalty only: poor mobility with otherwise uniform layering.',
                        paramPatch: {
                            mu_o: 5.0,
                            permMode: 'uniform',
                            uniformPermX: 100,
                            uniformPermY: 100,
                            uniformPermZ: 10,
                        },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'interaction_favorable_layered',
                        label: 'Favorable + layered',
                        description: 'Vertical heterogeneity penalty only: good mobility in a layered reservoir.',
                        paramPatch: { mu_o: 0.5 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'interaction_unfavorable_layered',
                        label: 'Unfavorable + layered',
                        description: 'Compounded mobility plus vertical layering penalty in the same 3D flood.',
                        paramPatch: { mu_o: 5.0 },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'sweep_ladder',
                label: 'Ideal to Worst',
                description: 'Progressive sweep comparison from ideal to fully degraded: starts with uniform permeability and favorable mobility, then adds vertical heterogeneity, full-field randomness, and finally unfavorable mobility. Random-heterogeneity variants are simulation-focused; the analytical sweep curve remains shared context.',
                variants: [
                    {
                        key: 'ladder_ideal',
                        label: 'Ideal  (uniform, favorable)',
                        description: 'Best-case 3D sweep: uniform permeability and favorable mobility.',
                        paramPatch: {
                            mu_o: 0.5,
                            permMode: 'uniform',
                            uniformPermX: 100,
                            uniformPermY: 100,
                            uniformPermZ: 10,
                        },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'ladder_vertical',
                        label: 'Vertical only  (layered, favorable)',
                        description: 'First degradation step: layered vertical heterogeneity with favorable mobility retained.',
                        paramPatch: { mu_o: 0.5 },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'ladder_full_het',
                        label: 'Vertical + areal heterogeneity',
                        description: 'Seeded full-field random permeability to approximate simultaneous areal and vertical non-uniformity at moderate mobility.',
                        paramPatch: {
                            permMode: 'random',
                            minPerm: 40,
                            maxPerm: 500,
                            useRandomSeed: true,
                            randomSeed: 4301,
                        },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'ladder_worst',
                        label: 'Worst case  (full heterogeneity, unfavorable)',
                        description: 'Fully degraded: full-field heterogeneity combined with strongly unfavorable mobility.',
                        paramPatch: {
                            mu_o: 5.0,
                            permMode: 'random',
                            minPerm: 20,
                            maxPerm: 700,
                            useRandomSeed: true,
                            randomSeed: 4302,
                        },
                        affectsAnalytical: false,
                    },
                ],
            },
        ],
    },

    // ════════════════════════════════════════════════════════════════════════
    // DEPLETION DOMAIN — Pseudo-steady-state (Dietz), Rate decline (Fetkovich)
    // ════════════════════════════════════════════════════════════════════════

    {
        key: 'dep_pss',
        label: 'Pressure Depletion',
        description: 'Bounded square reservoir under pseudo-steady-state. Well position sets the Dietz shape factor C_A (centre C_A ≈ 30.88 vs corner C_A ≈ 0.56), controlling the rate of pressure decline.',
        analyticalMethodSummary: 'PSS productivity-index model — predicts exponential rate and pressure decline for the active well location, skin, and permeability.',
        analyticalMethodReference: 'Dietz (1965); standard pseudo-steady-state productivity-index formulation.',
        scenarioClass: 'depletion',
        domain: 'depletion',
        chartPreset: 'oil_depletion',
        defaultSensitivityDimensionKey: 'shape_factor',
        params: {
            analyticalSolutionMode: 'depletion',
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
            // Rock / rel perm
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
            // Grid: 21×21×1 square drainage area, 420 m × 420 m × 10 m
            // Base: central producer (C_A ≈ 30.88)
            nx: 21,
            ny: 21,
            nz: 1,
            cellDx: 20,
            cellDy: 20,
            cellDz: 10,
            permMode: 'uniform',
            uniformPermX: 50,
            uniformPermY: 50,
            uniformPermZ: 5,
            // Initial conditions
            initialPressure: 300,
            initialSaturation: 0.1,
            // Wells: no injector — single producer
            injectorEnabled: false,
            injectorControlMode: 'pressure',
            producerControlMode: 'pressure',
            injectorBhp: 500,
            producerBhp: 100,
            targetInjectorRate: 0,
            targetProducerRate: 0,
            injectorI: 0,
            injectorJ: 0,
            producerI: 10,
            producerJ: 10,
            well_radius: 0.1,
            well_skin: 0,
            // Numerics
            delta_t_days: 1,
            steps: 50,
            max_sat_change_per_step: 0.05,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
            gravityEnabled: false,
        },
        sensitivities: [
            {
                key: 'shape_factor',
                label: 'Well Location  (C_A)',
                description: 'Well position within a bounded square reservoir determines the Dietz shape factor C_A. Central well C_A ≈ 30.88 vs corner well C_A ≈ 0.56 — a ~55× difference that dramatically changes decline rate. Both simulator and analytical update with well position.',
                variants: [
                    {
                        key: 'ca_center',
                        label: 'Centre  (C_A ≈ 30.88)',
                        description: 'Central producer — maximum drainage efficiency, slow decline. Base case.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'ca_corner',
                        label: 'Corner  (C_A ≈ 0.56)',
                        description: 'Corner producer — minimum drainage efficiency, fast decline.',
                        paramPatch: { producerI: 0, producerJ: 0 },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'skin',
                label: 'Skin Factor  s',
                description: 'Skin modifies the near-wellbore pressure drop, scaling the productivity index and hence the decline rate. Positive skin = formation damage; negative skin = stimulation. — Hawkins (1956)',
                variants: [
                    {
                        key: 'skin_neg',
                        label: 's = −2  (stimulated)',
                        description: 'Mild stimulation (e.g. acid job) — higher PI, faster decline.',
                        paramPatch: { well_skin: -2 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'skin_zero',
                        label: 's = 0  (clean)',
                        description: 'No damage or stimulation — base case.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'skin_pos',
                        label: 's = +5  (damaged)',
                        description: 'Moderate formation damage (fines, mud filtrate) — reduced PI, slower decline.',
                        paramPatch: { well_skin: 5 },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'permeability',
                label: 'Permeability  k',
                description: 'PI ∝ k; decline time constant τ = V_p·c_t/PI is inversely proportional to k. Higher permeability → faster, steeper decline. Both analytical and simulation update.',
                variants: [
                    {
                        key: 'perm_tight',
                        label: 'k = 10 mD  (tight)',
                        description: 'Tight reservoir — low PI, slow decline, long-lived production.',
                        paramPatch: { uniformPermX: 10, uniformPermY: 10, uniformPermZ: 1 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'perm_base',
                        label: 'k = 50 mD  (base)',
                        description: 'Base Dietz permeability.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'perm_good',
                        label: 'k = 200 mD  (good)',
                        description: 'Good reservoir — high PI, rapid initial rate, steep decline.',
                        paramPatch: { uniformPermX: 200, uniformPermY: 200, uniformPermZ: 20 },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'compressibility',
                label: 'Compressibility  c_o',
                description: 'Total compressibility c_t governs reservoir storage. Higher c_t → more energy stored → longer decline. τ = V_p·c_t/PI. — Craft & Hawkins (1959)',
                variants: [
                    {
                        key: 'ct_low',
                        label: 'c_o = 5×10⁻⁶ bar⁻¹  (stiff)',
                        description: 'Stiff undersaturated oil — low storage, fast decline.',
                        paramPatch: { c_o: 5e-6 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'ct_base',
                        label: 'c_o = 1×10⁻⁵ bar⁻¹  (base)',
                        description: 'Typical black-oil compressibility — base case.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'ct_high',
                        label: 'c_o = 5×10⁻⁵ bar⁻¹  (volatile)',
                        description: 'High compressibility (near bubble point or volatile oil) — large energy storage, extended decline.',
                        paramPatch: { c_o: 5e-5 },
                        affectsAnalytical: true,
                    },
                ],
            },
        ],
    },

    {
        key: 'dep_decline',
        label: 'Rate Decline',
        description: 'Constant-BHP production from a finite reservoir. PI sets the initial rate level; total compressibility c_t sets the decline duration. Displayed on log-time axes to reveal the exponential trend.',
        analyticalMethodSummary: 'Fetkovich exponential decline — rate and recovery reference curves for constant-BHP depletion, updated for active skin and permeability.',
        analyticalMethodReference: 'Fetkovich (1971).',
        scenarioClass: 'depletion',
        domain: 'depletion',
        chartPreset: 'fetkovich',
        defaultSensitivityDimensionKey: 'permeability',
        params: {
            analyticalSolutionMode: 'depletion',
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
            // Rock / rel perm
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
            // Grid: 1D slab 48 cells × 1 × 1, 480 m × 10 m × 10 m
            // High initial pressure + low BHP → drives exponential decline
            // Peaceman r_eq ≈ 1.98 m for 10×10 m cells; skin = -2 safe minimum
            // (ln(r_eq/r_w) ≈ 2.99; denominator = 2.99 + skin must remain > 0)
            nx: 48,
            ny: 1,
            nz: 1,
            cellDx: 10,
            cellDy: 10,
            cellDz: 10,
            permMode: 'uniform',
            uniformPermX: 20,
            uniformPermY: 20,
            uniformPermZ: 2,
            // Initial conditions: high pressure reservoir depleting to low BHP
            initialPressure: 1500,
            initialSaturation: 0.1,
            // Wells: single producer, no injector
            injectorEnabled: false,
            injectorControlMode: 'pressure',
            producerControlMode: 'pressure',
            injectorBhp: 500,
            producerBhp: 50,
            targetInjectorRate: 0,
            targetProducerRate: 0,
            injectorI: 0,
            injectorJ: 0,
            producerI: 47,
            producerJ: 0,
            well_radius: 0.1,
            well_skin: 0,
            // Numerics
            delta_t_days: 0.25,
            steps: 250,
            max_sat_change_per_step: 0.05,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
            gravityEnabled: false,
        },
        sensitivities: [
            {
                key: 'permeability',
                label: 'Permeability  k',
                description: 'PI ∝ k; time constant τ = V_p·c_t/PI ∝ 1/k. Higher permeability → faster decline. Both analytical and simulation update.',
                variants: [
                    {
                        key: 'perm_tight',
                        label: 'k = 5 mD  (tight)',
                        description: 'Tight reservoir — low PI, slow decline, long-lived production.',
                        paramPatch: { uniformPermX: 5, uniformPermY: 5, uniformPermZ: 0.5 },
                        affectsAnalytical: true,
                    },
                    {
                        key: 'perm_base',
                        label: 'k = 20 mD  (base)',
                        description: 'Base Fetkovich permeability — matches the reference decline curve.',
                        paramPatch: {},
                        affectsAnalytical: true,
                    },
                    {
                        key: 'perm_good',
                        label: 'k = 100 mD  (good)',
                        description: 'Good reservoir — high PI, rapid initial rate, steep exponential decline.',
                        paramPatch: { uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10 },
                        affectsAnalytical: true,
                    },
                ],
            },
            {
                key: 'timestep',
                label: 'Timestep  Δt',
                description: 'Timestep size modifies the numerical stability and accuracy of the simulation.',
                variants: [
                    {
                        key: 'timestep_small',
                        label: 'Δt = 0.1 days  (small)',
                        description: 'Small timestep — higher accuracy, slower simulation.',
                        paramPatch: { delta_t_days: 0.1 },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'timestep_large',
                        label: 'Δt = 1 days  (large)',
                        description: 'Large timestep — lower accuracy, faster simulation.',
                        paramPatch: { delta_t_days: 1 },
                        affectsAnalytical: false,
                    },
                    {
                        key: 'timestep_very_large',
                        label: 'Δt = 10 days  (very large)',
                        description: 'Very large timestep — lower accuracy, faster simulation.',
                        paramPatch: { delta_t_days: 10 },
                        affectsAnalytical: false,
                    },
                ],
            },            
        ],
    },

    // ════════════════════════════════════════════════════════════════════════
    // GAS DOMAIN — experimental; physics bugs documented in TODO.md
    // ════════════════════════════════════════════════════════════════════════
    // TODO(gas): Add sensitivity dimensions once gas physics bugs are fixed:
    //   - Gas Injection: mobility (mu_o/mu_g), S_gc, permeability
    //   - Solution Gas Drive: initial GOR (initialGasSaturation), c_o
    // TODO(physics): gas-oil Pc direction is wrong (see TODO.md — Physics Correctness Issues)
    // TODO(physics): Stone II missing Sorg parameter
    // TODO(physics): 3-phase material balance tracks water only

    {
        key: 'gas_injection',
        label: 'Gas Injection',
        description: 'Gas injector displacing oil in a 1D homogeneous reservoir; no initial free gas. Experimental — known three-phase physics issues (see TODO.md).',
        analyticalMethodSummary: 'Simulation-only — no analytical overlay while the three-phase gas-injection model remains under validation.',
        analyticalMethodReference: 'No validated analytical reference in the current repo yet.',
        scenarioClass: '3phase',
        domain: 'gas',
        chartPreset: 'gas',
        params: {
            analyticalSolutionMode: 'waterflood',
            nx: 20, ny: 1, nz: 1,
            cellDx: 50, cellDy: 50, cellDz: 10,
            initialPressure: 250,
            initialSaturation: 0.2,
            initialGasSaturation: 0,
            reservoirPorosity: 0.2,
            uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10,
            permMode: 'uniform',
            s_wc: 0.2, s_or: 0.15,
            s_gc: 0.05, s_gr: 0.05, s_org: 0.20,
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
            steps: 60,
            max_sat_change_per_step: 0.1,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
        },
        sensitivities: [],
    },

    {
        key: 'gas_drive',
        label: 'Solution Gas Drive',
        description: 'Pressure depletion with initial free gas — models a reservoir already below bubble point. Experimental — known three-phase physics issues (see TODO.md).',
        analyticalMethodSummary: 'Simulation-only — no analytical overlay while three-phase depletion physics remains experimental.',
        analyticalMethodReference: 'No validated analytical reference in the current repo yet.',
        scenarioClass: '3phase',
        domain: 'gas',
        chartPreset: 'gas',
        params: {
            analyticalSolutionMode: 'depletion',
            nx: 20, ny: 1, nz: 1,
            cellDx: 50, cellDy: 50, cellDz: 10,
            initialPressure: 200,
            initialSaturation: 0.2,
            initialGasSaturation: 0.08,
            reservoirPorosity: 0.2,
            uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10,
            permMode: 'uniform',
            s_wc: 0.2, s_or: 0.15,
            s_gc: 0.05, s_gr: 0.05, s_org: 0.20,
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
            steps: 60,
            max_sat_change_per_step: 0.1,
            max_pressure_change_per_step: 75,
            max_well_rate_change_fraction: 0.75,
        },
        sensitivities: [],
    },
];

// Freeze all scenario params objects to catch accidental in-place mutation early.
// A mutation to one scenario's params cannot silently corrupt another.
for (const scenario of SCENARIOS) {
    Object.freeze(scenario.params);
}

// ─── Lookup helpers ───────────────────────────────────────────────────────────

const scenarioMap = new Map(SCENARIOS.map((s) => [s.key, s]));

export function getScenario(key: string | null | undefined): Scenario | null {
    if (!key) return null;
    const found = scenarioMap.get(key);
    if (!found && import.meta.env.DEV) {
        console.warn(`[scenarios] getScenario: unknown key "${key}"`);
    }
    return found ?? null;
}

/** Returns the full base params for a scenario, or {} if not found. */
export function getScenarioParams(key: string | null | undefined): Record<string, unknown> {
    return getScenario(key)?.params ?? {};
}

/**
 * Returns the full params for a scenario + sensitivity dimension + variant combination.
 * Merges the variant's paramPatch on top of the scenario base params.
 * If dimensionKey or variantKey are null/undefined, returns the base scenario params.
 */
export function getScenarioWithVariantParams(
    scenarioKey: string,
    dimensionKey: string | null | undefined,
    variantKey: string | null | undefined,
): Record<string, unknown> {
    const scenario = getScenario(scenarioKey);
    if (!scenario) return {};
    if (!dimensionKey || !variantKey) return scenario.params;

    const dimension = scenario.sensitivities.find((d) => d.key === dimensionKey);
    if (!dimension) {
        if (import.meta.env.DEV) {
            console.warn(`[scenarios] getScenarioWithVariantParams: unknown dimensionKey "${dimensionKey}" for scenario "${scenarioKey}"`);
        }
        return scenario.params;
    }

    const variant = dimension.variants.find((v) => v.key === variantKey);
    if (!variant) {
        if (import.meta.env.DEV) {
            console.warn(`[scenarios] getScenarioWithVariantParams: unknown variantKey "${variantKey}" in dimension "${dimensionKey}" of scenario "${scenarioKey}"`);
        }
        return scenario.params;
    }

    return { ...scenario.params, ...variant.paramPatch };
}

/**
 * Returns the variant keys that should be pre-selected when a dimension loads.
 * All variants are enabled by default unless explicitly set enabledByDefault: false.
 */
export function getDefaultVariantKeys(dimension: SensitivityDimension): string[] {
    return dimension.variants
        .filter((v) => v.enabledByDefault !== false)
        .map((v) => v.key);
}

/** Returns the chart layout config for a preset key, or {} if not found. */
export function getChartPreset(presetKey: string | null | undefined): RateChartLayoutConfig {
    return CHART_PRESETS[presetKey ?? ''] ?? {};
}

export function listScenarios(): Scenario[] {
    return SCENARIOS;
}
