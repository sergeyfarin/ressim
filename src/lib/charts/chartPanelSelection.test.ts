import { describe, expect, it } from 'vitest';
import {
    coerceChartAxisState,
    getConfiguredXAxisOptions,
    REFERENCE_COMPARISON_X_AXIS_OPTIONS,
    resolveChartPanelDefinition,
    resolveChartPanelLayout,
    suppressLeadingOutliers,
    type ChartPanelEntry,
} from './chartPanelSelection';

type TestCurve = {
    label: string;
    curveKey?: string;
    referenceSourceType?: 'published-reference' | 'opm-flow-precomputed';
};

describe('chartPanelSelection', () => {
    it('suppresses a short leading outlier cluster against the median operating rate', () => {
        const transient = [
            { x: 0, y: 9_100 }, { x: 1, y: 4_000 },
            ...Array.from({ length: 38 }, (_, index) => ({ x: index + 2, y: 100 + index % 3 })),
        ];
        const filtered = suppressLeadingOutliers(transient, {
            medianRatio: 2,
            maxLeadingFraction: 0.1,
        });

        expect(filtered.slice(0, 3)).toEqual([
            { x: 0, y: null },
            { x: 1, y: null },
            { x: 2, y: 100 },
        ]);
        expect(suppressLeadingOutliers(transient, undefined)).toBe(transient);
    });

    it('does not partially suppress a sustained high-rate decline', () => {
        const decline = Array.from({ length: 40 }, (_, index) => ({
            x: index,
            y: index < 10 ? 1_000 - index * 50 : 100,
        }));
        expect(suppressLeadingOutliers(decline, {
            medianRatio: 2,
            maxLeadingFraction: 0.1,
        })).toBe(decline);
    });

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

    it('keeps cumulative gas as the default for gas material-balance layouts', () => {
        const options = getConfiguredXAxisOptions(
            REFERENCE_COMPARISON_X_AXIS_OPTIONS,
            ['cumGas', 'time'],
        );
        const state = coerceChartAxisState({
            xAxisMode: 'cumGas',
            xAxisOptions: options,
            logScale: false,
        });

        expect(options.map((option) => option.value)).toEqual(['cumGas', 'time']);
        expect(state.xAxisMode).toBe('cumGas');
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

    it('retains additive published and OPM artifact curves outside the configured curve keys', () => {
        const entries: Array<ChartPanelEntry<TestCurve, number[]>> = [
            { curve: { label: 'Oil Rate', curveKey: 'oil-rate-sim' }, series: [1] },
            { curve: { label: 'Published Oil Rate', curveKey: 'published-oil-rate', referenceSourceType: 'published-reference' }, series: [2] },
            { curve: { label: 'OPM Oil Rate', curveKey: 'opm-oil-rate', referenceSourceType: 'opm-flow-precomputed' }, series: [3] },
        ];

        const panel = resolveChartPanelDefinition({
            fallback: {
                title: 'Oil Rate',
                curveKeys: ['oil-rate-sim'],
                scalePreset: 'rates',
            },
            entries,
            getScalePresetConfig: () => ({}),
        });

        expect(panel.curves.map((curve) => curve.curveKey)).toEqual([
            'oil-rate-sim',
            'published-oil-rate',
            'opm-oil-rate',
        ]);
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
