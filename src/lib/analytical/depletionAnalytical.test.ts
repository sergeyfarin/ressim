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

    // ── Arps decline tests ────────────────────────────────────────────────

    it('arpsB=0 produces identical results to legacy exponential decline', () => {
        const baseParams = {
            reservoir: { length: 100, area: 1000, porosity: 0.2 } as const,
            timeHistory: [0, 5, 10, 50, 100],
            initialSaturation: 0.2,
            nz: 1,
            permMode: 'uniform',
            uniformPermX: 100,
            uniformPermY: 100,
            layerPermsX: [] as number[],
            layerPermsY: [] as number[],
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
        };

        const withoutB = calculateDepletionAnalyticalProduction(baseParams);
        const withB0 = calculateDepletionAnalyticalProduction({ ...baseParams, arpsB: 0 });

        expect(withB0.meta.arpsB).toBe(0);
        for (let i = 0; i < withoutB.production.length; i++) {
            expect(withB0.production[i].oilRate).toBeCloseTo(withoutB.production[i].oilRate, 12);
            expect(withB0.production[i].cumulativeOil).toBeCloseTo(withoutB.production[i].cumulativeOil, 12);
            expect(withB0.production[i].avgPressure).toBeCloseTo(withoutB.production[i].avgPressure, 12);
        }
    });

    it('arpsB=0.5 matches the Arps hyperbolic formula exactly', () => {
        // Use a simple 1D slab so q0 and tau are straightforward
        const params = {
            reservoir: { length: 480, area: 100, porosity: 0.2 } as const,
            timeHistory: [0, 10, 50, 100],
            initialSaturation: 0.1,
            nz: 1,
            permMode: 'uniform',
            uniformPermX: 20,
            uniformPermY: 20,
            layerPermsX: [] as number[],
            layerPermsY: [] as number[],
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
            initialPressure: 1500,
            producerBhp: 50,
            depletionRateScale: 1,
            arpsB: 0.5,
        };

        const result = calculateDepletionAnalyticalProduction(params);
        const q0 = result.meta.q0!;
        const tau = result.meta.tau!;
        const Di = 1 / tau;
        const b = 0.5;

        expect(result.meta.arpsB).toBe(0.5);
        expect(q0).toBeGreaterThan(0);

        for (const pt of result.production) {
            const t = pt.time;
            // Arps hyperbolic: q(t) = q_i / (1 + b·Di·t)^(1/b)
            const expectedRate = q0 * Math.pow(1 + b * Di * t, -1 / b);
            // N_p(t) = q_i/((1-b)·Di) · [1 - (1+b·Di·t)^((b-1)/b)]
            const expectedCum = (q0 / ((1 - b) * Di)) * (1 - Math.pow(1 + b * Di * t, (b - 1) / b));

            expect(pt.oilRate).toBeCloseTo(expectedRate, 9);
            expect(pt.cumulativeOil).toBeCloseTo(expectedCum, 9);
        }
    });

    it('arpsB=1 matches the Arps harmonic formula exactly', () => {
        const params = {
            reservoir: { length: 480, area: 100, porosity: 0.2 } as const,
            timeHistory: [0, 10, 50, 100],
            initialSaturation: 0.1,
            nz: 1,
            permMode: 'uniform',
            uniformPermX: 20,
            uniformPermY: 20,
            layerPermsX: [] as number[],
            layerPermsY: [] as number[],
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
            initialPressure: 1500,
            producerBhp: 50,
            depletionRateScale: 1,
            arpsB: 1.0,
        };

        const result = calculateDepletionAnalyticalProduction(params);
        const q0 = result.meta.q0!;
        const tau = result.meta.tau!;
        const Di = 1 / tau;

        expect(result.meta.arpsB).toBe(1);

        for (const pt of result.production) {
            const t = pt.time;
            // Harmonic: q(t) = q_i / (1 + Di·t)
            const expectedRate = q0 / (1 + Di * t);
            // N_p(t) = q_i/Di · ln(1 + Di·t)
            const expectedCum = (q0 / Di) * Math.log(1 + Di * t);

            expect(pt.oilRate).toBeCloseTo(expectedRate, 9);
            expect(pt.cumulativeOil).toBeCloseTo(expectedCum, 9);
        }
    });

    it('higher arpsB produces slower rate decline and higher cumulative production', () => {
        const baseParams = {
            reservoir: { length: 480, area: 100, porosity: 0.2 } as const,
            timeHistory: [0, 10, 50, 100, 200],
            initialSaturation: 0.1,
            nz: 1,
            permMode: 'uniform',
            uniformPermX: 20,
            uniformPermY: 20,
            layerPermsX: [] as number[],
            layerPermsY: [] as number[],
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
            initialPressure: 1500,
            producerBhp: 50,
            depletionRateScale: 1,
        };

        const exp = calculateDepletionAnalyticalProduction({ ...baseParams, arpsB: 0 });
        const hyp = calculateDepletionAnalyticalProduction({ ...baseParams, arpsB: 0.5 });
        const har = calculateDepletionAnalyticalProduction({ ...baseParams, arpsB: 1.0 });

        // All start at the same q0
        expect(exp.production[0].oilRate).toBeCloseTo(hyp.production[0].oilRate, 9);
        expect(exp.production[0].oilRate).toBeCloseTo(har.production[0].oilRate, 9);

        // At later times, higher b → slower decline → higher rate
        const lastIdx = baseParams.timeHistory.length - 1;
        expect(hyp.production[lastIdx].oilRate).toBeGreaterThan(exp.production[lastIdx].oilRate);
        expect(har.production[lastIdx].oilRate).toBeGreaterThan(hyp.production[lastIdx].oilRate);

        // Higher b → more cumulative production over same time span
        expect(hyp.production[lastIdx].cumulativeOil).toBeGreaterThan(exp.production[lastIdx].cumulativeOil);
        expect(har.production[lastIdx].cumulativeOil).toBeGreaterThan(hyp.production[lastIdx].cumulativeOil);
    });

    it('pressure tracks rate through PI for all Arps b values', () => {
        const baseParams = {
            reservoir: { length: 480, area: 100, porosity: 0.2 } as const,
            timeHistory: [0, 50, 100],
            initialSaturation: 0.1,
            nz: 1,
            permMode: 'uniform',
            uniformPermX: 20,
            uniformPermY: 20,
            layerPermsX: [] as number[],
            layerPermsY: [] as number[],
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
            initialPressure: 1500,
            producerBhp: 50,
            depletionRateScale: 1,
        };

        for (const b of [0, 0.3, 0.5, 0.7, 1.0]) {
            const result = calculateDepletionAnalyticalProduction({ ...baseParams, arpsB: b });
            const q0 = result.meta.q0!;
            const dP = 1500 - 50;

            for (const pt of result.production) {
                // P_avg = P_bhp + ΔP · q(t)/q_i
                const expectedPressure = 50 + dP * (pt.oilRate / q0);
                expect(pt.avgPressure).toBeCloseTo(expectedPressure, 9);
            }
        }
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