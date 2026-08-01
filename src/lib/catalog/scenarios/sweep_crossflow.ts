import { sweepLivePanels } from '../chartPanels/sweepLivePanels';
import type { Scenario } from '../scenarios';
import { waterfloodBLDef } from '../analyticalAdapters';

/**
 * The assumption inside Dykstra-Parsons, tested directly.
 *
 * `sweep_vertical` varies the layer permeabilities the correlation is built
 * from, and the analytical curve follows. This case varies the one property
 * the correlation cannot see — the vertical permeability that lets layers
 * exchange fluid — and watches the simulation move while the reference stands
 * still. Dykstra-Parsons (1950) and Stiles (1949) both solve a stack of
 * isolated tubes; nothing in either model has a term for kv.
 *
 * Three mechanisms can drive fluid across a layer boundary, and the case
 * separates them: a viscous pressure difference between swept and unswept
 * regions (the kv ladder), the mobility contrast that creates it (the sign of
 * the effect reverses with M), and capillary suction into the finer rock
 * (invisible with the layers sealed, worth another 0.013 of recovery with them
 * open). Gravity, the fourth, is the subject of `wf_gravity` and is switched
 * off here so the other three are read without it.
 *
 * References: Dykstra and Parsons (1950); Stiles (1949); Root and Skiba
 * (1965), SPEJ 5(3); Goddin et al. (1966), SPEJ 6(3); Zapata and Lake (1981),
 * SPE 10111 — the viscous-crossflow scaling and its mobility dependence;
 * Willhite, Waterflooding, SPE Textbook 3, ch. 5.
 */
export const sweep_crossflow: Scenario = {
    key: 'sweep_crossflow',
    label: 'Layer Crossflow — Do the Layers Talk?',
    catalog: {
        group: 'sweep-efficiency',
        role: 'simulation',
        caseMode: 'wf',
        parameterSummary: 'Layered waterflood · vertical communication against the non-communicating Dykstra–Parsons assumption',
    },
    description: 'Five layers spanning a 5:1 permeability range, flooded end to end, with one property varied over six orders of magnitude: the vertical permeability. Seal the layers and water breaks through at 0.427 pore volumes injected; let them communicate freely and it holds off until 0.599, a 40 % difference, with recovery at one pore volume injected rising from 0.735 to 0.781. The Dykstra-Parsons and Stiles curves are identical across all of it — both models solve a stack of isolated tubes, and kv is not one of their inputs, so the analytical overlay is a fixed line the simulation walks away from. That is the case in one sentence: a correlation is only as good as the assumption you are not testing. The direction of the error is not fixed either. Crossflow is driven by the mobility difference between the swept and unswept parts of a layer, so at a favourable mobility ratio it recovers 0.046 more oil, at M = 2 it changes recovery by nothing at all, and at M = 10 it costs 0.021 — the same physical connection, helping, doing nothing, and hurting.',
    analyticalMethodSummary: 'Dykstra-Parsons (default) or Stiles layered sweep, built from the layer permeabilities and the end-point mobility ratio. Both assume the layers are hydraulically isolated, which is exactly the assumption this case violates on purpose — so the reference is correct only on the sealed rung of the ladder and is offered elsewhere as a fixed baseline, not a prediction.',
    analyticalMethodReference: 'Dykstra and Parsons (1950); Stiles (1949); Zapata and Lake (1981), SPE 10111 — "A Theoretical Analysis of Viscous Crossflow"; Root and Skiba (1965), SPEJ 5(3); Willhite, Waterflooding (SPE Textbook 3), ch. 5.',
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
    defaultSensitivityDimensionKey: 'vertical_communication',
    capabilities: {
        analyticalMethod: 'sweep',
        sweepGeometry: 'vertical',
        // Same layered vertical geometry as sweep_vertical, where the choice
        // between the two correlations is measurably live.
        sweepMethods: ['dykstra-parsons', 'stiles'],
        hasInjector: true,
        default3DScalar: 'saturation_water',
        spatialProfile: { defaultAxis: 'i' },
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES is the validated default for this two-phase layered section; every shipped variant completes in a few seconds at this resolution.',
    },
    params: {
        // Fluid — a 1 cp oil against 0.5 cp water. With the water end point
        // below, this is an end-point mobility ratio of 0.5.
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
        // Rock / relative permeability. k_rw_max = 0.25 is a water-wet
        // sandstone end point; it is what lets the mobility ratio be set by
        // oil viscosity alone across the mobility dimension without asking for
        // a sub-centipoise oil.
        reservoirPorosity: 0.2,
        s_wc: 0.1,
        s_or: 0.1,
        n_w: 2,
        n_o: 2,
        k_rw_max: 0.25,
        k_ro_max: 1,
        capillaryEnabled: false,
        capillaryPEntry: 0,
        capillaryLambda: 2,
        // Grid: 480 m x 10 m x 20 m vertical section, 48 x 1 x 5 —
        // 19,200 m3 pore volume. Layer permeabilities span 5:1 (V_DP ~ 0.5),
        // the moderate clastic contrast sweep_vertical uses as its base.
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
        // Base case: kv/kh = 0.1, the conventional default for a clastic
        // section with no explicit barriers.
        layerPermsZ: [20, 15, 10, 6, 4],
        // Initial conditions
        initialPressure: 300,
        initialSaturation: 0.1,
        // Wells — both fully perforated, both pressure-controlled, as in
        // sweep_vertical. Gravity is off, so a shared completion BHP carries no
        // hydrostatic bias and the whole vertical exchange is viscous and
        // capillary.
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
        // 310 days reaches 1.25 pore volumes injected at the base mobility.
        // The mobility variants rescale the report step with oil viscosity so
        // every curve carries the same PVI per step, which is the axis they
        // are compared on.
        fimEnabled: false,
        delta_t_days: 1,
        steps: 310,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    analyticalDef: waterfloodBLDef,
    liveChartPanels: sweepLivePanels,
    sensitivities: [
        {
            key: 'vertical_communication',
            label: 'Vertical Permeability  k_v/k_h',
            description: 'The same five layers, the same permeabilities along the flow, the same wells — and vertical permeability ranging from effectively zero to isotropic. Breakthrough moves 0.427 → 0.457 → 0.518 → 0.576 → 0.599 pore volumes injected, and recovery at one pore volume injected 0.735 → 0.755 → 0.779 → 0.783 → 0.781. Most of the change is spent between kv/kh = 0.001 and 0.1; beyond that the section is already in vertical equilibrium and further communication buys nothing. The analytical curve does not move at any point on the ladder, because neither Dykstra-Parsons nor Stiles has an input for kv — they solve isolated tubes. It is right on the sealed rung and increasingly conservative from there, and no amount of tuning the layer permeabilities would find that out, because the parameter responsible is not in the model.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'kv_sealed',
                    label: 'k_v/k_h = 10⁻⁶  (sealed — the correlation\'s own assumption)',
                    description: 'Shale-sealed layers, the configuration Dykstra-Parsons and Stiles actually describe. Breakthrough 0.427 PVI in the fast layer, recovery 0.735 at one pore volume injected. This is the rung on which the analytical curve is a prediction rather than a baseline.',
                    paramPatch: { layerPermsZ: [2e-4, 1.5e-4, 1e-4, 6e-5, 4e-5] },
                    affectsAnalytical: false,
                },
                {
                    key: 'kv_001',
                    label: 'k_v/k_h = 0.001',
                    description: 'Barely connected. Breakthrough 0.457, recovery 0.755 — a thousandth of the horizontal permeability is already worth 0.02 of recovery.',
                    paramPatch: { layerPermsZ: [0.2, 0.15, 0.1, 0.06, 0.04] },
                    affectsAnalytical: false,
                },
                {
                    key: 'kv_01',
                    label: 'k_v/k_h = 0.01',
                    description: 'Breakthrough 0.518, recovery 0.779. The steepest part of the ladder: this rung and the one below it account for most of the total effect.',
                    paramPatch: { layerPermsZ: [2, 1.5, 1, 0.6, 0.4] },
                    affectsAnalytical: false,
                },
                {
                    key: 'kv_base',
                    label: 'k_v/k_h = 0.1  (base)',
                    description: 'The conventional default for a clastic section with no mapped barriers. Breakthrough 0.576, recovery 0.783 — the peak of the ladder, and 35 % later breakthrough than the sealed case for a parameter that is almost never measured directly.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'kv_isotropic',
                    label: 'k_v/k_h = 1  (isotropic)',
                    description: 'Free vertical exchange. Breakthrough 0.599, recovery 0.781 — marginally below the base case, because the section reached vertical equilibrium before this point and the last decade of kv adds nothing. A sensitivity that saturates is worth knowing about: it bounds how much the unmeasured parameter can cost you.',
                    paramPatch: { layerPermsZ: [200, 150, 100, 60, 40] },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'mobility_crossover',
            label: 'Which Way Crossflow Helps',
            description: 'Crossflow is not a mechanism with a fixed sign. It is driven by the difference in total mobility between the flooded and unflooded parts of a layer, so its direction follows the mobility ratio. Three sealed/open pairs at the same geology: at M = 0.5 opening the layers is worth +0.046 of recovery at one pore volume injected (0.735 → 0.781), at M = 2 it is worth +0.000 (0.617 → 0.617), and at M = 10 it costs -0.021 (0.451 → 0.430). Breakthrough tells the same story with more contrast — 0.427 → 0.599, 0.302 → 0.355, and 0.174 → 0.174, where the unfavourable case has effectively stopped responding. The crossover sits between M = 2 and M = 10 rather than at M = 1, because the group that governs is the mobility contrast across the shock front, not the end-point ratio (Zapata and Lake 1981). For a screening study the consequence is blunt: "how good is my vertical communication" has no answer until you also say how mobile the injectant is.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'mob_fav_sealed',
                    label: 'M = 0.5, sealed',
                    description: 'Favourable displacement in isolated layers: breakthrough 0.427 PVI, recovery 0.735.',
                    paramPatch: { layerPermsZ: [2e-4, 1.5e-4, 1e-4, 6e-5, 4e-5] },
                    affectsAnalytical: false,
                },
                {
                    key: 'mob_fav_open',
                    label: 'M = 0.5, isotropic  (crossflow gains 0.046)',
                    description: 'The same rock with the layers open: breakthrough 0.599, recovery 0.781. Water leaving the fast layer into its slower neighbours is being spent on oil it would otherwise have bypassed.',
                    paramPatch: { layerPermsZ: [200, 150, 100, 60, 40] },
                    affectsAnalytical: false,
                },
                {
                    key: 'mob_unit_sealed',
                    label: 'M = 2, sealed  (μ_o = 4 cp)',
                    description: 'A four-centipoise oil in isolated layers: breakthrough 0.302, recovery 0.617.',
                    paramPatch: {
                        mu_o: 4, delta_t_days: 2, steps: 265,
                        layerPermsZ: [2e-4, 1.5e-4, 1e-4, 6e-5, 4e-5],
                    },
                    affectsAnalytical: true,
                },
                {
                    key: 'mob_unit_open',
                    label: 'M = 2, isotropic  (crossflow gains nothing)',
                    description: 'Breakthrough moves out to 0.355 and recovery at one pore volume injected does not move at all — 0.617 either way. The water that crosses over arrives at the producer by a different route and on a different day, and the same amount of oil comes out.',
                    paramPatch: { mu_o: 4, delta_t_days: 2, steps: 265, layerPermsZ: [200, 150, 100, 60, 40] },
                    affectsAnalytical: true,
                },
                {
                    key: 'mob_unfav_sealed',
                    label: 'M = 10, sealed  (μ_o = 20 cp)',
                    description: 'A twenty-centipoise oil in isolated layers: breakthrough 0.174, recovery 0.451. Displacement is poor for reasons that have nothing to do with the layering.',
                    paramPatch: {
                        mu_o: 20, delta_t_days: 4, steps: 310,
                        layerPermsZ: [2e-4, 1.5e-4, 1e-4, 6e-5, 4e-5],
                    },
                    affectsAnalytical: true,
                },
                {
                    key: 'mob_unfav_open',
                    label: 'M = 10, isotropic  (crossflow costs 0.021)',
                    description: 'The sign has reversed: recovery falls to 0.430 and breakthrough does not move at all (0.174). With water far more mobile than the oil it is displacing, the connection between layers is a route into the swept zone rather than out of it.',
                    paramPatch: { mu_o: 20, delta_t_days: 4, steps: 310, layerPermsZ: [200, 150, 100, 60, 40] },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'capillary_crossflow',
            label: 'Capillary Crossflow Needs a Path',
            description: 'The second driving force across a layer boundary, in a 2x2 with a genuinely empty corner. Capillary pressure pulls water from the coarse fast layer into the finer slow ones, but only if there is vertical permeability for it to move through. With the layers sealed, raising the Brooks-Corey entry pressure from 0 to 6 bar changes recovery at one pore volume injected by nothing measurable (0.735 either way) and breakthrough by 0.002 PVI — capillarity along the flow direction is negligible at this drawdown, as `wf_capillary` measures directly. Open the layers and the same 6 bar is worth 0.013 of recovery and 0.039 PVI of breakthrough delay on top of what viscous crossflow already gave (0.781 → 0.794, 0.599 → 0.638). The two effects are super-additive: separately they are worth +0.000 and +0.046, together +0.059. This is the opposite interaction sign to the rock-curves-or-geology study on `sweep_vertical`, where the two mechanisms mask each other — one more reason a one-at-a-time sensitivity cannot tell you which pairs matter.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'cap_sealed_dry',
                    label: 'Sealed, P_e = 0  (reference corner)',
                    description: 'No vertical permeability and no capillary pressure. Breakthrough 0.427 PVI, recovery 0.735.',
                    paramPatch: { layerPermsZ: [2e-4, 1.5e-4, 1e-4, 6e-5, 4e-5] },
                    affectsAnalytical: false,
                },
                {
                    key: 'cap_sealed_pc',
                    label: 'Sealed, P_e = 6 bar  (capillarity alone: nothing)',
                    description: 'A substantial entry pressure with nowhere to act. Breakthrough 0.425, recovery 0.735 — indistinguishable from the corner with no capillarity at all. The mechanism is present in the rock and absent from the result.',
                    paramPatch: {
                        layerPermsZ: [2e-4, 1.5e-4, 1e-4, 6e-5, 4e-5],
                        capillaryEnabled: true, capillaryPEntry: 6,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'cap_open_dry',
                    label: 'Isotropic, P_e = 0  (viscous crossflow alone)',
                    description: 'Vertical permeability with no capillary pressure: breakthrough 0.599, recovery 0.781. The +0.046 is entirely viscous.',
                    paramPatch: { layerPermsZ: [200, 150, 100, 60, 40] },
                    affectsAnalytical: false,
                },
                {
                    key: 'cap_open_pc',
                    label: 'Isotropic, P_e = 6 bar  (both: +0.059)',
                    description: 'Breakthrough 0.638 PVI and recovery 0.794 — more than the two effects add up to separately, because capillary suction into the fine layers is only available once the viscous exchange has established a path and a saturation contrast for it to act on.',
                    paramPatch: {
                        layerPermsZ: [200, 150, 100, 60, 40],
                        capillaryEnabled: true, capillaryPEntry: 6,
                    },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
