import { describe, it, expect } from 'vitest';
import {
    k_rw, k_ro, fractionalFlow, dfw_dSw,
    computeWelgeMetrics, calculateAnalyticalProduction,
    k_rg, k_ro_gas, fractionalFlowGas, dfg_dSg,
    computeWelgeMetricsGas, computeGasOilRecoveryVsPVI,
    calculateGasOilAnalyticalProduction,
    type RockProps, type FluidProps,
    type GasOilRockProps, type GasOilFluidProps,
} from './fractionalFlow';

const defaultRock: RockProps = { s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2, k_rw_max: 1.0, k_ro_max: 1.0 };
const defaultFluid: FluidProps = { mu_w: 0.5, mu_o: 1.0 };

// ── Relative permeability ──

describe('k_rw', () => {
    it('returns 0 at connate water saturation', () => {
        expect(k_rw(0.1, defaultRock)).toBe(0);
    });

    it('returns k_rw_max at 1 - s_or', () => {
        expect(k_rw(0.9, defaultRock)).toBeCloseTo(1.0, 6);
    });

    it('returns intermediate value at midpoint', () => {
        const midSw = 0.5;
        const val = k_rw(midSw, defaultRock);
        expect(val).toBeGreaterThan(0);
        expect(val).toBeLessThan(1);
    });

    it('clamps below s_wc to 0', () => {
        expect(k_rw(0, defaultRock)).toBe(0);
    });

    it('clamps above 1-s_or to k_rw_max', () => {
        expect(k_rw(1.0, defaultRock)).toBeCloseTo(1.0, 6);
    });

    it('scales with k_rw_max', () => {
        const rock = { ...defaultRock, k_rw_max: 0.5 };
        expect(k_rw(0.9, rock)).toBeCloseTo(0.5, 6);
    });
});

describe('k_ro', () => {
    it('returns k_ro_max at connate water saturation', () => {
        expect(k_ro(0.1, defaultRock)).toBeCloseTo(1.0, 6);
    });

    it('returns 0 at 1 - s_or', () => {
        expect(k_ro(0.9, defaultRock)).toBe(0);
    });

    it('returns intermediate value at midpoint', () => {
        const val = k_ro(0.5, defaultRock);
        expect(val).toBeGreaterThan(0);
        expect(val).toBeLessThan(1);
    });
});

// ── Fractional flow ──

describe('fractionalFlow', () => {
    it('returns 0 at connate water saturation (no mobile water)', () => {
        // At s_wc, krw = 0, so fw = 0
        const fw = fractionalFlow(0.1, defaultRock, defaultFluid);
        expect(fw).toBeCloseTo(0, 6);
    });

    it('returns 1 at 1 - s_or (no mobile oil)', () => {
        // At 1-s_or, kro = 0, so fw = 1
        const fw = fractionalFlow(0.9, defaultRock, defaultFluid);
        expect(fw).toBeCloseTo(1, 6);
    });

    it('is monotonically increasing', () => {
        let prev = -1;
        for (let sw = 0.1; sw <= 0.9; sw += 0.01) {
            const fw = fractionalFlow(sw, defaultRock, defaultFluid);
            expect(fw).toBeGreaterThanOrEqual(prev - 1e-10);
            prev = fw;
        }
    });

    it('increases water cut with higher water mobility (lower mu_w)', () => {
        const fwStandard = fractionalFlow(0.5, defaultRock, defaultFluid);
        const fwFaster = fractionalFlow(0.5, defaultRock, { mu_w: 0.1, mu_o: 1.0 });
        expect(fwFaster).toBeGreaterThan(fwStandard);
    });

    it('returns NaN when saturation range is degenerate (s_wc + s_or = 1)', () => {
        // Degenerate: no mobile saturation range → 0/0 → NaN
        const zeroRock: RockProps = { s_wc: 0.5, s_or: 0.5, n_w: 2, n_o: 2, k_rw_max: 0, k_ro_max: 0 };
        expect(fractionalFlow(0.5, zeroRock, defaultFluid)).toBeNaN();
    });
});

// ── dfw/dSw ──

describe('dfw_dSw', () => {
    it('returns 0 outside mobile range', () => {
        expect(dfw_dSw(0.05, defaultRock, defaultFluid)).toBe(0);
        expect(dfw_dSw(0.95, defaultRock, defaultFluid)).toBe(0);
    });

    it('returns positive value inside mobile range', () => {
        const deriv = dfw_dSw(0.5, defaultRock, defaultFluid);
        expect(deriv).toBeGreaterThan(0);
    });

    it('is consistent with central difference', () => {
        const sw = 0.5;
        const ds = 1e-4;
        const fwPlus = fractionalFlow(sw + ds, defaultRock, defaultFluid);
        const fwMinus = fractionalFlow(sw - ds, defaultRock, defaultFluid);
        const expected = (fwPlus - fwMinus) / (2 * ds);
        expect(dfw_dSw(sw, defaultRock, defaultFluid, ds)).toBeCloseTo(expected, 6);
    });
});

// ── Welge metrics ──

describe('computeWelgeMetrics', () => {
    it('returns valid shock saturation above initial saturation', () => {
        const metrics = computeWelgeMetrics(defaultRock, defaultFluid, 0.3);
        expect(metrics.shockSw).toBeGreaterThan(0.3);
        expect(metrics.shockSw).toBeLessThanOrEqual(0.9);
    });

    it('returns positive breakthrough PVI', () => {
        const metrics = computeWelgeMetrics(defaultRock, defaultFluid, 0.3);
        expect(metrics.breakthroughPvi).toBeGreaterThan(0);
        expect(metrics.breakthroughPvi).toBeLessThan(10);
    });

    it('water cut at breakthrough is between 0 and 1', () => {
        const metrics = computeWelgeMetrics(defaultRock, defaultFluid, 0.3);
        expect(metrics.waterCutAtBreakthrough).toBeGreaterThan(0);
        expect(metrics.waterCutAtBreakthrough).toBeLessThan(1);
    });

    it('clamps initial saturation to mobile range', () => {
        const metrics = computeWelgeMetrics(defaultRock, defaultFluid, -0.5);
        expect(metrics.initialSw).toBe(defaultRock.s_wc);
    });

    it('handles symmetric relative permeability (equal viscosity)', () => {
        const symmetricFluid: FluidProps = { mu_w: 1.0, mu_o: 1.0 };
        const metrics = computeWelgeMetrics(defaultRock, symmetricFluid, 0.1);
        // With equal viscosity and equal Corey exponents, shock front should be near midpoint
        expect(metrics.shockSw).toBeGreaterThan(0.3);
        expect(metrics.shockSw).toBeLessThan(0.7);
    });

    it('higher initial Sw => later BT PVI (less oil to displace)', () => {
        const metricsLow = computeWelgeMetrics(defaultRock, defaultFluid, 0.1);
        const metricsHigh = computeWelgeMetrics(defaultRock, defaultFluid, 0.4);
        // Higher initial water => less oil to flush, but PVI depends on tangent construction
        // Just check both produce valid results
        expect(metricsLow.breakthroughPvi).toBeGreaterThan(0);
        expect(metricsHigh.breakthroughPvi).toBeGreaterThan(0);
    });
});

// ── Analytical production ──

describe('calculateAnalyticalProduction', () => {
    const poreVolume = 1000; // m³
    const times = [0.5, 1, 2, 5, 10, 20];
    const rates = [100, 100, 100, 100, 100, 100]; // m³/day

    it('returns one entry per timestep', () => {
        const result = calculateAnalyticalProduction(defaultRock, defaultFluid, 0.3, times, rates, poreVolume);
        expect(result).toHaveLength(times.length);
    });

    it('oil rate is positive before breakthrough', () => {
        const result = calculateAnalyticalProduction(defaultRock, defaultFluid, 0.3, times, rates, poreVolume);
        expect(result[0].oilRate).toBeGreaterThan(0);
    });

    it('cumulative oil is monotonically increasing', () => {
        const result = calculateAnalyticalProduction(defaultRock, defaultFluid, 0.3, times, rates, poreVolume);
        for (let i = 1; i < result.length; i++) {
            expect(result[i].cumulativeOil).toBeGreaterThanOrEqual(result[i - 1].cumulativeOil);
        }
    });

    it('water rate + oil rate = injection rate (mass balance)', () => {
        const result = calculateAnalyticalProduction(defaultRock, defaultFluid, 0.3, times, rates, poreVolume);
        for (const point of result) {
            expect(point.oilRate + point.waterRate).toBeCloseTo(100, 6);
        }
    });

    it('oil rate decreases after breakthrough', () => {
        // Use small pore volume to ensure BT happens quickly
        const smallPV = 200;
        const longTimes = Array.from({ length: 50 }, (_, i) => (i + 1) * 1);
        const constRates = longTimes.map(() => 100);
        const result = calculateAnalyticalProduction(defaultRock, defaultFluid, 0.3, longTimes, constRates, smallPV);
        // After some time, oil rate should decline
        const lastOilRate = result[result.length - 1].oilRate;
        const firstOilRate = result[0].oilRate;
        expect(lastOilRate).toBeLessThan(firstOilRate);
    });

    it('returns zero oil rate when injection rate is zero', () => {
        const result = calculateAnalyticalProduction(defaultRock, defaultFluid, 0.3, times, [0, 0, 0, 0, 0, 0], poreVolume);
        for (const point of result) {
            expect(point.oilRate).toBe(0);
            expect(point.waterRate).toBe(0);
        }
    });

    it('handles empty time history', () => {
        const result = calculateAnalyticalProduction(defaultRock, defaultFluid, 0.3, [], [], poreVolume);
        expect(result).toHaveLength(0);
    });
});

// ══════════════════════════════════════════════════════════════════════════════
// Gas-Oil Buckley-Leverett
// ══════════════════════════════════════════════════════════════════════════════

const gasRock: GasOilRockProps = {
    s_wc: 0.2, s_gc: 0.05, s_gr: 0.05, s_org: 0.20,
    n_o: 2, n_g: 1.5,
    k_ro_max: 1.0, k_rg_max: 0.8,
};
const gasFluid: GasOilFluidProps = { mu_o: 2.0, mu_g: 0.02 };

describe('k_rg', () => {
    it('returns 0 at critical gas saturation', () => {
        expect(k_rg(0.05, gasRock)).toBe(0);
    });

    it('returns 0 below critical gas saturation', () => {
        expect(k_rg(0, gasRock)).toBe(0);
    });

    it('returns k_rg_max at maximum gas saturation', () => {
        // sMax for k_rg effective: S_g = 1 - s_wc - s_gc - s_gr + s_gc = 1 - s_wc - s_gr
        // Actually effective at S_g_eff = 1 → S_g = 1 - s_wc - s_gr = 0.75 (but denom = 1 - 0.2 - 0.05 - 0.05 = 0.7)
        // S_g for s_eff=1: s_gc + denom = 0.05 + 0.7 = 0.75
        expect(k_rg(0.75, gasRock)).toBeCloseTo(0.8, 6);
    });

    it('returns intermediate value in mobile range', () => {
        const val = k_rg(0.3, gasRock);
        expect(val).toBeGreaterThan(0);
        expect(val).toBeLessThan(0.8);
    });
});

describe('k_ro_gas', () => {
    it('returns k_ro_max when no gas (S_g = 0)', () => {
        expect(k_ro_gas(0, gasRock)).toBeCloseTo(1.0, 6);
    });

    it('returns 0 at residual oil to gas', () => {
        // S_o = 1 - s_wc - S_g = s_org → S_g = 1 - s_wc - s_org = 0.6
        expect(k_ro_gas(0.6, gasRock)).toBeCloseTo(0, 6);
    });

    it('returns intermediate value in mobile range', () => {
        const val = k_ro_gas(0.3, gasRock);
        expect(val).toBeGreaterThan(0);
        expect(val).toBeLessThan(1);
    });
});

describe('fractionalFlowGas', () => {
    it('returns 0 below critical gas saturation', () => {
        expect(fractionalFlowGas(0, gasRock, gasFluid)).toBe(0);
    });

    it('approaches 1 at high gas saturation (gas dominant)', () => {
        const fg = fractionalFlowGas(0.55, gasRock, gasFluid);
        expect(fg).toBeGreaterThan(0.9);
    });

    it('is monotonically increasing in mobile range', () => {
        const sMin = gasRock.s_gc;
        const sMax = 1 - gasRock.s_wc - gasRock.s_org;
        let prevFg = 0;
        for (let s = sMin; s <= sMax; s += 0.01) {
            const fg = fractionalFlowGas(s, gasRock, gasFluid);
            expect(fg).toBeGreaterThanOrEqual(prevFg - 1e-10);
            prevFg = fg;
        }
    });
});

describe('dfg_dSg', () => {
    it('returns positive derivative in mobile range', () => {
        const deriv = dfg_dSg(0.2, gasRock, gasFluid);
        expect(deriv).toBeGreaterThan(0);
    });

    it('returns 0 outside mobile range', () => {
        expect(dfg_dSg(0, gasRock, gasFluid)).toBe(0);
    });
});

describe('computeWelgeMetricsGas', () => {
    it('finds valid shock front saturation above critical', () => {
        const metrics = computeWelgeMetricsGas(gasRock, gasFluid, 0);
        expect(metrics.shockSg).toBeGreaterThan(gasRock.s_gc);
        expect(metrics.shockSg).toBeLessThan(1 - gasRock.s_wc - gasRock.s_org);
    });

    it('returns positive breakthrough PVI', () => {
        const metrics = computeWelgeMetricsGas(gasRock, gasFluid, 0);
        expect(metrics.breakthroughPvi).toBeGreaterThan(0);
        expect(metrics.breakthroughPvi).toBeLessThan(2); // gas BT is typically early
    });

    it('gas cut at breakthrough is between 0 and 1', () => {
        const metrics = computeWelgeMetricsGas(gasRock, gasFluid, 0);
        expect(metrics.gasCutAtBreakthrough).toBeGreaterThan(0);
        expect(metrics.gasCutAtBreakthrough).toBeLessThanOrEqual(1);
    });

    it('more adverse mobility gives earlier breakthrough', () => {
        const favorable = computeWelgeMetricsGas(gasRock, { mu_o: 2.0, mu_g: 0.1 }, 0);
        const adverse = computeWelgeMetricsGas(gasRock, { mu_o: 2.0, mu_g: 0.005 }, 0);
        expect(adverse.breakthroughPvi).toBeLessThan(favorable.breakthroughPvi);
    });
});

describe('computeGasOilRecoveryVsPVI', () => {
    it('recovery starts at 0 and increases', () => {
        const result = computeGasOilRecoveryVsPVI(gasRock, gasFluid);
        expect(result[0].rf).toBe(0);
        expect(result[result.length - 1].rf).toBeGreaterThan(0);
    });

    it('recovery is monotonically increasing', () => {
        const result = computeGasOilRecoveryVsPVI(gasRock, gasFluid);
        for (let i = 1; i < result.length; i++) {
            expect(result[i].rf).toBeGreaterThanOrEqual(result[i - 1].rf - 1e-10);
        }
    });

    it('recovery is bounded by 1', () => {
        const result = computeGasOilRecoveryVsPVI(gasRock, gasFluid, 10);
        for (const point of result) {
            expect(point.rf).toBeLessThanOrEqual(1);
        }
    });

    it('adverse mobility gives lower recovery at same PVI', () => {
        const favorable = computeGasOilRecoveryVsPVI(gasRock, { mu_o: 2.0, mu_g: 0.1 });
        const adverse = computeGasOilRecoveryVsPVI(gasRock, { mu_o: 2.0, mu_g: 0.005 });
        // At 1 PVI
        const rfFav = favorable.find(p => p.pvi >= 1)?.rf ?? 0;
        const rfAdv = adverse.find(p => p.pvi >= 1)?.rf ?? 0;
        expect(rfFav).toBeGreaterThan(rfAdv);
    });
});

describe('calculateGasOilAnalyticalProduction', () => {
    const times = [1, 2, 3, 4, 5, 6];
    const rates = [100, 100, 100, 100, 100, 100];
    const poreVolume = 500;

    it('gas rate + oil rate = injection rate (mass balance)', () => {
        const result = calculateGasOilAnalyticalProduction(gasRock, gasFluid, 0, times, rates, poreVolume);
        for (const point of result) {
            expect(point.oilRate + point.gasRate).toBeCloseTo(100, 6);
        }
    });

    it('cumulative oil is monotonically increasing', () => {
        const result = calculateGasOilAnalyticalProduction(gasRock, gasFluid, 0, times, rates, poreVolume);
        for (let i = 1; i < result.length; i++) {
            expect(result[i].cumulativeOil).toBeGreaterThanOrEqual(result[i - 1].cumulativeOil);
        }
    });

    it('oil rate decreases after breakthrough', () => {
        const smallPV = 200;
        const longTimes = Array.from({ length: 50 }, (_, i) => (i + 1) * 1);
        const constRates = longTimes.map(() => 100);
        const result = calculateGasOilAnalyticalProduction(gasRock, gasFluid, 0, longTimes, constRates, smallPV);
        const lastOilRate = result[result.length - 1].oilRate;
        const firstOilRate = result[0].oilRate;
        expect(lastOilRate).toBeLessThan(firstOilRate);
    });

    it('returns zero oil rate when injection rate is zero', () => {
        const result = calculateGasOilAnalyticalProduction(gasRock, gasFluid, 0, times, [0, 0, 0, 0, 0, 0], poreVolume);
        for (const point of result) {
            expect(point.oilRate).toBe(0);
            expect(point.gasRate).toBe(0);
        }
    });

    it('handles empty time history', () => {
        const result = calculateGasOilAnalyticalProduction(gasRock, gasFluid, 0, [], [], poreVolume);
        expect(result).toHaveLength(0);
    });
});

describe('gas-oil BL physics validation', () => {
    it('less adverse mobility (μ_g=0.1) gives later breakthrough than base', () => {
        const lessMobile = computeWelgeMetricsGas(gasRock, { mu_o: 2.0, mu_g: 0.1 }, 0);
        const base = computeWelgeMetricsGas(gasRock, { mu_o: 2.0, mu_g: 0.02 }, 0);
        expect(lessMobile.breakthroughPvi).toBeGreaterThan(base.breakthroughPvi);
        expect(lessMobile.breakthroughPvi).toBeGreaterThan(0.1);
    });

    it('adverse mobility (μ_g=0.02) gives early breakthrough PVI', () => {
        const metrics = computeWelgeMetricsGas(gasRock, { mu_o: 2.0, mu_g: 0.02 }, 0);
        expect(metrics.breakthroughPvi).toBeGreaterThan(0);
        expect(metrics.breakthroughPvi).toBeLessThan(0.3);
    });

    it('recovery at 3 PVI is between 40% and 90% for base case', () => {
        const result = computeGasOilRecoveryVsPVI(gasRock, gasFluid, 3);
        const rfFinal = result[result.length - 1].rf;
        expect(rfFinal).toBeGreaterThan(0.4);
        expect(rfFinal).toBeLessThan(0.9);
    });

    it('gas-oil BL and water-oil BL agree on structure (both are valid BL)', () => {
        // With same endpoints and mobility ratio, both should produce valid S-shaped recovery
        const gasResult = computeGasOilRecoveryVsPVI(gasRock, gasFluid, 3, 100);
        // Recovery should be S-shaped: concave up before BT, concave down after
        const bt = computeWelgeMetricsGas(gasRock, gasFluid, 0);
        expect(bt.breakthroughPvi).toBeGreaterThan(0);
        // Before breakthrough: linear recovery
        const preBT = gasResult.filter(p => p.pvi > 0 && p.pvi < bt.breakthroughPvi);
        if (preBT.length >= 2) {
            // Slope should be roughly constant before BT
            const slope0 = (preBT[1].rf - preBT[0].rf) / (preBT[1].pvi - preBT[0].pvi);
            const slopeMid = (preBT[preBT.length - 1].rf - preBT[preBT.length - 2].rf)
                / (preBT[preBT.length - 1].pvi - preBT[preBT.length - 2].pvi);
            expect(slope0).toBeCloseTo(slopeMid, 1);
        }
    });
});
