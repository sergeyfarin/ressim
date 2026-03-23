import type { Scenario, PublishedReferenceSeries } from '../scenarios';

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
 * - Corey relperm approximation instead of tabular SGOF/SWOF
 * - Reservoir-condition rate targets (approximate field-to-metric conversion)
 * - DRSDT behavior depends on simulator's Rs update logic
 *
 * Published Eclipse reference data (Case 1, OPM test suite) is overlaid on
 * the pressure and GOR panels for visual comparison.
 */

// ── PVT table (SPE1 saturated oil, converted to metric) ────────────────────
// Conversions: P_bar = P_psia / 14.5038
//              Rs_m3m3 = Rs_Mscf_stb × 178.108
//              Bo_m3m3 = Bo_rb_stb (dimensionless, same numeric value)
//              Bg_m3m3 = Bg_rb_Mscf × 0.158987 / 28.3168
//              μ_o, μ_g unchanged (cP)
const SPE1_PVT_TABLE = [
    { p_bar:   1.01, rs_m3m3:   0.18, bo_m3m3: 1.062, mu_o_cp: 1.040, bg_m3m3: 0.9361, mu_g_cp: 0.0080 },
    { p_bar:  18.25, rs_m3m3:  16.12, bo_m3m3: 1.150, mu_o_cp: 0.975, bg_m3m3: 0.0679, mu_g_cp: 0.0096 },
    { p_bar:  35.49, rs_m3m3:  32.06, bo_m3m3: 1.207, mu_o_cp: 0.910, bg_m3m3: 0.0352, mu_g_cp: 0.0112 },
    { p_bar:  69.96, rs_m3m3:  66.08, bo_m3m3: 1.295, mu_o_cp: 0.830, bg_m3m3: 0.0179, mu_g_cp: 0.0140 },
    { p_bar: 138.91, rs_m3m3: 113.29, bo_m3m3: 1.435, mu_o_cp: 0.695, bg_m3m3: 0.00906, mu_g_cp: 0.0189 },
    { p_bar: 173.38, rs_m3m3: 138.03, bo_m3m3: 1.500, mu_o_cp: 0.641, bg_m3m3: 0.00727, mu_g_cp: 0.0208 },
    { p_bar: 207.85, rs_m3m3: 165.64, bo_m3m3: 1.565, mu_o_cp: 0.594, bg_m3m3: 0.00607, mu_g_cp: 0.0228 },
    { p_bar: 276.79, rs_m3m3: 226.20, bo_m3m3: 1.695, mu_o_cp: 0.510, bg_m3m3: 0.00455, mu_g_cp: 0.0268 },
    { p_bar: 345.73, rs_m3m3: 288.17, bo_m3m3: 1.827, mu_o_cp: 0.449, bg_m3m3: 0.00364, mu_g_cp: 0.0309 },
];

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
    panelKey: 'diagnostics',
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
    yAxisID: 'y1',
};

export const spe1_gas_injection: Scenario = {
    key: 'spe1_gas_injection',
    label: 'SPE1 Black-Oil Benchmark',
    description:
        'SPE Comparative Solution Project #1 (Odeh, 1981): 10×10×3 black-oil reservoir with gas injection. ' +
        'Validates PVT coupling, dissolved-gas tracking, and three-phase transport against published Eclipse results. ' +
        'Corey relperm approximation — tabular SCAL not yet supported. Reservoir-condition rate targets are approximate conversions from field units.',
    analyticalMethodSummary:
        'No analytical solution. Comparison is against published Eclipse simulator results from the OPM test suite (SPE1 Case 1).',
    analyticalMethodReference:
        'Odeh, A.S. (1981) "Comparison of Solutions to a Three-Dimensional Black-Oil Reservoir Simulation Problem", JPT, SPE 9723.',
    chartLayoutKey: 'spe1',
    capabilities: {
        analyticalMethod: 'none',
        primaryRateCurve: 'oil-rate',
        showSweepPanel: false,
        hasInjector: true,
        default3DScalar: 'saturation_gas',
        requiresThreePhaseMode: true,
    },
    publishedReferenceSeries: [ECLIPSE_PRESSURE, ECLIPSE_GOR],
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
        c_o: 2.06e-4,    // undersaturated oil compressibility (from SPE1 undersaturated data)
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

        // ── SCAL (Corey approximation of SPE1 tabular data) ────────────
        // Water is immobile in SPE1 (max krw ≈ 1e-5)
        s_wc: 0.12, s_or: 0.12,
        n_w: 2.0, n_o: 2.5,
        k_rw_max: 0.0, k_ro_max: 1.0,
        // Gas-oil Corey fit
        s_gc: 0.04, s_gr: 0.04, s_org: 0.18,
        n_g: 1.6,
        k_rg_max: 0.984,

        // ── Initial conditions ──────────────────────────────────────────
        initialPressure: 331,     // 4800 psia — undersaturated
        initialSaturation: 0.12,  // Swi = 0.12 (connate, immobile)
        initialGasSaturation: 0,  // no free gas initially

        // ── Wells ───────────────────────────────────────────────────────
        injectorEnabled: true,
        injectedFluid: 'gas',
        threePhaseModeEnabled: true,
        // Producer at (9,9), layer 2 only (bottom layer)
        producerI: 9, producerJ: 9,
        producerKLayers: [2],
        // Injector at (0,0), layer 0 only (top layer)
        injectorI: 0, injectorJ: 0,
        injectorKLayers: [0],
        well_radius: 0.0762,      // 0.25 ft
        well_skin: 0,
        // Rate control (approximate reservoir-condition conversions)
        producerControlMode: 'rate',
        injectorControlMode: 'rate',
        targetProducerRate: 5400,  // ≈ 20,000 STB/d × Bo × 0.159
        targetInjectorRate: 12000, // ≈ 100 MMscf/d at reservoir Bg
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
        delta_t_days: 5,
        steps: 800,               // 4000 days coverage
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 50,
        max_well_rate_change_fraction: 0.5,
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
                    description: 'Refined grid — 1200 cells, sharper gas front.',
                    paramPatch: {
                        nx: 20, ny: 20, cellDx: 152.4, cellDy: 152.4,
                        producerI: 19, producerJ: 19,
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
    ],
};
