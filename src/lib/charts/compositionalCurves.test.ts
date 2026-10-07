import { describe, expect, it } from 'vitest';

import type { BenchmarkFamily } from '../scenario/referenceTypes';
import { buildRunResult, type RunSpec } from '../scenario/runModel';
import { getScenario, getScenarioChartLayout } from '../catalog/scenarios';
import { buildCompositionalRunRecord } from '../compositional/runRecord';
import type { CompositionalCaseConfig, CompositionalSnapshot } from '../compositional/types';
import { buildReferenceComparisonModel } from './buildChartData';
import {
    COMPOSITIONAL_PANEL_PLACEMENT,
    compositionalCurvesForRun,
    panelGroupMemberId,
} from './compositionalCurves';
import { getReferenceComparisonCaseColor } from './referenceChartTypes';
import { RUN_QUANTITIES } from './runQuantities';
import { SIMULATION_CURVES } from './simulationCurves';

const scenario = getScenario('comp_co2_1d')!;
const config = scenario.params.compositional as CompositionalCaseConfig;

/** A three-cell run that floods with CO2: inventory shifts from C10 to CO2 as time goes on. */
function snapshots(): CompositionalSnapshot[] {
    return [0, 1, 2].map((t) => ({
        time_days: t,
        component_ids: ['CO2', 'C1', 'C10'],
        pressure: [100 - t, 90 - t, 80 - t],
        composition: [
            [0.1 + 0.2 * t, 0.1, 0.1],
            [0.3, 0.3, 0.3],
            [0.6 - 0.2 * t, 0.6, 0.6],
        ],
        phase_state: ['two-phase', t > 0 ? 'two-phase' : 'single-liquid', 'single-liquid'],
        vapour_saturation: [0.2 * t, 0.1 * t, 0],
        inventory: [10 + 5 * t, 30, 60 - 5 * t],
        cumulative_well_moles: [[5 * t, 0, 0], [0, 0, -5 * t]],
    }));
}

function compositionalResult(variantKey: string | null, label: string) {
    const spec = {
        key: variantKey ?? scenario.key,
        caseKey: variantKey ?? scenario.key,
        familyKey: scenario.key,
        analyticalMethod: 'none',
        variantKey,
        variantLabel: variantKey ? label : null,
        label,
        description: '',
        params: scenario.params,
        steps: 2,
        deltaTDays: 1,
        historyInterval: 1,
        reference: { kind: 'analytical', source: `${scenario.key}:analytical` },
        comparisonMetric: null,
        breakthroughCriterion: null,
        comparisonMeaning: '',
    } as unknown as RunSpec;
    return buildRunResult({
        spec,
        rateHistory: [],
        compositional: buildCompositionalRunRecord({
            config,
            snapshots: snapshots(),
            stop: { reason: 'completed', message: 'Finished 2 steps.' },
        }),
    });
}

function family(): BenchmarkFamily {
    return {
        key: scenario.key,
        label: scenario.label,
        description: scenario.description,
        analyticalMethod: 'none',
        chartLayoutKey: scenario.chartLayoutKey,
    } as unknown as BenchmarkFamily;
}

const BLACK_OIL_CURVE_KEYS = new Set(SIMULATION_CURVES.map((curve) => curve.curveKey));

describe('compositionalCurvesForRun', () => {
    const record = buildCompositionalRunRecord({
        config,
        snapshots: snapshots(),
        stop: { reason: 'completed', message: '' },
    });

    it('sources every curve from the compositional registry, never a black-oil quantity', () => {
        const curves = compositionalCurvesForRun(record, 'time');
        expect(curves.length).toBeGreaterThan(0);
        for (const curve of curves) {
            expect(curve.curveKey.startsWith('comp-')).toBe(true);
            expect(BLACK_OIL_CURVE_KEYS.has(curve.curveKey)).toBe(false);
            expect(Object.keys(RUN_QUANTITIES)).not.toContain(curve.curveKey.replace(/-sim$/, ''));
        }
    });

    it('gives each component its own panel in a per-property group', () => {
        const curves = compositionalCurvesForRun(record, 'time');
        const inventory = curves.filter((curve) => curve.group?.group === 'comp_inventory');
        expect(inventory.map((curve) => curve.panel)).toEqual(
            ['CO2', 'C1', 'C10'].map((component) => panelGroupMemberId('comp_inventory', component)),
        );
        expect(inventory[0].group?.title).toBe('CO2 In Place (mol)');
        expect(inventory[0].values).toEqual([10, 15, 20]);
        // Each panel holds one property.
        const byPanel = new Map<string, Set<string>>();
        for (const curve of curves) {
            byPanel.set(curve.panel, (byPanel.get(curve.panel) ?? new Set()).add(curve.property));
        }
        for (const properties of byPanel.values()) expect(properties.size).toBe(1);
    });

    it('plots against report time, and emits nothing on an axis the run does not have', () => {
        const curves = compositionalCurvesForRun(record, 'time');
        expect(curves[0].xValues).toEqual([0, 1, 2]);
        expect(compositionalCurvesForRun(record, 'pvi')).toEqual([]);
        expect(compositionalCurvesForRun(record, 'cumLiquid')).toEqual([]);
    });
});

describe('buildReferenceComparisonModel with compositional runs', () => {
    const results = [compositionalResult('grid_5', '5 cells'), compositionalResult('grid_10', '10 cells')];
    const model = buildReferenceComparisonModel({
        family: family(),
        results,
        xAxisMode: 'time',
        caseOrder: ['grid_5', 'grid_10'],
    });
    const populated = Object.entries(model.panels).filter(([, panel]) => (panel?.curves.length ?? 0) > 0);

    it('emits no black-oil curve key under a compositional scenario', () => {
        expect(populated.length).toBeGreaterThan(0);
        for (const [panelKey, panel] of populated) {
            expect(panelKey.startsWith('comp_')).toBe(true);
            for (const curve of panel!.curves) {
                expect(curve.curveKey?.startsWith('comp-')).toBe(true);
            }
        }
    });

    it('draws each case in its declared colour, solid', () => {
        const panel = model.panels.comp_pressure!;
        expect(panel.curves.map((curve) => curve.caseKey)).toEqual(['grid_5', 'grid_10']);
        expect(panel.curves.map((curve) => curve.color)).toEqual([
            getReferenceComparisonCaseColor(model.caseColorIndices.grid_5),
            getReferenceComparisonCaseColor(model.caseColorIndices.grid_10),
        ]);
        for (const curve of panel.curves) expect(curve.borderDash).toBeUndefined();
    });

    it('expands per-component groups once, in component order, holding every case', () => {
        expect(model.panelGroups.comp_inventory).toEqual([
            { id: 'comp_inventory_CO2', title: 'CO2 In Place (mol)' },
            { id: 'comp_inventory_C1', title: 'C1 In Place (mol)' },
            { id: 'comp_inventory_C10', title: 'C10 In Place (mol)' },
        ]);
        expect(model.panels.comp_inventory_CO2!.curves).toHaveLength(2);
        expect(model.panelGroups.comp_net_injected.map((member) => member.id)).toEqual([
            'comp_net_injected_CO2', 'comp_net_injected_C1', 'comp_net_injected_C10',
        ]);
    });

    it('leaves the compositional panels empty on an axis the runs do not have', () => {
        const pvi = buildReferenceComparisonModel({ family: family(), results, xAxisMode: 'pvi' });
        for (const panel of Object.values(pvi.panels)) expect(panel?.curves.length ?? 0).toBe(0);
    });
});

describe('the compositional chart layout', () => {
    const layout = getScenarioChartLayout(scenario, null);

    it('places every compositional panel and group, and nothing black-oil', () => {
        const order = layout.chart?.panelOrder ?? [];
        const placed = Object.values(COMPOSITIONAL_PANEL_PLACEMENT).map((placement) =>
            placement.kind === 'panel' ? placement.panel : placement.group,
        );
        expect([...order].sort()).toEqual([...placed].sort());
    });

    it('offers only the axes a compositional run can be drawn against', () => {
        expect(layout.chart?.xAxisOptions).toEqual(['time', 'logTime']);
    });
});
