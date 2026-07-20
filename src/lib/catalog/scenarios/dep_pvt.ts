import { gasDriveLivePanels } from '../chartPanels/gasLivePanels';
import type { Scenario } from '../scenarios';
import { generateBlackOilTable } from '../../physics/pvt';

/**
 * "Two fluid models, one calibration point" — PVT representation risk.
 *
 * Physics basis: two black-oil PVT tables are built from the same
 * correlation inputs (API, gas gravity, temperature, bubble point) via
 * generateBlackOilTable(), so they are numerically IDENTICAL at and below
 * the bubble point — same Rs(P), same Bo(P), the one point a single flash
 * test can directly calibrate. They differ only in the assumed
 * undersaturated oil compressibility (1e-5/bar "correlation" vs 1e-4/bar
 * "lab report") — a value a single flash test cannot pin down, since it
 * governs behavior only *above* the bubble point.
 *
 * Verified (headless wasm probe, 2026-07-16, three-phase mode required —
 * a two-phase oil/water run does not consult the PVT table's undersaturated
 * Bo trend at all, only the scalar c_o): the two tables produce genuinely
 * different average-pressure and GOR trajectories above the bubble point,
 * converging once pressure drops back into the saturated region where both
 * tables share the identical, directly-calibrated Rs(P)/Bo(P) branch. The
 * divergence is real but modest (order 0.1% early, growing slowly with
 * distance from the bubble point) — undersaturated oil compressibility is
 * inherently a second-order effect on Bo (Bo = Bo_pb * exp(-c_o * dP)), so
 * even a 10x disagreement in c_o cannot produce a dramatic swing the way
 * OOIP/N ambiguity (dep_nct) or a kv/gravity interaction (wf_tornado) can.
 * That itself is the honest lesson: PVT-model risk from unmeasured
 * undersaturated compressibility is bounded, and matters most for large,
 * sustained excursions above the bubble point — not for typical depletion.
 *
 * References: Standing (1947) correlations; McCain, "The Properties of
 * Petroleum Fluids" on undersaturated-oil PVT uncertainty; the general
 * "representation risk" framing extends dep_nct's material-balance
 * non-uniqueness point to the PVT-table axis.
 *
 * No analytical reference: this is a three-phase black-oil blowdown: the
 * Dietz depletion model (used by dep_decline/dep_pss/dep_arps) is an
 * oil-only PSS model and does not represent gas liberation/expansion, so no
 * honest quantitative overlay exists here (same precedent as gas_drive) —
 * the comparison is between the two simulated PVT-table variants directly.
 */

// Correlation inputs shared by both tables — this is the ONE calibration
// point (API, gas gravity, temperature, bubble point) both variants agree on.
const API_GRAVITY = 35;
const GAS_SPECIFIC_GRAVITY = 0.75;
const RESERVOIR_TEMP_C = 80;
const BUBBLE_POINT_BAR = 150;
const PVT_TABLE_PMAX_BAR = 300;
const PVT_TABLE_POINTS = 20;

const PVT_TABLE_CORRELATION = generateBlackOilTable(
    API_GRAVITY, GAS_SPECIFIC_GRAVITY, RESERVOIR_TEMP_C,
    BUBBLE_POINT_BAR, PVT_TABLE_PMAX_BAR, PVT_TABLE_POINTS,
    1e-5, // "correlation" undersaturated compressibility
);
const PVT_TABLE_LAB_REPORT = generateBlackOilTable(
    API_GRAVITY, GAS_SPECIFIC_GRAVITY, RESERVOIR_TEMP_C,
    BUBBLE_POINT_BAR, PVT_TABLE_PMAX_BAR, PVT_TABLE_POINTS,
    1e-4, // "lab report" undersaturated compressibility — 10x steeper Bo decline above Pb
);

export const dep_pvt: Scenario = {
    key: 'dep_pvt',
    label: 'Two Fluid Models, One Calibration Point',
    description: 'Single-well black-oil blowdown starting above the bubble point. Two PVT tables share the exact same bubble-point pressure, Rs, and Bo — the one point a flash test directly measures — but assume different undersaturated oil compressibility above it (a value no single flash test can pin down). Watch Avg Pressure and GOR diverge while undersaturated, then reconverge once pressure drops back below the bubble point into the shared, directly-calibrated branch. The divergence is real but modest — that bound is itself the lesson.',
    analyticalMethodSummary: 'Simulation-only — no analytical overlay. The Dietz PSS depletion model used elsewhere in this catalog is oil-only and does not represent gas liberation, so no honest quantitative reference exists for a black-oil blowdown (same precedent as gas_drive).',
    analyticalMethodReference: 'Standing (1947); McCain, "The Properties of Petroleum Fluids".',
    chartLayoutKey: 'gas',
    defaultSensitivityDimensionKey: 'pvt_model',
    capabilities: {
        analyticalMethod: 'none',
        showSweepPanel: false,
        hasInjector: false,
        default3DScalar: 'saturation_gas',
        requiresThreePhaseMode: true,
    },
    params: {
        // Fluid
        mu_w: 0.5,
        mu_o: 1.0,
        mu_g: 0.02,
        c_o: 1e-5,
        c_w: 3e-6,
        c_g: 1e-4,
        rock_compressibility: 1e-6,
        depth_reference: 0,
        volume_expansion_o: 1.1,
        volume_expansion_w: 1,
        rho_w: 1000,
        rho_o: 800,
        rho_g: 10.0,
        // Rock / rel perm (oil-water)
        reservoirPorosity: 0.2,
        s_wc: 0.1,
        s_or: 0.1,
        n_w: 2,
        n_o: 2,
        k_rw_max: 1,
        k_ro_max: 1,
        // Rel perm (gas, Corey fallback — no scalTables supplied)
        s_gc: 0.05,
        s_gr: 0.05,
        s_org: 0.15,
        n_g: 1.5,
        k_rg_max: 1,
        capillaryEnabled: false,
        capillaryPEntry: 0,
        capillaryLambda: 2,
        // Grid: tight single-cell-column 1D slab, 48 x 1 x 1, 480 m x 10 m x 10 m.
        // Low permeability keeps the reservoir undersaturated for a long enough
        // window for the two tables' Bo trends to visibly separate before the
        // system depletes down through the bubble point.
        nx: 48,
        ny: 1,
        nz: 1,
        cellDx: 10,
        cellDy: 10,
        cellDz: 10,
        permMode: 'uniform',
        uniformPermX: 0.5,
        uniformPermY: 0.5,
        uniformPermZ: 0.05,
        // Initial conditions: undersaturated start, well above the bubble point
        initialPressure: 280,
        initialSaturation: 0.1,
        initialGasSaturation: 0,
        // PVT
        pvtMode: 'black-oil',
        pvtTable: PVT_TABLE_CORRELATION,
        threePhaseModeEnabled: true,
        gasRedissolutionEnabled: true,
        // Wells: single producer, no injector
        injectorEnabled: false,
        injectorControlMode: 'pressure',
        producerControlMode: 'pressure',
        injectorBhp: 500,
        producerBhp: 30,
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
        delta_t_days: 0.5,
        steps: 300,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    liveChartPanels: gasDriveLivePanels,
    sensitivities: [
        {
            key: 'pvt_model',
            label: 'PVT Table (Undersaturated Compressibility)',
            description: 'Both tables share the identical bubble-point pressure, Rs, and Bo — the one calibrated point — and diverge only above it, where undersaturated compressibility cannot be measured by a single flash test. Watch Avg Pressure (Diagnostics panel) and GOR diverge while pressure stays above 150 bar, then reconverge once it drops below.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'pvt_correlation',
                    label: 'Correlation  (c_o = 1e-5/bar above Pb)',
                    description: 'Standing-correlation-derived undersaturated compressibility — base case.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'pvt_lab_report',
                    label: 'Lab Report  (c_o = 1e-4/bar above Pb)',
                    description: 'A different, equally plausible undersaturated compressibility — 10x steeper Bo decline above the bubble point, identical Rs/Bo at and below it.',
                    paramPatch: { pvtTable: PVT_TABLE_LAB_REPORT },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
