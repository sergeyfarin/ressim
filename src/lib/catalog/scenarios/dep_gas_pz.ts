import { gasDriveLivePanels } from '../chartPanels/gasLivePanels';
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
        group: 'flow-regimes-decline',
        role: 'simulation',
        caseMode: '3p',
        parameterSummary: 'Dry-gas depletion · p/z material balance · what bends the straight line and what it costs in reserves',
    },
    description: 'A dry-gas reservoir depleting from 400 bar, plotted the way gas reserves are actually estimated: p/z against cumulative production, extrapolated to zero. For a volumetric reservoir with a rigid pore volume that plot is a straight line whose x-intercept is the gas initially in place, and the base case here lands on it. The case then removes one assumption at a time. Raise pore compressibility to a geopressured value and the plot bows upward, because compaction and connate-water expansion are supporting pressure and the straight line reads that extra energy as extra gas. Put a tight interval between the well and most of the reservoir and the plot flatters even harder — it only knows about the compartment being drained, and reads a slow-feeding neighbour as extra gas. In each case the simulation is right and the interpretation is wrong, which is the point: the gas is where the volumetrics say it is, and the line is answering a question about the pore volume rather than about the reservoir. z is taken from the same gas table the simulator integrates, so the reference is never separated from the simulation by a correlation choice.',
    analyticalMethodSummary: 'Dry-gas material balance, G_p/G = 1 − (p/z)/(p_i/z_i), against cumulative gas produced. Two curves are drawn: the volumetric straight line, which assumes a rigid pore volume and no water influx, and the Ramagost–Farshad form that restores the pore and connate-water compressibility terms. Both use the gas initially in place computed from grid volumes, not fitted, so the vertical gap to the simulation is a reserves error rather than a curve fit.',
    analyticalMethodReference: 'Craft & Hawkins, Applied Petroleum Reservoir Engineering, ch. 5; Dake (1978), Fundamentals of Reservoir Engineering, §1.6 and ch. 3; Ramagost, B.P. & Farshad, F.F. (1981), "P/Z Abnormally Pressured Gas Reservoirs", SPE 10125; Fetkovich, Reese & Whitson (1998), SPEJ 3(1); Payne, D.A. (1996), "Material Balance Calculations in Tight Gas Reservoirs", SPERE 11(4).',
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
    liveChartPanels: gasDriveLivePanels,
    sensitivities: [
        {
            key: 'pore_compressibility',
            label: 'Pore Compressibility',
            description: 'The same gas, the same grid, the same well, with only the pore compressibility changing across the range real rock spans — from a consolidated sandstone to a geopressured, poorly consolidated one. Extrapolate the stabilised part of each p/z plot to zero and the reserves it claims come out 0.8 %, 1.7 %, 4.7 % and 10.5 % above the volumetric gas in place. Nothing about the gas changed; what changed is how much of the pressure support came from the rock closing in and the connate water expanding, and a straight line has no term for either, so it books that energy as gas. The last rung produces 1.03 times the gas the volumetrics said was there — more than was ever "in place" — which is not an error but the compaction term arriving in the production stream. The second reference curve on the chart is the Ramagost–Farshad form with those terms restored; where the simulation leaves the straight line it stays with the corrected one.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'cf_normal',
                    label: 'c_f = 5e-6 /bar  (consolidated, base)',
                    description: 'A competent sandstone, where the pore volume is effectively rigid. Recovery 0.929 of the gas in place at a 30 bar abandonment pressure, and the straight line over-reads reserves by 0.8 % — the reference case, where the assumption holds and the plot earns its reputation.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'cf_moderate',
                    label: 'c_f = 5e-5 /bar',
                    description: 'An order of magnitude softer, and still nearly invisible: reserves error 1.7 %, recovery 0.938. The mechanism is present and does not yet matter, which is why it is so often left out.',
                    paramPatch: { rock_compressibility: 5e-5 },
                    affectsAnalytical: true,
                },
                {
                    key: 'cf_high',
                    label: 'c_f = 2e-4 /bar',
                    description: 'Now it matters: reserves error 4.7 %, recovery 0.969. The plot is visibly bowed above its own chord, and an engineer drawing a ruler through the late points is already booking gas that is not there.',
                    paramPatch: { rock_compressibility: 2e-4 },
                    affectsAnalytical: true,
                },
                {
                    key: 'cf_geopressured',
                    label: 'c_f = 5e-4 /bar  (geopressured)',
                    description: 'The abnormally pressured case Ramagost and Farshad wrote about. Reserves error 10.5 %, and recovery reaches 1.029 of the volumetric gas in place — the reservoir delivers more gas than the initial volume contained, because compaction and water expansion keep pushing it out as the pressure falls. This is the rung where using the uncorrected line is a commercial mistake rather than a rounding one.',
                    paramPatch: { rock_compressibility: 5e-4 },
                    affectsAnalytical: true,
                },
            ],
        },
        {
            key: 'connectivity',
            label: 'One Tank or Two?',
            description: 'A tight interval between the well and the lower three layers, with the producer perforating only the top compartment. Material balance is still exact — it is the *reservoir* the plot has stopped describing. The connected section recovers 0.929 and ends at 30.3 bar; the compartmentalised one recovers 0.278 and ends at 293 bar, because most of the gas is behind a barrier that has barely begun to feed. Extrapolating that history gives a gas in place 39 % above the volumetric figure, and the direction is the dangerous one: the plot is flattering, the reserves look larger than they are, and the giveaway is not in the p/z plot at all but in an average reservoir pressure that has hardly moved while the well pressure is on the floor. Payne (1996) is the reference for the same trap in tight gas.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'conn_one_tank',
                    label: 'Connected section  (base)',
                    description: 'Vertical permeability one tenth of horizontal throughout — one tank in every practical sense. Recovery 0.929, final average pressure 30.3 bar, straight-line reserves 0.8 % high.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'conn_sealed',
                    label: 'Tight interval, well in the upper compartment',
                    description: 'The same gas in the same rock, with 1e-6 of the horizontal permeability across two layer boundaries and the well perforating only the top layer. Recovery 0.278 after the same 4,000 days, final average pressure 293 bar, and a straight-line reserves estimate 39 % above the truth.',
                    paramPatch: { permMode: 'perLayer', layerPermsZ: SEALED_KV, producerKLayers: [0] },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'abandonment',
            label: 'How Far the Line Is Drawn',
            description: 'The same reservoir read at two points in its life. Stopping at a 200 bar abandonment pressure — half the initial pressure, and a perfectly ordinary place to be when the reserves report is due — leaves 0.501 recovery and a straight-line estimate 9.0 % high, against 0.8 % for the run taken to 30 bar. No parameter differs between the two; the only difference is how much of the curve there was to draw a line through. A p/z extrapolation is a statement about the future made from the part of the past you happen to have, and it is at its worst exactly when it is most needed.',
            analyticalOverlayMode: 'per-result',
            variants: [
                {
                    key: 'aband_30',
                    label: 'Produce to 30 bar  (base)',
                    description: 'The full depletion: recovery 0.929, reserves error 0.8 %. Two thirds of the p/z plot is available to fit.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'aband_200',
                    label: 'Produce to 200 bar',
                    description: 'The same run stopped at half its initial pressure: recovery 0.501 and reserves error 9.0 %, eleven times the fully depleted case, from a history that looks perfectly linear on its own.',
                    paramPatch: { producerBhp: 200 },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
