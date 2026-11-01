import { describe, expect, it } from 'vitest';

import { COMPOSITIONAL_SCHEDULE_CASE_SCHEMA } from '../../compositional/types';
import type { CompositionalCaseConfig } from '../../compositional/types';
import { getScenario, listScenarios } from '../scenarios';
import committedCase from '../../../../opm/compositional/spe5/case.json';
import { comp_spe5_wag } from './comp_spe5_wag';

const base = comp_spe5_wag.params.compositional as CompositionalCaseConfig;

describe('comp_spe5_wag', () => {
    it('is defined and resolvable, but withheld until #93 and its reference land', () => {
        expect(listScenarios().map((scenario) => scenario.key)).not.toContain('comp_spe5_wag');
        expect(getScenario('comp_spe5_wag')).toBe(comp_spe5_wag);
        expect(comp_spe5_wag.catalog.group).toBe('published-benchmark-decks');
        expect(comp_spe5_wag.chartLayoutKey).toBe('compositional');
    });

    it('matches the committed native/offline payload', () => {
        // The Rust replay and the eventual Jutul reference read this file; if the scenario moves,
        // it has to be re-exported deliberately rather than drifting from the validated payload.
        expect(committedCase).toEqual(base);
    });

    it('runs the six-component SPE5 fluid on a scheduled case', () => {
        expect(comp_spe5_wag.params.fluidModel).toBe('compositional');
        expect(base.schema).toBe(COMPOSITIONAL_SCHEDULE_CASE_SCHEMA);
        expect(base.fluid).toBe('pinned-spe5');
        expect(base.initial_composition).toHaveLength(6);
        expect(base.schedule?.length).toBeGreaterThan(0);
    });

    it('transcribes the 7×7×3 deck grid, layer thicknesses and directional permeability', () => {
        expect(base.grid.cells).toBe(7 * 7 * 3);
        expect(base.grid.box_grid).toMatchObject({ nx: 7, ny: 7, nz: 3 });
        expect(base.grid.dx_m).toBeCloseTo(152.4, 9);
        expect(base.grid.dy_m).toBeCloseTo(152.4, 9);
        expect(base.grid.box_grid?.layer_dz_m).toEqual([6.096, 9.144, 15.24]);
        expect(base.grid.box_grid?.layer_kx_md).toEqual([500, 50, 200]);
        expect(base.grid.box_grid?.layer_ky_md).toEqual([500, 50, 200]);
        expect(base.grid.box_grid?.layer_kz_md).toEqual([50, 50, 25]);
        expect(base.grid.porosity).toBeCloseTo(0.3, 12);
        expect(base.gravity_enabled).toBe(true);
        expect(base.initial_pressure_gradient_bar_per_m).toBeCloseTo(0.054611196, 12);
    });

    it('starts on the paper oil and injects the paper gas', () => {
        expect(base.initial_composition).toEqual([0.5, 0.03, 0.07, 0.2, 0.15, 0.05]);
        expect(base.initial_composition.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 12);
        const gas = base.wells.find((well) => well.id === 'INJG');
        expect(gas?.injection_composition).toEqual([0.77, 0.2, 0.03, 0, 0, 0]);
        expect(gas?.injection_composition?.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 12);
    });

    it('carries the deck SWOF/SGOF on Stone II, with zero capillary pressure', () => {
        expect(base.relperm.model).toBe('stone-ii');
        if (base.relperm.model !== 'stone-ii') return;
        const { swof, sgof } = base.relperm;
        // The union of the raw table knots and the transformed SOF3 knots; no original
        // breakpoint may be dropped.
        expect(swof).toHaveLength(14);
        expect(sgof).toHaveLength(14);
        for (let i = 1; i < swof.length; i += 1) {
            expect(swof[i].sw).toBeGreaterThan(swof[i - 1].sw);
        }
        for (let i = 1; i < sgof.length; i += 1) {
            expect(sgof[i].sg).toBeGreaterThan(sgof[i - 1].sg);
        }
        // Every stated knot survives: the raw SWFN/SGFN nodes and the transformed SOF3 nodes.
        const swfn = [0.2, 0.2899, 0.3778, 0.4667, 0.5556, 0.6444, 0.7, 0.7333, 0.8222, 0.9111, 1];
        const sgfn = [0, 0.05, 0.0889, 0.1778, 0.2667, 0.3556, 0.4443, 0.5333, 0.6222, 0.65, 0.7111, 0.8];
        const sof3 = [0, 0.0889, 0.15, 0.1778, 0.2667, 0.3, 0.3556, 0.4444, 0.5333, 0.6222, 0.7111, 0.75, 0.8];
        for (const sw of [...swfn, ...sof3.map((so) => 1 - so)]) {
            expect(swof.some((row) => Math.abs(row.sw - sw) < 1e-12), `Sw = ${sw}`).toBe(true);
        }
        for (const sg of [...sgfn, ...sof3.map((so) => 0.8 - so)]) {
            expect(sgof.some((row) => Math.abs(row.sg - sg) < 1e-12), `Sg = ${sg}`).toBe(true);
        }
        expect(swof[0]).toMatchObject({ sw: 0.2, krw: 0, krow: 1, pcow: 0 });
        expect(swof[swof.length - 1]).toMatchObject({ sw: 1, krw: 1, krow: 0 });
        expect(sgof[0]).toMatchObject({ sg: 0, krg: 0, krog: 1, pcog: 0 });
        expect(sgof[sgof.length - 1]).toMatchObject({ sg: 0.8, krg: 1, krog: 0 });
        expect(swof.every((row) => row.pcow === 0)).toBe(true);
        expect(sgof.every((row) => row.pcog === 0)).toBe(true);
    });

    it('carries the connate water and PVTW record', () => {
        expect(base.water).toBeDefined();
        expect(base.water?.initial_saturation).toBeCloseTo(0.2, 12);
        expect(base.water?.viscosity_cp).toBeCloseTo(0.7, 12);
        expect(base.water?.formation_volume_factor).toBeCloseTo(1.0, 12);
    });

    it('places the producer and both injectors on the deck cells as surface-rate wells', () => {
        expect(base.wells.map((well) => well.id)).toEqual(['PROD', 'INJW', 'INJG']);
        const producer = base.wells.find((well) => well.id === 'PROD');
        expect(producer?.completions[0].cell).toBe(7 * 7 * 3 - 1);
        expect(producer?.control).toBe('surface-rate');
        if (producer?.control === 'surface-rate') {
            expect(producer.phase).toBe('liquid');
            expect(producer.target_m3_per_day).toBeLessThan(0);
            expect(producer.bhp_limit_bar).toBeCloseTo(1000 * 0.0689476, 9);
        }
        for (const id of ['INJW', 'INJG']) {
            const injector = base.wells.find((well) => well.id === id);
            expect(injector?.completions[0].cell, id).toBe(0);
            expect(injector?.control, id).toBe('shut-in');
        }
        expect(base.wells.find((well) => well.id === 'INJW')?.inject_water).toBe(true);
    });

    it('schedules two years of production, then nine one-year water/gas pairs', () => {
        const events = base.schedule ?? [];
        expect(events).toHaveLength(36);
        expect(events[0].time_days).toBeCloseTo(2 * 365.25, 9);
        expect(events[0].well).toBe('INJW');
        expect(events[1].time_days).toBeCloseTo(2 * 365.25, 9);
        expect(events[1].well).toBe('INJG');
        expect(events[2].time_days).toBeCloseTo(3 * 365.25, 9);
        expect(events[2].well).toBe('INJW');
        expect(events[3].time_days).toBeCloseTo(3 * 365.25, 9);
        expect(events[3].well).toBe('INJG');
        // The last event opens the gas injector for year twenty; there is no blowdown event.
        expect(events[events.length - 1].time_days).toBeCloseTo(19 * 365.25, 9);
        expect(events[events.length - 1].well).toBe('INJG');
        expect(comp_spe5_wag.params.steps).toBe(240);
        expect(Number(comp_spe5_wag.params.delta_t_days) * 240).toBeCloseTo(20 * 365.25, 4);
    });

    it('declares no analytical overlay and cites the case and its validation', () => {
        expect(comp_spe5_wag.capabilities.analyticalMethod).toBe('none');
        expect(comp_spe5_wag.analyticalDef).toBeUndefined();
        expect(comp_spe5_wag.analyticalMethodReference).toMatch(/SPE 16000/);
        expect(comp_spe5_wag.analyticalMethodReference).toMatch(/Open Database License/i);
        expect(comp_spe5_wag.analyticalMethodReference).toMatch(/SPE5_VALIDATION/);
    });
});
