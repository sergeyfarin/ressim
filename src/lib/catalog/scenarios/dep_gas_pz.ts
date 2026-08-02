import type { Scenario } from '../scenarios';
import { gasMaterialBalanceDef } from '../analyticalAdapters';
import { generateBlackOilTable } from '../../physics/pvt';

/**
 * The p/z plot, and the three things that bend it.
 *
 * Material balance does not fail. It is an exact statement about volumes, and
 * the straight line drawn on a p/z plot is not material balance — it is
 * material balance plus the assumption that the pore volume never changes and
 * the pressure you plotted is the whole reservoir's. This case removes those
 * assumptions one at a time and measures what the resulting reserves estimate
 * does, which is the only part an engineer is paid to get right.
 *
 * The reference curve and the simulation share one gas law. z is recovered from
 * the same B_g table the simulator integrates rather than from an independent
 * correlation, so a gap between the two curves is a statement about the
 * reservoir and never about which z-factor correlation was picked.
 */

const RESERVOIR_TEMPERATURE_C = 90;
const GAS_SPECIFIC_GRAVITY = 0.65;
/**
 * Surface gas density [kg/m3] for the gravity above, at the standard conditions
 * the engine's own B_g is referenced to (14.7 psia, 60 F, air 1.2232 kg/m3).
 * The black-oil density relations use this to convert reservoir gas to surface
 * volumes, so it must match the gravity the PVT table was generated with — a
 * mismatch shows up as a several-percent disagreement between produced gas and
 * gas in place, which on a p/z plot is indistinguishable from real physics.
 */
const SURFACE_GAS_DENSITY = 0.7951;

const PVT_TABLE = generateBlackOilTable(
    35, GAS_SPECIFIC_GRAVITY, RESERVOIR_TEMPERATURE_C,
    // Bubble point at the bottom of the range: this reservoir has no oil, and
    // the table is carried for its gas branch (B_g, mu_g) alone.
    20, 450, 60, 1e-4,
);

/** Layer permeabilities [mD] for the connected and compartmentalised cases. */
const CONNECTED_KH = [5, 5, 5, 5, 5];
const CONNECTED_KV = [0.5, 0.5, 0.5, 0.5, 0.5];
const SEALED_KV = [0.5, 1e-6, 1e-6, 0.5, 0.5];

export const dep_gas_pz: Scenario = {
    key: 'dep_gas_pz',
    label: 'Gas Reserves from p/z',
    catalog: {
        group: 'material-balance-drive',
        role: 'simulation',
        caseMode: '3p',
        parameterSummary: 'Dry-gas depletion · p/z material balance · what bends the straight line and what it costs in reserves',
    },
    description: 'Dry-gas depletion shown as p/z versus cumulative gas produced. Compare the volumetric extrapolation with compaction and compartmentalization cases.',
    analyticalMethodSummary: 'Dry-gas material balance, G_p/G = 1 − (p/z)/(p_i/z_i), against cumulative gas produced. Two curves are drawn: the volumetric straight line, which assumes a rigid pore volume and no water influx, and the Ramagost–Farshad form that restores the pore and connate-water compressibility terms. Both use the gas initially in place computed from grid volumes, not fitted, so the vertical gap to the simulation is a reserves error rather than a curve fit.',
    analyticalMethodReference: 'Craft & Hawkins, Applied Petroleum Reservoir Engineering, ch. 5; Dake (1978), Fundamentals of Reservoir Engineering, §1.6 and ch. 3; Ramagost, B.P. & Farshad, F.F. (1981), "P/Z Abnormally Pressured Gas Reservoirs", SPE 10125; Fetkovich, Reese & Whitson (1998), SPEJ 3(1); Payne, D.A. (1996), "Material Balance Calculations in Tight Gas Reservoirs", SPERE 11(4).',
    // OPM Flow on the same reservoir at both ends of the compressibility ladder.
    // It earned its place: it disagreed with ResSim by a factor of six on the
    // compaction term, and it was right. See TODO.md and the engine fix of
    // 2026-08-01.
    referenceSources: [{
        kind: 'opm-flow',
        artifactKeys: ['dep_gas_pz', 'dep_gas_pz_geopressured'],
        // No `artifactVariantLabels` here on purpose: these two decks already
        // carry their rung in every curve label ("… (c_f = 5e-6)"), and their
        // legend toggles fall back to the artifacts' own distinct names. Adding
        // a suffix would print the rung twice.
    }],
    chartLayoutKey: 'gas_material_balance',
    defaultSensitivityDimensionKey: 'pore_compressibility',
    capabilities: {
        analyticalMethod: 'gas-material-balance',
        hasInjector: false,
        default3DScalar: 'pressure',
        spatialProfile: { defaultAxis: 'i' },
        requiresThreePhaseMode: true,
    },
    solverPolicy: {
        defaultSolver: 'fim',
        rationale: 'FIM is required for black-oil gas PVT and pressure-dependent gas properties through a 400 bar depletion; the explicit path is not offered as an equivalent here.',
    },
    params: {
        // Fluid. The oil branch of the table is inert — this reservoir has no
        // oil — but the black-oil machinery still wants the properties defined.
        mu_w: 0.4,
        mu_o: 1.0,
        mu_g: 0.02,
        c_o: 1e-5,
        c_w: 4e-5,
        c_g: 1e-4,
        rock_compressibility: 5e-6,
        depth_reference: 0,
        volume_expansion_o: 1,
        volume_expansion_w: 1,
        rho_w: 1000,
        rho_o: 800,
        rho_g: SURFACE_GAS_DENSITY,
        pvtMode: 'black-oil',
        pvtTable: PVT_TABLE,
        initialRs: 0,
        gasRedissolutionEnabled: false,
        reservoirTemperature: RESERVOIR_TEMPERATURE_C,
        // Rock / relative permeability. Water sits at connate and never moves;
        // residual oil is zero because there is no oil to leave behind.
        reservoirPorosity: 0.15,
        s_wc: 0.2,
        s_or: 0,
        s_gc: 0.02,
        s_gr: 0.02,
        s_org: 0,
        n_w: 2,
        n_o: 2,
        n_g: 1.5,
        k_rw_max: 0.4,
        k_ro_max: 1,
        k_rg_max: 0.9,
        capillaryEnabled: false,
        capillaryPEntry: 0,
        capillaryLambda: 2,
        pcogEnabled: false,
        pcogPEntry: 3,
        pcogLambda: 2,
        // Grid: 2,000 m x 100 m x 20 m section in five layers — 600,000 m3 pore
        // volume, of which 80 % is gas.
        nx: 20,
        ny: 1,
        nz: 5,
        cellDx: 100,
        cellDy: 100,
        cellDz: 4,
        // Uniform in the base case: the layers exist so the compartment variant
        // has a boundary to seal, not because the rock is layered.
        permMode: 'uniform',
        uniformPermX: 5,
        uniformPermY: 5,
        uniformPermZ: 0.5,
        layerPermsX: CONNECTED_KH,
        layerPermsY: CONNECTED_KH,
        layerPermsZ: CONNECTED_KV,
        // Initial conditions: connate water and free gas, no oil at all.
        initialPressure: 400,
        initialSaturation: 0.2,
        initialGasSaturation: 0.8,
        // Wells — one producer on a 30 bar abandonment pressure, perforating the
        // whole section unless a variant says otherwise.
        injectorEnabled: false,
        injectorControlMode: 'pressure',
        producerControlMode: 'pressure',
        injectorBhp: 700,
        producerBhp: 30,
        targetInjectorRate: 0,
        targetProducerRate: 0,
        injectorI: 0,
        injectorJ: 0,
        // Centred, because a tank material balance has no well-position term
        // and an edge well would only invite the question.
        producerI: 10,
        producerJ: 0,
        well_radius: 0.1,
        well_skin: 0,
        threePhaseModeEnabled: true,
        injectedFluid: 'gas',
        // 4,000 days is long enough for the connected cases to reach the
        // abandonment pressure and for the compartmentalised one to show both
        // of its slopes.
        fimEnabled: true,
        delta_t_days: 20,
        steps: 200,
        max_sat_change_per_step: 0.1,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    analyticalDef: gasMaterialBalanceDef,
    sensitivities: [
        {
            key: 'pore_compressibility',
            label: 'Pore Compressibility',
            description: 'Higher pore compressibility supports pressure and bends the p/z trend above the rigid-volume line.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'cf_normal',
                    label: 'c_f = 5e-6 /bar  (consolidated, base)',
                    description: 'Nearly rigid pore volume; the volumetric p/z assumption is appropriate.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'cf_moderate',
                    label: 'c_f = 5e-5 /bar',
                    description: 'Moderate rock compressibility with little visible p/z curvature.',
                    paramPatch: { rock_compressibility: 5e-5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'cf_high',
                    label: 'c_f = 2e-4 /bar',
                    description: 'High compressibility; pressure support becomes visible.',
                    paramPatch: { rock_compressibility: 2e-4 },
                    affectsAnalytical: true,
                },
                {
                    key: 'cf_geopressured',
                    label: 'c_f = 5e-4 /bar  (geopressured)',
                    description: 'Geopressured case where compaction materially affects the p/z estimate.',
                    paramPatch: { rock_compressibility: 5e-4 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'connectivity',
            label: 'One Tank or Two?',
            description: 'Compare a connected tank with a producer isolated from most of the gas volume.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'conn_one_tank',
                    label: 'Connected section  (base)',
                    description: 'Connected layers behave as one tank.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'conn_sealed',
                    label: 'Tight interval, well in the upper compartment',
                    description: 'Sealed layer boundaries leave most gas poorly connected to the producer.',
                    paramPatch: { permMode: 'perLayer', layerPermsZ: SEALED_KV, producerKLayers: [0] },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'abandonment',
            label: 'How Far the Line Is Drawn',
            description: 'Compare reserves estimates from an early history and from depletion to the 30 bar abandonment limit.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'aband_30',
                    label: 'Produce to 30 bar  (base)',
                    description: 'Run to the 30 bar abandonment limit.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'aband_200',
                    label: 'Produce to 200 bar',
                    description: 'Stop at 200 bar and extrapolate from the shorter history.',
                    paramPatch: { producerBhp: 200 },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
