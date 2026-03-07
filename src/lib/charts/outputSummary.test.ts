import { describe, expect, it } from 'vitest';
import {
    buildLiveOutputSummaryItems,
    buildReferenceComparisonSummaryItems,
} from './outputSummary';
import type { BenchmarkFamily } from '../catalog/benchmarkCases';
import type { BenchmarkRunResult } from '../benchmarkRunModel';

const waterfloodFamily = {
    key: 'bl_case_a_refined',
    scenarioClass: 'buckley-leverett',
} as BenchmarkFamily;

describe('outputSummary', () => {
    it('builds compact live-run summary cards for waterflood outputs', () => {
        const items = buildLiveOutputSummaryItems({
            activeMode: 'waterflood',
            activeCase: 'bl_case_a_refined',
            timeValues: [10, 20, 30],
            pviSeries: [0.2, 0.5, 0.82],
            oilRateSeries: [100, 90, 80],
            waterCutSeries: [0.01, 0.14, 0.37],
            cumulativeOilSeries: [1000, 1800, 2400],
            recoverySeries: [0.1, 0.18, 0.24],
            pressureSeries: [285, 272, 261],
            mismatchSummary: { pointsCompared: 3, mae: 0.2, rmse: 0.3, mape: 4.5 },
        });

        expect(items.map((item) => item.label)).toEqual([
            'Run Context',
            'Primary Output',
            'Recovery',
            'Avg Pressure',
        ]);
        expect(items[0]?.value).toBe('Waterflood');
        expect(items[1]?.detail).toContain('Final PVI 0.82');
        expect(items[1]?.detail).toContain('Reference MAPE 4.5%');
    });

    it('builds focused comparison summary cards from the selected reference result', () => {
        const result = {
            key: 'base',
            variantKey: null,
            label: 'Base case',
            params: {
                nx: 10,
                ny: 1,
                nz: 1,
                cellDx: 10,
                cellDy: 10,
                cellDz: 10,
                reservoirPorosity: 0.2,
                initialSaturation: 0.25,
            },
            recoverySeries: [0.1, 0.18, 0.24],
            pressureSeries: [280, 268, 256],
            breakthroughPvi: 0.82,
            referencePolicy: {
                referenceLabel: 'Buckley-Leverett reference solution',
            },
            referenceComparison: {
                status: 'within-tolerance',
                referenceValue: 0.8,
                relativeError: 0.025,
                summary: 'Reference comparison is within tolerance.',
            },
            comparisonOutputs: {
                breakthroughShiftPvi: 0.02,
                recoveryDifferenceAtFinalCoordinate: 0.01,
                pressureDifferenceAtFinalTime: -3,
                oilRateRelativeErrorAtFinalTime: null,
                errorSummary: 'Breakthrough shift +0.02 PVI.',
            },
        } as BenchmarkRunResult;

        const items = buildReferenceComparisonSummaryItems({
            family: waterfloodFamily,
            results: [result],
            primaryResultKey: 'base',
        });

        expect(items.map((item) => item.label)).toEqual([
            'Focused Run',
            'Primary Review',
            'Recovery',
            'Avg Pressure',
        ]);
        expect(items[0]?.value).toBe('Base case');
        expect(items[1]?.value).toBe('0.82 PVI');
        expect(items[1]?.detail).toContain('Ref 0.8 PVI');
        expect(items[1]?.detail).toContain('+0.02 PVI');
    });
});