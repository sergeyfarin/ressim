import { describe, expect, it } from 'vitest';
import { buildScenarioComparisonFamily } from './scenarioChartModel';
import { getScenario } from '../catalog/scenarios';

describe('buildScenarioComparisonFamily', () => {
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

});
