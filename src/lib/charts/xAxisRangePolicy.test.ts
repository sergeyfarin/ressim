import { describe, expect, it } from 'vitest';

import { interpolateMappedAxisValue, resolveSharedXAxisRange, type AxisMapping, type XYPoint } from './xAxisRangePolicy';

describe('xAxisRangePolicy', () => {
    it('clips breakthrough-style ranges by late-time rate tail', () => {
        const allSeries: XYPoint[][] = [
            [
                { x: 0, y: 0 },
                { x: 0.5, y: 0.6 },
                { x: 1.0, y: 1e-4 },
                { x: 2.0, y: 1e-10 },
            ],
        ];

        const range = resolveSharedXAxisRange({
            allSeries,
            rateSeries: allSeries,
            xAxisMode: 'pvi',
            policy: { mode: 'rate-tail-threshold', relativeThreshold: 1e-6 },
        });

        expect(range).toEqual({ min: 0, max: 1.0 });
    });

    it('preserves wider data extents while guaranteeing the mapped PVI window', () => {
        const mapping: AxisMapping = {
            domainValues: [0, 1, 2, 4],
            rangeValues: [0, 10, 22, 46],
        };

        expect(interpolateMappedAxisValue(3, mapping, 'time')).toBeCloseTo(34, 10);

        const range = resolveSharedXAxisRange({
            allSeries: [[{ x: 0, y: 0 }, { x: 46, y: 1 }]],
            xAxisMode: 'time',
            policy: { mode: 'pvi-window', minPvi: 0, maxPvi: 3 },
            pviMappings: [mapping],
        });

        expect(range).toEqual({ min: 0, max: 46 });
    });

    it('keeps a minimum PVI window even when the simulated extent is shorter', () => {
        const range = resolveSharedXAxisRange({
            allSeries: [[{ x: 0, y: 0 }, { x: 0.6, y: 0.4 }]],
            xAxisMode: 'pvi',
            policy: { mode: 'pvi-window', minPvi: 0, maxPvi: 2.5 },
        });

        expect(range).toEqual({ min: 0, max: 2.5 });
    });

    it('extrapolates the mapped axis when a minimum PVI window extends past the last run point', () => {
        const mapping: AxisMapping = {
            domainValues: [0, 0.3, 0.6],
            rangeValues: [0, 12, 24],
        };

        const range = resolveSharedXAxisRange({
            allSeries: [[{ x: 0, y: 0 }, { x: 24, y: 0.4 }]],
            xAxisMode: 'time',
            policy: { mode: 'pvi-window', minPvi: 0, maxPvi: 2.5 },
            pviMappings: [mapping],
        });

        expect(range).toEqual({ min: 0, max: 100 });
    });

    it('falls back to full extent when no mapping exists for a non-PVI axis', () => {
        const range = resolveSharedXAxisRange({
            allSeries: [[{ x: 5, y: 0.2 }, { x: 20, y: 0.6 }]],
            xAxisMode: 'time',
            policy: { mode: 'pvi-window', maxPvi: 3 },
        });

        expect(range).toEqual({ min: 5, max: 20 });
    });
});