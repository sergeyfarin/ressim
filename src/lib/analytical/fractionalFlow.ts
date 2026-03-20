/**
 * Pure-function implementations of Buckley-Leverett / fractional flow analytics.
 * Extracted from FractionalFlow.svelte for testability.
 *
 * Supports both water-oil and gas-oil displacement systems.
 */

export type RockProps = {
    s_wc: number;
    s_or: number;
    n_w: number;
    n_o: number;
    k_rw_max: number;
    k_ro_max: number;
};

export type FluidProps = {
    mu_w: number;
    mu_o: number;
};

/** Gas-oil rock properties for gas-oil Buckley-Leverett. */
export type GasOilRockProps = {
    s_wc: number;   // connate water (immobile)
    s_gc: number;   // critical gas saturation
    s_gr: number;   // residual (trapped) gas
    s_org: number;  // residual oil to gas
    n_o: number;    // Corey exponent — oil
    n_g: number;    // Corey exponent — gas
    k_ro_max: number;
    k_rg_max: number;
};

/** Gas-oil fluid properties. */
export type GasOilFluidProps = {
    mu_o: number;
    mu_g: number;
};

export type WelgeMetrics = {
    shockSw: number;
    breakthroughPvi: number;
    waterCutAtBreakthrough: number;
    initialSw: number;
};

/** Welge metrics for gas-oil system (saturation variable is S_g). */
export type GasOilWelgeMetrics = {
    shockSg: number;
    breakthroughPvi: number;
    gasCutAtBreakthrough: number;
    initialSg: number;
};

export type AnalyticalPoint = {
    time: number;
    oilRate: number;
    waterRate: number;
    cumulativeOil: number;
};

// ── Relative permeability (Corey model) ──
export function k_rw(s_w: number, rock: RockProps): number {
    const s_eff = Math.max(0, Math.min(1, (s_w - rock.s_wc) / (1 - rock.s_wc - rock.s_or)));
    return rock.k_rw_max * Math.pow(s_eff, rock.n_w);
}

export function k_ro(s_w: number, rock: RockProps): number {
    const s_eff = Math.max(0, Math.min(1, (1 - s_w - rock.s_or) / (1 - rock.s_wc - rock.s_or)));
    return rock.k_ro_max * Math.pow(s_eff, rock.n_o);
}

// ── Fractional flow ──
export function fractionalFlow(s_w: number, rock: RockProps, fluid: FluidProps): number {
    const krw = k_rw(s_w, rock);
    const kro = k_ro(s_w, rock);
    const numerator = krw / fluid.mu_w;
    const denominator = numerator + kro / fluid.mu_o;
    if (denominator === 0) return 0;
    return numerator / denominator;
}

// ── Fractional flow derivative (central difference) ──
export function dfw_dSw(s_w: number, rock: RockProps, fluid: FluidProps, ds = 1e-6): number {
    const sMin = rock.s_wc;
    const sMax = 1 - rock.s_or;
    if (s_w < sMin || s_w > sMax) return 0;
    const fw_plus = fractionalFlow(Math.min(sMax, s_w + ds), rock, fluid);
    const fw_minus = fractionalFlow(Math.max(sMin, s_w - ds), rock, fluid);
    return (fw_plus - fw_minus) / (2 * ds);
}

// ── Welge tangent construction ──
export function computeWelgeMetrics(rock: RockProps, fluid: FluidProps, initialSaturation: number): WelgeMetrics {
    const sMin = rock.s_wc;
    const sMax = 1 - rock.s_or;
    const initialSwClamped = Math.max(sMin, Math.min(sMax, initialSaturation));

    const fwInitial = fractionalFlow(initialSwClamped, rock, fluid);
    let swShock = initialSwClamped;
    let maxSlope = 0;
    for (let s = initialSwClamped + 5e-4; s <= sMax; s += 5e-4) {
        const fw = fractionalFlow(s, rock, fluid);
        const slope = (fw - fwInitial) / Math.max(1e-12, s - initialSwClamped);
        if (slope > maxSlope && Number.isFinite(slope)) {
            maxSlope = slope;
            swShock = s;
        }
    }

    const fwShock = fractionalFlow(swShock, rock, fluid);
    const dfwAtShock = (fwShock - fwInitial) / Math.max(1e-12, swShock - initialSwClamped);
    const breakthroughPvi = dfwAtShock > 1e-12 ? 1.0 / dfwAtShock : 0;

    return {
        shockSw: swShock,
        breakthroughPvi,
        waterCutAtBreakthrough: fwShock,
        initialSw: initialSwClamped,
    };
}

// ── BL recovery factor vs PVI (Welge construction, pure analytical) ──

/**
 * Compute Buckley-Leverett recovery factor as a function of PVI for a 1D tube
 * with perfect areal and vertical sweep (E_A = E_V = 1).
 *
 * Uses the Welge (1952) material balance:
 *   Before breakthrough:  S̄_w = S_wc + PVI × (1 − fw_initial)
 *   After breakthrough:   S̄_w = S_w2 + PVI × (1 − fw(S_w2))
 *     where S_w2 satisfies  1/PVI = dfw/dSw|_{S_w2}
 *
 * RF = (S̄_w − S_wc) / (1 − S_wc)
 *
 * This is parameterised purely by PVI and rock/fluid props — independent of
 * rate, time, or grid geometry. It represents the maximum possible recovery
 * for a given fluid system, achieved only when sweep is perfect (1D slab).
 *
 * Assumes initial water saturation = S_wc (connate only).
 * Ignores expansion corrections (Bo ≈ 1, incompressible).
 */
export function computeBLRecoveryVsPVI(
    rock: RockProps,
    fluid: FluidProps,
    pviMax: number = 3.0,
    nPoints: number = 200,
): { pvi: number; rf: number }[] {
    const s_wc = rock.s_wc;
    const sMax = 1 - rock.s_or;
    const fw_initial = fractionalFlow(s_wc, rock, fluid);

    // Welge tangent: find shock front saturation (replicates computeWelgeMetrics logic)
    let s_wf = s_wc;
    let maxSlope = 0;
    for (let s = s_wc + 5e-4; s <= sMax; s += 5e-4) {
        const fw = fractionalFlow(s, rock, fluid);
        const slope = (fw - fw_initial) / Math.max(1e-12, s - s_wc);
        if (slope > maxSlope && Number.isFinite(slope)) { maxSlope = slope; s_wf = s; }
    }

    const fw_shock = fractionalFlow(s_wf, rock, fluid);
    const dfw_shock = (fw_shock - fw_initial) / Math.max(1e-12, s_wf - s_wc);
    const pvi_bt = dfw_shock > 1e-12 ? 1.0 / dfw_shock : Infinity;

    // Binary-search Sw at outlet post-BT: 1/PVI = dfw/dSw|_{Sw_outlet}
    function findOutletSw(targetDfw: number): number {
        let lo = s_wf, hi = sMax;
        const dfwLo = dfw_dSw(lo, rock, fluid, 1e-4);
        const dfwHi = dfw_dSw(hi, rock, fluid, 1e-4);
        if (targetDfw >= dfwLo) return lo;
        if (targetDfw <= dfwHi) return hi;
        for (let iter = 0; iter < 60; iter++) {
            const mid = 0.5 * (lo + hi);
            if (dfw_dSw(mid, rock, fluid, 1e-4) > targetDfw) lo = mid; else hi = mid;
            if (hi - lo < 1e-7) break;
        }
        return 0.5 * (lo + hi);
    }

    const result: { pvi: number; rf: number }[] = [];
    for (let i = 0; i <= nPoints; i++) {
        const pvi = (i / nPoints) * pviMax;
        let swAvg: number;
        if (pvi <= 0) {
            swAvg = s_wc;
        } else if (pvi <= pvi_bt) {
            // Before breakthrough: Welge material balance
            swAvg = s_wc + pvi * (1 - fw_initial);
        } else {
            // After breakthrough: Welge equation
            const s_w2 = findOutletSw(1.0 / pvi);
            swAvg = s_w2 + pvi * (1 - fractionalFlow(s_w2, rock, fluid));
        }
        const rf = Math.max(0, Math.min(1, (swAvg - s_wc) / Math.max(1e-12, 1 - s_wc)));
        result.push({ pvi, rf });
    }
    return result;
}

// ── Analytical production (Buckley-Leverett) ──
export function calculateAnalyticalProduction(
    rock: RockProps,
    fluid: FluidProps,
    initialSaturation: number,
    timeHistory: number[],
    injectionRateSeries: number[],
    poreVolume: number,
): AnalyticalPoint[] {
    const initial_sw = Math.max(rock.s_wc, Math.min(1 - rock.s_or, initialSaturation));
    const fw_initial = fractionalFlow(initial_sw, rock, fluid);

    let sw_f = initial_sw;
    let max_slope = 0;
    for (let s = initial_sw + 5e-4; s <= 1 - rock.s_or; s += 5e-4) {
        const fw = fractionalFlow(s, rock, fluid);
        const slope = (fw - fw_initial) / Math.max(1e-12, s - initial_sw);
        if (slope > max_slope) {
            max_slope = slope;
            sw_f = s;
        }
    }

    const fw_at_shock = fractionalFlow(sw_f, rock, fluid);
    const dfw_at_shock = (fw_at_shock - fw_initial) / Math.max(1e-12, sw_f - initial_sw);
    const breakthroughPVI = dfw_at_shock > 1e-12 ? 1.0 / dfw_at_shock : Number.POSITIVE_INFINITY;

    const q0 = injectionRateSeries.find(rate => Number.isFinite(rate) && rate > 0) ?? 0;
    if (q0 <= 0) {
        return timeHistory.map(t => ({ time: t, oilRate: 0, waterRate: 0, cumulativeOil: 0 }));
    }

    function findOutletSw(target_dfw: number): number {
        let lo = sw_f;
        let hi = 1 - rock.s_or;
        const dfw_lo = dfw_dSw(lo, rock, fluid, 1e-4);
        const dfw_hi = dfw_dSw(hi, rock, fluid, 1e-4);
        if (target_dfw >= dfw_lo) return lo;
        if (target_dfw <= dfw_hi) return hi;
        for (let iter = 0; iter < 50; iter++) {
            const mid = 0.5 * (lo + hi);
            const dfw_mid = dfw_dSw(mid, rock, fluid, 1e-4);
            if (dfw_mid > target_dfw) lo = mid;
            else hi = mid;
            if (hi - lo < 1e-6) break;
        }
        return 0.5 * (lo + hi);
    }

    const result: AnalyticalPoint[] = [];
    let cumulativeOil = 0;
    let cumulativePVI = 0;

    for (let i = 0; i < timeHistory.length; i++) {
        const t = timeHistory[i];
        const q = Number.isFinite(injectionRateSeries[i]) && injectionRateSeries[i] > 0
            ? injectionRateSeries[i] : q0;
        const dt = i > 0 ? Math.max(0, t - timeHistory[i - 1]) : Math.max(0, t);
        if (poreVolume > 0) cumulativePVI += (q * dt) / poreVolume;

        let oilRate: number;
        if (cumulativePVI <= breakthroughPVI) {
            oilRate = q * (1 - fw_initial);
        } else {
            const target_dfw = cumulativePVI > 1e-12 ? 1.0 / cumulativePVI : dfw_at_shock;
            const s_w_at_outlet = findOutletSw(target_dfw);
            const fw_at_outlet = fractionalFlow(s_w_at_outlet, rock, fluid);
            oilRate = q * (1 - fw_at_outlet);
        }
        const boundedOilRate = Math.max(0, oilRate);
        const waterRate = Math.max(0, q - boundedOilRate);
        cumulativeOil += boundedOilRate * dt;

        result.push({ time: t, oilRate: boundedOilRate, waterRate, cumulativeOil });
    }
    return result;
}

// ══════════════════════════════════════════════════════════════════════════════
// Gas-Oil Buckley-Leverett
// ══════════════════════════════════════════════════════════════════════════════
//
// Displacement variable: S_g (gas saturation).
// Water is immobile at S_wc throughout.
// Oil saturation: S_o = 1 − S_wc − S_g.
//
// Gas relperm:   k_rg(S_g)   = k_rg_max × [(S_g − S_gc) / (1 − S_wc − S_gc − S_gr)]^n_g
// Oil relperm:   k_ro_g(S_g) = k_ro_max × [(1 − S_wc − S_g − S_org) / (1 − S_wc − S_org)]^n_o
// Fractional flow: f_g = (k_rg/μ_g) / (k_rg/μ_g + k_ro_g/μ_o)
//
// Mobile gas range: S_g ∈ [S_gc, 1 − S_wc − S_org]
// ══════════════════════════════════════════════════════════════════════════════

// ── Gas-oil relative permeability (Corey model) ──

export function k_rg(s_g: number, rock: GasOilRockProps): number {
    const denom = 1 - rock.s_wc - rock.s_gc - rock.s_gr;
    if (denom <= 0) return 0;
    const s_eff = Math.max(0, Math.min(1, (s_g - rock.s_gc) / denom));
    return rock.k_rg_max * Math.pow(s_eff, rock.n_g);
}

export function k_ro_gas(s_g: number, rock: GasOilRockProps): number {
    const denom = 1 - rock.s_wc - rock.s_org;
    if (denom <= 0) return 0;
    const s_eff = Math.max(0, Math.min(1, (1 - rock.s_wc - s_g - rock.s_org) / denom));
    return rock.k_ro_max * Math.pow(s_eff, rock.n_o);
}

// ── Gas-oil fractional flow ──

export function fractionalFlowGas(s_g: number, rock: GasOilRockProps, fluid: GasOilFluidProps): number {
    const krg = k_rg(s_g, rock);
    const kro = k_ro_gas(s_g, rock);
    const numerator = krg / fluid.mu_g;
    const denominator = numerator + kro / fluid.mu_o;
    if (denominator === 0) return 0;
    return numerator / denominator;
}

// ── Gas-oil fractional flow derivative (central difference) ──

export function dfg_dSg(s_g: number, rock: GasOilRockProps, fluid: GasOilFluidProps, ds = 1e-6): number {
    const sMin = rock.s_gc;
    const sMax = 1 - rock.s_wc - rock.s_org;
    if (s_g < sMin || s_g > sMax) return 0;
    const fg_plus = fractionalFlowGas(Math.min(sMax, s_g + ds), rock, fluid);
    const fg_minus = fractionalFlowGas(Math.max(sMin, s_g - ds), rock, fluid);
    return (fg_plus - fg_minus) / (2 * ds);
}

// ── Gas-oil Welge tangent construction ──

export function computeWelgeMetricsGas(
    rock: GasOilRockProps,
    fluid: GasOilFluidProps,
    initialGasSaturation: number,
): GasOilWelgeMetrics {
    const sMin = rock.s_gc;
    const sMax = 1 - rock.s_wc - rock.s_org;
    const initialSgClamped = Math.max(0, Math.min(sMax, initialGasSaturation));

    // fg at initial gas saturation (typically 0 since S_g_init < S_gc)
    const fgInitial = fractionalFlowGas(Math.max(sMin, initialSgClamped), rock, fluid);

    let sgShock = sMin;
    let maxSlope = 0;
    for (let s = sMin + 5e-4; s <= sMax; s += 5e-4) {
        const fg = fractionalFlowGas(s, rock, fluid);
        // Tangent from (initialSg, fg(initialSg)) to (s, fg(s))
        const slope = (fg - fgInitial) / Math.max(1e-12, s - initialSgClamped);
        if (slope > maxSlope && Number.isFinite(slope)) {
            maxSlope = slope;
            sgShock = s;
        }
    }

    const fgShock = fractionalFlowGas(sgShock, rock, fluid);
    const dfgAtShock = (fgShock - fgInitial) / Math.max(1e-12, sgShock - initialSgClamped);
    const breakthroughPvi = dfgAtShock > 1e-12 ? 1.0 / dfgAtShock : 0;

    return {
        shockSg: sgShock,
        breakthroughPvi,
        gasCutAtBreakthrough: fgShock,
        initialSg: initialSgClamped,
    };
}

// ── Gas-oil BL recovery factor vs PVI ──

export function computeGasOilRecoveryVsPVI(
    rock: GasOilRockProps,
    fluid: GasOilFluidProps,
    pviMax: number = 3.0,
    nPoints: number = 200,
): { pvi: number; rf: number }[] {
    const sMax = 1 - rock.s_wc - rock.s_org;
    const s_gc = rock.s_gc;
    // Initial state: no free gas (S_g = 0), oil saturation = 1 − S_wc
    const initialSg = 0;
    const fgInitial = fractionalFlowGas(Math.max(s_gc, initialSg), rock, fluid);

    // Welge tangent: find shock front gas saturation
    let s_gf = s_gc;
    let maxSlope = 0;
    for (let s = s_gc + 5e-4; s <= sMax; s += 5e-4) {
        const fg = fractionalFlowGas(s, rock, fluid);
        const slope = (fg - fgInitial) / Math.max(1e-12, s - initialSg);
        if (slope > maxSlope && Number.isFinite(slope)) { maxSlope = slope; s_gf = s; }
    }

    const fg_shock = fractionalFlowGas(s_gf, rock, fluid);
    const dfg_shock = (fg_shock - fgInitial) / Math.max(1e-12, s_gf - initialSg);
    const pvi_bt = dfg_shock > 1e-12 ? 1.0 / dfg_shock : Infinity;

    // Binary-search Sg at outlet post-BT: 1/PVI = dfg/dSg|_{Sg_outlet}
    function findOutletSg(targetDfg: number): number {
        let lo = s_gf, hi = sMax;
        const dfgLo = dfg_dSg(lo, rock, fluid, 1e-4);
        const dfgHi = dfg_dSg(hi, rock, fluid, 1e-4);
        if (targetDfg >= dfgLo) return lo;
        if (targetDfg <= dfgHi) return hi;
        for (let iter = 0; iter < 60; iter++) {
            const mid = 0.5 * (lo + hi);
            if (dfg_dSg(mid, rock, fluid, 1e-4) > targetDfg) lo = mid; else hi = mid;
            if (hi - lo < 1e-7) break;
        }
        return 0.5 * (lo + hi);
    }

    // Oil initially in place (as fraction of pore volume): S_oi = 1 − S_wc
    const s_oi = 1 - rock.s_wc;

    const result: { pvi: number; rf: number }[] = [];
    for (let i = 0; i <= nPoints; i++) {
        const pvi = (i / nPoints) * pviMax;
        let sgAvg: number;
        if (pvi <= 0) {
            sgAvg = 0;
        } else if (pvi <= pvi_bt) {
            // Before breakthrough: Welge material balance
            // Average gas saturation = initial + PVI × (1 − fg_initial)
            sgAvg = initialSg + pvi * (1 - fgInitial);
        } else {
            // After breakthrough: Welge equation
            const s_g2 = findOutletSg(1.0 / pvi);
            sgAvg = s_g2 + pvi * (1 - fractionalFlowGas(s_g2, rock, fluid));
        }
        // Recovery = oil displaced / OOIP = (S_oi − S_o) / S_oi = S_g_avg / S_oi
        const rf = Math.max(0, Math.min(1, sgAvg / Math.max(1e-12, s_oi)));
        result.push({ pvi, rf });
    }
    return result;
}

// ── Gas-oil analytical production (time-based, for overlay on simulation) ──

export type GasOilAnalyticalPoint = {
    time: number;
    oilRate: number;
    gasRate: number;
    cumulativeOil: number;
};

export function calculateGasOilAnalyticalProduction(
    rock: GasOilRockProps,
    fluid: GasOilFluidProps,
    initialGasSaturation: number,
    timeHistory: number[],
    injectionRateSeries: number[],
    poreVolume: number,
): GasOilAnalyticalPoint[] {
    const sMax = 1 - rock.s_wc - rock.s_org;
    const s_gc = rock.s_gc;
    const initialSg = Math.max(0, Math.min(sMax, initialGasSaturation));
    const fgInitial = fractionalFlowGas(Math.max(s_gc, initialSg), rock, fluid);

    // Welge tangent
    let sg_f = s_gc;
    let max_slope = 0;
    for (let s = s_gc + 5e-4; s <= sMax; s += 5e-4) {
        const fg = fractionalFlowGas(s, rock, fluid);
        const slope = (fg - fgInitial) / Math.max(1e-12, s - initialSg);
        if (slope > max_slope) {
            max_slope = slope;
            sg_f = s;
        }
    }

    const fg_at_shock = fractionalFlowGas(sg_f, rock, fluid);
    const dfg_at_shock = (fg_at_shock - fgInitial) / Math.max(1e-12, sg_f - initialSg);
    const breakthroughPVI = dfg_at_shock > 1e-12 ? 1.0 / dfg_at_shock : Number.POSITIVE_INFINITY;

    const q0 = injectionRateSeries.find(rate => Number.isFinite(rate) && rate > 0) ?? 0;
    if (q0 <= 0) {
        return timeHistory.map(t => ({ time: t, oilRate: 0, gasRate: 0, cumulativeOil: 0 }));
    }

    function findOutletSg(target_dfg: number): number {
        let lo = sg_f;
        let hi = sMax;
        const dfg_lo = dfg_dSg(lo, rock, fluid, 1e-4);
        const dfg_hi = dfg_dSg(hi, rock, fluid, 1e-4);
        if (target_dfg >= dfg_lo) return lo;
        if (target_dfg <= dfg_hi) return hi;
        for (let iter = 0; iter < 50; iter++) {
            const mid = 0.5 * (lo + hi);
            const dfg_mid = dfg_dSg(mid, rock, fluid, 1e-4);
            if (dfg_mid > target_dfg) lo = mid;
            else hi = mid;
            if (hi - lo < 1e-6) break;
        }
        return 0.5 * (lo + hi);
    }

    const result: GasOilAnalyticalPoint[] = [];
    let cumulativeOil = 0;
    let cumulativePVI = 0;

    for (let i = 0; i < timeHistory.length; i++) {
        const t = timeHistory[i];
        const q = Number.isFinite(injectionRateSeries[i]) && injectionRateSeries[i] > 0
            ? injectionRateSeries[i] : q0;
        const dt = i > 0 ? Math.max(0, t - timeHistory[i - 1]) : Math.max(0, t);
        if (poreVolume > 0) cumulativePVI += (q * dt) / poreVolume;

        let oilRate: number;
        if (cumulativePVI <= breakthroughPVI) {
            // Before gas breakthrough: all injected gas displaces oil
            oilRate = q * (1 - fgInitial);
        } else {
            const target_dfg = cumulativePVI > 1e-12 ? 1.0 / cumulativePVI : dfg_at_shock;
            const s_g_at_outlet = findOutletSg(target_dfg);
            const fg_at_outlet = fractionalFlowGas(s_g_at_outlet, rock, fluid);
            oilRate = q * (1 - fg_at_outlet);
        }
        const boundedOilRate = Math.max(0, oilRate);
        const gasRate = Math.max(0, q - boundedOilRate);
        cumulativeOil += boundedOilRate * dt;

        result.push({ time: t, oilRate: boundedOilRate, gasRate, cumulativeOil });
    }
    return result;
}
