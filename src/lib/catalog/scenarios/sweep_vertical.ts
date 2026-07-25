import { sweepLivePanels } from '../chartPanels/sweepLivePanels';
import type { Scenario } from '../scenarios';
import { waterfloodBLDef } from '../analyticalAdapters';

export const sweep_vertical: Scenario = {
    key: 'sweep_vertical',
    label: 'Vertical Sweep (XZ)',
    description: 'Non-communicating layered reservoir (XZ). Higher permeability contrast V_DP causes earlier breakthrough in faster layers, reducing vertical sweep E_V.',
    analyticalMethodSummary: 'Dykstra-Parsons layered sweep model — predicts per-layer breakthrough ordering and combined E_V for the active layer permeabilities.',
    analyticalMethodReference: 'Dykstra and Parsons (1950).',
    chartLayoutKey: 'sweep',
    chartLayoutPatch: {
        rateChart: {
            panels: {
                sweep_areal: { visible: false },
                sweep_combined: { visible: false },
                sweep_combined_mobile_oil: { visible: false },
            },
        },
    },
    defaultSensitivityDimensionKey: 'heterogeneity',
    capabilities: {
        analyticalMethod: 'sweep',
        sweepGeometry: 'vertical',
        // Dykstra-Parsons first to preserve this scenario's existing default.
        // The choice is live here: max |Δ| ≈ 0.97 on E_V, 0.037 on RF.
        sweepMethods: ['dykstra-parsons', 'stiles'],
        hasInjector: true,
        default3DScalar: 'saturation_water',
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
        // Grid: 5-layer 1D slab, 48 cells × 5 layers, 960 m × 20 m × 20 m
        // Base case: moderate heterogeneity V_DP ≈ 0.5
        nx: 48,
        ny: 1,
        nz: 5,
        cellDx: 10,
        cellDy: 10,
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
        fimEnabled: false,
        delta_t_days: 0.25,
        steps: 500,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    analyticalDef: waterfloodBLDef,
    liveChartPanels: sweepLivePanels,
    sensitivities: [
        {
            key: 'heterogeneity',
            label: 'Layer Heterogeneity  V_DP',
            description: 'Compare uniform, moderate, and extreme permeability contrasts across layers. The Dykstra-Parsons analytical model uses the active layer permeabilities, so both simulation and analytical sweep overlays update.',
            analyticalOverlayMode: 'per-result',
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
            analyticalOverlayMode: 'per-result',
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
        {
            // T7.18 — the second combined-uncertainty case in the library, and
            // deliberately the opposite sign to the first. wf_tornado shows two
            // parameters whose combination is *worse* than the sum of their
            // one-at-a-time effects; this one shows two whose combination is
            // considerably better, because they compete for the same barrels.
            // Both are failures of one-at-a-time reasoning; only together do
            // they make the point that the direction of the error is not
            // predictable either. Measured 2026-07-24, see sweep_vertical.test.ts:
            //   base 0.726 | curve only 0.579 | layers only 0.379 | both 0.357
            // (recovery of mobile oil at 0.625 PVI).
            // See docs/CASE_LIBRARY_ROADMAP.md T7.18.
            key: 'endpoints_vs_geology',
            label: 'Rock Curves or Geology?',
            description: 'Two explanations for the same disappointing recovery, and the mirror image of the tornado-plot case. Steepening the oil relative-permeability curve (Corey n_o) and widening the layer permeability contrast both push recovery down by completely different mechanisms — one is how the rock lets oil flow, the other is where the water goes. Measured at equal injected pore volumes, each alone costs recovery (-0.147 and -0.346 of mobile oil), but together they cost only -0.368, not the -0.493 that adding them would predict. They are competing for the same barrels: once a thief zone is bypassing most of the section, making the oil curve worse has little left to spoil. The decision consequence is direct — a one-at-a-time study says fixing the rock curve buys back 0.147, while in the layered reservoir it actually buys back 0.022, so a SCAL programme justified on the first number is being over-valued by nearly 7x. The Dykstra-Parsons overlay is the second tell: built from layer permeabilities and mobility endpoints, it moves for the geology change and is completely blind to the Corey change. When the simulation drops and the reference does not, the model you are comparing against has no term for what you just varied.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'evg_base',
                    label: 'n_o = 2, uniform  (base)',
                    description: 'Benign rock curve, no layering. The reference corner of the 2x2.',
                    paramPatch: {
                        n_o: 2,
                        permMode: 'uniform',
                        uniformPermX: 100,
                        uniformPermY: 100,
                        uniformPermZ: 10,
                    },
                    affectsAnalytical: true,
                },
                {
                    key: 'evg_curve_only',
                    label: 'n_o = 4 alone  (rock curves)',
                    description: 'A steeply curving oil relative permeability on the same uniform rock. Recovery falls — and the Dykstra-Parsons curve does not move at all, because the Corey exponent is not one of its inputs. The analytical reference here is shared context, not a prediction.',
                    paramPatch: {
                        n_o: 4,
                        permMode: 'uniform',
                        uniformPermX: 100,
                        uniformPermY: 100,
                        uniformPermZ: 10,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'evg_layers_only',
                    label: 'V_DP ≈ 0.8 alone  (geology)',
                    description: 'The benign rock curve in a strongly layered reservoir. Recovery falls for an entirely different reason — the water is going somewhere else — and this time the analytical sweep model does follow, because layer permeabilities are exactly what it is built from.',
                    paramPatch: {
                        n_o: 2,
                        layerPermsX: [500, 150, 50, 20, 10],
                        layerPermsY: [500, 150, 50, 20, 10],
                        layerPermsZ: [50, 15, 5, 2, 1],
                    },
                    affectsAnalytical: true,
                },
                {
                    key: 'evg_both',
                    label: 'n_o = 4 AND V_DP ≈ 0.8',
                    description: 'Both together — and markedly better than adding the two individual penalties would suggest. The steep curve, which cost 0.147 of mobile oil on uniform rock, costs only 0.022 more once a thief zone is already bypassing most of the section. Neither one-at-a-time result predicts this corner, and here they over-predict rather than under-predict it.',
                    paramPatch: {
                        n_o: 4,
                        layerPermsX: [500, 150, 50, 20, 10],
                        layerPermsY: [500, 150, 50, 20, 10],
                        layerPermsZ: [50, 15, 5, 2, 1],
                    },
                    affectsAnalytical: true,
                },
            ],
        },
    ],
};
