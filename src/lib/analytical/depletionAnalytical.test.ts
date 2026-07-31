import { describe, expect, it } from 'vitest';

import {
    calculateDepletionAnalyticalProduction,
    computeShapeFactor,
    dietzProductivityIndex,
    dietzShapeFactorFromProductivityIndex,
    emptyDepletionAnalyticalResult,
    finiteSlabModes,
    type DepletionAnalyticalParams,
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
            0.5 * Math.log((4 * drainageArea) / (shapeFactor * Math.exp(eulerGamma) * 0.1 * 0.1));
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

    it('centred and quadrant-centre wells produce divergent analytical curves on a square grid', () => {
        // 21×21 grid matching dep_pss scenario: centre (10,10) vs quadrant centre (5,5)
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
        const quadrant = calculateDepletionAnalyticalProduction({
            ...sharedParams,
            producerI: 5,
            producerJ: 5,
        });

        // Shape factors must match tabulated Dietz values
        expect(center.meta.shapeFactor).toBeCloseTo(30.8828, 2);
        expect(quadrant.meta.shapeFactor).toBeCloseTo(4.5132, 2);

        // ~6.8× shape factor ratio produces materially different initial rates
        const q0Center = center.meta.q0 ?? 0;
        const q0Quadrant = quadrant.meta.q0 ?? 0;
        expect(q0Center).toBeGreaterThan(0);
        expect(q0Quadrant).toBeGreaterThan(0);
        expect(q0Center).toBeGreaterThan(q0Quadrant);

        // Off-centre well has a smaller PI → lower q0 and longer tau
        const tauCenter = center.meta.tau ?? 0;
        const tauQuadrant = quadrant.meta.tau ?? 0;
        expect(tauQuadrant).toBeGreaterThan(tauCenter);

        // The C_A ratio compresses through the log in the Dietz PI formula,
        // so a 6.8× shape-factor change is a ~13% PI change — small, but the
        // curves stay visibly distinct across the whole history.
        expect(q0Center / q0Quadrant).toBeGreaterThan(1.1);
        expect(tauQuadrant / tauCenter).toBeGreaterThan(1.1);
    });

    it('computeShapeFactor returns the tabulated Dietz value for each shipped geometry', () => {
        const cases: Array<[string, Parameters<typeof computeShapeFactor>[0], number, string]> = [
            [
                'square, centred',
                { nxCells: 21, nyCells: 21, aspectRatio: 1, nx: 21, ny: 21, producerI: 10, producerJ: 10 },
                30.8828, 'Square (centred well)',
            ],
            [
                'square, quadrant centre',
                { nxCells: 22, nyCells: 22, aspectRatio: 1, nx: 22, ny: 22, producerI: 5, producerJ: 5 },
                4.5132, 'Square (well at quadrant centre)',
            ],
            [
                '2:1 rectangle, centred',
                { nxCells: 27, nyCells: 11, aspectRatio: (27 * 22) / (11 * 27), nx: 27, ny: 11, producerI: 13, producerJ: 5 },
                21.8369, '2:1 rectangle (centred well)',
            ],
            [
                '4:1 rectangle, centred',
                { nxCells: 35, nyCells: 7, aspectRatio: (35 * 24) / (7 * 30), nx: 35, ny: 7, producerI: 17, producerJ: 3 },
                5.379, '4:1 rectangle (centred well)',
            ],
            [
                'square, well at quarter length',
                { nxCells: 22, nyCells: 21, aspectRatio: 1, nx: 22, ny: 21, producerI: 5, producerJ: 10 },
                12.9851, 'Square (well at quarter length)',
            ],
            [
                '2:1 rectangle, well at quarter length',
                { nxCells: 22, nyCells: 11, aspectRatio: 2, nx: 22, ny: 11, producerI: 5, producerJ: 5 },
                4.5141, '2:1 rectangle (well at quarter length)',
            ],
            [
                '5:1 rectangle, centred',
                { nxCells: 35, nyCells: 7, aspectRatio: 5, nx: 35, ny: 7, producerI: 17, producerJ: 3 },
                2.36, '5:1 rectangle (centred well)',
            ],
            [
                '4:1 rectangle, well at quarter length',
                { nxCells: 28, nyCells: 7, aspectRatio: 4, nx: 28, ny: 7, producerI: 6, producerJ: 3 },
                0.2318, '4:1 rectangle (well at quarter length)',
            ],
        ];
        for (const [label, input, expected, expectedLabel] of cases) {
            const result = computeShapeFactor(input);
            expect(result.shapeFactor, label).toBeCloseTo(expected, 3);
            expect(result.shapeLabel, label).toBe(expectedLabel);
        }
    });

    it('computeShapeFactor folds mirrored well positions onto one entry', () => {
        const nearEnd = computeShapeFactor({
            nxCells: 22, nyCells: 11, aspectRatio: 2, nx: 22, ny: 11, producerI: 5, producerJ: 5,
        });
        const farEnd = computeShapeFactor({
            nxCells: 22, nyCells: 11, aspectRatio: 2, nx: 22, ny: 11, producerI: 16, producerJ: 5,
        });
        expect(farEnd.shapeFactor).toBe(nearEnd.shapeFactor);
        expect(nearEnd.shapeFactor).toBeCloseTo(4.5141, 4);
    });

    it('computeShapeFactor is orientation-symmetric for rectangles', () => {
        const wide = computeShapeFactor({
            nxCells: 27, nyCells: 11, aspectRatio: 2, nx: 27, ny: 11, producerI: 13, producerJ: 5,
        });
        const tall = computeShapeFactor({
            nxCells: 11, nyCells: 27, aspectRatio: 0.5, nx: 11, ny: 27, producerI: 5, producerJ: 13,
        });
        expect(tall.shapeFactor).toBe(wide.shapeFactor);
        expect(tall.shapeLabel).toBe(wide.shapeLabel);
    });

    it('computeShapeFactor rejects an off-centre rectangle and an untabulated aspect ratio', () => {
        expect(computeShapeFactor({
            nxCells: 27, nyCells: 11, aspectRatio: 2, nx: 27, ny: 11, producerI: 4, producerJ: 5,
        })).toMatchObject({ shapeFactor: null, shapeLabel: 'Unsupported well position' });

        expect(computeShapeFactor({
            nxCells: 27, nyCells: 9, aspectRatio: 3, nx: 27, ny: 9, producerI: 13, producerJ: 4,
        })).toMatchObject({ shapeFactor: null, shapeLabel: 'Unsupported aspect ratio' });
    });

    it('computeShapeFactor falls back to center when position is absent', () => {
        const result = computeShapeFactor({
            nxCells: 21, nyCells: 21, aspectRatio: 1.0,
        });
        expect(result.shapeFactor).toBeCloseTo(30.8828, 2);
    });

    it('computeShapeFactor rejects unsourced off-center interpolation', () => {
        const offCenter = computeShapeFactor({
            nxCells: 21, nyCells: 21, aspectRatio: 1.0,
            nx: 21, ny: 21, producerI: 3, producerJ: 10,
        });
        expect(offCenter.shapeFactor).toBeNull();
        expect(offCenter.shapeLabel).toContain('Unsupported');
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

    it('sums independent layer exponentials instead of collapsing their time constants', () => {
        const common = {
            reservoir: { length: 480, area: 500, porosity: 0.2 } as const,
            timeHistory: [0, 0.5, 2, 10],
            initialSaturation: 0.1,
            nz: 5,
            permMode: 'perLayer',
            uniformPermX: 20,
            uniformPermY: 20,
            layerPermsX: [30, 25, 20, 15, 10],
            layerPermsY: [30, 25, 20, 15, 10],
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

        const composite = calculateDepletionAnalyticalProduction({
            ...common,
            layeredComposite: true,
            arpsB: 1,
        });
        const collapsed = calculateDepletionAnalyticalProduction({
            ...common,
            layeredComposite: false,
            arpsB: 0,
        });

        expect(composite.meta.arpsB).toBeUndefined();
        expect(composite.meta.layerTimeConstants).toHaveLength(5);
        expect(new Set(composite.meta.layerTimeConstants?.map((tau) => tau.toFixed(8))).size).toBe(5);
        expect(composite.production[0].oilRate).toBeCloseTo(collapsed.production[0].oilRate, 10);
        expect(composite.production[2].oilRate).not.toBeCloseTo(collapsed.production[2].oilRate, 4);
    });

    it('volume-averages layered pressure instead of deriving it from total PI', () => {
        const result = calculateDepletionAnalyticalProduction({
            reservoir: { length: 100, area: 200, porosity: 0.2 },
            timeHistory: [1],
            initialSaturation: 0.1,
            nz: 2,
            permMode: 'perLayer',
            uniformPermX: 10,
            uniformPermY: 10,
            layerPermsX: [100, 1],
            layerPermsY: [100, 1],
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
            layeredComposite: true,
        });

        const [fastTau, slowTau] = result.meta.layerTimeConstants!;
        const expectedPressure = 100 + 200 * (
            Math.exp(-1 / fastTau) + Math.exp(-1 / slowTau)
        ) / 2;
        expect(result.production[0].avgPressure).toBeCloseTo(expectedPressure, 10);
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

const finiteSlabParams: DepletionAnalyticalParams = {
    reservoir: { length: 480, area: 100, porosity: 0.2 },
    timeHistory: [1e-5, 0.1, 1, 10],
    initialSaturation: 0.1,
    nz: 1,
    permMode: 'uniform',
    uniformPermX: 20,
    uniformPermY: 20,
    layerPermsX: [], layerPermsY: [],
    cellDx: 10, cellDy: 10, cellDz: 10,
    wellRadius: 0.1, wellSkin: 0,
    muO: 1, sWc: 0.1, sOr: 0.1, nO: 2,
    c_o: 1e-5, c_w: 3e-6, cRock: 1e-6,
    initialPressure: 1500, producerBhp: 50,
    depletionRateScale: 1,
    model: 'finite-slab', nx: 48, ny: 1, producerI: 47, producerJ: 0,
};

describe('finite-reservoir depletion reference', () => {
    it('solves the Robin eigenvalue equation and produces ordered modes', () => {
        const beta = 12.5;
        const modes = finiteSlabModes(beta, 6);
        expect(modes).toHaveLength(6);
        for (let i = 0; i < modes.length; i++) {
            expect(modes[i].lambda * Math.tan(modes[i].lambda)).toBeCloseTo(beta, 8);
            if (i > 0) expect(modes[i].lambda).toBeGreaterThan(modes[i - 1].lambda);
        }
    });

    it('conserves tank storage while rate and pressure decline monotonically', () => {
        const result = calculateDepletionAnalyticalProduction(finiteSlabParams);
        const storage = 480 * 100 * 0.2 * (0.9e-5 + 0.1 * 3e-6 + 1e-6);
        for (const point of result.production) {
            expect(point.cumulativeOil).toBeCloseTo(storage * (1500 - point.avgPressure), 9);
        }
        for (let i = 1; i < result.production.length; i++) {
            expect(result.production[i].oilRate).toBeLessThan(result.production[i - 1].oilRate);
            expect(result.production[i].avgPressure).toBeLessThan(result.production[i - 1].avgPressure);
        }
    });
});

describe('bounded PSS reference safeguards', () => {
    it('refuses unsourced interpolation for an arbitrary off-centre well', () => {
        expect(computeShapeFactor({
            nxCells: 21, nyCells: 21, aspectRatio: 1,
            nx: 21, ny: 21, producerI: 3, producerJ: 10,
        })).toMatchObject({ shapeFactor: null });
    });

    it('makes Dietz PI linear in permeability and lower with positive skin', () => {
        const base = {
            permeabilityMd: 20, thicknessM: 10, mobilityPerCp: 1,
            drainageAreaM2: 420 * 420, shapeFactor: 30.8828,
            wellRadiusM: 0.1, skin: 0,
        };
        const pi = dietzProductivityIndex(base);
        expect(dietzProductivityIndex({ ...base, permeabilityMd: 40 })).toBeCloseTo(2 * pi, 12);
        expect(dietzProductivityIndex({ ...base, skin: 3 })).toBeLessThan(pi);
    });

    it('recovers the tabulated shape factor from Dietz productivity', () => {
        const base = {
            permeabilityMd: 20, thicknessM: 10, mobilityPerCp: 1,
            drainageAreaM2: 420 * 420, shapeFactor: 30.8828,
            wellRadiusM: 0.1, skin: 0,
        };
        const productivityIndex = dietzProductivityIndex(base);
        expect(dietzShapeFactorFromProductivityIndex({
            ...base,
            productivityIndex,
        })).toBeCloseTo(base.shapeFactor, 8);
    });
});
