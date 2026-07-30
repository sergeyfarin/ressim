import { depletionLivePanels } from '../chartPanels/depletionLivePanels';
import type { Scenario } from '../scenarios';
import { depletionDef } from '../analyticalAdapters';

/**
 * Layered composite-decline scenario.
 *
 * Physics basis: a 5-layer commingled reservoir with permeability contrast
 * produces a composite decline that can resemble an Arps hyperbola over a
 * finite window, even though each bounded layer eventually declines
 * exponentially. The quantitative reference is the late-time superposition
 * of five Dietz/Fetkovich boundary-dominated layer responses, not an imposed
 * Arps exponent.
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
        parameterSummary: '9×9×5 grid · five layered flow units · centered commingled producer',
    },
    description: 'A spatial 1 km square reservoir with five layers depletes through a centered, fully penetrating constant-BHP producer. Early radial-to-boundary pressure propagation is resolved numerically; after boundary-dominated flow begins, the noncommunicating-layer result approaches a Dietz/Fetkovich exponential superposition. Fixed-mean layer contrast changes the spread of decline time constants, while a separate vertical-communication study deliberately violates the independent-layer reference.',
    analyticalMethodSummary: 'Late-time superposition of five Dietz centered-square productivity/storage responses. The overlay begins only after the early spatial transient; Arps decline is an interpretation of the composite shape, not the quantitative reference.',
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
        // Grid: a spatial 1 km x 1 km x 50 m reservoir. Each layer has a
        // resolved areal pressure field and a centered producer completion.
        // Log-spaced layer permeabilities, normalised to a 20 mD arithmetic
        // mean. The sensitivity changes contrast without changing total PI.
        nx: 9,
        ny: 9,
        nz: 5,
        cellDx: 111.11111111111111,
        cellDy: 111.11111111111111,
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
        producerI: 4,
        producerJ: 4,
        well_radius: 0.1,
        well_skin: 0,
        analyticalLayeredComposite: true,
        // Dietz/Fetkovich is a boundary-dominated reference, not an
        // early-time radial-flow solution. Hide it during pressure propagation.
        analyticalDepletionStartDays: 12,
        // Numerics
        fimEnabled: false,
        delta_t_days: 0.2,
        steps: 300,
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
            description: 'The late-time composite reference assumes noncommunicating layers. Increasing kv/kh deliberately violates that assumption: crossflow redistributes pressure between fast and slow layers, so numerical departure from the shared no-crossflow reference is the expected limitation signal, not simulator error.',
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
                    label: 'Weak crossflow  (kv/kh = 10⁻⁵)',
                    description: 'Small vertical communication begins to couple the layer pressure histories.',
                    paramPatch: { layerPermsZ: [5.398942e-4, 2.553002e-4, 1.20724e-4, 5.70869e-5, 2.69947e-5] },
                    affectsAnalytical: false,
                },
                {
                    key: 'communication_strong',
                    label: 'Strong crossflow  (kv/kh = 0.001)',
                    description: 'Communicating layers violate the independent-layer late-time superposition and should visibly depart from its analytical rate and pressure curves.',
                    paramPatch: { layerPermsZ: [5.398942e-2, 2.553002e-2, 1.20724e-2, 5.70869e-3, 2.69947e-3] },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
