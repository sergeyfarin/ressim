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
 * undersaturated oil compressibility (1.0e-4/bar "correlation" vs
 * 2.5e-4/bar "lab report") — a value a single flash test cannot pin down,
 * since it governs behavior only *above* the bubble point.
 *
 * WHAT THE DIVERGENCE ACTUALLY IS. It is not Bo. Over the 130 bar of
 * undersaturation here, Bo = Bo_pb * exp(-c_o * dP) differs between the two
 * tables by only ~2%; that really is a second-order effect. What separates
 * the curves is *storage*: the well withdraws a fixed volume per day, and an
 * undersaturated reservoir can only supply it by expanding, so
 *
 *     dP/dt = -q_res / (V_p * c_t),   c_t = c_o*S_o + c_w*S_w + c_rock
 *
 * c_t is 9.1e-5/bar for the correlation table and 2.26e-4/bar for the lab
 * report — a factor 2.48 — and the depletion rate is inversely proportional
 * to it. The two fluid models therefore reach the bubble point at *measured*
 * 36 d and 88 d, a factor 2.4. Unmeasured undersaturated compressibility does
 * not bend the fluid; it rescales the clock.
 *
 * WHY IT RECONVERGES. Below the bubble point both variants are the same
 * fluid — identical Rs(P), Bo(P) — and liberated gas dominates c_t, so the
 * two trajectories stop diverging and close back up. Measured average-pressure
 * gap over the shipped 225 d run: 15 bar at t = 7 d, peaking at 77.5 bar at
 * t = 35 d (just as the fast variant crosses Pb), then falling back to 14 bar
 * at t = 169 d and 17 bar at the end. The residual is the offset banked above
 * the bubble point, not continuing disagreement.
 *
 * WHY THE PRODUCER IS RATE-CONTROLLED, AND WHY k IS 200 mD. Both exist to
 * make one number — the volumetric average pressure — actually describe the
 * reservoir, because that is this case's headline chart and its
 * material-balance self-check. An earlier design produced 0.5 mD against a
 * 30 bar BHP; the near-well cells sat far below the bubble point liberating
 * gas while the volumetric average was still undersaturated and saw only c_o,
 * and the Havlena-Odeh balance read N_mbe/N_volumetric = 2.5 to 7.8 instead of
 * 1 — the tank under-counted the reservoir's energy up to eight-fold. A
 * near-uniform-pressure reservoir drawn down at a constant rate closes that
 * balance to 0.9998-1.0166 (correlation) and 0.9995-1.0069 (lab report)
 * across the whole run, which is why the `gas` layout's material-balance
 * panels are shown here rather than hidden.
 *
 * References: Standing (1947) correlations; McCain, "The Properties of
 * Petroleum Fluids" on undersaturated-oil PVT uncertainty (undersaturated
 * black oils run 5-30e-6 psi^-1, i.e. 0.7-4e-4 /bar — both rungs here sit
 * inside that band, which is what makes them "equally plausible"); the
 * general "representation risk" framing complements the material-balance
 * non-uniqueness point to the PVT-table axis.
 *
 * No analytical overlay is wired: the run spans the bubble point, and the
 * Dietz depletion model used by dep_decline/dep_pss/dep_arps is an oil-only
 * PSS model that does not represent gas liberation. The undersaturated leg
 * *is* analytically predictable (the dP/dt relation above, which the measured
 * 36 d / 88 d crossings satisfy), and the on-chart material-balance ratio is
 * a real quantitative self-check.
 */

// Correlation inputs shared by both tables — this is the ONE calibration
// point (API, gas gravity, temperature, bubble point) both variants agree on.
const API_GRAVITY = 35;
const GAS_SPECIFIC_GRAVITY = 0.75;
const RESERVOIR_TEMP_C = 80;
const BUBBLE_POINT_BAR = 150;
const PVT_TABLE_PMAX_BAR = 300;
const PVT_TABLE_POINTS = 20;

/**
 * Vasquez-Beggs at this case's own inputs (35 API, 0.75 gas gravity, 80 C,
 * Rs at Pb, ~4000 psia) gives ~1e-5 psi^-1 = ~1.4e-4 /bar. Both rungs are
 * within a factor ~1.8 of that and inside McCain's 0.7-4e-4 /bar band for
 * undersaturated black oils, which is the point: neither is the "wrong"
 * answer a flash test would have caught.
 */
const C_O_CORRELATION_PER_BAR = 1.0e-4;
const C_O_LAB_REPORT_PER_BAR = 2.5e-4;

const PVT_TABLE_CORRELATION = generateBlackOilTable(
    API_GRAVITY, GAS_SPECIFIC_GRAVITY, RESERVOIR_TEMP_C,
    BUBBLE_POINT_BAR, PVT_TABLE_PMAX_BAR, PVT_TABLE_POINTS,
    C_O_CORRELATION_PER_BAR,
);
const PVT_TABLE_LAB_REPORT = generateBlackOilTable(
    API_GRAVITY, GAS_SPECIFIC_GRAVITY, RESERVOIR_TEMP_C,
    BUBBLE_POINT_BAR, PVT_TABLE_PMAX_BAR, PVT_TABLE_POINTS,
    C_O_LAB_REPORT_PER_BAR,
);

export const dep_pvt: Scenario = {
    key: 'dep_pvt',
    label: 'PVT Model Risk — One Calibration Point',
    catalog: {
        group: 'material-balance-drive',
        role: 'interpretation',
        caseMode: '3p',
        parameterSummary: 'Constant-rate black-oil blowdown · two PVT tables share one calibration point · time to bubble point differs 2.4x',
    },
    description: 'Constant-rate blowdown of a black-oil reservoir starting 130 bar above the bubble point.',
    analyticalMethodSummary: 'No closed-form overlay is drawn: the run crosses the bubble point, and the Dietz PSS depletion model used elsewhere in this catalog is oil-only and does not represent gas liberation. Two quantitative checks stand in its place. The undersaturated leg must obey dP/dt = -q_res/(V_p·c_t), and does — the 2.48x ratio in c_t between the two tables produces a measured 2.4x ratio in time-to-bubble-point. And the Havlena-Odeh material-balance ratio on the chart is a genuine self-check: it holds within 2% of 1.0 for the whole run.',
    analyticalMethodReference: 'Standing (1947); McCain, "The Properties of Petroleum Fluids" (undersaturated oil compressibility 5-30e-6 psi^-1); Havlena & Odeh (1963), The Material Balance as an Equation of a Straight Line.',
    chartLayoutKey: 'gas',
    /**
     * Pressure leads, and the balance that grades it follows immediately.
     *
     * This is the whole shape of the case: the exhibit is a depletion *rate*,
     * so Avg Pressure is the headline rather than the fifth panel the `gas`
     * layout gives a solution-gas-drive case, and `mbe_ooip` sits next to it
     * because it is the evidence that one pressure describes this reservoir.
     * The predecessor patched this same field to *hide* those two panels; see
     * the header for what changed and what it was measured at.
     */
    chartLayoutPatch: {
        chart: {
            panelOrder: ['diagnostics', 'mbe_ooip', 'gor', 'recovery', 'rates', 'cumulative', 'drive_indices'],
            panels: {
                mbe_ooip: { expanded: true },
            },
        },
    },
    defaultSensitivityDimensionKey: 'pvt_model',
    capabilities: {
        analyticalMethod: 'none',
        hasInjector: false,
        default3DScalar: 'saturation_gas',
        requiresThreePhaseMode: true,
    },
    solverPolicy: {
        defaultSolver: 'fim',
        rationale: 'FIM is required here because the exhibit depends on coupled black-oil PVT, gas liberation, phase appearance, and pressure-dependent well behavior.',
    },
    params: {
        // Fluid
        mu_w: 0.5,
        mu_o: 1.0,
        mu_g: 0.02,
        c_o: C_O_CORRELATION_PER_BAR,
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
        // Grid: single-cell-column 1D slab, 48 x 1 x 1, 480 m x 10 m x 10 m
        // (pore volume 9,600 m3). 200 mD is high enough that the pressure
        // gradient across the slab stays small next to the 130 bar of
        // undersaturation, so the volumetric average is a fair description of
        // the whole reservoir and the tank balance closes — see the header.
        nx: 48,
        ny: 1,
        nz: 1,
        cellDx: 10,
        cellDy: 10,
        cellDz: 10,
        permMode: 'uniform',
        uniformPermX: 200,
        uniformPermY: 200,
        uniformPermZ: 20,
        // Initial conditions: undersaturated start, 130 bar above the bubble point
        initialPressure: 280,
        initialSaturation: 0.1,
        initialGasSaturation: 0,
        // PVT
        pvtMode: 'black-oil',
        pvtTable: PVT_TABLE_CORRELATION,
        threePhaseModeEnabled: true,
        gasRedissolutionEnabled: true,
        // Wells: one rate-controlled producer, no injector. Constant withdrawal
        // is what turns the unmeasured compressibility into a depletion *rate*
        // and keeps the reservoir near-uniform in pressure; a BHP-controlled
        // well instead front-loads the drawdown and breaks the tank
        // description this case's headline chart depends on.
        injectorEnabled: false,
        injectorControlMode: 'pressure',
        producerControlMode: 'rate',
        injectorBhp: 500,
        // Floor only — never reached: the minimum cell pressure at the end of
        // the run is ~78 bar.
        producerBhp: 30,
        targetInjectorRate: 0,
        targetProducerRate: 3,
        targetProducerSurfaceRate: 3,
        injectorI: 0,
        injectorJ: 0,
        producerI: 47,
        producerJ: 0,
        well_radius: 0.1,
        well_skin: 0,
        // Numerics: 300 x 0.75 d = 225 d, long enough to carry the slower
        // variant across the bubble point at 88 d and past the 169 d minimum
        // of the average-pressure gap.
        fimEnabled: true,
        delta_t_days: 0.75,
        steps: 300,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    sensitivities: [
        {
            key: 'pvt_model',
            label: 'PVT Table (Undersaturated Compressibility)',
            description: 'Both tables share the identical bubble-point pressure, Rs, and Bo — the one calibrated point — and diverge only above it, where undersaturated compressibility cannot be measured by a single flash test.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'pvt_correlation',
                    label: 'Correlation  (c_o = 1.0e-4/bar above Pb)',
                    description: 'Correlation-derived undersaturated compressibility — base case. c_t = 9.1e-5/bar; bubble point reached at t = 36 d.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'pvt_lab_report',
                    label: 'Lab Report  (c_o = 2.5e-4/bar above Pb)',
                    description: 'A different, equally plausible undersaturated compressibility — 2.5x more storage above the bubble point, identical Rs/Bo at and below it. c_t = 2.26e-4/bar; bubble point reached at t = 88 d.',
                    // FIM extrapolates a fixed-Rs undersaturated branch with
                    // scalar c_o; keep it consistent with the generated table.
                    paramPatch: { pvtTable: PVT_TABLE_LAB_REPORT, c_o: C_O_LAB_REPORT_PER_BAR },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
