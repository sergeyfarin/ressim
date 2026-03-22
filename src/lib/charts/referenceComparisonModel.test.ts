import { describe, expect, it } from 'vitest';
import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { computeWelgeMetrics } from '../analytical/fractionalFlow';
import { getBenchmarkFamily, getBenchmarkVariantsForFamily } from '../catalog/caseCatalog';
import { buildBenchmarkRunResult, buildBenchmarkRunSpecs } from '../benchmarkRunModel';
import type { SimulatorSnapshot } from '../simulator-types';
import { buildReferenceComparisonModel } from './referenceComparisonModel';

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

function buildSweepSnapshots(params: Record<string, any>, counts: number[]): SimulatorSnapshot[] {
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const nz = Number(params.nz);
    const total = nx * ny * nz;
    const swc = Number(params.s_wc ?? 0.1);
    const pressure = Number(params.initialPressure ?? 300);

    return counts.map((sweptCount, index) => {
        const satWater = new Float64Array(total).fill(swc);
        const satOil = new Float64Array(total).fill(Math.max(0, 1 - swc));
        const satGas = new Float64Array(total).fill(0);
        for (let cell = 0; cell < Math.min(total, sweptCount); cell += 1) {
            satWater[cell] = swc + 0.2;
            satOil[cell] = Math.max(0, 1 - satWater[cell]);
        }

        return {
            time: index + 1,
            grid: {
                pressure: new Float64Array(total).fill(pressure),
                sat_water: satWater,
                sat_oil: satOil,
                sat_gas: satGas,
            },
            wells: [],
        };
    });
}

function buildSweepRunResult(spec: ReturnType<typeof buildBenchmarkRunSpecs>[number]) {
    const rateHistory = buildSyntheticWaterfloodRateHistory(spec.params, 0.55, 0);
    const total = Number(spec.params.nx) * Number(spec.params.ny) * Number(spec.params.nz);
    const history = buildSweepSnapshots(spec.params, [1, Math.max(2, Math.floor(total / 4)), Math.max(3, Math.floor(total / 2))]);
    return buildBenchmarkRunResult({
        spec,
        rateHistory,
        history,
    });
}

describe('referenceComparisonModel', () => {
    it('preserves provided run order while still building reference-solution curves', () => {
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

        const model = buildReferenceComparisonModel({
            family,
            results: [gridVariantResult, baseResult],
            xAxisMode: 'pvi',
        });

        expect(model.orderedResults.map((result) => result.key)).toEqual([
            gridVariantResult.key,
            baseResult.key,
        ]);
        expect(model.panels.rates.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining([
                `${baseResult.label} Water Cut`,
                `${gridVariantResult.label} Water Cut`,
                'Reference Solution Water Cut',
            ]),
        );
        expect(model.panels.cumulative.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining(['Reference Solution Cum Oil']),
        );
        expect(model.panels.recovery.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining(['Reference Solution Recovery']),
        );
        expect(model.panels.diagnostics.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining([
                `${baseResult.label} Avg Pressure`,
                `${gridVariantResult.label} Avg Pressure`,
            ]),
        );
    });

    it('builds depletion overlay panels with reference-solution oil-rate and pressure curves', () => {
        const family = getBenchmarkFamily('dietz_sq_center');
        const [baseSpec] = buildBenchmarkRunSpecs(family!);
        const result = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory: buildDepletionReferenceRateHistory(baseSpec.params),
        });

        const model = buildReferenceComparisonModel({
            family,
            results: [result],
            xAxisMode: 'tD',
        });

        expect(model.panels.rates.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining([`${result.label} Oil Rate`, 'Reference Solution Oil Rate']),
        );
        expect(model.panels.diagnostics.curves.map((curve) => curve.label)).toEqual(
            expect.arrayContaining([`${result.label} Avg Pressure`, 'Reference Solution Avg Pressure']),
        );
        expect(model.axisMappingWarning).toBeNull();
        // Simulation curve (first series) should use tD, not raw time.
        // tD = time / tau; for typical depletion params tau >> 1, so tD < time.
        const simSeries = model.panels.rates.series[0];
        const lastSimX = simSeries?.at(-1)?.x ?? 0;
        expect(lastSimX).toBeGreaterThan(0);
        // With tD x-axis, the max x should be well below the raw time (which is days).
        // Raw time from buildDepletionReferenceRateHistory is steps * dt (e.g. 200 * 5 = 1000 days).
        // tD at that point should be much smaller than 1000.
        expect(lastSimX).toBeLessThan(100);
        expect(model.panels.rates.series.at(-1)?.at(-1)?.x).toBeGreaterThan(0);
    });

    it('uses theme-aware reference-solution colors so overlays stay visible in both themes', () => {
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

        const darkModel = buildReferenceComparisonModel({
            family,
            results: [baseResult],
            xAxisMode: 'pvi',
            theme: 'dark',
        });
        const lightModel = buildReferenceComparisonModel({
            family,
            results: [baseResult],
            xAxisMode: 'pvi',
            theme: 'light',
        });

        const darkReferenceCurve = darkModel.panels.rates.curves.find((curve) => curve.label === 'Reference Solution Water Cut');
        const lightReferenceCurve = lightModel.panels.rates.curves.find((curve) => curve.label === 'Reference Solution Water Cut');

        expect(darkReferenceCurve?.color).toBe('#f8fafc');
        expect(lightReferenceCurve?.color).toBe('#0f172a');
    });

    it('attaches case keys to simulated curves so charts can toggle runs independently of run-table focus', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const variants = getBenchmarkVariantsForFamily('bl_case_a_refined');
        const specs = buildBenchmarkRunSpecs(family!, [variants[0], variants[1]]);

        const reference = computeWelgeMetrics(
            {
                s_wc: Number(specs[0].params.s_wc),
                s_or: Number(specs[0].params.s_or),
                n_w: Number(specs[0].params.n_w),
                n_o: Number(specs[0].params.n_o),
                k_rw_max: Number(specs[0].params.k_rw_max),
                k_ro_max: Number(specs[0].params.k_ro_max),
            },
            {
                mu_w: Number(specs[0].params.mu_w),
                mu_o: Number(specs[0].params.mu_o),
            },
            Number(specs[0].params.initialSaturation),
        );

        const [baseSpec, firstVariantSpec, secondVariantSpec] = specs;
        const baseResult = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory: buildSyntheticWaterfloodRateHistory(baseSpec.params, reference.breakthroughPvi, 0),
        });
        const firstVariantResult = buildBenchmarkRunResult({
            spec: firstVariantSpec,
            rateHistory: buildSyntheticWaterfloodRateHistory(firstVariantSpec.params, reference.breakthroughPvi * 1.04, 0.02),
        });
        const secondVariantResult = buildBenchmarkRunResult({
            spec: secondVariantSpec,
            rateHistory: buildSyntheticWaterfloodRateHistory(secondVariantSpec.params, reference.breakthroughPvi * 1.09, 0.04),
        });

        const model = buildReferenceComparisonModel({
            family,
            results: [baseResult, firstVariantResult, secondVariantResult],
            xAxisMode: 'pvi',
        });

        const waterCutCurves = model.panels.rates.curves.filter((curve) => curve.curveKey === 'water-cut-sim');

        expect(waterCutCurves.map((curve) => curve.caseKey)).toEqual([
            baseResult.key,
            firstVariantResult.key,
            secondVariantResult.key,
        ]);
    });

    it('keeps four compared simulation curves distinct when more than three runs are shown', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const variants = getBenchmarkVariantsForFamily('bl_case_a_refined');
        const specs = buildBenchmarkRunSpecs(family!, [variants[0], variants[1], variants[2], variants[3]]);

        const reference = computeWelgeMetrics(
            {
                s_wc: Number(specs[0].params.s_wc),
                s_or: Number(specs[0].params.s_or),
                n_w: Number(specs[0].params.n_w),
                n_o: Number(specs[0].params.n_o),
                k_rw_max: Number(specs[0].params.k_rw_max),
                k_ro_max: Number(specs[0].params.k_ro_max),
            },
            {
                mu_w: Number(specs[0].params.mu_w),
                mu_o: Number(specs[0].params.mu_o),
            },
            Number(specs[0].params.initialSaturation),
        );

        const results = specs.map((spec, index) => buildBenchmarkRunResult({
            spec,
            rateHistory: buildSyntheticWaterfloodRateHistory(
                spec.params,
                reference.breakthroughPvi * (1 + index * 0.03),
                index * 0.01,
            ),
        }));

        const model = buildReferenceComparisonModel({
            family,
            results,
            xAxisMode: 'pvi',
        });

        const waterCutCurves = model.panels.rates.curves.filter((curve) => curve.curveKey === 'water-cut-sim');
        const waterCutSeries = model.panels.rates.series.filter((_, index) => model.panels.rates.curves[index]?.curveKey === 'water-cut-sim');

        expect(model.orderedResults.map((result) => result.key)).toEqual(results.map((result) => result.key));
        expect(waterCutCurves).toHaveLength(5);
        expect(new Set(waterCutCurves.map((curve) => curve.caseKey)).size).toBe(5);
        expect(waterCutSeries.every((series) => series.length > 0)).toBe(true);
        expect(model.panels.rates.curves).toHaveLength(model.panels.rates.series.length);
        expect(model.panels.cumulative.curves).toHaveLength(model.panels.cumulative.series.length);
        expect(model.panels.diagnostics.curves).toHaveLength(model.panels.diagnostics.series.length);
    });

    it('stays length-aligned for large comparison sets', () => {
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

        const results = Array.from({ length: 21 }, (_, index) => {
            const spec = {
                ...baseSpec,
                key: index === 0 ? baseSpec.key : `synthetic_${index}`,
                caseKey: index === 0 ? baseSpec.caseKey : `synthetic_${index}`,
                variantKey: index === 0 ? null : `synthetic_${index}`,
                variantLabel: index === 0 ? null : `Synthetic ${index}`,
                label: index === 0 ? baseSpec.label : `Synthetic ${index}`,
            };

            return buildBenchmarkRunResult({
                spec,
                rateHistory: buildSyntheticWaterfloodRateHistory(
                    spec.params,
                    reference.breakthroughPvi * (1 + index * 0.01),
                    index * 0.002,
                ),
            });
        });

        const model = buildReferenceComparisonModel({
            family,
            results,
            xAxisMode: 'pvi',
        });

        expect(model.orderedResults).toHaveLength(21);
        expect(model.panels.rates.curves).toHaveLength(model.panels.rates.series.length);
        expect(model.panels.cumulative.curves).toHaveLength(model.panels.cumulative.series.length);
        expect(model.panels.diagnostics.curves).toHaveLength(model.panels.diagnostics.series.length);
        expect(model.panels.rates.curves.filter((curve) => curve.curveKey === 'water-cut-sim')).toHaveLength(21);
    });

    it('remaps shared waterflood analytical overlays per completed run on time axis', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const [baseSpec] = buildBenchmarkRunSpecs(family!);
        const secondSpec = {
            ...baseSpec,
            key: 'grid_like_variant',
            caseKey: 'grid_like_variant',
            variantKey: 'grid_like_variant',
            variantLabel: 'Grid-like variant',
            label: 'Grid-like variant',
            params: {
                ...baseSpec.params,
                nx: 24,
                producerI: 23,
                cellDx: 40,
            },
        };

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
        const variantResult = buildBenchmarkRunResult({
            spec: secondSpec,
            rateHistory: buildSyntheticWaterfloodRateHistory(secondSpec.params, reference.breakthroughPvi * 1.02, 0.01),
        });

        const model = buildReferenceComparisonModel({
            family,
            results: [baseResult, variantResult],
            xAxisMode: 'time',
            analyticalPerVariant: false,
        });

        expect(model.panels.rates.curves.filter((curve) => curve.curveKey === 'water-cut-reference')).toHaveLength(2);
        expect(model.axisMappingWarning).toContain('remapped from each completed simulation run');
    });

    it('hides waterflood analytical preview curves on time axis until runs exist for remapping', () => {
        const model = buildReferenceComparisonModel({
            family: null,
            results: [],
            xAxisMode: 'time',
            previewAnalyticalMethod: 'waterflood',
            previewVariantParams: [
                { label: 'Base', variantKey: 'base', params: { s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2, k_rw_max: 1, k_ro_max: 1, mu_w: 0.5, mu_o: 1.0, initialSaturation: 0.1 } },
            ],
        });

        expect(model.panels.rates.curves).toHaveLength(0);
        expect(model.previewCases).toHaveLength(0);
        expect(model.axisMappingWarning).toContain('hidden on this axis until remapping data exists');
    });

    it('assigns shared metric keys so compared cases stay aligned within the same family', () => {
        const family = getBenchmarkFamily('dietz_sq_center');
        const [baseSpec] = buildBenchmarkRunSpecs(family!);
        const result = buildBenchmarkRunResult({
            spec: baseSpec,
            rateHistory: buildDepletionReferenceRateHistory(baseSpec.params),
        });

        const model = buildReferenceComparisonModel({
            family,
            results: [result],
            xAxisMode: 'time',
        });

        expect(model.panels.rates.curves.find((curve) => curve.label === `${result.label} Oil Rate`)?.curveKey).toBe('oil-rate-sim');
        expect(model.panels.recovery.curves.find((curve) => curve.label === `${result.label} Recovery`)?.curveKey).toBe('recovery-factor-primary');
        expect(model.panels.diagnostics.curves.find((curve) => curve.label === `${result.label} Avg Pressure`)?.curveKey).toBe('avg-pressure-sim');
    });

    it('builds sweep preview panels before any sweep runs complete', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true };
        const variants = getBenchmarkVariantsForFamily('bl_case_a_refined');
        const specs = buildBenchmarkRunSpecs(baseFamily!, [variants[0], variants[1]]);
        const previewVariantParams = specs.slice(1).map((spec) => ({
            label: spec.label,
            variantKey: spec.key,
            params: spec.params,
        }));

        const model = buildReferenceComparisonModel({
            family,
            results: [],
            xAxisMode: 'pvi',
            analyticalPerVariant: true,
            previewVariantParams,
            previewAnalyticalMethod: family.analyticalMethod,
        });

        expect(model.previewCases).toHaveLength(2);
        expect(model.sweepPanels.rf?.curves).toHaveLength(2);
        expect(model.sweepPanels.areal?.curves).toHaveLength(2);
        expect(model.sweepPanels.combined?.curves.map((curve) => curve.curveKey)).toEqual([
            'sweep-combined-reference',
            'sweep-combined-reference',
        ]);
    });

    it('keeps pending sweep variants visible as dashed overlays while completed runs show solid sweep curves', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true };
        const variants = getBenchmarkVariantsForFamily('bl_case_a_refined');
        const [baseSpec, variantSpec] = buildBenchmarkRunSpecs(baseFamily!, [variants[0]]);
        const baseResult = buildSweepRunResult(baseSpec);
        const pendingVariant = {
            label: variantSpec.label,
            variantKey: variantSpec.key,
            params: variantSpec.params,
        };

        const model = buildReferenceComparisonModel({
            family,
            results: [baseResult],
            xAxisMode: 'pvi',
            analyticalPerVariant: false,
            previewVariantParams: [
                { label: baseSpec.label, variantKey: baseSpec.key, params: baseSpec.params },
                pendingVariant,
            ],
            pendingPreviewVariants: [pendingVariant],
        });

        expect(model.previewCases.map((entry) => entry.key)).toEqual([pendingVariant.variantKey]);

        const combinedPanel = model.sweepPanels.combined;
        expect(combinedPanel).not.toBeNull();

        const completedSimIndex = combinedPanel!.curves.findIndex(
            (curve) => curve.curveKey === 'sweep-combined-sim' && curve.caseKey === baseResult.key,
        );
        const completedAnalyticalIndex = combinedPanel!.curves.findIndex(
            (curve) => curve.curveKey === 'sweep-combined-reference' && curve.caseKey === baseResult.key,
        );
        const pendingAnalyticalIndex = combinedPanel!.curves.findIndex(
            (curve) => curve.curveKey === 'sweep-combined-reference' && curve.caseKey === pendingVariant.variantKey,
        );

        expect(completedSimIndex).toBeGreaterThanOrEqual(0);
        expect(completedAnalyticalIndex).toBeGreaterThanOrEqual(0);
        expect(pendingAnalyticalIndex).toBeGreaterThanOrEqual(0);
        expect(combinedPanel!.series[completedSimIndex]?.length).toBeGreaterThan(0);
        expect(combinedPanel!.series[pendingAnalyticalIndex]?.length).toBeGreaterThan(0);
        expect(model.sweepPanels.rf?.curves.some((curve) => curve.curveKey === 'sweep-rf-sim')).toBe(true);
    });

    it('hides the areal sweep panel for vertical sweep geometry', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);
        const verticalSpec = {
            ...baseSpec,
            key: 'vertical_sweep_case',
            caseKey: 'vertical_sweep_case',
            label: 'Vertical sweep case',
            params: {
                ...baseSpec.params,
                nx: 24,
                ny: 1,
                nz: 3,
                cellDx: 10,
                cellDy: 10,
                cellDz: 4,
                permMode: 'perLayer',
                layerPermsX: [300, 100, 30],
                layerPermsY: [300, 100, 30],
                producerI: 23,
                producerJ: 0,
            },
        };

        const result = buildSweepRunResult(verticalSpec);
        const model = buildReferenceComparisonModel({
            family,
            results: [result],
            xAxisMode: 'pvi',
        });

        expect(model.sweepPanels.areal).toBeNull();
        expect(model.sweepPanels.vertical?.curves.length).toBeGreaterThan(0);
        expect(model.sweepPanels.combined?.curves.length).toBeGreaterThan(0);
    });

    it('hides the vertical sweep panel for areal sweep geometry', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);
        const arealSpec = {
            ...baseSpec,
            key: 'areal_sweep_case',
            caseKey: 'areal_sweep_case',
            label: 'Areal sweep case',
            params: {
                ...baseSpec.params,
                nx: 21,
                ny: 21,
                nz: 3,
                cellDx: 20,
                cellDy: 20,
                cellDz: 4,
                permMode: 'uniform',
                uniformPermX: 150,
                producerI: 20,
                producerJ: 20,
            },
        };

        const result = buildSweepRunResult(arealSpec);
        const model = buildReferenceComparisonModel({
            family,
            results: [result],
            xAxisMode: 'pvi',
        });

        expect(model.sweepPanels.vertical).toBeNull();
        expect(model.sweepPanels.areal?.curves.length).toBeGreaterThan(0);
        expect(model.sweepPanels.combined?.curves.length).toBeGreaterThan(0);
    });

    it('starts sweep simulation series at zero and uses volumetric sweep for vertical simulation E_V', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);
        const verticalSpec = {
            ...baseSpec,
            key: 'vertical_sim_metric_case',
            caseKey: 'vertical_sim_metric_case',
            label: 'Vertical sim metric case',
            params: {
                ...baseSpec.params,
                nx: 6,
                ny: 1,
                nz: 3,
                permMode: 'perLayer',
                layerPermsX: [300, 100, 30],
                layerPermsY: [300, 100, 30],
                producerI: 5,
                producerJ: 0,
            },
        };

        const result = buildSweepRunResult(verticalSpec);
        const model = buildReferenceComparisonModel({
            family,
            results: [result],
            xAxisMode: 'pvi',
        });

        const verticalPanel = model.sweepPanels.vertical;
        const simIndex = verticalPanel!.curves.findIndex((curve) => curve.curveKey === 'sweep-vertical-sim');
        const combinedPanel = model.sweepPanels.combined;
        const combinedSimIndex = combinedPanel!.curves.findIndex((curve) => curve.curveKey === 'sweep-combined-sim');

        expect(verticalPanel!.series[simIndex]?.[0]).toEqual({ x: 0, y: 0 });
        expect(combinedPanel!.series[combinedSimIndex]?.[0]).toEqual({ x: 0, y: 0 });
        expect(verticalPanel!.series[simIndex]?.[1]?.y).toBeCloseTo(combinedPanel!.series[combinedSimIndex]?.[1]?.y ?? NaN, 10);
    });
});