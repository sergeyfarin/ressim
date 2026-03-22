import { describe, expect, it } from 'vitest';
import {
    coerceChartAxisState,
    getConfiguredXAxisOptions,
    resolveChartPanelDefinition,
    resolveChartPanelLayout,
    type ChartPanelEntry,
} from './chartPanelSelection';

type TestCurve = {
    label: string;
    curveKey?: string;
};

describe('chartPanelSelection', () => {
    it('filters x-axis options to the configured modes', () => {
        const options = getConfiguredXAxisOptions(
            [
                { value: 'time', label: 'Time' },
                { value: 'pvi', label: 'PVI' },
                { value: 'cumInjection', label: 'Cum Inj' },
            ],
            ['pvi', 'cumInjection'],
        );

        expect(options.map((option) => option.value)).toEqual(['pvi', 'cumInjection']);
    });

    it('coerces invalid axis state back onto the allowed x-axis set and disables forbidden log scale', () => {
        const nextState = coerceChartAxisState({
            xAxisMode: 'time',
            xAxisOptions: [
                { value: 'pvi', label: 'PVI' },
                { value: 'cumInjection', label: 'Cum Inj' },
            ],
            logScale: true,
            allowLogScale: false,
        });

        expect(nextState).toEqual({
            xAxisMode: 'pvi',
            logScale: false,
        });
    });

    it('resolves panel entries by curve keys before applying scale and title overrides', () => {
        const entries: Array<ChartPanelEntry<TestCurve, number[]>> = [
            { curve: { label: 'Oil Rate', curveKey: 'oil-rate-sim' }, series: [1, 2] },
            { curve: { label: 'Oil Rate (Reference)', curveKey: 'oil-rate-reference' }, series: [3, 4] },
            { curve: { label: 'Pressure', curveKey: 'avg-pressure-sim' }, series: [5, 6] },
        ];

        const panel = resolveChartPanelDefinition({
            override: {
                title: 'Configured Rates',
                curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
                scalePreset: 'pressure',
                allowLogToggle: true,
            },
            fallback: {
                title: 'Rates',
                curveLabels: ['Oil Rate', 'Pressure'],
                scalePreset: 'rates',
            },
            entries,
            getScalePresetConfig: (scalePreset) => ({ preset: scalePreset }),
        });

        expect(panel.title).toBe('Configured Rates');
        expect(panel.allowLogToggle).toBe(true);
        expect(panel.scales).toEqual({ preset: 'pressure' });
        expect(panel.curves.map((curve) => curve.label)).toEqual(['Oil Rate', 'Oil Rate (Reference)']);
    });

    it('falls back to label-based panel selection when no curve keys are provided', () => {
        const entries: Array<ChartPanelEntry<TestCurve, number[]>> = [
            { curve: { label: 'Oil Rate' }, series: [1] },
            { curve: { label: 'Pressure' }, series: [2] },
        ];

        const panel = resolveChartPanelDefinition({
            fallback: {
                title: 'Diagnostics',
                curveLabels: ['Pressure'],
                scalePreset: 'pressure',
            },
            entries,
            getScalePresetConfig: (scalePreset) => ({ preset: scalePreset }),
        });

        expect(panel.curves.map((curve) => curve.label)).toEqual(['Pressure']);
        expect(panel.series).toEqual([[2]]);
    });

    it('merges visibility and expansion metadata from the panel layout override', () => {
        const panel = resolveChartPanelLayout({
            override: {
                title: 'Configured Sweep',
                scalePreset: 'sweep',
                visible: false,
                expanded: true,
            },
            fallback: {
                title: 'Sweep',
                scalePreset: 'sweep_rf',
                visible: true,
                expanded: false,
            },
        });

        expect(panel).toEqual({
            title: 'Configured Sweep',
            scalePreset: 'sweep',
            visible: false,
            expanded: true,
            allowLogToggle: false,
        });
    });
});