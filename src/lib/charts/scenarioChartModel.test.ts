import { describe, expect, it } from 'vitest';
import { buildScenarioComparisonFamily } from './scenarioChartModel';
import { getScenario, getScenarioChartLayout } from '../catalog/scenarios';
import { buildReferenceComparisonModel } from './buildChartData';
import { resolveChartPanelDefinition } from './chartPanelSelection';
import { PANEL_DEFS } from './panelDefs';
import type { RateChartPanelKey } from './rateChartLayoutConfig';

describe('buildScenarioComparisonFamily', () => {
    it('merges real parsed OPM Flow series into publishedReferenceSeries for wf_bl1d', () => {
        // This is the exact production call site (see scenarioChartModel.ts,
        // used by ScenarioChart.svelte / ReferenceComparisonChart) that
        // decides what dashed/reference curves reach the comparison chart.
        // A live-browser check isn't available in this environment, so this
        // exercises the real scenario object end-to-end instead.
        const scenario = getScenario('wf_bl1d');
        const family = buildScenarioComparisonFamily({ scenario });

        expect(family).not.toBeNull();
        const opmSeries = family!.publishedReferenceSeries!.filter(
            (series) => (series as { sourceType?: string }).sourceType === 'opm-flow-precomputed',
        );
        expect(opmSeries.length).toBeGreaterThan(0);

        const oilRate = opmSeries.find((s) => s.curveKey === 'opm-oil-rate');
        expect(oilRate).toBeDefined();
        expect(oilRate!.panelKey).toBe('rates');
        expect(oilRate!.data.length).toBeGreaterThan(0);
        expect(oilRate!.data.every((point) => Number.isFinite(point.x) && Number.isFinite(point.y))).toBe(true);
    });

    it('merges real parsed OPM Flow series for spe1_gas_injection alongside its digitized Eclipse references', () => {
        const scenario = getScenario('spe1_gas_injection');
        const family = buildScenarioComparisonFamily({ scenario });

        expect(family).not.toBeNull();
        const allSeries = family!.publishedReferenceSeries!;
        const opmSeries = allSeries.filter(
            (series) => (series as { sourceType?: string }).sourceType === 'opm-flow-precomputed',
        );
        const digitizedSeries = allSeries.filter((series) => series.curveKey.startsWith('published-'));

        expect(opmSeries.length).toBeGreaterThan(0);
        expect(digitizedSeries.length).toBeGreaterThan(0);

        const injectorBhp = opmSeries.find((s) => s.curveKey === 'opm-injector-bhp');
        expect(injectorBhp).toBeDefined();
        expect(injectorBhp!.panelKey).toBe('injector_bhp');
    });

    it('renders a prerun-artifacts scenario\'s bundled artifact as primary series (E7)', () => {
        // wf_bl1d_opm is the pre-run demonstrator: it pulls the bundled wf_bl1d
        // OPM artifact by key and marks it primary (solid) content, since there
        // is no live simulation curve to compare against.
        const scenario = getScenario('wf_bl1d_opm');
        expect(scenario?.capabilities.runMode).toBe('prerun-artifacts');

        const family = buildScenarioComparisonFamily({ scenario });
        expect(family).not.toBeNull();

        const opmSeries = family!.publishedReferenceSeries!.filter(
            (series) => (series as { sourceType?: string }).sourceType === 'opm-flow-precomputed',
        );
        expect(opmSeries.length).toBeGreaterThan(0);
        // Every artifact series must be flagged primary so the chart renders it
        // solid rather than as a faint dashed reference overlay.
        expect(opmSeries.every((series) => (series as { primary?: boolean }).primary === true)).toBe(true);

        const oilRate = opmSeries.find((s) => s.curveKey === 'opm-oil-rate');
        expect(oilRate).toBeDefined();
        expect(oilRate!.data.length).toBeGreaterThan(0);
    });

    it('retains E7 OPM artifact curves after the production panel layout resolution', () => {
        const scenario = getScenario('wf_bl1d_opm')!;
        const family = buildScenarioComparisonFamily({ scenario })!;
        const model = buildReferenceComparisonModel({ family, results: [], xAxisMode: 'time' });
        const layout = getScenarioChartLayout(scenario);

        const resolvedCurveKeys = (panelKey: RateChartPanelKey) => resolveChartPanelDefinition({
            override: layout.rateChart?.panels?.[panelKey],
            fallback: PANEL_DEFS[panelKey],
            entries: model.panels[panelKey].curves.map((curve, index) => ({
                curve,
                series: model.panels[panelKey].series[index] ?? [],
            })),
            getScalePresetConfig: () => ({}),
        }).curves.map((curve) => curve.curveKey);

        expect(resolvedCurveKeys('rates')).toEqual(expect.arrayContaining([
            'opm-oil-rate',
            'opm-water-rate',
            'opm-injection-rate',
        ]));
        expect(resolvedCurveKeys('cumulative')).toEqual(expect.arrayContaining([
            'opm-cum-oil',
            'opm-cum-water',
        ]));
        expect(resolvedCurveKeys('diagnostics')).toContain('opm-avg-pressure');
    });
});
