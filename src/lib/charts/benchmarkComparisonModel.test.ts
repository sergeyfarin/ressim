import { describe, expect, it } from 'vitest';
import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { computeWelgeMetrics } from '../analytical/fractionalFlow';
import { getBenchmarkFamily, getBenchmarkVariantsForFamily } from '../catalog/caseCatalog';
import { buildBenchmarkRunResult, buildBenchmarkRunSpecs } from '../benchmarkRunModel';
import { buildBenchmarkComparisonModel } from './benchmarkComparisonModel';

function buildSyntheticWaterfloodRateHistory(
    params: Record<string, any>,
    breakthroughPvi: number,
    watercutShift = 0,
) {
    const poreVolume = Number(params.nx)
        * Number(params.ny)
        * Number(params.nz)
        * Number(params.cellDx)
        * Number(params.cellDy)
        * Number(params.cellDz)
        * Number(params.reservoirPorosity ?? 0.2);

    const firstInjection = 0.4 * breakthroughPvi * poreVolume;
    const secondInjection = 0.6 * breakthroughPvi * poreVolume;
    const thirdInjection = 0.6 * breakthroughPvi * poreVolume;

    return [
        {
            time: 1,
            total_injection: firstInjection,
            total_production_liquid: 100,
            total_production_oil: 100,
            avg_reservoir_pressure: 285,
            avg_water_saturation: 0.18 + watercutShift,
        },
        {
            time: 2,
            total_injection: secondInjection,
            total_production_liquid: 100,
            total_production_oil: 99,
            avg_reservoir_pressure: 270,
            avg_water_saturation: 0.24 + watercutShift,
        },
        {
            time: 3,
            total_injection: thirdInjection,
            total_production_liquid: 100,
            total_production_oil: 75 - watercutShift * 10,
            avg_reservoir_pressure: 255,
            avg_water_saturation: 0.34 + watercutShift,
        },
    ];
}

function buildDepletionReferenceRateHistory(params: Record<string, any>) {
    const timeHistory = [5, 10, 20];
    const reference = calculateDepletionAnalyticalProduction({
        reservoir: {
            length: Number(params.nx) * Number(params.cellDx),
            area: Number(params.ny) * Number(params.cellDy) * Number(params.nz) * Number(params.cellDz),
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
        cellDz: Number(params.cellDz),
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
    });

    return reference.production.map((point) => ({
        time: point.time,
        total_injection: 0,
        total_production_liquid: point.oilRate,
        total_production_oil: point.oilRate,
        avg_reservoir_pressure: point.avgPressure,
    }));
}

describe('benchmarkComparisonModel', () => {
    it('builds BL overlay panels with base-first ordering and analytical references', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const variants = getBenchmarkVariantsForFamily('bl_case_a_refined');
        const [baseSpec, gridVariantSpec] = buildBenchmarkRunSpecs(family!, [variants[0]]);

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

        const baseResult = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory: buildSyntheticWaterfloodRateHistory(baseSpec.params, reference.breakthroughPvi, 0),
        });
        const gridVariantResult = buildBenchmarkRunResult({
            spec: gridVariantSpec,
            rateHistory: buildSyntheticWaterfloodRateHistory(gridVariantSpec.params, reference.breakthroughPvi * 1.08, 0.03),
        });

        const model = buildBenchmarkComparisonModel({
            family,
            results: [gridVariantResult, baseResult],
            xAxisMode: 'pvi',
        });

        expect(model.orderedResults[0].variantKey).toBeNull();
        expect(model.panels.rates.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining([
                `${baseResult.label} Water Cut`,
                `${gridVariantResult.label} Water Cut`,
                'Analytical Water Cut',
            ]),
        );
        expect(model.panels.cumulative.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining(['Analytical Recovery']),
        );
        expect(model.panels.diagnostics.curves).toHaveLength(2);
    });

    it('builds depletion overlay panels with analytical oil-rate and pressure references', () => {
        const family = getBenchmarkFamily('dietz_sq_center');
        const [baseSpec] = buildBenchmarkRunSpecs(family!);
        const result = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory: buildDepletionReferenceRateHistory(baseSpec.params),
        });

        const model = buildBenchmarkComparisonModel({
            family,
            results: [result],
            xAxisMode: 'tD',
        });

        expect(model.panels.rates.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining([`${result.label} Oil Rate`, 'Analytical Oil Rate']),
        );
        expect(model.panels.diagnostics.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining([`${result.label} Avg Pressure`, 'Analytical Avg Pressure']),
        );
        expect(model.panels.rates.series.at(-1)?.at(-1)?.x).toBeGreaterThan(0);
    });

    it('uses theme-aware analytical reference colors so overlays stay visible in both themes', () => {
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
        const baseResult = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory: buildSyntheticWaterfloodRateHistory(baseSpec.params, reference.breakthroughPvi, 0),
        });

        const darkModel = buildBenchmarkComparisonModel({
            family,
            results: [baseResult],
            xAxisMode: 'pvi',
            theme: 'dark',
        });
        const lightModel = buildBenchmarkComparisonModel({
            family,
            results: [baseResult],
            xAxisMode: 'pvi',
            theme: 'light',
        });

        const darkReferenceCurve = darkModel.panels.rates.curves.find((curve) => curve.label === 'Analytical Water Cut');
        const lightReferenceCurve = lightModel.panels.rates.curves.find((curve) => curve.label === 'Analytical Water Cut');

        expect(darkReferenceCurve?.color).toBe('#f8fafc');
        expect(lightReferenceCurve?.color).toBe('#0f172a');
    });
});