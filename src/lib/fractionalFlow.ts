/**
 * Pure-function implementations of Buckley-Leverett / fractional flow analytics.
 * Extracted from FractionalFlow.svelte for testability.
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

export type WelgeMetrics = {
    shockSw: number;
    breakthroughPvi: number;
    waterCutAtBreakthrough: number;
    initialSw: number;
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
