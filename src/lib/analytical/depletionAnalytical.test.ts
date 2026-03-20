import { describe, expect, it } from 'vitest';

import {
    calculateDepletionAnalyticalProduction,
    computeShapeFactor,
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

    it('center and corner well produce divergent analytical curves on a square grid', () => {
        // 21×21 grid matching dep_pss scenario: center (10,10) vs corner (0,0)
        const sharedParams = {
            reservoir: { length: 420, area: 420 * 10, porosity: 0.2 },
            timeHistory: [0, 1, 5, 10, 25, 50],
            initialSaturation: 0.1,
            nz: 1,
            permMode: 'uniform' as const,
            uniformPermX: 50,
            uniformPermY: 50,
            layerPermsX: [] as number[],
            layerPermsY: [] as number[],
            cellDx: 20,
            cellDy: 20,
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
            nx: 21,
            ny: 21,
        };

        const center = calculateDepletionAnalyticalProduction({
            ...sharedParams,
            producerI: 10,
            producerJ: 10,
        });
        const corner = calculateDepletionAnalyticalProduction({
            ...sharedParams,
            producerI: 0,
            producerJ: 0,
        });

        // Shape factors must match tabulated Dietz values
        expect(center.meta.shapeFactor).toBeCloseTo(30.8828, 2);
        expect(corner.meta.shapeFactor).toBeCloseTo(0.5598, 2);

        // ~55× shape factor ratio produces materially different initial rates
        const q0Center = center.meta.q0 ?? 0;
        const q0Corner = corner.meta.q0 ?? 0;
        expect(q0Center).toBeGreaterThan(0);
        expect(q0Corner).toBeGreaterThan(0);
        expect(q0Center).toBeGreaterThan(q0Corner);

        // Corner well has a smaller PI → lower q0 and longer tau
        const tauCenter = center.meta.tau ?? 0;
        const tauCorner = corner.meta.tau ?? 0;
        expect(tauCorner).toBeGreaterThan(tauCenter);

        // ~55× C_A ratio compresses through the log (Dietz PI formula) to
        // a ~30% PI difference — enough to produce visibly distinct curves
        const q0Ratio = q0Center / q0Corner;
        const tauRatio = tauCorner / tauCenter;
        expect(q0Ratio).toBeGreaterThan(1.15);
        expect(tauRatio).toBeGreaterThan(1.15);
    });

    it('computeShapeFactor returns exact Dietz values at center and corner', () => {
        const center = computeShapeFactor({
            nxCells: 21, nyCells: 21, aspectRatio: 1.0,
            nx: 21, ny: 21, producerI: 10, producerJ: 10,
        });
        expect(center.shapeFactor).toBeCloseTo(30.8828, 2);
        expect(center.shapeLabel).toContain('center');

        const corner = computeShapeFactor({
            nxCells: 21, nyCells: 21, aspectRatio: 1.0,
            nx: 21, ny: 21, producerI: 0, producerJ: 0,
        });
        expect(corner.shapeFactor).toBeCloseTo(0.5598, 2);
        expect(corner.shapeLabel).toContain('corner');
    });

    it('computeShapeFactor falls back to center when position is absent', () => {
        const result = computeShapeFactor({
            nxCells: 21, nyCells: 21, aspectRatio: 1.0,
        });
        expect(result.shapeFactor).toBeCloseTo(30.8828, 2);
    });

    it('computeShapeFactor gives intermediate value for off-center well', () => {
        const offCenter = computeShapeFactor({
            nxCells: 21, nyCells: 21, aspectRatio: 1.0,
            nx: 21, ny: 21, producerI: 5, producerJ: 10,
        });
        // Must be between corner and center values
        expect(offCenter.shapeFactor).toBeGreaterThan(0.56);
        expect(offCenter.shapeFactor).toBeLessThan(30.88);
        expect(offCenter.shapeLabel).toContain('off-center');
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