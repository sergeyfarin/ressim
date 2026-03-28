import type { Scenario, PublishedReferenceSeries } from '../scenarios';
import type { ThreePhaseScalTables } from '../../simulator-types';

/**
 * SPE1 Comparative Solution Project — Case 1 (DRSDT = 0, no gas re-dissolution).
 *
 * Reference: Odeh, A.S. (1981) "Comparison of Solutions to a Three-Dimensional
 * Black-Oil Reservoir Simulation Problem", JPT, January 1981, pp. 13–25 (SPE 9723).
 *
 * 10×10×3 reservoir with 3 layers of different thickness and permeability.
 * Gas injector in layer 1 at (0,0), oil producer in layer 3 at (9,9).
 * Oil is initially undersaturated (P_i > P_b). Water is immobile.
 *
 * Key physics exercised:
 * - Black-oil PVT with pressure-dependent Bo, Rs, Bg, μ_o, μ_g
 * - Three-phase transport (oil, gas, immobile water)
 * - Per-layer cell thickness and permeability
 * - Per-layer well completions
 * - Gravity in a layered system
 *
 * Approximations vs. the original SPE1 specification:
 * - Three-phase oil relperm still uses Stone II interpolation over the exact SWOF/SGOF inputs
 * - Surface-rate controls are converted to reservoir rates from local PVT/state each step,
 *   which is closer to the deck semantics but still not a full scheduler/well-model implementation
 * - Case 1 disables gas re-dissolution (`DRSDT = 0` semantics), but full schedule/deck semantics are still not implemented
 *
 * Published benchmark reference data is overlaid for visual comparison:
 * - OPM-derived yearly samples for average pressure and GOR
 * - brontosaurus `expected_summary.csv` representative samples for oil rate and well BHP
 *
 * Current verification status:
 * - Pressure mapping is treated as average reservoir pressure versus time.
 * - GOR mapping is treated as producing GOR versus time.
 * - OPM `SPE1CASE1.DATA` confirms the Case 1 controls are:
 *   `WCONPROD 'PROD' 'OPEN' 'ORAT' 20000 ... 1000 /` and
 *   `WCONINJE 'INJ' 'GAS' 'OPEN' 'RATE' 100000 ... 9014 /`.
 * - Well controls now follow the deck's surface-rate intent more closely, but do not yet reproduce the published
 *   pressure peak-and-decline shape closely enough.
 */

// ── PVT table (SPE1 PVTO rows, converted to metric) ─────────────────────────
// Conversions: P_bar = P_psia / 14.5038
//              Rs_m3m3 = Rs_Mscf_stb × 178.108
//              Bo_m3m3 = Bo_rb_stb (dimensionless, same numeric value)
//              Bg_m3m3 = Bg_rb_Mscf × 0.158987 / 28.3168
//              μ_o, μ_g unchanged (cP)
// Repeated-Rs rows encode undersaturated PVTO continuations above bubble point.
// The scalar c_o remains as a fallback for branches without explicit continuation
// data and for pressures above the tabulated continuation range.
const SPE1_PVT_TABLE = [
    { p_bar:   1.01, rs_m3m3:   0.18, bo_m3m3: 1.062, mu_o_cp: 1.040, bg_m3m3: 0.9361, mu_g_cp: 0.0080 },
    { p_bar:  18.25, rs_m3m3:  16.12, bo_m3m3: 1.150, mu_o_cp: 0.975, bg_m3m3: 0.0679, mu_g_cp: 0.0096 },
    { p_bar:  35.49, rs_m3m3:  32.06, bo_m3m3: 1.207, mu_o_cp: 0.910, bg_m3m3: 0.0352, mu_g_cp: 0.0112 },
    { p_bar:  69.96, rs_m3m3:  66.08, bo_m3m3: 1.295, mu_o_cp: 0.830, bg_m3m3: 0.0179, mu_g_cp: 0.0140 },
    { p_bar: 138.91, rs_m3m3: 113.29, bo_m3m3: 1.435, mu_o_cp: 0.695, bg_m3m3: 0.00906, mu_g_cp: 0.0189 },
    { p_bar: 173.38, rs_m3m3: 138.03, bo_m3m3: 1.500, mu_o_cp: 0.641, bg_m3m3: 0.00727, mu_g_cp: 0.0208 },
    { p_bar: 207.85, rs_m3m3: 165.64, bo_m3m3: 1.565, mu_o_cp: 0.594, bg_m3m3: 0.00607, mu_g_cp: 0.0228 },
    { p_bar: 276.79, rs_m3m3: 226.20, bo_m3m3: 1.695, mu_o_cp: 0.510, bg_m3m3: 0.00455, mu_g_cp: 0.0268 },
    { p_bar: 621.54, rs_m3m3: 226.20, bo_m3m3: 1.579, mu_o_cp: 0.740, bg_m3m3: 0.00455, mu_g_cp: 0.0268 },
    { p_bar: 345.73, rs_m3m3: 288.17, bo_m3m3: 1.827, mu_o_cp: 0.449, bg_m3m3: 0.00364, mu_g_cp: 0.0309 },
    { p_bar: 621.54, rs_m3m3: 288.17, bo_m3m3: 1.737, mu_o_cp: 0.631, bg_m3m3: 0.00364, mu_g_cp: 0.0309 },
];

// Exact SWOF/SGOF inputs from OPM's props_spe1case1b.inc.
// OPM adds the final Sg = 0.88 row so SWOF first saturation + SGOF last saturation sums to 1.0.
const SPE1_SCAL_TABLES: ThreePhaseScalTables = {
    swof: [
        { sw: 0.12, krw: 0, krow: 1, pcow: 0 },
        { sw: 0.18, krw: 4.64876033057851e-8, krow: 1, pcow: 0 },
        { sw: 0.24, krw: 1.86e-7, krow: 0.997, pcow: 0 },
        { sw: 0.3, krw: 4.18388429752066e-7, krow: 0.98, pcow: 0 },
        { sw: 0.36, krw: 7.43801652892562e-7, krow: 0.7, pcow: 0 },
        { sw: 0.42, krw: 1.16219008264463e-6, krow: 0.35, pcow: 0 },
        { sw: 0.48, krw: 1.67355371900826e-6, krow: 0.2, pcow: 0 },
        { sw: 0.54, krw: 2.27789256198347e-6, krow: 0.09, pcow: 0 },
        { sw: 0.6, krw: 2.97520661157025e-6, krow: 0.021, pcow: 0 },
        { sw: 0.66, krw: 3.7654958677686e-6, krow: 0.01, pcow: 0 },
        { sw: 0.72, krw: 4.64876033057851e-6, krow: 0.001, pcow: 0 },
        { sw: 0.78, krw: 5.625e-6, krow: 0.0001, pcow: 0 },
        { sw: 0.84, krw: 6.69421487603306e-6, krow: 0, pcow: 0 },
        { sw: 0.91, krw: 8.05914256198347e-6, krow: 0, pcow: 0 },
        { sw: 1.0, krw: 1e-5, krow: 0, pcow: 0 },
    ],
    sgof: [
        { sg: 0, krg: 0, krog: 1, pcog: 0 },
        { sg: 0.001, krg: 0, krog: 1, pcog: 0 },
        { sg: 0.02, krg: 0, krog: 0.997, pcog: 0 },
        { sg: 0.05, krg: 0.005, krog: 0.98, pcog: 0 },
        { sg: 0.12, krg: 0.025, krog: 0.7, pcog: 0 },
        { sg: 0.2, krg: 0.075, krog: 0.35, pcog: 0 },
        { sg: 0.25, krg: 0.125, krog: 0.2, pcog: 0 },
        { sg: 0.3, krg: 0.19, krog: 0.09, pcog: 0 },
        { sg: 0.4, krg: 0.41, krog: 0.021, pcog: 0 },
        { sg: 0.45, krg: 0.6, krog: 0.01, pcog: 0 },
        { sg: 0.5, krg: 0.72, krog: 0.001, pcog: 0 },
        { sg: 0.6, krg: 0.87, krog: 0.0001, pcog: 0 },
        { sg: 0.7, krg: 0.94, krog: 0, pcog: 0 },
        { sg: 0.85, krg: 0.98, krog: 0, pcog: 0 },
        { sg: 0.88, krg: 0.984, krog: 0, pcog: 0 },
    ],
};

// ── Eclipse reference data (Case 1, yearly intervals) ──────────────────────
// Pressure in bar (converted from psia / 14.5038)
// GOR in m³/m³ (converted from Mscf/stb × 178.108)
const ECLIPSE_PRESSURE: PublishedReferenceSeries = {
    panelKey: 'diagnostics',
    label: 'Eclipse — Avg Pressure',
    curveKey: 'published-pressure',
    data: [
        { x:    0, y: 330.9 },
        { x:  365, y: 397.9 },
        { x:  730, y: 441.6 },
        { x: 1095, y: 404.1 },
        { x: 1460, y: 355.5 },
        { x: 1825, y: 323.4 },
        { x: 2190, y: 302.0 },
        { x: 2555, y: 285.8 },
        { x: 2920, y: 273.9 },
        { x: 3285, y: 265.3 },
        { x: 3650, y: 256.9 },
    ],
};

const ECLIPSE_GOR: PublishedReferenceSeries = {
    panelKey: 'gor',
    label: 'Eclipse — GOR',
    curveKey: 'published-gor',
    data: [
        { x:    0, y:  226.2 },
        { x:  365, y:  219.1 },
        { x:  730, y:  393.7 },
        { x: 1095, y: 1328.5 },
        { x: 1460, y: 1638.5 },
        { x: 1825, y: 1866.6 },
        { x: 2190, y: 2091.0 },
        { x: 2555, y: 2392.1 },
        { x: 2920, y: 2837.6 },
        { x: 3285, y: 3318.3 },
        { x: 3650, y: 3824.0 },
    ],
};

// ── Brontosaurus representative summary samples (converted from FIELD units) ──
// Source: examples/spe1/expected_output/expected_summary.csv
// Conversions: FOPR_sm3d = FOPR_stbd × 0.158987
//              WBHP_bar = WBHP_psia / 14.5038
const BRONTOSAURUS_OIL_RATE: PublishedReferenceSeries = {
    panelKey: 'oil_rate',
    label: 'Brontosaurus — Oil Rate',
    curveKey: 'published-oil-rate',
    data: [
        { x:    0.0, y: 3179.75 },
        { x:  365.25, y: 3179.75 },
        { x: 1826.25, y: 3155.9 },
        { x: 3652.5, y: 2421.38 },
    ],
};

const BRONTOSAURUS_PRODUCER_WBHP: PublishedReferenceSeries = {
    panelKey: 'producer_bhp',
    label: 'Brontosaurus — PROD WBHP',
    curveKey: 'published-producer-bhp',
    data: [
        { x:    0.0, y: 292.7 },
        { x:  365.25, y: 268.2 },
        { x: 1826.25, y: 221.7 },
        { x: 3652.5, y: 127.6 },
    ],
};

const BRONTOSAURUS_INJECTOR_WBHP: PublishedReferenceSeries = {
    panelKey: 'injector_bhp',
    label: 'Brontosaurus — INJ WBHP',
    curveKey: 'published-injector-bhp',
    data: [
        { x:    0.0, y: 551.6 },
        { x:  365.25, y: 589.5 },
        { x: 1826.25, y: 608.1 },
        { x: 3652.5, y: 621.5 },
    ],
};

const SPE1_INJECTION_RATE_TARGET: PublishedReferenceSeries = {
    panelKey: 'injection_rate',
    label: 'Deck Target — Gas Injection Rate',
    curveKey: 'published-injection-rate',
    data: [
        { x: 0.0, y: 2_831_680 },
        { x: 3652.5, y: 2_831_680 },
    ],
};

export const spe1_gas_injection: Scenario = {
    key: 'spe1_gas_injection',
    label: 'SPE1 Black-Oil Benchmark',
    description:
        'SPE Comparative Solution Project #1 (Odeh, 1981): 10×10×3 black-oil reservoir with gas injection. ' +
        'Validates PVT coupling, dissolved-gas tracking, and three-phase transport against published Eclipse results. ' +
        'Exact OPM SWOF/SGOF tables are supplied. Surface-rate controls now follow the Case 1 deck intent; remaining mismatch, if any, is a simulator-model gap rather than a benchmark-specific curve fit.',
    analyticalMethodSummary:
        'No analytical solution. Comparison is against published Eclipse simulator results from the OPM test suite (SPE1 Case 1).',
    analyticalMethodReference:
        'Odeh, A.S. (1981) "Comparison of Solutions to a Three-Dimensional Black-Oil Reservoir Simulation Problem", JPT, SPE 9723.',
    chartLayoutKey: 'spe1',
    capabilities: {
        analyticalMethod: 'digitized-reference',
        primaryRateCurve: 'oil-rate',
        showSweepPanel: false,
        hasInjector: true,
        default3DScalar: 'saturation_gas',
        requiresThreePhaseMode: true,
    },
    publishedReferenceSeries: [
        ECLIPSE_PRESSURE,
        ECLIPSE_GOR,
        BRONTOSAURUS_OIL_RATE,
        SPE1_INJECTION_RATE_TARGET,
        BRONTOSAURUS_PRODUCER_WBHP,
        BRONTOSAURUS_INJECTOR_WBHP,
    ],
    params: {
        // ── Grid: 10×10×3, non-uniform dz ──────────────────────────────
        nx: 10, ny: 10, nz: 3,
        cellDx: 304.8,            // 1000 ft
        cellDy: 304.8,            // 1000 ft
        cellDz: 10.16,            // fallback (overridden by per-layer)
        cellDzPerLayer: [6.096, 9.144, 15.24],  // 20, 30, 50 ft

        // ── Rock ────────────────────────────────────────────────────────
        reservoirPorosity: 0.3,
        permMode: 'perLayer',
        uniformPermX: 200, uniformPermY: 200, uniformPermZ: 200,
        layerPermsX: [500, 50, 200],
        layerPermsY: [500, 50, 200],
        layerPermsZ: [500, 50, 200],    // kz = kx (SPE1 ambiguity — OPM default)
        rock_compressibility: 4.35e-5,   // 3.0e-6 psi⁻¹ → bar⁻¹

        // ── Fluids ──────────────────────────────────────────────────────
        mu_w: 0.318,
        mu_o: 0.51,      // at initial conditions (from PVT table at ~330 bar)
        mu_g: 0.027,     // at initial conditions
        c_o: 2.06e-4,    // fallback undersaturated c_o for PVTO branches without explicit continuation rows
        c_w: 4.67e-5,    // 3.22e-6 psi⁻¹ → bar⁻¹
        c_g: 1e-4,       // gas compressibility (approximate)
        rho_w: 1033,
        rho_o: 860,
        rho_g: 0.854,    // surface gas density
        depth_reference: 2560,   // 8400 ft datum depth in meters
        volume_expansion_o: 1.695,  // Bo at bubble point
        volume_expansion_w: 1.038,  // Bw at reference

        // ── PVT ─────────────────────────────────────────────────────────
        pvtMode: 'black-oil',
        pvtTable: SPE1_PVT_TABLE,

        // ── SCAL ────────────────────────────────────────────────────────
        // Exact SWOF/SGOF tables are supplied below; Corey endpoints remain as fallback metadata.
        s_wc: 0.12, s_or: 0.12,
        n_w: 2.0, n_o: 2.5,
        k_rw_max: 0.00001, k_ro_max: 1.0,
        s_gc: 0.04, s_gr: 0.04, s_org: 0.18,
        n_g: 1.5,
        k_rg_max: 0.984,
        scalTables: SPE1_SCAL_TABLES,

        // ── Initial conditions ──────────────────────────────────────────
        initialPressure: 331,     // 4800 psia — undersaturated
        initialSaturation: 0.12,  // Swi = 0.12 (connate, immobile)
        initialGasSaturation: 0,  // no free gas initially

        // ── Wells ───────────────────────────────────────────────────────
        injectorEnabled: true,
        injectedFluid: 'gas',
        threePhaseModeEnabled: true,
        gasRedissolutionEnabled: false,
        // OPM RSVD table: constant Rs = 1.270 Mscf/stb = 226.197 Sm³/Sm³
        // Oil starts undersaturated (bubble point ≈ 277 bar < initial pressure 331 bar)
        initialRs: 226.197,
        // Producer at (9,9), layer 2 only (bottom layer)
        producerI: 9, producerJ: 9,
        producerKLayers: [2],
        // Injector at (0,0), layer 0 only (top layer)
        injectorI: 0, injectorJ: 0,
        injectorKLayers: [0],
        well_radius: 0.0762,      // 0.25 ft
        well_skin: 0,
        // Rate control (approximate reservoir-condition conversions of the OPM/Eclipse deck)
        // Case 1 source deck uses:
        // - producer ORAT 20,000 STB/d with 1000 psia BHP floor
        // - injector GAS RATE 100 MMscf/d with 9014 psia BHP ceiling
        // Surface-rate targets are the authoritative deck-matching controls.
        // The legacy reservoir-rate fields below are retained only as fallback/context;
        // WASM rate control uses the pressure-dependent surface-rate conversion path.
        producerControlMode: 'rate',
        injectorControlMode: 'rate',
        targetProducerRate: 5400,  // legacy fallback only; dynamic reservoir withdrawal is solved from the surface target
        targetInjectorRate: 12000, // legacy fallback only; dynamic reservoir injection is solved from the surface target
        targetProducerSurfaceRate: 3179.74,   // 20,000 STB/d → Sm3/d
        targetInjectorSurfaceRate: 2_831_680, // 100 MMscf/d → Sm3/d
        producerBhp: 69,           // 1000 psia min BHP
        injectorBhp: 621,          // 9014 psia max BHP
        bhpMin: 69,
        bhpMax: 621,

        // ── Capillary / gravity ─────────────────────────────────────────
        capillaryEnabled: false,
        capillaryPEntry: 0, capillaryLambda: 2,
        pcogEnabled: false, pcogPEntry: 0, pcogLambda: 2,
        gravityEnabled: true,      // layered system with density differences

        // ── Numerics ────────────────────────────────────────────────────
        fimEnabled: false,
        delta_t_days: 30,
        steps: 120,               // 4000 days coverage
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 20,
        max_well_rate_change_fraction: 0.2,
    },
    sensitivities: [
        {
            key: 'grid',
            label: 'Grid Resolution',
            description: 'Grid convergence study — refine or coarsen the 10×10×3 base grid while maintaining the same physical domain.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'grid_5',
                    label: '5×5×3  (coarse)',
                    description: 'Coarse grid — larger numerical diffusion, faster gas breakthrough.',
                    paramPatch: {
                        nx: 5, ny: 5, cellDx: 609.6, cellDy: 609.6,
                        producerI: 4, producerJ: 4,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_10',
                    label: '10×10×3  (base)',
                    description: 'Base SPE1 grid — 300 cells.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_20',
                    label: '20×20×3  (fine)',
                    description: 'Refined grid — 1200 cells, sharper gas front, with tighter timestep and control-change limits to keep the fine-grid case stable.',
                    paramPatch: {
                        nx: 20, ny: 20, cellDx: 152.4, cellDy: 152.4,
                        producerI: 19, producerJ: 19,
                        delta_t_days: 2.5,
                        steps: 1600,
                        max_sat_change_per_step: 0.03,
                        max_pressure_change_per_step: 30,
                        max_well_rate_change_fraction: 0.35,
                    },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'kz_ratio',
            label: 'Vertical Permeability (k_v/k_h)',
            description: 'SPE1 does not specify kz. Vary the vertical-to-horizontal permeability ratio to explore its effect on gas override and breakthrough timing.',
            variants: [
                {
                    key: 'kz_01',
                    label: 'k_v/k_h = 0.1',
                    description: 'Low vertical permeability — restricts gravity override, delays gas breakthrough in the producer layer.',
                    paramPatch: {
                        layerPermsZ: [50, 5, 20],
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'kz_10',
                    label: 'k_v/k_h = 1.0  (base)',
                    description: 'Isotropic — OPM default assumption.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'kz_03',
                    label: 'k_v/k_h = 0.3',
                    description: 'Moderate vertical restriction — common in layered clastics.',
                    paramPatch: {
                        layerPermsZ: [150, 15, 60],
                    },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'delta_t',
            label: 'Time Step',
            description: 'Time step convergence study — refine or coarsen the time step while maintaining the same physical domain.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'delta_t_5',
                    label: 'Δt = 5 days  (coarse)',
                    description: 'Coarse time step — larger numerical diffusion, faster gas breakthrough.',
                    paramPatch: {
                        delta_t_days: 5,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'delta_t_2_5',
                    label: 'Δt = 2.5 days  (base)',
                    description: 'Base time step — 2.5 days.',
                    paramPatch: {
                        delta_t_days: 2.5,
                        steps: 1600,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'delta_t_1_25',
                    label: 'Δt = 1.25 days  (fine)',
                    description: 'Refined time step — 1.25 days, sharper gas front, with tighter timestep and control-change limits to keep the fine-grid case stable.',
                    paramPatch: {
                        delta_t_days: 1.25,
                        steps: 3200,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'delta_t_0_25',
                    label: 'Δt = 0.25 days  (fine)',
                    description: 'Refined time step — 0.25 days, sharper gas front, with tighter timestep and control-change limits to keep the fine-grid case stable.',
                    paramPatch: {
                        delta_t_days: 0.25,
                        steps: 16000,
                    },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
