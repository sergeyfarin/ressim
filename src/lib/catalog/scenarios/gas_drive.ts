import { gasDriveLivePanels } from '../chartPanels/gasLivePanels';
import type { Scenario } from '../scenarios';
import { generateBlackOilTable } from '../../physics/pvt';

/**
 * Solution gas drive — the classical below-bubble-point depletion mechanism.
 *
 * The reservoir starts *saturated*: initial pressure equals the bubble point,
 * so the oil already holds its maximum dissolved gas and a small free-gas
 * volume is present. As the producer draws pressure down, Rs(P) falls, gas
 * comes out of solution, Sg climbs past the critical gas saturation, and gas
 * relative permeability turns on — the producing GOR rises sharply while oil
 * rate declines. That trajectory is only representable with a black-oil PVT
 * table; a constant-PVT approximation reproduces free-gas expansion but not
 * liberation, which is the dominant term here.
 *
 * PVT comes from `generateBlackOilTable()` (Standing correlations), the same
 * generator used by `dep_pvt`. Oil viscosity and Bo are therefore table
 * properties, not the scalar `mu_o` — the scalar is retained only as the
 * two-phase fallback and is inert while `pvtMode` is `black-oil`. The oil
 * sensitivity dimension consequently varies API gravity, which moves Rs, Bo
 * and mu_o together the way a different crude actually would, rather than
 * varying a viscosity the black-oil path would ignore.
 *
 * Reference: Craft & Hawkins, *Applied Petroleum Reservoir Engineering*,
 * solution-gas-drive material balance; Standing (1947) correlations.
 *
 * Graded against an OPM Flow reference — see `docs/THREE_PHASE_VALIDATION.md`.
 */

const GAS_SPECIFIC_GRAVITY = 0.75;
const RESERVOIR_TEMP_C = 80;
// Saturated start: bubble point == initial pressure, so depletion immediately
// liberates gas rather than first traversing an undersaturated leg.
const BUBBLE_POINT_BAR = 200;
const PVT_TABLE_PMAX_BAR = 300;
const PVT_TABLE_POINTS = 20;
const UNDERSATURATED_C_O_PER_BAR = 1e-5;

const makeTable = (api: number) =>
    generateBlackOilTable(
        api, GAS_SPECIFIC_GRAVITY, RESERVOIR_TEMP_C,
        BUBBLE_POINT_BAR, PVT_TABLE_PMAX_BAR, PVT_TABLE_POINTS,
        UNDERSATURATED_C_O_PER_BAR,
    );

const PVT_TABLE_BASE = makeTable(35);
const PVT_TABLE_LIGHT = makeTable(45);
const PVT_TABLE_HEAVY = makeTable(22);

export const gas_drive: Scenario = {
    key: 'gas_drive',
    label: 'Solution Gas Drive',
    catalog: {
        group: 'simulation-only',
        role: 'simulation',
        caseMode: '3p',
        parameterSummary: 'Saturated black-oil depletion · solution-gas liberation · OPM Flow reference',
    },
    description: 'Pressure depletion of a saturated black-oil reservoir. Initial pressure sits at the bubble point, so drawdown immediately liberates gas from solution: Rs falls, free gas builds past the critical gas saturation, and producing GOR climbs while oil rate declines. Vary the initial free-gas volume, the oil gravity, or the permeability to see how each changes the strength and timing of the gas-expansion drive.',
    analyticalMethodSummary: 'Simulation-only — no analytical overlay. A Tarner–Tracy tank model was evaluated but rejected for this chart because its uniform-pressure/saturation assumptions do not represent the localized BHP drawdown and initially mobile free gas; its producing GOR diverges strongly even though ResSim agrees with OPM Flow. The optional OPM benchmark is disabled by default.',
    analyticalMethodReference: 'Standing (1979), Notes on Calculating Solution Gas Drive Reservoir Performance by Tarner’s Method (assumption review); Craft & Hawkins, Applied Petroleum Reservoir Engineering; quantitative grading: docs/THREE_PHASE_VALIDATION.md.',
    referenceSources: [{ kind: 'opm-flow', artifactKeys: ['gas_drive'], defaultVisible: false }],
    chartLayoutKey: 'gas',
    defaultSensitivityDimensionKey: 'sg_init',
    capabilities: {
        analyticalMethod: 'none',
        hasInjector: false,
        default3DScalar: 'saturation_gas',
        requiresThreePhaseMode: true,
    },
    solverPolicy: {
        defaultSolver: 'fim',
        rationale: 'FIM is required for coupled dissolved/free-gas evolution, phase appearance, black-oil PVT, and producing-GOR behavior during depletion.',
    },
    params: {
        nx: 20, ny: 1, nz: 1,
        cellDx: 50, cellDy: 50, cellDz: 10,
        initialPressure: 200,
        initialSaturation: 0.2,
        initialGasSaturation: 0.08,
        pvtMode: 'black-oil',
        pvtTable: PVT_TABLE_BASE,
        gasRedissolutionEnabled: true,
        reservoirPorosity: 0.2,
        uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10,
        permMode: 'uniform',
        s_wc: 0.2, s_or: 0.15,
        s_gc: 0.05, s_gr: 0.05, s_org: 0.20,
        n_w: 2.0, n_o: 2.0, n_g: 1.5,
        k_rw_max: 0.4, k_ro_max: 1.0, k_rg_max: 0.8,
        mu_w: 0.5, mu_o: 2.0, mu_g: 0.02,
        c_w: 3e-6, c_o: 1e-5, c_g: 1e-4,
        // rho_g is a *surface* gas density in the black-oil density relations
        // (pvt.rs: rho_o = (rho_o_sc + Rs*rho_g_sc)/Bo), so it must be the surface density of
        // the 0.75-gravity gas the PVT table was generated with, not a reservoir-condition
        // value. Same number as the OPM deck's DENSITY record.
        rho_w: 1000, rho_o: 800, rho_g: 0.9172,
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
        fimEnabled: true,
    },
    sensitivities: [
        {
            key: 'sg_init',
            label: 'Initial Gas Saturation',
            description: 'Vary the initial gas saturation to explore the effect of free gas volume at the start of pressure depletion.',
            variants: [
                {
                    key: 'sg_low',
                    label: 'S_g = 0.05',
                    description: 'At critical gas saturation — gas is initially immobile.',
                    paramPatch: { initialGasSaturation: 0.05 },
                    affectsAnalytical: false,
                },
                {
                    key: 'sg_base',
                    label: 'S_g = 0.08  (base)',
                    description: 'Base case — initial free gas slightly above critical saturation.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'sg_high',
                    label: 'S_g = 0.15',
                    description: 'High initial gas saturation — more free gas expansion support.',
                    paramPatch: { initialGasSaturation: 0.15 },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'oil_gravity',
            label: 'Oil Gravity',
            description: 'Vary the crude API gravity. Under black-oil PVT the oil viscosity, formation volume factor and solution gas-oil ratio all come from the PVT table, so they move together: a lighter crude carries more dissolved gas at the same bubble point and is far less viscous, giving a stronger, faster gas drive.',
            variants: [
                {
                    key: 'api_light',
                    label: '45° API  (light)',
                    description: 'Light crude — high Rs, low viscosity: more liberated gas and higher oil mobility.',
                    paramPatch: { pvtTable: PVT_TABLE_LIGHT },
                    affectsAnalytical: false,
                },
                {
                    key: 'api_base',
                    label: '35° API  (base)',
                    description: 'Base crude.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'api_heavy',
                    label: '22° API  (heavy)',
                    description: 'Heavy crude — low Rs, high viscosity: weaker gas drive and slower oil production.',
                    paramPatch: { pvtTable: PVT_TABLE_HEAVY },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'perm',
            label: 'Permeability',
            description: 'Permeability controls total flow rate, affecting how quickly the reservoir depressurizes.',
            variants: [
                {
                    key: 'perm_low',
                    label: 'k = 10 mD  (tight)',
                    description: 'Low permeability — slower depletion.',
                    paramPatch: { uniformPermX: 10, uniformPermY: 10, uniformPermZ: 1 },
                    affectsAnalytical: false,
                },
                {
                    key: 'perm_base',
                    label: 'k = 100 mD  (base)',
                    description: 'Base permeability.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'perm_high',
                    label: 'k = 1000 mD  (high)',
                    description: 'High permeability — faster depletion.',
                    paramPatch: { uniformPermX: 1000, uniformPermY: 1000, uniformPermZ: 100 },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
    liveChartPanels: gasDriveLivePanels,
};
