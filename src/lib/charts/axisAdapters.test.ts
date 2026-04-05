import { describe, expect, it } from 'vitest';
import {
    buildXAxisValues,
    getSweepZeroXAxisValue,
    mapPviSeriesToXAxis,
    interpolateXAxisAtTimes,
    requiresRunMappedAnalyticalXAxis,
    buildAnalyticalAxisWarning,
    toXYSeries,
    type DerivedRunSeries,
} from './axisAdapters';

// ─── Helpers ──────────────────────────────────────────────────────────────────

function makeDerived(override: Partial<DerivedRunSeries> = {}): DerivedRunSeries {
    const n = 5;
    const time = [0, 10, 20, 30, 40];
    const pvi  = [0, 0.5, 1.0, 1.5, 2.0];
    return {
        time,
        historyTime: time,
        oilRate: Array(n).fill(null),
        injectionRate: Array(n).fill(null),
        waterCut: Array(n).fill(null),
        gasCut: Array(n).fill(null),
        avgWaterSat: Array(n).fill(null),
        pressure: Array(n).fill(null),
        producerBhp: Array(n).fill(null),
        injectorBhp: Array(n).fill(null),
        recovery: Array(n).fill(null),
        cumulativeOil: Array(n).fill(null),
        cumulativeInjection: [0, 100, 200, 300, 400],
        cumulativeLiquid: [0, 80, 160, 240, 320],
        cumulativeGas: [0, 10, 20, 30, 40],
        p_z: Array(n).fill(null),
        pvi,
        pvp: [0, 0.4, 0.8, 1.2, 1.6],
        gor: Array(n).fill(null),
        producerBhpLimitedFraction: Array(n).fill(null),
        injectorBhpLimitedFraction: Array(n).fill(null),
        ...override,
    };
}

// ─── toXYSeries ───────────────────────────────────────────────────────────────

describe('toXYSeries', () => {
    it('zips finite x/y pairs', () => {
        const result = toXYSeries([1, 2, 3], [10, 20, 30]);
        expect(result).toEqual([
            { x: 1, y: 10 },
            { x: 2, y: 20 },
            { x: 3, y: 30 },
        ]);
    });

    it('skips entries where x is non-finite', () => {
        const result = toXYSeries([null, 2, NaN], [10, 20, 30]);
        expect(result).toEqual([{ x: 2, y: 20 }]);
    });

    it('sets y to null when y is non-finite', () => {
        const result = toXYSeries([1, 2], [null, NaN]);
        expect(result).toEqual([
            { x: 1, y: null },
            { x: 2, y: null },
        ]);
    });
});

// ─── buildXAxisValues ─────────────────────────────────────────────────────────

describe('buildXAxisValues', () => {
    const derived = makeDerived();

    it('returns time for "time" mode', () => {
        expect(buildXAxisValues(derived, 'time')).toEqual([...derived.time]);
    });

    it('returns pvi copy for "pvi" mode', () => {
        const result = buildXAxisValues(derived, 'pvi');
        expect(result).toEqual([...derived.pvi]);
        expect(result).not.toBe(derived.pvi); // must be a copy
    });

    it('returns pvp for "pvp" mode', () => {
        expect(buildXAxisValues(derived, 'pvp')).toEqual([...derived.pvp]);
    });

    it('returns cumulativeInjection for "cumInjection" mode', () => {
        expect(buildXAxisValues(derived, 'cumInjection')).toEqual([...derived.cumulativeInjection]);
    });

    it('returns cumulativeLiquid for "cumLiquid" mode', () => {
        expect(buildXAxisValues(derived, 'cumLiquid')).toEqual([...derived.cumulativeLiquid]);
    });

    it('returns cumulativeGas for "cumGas" mode', () => {
        expect(buildXAxisValues(derived, 'cumGas')).toEqual([...derived.cumulativeGas]);
    });

    it('returns log10(time) for "logTime" mode, null for t=0', () => {
        const result = buildXAxisValues(derived, 'logTime');
        expect(result[0]).toBeNull(); // log10(0) → null
        expect(result[1]).toBeCloseTo(Math.log10(10));
        expect(result[4]).toBeCloseTo(Math.log10(40));
    });

    it('returns time/tau for "tD" mode when tau is valid', () => {
        const result = buildXAxisValues(derived, 'tD', 20);
        expect(result).toEqual([0, 0.5, 1.0, 1.5, 2.0]);
    });

    it('falls back to time when tD mode but tau is null', () => {
        expect(buildXAxisValues(derived, 'tD', null)).toEqual([...derived.time]);
    });

    it('falls back to time when tD mode but tau is zero', () => {
        expect(buildXAxisValues(derived, 'tD', 0)).toEqual([...derived.time]);
    });
});

// ─── getSweepZeroXAxisValue ───────────────────────────────────────────────────

describe('getSweepZeroXAxisValue', () => {
    it('returns null for logTime (log(0) is undefined)', () => {
        expect(getSweepZeroXAxisValue('logTime')).toBeNull();
    });

    it('returns 0 for all other modes', () => {
        for (const mode of ['time', 'pvi', 'pvp', 'tD', 'cumInjection', 'cumLiquid'] as const) {
            expect(getSweepZeroXAxisValue(mode)).toBe(0);
        }
    });
});

// ─── mapPviSeriesToXAxis ──────────────────────────────────────────────────────

describe('mapPviSeriesToXAxis', () => {
    const derived = makeDerived();

    it('returns a copy of pviValues when mode is "pvi"', () => {
        const pviValues = [0, 0.5, 1.0];
        const result = mapPviSeriesToXAxis(pviValues, derived, 'pvi', null);
        expect(result).toEqual(pviValues);
        expect(result).not.toBe(pviValues);
    });

    it('remaps PVI values onto the time axis via interpolation', () => {
        // derived.pvi = [0, 0.5, 1.0, 1.5, 2.0]
        // derived.time = [0, 10, 20, 30, 40]
        // PVI 0.5 → time 10 (exact), PVI 0.75 → time 15 (interpolated)
        const result = mapPviSeriesToXAxis([0, 0.5, 0.75, 1.0], derived, 'time', null);
        expect(result[0]).toBe(0);
        expect(result[1]).toBeCloseTo(10);
        expect(result[2]).toBeCloseTo(15);
        expect(result[3]).toBeCloseTo(20);
    });

    it('returns null for non-finite pvi values', () => {
        const result = mapPviSeriesToXAxis([null, 0.5], derived, 'time', null);
        expect(result[0]).toBeNull();
        expect(result[1]).toBeCloseTo(10);
    });

    it('clamps pvi ≤ 1e-12 to getSweepZeroXAxisValue', () => {
        const result = mapPviSeriesToXAxis([0, 1e-13], derived, 'time', null);
        expect(result[0]).toBe(0);
        expect(result[1]).toBe(0);
    });

    it('extrapolates with the last known value when pvi exceeds the domain', () => {
        // pvi 3.0 exceeds derived max of 2.0 → returns last mapped value
        const result = mapPviSeriesToXAxis([3.0], derived, 'time', null);
        expect(result[0]).toBeCloseTo(40); // last time value
    });
});

// ─── interpolateXAxisAtTimes ──────────────────────────────────────────────────

describe('interpolateXAxisAtTimes', () => {
    const sourceTimes: Array<number | null> = [0, 10, 20, 30];
    const sourceXAxis: Array<number | null> = [0, 100, 200, 300];

    it('interpolates at exact source times', () => {
        const result = interpolateXAxisAtTimes(sourceTimes, sourceXAxis, [0, 10, 20, 30]);
        expect(result).toEqual([0, 100, 200, 300]);
    });

    it('linearly interpolates between source points', () => {
        const result = interpolateXAxisAtTimes(sourceTimes, sourceXAxis, [5, 15, 25]);
        expect(result[0]).toBeCloseTo(50);
        expect(result[1]).toBeCloseTo(150);
        expect(result[2]).toBeCloseTo(250);
    });

    it('clamps to first value when target is before the domain', () => {
        const result = interpolateXAxisAtTimes(sourceTimes, sourceXAxis, [-5]);
        expect(result[0]).toBe(0);
    });

    it('clamps to last value when target is beyond the domain', () => {
        const result = interpolateXAxisAtTimes(sourceTimes, sourceXAxis, [50]);
        expect(result[0]).toBe(300);
    });

    it('returns null for non-finite target times', () => {
        const result = interpolateXAxisAtTimes(sourceTimes, sourceXAxis, [null, NaN]);
        expect(result[0]).toBeNull();
        expect(result[1]).toBeNull();
    });
});

// ─── requiresRunMappedAnalyticalXAxis ────────────────────────────────────────

describe('requiresRunMappedAnalyticalXAxis', () => {
    it('returns false when method is buckley-leverett and axis is pvi', () => {
        expect(requiresRunMappedAnalyticalXAxis('buckley-leverett', 'pvi')).toBe(false);
    });

    it('returns true when method is buckley-leverett and axis is not pvi', () => {
        expect(requiresRunMappedAnalyticalXAxis('buckley-leverett', 'time')).toBe(true);
        expect(requiresRunMappedAnalyticalXAxis('buckley-leverett', 'cumInjection')).toBe(true);
        expect(requiresRunMappedAnalyticalXAxis('buckley-leverett', 'logTime')).toBe(true);
    });

    it('returns true for waterflood alias on non-pvi axis', () => {
        expect(requiresRunMappedAnalyticalXAxis('waterflood', 'time')).toBe(true);
    });

    it('returns true for gas-oil-bl on non-pvi axis', () => {
        expect(requiresRunMappedAnalyticalXAxis('gas-oil-bl', 'time')).toBe(true);
    });

    it('returns false for depletion on any axis', () => {
        expect(requiresRunMappedAnalyticalXAxis('depletion', 'time')).toBe(false);
        expect(requiresRunMappedAnalyticalXAxis('depletion', 'pvi')).toBe(false);
        expect(requiresRunMappedAnalyticalXAxis('depletion', 'logTime')).toBe(false);
    });

    it('returns false for null/undefined method', () => {
        expect(requiresRunMappedAnalyticalXAxis(null, 'time')).toBe(false);
        expect(requiresRunMappedAnalyticalXAxis(undefined, 'pvi')).toBe(false);
    });
});

// ─── buildAnalyticalAxisWarning ───────────────────────────────────────────────

describe('buildAnalyticalAxisWarning', () => {
    it('returns null when both flags are false', () => {
        expect(buildAnalyticalAxisWarning({
            usesRunMappedAnalyticalXAxis: false,
            hidesPendingAnalyticalWithoutMapping: false,
        })).toBeNull();
    });

    it('includes remapping message when usesRunMappedAnalyticalXAxis is true', () => {
        const warning = buildAnalyticalAxisWarning({
            usesRunMappedAnalyticalXAxis: true,
            hidesPendingAnalyticalWithoutMapping: false,
        });
        expect(warning).toContain('remapped');
    });

    it('includes hiding message when hidesPendingAnalyticalWithoutMapping is true', () => {
        const warning = buildAnalyticalAxisWarning({
            usesRunMappedAnalyticalXAxis: false,
            hidesPendingAnalyticalWithoutMapping: true,
        });
        expect(warning).toContain('hidden');
    });

    it('joins both messages when both flags are true', () => {
        const warning = buildAnalyticalAxisWarning({
            usesRunMappedAnalyticalXAxis: true,
            hidesPendingAnalyticalWithoutMapping: true,
        });
        expect(warning).toContain('remapped');
        expect(warning).toContain('hidden');
    });
});
