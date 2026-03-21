/**
 * Havlena-Odeh (1963) Material Balance Equation.
 *
 * The general volumetric MBE in reservoir-engineering form:
 *
 *     F = N × (E_o + m × E_g + E_fw)
 *
 * where:
 *   F   = cumulative withdrawal at reservoir conditions
 *   N   = original oil in place (OOIP) [surface m³]
 *   E_o = oil expansion + dissolved-gas liberation
 *   E_g = gas-cap expansion
 *   E_fw = connate-water expansion + pore compaction
 *   m   = ratio of initial gas-cap to oil-zone reservoir volume
 *
 * References:
 *   Havlena, D. & Odeh, A.S. (1963) "The Material Balance as an
 *   Equation of a Straight Line", JPT 15(8).
 *   Schilthuis, R.J. (1936) "Active Oil and Reservoir Energy",
 *   Trans. AIME 118.
 *   Dake, L.P. (1978) "Fundamentals of Reservoir Engineering", Ch. 3.
 */

import {
    standingRs,
    standingBo,
    hallYarboroughZFactor,
    suttonPseudoCriticals,
    cToF,
    cToR,
    PSI_PER_BAR,
    SCF_PER_BBL_TO_M3_PER_M3,
} from '../physics/pvt';

// ─── Types ───────────────────────────────────────────────────────────────────

export type MaterialBalanceParams = {
    // Initial reservoir state
    initialPressure: number;        // P_i [bar]
    initialWaterSaturation: number; // S_wi [-]
    initialGasSaturation: number;   // S_gi [-] (> 0 if gas cap present)
    porosity: number;               // φ [-]
    poreVolume: number;             // V_p [m³]

    // Compressibility
    c_w: number;                    // Water compressibility [1/bar]
    c_rock: number;                 // Rock compressibility [1/bar]

    // PVT mode
    pvtMode: 'constant' | 'black-oil';

    // Constant-PVT properties (used when pvtMode = 'constant')
    Bo_constant: number;            // Oil FVF [m³/m³] (typically ~1.0)
    Bw_constant: number;            // Water FVF [m³/m³] (typically ~1.0)
    c_o: number;                    // Oil compressibility [1/bar] (constant PVT only)

    // Black-oil PVT correlation inputs (used when pvtMode = 'black-oil')
    apiGravity: number;
    gasSpecificGravity: number;
    reservoirTemperature: number;   // [°C]
    bubblePoint: number;            // [bar]

    // Simulation time-series output
    pressureHistory: number[];      // Average reservoir pressure [bar] per timestep
    cumulativeOilSC: number[];      // Cumulative oil produced [m³ SC] per timestep
    cumulativeGasSC: number[];      // Cumulative gas produced [m³ SC] per timestep
    cumulativeWaterSC: number[];    // Cumulative water produced [m³ SC] per timestep
    timeHistory: number[];          // Time [days] per timestep
};

export type MaterialBalancePoint = {
    time: number;
    pressure: number;

    // MBE components
    F: number;                      // Cumulative withdrawal [res m³]
    Eo: number;                     // Oil + dissolved gas expansion [m³/m³ SC OOIP]
    Eg: number;                     // Gas cap expansion [m³/m³ SC OOIP]
    Efw: number;                    // Water/rock compaction [m³/m³ SC OOIP]
    Et: number;                     // Total expansion = Eo + m·Eg + Efw

    // OOIP estimate
    N_mbe: number | null;           // MBE-estimated OOIP [m³ SC] — null if Et ≈ 0
    N_volumetric: number;           // Known volumetric OOIP [m³ SC]

    // Drive indices (fractional contribution of each mechanism)
    driveIndex_oilExpansion: number;
    driveIndex_gasCap: number;
    driveIndex_compaction: number;
};

export type MaterialBalanceResult = {
    volumetricOoip: number;         // N from grid volumes [m³ SC]
    gasCapRatio: number;            // m [-]
    points: MaterialBalancePoint[];
};

// ─── PVT Evaluator ───────────────────────────────────────────────────────────

type PvtAtPressure = {
    Bo: number;     // Oil FVF [res m³ / SC m³]
    Rs: number;     // Solution GOR [SC m³ gas / SC m³ oil]
    Bg: number;     // Gas FVF [res m³ / SC m³]
    Bw: number;     // Water FVF [res m³ / SC m³]
};

function evaluateConstantPvt(params: MaterialBalanceParams, _pressure: number): PvtAtPressure {
    return {
        Bo: params.Bo_constant,
        Rs: 0, // No dissolved gas in constant-PVT mode
        Bg: 0,
        Bw: params.Bw_constant,
    };
}

function evaluateBlackOilPvt(params: MaterialBalanceParams, pressure: number): PvtAtPressure {
    const { apiGravity, gasSpecificGravity, reservoirTemperature, bubblePoint } = params;
    const tempF = cToF(reservoirTemperature);
    const tempR = cToR(reservoirTemperature);
    const pPsia = pressure * PSI_PER_BAR;
    const pbPsia = bubblePoint * PSI_PER_BAR;
    const sgOil = 141.5 / (131.5 + apiGravity);
    const { Tpc, Ppc } = suttonPseudoCriticals(gasSpecificGravity);

    let Rs_scf: number;
    let Bo: number;
    if (pressure <= bubblePoint) {
        // Saturated: pressure at or below bubble point
        Rs_scf = standingRs(pPsia, gasSpecificGravity, apiGravity, tempF);
        Bo = standingBo(Rs_scf, gasSpecificGravity, sgOil, tempF);
    } else {
        // Undersaturated: Rs frozen at bubble-point value, Bo shrinks via compressibility
        Rs_scf = standingRs(pbPsia, gasSpecificGravity, apiGravity, tempF);
        const Bo_pb = standingBo(Rs_scf, gasSpecificGravity, sgOil, tempF);
        const c_o = 1e-5; // undersaturated oil compressibility [1/bar]
        Bo = Bo_pb * Math.exp(-c_o * (pressure - bubblePoint));
    }

    // Gas FVF: Bg = (Psc/Tsc) × (z×T/P)
    const z = hallYarboroughZFactor(Math.max(1, pPsia), tempR, Tpc, Ppc);
    const Psc = 14.7;   // psia
    const Tsc = 519.67; // Rankine (60°F)
    const Bg = pPsia > 0 ? (Psc / Tsc) * (z * tempR / pPsia) : 1.0;

    return {
        Bo,
        Rs: Rs_scf * SCF_PER_BBL_TO_M3_PER_M3,
        Bg,
        Bw: 1.0, // Water FVF ≈ 1.0 for most conditions
    };
}

// ─── Main Computation ────────────────────────────────────────────────────────

export function calculateMaterialBalance(
    params: MaterialBalanceParams,
): MaterialBalanceResult {
    const {
        initialPressure,
        initialWaterSaturation,
        initialGasSaturation,
        poreVolume,
        c_w,
        c_rock,
        pvtMode,
        pressureHistory,
        cumulativeOilSC,
        cumulativeGasSC,
        cumulativeWaterSC,
        timeHistory,
    } = params;

    const Swi = Math.max(0, Math.min(1, initialWaterSaturation));
    const Sgi = Math.max(0, Math.min(1, initialGasSaturation));
    const Soi = Math.max(0, 1 - Swi - Sgi);

    // Evaluate PVT at initial pressure
    const evalPvt = pvtMode === 'black-oil' ? evaluateBlackOilPvt : evaluateConstantPvt;
    const pvtInitial = evalPvt(params, initialPressure);
    const Boi = pvtInitial.Bo;
    const Rsi = pvtInitial.Rs;
    const Bgi = pvtInitial.Bg;

    // Volumetric OOIP [m³ SC]
    const N_vol = Boi > 1e-12 ? poreVolume * Soi / Boi : poreVolume * Soi;

    // Gas-cap ratio m: ratio of initial free-gas reservoir volume to oil-zone reservoir volume.
    // m = (V_p × Sgi × Bgi) / (V_p × Soi × Boi) -- but cancel Vp:
    // m = (Sgi / Bgi) / (Soi / Boi) when Bgi > 0
    // For the simplified definition: m = (Sgi × Boi) / (Soi × Bgi) when both are > 0
    // Dake (1978) Eq 3.13: m = G_fgi × Bgi / (N × Boi) = Sgi / Soi (reservoir volume ratio)
    // In reservoir volumes: gas cap = Vp × Sgi, oil zone = Vp × Soi
    // m = Sgi / Soi (if we define m as the reservoir-volume ratio)
    // But the MBE uses m as defined in Havlena-Odeh where E_g = Boi × (Bg/Bgi - 1)
    // and the term is m × E_g. So m = G_fgi × Bgi / (N × Boi).
    // G_fgi = Vp × Sgi / Bgi [SC m³ gas], N × Boi = Vp × Soi [res m³ oil]
    // m = (Vp × Sgi / Bgi × Bgi) / (Vp × Soi) = Sgi / Soi
    const m = Soi > 1e-12 ? Sgi / Soi : 0;

    const n = Math.min(
        timeHistory.length,
        pressureHistory.length,
        cumulativeOilSC.length,
        cumulativeGasSC.length,
        cumulativeWaterSC.length,
    );

    const points: MaterialBalancePoint[] = [];

    for (let i = 0; i < n; i++) {
        const t = timeHistory[i] ?? 0;
        const P = pressureHistory[i] ?? initialPressure;
        const Np = Math.max(0, cumulativeOilSC[i] ?? 0);
        const Gp = Math.max(0, cumulativeGasSC[i] ?? 0);
        const Wp = Math.max(0, cumulativeWaterSC[i] ?? 0);

        const pvt = evalPvt(params, P);
        const Bo = pvt.Bo;
        const Rs = pvt.Rs;
        const Bg = pvt.Bg;
        const Bw = pvt.Bw;

        const dP = initialPressure - P; // pressure drop [bar]

        // ── F: Cumulative withdrawal at reservoir conditions ─────────
        // F = Np × [Bo + (Rp - Rs) × Bg] + Wp × Bw
        // where Rp = cumulative GOR = Gp / Np
        let F: number;
        if (pvtMode === 'black-oil' && Bg > 1e-15) {
            const Rp = Np > 1e-12 ? Gp / Np : Rsi;
            F = Np * (Bo + (Rp - Rs) * Bg) + Wp * Bw;
        } else {
            // Constant PVT: no gas production, F = Np × Bo + Wp × Bw
            F = Np * Bo + Wp * Bw;
        }

        // ── E_o: Oil expansion + dissolved gas liberation ────────────
        // E_o = (Bo - Boi) + (Rsi - Rs) × Bg
        let Eo: number;
        if (pvtMode === 'black-oil' && Bg > 1e-15) {
            Eo = (Bo - Boi) + (Rsi - Rs) * Bg;
        } else {
            // Constant PVT: Bo doesn't change, Rs = 0
            // However, for undersaturated oil with constant co:
            // Bo(P) ≈ Boi × [1 + co × (P - Pi)] (small compressibility expansion)
            // E_o = Bo - Boi ≈ Boi × co × (Pi - P)
            // This is already captured in Efw for the simplified single-phase case.
            // To avoid double-counting, set Eo = 0 for constant PVT.
            Eo = 0;
        }

        // ── E_g: Gas cap expansion ───────────────────────────────────
        // E_g = Boi × (Bg / Bgi - 1)
        let Eg: number;
        if (m > 0 && Bgi > 1e-15 && Bg > 1e-15) {
            Eg = Boi * (Bg / Bgi - 1);
        } else {
            Eg = 0;
        }

        // ── E_fw: Connate water expansion + pore compaction ──────────
        // E_fw = Boi × (c_w × S_wi + c_f) / (1 - S_wi) × ΔP
        // Note: for constant PVT, also include oil compressibility since
        // Eo = 0 above. The total compressibility term effectively captures
        // all expansion: E_fw = Boi × c_t / (1 - S_wi) × ΔP
        // where c_t = S_o × c_o + S_w × c_w + c_f (Craft & Hawkins 1959)
        let Efw: number;
        if (pvtMode === 'constant') {
            // For constant PVT, include oil compressibility in the effective
            // compaction term since Eo = 0. This gives the standard depletion MBE.
            const Soi_eff = 1 - Swi;
            const ct = Soi_eff * params.c_o + Swi * c_w + c_rock;
            Efw = Boi * ct / (1 - Swi) * dP;
        } else {
            // Black-oil PVT: oil expansion is captured in Eo, so Efw only
            // includes water + rock compressibility.
            Efw = Boi * (c_w * Swi + c_rock) / (1 - Swi) * dP;
        }

        // ── Total expansion ──────────────────────────────────────────
        const Et = Eo + m * Eg + Efw;

        // ── MBE OOIP estimate ────────────────────────────────────────
        const N_mbe = Et > 1e-15 ? F / Et : null;

        // ── Drive indices ────────────────────────────────────────────
        // Fractional contribution of each drive mechanism:
        //   N × Eo / F, N × m × Eg / F, N × Efw / F
        // Since N_mbe = F/Et, we can compute indices as Eo/Et, m×Eg/Et, Efw/Et
        let driveOil = 0;
        let driveGas = 0;
        let driveCompaction = 0;
        if (Et > 1e-15) {
            driveOil = Eo / Et;
            driveGas = m * Eg / Et;
            driveCompaction = Efw / Et;
        }

        points.push({
            time: t,
            pressure: P,
            F,
            Eo,
            Eg,
            Efw,
            Et,
            N_mbe,
            N_volumetric: N_vol,
            driveIndex_oilExpansion: driveOil,
            driveIndex_gasCap: driveGas,
            driveIndex_compaction: driveCompaction,
        });
    }

    return {
        volumetricOoip: N_vol,
        gasCapRatio: m,
        points,
    };
}
