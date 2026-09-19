import { describe, expect, it } from 'vitest';

import { COMPOSITIONAL_CASE_SCHEMA } from '../../compositional/types';
import type { CompositionalCaseConfig } from '../../compositional/types';
import { getScenario } from '../scenarios';
import { comp_co2_1d } from './comp_co2_1d';

const base = comp_co2_1d.params.compositional as CompositionalCaseConfig;

function patchedCase(dimension: string, variant: string): CompositionalCaseConfig | undefined {
    const dim = comp_co2_1d.sensitivities.find((d) => d.key === dimension);
    const found = dim?.variants.find((v) => v.key === variant);
    return found?.paramPatch.compositional as CompositionalCaseConfig | undefined;
}

describe('comp_co2_1d', () => {
    it('is defined but withheld from the picker', () => {
        // It is reachable by key — its engine path works — but it is not offered, because
        // `buildChartData` cannot yet source compositional curves and every panel would be empty.
        // See WITHHELD_SCENARIOS in scenarios.ts.
        expect(getScenario('comp_co2_1d')).toBeTruthy();
        expect(comp_co2_1d.catalog.group).toBe('published-benchmark-decks');
    });

    it('carries a compositional case behind the fluid-model discriminator', () => {
        expect(comp_co2_1d.params.fluidModel).toBe('compositional');
        expect(base.schema).toBe(COMPOSITIONAL_CASE_SCHEMA);
        expect(base.fluid).toBe('pinned-ternary');
    });

    it('reproduces the deck: five cells over 300 m at 100 mD and 10% porosity', () => {
        expect(base.grid.cells).toBe(5);
        expect(base.grid.cells * base.grid.dx_m).toBeCloseTo(300, 9);
        expect(base.grid.permeability_md).toBe(100);
        expect(base.grid.porosity).toBeCloseTo(0.1, 12);
        // `ROCK 68.9476 0` — incompressible.
        expect(base.grid.rock_compressibility_per_bar).toBe(0);
        expect(base.initial_pressure_bar).toBe(75);
    });

    it('uses the deck ZMF and injects pure CO2', () => {
        expect(base.initial_composition).toEqual([0.1, 0.3, 0.6]);
        expect(base.initial_composition.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 12);
        const injector = base.wells.find((w) => w.id === 'INJ');
        expect(injector?.injection_composition).toEqual([1, 0, 0]);
        expect(base.wells.find((w) => w.id === 'PROD')?.injection_composition).toBeUndefined();
    });

    it('transcribes the deck SGOF table, reversed onto liquid saturation', () => {
        expect(base.relperm.model).toBe('tabulated');
        if (base.relperm.model !== 'tabulated') return;
        const { liquid_saturation, kr_liquid, kr_vapour } = base.relperm;
        expect(liquid_saturation).toHaveLength(21);
        // Strictly increasing in liquid saturation, which is what the engine's table requires.
        for (let i = 1; i < liquid_saturation.length; i += 1) {
            expect(liquid_saturation[i]).toBeGreaterThan(liquid_saturation[i - 1]);
        }
        // Spot-check against the deck's own rows (Sg, Krg, Kro).
        for (const [sg, krg, kro] of [
            [0, 0, 1],
            [0.25, 0.0625, 0.5625],
            [0.5, 0.25, 0.25],
            [1, 1, 0],
        ]) {
            const index = liquid_saturation.findIndex((sl) => Math.abs(sl - (1 - sg)) < 1e-9);
            expect(index, `Sg = ${sg} is missing`).toBeGreaterThanOrEqual(0);
            expect(kr_vapour[index]).toBeCloseTo(krg, 12);
            expect(kr_liquid[index]).toBeCloseTo(kro, 12);
        }
    });

    it('is a sourced relperm model, which is why it may ship', () => {
        // The engine flags `linear` as verification-only and COMPOSITIONAL_VALIDATION.md section 6
        // says not to ship on it. This case uses the deck's own table instead.
        expect(base.relperm.model).not.toBe('linear');
    });

    it('refines the grid without changing the reservoir', () => {
        for (const [variant, cells] of [
            ['grid_5', 5],
            ['grid_10', 10],
            ['grid_20', 20],
            ['grid_40', 40],
        ] as const) {
            const config = patchedCase('grid_refinement', variant);
            expect(config, variant).toBeDefined();
            if (!config) continue;
            expect(config.grid.cells, variant).toBe(cells);
            // The same 300 m of reservoir, however it is cut up.
            expect(config.grid.cells * config.grid.dx_m, variant).toBeCloseTo(300, 9);
            expect(config.grid.permeability_md, variant).toBe(base.grid.permeability_md);
            expect(config.initial_composition, variant).toEqual(base.initial_composition);
            expect(config.relperm, variant).toEqual(base.relperm);
            // The producer moves to the far end, and only there.
            expect(config.wells.find((w) => w.id === 'PROD')?.completions[0].cell, variant)
                .toBe(cells - 1);
            expect(config.wells.find((w) => w.id === 'INJ')?.completions[0].cell, variant).toBe(0);
        }
    });

    it('raises the well index as the grid refines, because the equivalent radius shrinks', () => {
        const coarse = patchedCase('grid_refinement', 'grid_5');
        const fine = patchedCase('grid_refinement', 'grid_40');
        const wi = (c?: CompositionalCaseConfig) => c?.wells[0].completions[0].well_index ?? 0;
        expect(wi(fine)).toBeGreaterThan(wi(coarse));
    });

    it('keeps its timestep ladder covering the same span of days', () => {
        const dim = comp_co2_1d.sensitivities.find((d) => d.key === 'timestep');
        expect(dim).toBeDefined();
        for (const variant of dim?.variants ?? []) {
            const dt = variant.paramPatch.delta_t_days as number;
            const steps = variant.paramPatch.steps as number;
            expect(dt * steps, variant.key).toBeCloseTo(20, 9);
        }
    });

    it('declares no analytical overlay, and says why', () => {
        expect(comp_co2_1d.capabilities.analyticalMethod).toBe('none');
        expect(comp_co2_1d.analyticalDef).toBeUndefined();
        // The reason is the point: a Buckley-Leverett curve assumes what this case violates.
        expect(comp_co2_1d.analyticalMethodSummary).toMatch(/no closed-form|Buckley/i);
    });

    it('cites the deck it comes from and the validation it rests on', () => {
        expect(comp_co2_1d.analyticalMethodReference).toMatch(/1D_COMP\.DATA/);
        expect(comp_co2_1d.analyticalMethodReference).toMatch(/Open Database License/i);
        expect(comp_co2_1d.analyticalMethodReference).toMatch(/COMPOSITIONAL_VALIDATION/);
    });
});
