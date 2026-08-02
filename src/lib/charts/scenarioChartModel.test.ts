import { describe, expect, it } from 'vitest';
import { buildScenarioComparisonFamily } from './scenarioChartModel';
import { getScenario } from '../catalog/scenarios';

describe('buildScenarioComparisonFamily', () => {
    it('retains gas-drive OPM as a benchmark-only overlay disabled by default', () => {
        const scenario = getScenario('gas_drive')!;
        const family = buildScenarioComparisonFamily({ scenario })!;
        const opmSeries = family.publishedReferenceSeries!.filter(
            (series) => series.sourceType === 'opm-flow-precomputed',
        );
        expect(opmSeries.length).toBeGreaterThan(0);
        expect(opmSeries.every((series) => series.defaultVisible === false)).toBe(true);
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

    it('places derived OPM p/z curves in the gas material-balance panel', () => {
        const scenario = getScenario('dep_gas_pz')!;
        const family = buildScenarioComparisonFamily({ scenario })!;
        const pzSeries = family.publishedReferenceSeries!.filter(
            (series) => series.sourceType === 'opm-flow-precomputed' && series.panelKey === 'pz',
        );

        expect(pzSeries).toHaveLength(2);
        expect(pzSeries.every((series) => series.xAxisMap?.cumulativeGasSm3?.length)).toBeTruthy();
    });

});
