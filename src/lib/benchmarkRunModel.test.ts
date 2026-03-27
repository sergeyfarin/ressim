import { describe, expect, it } from 'vitest';
import { calculateDepletionAnalyticalProduction } from './analytical/depletionAnalytical';
import { computeWelgeMetrics } from './analytical/fractionalFlow';
import { getBenchmarkFamily, getBenchmarkVariantsForFamily } from './catalog/caseCatalog';
import {
    buildBenchmarkCreatePayload,
    buildBenchmarkRunResult,
    buildBenchmarkRunSpecs,
    resolveBenchmarkReferenceComparisons,
} from './benchmarkRunModel';

function getTotalThickness(params: Record<string, any>) {
    if (Array.isArray(params.cellDzPerLayer) && params.cellDzPerLayer.length > 0) {
        return params.cellDzPerLayer.map(Number).reduce((sum: number, thickness: number) => sum + thickness, 0);
    }
    return Number(params.nz) * Number(params.cellDz);
}

function getAverageLayerThickness(params: Record<string, any>) {
    return getTotalThickness(params) / Number(params.nz);
}

function buildSyntheticRateHistory(params: Record<string, any>, breakthroughPvi: number, watercut = 0.01) {
    const poreVolume = Number(params.nx)
        * Number(params.ny)
        * Number(params.cellDx)
        * Number(params.cellDy)
        * getTotalThickness(params)
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

function buildDepletionReferenceRateHistory(params: Record<string, any>) {
    const timeHistory = [5, 10, 20];
    const reference = calculateDepletionAnalyticalProduction({
        reservoir: {
            length: Number(params.nx) * Number(params.cellDx),
            area: Number(params.ny) * Number(params.cellDy) * getTotalThickness(params),
            porosity: Number(params.reservoirPorosity ?? 0.2),
        },
        timeHistory,
        initialSaturation: Number(params.initialSaturation ?? 0.3),
        nz: Number(params.nz),
        permMode: String(params.permMode ?? 'uniform'),
        uniformPermX: Number(params.uniformPermX ?? 100),
        uniformPermY: Number(params.uniformPermY ?? params.uniformPermX ?? 100),
        layerPermsX: Array.isArray(params.layerPermsX) ? params.layerPermsX.map(Number) : [],
        layerPermsY: Array.isArray(params.layerPermsY) ? params.layerPermsY.map(Number) : [],
        cellDx: Number(params.cellDx),
        cellDy: Number(params.cellDy),
        cellDz: getAverageLayerThickness(params),
        wellRadius: Number(params.well_radius ?? 0.1),
        wellSkin: Number(params.well_skin ?? 0),
        muO: Number(params.mu_o ?? 1),
        sWc: Number(params.s_wc ?? 0.1),
        sOr: Number(params.s_or ?? 0.1),
        nO: Number(params.n_o ?? 2),
        c_o: Number(params.c_o ?? 1e-5),
        c_w: Number(params.c_w ?? 3e-6),
        cRock: Number(params.rock_compressibility ?? 1e-6),
        initialPressure: Number(params.initialPressure),
        producerBhp: Number(params.producerBhp),
        depletionRateScale: Number(params.analyticalDepletionRateScale ?? 1),
        arpsB: Number(params.analyticalArpsB ?? 0),
        nx: params.nx != null ? Number(params.nx) : undefined,
        ny: params.ny != null ? Number(params.ny) : undefined,
        producerI: params.producerI != null ? Number(params.producerI) : undefined,
        producerJ: params.producerJ != null ? Number(params.producerJ) : undefined,
    });

    return reference.production.map((point) => ({
        time: point.time,
        total_injection: 0,
        total_production_liquid: point.oilRate,
        total_production_oil: point.oilRate,
        avg_reservoir_pressure: point.avgPressure,
    }));
}

describe('benchmarkRunModel', () => {
    it('forces benchmark runtime payloads onto the non-FIM path for frontend stability', () => {
        const payload = buildBenchmarkCreatePayload({
            nx: 4,
            ny: 1,
            nz: 1,
            reservoirPorosity: 0.2,
            injectorControlMode: 'pressure',
            producerControlMode: 'pressure',
        });

        expect(payload.fimEnabled).toBe(false);
    });

    it('builds deterministic base-plus-variant run specs for BL families', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const variants = getBenchmarkVariantsForFamily('bl_case_a_refined');

        expect(family).not.toBeNull();
        const specs = buildBenchmarkRunSpecs(family!, variants);

        expect(specs).toHaveLength(9);
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

    it('builds deterministic base-plus-selected-variant specs without reintroducing unselected variants', () => {
        const family = getBenchmarkFamily('bl_case_b_refined');
        const variants = getBenchmarkVariantsForFamily('bl_case_b_refined');
        const selected = [
            variants.find((variant) => variant.variantKey === 'heterogeneity_mild_random')!,
            variants.find((variant) => variant.variantKey === 'dt_0_25')!,
        ];

        const specs = buildBenchmarkRunSpecs(family!, selected);

        expect(specs.map((spec) => spec.variantKey)).toEqual([
            null,
            'heterogeneity_mild_random',
            'dt_0_25',
        ]);
        expect(specs).toHaveLength(3);
        expect(specs.every((spec) => spec.familyKey === 'bl_case_b_refined')).toBe(true);
    });

    it('scores reference-solution arrival comparison for homogeneous BL runs', () => {
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
        expect(result.referencePolicy.referenceLabel).toContain('Buckley-Leverett reference solution');
        expect(result.referencePolicy.analyticalOverlayRole).toBe('primary');
        expect(result.referenceComparison.status).toBe('within-tolerance');
        expect(result.referenceComparison.referenceKind).toBe('analytical');
        expect(result.comparisonOutputs.breakthroughShiftPvi).toBeCloseTo(0, 6);
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

        expect(resolvedHeterogeneity?.referencePolicy.referenceLabel).toContain('Refined numerical reference');
        expect(resolvedHeterogeneity?.referencePolicy.analyticalOverlayRole).toBe('secondary');
        expect(resolvedHeterogeneity?.referenceComparison.referenceKind).toBe('numerical-refined');
        expect(resolvedHeterogeneity?.referenceComparison.referenceValue).toBeCloseTo(1.0, 6);
        expect(resolvedHeterogeneity?.referenceComparison.status).toBe('outside-tolerance');
        expect(resolvedHeterogeneity?.comparisonOutputs.breakthroughShiftPvi).toBeCloseTo(0.5, 6);
    });

    it('keeps depletion references on an explicit reference-solution policy with trend diagnostics', () => {
        const family = getBenchmarkFamily('dietz_sq_center');
        const [baseSpec] = buildBenchmarkRunSpecs(family!);
        const result = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory: buildDepletionReferenceRateHistory(baseSpec.params),
        });

        expect(result.analyticalMethod).toBe('depletion');
        expect(result.referencePolicy.referenceLabel).toContain('Depletion reference solution');
        expect(result.referencePolicy.analyticalOverlayRole).toBe('primary');
        expect(result.referenceComparison.status).toBe('not-applicable');
        expect(result.comparisonOutputs.oilRateRelativeErrorAtFinalTime).toBeCloseTo(0, 6);
        expect(result.comparisonOutputs.cumulativeOilRelativeErrorAtFinalTime).toBeGreaterThanOrEqual(0);
        expect(result.comparisonOutputs.cumulativeOilRelativeErrorAtFinalTime).toBeLessThan(1);
    });

    it('snapshots params and rate history so later mutations cannot corrupt stored runs', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const [baseSpec] = buildBenchmarkRunSpecs(family!);
        const rateHistory = buildSyntheticRateHistory(baseSpec.params, 1.0);
        const result = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory,
        });

        rateHistory[0].time = 999;
        rateHistory[0].total_production_oil = 0;
        baseSpec.params.mu_o = 999;

        expect(result.rateHistory[0]?.time).toBe(1);
        expect(result.rateHistory[0]?.total_production_oil).not.toBe(0);
        expect(result.params.mu_o).not.toBe(999);
    });

    it('uses per-layer dz when computing pore-volume-based benchmark series', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const [baseSpec] = buildBenchmarkRunSpecs(family!);
        const spec = {
            ...baseSpec,
            params: {
                ...baseSpec.params,
                nx: 1,
                ny: 1,
                nz: 3,
                cellDx: 10,
                cellDy: 10,
                cellDz: 1,
                cellDzPerLayer: [1, 2, 3],
                reservoirPorosity: 0.2,
                initialSaturation: 0.25,
            },
        };

        const poreVolume = 1 * 1 * 10 * 10 * 6 * 0.2;
        const oilRate = 18;
        const result = buildBenchmarkRunResult({
            spec,
            rateHistory: [{
                time: 1,
                total_injection: poreVolume,
                total_production_liquid: oilRate,
                total_production_oil: oilRate,
                avg_reservoir_pressure: 250,
            }],
        });

        expect(result.pviSeries[0]).toBeCloseTo(1, 10);
        expect(result.recoverySeries[0]).toBeCloseTo(oilRate / (poreVolume * (1 - 0.25)), 10);
    });
});