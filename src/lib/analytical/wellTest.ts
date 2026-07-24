/**
 * Pressure-transient analysis — radial flow, drawdown and Horner buildup.
 *
 * The classical well-test toolkit: the line-source (exponential-integral)
 * solution for an infinite-acting radial system, its semilog approximation,
 * and the two inverse problems built on that approximation — recovering
 * permeability from the semilog slope and skin from the one-hour intercept.
 *
 * Status: the mathematics and its tests. This module is not yet wired to a
 * scenario — that needs a new `AnalyticalMethod` union member, an adapter in
 * `catalog/analyticalAdapters.ts`, and a semilog chart layout (enabler E10 /
 * case T7.1 in `docs/CASE_LIBRARY_ROADMAP.md`). Kept separate deliberately:
 * the `add-scenario` skill asks for a new analytical method to land as its own
 * commit, ahead of the scenario that consumes it.
 *
 * Units — the project's metric convention throughout (`docs/UNIT_REFERENCE.md`):
 *   permeability   k      mD
 *   thickness      h      m
 *   radius         r      m
 *   viscosity      mu     cP
 *   pressure       p      bar
 *   rate           q      m3/day  (reservoir volume, not surface)
 *   compressibility c_t   1/bar
 *   time           t      days
 *
 * Everything is derived from the engine's own Darcy constant rather than
 * imported from a field-unit textbook formula, so the constants here cannot
 * drift away from the simulator's transmissibility convention:
 *
 *   q = C . k . A . dp / (mu . L),   C = DARCY_METRIC_FACTOR = 8.5269888e-3
 *
 * From that, radial Darcy flow over a log span gives the pressure group
 * q.mu / (4.pi.C.k.h), and hydraulic diffusivity eta = C.k / (phi.mu.c_t)
 * in m2/day (derived in `hydraulicDiffusivity` below).
 *
 * References:
 *   Theis, C.V. (1935) Trans. AGU 16 — the line-source solution.
 *   Horner, D.R. (1951) "Pressure Build-up in Wells", 3rd World Pet. Congress.
 *   Matthews, C.S. & Russell, D.G. (1967) "Pressure Buildup and Flow Tests in
 *     Wells", SPE Monograph 1.
 *   Earlougher, R.C. (1977) "Advances in Well Test Analysis", SPE Monograph 5.
 *   Bourdet, D. et al. (1983) "A New Set of Type Curves…", World Oil — the
 *     pressure-derivative diagnostic.
 *   Dake, L.P. (1978) "Fundamentals of Reservoir Engineering", ch. 7.
 */

import { DARCY_METRIC_FACTOR } from './depletionAnalytical';

/** Euler-Mascheroni constant. */
export const EULER_GAMMA = 0.5772156649015329;

/** exp(gamma) — the 1.781 that appears inside the semilog log argument. */
export const EXP_EULER_GAMMA = Math.exp(EULER_GAMMA);

/**
 * The semilog approximation E1(u) ~= -ln(u) - gamma is standard practice for
 * u below this; the error at u = 0.01 is about 1%. Exposed so callers (and
 * tests) can state where infinite-acting radial flow analysis is valid.
 */
export const SEMILOG_VALIDITY_U = 0.01;

export type ReservoirTestProps = {
    /** Permeability [mD]. */
    k: number;
    /** Net thickness [m]. */
    h: number;
    /** Porosity [-]. */
    porosity: number;
    /** Viscosity [cP]. */
    mu: number;
    /** Total compressibility [1/bar]. */
    c_t: number;
    /** Wellbore radius [m]. */
    r_w: number;
    /** Skin factor [-]. */
    skin: number;
};

/**
 * Exponential integral E1(u) = integral from u to infinity of exp(-x)/x dx.
 *
 * Two regimes, both from Abramowitz & Stegun ch. 5: the power series (5.1.11)
 * below u = 1, and the rational approximation (5.1.56) above it. Accurate to
 * better than 1e-10 relative across the range this module needs.
 *
 * Returns Infinity at u = 0 (the line-source singularity) and NaN for u < 0.
 */
export function exponentialIntegralE1(u: number): number {
    if (Number.isNaN(u) || u < 0) return Number.NaN;
    if (u === 0) return Number.POSITIVE_INFINITY;

    if (u < 1) {
        // E1(u) = -gamma - ln(u) + sum_{n>=1} (-1)^(n+1) u^n / (n . n!)
        let sum = 0;
        let term = 1;
        for (let n = 1; n <= 60; n++) {
            term *= -u / n;
            const contribution = -term / n;
            sum += contribution;
            if (Math.abs(contribution) < 1e-18 * Math.abs(sum)) break;
        }
        return -EULER_GAMMA - Math.log(u) + sum;
    }

    // A&S 5.1.56: E1(u) = exp(-u)/u . (u^2 + a1.u + a2) / (u^2 + b1.u + b2) . (1 + eps),
    // |eps| < 5e-5. Refined below with a continued-fraction sweep for full
    // double precision.
    let cf = 0;
    for (let n = 60; n >= 1; n--) {
        cf = n / (1 + n / (u + cf));
    }
    return Math.exp(-u) / (u + cf);
}

/**
 * Hydraulic diffusivity eta [m2/day].
 *
 * Derivation in the project's units, so this stays tied to the simulator's own
 * Darcy constant. For a block of length L and area A:
 *   transmissibility T = C.k.A/(mu.L)   [m3/day/bar]
 *   storage          S = phi.c_t.A.L    [m3/bar]
 *   time constant  tau = S/T = phi.mu.c_t.L^2 / (C.k)
 * so eta = L^2/tau = C.k / (phi.mu.c_t).
 */
export function hydraulicDiffusivity(props: Pick<ReservoirTestProps, 'k' | 'porosity' | 'mu' | 'c_t'>): number {
    const { k, porosity, mu, c_t } = props;
    if (porosity <= 0 || mu <= 0 || c_t <= 0) return Number.NaN;
    return (DARCY_METRIC_FACTOR * k) / (porosity * mu * c_t);
}

/**
 * The pressure group q.mu / (4.pi.C.k.h) [bar] that scales every drawdown in
 * radial flow. One semilog cycle of drawdown is ln(10) times this.
 */
export function radialPressureGroup(q: number, props: Pick<ReservoirTestProps, 'k' | 'h' | 'mu'>): number {
    const { k, h, mu } = props;
    if (k <= 0 || h <= 0) return Number.NaN;
    return (q * mu) / (4 * Math.PI * DARCY_METRIC_FACTOR * k * h);
}

/**
 * Semilog slope m [bar per log10 cycle] of an infinite-acting radial
 * drawdown. The quantity a straight-line fit on a p vs log10(t) plot
 * recovers, and the basis of `permeabilityFromSemilogSlope`.
 */
export function semilogSlope(q: number, props: Pick<ReservoirTestProps, 'k' | 'h' | 'mu'>): number {
    return Math.LN10 * radialPressureGroup(q, props);
}

/**
 * Pressure at radius r and time t for a well produced at constant rate q from
 * time zero in an infinite-acting radial system — the full line-source (Theis)
 * solution, valid at all times rather than only in the semilog regime.
 *
 * Skin is a wellbore-only pressure drop: it is applied when `r` is the
 * wellbore radius and ignored in the reservoir, which is what the thin-skin
 * idealisation means.
 *
 * @param p_i initial reservoir pressure [bar]
 * @param q   constant production rate [m3/day], positive for production
 * @param r   radius [m]
 * @param t   time since the rate change [days]
 */
export function lineSourcePressure(
    p_i: number,
    q: number,
    r: number,
    t: number,
    props: ReservoirTestProps,
): number {
    if (t <= 0) return p_i;
    const eta = hydraulicDiffusivity(props);
    const group = radialPressureGroup(q, props);
    const u = (r * r) / (4 * eta * t);
    const skinTerm = r <= props.r_w * (1 + 1e-9) ? 2 * props.skin : 0;
    return p_i - group * (exponentialIntegralE1(u) + skinTerm);
}

/**
 * Flowing bottomhole pressure during a constant-rate drawdown, using the
 * semilog (log-approximation) form rather than the full E1. Equivalent to
 * `lineSourcePressure` at the wellbore once u < SEMILOG_VALIDITY_U; the two
 * are cross-checked against each other in the tests.
 *
 * p_wf(t) = p_i - group . [ ln(4.eta.t / (exp(gamma).r_w^2)) + 2.s ]
 */
export function drawdownPressure(p_i: number, q: number, t: number, props: ReservoirTestProps): number {
    if (t <= 0) return p_i;
    const eta = hydraulicDiffusivity(props);
    const group = radialPressureGroup(q, props);
    const logArg = (4 * eta * t) / (EXP_EULER_GAMMA * props.r_w * props.r_w);
    return p_i - group * (Math.log(logArg) + 2 * props.skin);
}

/**
 * Horner time (t_p + dt)/dt for a buildup following a production period.
 *
 * @param producingTime t_p [days] — total production before shut-in
 * @param shutInTime    dt  [days] — elapsed time since shut-in
 */
export function hornerTime(producingTime: number, shutInTime: number): number {
    if (shutInTime <= 0) return Number.POSITIVE_INFINITY;
    return (producingTime + shutInTime) / shutInTime;
}

/**
 * Shut-in pressure during a Horner buildup, by superposition of the producing
 * well and an injection well started at shut-in:
 *
 *   p_ws(dt) = p_i - group . ln( (t_p + dt) / dt )
 *
 * Skin cancels in the buildup — which is why a Horner plot gives permeability
 * cleanly but needs the flowing pressure at shut-in to give skin.
 */
export function buildupPressure(
    p_i: number,
    q: number,
    producingTime: number,
    shutInTime: number,
    props: ReservoirTestProps,
): number {
    if (shutInTime <= 0) return drawdownPressure(p_i, q, producingTime, props);
    const group = radialPressureGroup(q, props);
    return p_i - group * Math.log(hornerTime(producingTime, shutInTime));
}

/**
 * Permeability from a measured semilog slope — the primary well-test result.
 *
 * Inverts `semilogSlope`: k = ln(10) . q . mu / (4.pi.C.h.m).
 *
 * @param slope m [bar per log10 cycle], as a positive magnitude
 */
export function permeabilityFromSemilogSlope(
    slope: number,
    q: number,
    h: number,
    mu: number,
): number {
    const m = Math.abs(slope);
    if (m <= 0 || h <= 0) return Number.NaN;
    return (Math.LN10 * q * mu) / (4 * Math.PI * DARCY_METRIC_FACTOR * h * m);
}

/**
 * Skin from the semilog straight line's one-hour intercept.
 *
 * Rearranging the drawdown form at t = t_ref:
 *   s = 0.5 . [ ln(10) . dp_ref / m  -  ln( 4.eta.t_ref / (exp(gamma).r_w^2) ) ]
 *
 * Note this is derived from the module's own constants rather than lifted from
 * the field-unit "3.2275" version in the textbooks, so it carries no hidden
 * unit conversion.
 *
 * @param deltaPAtRef p_i - p_wf read off the *extrapolated straight line* at
 *                    t_ref [bar] — not the raw measurement, which may still be
 *                    in wellbore storage
 * @param slope       semilog slope magnitude [bar/cycle]
 * @param tRefDays    reference time [days]; defaults to one hour, the convention
 */
export function skinFromSemilogIntercept(
    deltaPAtRef: number,
    slope: number,
    props: Pick<ReservoirTestProps, 'k' | 'porosity' | 'mu' | 'c_t' | 'r_w'>,
    tRefDays: number = 1 / 24,
): number {
    const m = Math.abs(slope);
    if (m <= 0 || tRefDays <= 0) return Number.NaN;
    const eta = hydraulicDiffusivity(props);
    const logArg = (4 * eta * tRefDays) / (EXP_EULER_GAMMA * props.r_w * props.r_w);
    return 0.5 * ((Math.LN10 * deltaPAtRef) / m - Math.log(logArg));
}

/**
 * Radius of investigation [m], r_inv = 2.sqrt(eta.t).
 *
 * One of several conventions in the literature (Earlougher §2.7 discusses the
 * spread); this is the common "pressure-disturbance" definition. Treat it as
 * an order-of-magnitude statement of how far the test has seen, not a sharp
 * boundary.
 */
export function radiusOfInvestigation(t: number, props: Pick<ReservoirTestProps, 'k' | 'porosity' | 'mu' | 'c_t'>): number {
    if (t <= 0) return 0;
    return 2 * Math.sqrt(hydraulicDiffusivity(props) * t);
}

/**
 * Earliest time at which the semilog approximation is valid at the wellbore,
 * i.e. when u = r_w^2/(4.eta.t) falls below SEMILOG_VALIDITY_U [days].
 *
 * The honest companion to every k and s this module reports: a straight line
 * fitted before this time is not measuring infinite-acting radial flow.
 */
export function semilogValidFromTime(props: Pick<ReservoirTestProps, 'k' | 'porosity' | 'mu' | 'c_t' | 'r_w'>): number {
    const eta = hydraulicDiffusivity(props);
    if (!(eta > 0)) return Number.NaN;
    return (props.r_w * props.r_w) / (4 * eta * SEMILOG_VALIDITY_U);
}

export type SemilogFit = {
    /** Slope [bar per log10 cycle], positive magnitude. */
    slope: number;
    /** Intercept of the fitted line, in the same pressure units. */
    intercept: number;
    /** Number of points used. */
    count: number;
};

/**
 * Least-squares straight-line fit of pressure against log10(x) — the manual
 * step of both a drawdown (x = t) and a Horner (x = Horner time) analysis.
 *
 * Points with x <= 0 are skipped. Returns slope as a positive magnitude
 * together with the signed intercept, so callers can extrapolate the line
 * without having to remember the sign convention of the plot they are on.
 */
export function fitSemilogLine(points: Array<{ x: number; y: number }>): SemilogFit {
    let n = 0;
    let sumX = 0;
    let sumY = 0;
    let sumXX = 0;
    let sumXY = 0;
    for (const { x, y } of points) {
        if (!(x > 0) || !Number.isFinite(y)) continue;
        const lx = Math.log10(x);
        n++;
        sumX += lx;
        sumY += y;
        sumXX += lx * lx;
        sumXY += lx * y;
    }
    if (n < 2) return { slope: Number.NaN, intercept: Number.NaN, count: n };
    const denom = n * sumXX - sumX * sumX;
    if (denom === 0) return { slope: Number.NaN, intercept: Number.NaN, count: n };
    const rawSlope = (n * sumXY - sumX * sumY) / denom;
    const intercept = (sumY - rawSlope * sumX) / n;
    return { slope: Math.abs(rawSlope), intercept, count: n };
}
