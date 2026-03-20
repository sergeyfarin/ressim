import { describe, expect, it } from 'vitest';
import {
    buildLiveOutputSummaryItems,
} from './outputSummary';

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

});