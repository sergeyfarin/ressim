import { describe, expect, it } from 'vitest';
import { integrateRunSeries } from './runSeries';
import type { RateHistoryPoint } from './simulator-types';

const history = [
    { time: 10, total_production_oil: 40, total_production_liquid: 50, total_production_gas: 400, total_injection: 60 },
    { time: 20, total_production_oil: 30, total_production_liquid: 45, total_production_gas: 600, total_injection: 60 },
    { time: 30, total_production_oil: 20, total_production_liquid: 44, total_production_gas: 900, total_injection: 60 },
] as unknown as RateHistoryPoint[];

describe('integrateRunSeries', () => {
    it('integrates the first interval from t = 0', () => {
        const series = integrateRunSeries(history);
        expect(series.oil[0]).toBeCloseTo(40 * 10, 9);
        expect(series.injection[0]).toBeCloseTo(60 * 10, 9);
    });

    it('applies the rectangle rule, not a trapezoid', () => {
        // Deliberately pinned. The engine reports a step-*average* rate over the
        // interval ending at `time`, so rate × dt is exact. Measured on
        // `gas_drive`: the rectangle rule closes the Havlena-Odeh balance to
        // N_mbe/N_volumetric = 1.0000, a trapezoid gives 1.124. A "smoother"
        // integration here is a regression, not an improvement.
        const series = integrateRunSeries(history);
        expect(series.oil.at(-1)).toBeCloseTo((40 + 30 + 20) * 10, 9);
        const trapezoid = ((40 + 40) / 2 + (40 + 30) / 2 + (30 + 20) / 2) * 10;
        expect(series.oil.at(-1)).not.toBeCloseTo(trapezoid, 6);
    });

    it('derives water as liquid minus oil, clamped at zero', () => {
        const series = integrateRunSeries(history);
        expect(series.water.at(-1)).toBeCloseTo((10 + 15 + 24) * 10, 9);
        const oilExceedsLiquid = integrateRunSeries([
            { time: 10, total_production_oil: 50, total_production_liquid: 40 },
        ] as unknown as RateHistoryPoint[]);
        expect(oilExceedsLiquid.water[0]).toBe(0);
    });

    it('integrates every quantity over the same intervals', () => {
        const series = integrateRunSeries(history);
        for (const key of ['oil', 'gas', 'liquid', 'water', 'injection'] as const) {
            expect(series[key], key).toHaveLength(history.length);
        }
        expect(series.time).toEqual([10, 20, 30]);
    });

    it('is monotone and returns empty series for an empty history', () => {
        const series = integrateRunSeries(history);
        for (const key of ['oil', 'gas', 'liquid', 'water', 'injection'] as const) {
            for (let index = 1; index < series[key].length; index += 1) {
                expect(series[key][index]).toBeGreaterThanOrEqual(series[key][index - 1]);
            }
        }
        expect(integrateRunSeries([]).oil).toEqual([]);
    });
});
