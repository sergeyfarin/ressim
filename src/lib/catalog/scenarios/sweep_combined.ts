import type { Scenario } from '../scenarios';
import { waterfloodBLDef } from '../analyticalAdapters';

export const sweep_combined: Scenario = {
    key: 'sweep_combined',
    label: 'Combined Sweep',
    catalog: {
        group: 'sweep-efficiency',
        role: 'simulation',
        caseMode: 'wf',
        parameterSummary: 'Layered five-spot waterflood · combined areal and vertical sweep · interaction matrix',
    },
    description: 'Volumetric sweep E_vol = E_A × E_V in a 3D five-spot-over-layers flood. Two axes: a 2×2 interaction matrix of mobility vs. layering, and a progressive sweep from ideal to degraded conditions that ends in per-cell random permeability fields.',
    analyticalMethodSummary: 'Stiles layer-by-layer sweep: the total RF and combined E_vol use layer-by-layer Buckley-Leverett displacement inside the Craig-contacted region, while E_A and E_V remain analytical diagnostic decomposition views.',
    analyticalMethodReference: 'Stiles (1949); Craig (1971); Buckley and Leverett (1942); Welge (1952).',
    chartLayoutKey: 'sweep',
    chartLayoutPatch: {
        chart: {
            panels: {
                rates: { curveKeys: ['water-cut-sim'] },
                cumulative: { curveKeys: ['cum-oil-sim'] },
                diagnostics: { curveKeys: ['avg-pressure-sim'] },
                // At 'both' geometry the engine reports E_vol but deliberately
                // returns no E_A / E_V: a single simulation cannot separate
                // areal from vertical contact. These two panels are therefore
                // analytical-only decomposition views, and are collapsed by
                // default so the page leads with the panels that carry both a
                // numerical and an analytical curve.
                sweep_areal: { expanded: false },
                sweep_vertical: { expanded: false },
                sweep_combined: {
                    title: 'Total Sweep Efficiency (E_vol)',
                    visible: true,
                },
                sweep_combined_mobile_oil: {
                    title: 'Mobile Oil Recovered vs Analytical E_vol',
                    visible: true,
                    expanded: false,
                },
            },
        },
    },
    defaultSensitivityDimensionKey: 'interaction_core',
    capabilities: {
        analyticalMethod: 'sweep',
        sweepGeometry: 'both',
        // Stiles first: it is the default here. Both correlations move E_vol
        // materially at this geometry (max |Δ| ≈ 0.12 on E_vol, 0.055 on RF).
        sweepMethods: ['stiles', 'dykstra-parsons'],
        hasInjector: true,
        default3DScalar: 'saturation_water',
        spatialProfile: {
            defaultAxis: 'well-path',
            wellPathLabel: 'Injector → producer',
        },
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES is the interactive default for the larger combined-sweep grid.',
    },
    params: {
        // Fluid — M = 1 (favorable), the base case both dimensions run as
        // "Favorable + layered" / "Vertical only".
        mu_w: 0.5,
        mu_o: 0.5,
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
        // Base: V_DP ≈ 0.55 layering, sealed layers, M = 1.
        nx: 21,
        ny: 21,
        nz: 5,
        cellDx: 20,
        cellDy: 20,
        cellDz: 4,
        permMode: 'perLayer',
        // The uniform variants' permeability: the layers' arithmetic mean, so a
        // uniform and a layered run have the same kh and inject at comparable
        // rates. Only the heterogeneity differs between them.
        uniformPermX: 251,
        uniformPermY: 251,
        uniformPermZ: 25.1,
        layerPermsX: [1000, 150, 5, 60, 40],
        layerPermsY: [1000, 150, 5, 60, 40],
        // Sealed layers, the Stiles and Dykstra-Parsons assumption. This used to
        // come from leaving layerPermsZ out, so the payload builder filled it with
        // its 0.001 mD floor; it is the same value, written down.
        layerPermsZ: [0.001, 0.001, 0.001, 0.001, 0.001],
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
        fimEnabled: false,
        // Each run goes to about 3 PVI, past the chart's 2.5 PVI window. The
        // variants inject at very different rates (measured 2026-09-25: 2.5 PVI
        // at 296 d here, at 796 d for the unfavorable uniform case), so each
        // sets its own step count; one shared horizon either stopped short of
        // the window or ran several times past it.
        delta_t_days: 5.0,
        steps: 75,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    analyticalDef: waterfloodBLDef,
    referenceSources: [{
        kind: 'opm-flow',
        artifactKeys: ['sweep_combined'],
        artifactVariantLabels: { sweep_combined: 'base' },
    }],
    sensitivities: [
        {
            key: 'interaction_core',
            label: 'Mobility × Vertical Heterogeneity',
            description: '2 × 2 interaction map: M = 1 or 10, in uniform permeability or the sealed five-layer stack at the same kh. Separates mobility-only, layering-only and compounded penalties without areal randomness.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'interaction_favorable_uniform',
                    label: 'Favorable (M = 1) + uniform',
                    description: 'Near-piston 3D baseline: favorable mobility and no vertical heterogeneity.',
                    paramPatch: {
                        permMode: 'uniform',
                        steps: 80,
                    },
                    affectsAnalytical: true,
                },
                {
                    key: 'interaction_unfavorable_uniform',
                    label: 'Unfavorable (M = 10) + uniform',
                    description: 'Mobility penalty only: poor mobility in uniform permeability.',
                    paramPatch: {
                        mu_o: 5.0,
                        permMode: 'uniform',
                        steps: 180,
                    },
                    affectsAnalytical: true,
                },
                {
                    key: 'interaction_favorable_layered',
                    label: 'Favorable (M = 1) + layered  (base)',
                    description: 'Vertical heterogeneity penalty only: good mobility in the sealed layered stack. The base case.',
                    paramPatch: {},
                    affectsAnalytical: true,
                },
                {
                    key: 'interaction_unfavorable_layered',
                    label: 'Unfavorable (M = 10) + layered',
                    description: 'Compounded mobility plus vertical layering penalty in the same 3D flood.',
                    paramPatch: { mu_o: 5.0, steps: 130 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'sweep_ladder',
            label: 'Ideal to Worst',
            description: 'Uniform at M = 1, then the sealed layers, then per-cell random fields at M = 2 and M = 10. The random fields replace the layering rather than add to it. One analytical curve: the base case (layered, M = 1).',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'ladder_ideal',
                    label: 'Ideal  (uniform, M = 1)',
                    description: 'Best-case 3D sweep: uniform permeability and favorable mobility.',
                    paramPatch: {
                        permMode: 'uniform',
                        steps: 80,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'ladder_vertical',
                    label: 'Layered  (sealed layers, M = 1, base)',
                    description: 'First degradation step: the sealed layered stack with favorable mobility retained. The base case.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'ladder_full_het',
                    label: 'Random field  (40–500 mD per cell, M = 2)',
                    description: 'A seeded per-cell random field replaces the layers: each cell draws k_x and k_y from 40–500 mD and k_z from that range / 10. Heterogeneous in every direction, vertically communicating, not layered.',
                    paramPatch: {
                        mu_o: 1.0,
                        permMode: 'random',
                        minPerm: 40,
                        maxPerm: 500,
                        useRandomSeed: true,
                        randomSeed: 4301,
                        steps: 100,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'ladder_worst',
                    label: 'Random field, unfavorable  (20–700 mD per cell, M = 10)',
                    description: 'A wider per-cell random field drawn the same way (20–700 mD, a different seed) with strongly unfavorable mobility: the most degraded rung.',
                    paramPatch: {
                        mu_o: 5.0,
                        permMode: 'random',
                        minPerm: 20,
                        maxPerm: 700,
                        useRandomSeed: true,
                        randomSeed: 4302,
                        steps: 165,
                    },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
