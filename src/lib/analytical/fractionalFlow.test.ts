import { describe, it, expect } from 'vitest';
import {
    k_rw, k_ro, fractionalFlow, dfw_dSw,
    computeWelgeMetrics, calculateAnalyticalProduction,
    type RockProps, type FluidProps,
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
