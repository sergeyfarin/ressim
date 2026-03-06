import { describe, expect, it } from 'vitest';

import {
    calculateDepletionAnalyticalProduction,
    emptyDepletionAnalyticalResult,
} from './depletionAnalytical';

describe('depletionAnalytical', () => {
    it('returns an empty result when required inputs are missing', () => {
        expect(
            calculateDepletionAnalyticalProduction({
                reservoir: null,
                timeHistory: [],
                initialSaturation: 0.2,
                nz: 1,
                permMode: 'uniform',
                uniformPermX: 100,
                uniformPermY: 100,
                layerPermsX: [],
                layerPermsY: [],
                cellDx: 10,
                cellDy: 10,
                cellDz: 10,
                wellRadius: 0.1,
                wellSkin: 0,
                muO: 1,
                sWc: 0.1,
                sOr: 0.1,
                nO: 2,
                c_o: 1e-5,
                c_w: 3e-6,
                cRock: 1e-6,
                initialPressure: 300,
                producerBhp: 100,
                depletionRateScale: 1,
            }),
        ).toEqual(emptyDepletionAnalyticalResult());
    });

    it('matches the explicit square-case q0 and tau calculation', () => {
        const result = calculateDepletionAnalyticalProduction({
            reservoir: { length: 100, area: 1000, porosity: 0.2 },
            timeHistory: [0, 10, 20],
            initialSaturation: 0.2,
            nz: 1,
            permMode: 'uniform',
            uniformPermX: 100,
            uniformPermY: 100,
            layerPermsX: [],
            layerPermsY: [],
            cellDx: 10,
            cellDy: 10,
            cellDz: 10,
            wellRadius: 0.1,
            wellSkin: 0,
            muO: 1,
            sWc: 0.1,
            sOr: 0.1,
            nO: 2,
            c_o: 1e-5,
            c_w: 3e-6,
            cRock: 1e-6,
            initialPressure: 300,
            producerBhp: 100,
            depletionRateScale: 1,
        });

        const expectedFactor = 9.8692e-16 * 1e3 * 1e5 * 86400;
        const eulerGamma = 0.5772156649;
        const shapeFactor = 30.8828;
        const drainageArea = 100 * 100;
        const kroAtInitialSw = 0.875 ** 2;
        const denominator =
            0.5 * Math.log((4 * drainageArea) / (shapeFactor * Math.exp(2 * eulerGamma) * 0.1 * 0.1));
        const expectedJ =
            (expectedFactor * 2 * Math.PI * 100 * 10 * kroAtInitialSw) /
            denominator;
        const expectedCt = 0.8 * 1e-5 + 0.2 * 3e-6 + 1e-6;
        const expectedTau = (100 * 1000 * 0.2 * expectedCt) / expectedJ;
        const expectedQ0 = expectedJ * (300 - 100);

        expect(result.meta.shapeFactor).toBeCloseTo(shapeFactor, 6);
        expect(result.meta.q0 ?? 0).toBeCloseTo(expectedQ0, 9);
        expect(result.meta.tau ?? 0).toBeCloseTo(expectedTau, 9);
        expect(result.production[0].oilRate).toBeCloseTo(expectedQ0, 9);
        expect(result.production[0].avgPressure).toBeCloseTo(300, 9);
    });

    it('keeps tau fixed while depletion rate scale changes q0 and cumulative oil', () => {
        const baseParams = {
            reservoir: { length: 100, area: 1000, porosity: 0.2 },
            timeHistory: [0, 5, 10],
            initialSaturation: 0.2,
            nz: 1,
            permMode: 'uniform',
            uniformPermX: 100,
            uniformPermY: 100,
            layerPermsX: [],
            layerPermsY: [],
            cellDx: 10,
            cellDy: 10,
            cellDz: 10,
            wellRadius: 0.1,
            wellSkin: 0,
            muO: 1,
            sWc: 0.1,
            sOr: 0.1,
            nO: 2,
            c_o: 1e-5,
            c_w: 3e-6,
            cRock: 1e-6,
            initialPressure: 300,
            producerBhp: 100,
        };

        const unscaled = calculateDepletionAnalyticalProduction({
            ...baseParams,
            depletionRateScale: 1,
        });
        const scaled = calculateDepletionAnalyticalProduction({
            ...baseParams,
            depletionRateScale: 2,
        });

        expect(scaled.meta.tau).toBeCloseTo(unscaled.meta.tau ?? 0, 12);
        expect((scaled.meta.q0 ?? 0) / (unscaled.meta.q0 ?? 1)).toBeCloseTo(2, 12);
        expect(
            scaled.production[scaled.production.length - 1].cumulativeOil /
                unscaled.production[unscaled.production.length - 1].cumulativeOil,
        ).toBeCloseTo(2, 12);
    });
});