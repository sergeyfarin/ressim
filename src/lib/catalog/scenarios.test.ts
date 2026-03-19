import { describe, expect, it } from 'vitest';
import { getScenario, getScenarioWithVariantParams } from './scenarios';

describe('sweep scenario sensitivities', () => {
    it('provides analytical method metadata for every canonical scenario', () => {
        const scenarioKeys = [
            'wf_bl1d',
            'sweep_areal',
            'sweep_vertical',
            'sweep_combined',
            'dep_pss',
            'dep_decline',
            'gas_injection',
            'gas_drive',
        ];

        for (const key of scenarioKeys) {
            const scenario = getScenario(key);
            expect(scenario?.analyticalMethodSummary?.length).toBeGreaterThan(10);
            expect(scenario?.analyticalMethodReference?.length).toBeGreaterThan(5);
        }
    });

    it('adds a seeded areal heterogeneity axis for the areal sweep scenario', () => {
        const scenario = getScenario('sweep_areal');

        expect(scenario?.defaultSensitivityDimensionKey).toBe('mobility');
        expect(scenario?.sensitivities.map((dimension) => dimension.key)).toEqual([
            'mobility',
            'areal_heterogeneity',
            'sor',
        ]);

        const arealAxis = scenario?.sensitivities.find((dimension) => dimension.key === 'areal_heterogeneity');
        expect(arealAxis?.variants.map((variant) => variant.key)).toEqual([
            'areal_uniform',
            'areal_mild_random',
            'areal_strong_random',
        ]);
        expect(arealAxis?.variants.every((variant) => variant.affectsAnalytical === false)).toBe(true);

        const randomParams = getScenarioWithVariantParams('sweep_areal', 'areal_heterogeneity', 'areal_mild_random');
        expect(randomParams).toMatchObject({
            permMode: 'random',
            minPerm: 120,
            maxPerm: 280,
            useRandomSeed: true,
            randomSeed: 4201,
        });
    });

    it('treats vertical heterogeneity variants as analytically varying sweep overlays', () => {
        const scenario = getScenario('sweep_vertical');
        const heterogeneityAxis = scenario?.sensitivities.find((dimension) => dimension.key === 'heterogeneity');

        expect(heterogeneityAxis?.variants.map((variant) => variant.affectsAnalytical)).toEqual([
            true,
            true,
            true,
        ]);
    });

    it('splits the combined sweep scenario into interaction and penalty-buildup axes', () => {
        const scenario = getScenario('sweep_combined');

        expect(scenario?.defaultSensitivityDimensionKey).toBe('interaction_core');
        expect(scenario?.sensitivities.map((dimension) => dimension.key)).toEqual([
            'interaction_core',
            'penalty_buildup',
        ]);

        const interactionAxis = scenario?.sensitivities.find((dimension) => dimension.key === 'interaction_core');
        expect(interactionAxis?.variants.map((variant) => variant.key)).toEqual([
            'interaction_favorable_uniform',
            'interaction_unfavorable_uniform',
            'interaction_favorable_layered',
            'interaction_unfavorable_layered',
        ]);
        expect(interactionAxis?.variants.every((variant) => variant.affectsAnalytical)).toBe(true);

        const penaltyAxis = scenario?.sensitivities.find((dimension) => dimension.key === 'penalty_buildup');
        expect(penaltyAxis?.variants.map((variant) => variant.key)).toEqual([
            'buildup_ideal',
            'buildup_vertical_only',
            'buildup_full_heterogeneity',
            'buildup_all_penalties',
        ]);
        expect(penaltyAxis?.variants.every((variant) => variant.affectsAnalytical === false)).toBe(true);

        const interactionParams = getScenarioWithVariantParams(
            'sweep_combined',
            'interaction_core',
            'interaction_unfavorable_uniform',
        );
        expect(interactionParams).toMatchObject({
            mu_o: 5,
            permMode: 'uniform',
        });

        const buildupParams = getScenarioWithVariantParams(
            'sweep_combined',
            'penalty_buildup',
            'buildup_full_heterogeneity',
        );
        expect(buildupParams).toMatchObject({
            permMode: 'random',
            minPerm: 40,
            maxPerm: 500,
            useRandomSeed: true,
            randomSeed: 4301,
        });
    });
});