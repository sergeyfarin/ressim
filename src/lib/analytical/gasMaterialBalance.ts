/**
 * Dry-gas material balance — the p/z straight line.
 *
 * For a volumetric gas reservoir with no water influx and negligible pore or
 * connate-water compressibility, conservation of gas at standard conditions is
 *
 *     G_p / G = 1 - (p/z) / (p_i/z_i)
 *
 * so p/z plotted against cumulative gas production is a straight line from
 * (0, p_i/z_i) to (G, 0). Its x-intercept is the gas initially in place, which
 * is why it is the most-used plot in gas reservoir engineering: it estimates
 * reserves from production and pressure alone, with no volumetric input.
 *
 * The straight line is a *consequence* of assumptions, not of material balance
 * itself. Material balance is exact; what bends the plot is the pore volume
 * not staying constant. Restoring the rock and connate-water terms gives the
 * over-pressured form (Ramagost & Farshad 1981):
 *
 *     (p/z) [1 - c_e (p_i - p)] = (p_i/z_i) (1 - G_p / G),
 *     c_e = (c_w S_wi + c_f) / (1 - S_wi)
 *
 * Reading the uncorrected line on a reservoir that needs the corrected one
 * over-estimates G, because the extra energy released by compaction and water
 * expansion is mistaken for extra gas.
 *
 * z is not taken from a correlation here. It is recovered from the same gas
 * formation volume factor the simulator itself integrates, so the reference and
 * the simulation share one gas law by construction rather than agreeing by
 * luck:
 *
 *     B_g = (p_sc / T_sc) (z T / p)   ⇒   z = B_g (T_sc / p_sc) (p / T)
 *
 * References:
 *   Craft, B.C. & Hawkins, M.F., Applied Petroleum Reservoir Engineering, ch. 5.
 *   Dake, L.P. (1978), Fundamentals of Reservoir Engineering, §1.6 and ch. 3.
 *   Ramagost, B.P. & Farshad, F.F. (1981), "P/Z Abnormally Pressured Gas
 *     Reservoirs", SPE 10125.
 *   Fetkovich, M.J., Reese, D.E. & Whitson, C.H. (1998), "Application of a
 *     General Material Balance for High-Pressure Gas Reservoirs", SPEJ 3(1).
 */

import { PSI_PER_BAR, cToR } from '../physics/pvt';

/** Standard conditions the engine's own `B_g` is referenced to (`physics/pvt.ts`). */
const P_STANDARD_PSIA = 14.7;
const T_STANDARD_RANKINE = 519.67;

/** One row of the black-oil PVT table, as far as the gas branch is concerned. */
export type GasPvtRow = {
    p_bar: number;
    bg_m3m3: number;
};

export type GasMaterialBalanceParams = {
    /** Gas formation volume factor against pressure — the engine's own table. */
    pvtTable: readonly GasPvtRow[];
    /** Reservoir temperature [°C]; the table's `B_g` was generated at it. */
    reservoirTemperature: number;
    /** Initial reservoir pressure [bar]. */
    initialPressure: number;
    /** Gas initially in place [Sm³] — volumetric, from grid pore volume. */
    giip: number;
    /** Cumulative gas produced [Sm³] per report step. */
    cumulativeGas: readonly number[];
    /** Connate water saturation [-], immobile. */
    initialWaterSaturation: number;
    /** Water compressibility [1/bar]. */
    c_w: number;
    /** Pore (formation) compressibility [1/bar]. */
    c_f: number;
};

export type GasMaterialBalancePoint = {
    cumulativeGas: number;
    /** Volumetric straight line: p/z with no compaction term [bar]. */
    pOverZ: number;
    /** Same balance with the rock and connate-water terms restored [bar]. */
    pOverZCompactionCorrected: number;
};

export type GasMaterialBalanceResult = {
    /** p/z at initial conditions [bar] — the straight line's y-intercept. */
    initialPOverZ: number;
    /** Effective compressibility c_e [1/bar]; zero makes the two curves identical. */
    effectiveCompressibility: number;
    points: GasMaterialBalancePoint[];
};

// ─── Gas law ─────────────────────────────────────────────────────────────────

/**
 * Gas deviation factor z at `pressureBar`, inverted from the table's `B_g`.
 *
 * Interpolation is linear in 1/B_g rather than in B_g, matching both OPM/ECL's
 * PVDG convention and the engine's own `PvtTable::interpolate_gas_segment` — on
 * a coarse table the two differ by more than the effect this module measures.
 */
export function gasDeviationFactor(
    pvtTable: readonly GasPvtRow[],
    pressureBar: number,
    reservoirTemperature: number,
): number {
    const bg = gasFormationVolumeFactor(pvtTable, pressureBar);
    if (!(bg > 0)) return Number.NaN;
    const pressurePsia = pressureBar * PSI_PER_BAR;
    const temperatureRankine = cToR(reservoirTemperature);
    return bg * (T_STANDARD_RANKINE / P_STANDARD_PSIA) * (pressurePsia / temperatureRankine);
}

/** B_g [res m³/Sm³] at `pressureBar`, interpolated linearly in 1/B_g. */
export function gasFormationVolumeFactor(
    pvtTable: readonly GasPvtRow[],
    pressureBar: number,
): number {
    const rows = pvtTable.filter((row) => Number.isFinite(row.p_bar) && row.bg_m3m3 > 0);
    if (rows.length === 0) return Number.NaN;
    if (rows.length === 1) return rows[0].bg_m3m3;

    let lower = rows[0];
    let upper = rows[rows.length - 1];
    if (pressureBar <= rows[0].p_bar) {
        [lower, upper] = [rows[0], rows[1]];
    } else if (pressureBar >= rows[rows.length - 1].p_bar) {
        [lower, upper] = [rows[rows.length - 2], rows[rows.length - 1]];
    } else {
        for (let index = 1; index < rows.length; index += 1) {
            if (rows[index].p_bar >= pressureBar) {
                [lower, upper] = [rows[index - 1], rows[index]];
                break;
            }
        }
    }

    const span = upper.p_bar - lower.p_bar;
    const t = Math.abs(span) < 1e-12 ? 0 : (pressureBar - lower.p_bar) / span;
    const inverse = (1 / lower.bg_m3m3) + t * ((1 / upper.bg_m3m3) - (1 / lower.bg_m3m3));
    return inverse > 0 ? 1 / inverse : Number.NaN;
}

/** p/z [bar] at `pressureBar`, from the same table the simulator integrates. */
export function pressureOverZ(
    pvtTable: readonly GasPvtRow[],
    pressureBar: number,
    reservoirTemperature: number,
): number {
    const z = gasDeviationFactor(pvtTable, pressureBar, reservoirTemperature);
    return z > 0 ? pressureBar / z : Number.NaN;
}

// ─── Material balance ────────────────────────────────────────────────────────

/**
 * Effective compressibility c_e = (c_w S_wi + c_f) / (1 - S_wi).
 *
 * The pore volume the gas occupies shrinks as the rock compacts and the connate
 * water expands, and both release gas that the uncorrected plot attributes to a
 * larger reservoir.
 */
export function effectiveCompressibility(
    c_w: number,
    c_f: number,
    initialWaterSaturation: number,
): number {
    const gasFraction = 1 - initialWaterSaturation;
    if (!(gasFraction > 0)) return 0;
    return ((c_w * initialWaterSaturation) + c_f) / gasFraction;
}

/**
 * The p/z reference curves against a cumulative-production axis.
 *
 * The compaction-corrected curve is implicit — its correction term depends on
 * the pressure it is solving for — so it is closed by fixed-point iteration on
 * p/z, which converges in a handful of passes for any physical c_e·Δp < 1.
 */
export function computeGasMaterialBalance(
    params: GasMaterialBalanceParams,
): GasMaterialBalanceResult {
    const {
        pvtTable, reservoirTemperature, initialPressure, giip, cumulativeGas,
        initialWaterSaturation, c_w, c_f,
    } = params;

    const initialPOverZ = pressureOverZ(pvtTable, initialPressure, reservoirTemperature);
    const c_e = effectiveCompressibility(c_w, c_f, initialWaterSaturation);

    const points = cumulativeGas.map((produced) => {
        const remainingFraction = giip > 0
            ? Math.max(0, 1 - (Math.max(0, produced) / giip))
            : Number.NaN;
        const pOverZ = initialPOverZ * remainingFraction;
        return {
            cumulativeGas: produced,
            pOverZ,
            pOverZCompactionCorrected: solveCompactionCorrected(
                pOverZ,
                initialPOverZ,
                initialPressure,
                c_f,
                c_w,
                initialWaterSaturation,
                pvtTable,
                reservoirTemperature,
            ),
        };
    });

    return { initialPOverZ, effectiveCompressibility: c_e, points };
}

/**
 * Solves (p/z)[1 - c_e (p_i - p)] = target for p/z.
 *
 * Needs the pressure that goes with a given p/z, which the table supplies by
 * inversion — p/z is monotonic in p over any physical range, so a bisection is
 * both simplest and safe.
 */
function solveCompactionCorrected(
    target: number,
    initialPOverZ: number,
    initialPressure: number,
    c_f: number,
    c_w: number,
    initialWaterSaturation: number,
    pvtTable: readonly GasPvtRow[],
    reservoirTemperature: number,
): number {
    if (!Number.isFinite(target)) return Number.NaN;
    if (c_f <= 0 && c_w <= 0) return target;

    let value = target;
    for (let iteration = 0; iteration < 40; iteration += 1) {
        const pressure = pressureForPOverZ(value, initialPressure, pvtTable, reservoirTemperature);
        const drawdown = Math.max(0, initialPressure - pressure);
        const gasFraction = 1 - initialWaterSaturation;
        if (!(gasFraction > 0)) return Number.NaN;
        // Match the simulator/deck inventory exactly: pore volume contracts
        // exponentially from the reference pressure while connate water
        // expands from its initial volume. The usual Ramagost–Farshad linear
        // term is only the first-order form and visibly drifts in the high-c_f
        // cases this scenario is intended to demonstrate.
        const correction = (
            Math.exp(-Math.max(0, c_f) * drawdown)
            - initialWaterSaturation * (1 + Math.max(0, c_w) * drawdown)
        ) / gasFraction;
        if (!(correction > 1e-6)) return Number.NaN;
        const next = target / correction;
        if (Math.abs(next - value) < 1e-9 * Math.max(1, Math.abs(next))) return Math.min(next, initialPOverZ);
        value = next;
    }
    return Math.min(value, initialPOverZ);
}

/** Inverts p/z back to pressure by bisection on [0, p_i]. */
export function pressureForPOverZ(
    targetPOverZ: number,
    initialPressure: number,
    pvtTable: readonly GasPvtRow[],
    reservoirTemperature: number,
): number {
    if (!Number.isFinite(targetPOverZ) || targetPOverZ <= 0) return 0;
    let low = 0;
    let high = Math.max(initialPressure, 1);
    for (let iteration = 0; iteration < 80; iteration += 1) {
        const mid = 0.5 * (low + high);
        const value = mid <= 0 ? 0 : pressureOverZ(pvtTable, mid, reservoirTemperature);
        if (!Number.isFinite(value)) return mid;
        if (value < targetPOverZ) low = mid; else high = mid;
    }
    return 0.5 * (low + high);
}

// ─── Straight-line interpretation ────────────────────────────────────────────

export type StraightLineFit = {
    /** Gas initially in place implied by the line's x-intercept [Sm³]. */
    giip: number;
    /** Fitted y-intercept [bar] — the p/z the line claims at zero production. */
    initialPOverZ: number;
    /** Slope [bar/Sm³], negative for a depleting reservoir. */
    slope: number;
};

/**
 * The extrapolation an engineer actually performs: least squares through the
 * stabilised part of the history, extended to p/z = 0.
 *
 * `tailFraction` selects how much of the record to use, because the early
 * points of a real gas field are the ones most contaminated by transients and
 * are routinely discarded before drawing the line.
 */
export function fitStraightLineGiip(
    cumulativeGas: readonly number[],
    pOverZ: readonly (number | null)[],
    tailFraction = 1 / 3,
): StraightLineFit | null {
    const samples: Array<{ x: number; y: number }> = [];
    for (let index = 0; index < cumulativeGas.length; index += 1) {
        const x = cumulativeGas[index];
        const y = pOverZ[index];
        if (Number.isFinite(x) && Number.isFinite(y)) samples.push({ x, y: Number(y) });
    }
    const count = Math.max(2, Math.floor(samples.length * Math.min(1, Math.max(0, tailFraction))));
    const tail = samples.slice(-count);
    if (tail.length < 2) return null;

    const meanX = tail.reduce((sum, point) => sum + point.x, 0) / tail.length;
    const meanY = tail.reduce((sum, point) => sum + point.y, 0) / tail.length;
    const covariance = tail.reduce((sum, point) => sum + (point.x - meanX) * (point.y - meanY), 0);
    const variance = tail.reduce((sum, point) => sum + (point.x - meanX) ** 2, 0);
    if (!(variance > 0) || !(Math.abs(covariance) > 0)) return null;

    const slope = covariance / variance;
    if (!(slope < 0)) return null;
    return {
        slope,
        initialPOverZ: meanY - (slope * meanX),
        giip: meanX - (meanY / slope),
    };
}
