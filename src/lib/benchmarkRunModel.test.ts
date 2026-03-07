import { describe, expect, it } from 'vitest';
import { computeWelgeMetrics } from './analytical/fractionalFlow';
import { getBenchmarkFamily, getBenchmarkVariantsForFamily } from './catalog/caseCatalog';
import {
    buildBenchmarkRunResult,
    buildBenchmarkRunSpecs,
    resolveBenchmarkReferenceComparisons,
} from './benchmarkRunModel';

function buildSyntheticRateHistory(params: Record<string, any>, breakthroughPvi: number, watercut = 0.01) {
    const poreVolume = Number(params.nx)
        * Number(params.ny)
        * Number(params.nz)
        * Number(params.cellDx)
        * Number(params.cellDy)
        * Number(params.cellDz)
        * Number(params.reservoirPorosity ?? 0.2);

    return [
        {
            time: 1,
            total_injection: breakthroughPvi * poreVolume,
            total_production_liquid: 100,
            total_production_oil: 100 * (1 - watercut),
            avg_reservoir_pressure: 250,
        },
    ];
}

describe('benchmarkRunModel', () => {
    it('builds deterministic base-plus-variant run specs for BL families', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const variants = getBenchmarkVariantsForFamily('bl_case_a_refined');

        expect(family).not.toBeNull();
        const specs = buildBenchmarkRunSpecs(family!, variants);

        expect(specs).toHaveLength(7);
        expect(specs[0]).toMatchObject({
            familyKey: 'bl_case_a_refined',
            variantKey: null,
            caseKey: 'bl_case_a_refined',
        });
        expect(specs[1]).toMatchObject({
            variantKey: 'grid_24',
            familyKey: 'bl_case_a_refined',
        });
    });

    it('scores analytical breakthrough comparison for homogeneous BL runs', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const [baseSpec] = buildBenchmarkRunSpecs(family!);

        const reference = computeWelgeMetrics(
            {
                s_wc: Number(baseSpec.params.s_wc),
                s_or: Number(baseSpec.params.s_or),
                n_w: Number(baseSpec.params.n_w),
                n_o: Number(baseSpec.params.n_o),
                k_rw_max: Number(baseSpec.params.k_rw_max),
                k_ro_max: Number(baseSpec.params.k_ro_max),
            },
            {
                mu_w: Number(baseSpec.params.mu_w),
                mu_o: Number(baseSpec.params.mu_o),
            },
            Number(baseSpec.params.initialSaturation),
        );

        const result = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory: buildSyntheticRateHistory(baseSpec.params, reference.breakthroughPvi),
        });

        expect(result.breakthroughPvi).toBeCloseTo(reference.breakthroughPvi, 6);
        expect(result.referenceComparison.status).toBe('within-tolerance');
        expect(result.referenceComparison.referenceKind).toBe('analytical');
    });

    it('resolves numerical-reference comparisons for heterogeneous BL variants from the base run', () => {
        const family = getBenchmarkFamily('bl_case_b_refined');
        const variants = getBenchmarkVariantsForFamily('bl_case_b_refined');
        const specs = buildBenchmarkRunSpecs(family!, variants);
        const baseSpec = specs[0];
        const heterogeneitySpec = specs.find((spec) => spec.variantKey === 'heterogeneity_strong_random');

        const baseResult = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory: buildSyntheticRateHistory(baseSpec.params, 1.0),
        });
        const heterogeneityResult = buildBenchmarkRunResult({
            spec: heterogeneitySpec!,
            rateHistory: buildSyntheticRateHistory(heterogeneitySpec!.params, 1.5),
        });

        const resolved = resolveBenchmarkReferenceComparisons([baseResult, heterogeneityResult]);
        const resolvedHeterogeneity = resolved.find((result) => result.variantKey === 'heterogeneity_strong_random');

        expect(resolvedHeterogeneity?.referenceComparison.referenceKind).toBe('numerical-refined');
        expect(resolvedHeterogeneity?.referenceComparison.referenceValue).toBeCloseTo(1.0, 6);
        expect(resolvedHeterogeneity?.referenceComparison.status).toBe('outside-tolerance');
    });
});