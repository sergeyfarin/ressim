import { describe, expect, it } from 'vitest';

import { COMPOSITIONAL_SCHEDULE_CASE_SCHEMA } from '../../compositional/types';
import type { CompositionalCaseConfig } from '../../compositional/types';
import { getScenario, listScenarios } from '../scenarios';
import { compositionalWellState } from '../../compositional/runRecord';
import committedCase from '../../../../opm/compositional/spe3/case.json';
import { comp_spe3_gas_cycling } from './comp_spe3_gas_cycling';

const base = comp_spe3_gas_cycling.params.compositional as CompositionalCaseConfig;

describe('comp_spe3_gas_cycling', () => {
    it('is defined and resolvable, but withheld until its reference run is committed', () => {
        expect(listScenarios().map((scenario) => scenario.key)).not.toContain('comp_spe3_gas_cycling');
        expect(getScenario('comp_spe3_gas_cycling')).toBe(comp_spe3_gas_cycling);
        expect(comp_spe3_gas_cycling.catalog.group).toBe('published-benchmark-decks');
        expect(comp_spe3_gas_cycling.chartLayoutKey).toBe('compositional');
    });

    it('matches the committed native/offline payload', () => {
        expect(committedCase).toEqual(base);
    });

    it('labels the frozen recycle stream as a modified case with its own admission gap', () => {
        expect(comp_spe3_gas_cycling.label).toMatch(/modified/i);
        expect(comp_spe3_gas_cycling.description).toMatch(/frozen|fixed/i);
        expect(comp_spe3_gas_cycling.description).toMatch(/#97/);
        expect(comp_spe3_gas_cycling.analyticalMethodReference).toMatch(/#97/);
    });

    it('runs the nine-component SPE3 characterization on a scheduled case', () => {
        expect(comp_spe3_gas_cycling.params.fluidModel).toBe('compositional');
        expect(base.schema).toBe(COMPOSITIONAL_SCHEDULE_CASE_SCHEMA);
        expect(base.fluid).toBe('pinned-spe3');
        expect(base.initial_composition).toHaveLength(9);
        expect(base.initial_composition.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 12);
    });

    it('transcribes the 9×9×4 deck grid and its four layers', () => {
        expect(base.grid.cells).toBe(9 * 9 * 4);
        expect(base.grid.box_grid).toMatchObject({ nx: 9, ny: 9, nz: 4 });
        expect(base.grid.box_grid?.layer_dz_m).toEqual([30, 30, 50, 50].map((ft) => ft * 0.3048));
        expect(base.grid.box_grid?.layer_kx_md).toEqual([130, 40, 20, 150]);
        expect(base.grid.box_grid?.layer_kz_md).toEqual([13, 4, 2, 15]);
        expect(base.grid.porosity).toBeCloseTo(0.13, 12);
        expect(base.gravity_enabled).toBe(true);
    });

    it('places the producer and injector on two completions each', () => {
        const producer = base.wells.find((well) => well.id === 'PROD');
        const injector = base.wells.find((well) => well.id === 'INJ');
        expect(producer?.completions.map((completion) => completion.cell)).toEqual([
            (2 * 9 + 6) * 9 + 6,
            (3 * 9 + 6) * 9 + 6,
        ]);
        expect(injector?.completions.map((completion) => completion.cell)).toEqual([0, 81]);
        expect(producer?.control).toBe('surface-rate');
        expect(injector?.control).toBe('surface-rate');
        if (producer?.control === 'surface-rate') {
            expect(producer.phase).toBe('vapour');
            expect(producer.target_m3_per_day).toBeLessThan(0);
        }
        if (injector?.control === 'surface-rate') {
            expect(injector.phase).toBe('vapour');
            expect(injector.target_m3_per_day).toBeGreaterThan(0);
        }
        expect(producer?.completions.some((completion) => (completion.head_offset_bar ?? 0) > 0)).toBe(true);
        expect(injector?.completions.some((completion) => (completion.head_offset_bar ?? 0) > 0)).toBe(true);
    });

    it('keeps the generic spatial-profile metadata on the decoded completion cells', () => {
        const wells = compositionalWellState(base);
        const at = (id: string) =>
            wells.filter((well) => well.physical_well_id === id).map((well) => [well.i, well.j, well.k]);
        const producer = at('PROD');
        const injector = at('INJ');
        expect(producer).toEqual([
            [comp_spe3_gas_cycling.params.producerI, comp_spe3_gas_cycling.params.producerJ, 2],
            [comp_spe3_gas_cycling.params.producerI, comp_spe3_gas_cycling.params.producerJ, 3],
        ]);
        expect(injector).toEqual([
            [comp_spe3_gas_cycling.params.injectorI, comp_spe3_gas_cycling.params.injectorJ, 0],
            [comp_spe3_gas_cycling.params.injectorI, comp_spe3_gas_cycling.params.injectorJ, 1],
        ]);
    });

    it('injects the initial separator gas, normalised to a composition', () => {
        const injector = base.wells.find((well) => well.id === 'INJ');
        expect(injector?.injection_composition).toHaveLength(9);
        expect(injector?.injection_composition?.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 12);
        // The gas is methane-rich, as the separator gas of this condensate is.
        expect(injector?.injection_composition?.[2]).toBeGreaterThan(0.7);
    });

    it('cycles for ten years and shuts the injector for blowdown', () => {
        expect(base.schedule).toHaveLength(1);
        const event = base.schedule?.[0];
        expect(event?.time_days).toBeCloseTo(10 * 365.25, 9);
        expect(event?.well).toBe('INJ');
        expect(event?.control).toEqual({ control: 'shut-in' });
        expect(comp_spe3_gas_cycling.params.steps).toBe(180);
        expect(Number(comp_spe3_gas_cycling.params.delta_t_days) * 180).toBeCloseTo(15 * 365.25, 4);
    });

    it('carries the deck SWOF/SGOF on Stone II, with zero capillary pressure', () => {
        expect(base.relperm.model).toBe('stone-ii');
        if (base.relperm.model !== 'stone-ii') return;
        expect(base.relperm.swof).toHaveLength(22);
        expect(base.relperm.sgof).toHaveLength(22);
        expect(base.relperm.swof[0]).toMatchObject({ sw: 0.16, krw: 0, krow: 0.8, pcow: 0 });
        expect(base.relperm.sgof[0]).toMatchObject({ sg: 0, krg: 0, krog: 0.8, pcog: 0 });
        expect(base.relperm.swof.every((row) => row.pcow === 0)).toBe(true);
        expect(base.relperm.sgof.every((row) => row.pcog === 0)).toBe(true);
    });

    it('declares no analytical overlay and cites the case and its validation', () => {
        expect(comp_spe3_gas_cycling.capabilities.analyticalMethod).toBe('none');
        expect(comp_spe3_gas_cycling.analyticalDef).toBeUndefined();
        expect(comp_spe3_gas_cycling.analyticalMethodReference).toMatch(/Kenyon/);
        expect(comp_spe3_gas_cycling.analyticalMethodReference).toMatch(/SPE3_VALIDATION/);
    });
});
