import type { Scenario } from '../scenarios';
import { generateBlackOilTable, type SaturatedRsCorrelation } from '../../physics/pvt';

/**
 * "Two fluid models, one calibration point" — PVT representation risk.
 *
 * A single flash test pins one point of a black-oil fluid: the bubble point
 * P_b and the Rs_b and Bo_b there. Everything else in the table is a
 * correlation's extrapolation from that point, in two directions, and this
 * case has one sensitivity dimension per direction. Every table below is built
 * by generateBlackOilTable() from the same inputs (API, gas gravity,
 * temperature, P_b), so all of them share that calibration point exactly.
 *
 * ABOVE THE BUBBLE POINT (`pvt_model`): the undersaturated oil
 * compressibility, 1.0e-4/bar "correlation" vs 2.5e-4/bar "lab report". It is
 * not Bo that separates the runs: over 130 bar of undersaturation Bo differs
 * by ~2%. It is *storage*. The well withdraws a fixed volume per day, and an
 * undersaturated reservoir can only supply it by expanding, so
 *
 *     dP/dt = -q_res / (V_p * c_t),   c_t = c_o*S_o + c_w*S_w + c_rock
 *
 * c_t is 1.40e-4/bar on the correlation table and 2.75e-4/bar on the lab
 * report, a factor 1.96, and the runs reach the bubble point at measured
 * 46.5 d and 91.5 d, a factor 1.97. Unmeasured undersaturated compressibility
 * does not bend the fluid; it rescales the clock.
 *
 * Below the bubble point the two tables are the same fluid, so they
 * reconverge where it matters, at matched pressure: producing GOR agrees
 * within 1% at every pressure from 130 to 85 bar, and oil recovery stays a
 * constant ~1.7 points apart, the extra oil the lab-report fluid expanded out
 * before reaching P_b. In time the lab-report run is simply ~45 d behind: the
 * average-pressure gap peaks at 64 bar as the correlation run crosses P_b,
 * narrows to 6.7 bar at t = 134 d, and then widens again (25 bar at 360 d)
 * because the same 45 d lag costs more pressure as depletion accelerates.
 *
 * The undersaturated leg identifies only the banked expansion
 * c_t * (p_i - P_b). Starting the lab-report table at 216 bar instead of 280
 * reproduces the correlation run (P_b at 47.25 d vs 46.5 d, recovery within
 * 0.02 points at every matched pressure), which is why the degree of
 * undersaturation is not offered as a ladder of its own.
 *
 * BELOW THE BUBBLE POINT (`saturated_pvt`): how the oil gives up its gas. A
 * flash test does not measure the differential-liberation curve, and the
 * published correlations disagree about it. Petrosky & Farshad (1993),
 * Standing (1947) and Al-Marhoun (1988), each normalised through this case's
 * own (P_b, Rs_b), leave 0.735, 0.688 and 0.648 of Rs_b dissolved at 110 bar.
 * This is the mirror image of `pvt_model`: the three runs are identical until
 * the bubble point (all cross it at 46.5 d) and fan out after it. Measured at
 * matched average pressure, Petrosky-Farshad / Standing / Al-Marhoun:
 *
 *     130 bar: GOR 125 / 150 / 179 m3/m3, oil recovery 6.91 / 7.45 / 7.94 %
 *     110 bar: GOR 318 / 400 / 469 m3/m3, oil recovery 10.70 / 11.20 / 11.50 %
 *      90 bar: GOR 530 / 688 / 801 m3/m3, oil recovery 13.13 / 13.49 / 13.63 %
 *
 * The GOR spread holds for the whole run. The recovery spread narrows to 0.1
 * points by 60 bar (15.85 / 15.92 / 15.82 %): the fluid that releases its gas
 * early gets more oil out per bar at first and has less gas left to drive it
 * later. Recovery against *time* is the same on all three rungs by
 * construction, because the producer holds a surface-oil rate.
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
 * balance to within 1.4% of 1 on every rung of both ladders across the whole
 * run, which is why the `gas` layout's material-balance panels are shown here
 * rather than hidden.
 *
 * WHY c_rock IS 5e-5 /bar. Hall (1953) gives 5.2e-5/bar for a consolidated
 * sandstone at this case's 20% porosity. The case shipped 1e-6/bar until #26,
 * 50x too stiff; that left c_o alone in c_t and inflated the headline ratio
 * from ~2x to 2.5x, overstating the very risk the case is about.
 *
 * References: Standing (1947); Petrosky & Farshad (1993), SPE 26644;
 * Al-Marhoun (1988), JPT 40(5); Hall (1953), Trans. AIME 198; McCain, "The
 * Properties of Petroleum Fluids" on undersaturated-oil PVT uncertainty
 * (undersaturated black oils run 5-30e-6 psi^-1, i.e. 0.7-4e-4 /bar — both
 * `pvt_model` rungs sit inside that band, which is what makes them "equally
 * plausible").
 *
 * No analytical overlay is wired: the run spans the bubble point, and the
 * Dietz depletion model used by dep_decline/dep_pss/dep_arps is an oil-only
 * PSS model that does not represent gas liberation. The undersaturated leg
 * *is* analytically predictable (the dP/dt relation above, which the measured
 * crossings satisfy), and the on-chart material-balance ratio is a real
 * quantitative self-check.
 */

// Correlation inputs shared by every table — this is the ONE calibration
// point (API, gas gravity, temperature, bubble point) all variants agree on.
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

function pvtTable(undersaturatedCompressibilityPerBar: number, saturatedRs: SaturatedRsCorrelation) {
    return generateBlackOilTable(
        API_GRAVITY, GAS_SPECIFIC_GRAVITY, RESERVOIR_TEMP_C,
        BUBBLE_POINT_BAR, PVT_TABLE_PMAX_BAR, PVT_TABLE_POINTS,
        undersaturatedCompressibilityPerBar, saturatedRs,
    );
}

const PVT_TABLE_CORRELATION = pvtTable(C_O_CORRELATION_PER_BAR, 'standing');
const PVT_TABLE_LAB_REPORT = pvtTable(C_O_LAB_REPORT_PER_BAR, 'standing');
const PVT_TABLE_PETROSKY_FARSHAD = pvtTable(C_O_CORRELATION_PER_BAR, 'petrosky-farshad');
const PVT_TABLE_AL_MARHOUN = pvtTable(C_O_CORRELATION_PER_BAR, 'al-marhoun');

export const dep_pvt: Scenario = {
    key: 'dep_pvt',
    label: 'PVT Model Risk — One Calibration Point',
    catalog: {
        group: 'material-balance-drive',
        role: 'interpretation',
        caseMode: '3p',
        parameterSummary: 'Constant-rate black-oil blowdown · every PVT table shares one flash-test point · above it: 2x the time to bubble point · below it: ±20% producing GOR',
    },
    description: 'Constant-rate black-oil blowdown from 130 bar above the bubble point to 90 bar below it. One flash test pins the bubble point; each ladder varies what that test cannot pin, on one side of it.',
    analyticalMethodSummary: 'No closed-form overlay is drawn: the run crosses the bubble point, and the Dietz PSS depletion model used elsewhere in this catalog is oil-only and does not represent gas liberation. OPM Flow runs every rung of both ladders, shown as the OPM Flow reference curves. Two further quantitative checks stand beside them. The undersaturated leg must obey dP/dt = -q_res/(V_p·c_t), and does: the 1.96x ratio in c_t between the two compressibility rungs produces a measured 1.97x ratio in time-to-bubble-point. And the Havlena-Odeh material-balance ratio on the chart is a genuine self-check: it holds within 1.4% of 1.0 for the whole run on every rung.',
    analyticalMethodReference: 'Standing (1947); Petrosky & Farshad (1993), SPE 26644; Al-Marhoun (1988), JPT 40(5); Hall (1953), Trans. AIME 198 (pore compressibility); McCain, "The Properties of Petroleum Fluids" (undersaturated oil compressibility 5-30e-6 psi^-1); Havlena & Odeh (1963), The Material Balance as an Equation of a Straight Line.',
    // OPM Flow on every rung: decks generated from this scenario's own tables
    // (opm/reference-decks/small-direct/dep-pvt-*). Each run is shown only
    // under the ladder it is a rung of; the base case is a rung of both.
    referenceSources: [
        {
            kind: 'opm-flow',
            artifactKeys: ['dep_pvt_correlation'],
            // No `artifactVariantLabels`: each deck already names its rung
            // ("… (c_o = 2.5e-4)") in every curve label, as dep_gas_pz's do.
        },
        {
            kind: 'opm-flow',
            artifactKeys: ['dep_pvt_lab_report'],
            dimensionKeys: ['pvt_model'],
        },
        {
            kind: 'opm-flow',
            artifactKeys: ['dep_pvt_petrosky_farshad', 'dep_pvt_al_marhoun'],
            dimensionKeys: ['saturated_pvt'],
        },
    ],
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
        // Hall (1953) at 20% porosity: 5.2e-5 /bar. See the header.
        rock_compressibility: 5e-5,
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
        // Floor only — never reached: the lowest cell pressure at the end of
        // the run is ~49 bar, on the Al-Marhoun rung.
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
        // Numerics: 480 x 0.75 d = 360 d. The slower compressibility rung
        // crosses the bubble point at 91.5 d and ends 65 bar below it; the base
        // case ends at ~60 bar, 90 bar below it. Past ~440 d the Al-Marhoun
        // rung's near-well cells approach zero pressure.
        fimEnabled: true,
        delta_t_days: 0.75,
        steps: 480,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    sensitivities: [
        {
            key: 'pvt_model',
            label: 'Above the Bubble Point (Undersaturated Compressibility)',
            description: 'Same bubble point, Rs and Bo; only the undersaturated c_o differs, which one flash test cannot measure. It rescales the clock to the bubble point; below it both are one fluid. Only c_t·(p_i − P_b) is identifiable here.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'pvt_correlation',
                    label: 'Correlation  (c_o = 1.0e-4/bar above Pb)',
                    description: 'Correlation-derived undersaturated compressibility — base case. c_t = 1.40e-4/bar; bubble point reached at t = 46.5 d.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'pvt_lab_report',
                    label: 'Lab Report  (c_o = 2.5e-4/bar above Pb)',
                    description: 'A different, equally plausible undersaturated compressibility — 2.5x the oil compressibility and 2x the total storage above the bubble point, identical Rs/Bo at and below it. c_t = 2.75e-4/bar; bubble point reached at t = 91.5 d.',
                    // FIM extrapolates a fixed-Rs undersaturated branch with
                    // scalar c_o; keep it consistent with the generated table.
                    paramPatch: { pvtTable: PVT_TABLE_LAB_REPORT, c_o: C_O_LAB_REPORT_PER_BAR },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'saturated_pvt',
            label: 'Below the Bubble Point (Solution-Gas Liberation)',
            description: 'Same bubble point, Rs, Bo and undersaturated branch; only the published Rs(p) curve below the bubble point differs. Identical until it, then GOR fans out ±20% at matched pressure. Recovery vs time is fixed by the oil-rate control.',
            analyticalOverlayMode: 'shared',
            chartLayoutPatchOverride: {
                chart: {
                    panelOrder: ['gor', 'diagnostics', 'cumulative_gas', 'mbe_ooip', 'recovery', 'rates', 'cumulative', 'drive_indices'],
                    panels: {
                        gor: { expanded: true },
                        cumulative_gas: { expanded: true },
                        mbe_ooip: { expanded: false },
                    },
                },
            },
            variants: [
                {
                    key: 'sat_petrosky_farshad',
                    label: 'Petrosky–Farshad (1993)  (holds gas longest)',
                    description: 'Gulf of Mexico oils. Keeps 0.735 of Rs_b dissolved at 110 bar: the lowest GOR below the bubble point. Its −1391 psia intercept leaves ~0.2 of Rs_b dissolved at 1 bar, a tail this run never reaches.',
                    paramPatch: { pvtTable: PVT_TABLE_PETROSKY_FARSHAD },
                    affectsAnalytical: false,
                },
                {
                    key: 'sat_standing',
                    label: 'Standing (1947)  (base)',
                    description: 'California oils — base case. Keeps 0.688 of Rs_b dissolved at 110 bar.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'sat_al_marhoun',
                    label: 'Al-Marhoun (1988)  (releases gas soonest)',
                    description: 'Middle East oils. Keeps 0.648 of Rs_b dissolved at 110 bar: the highest GOR below the bubble point.',
                    paramPatch: { pvtTable: PVT_TABLE_AL_MARHOUN },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
