import { depletionLivePanels } from '../chartPanels/depletionLivePanels';
import type { Scenario } from '../scenarios';
import { depletionDef } from '../analyticalAdapters';

/**
 * Layered composite-decline scenario.
 *
 * Physics basis: a 5-layer commingled reservoir with permeability contrast
 * produces a composite decline that can resemble an Arps hyperbola over a
 * finite window, even though each bounded layer eventually declines
 * exponentially. The quantitative reference is the exact superposition of
 * the five layer responses, not an imposed Arps exponent.
 *
 * Reference: Arps, J.J. (1945) "Analysis of Decline Curves", Trans. AIME 160.
 *            Fetkovich, M.J. (1980) "Decline Curve Analysis Using Type
 *            Curves", JPT 32(6), SPE-4629-PA.
 */
export const dep_arps: Scenario = {
    key: 'dep_arps',
    label: 'Layered Depletion — Composite Decline',
    catalog: {
        group: 'depletion-decline',
        role: 'interpretation',
        caseMode: 'dep',
        parameterSummary: 'Five noncommunicating layers · fixed total PI · composite exponential decline',
    },
    description: 'Five layers deplete through a commingled constant-BHP producer. In the reference-valid limit they are noncommunicating: total productivity is fixed while permeability contrast changes the spread of layer time constants, and the exact superposition can resemble an Arps hyperbola over a finite window. A separate vertical-communication sensitivity deliberately enables crossflow and shows where that independent-layer analytical model ceases to apply.',
    analyticalMethodSummary: 'Exact superposition of five Fetkovich boundary-dominated exponential layer responses. Arps decline is an interpretation of the composite shape, not the quantitative reference.',
    analyticalMethodReference: 'Arps (1945), SPE-945228-G; Fetkovich (1980), SPE-4629-PA.',
    chartLayoutKey: 'fetkovich',
    defaultSensitivityDimensionKey: 'layer_contrast',
    capabilities: {
        analyticalMethod: 'depletion',
        hasInjector: false,
        default3DScalar: 'pressure',
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES is the interactive default for the commingled layered depletion run.',
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
        // Grid: five vertically stacked, noncommunicating tank cells. Large
        // areal cells provide enough storage for the decline to remain visible
        // over the interactive run while avoiding within-layer transients.
        // Log-spaced layer permeabilities, normalised to a 20 mD arithmetic
        // mean. The sensitivity changes contrast without changing total PI.
        nx: 1,
        ny: 1,
        nz: 5,
        cellDx: 1000,
        cellDy: 1000,
        cellDz: 10,
        permMode: 'perLayer',
        uniformPermX: 20,
        uniformPermY: 20,
        uniformPermZ: 2,
        layerPermsX: [53.98942, 25.53002, 12.0724, 5.70869, 2.69947],
        layerPermsY: [53.98942, 25.53002, 12.0724, 5.70869, 2.69947],
        // Suppress inter-layer crossflow so the numerical model satisfies the
        // independent-layer assumption used by the analytical superposition.
        layerPermsZ: [1e-9, 1e-9, 1e-9, 1e-9, 1e-9],
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
        producerI: 0,
        producerJ: 0,
        well_radius: 0.1,
        well_skin: 0,
        analyticalLayeredComposite: true,
        analyticalDepletionStartDays: 0,
        // Numerics
        fimEnabled: false,
        delta_t_days: 0.025,
        steps: 600,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    analyticalDef: depletionDef,
    liveChartPanels: depletionLivePanels,
    sensitivities: [
        {
            key: 'layer_contrast',
            label: 'Layer Contrast',
            description: 'Permeability contrast changes only the spread of layer decline time constants. Every case retains a 20 mD arithmetic mean and the same total initial PI, so differences reflect heterogeneity rather than a hidden productivity change.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'contrast_low',
                    label: 'Low contrast  (3:1)',
                    description: 'Mild heterogeneity with a 20 mD mean; the composite stays close to one exponential.',
                    paramPatch: {
                        layerPermsX: [32.1625, 24.43822, 18.56903, 14.10942, 10.72083],
                        layerPermsY: [32.1625, 24.43822, 18.56903, 14.10942, 10.72083],
                    },
                    affectsAnalytical: true,
                },
                {
                    key: 'contrast_base',
                    label: 'Moderate contrast  (20:1)',
                    description: 'Base log-spaced layering with a 20 mD mean and a wider spread of decline times.',
                    paramPatch: {},
                    affectsAnalytical: true,
                    enabledByDefault: true,
                },
                {
                    key: 'contrast_high',
                    label: 'High contrast  (100:1)',
                    description: 'Strong heterogeneity at the same 20 mD mean; fast layers deplete early while slow layers sustain the tail.',
                    paramPatch: {
                        layerPermsX: [68.59414, 21.69137, 6.85941, 2.16914, 0.68594],
                        layerPermsY: [68.59414, 21.69137, 6.85941, 2.16914, 0.68594],
                    },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'vertical_communication',
            label: 'Inter-layer Communication',
            description: 'The exact composite reference assumes noncommunicating layers. Increasing vertical permeability deliberately violates that assumption: crossflow redistributes pressure between fast and slow layers, so numerical departure from the shared no-crossflow reference is the expected limitation signal, not simulator error.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'communication_isolated',
                    label: 'Isolated  (k_z = 10⁻⁹ mD)',
                    description: 'Reference-valid limit: layers deplete independently.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'communication_weak',
                    label: 'Weak crossflow  (k_z = 10⁻⁵ mD)',
                    description: 'Small vertical communication begins to couple the layer pressure histories.',
                    paramPatch: { layerPermsZ: [1e-5, 1e-5, 1e-5, 1e-5, 1e-5] },
                    affectsAnalytical: false,
                },
                {
                    key: 'communication_strong',
                    label: 'Strong crossflow  (k_z = 10⁻³ mD)',
                    description: 'Communicating layers violate exact independent-tank superposition and should visibly depart from its analytical rate and pressure curves.',
                    paramPatch: { layerPermsZ: [1e-3, 1e-3, 1e-3, 1e-3, 1e-3] },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
