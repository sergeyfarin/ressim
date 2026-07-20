import { describe, expect, it } from 'vitest';
import { resolveHistoryDivider, historyDividerColors } from './historyDivider';
import type { HistoryWindow } from '../catalog/scenarios';

describe('resolveHistoryDivider', () => {
    const timeWindow: HistoryWindow = {
        boundary: 12,
        axis: 'time',
        historyLabel: 'Matched history',
        forecastLabel: 'Forecast',
    };

    it('resolves a matching-axis window with its labels', () => {
        expect(resolveHistoryDivider(timeWindow, 'time')).toEqual({
            boundary: 12,
            historyLabel: 'Matched history',
            forecastLabel: 'Forecast',
        });
    });

    it('defaults the axis to time and the labels when omitted', () => {
        expect(resolveHistoryDivider({ boundary: 5 }, 'time')).toEqual({
            boundary: 5,
            historyLabel: 'History',
            forecastLabel: 'Forecast',
        });
    });

    it('suppresses the divider when the active x-axis differs from the window axis', () => {
        expect(resolveHistoryDivider(timeWindow, 'pvi')).toBeNull();
        // A window with no explicit axis is treated as time-only.
        expect(resolveHistoryDivider({ boundary: 5 }, 'pvi')).toBeNull();
    });

    it('suppresses the divider for absent or non-finite boundaries', () => {
        expect(resolveHistoryDivider(null, 'time')).toBeNull();
        expect(resolveHistoryDivider(undefined, 'time')).toBeNull();
        expect(resolveHistoryDivider({ boundary: Number.NaN, axis: 'time' }, 'time')).toBeNull();
        expect(resolveHistoryDivider({ boundary: Infinity, axis: 'time' }, 'time')).toBeNull();
    });
});

describe('historyDividerColors', () => {
    it('returns distinct theme-aware palettes for dark and light', () => {
        const dark = historyDividerColors('dark');
        const light = historyDividerColors('light');
        for (const palette of [dark, light]) {
            expect(palette.shade).toMatch(/^rgba\(/);
            expect(palette.line).toMatch(/^rgba\(/);
            expect(palette.text).toMatch(/^rgba\(/);
        }
        expect(dark).not.toEqual(light);
    });
});
