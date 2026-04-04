import type { Scenario } from '../scenarios';
import { depletionDef } from '../analyticalAdapters';

/**
 * Arps Hyperbolic Decline scenario.
 *
 * Physics basis: a 5-layer commingled reservoir with permeability contrast
 * produces composite decline that looks hyperbolic — even though each layer
 * individually declines exponentially (Fetkovich 1971).  The superposition
 * of multiple exponential declines with different time constants τ_j = Vp_j·c_t/PI_j
 * yields an effective Arps b-parameter between 0 and 1.
 *
 * The sensitivity dimension varies the Arps b-parameter in the analytical
 * reference so the user can see which b value best matches the layered simulation.
 *
 * Reference: Arps, J.J. (1945) "Analysis of Decline Curves", Trans. AIME 160.
 *            Fetkovich, M.J. (1971) "A Simplified Approach to Water Influx
 *            Calculations", JPT 23(7).
 */
export const dep_arps: Scenario = {
    key: 'dep_arps',
    label: 'Arps Decline (Layered)',
    description: 'Commingled multi-layer depletion at constant BHP. Each layer declines exponentially with its own time constant; the composite rate follows Arps hyperbolic decline with 0 < b < 1. Vary the Arps b exponent to match the layered simulation.',
    analyticalMethodSummary: 'Arps (1945) generalised decline — exponential (b=0), hyperbolic (0<b<1), or harmonic (b=1) rate decline matched against commingled layered simulation.',
    analyticalMethodReference: 'Arps (1945); Fetkovich (1971).',
    chartLayoutKey: 'fetkovich',
    defaultSensitivityDimensionKey: 'arps_b',
    capabilities: {
        analyticalMethod: 'depletion',
        showSweepPanel: false,
        hasInjector: false,
        default3DScalar: null,
        requiresThreePhaseMode: false,
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
        // Grid: 1D slab 48 cells × 1 × 5 layers, 480 m × 10 m × 50 m
        // 5 layers with ~10:1 permeability contrast — produces b ≈ 0.3–0.5
        // Layers arranged high-to-low (top = best): 100, 50, 20, 10, 5 mD
        // This contrast is representative of a typical clastic sequence
        // (Dykstra-Parsons V_DP ≈ 0.72)
        nx: 48,
        ny: 1,
        nz: 5,
        cellDx: 10,
        cellDy: 10,
        cellDz: 10,
        permMode: 'perLayer',
        uniformPermX: 20,
        uniformPermY: 20,
        uniformPermZ: 2,
        layerPermsX: [100, 50, 20, 10, 5],
        layerPermsY: [100, 50, 20, 10, 5],
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
        // Analytical: default b = 0.4 as starting point for layered match
        analyticalArpsB: 0.4,
        // Numerics
        fimEnabled: false,
        delta_t_days: 0.25,
        steps: 250,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    analyticalDef: depletionDef,
    sensitivities: [
        {
            key: 'arps_b',
            label: 'Arps b Exponent',
            description: 'Arps decline exponent b controls the curvature of rate decline. b=0 is exponential (Fetkovich), b=1 is harmonic. For commingled layered reservoirs, b typically falls between 0.3 and 0.5. — Arps (1945)',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'b_exponential',
                    label: 'b = 0  (exponential)',
                    description: 'Pure exponential decline — correct for single-layer PSS. Poor fit for layered systems.',
                    paramPatch: { analyticalArpsB: 0 },
                    affectsAnalytical: true,
                },
                {
                    key: 'b_low',
                    label: 'b = 0.3  (mild hyperbolic)',
                    description: 'Mild hyperbolic — moderate layer contrast or partial heterogeneity.',
                    paramPatch: { analyticalArpsB: 0.3 },
                    affectsAnalytical: true,
                },
                {
                    key: 'b_base',
                    label: 'b = 0.5  (hyperbolic)',
                    description: 'Mid-range hyperbolic — typical for commingled layered systems with ~10:1 perm contrast.',
                    paramPatch: { analyticalArpsB: 0.5 },
                    affectsAnalytical: true,
                    enabledByDefault: true,
                },
                {
                    key: 'b_high',
                    label: 'b = 0.7  (strong hyperbolic)',
                    description: 'Strong hyperbolic — high heterogeneity or volatile oil effects.',
                    paramPatch: { analyticalArpsB: 0.7 },
                    affectsAnalytical: true,
                },
                {
                    key: 'b_harmonic',
                    label: 'b = 1  (harmonic)',
                    description: 'Harmonic decline — extreme case; rate decline inversely proportional to time.',
                    paramPatch: { analyticalArpsB: 1.0 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'layer_contrast',
            label: 'Layer Contrast',
            description: 'Permeability contrast between layers controls the effective Arps b. Higher contrast → more spread in layer time constants → higher effective b.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'contrast_low',
                    label: 'Low contrast  (3:1)',
                    description: 'Mild heterogeneity — layers from 30 to 10 mD. Effective b ≈ 0.1–0.2.',
                    paramPatch: {
                        layerPermsX: [30, 25, 20, 15, 10],
                        layerPermsY: [30, 25, 20, 15, 10],
                    },
                    affectsAnalytical: true,
                },
                {
                    key: 'contrast_base',
                    label: 'Moderate contrast  (20:1)',
                    description: 'Base case — layers from 100 to 5 mD (V_DP ≈ 0.72). Effective b ≈ 0.3–0.5.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'contrast_high',
                    label: 'High contrast  (100:1)',
                    description: 'Strong heterogeneity — layers from 500 to 5 mD (V_DP ≈ 0.87). Effective b ≈ 0.5–0.7.',
                    paramPatch: {
                        layerPermsX: [500, 100, 20, 10, 5],
                        layerPermsY: [500, 100, 20, 10, 5],
                    },
                    affectsAnalytical: true,
                },
            ],
        },
    ],
};
