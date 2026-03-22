import { describe, expect, it } from 'vitest';
import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { computeWelgeMetrics } from '../analytical/fractionalFlow';
import { getBenchmarkFamily, getBenchmarkVariantsForFamily } from '../catalog/caseCatalog';
import { buildBenchmarkRunResult, buildBenchmarkRunSpecs } from '../benchmarkRunModel';
import type { SimulatorSnapshot } from '../simulator-types';
import { buildReferenceComparisonModel } from './referenceComparisonModel';

function buildSyntheticGasOilRateHistory(
    params: Record<string, any>,
    breakthroughPvi: number,
    gasCutShift = 0,
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
            total_production_gas: 1 + gasCutShift * 5,
            avg_reservoir_pressure: 245,
            avg_water_saturation: Number(params.initialSaturation ?? 0.2),
        },
        {
            time: 2,
            total_injection: secondInjection,
            total_production_liquid: 100,
            total_production_oil: 85 - gasCutShift * 10,
            total_production_gas: 20 + gasCutShift * 20,
            avg_reservoir_pressure: 232,
            avg_water_saturation: Number(params.initialSaturation ?? 0.2),
        },
        {
            time: 3,
            total_injection: thirdInjection,
            total_production_liquid: 100,
            total_production_oil: 55 - gasCutShift * 10,
            total_production_gas: 55 + gasCutShift * 25,
            avg_reservoir_pressure: 220,
            avg_water_saturation: Number(params.initialSaturation ?? 0.2),
        },
    ];
}

function buildGasOilRunResult(spec: ReturnType<typeof buildBenchmarkRunSpecs>[number]) {
    return buildBenchmarkRunResult({
        spec,
        rateHistory: buildSyntheticGasOilRateHistory(spec.params, 0.35, 0),
    });
}

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

function buildSweepColumnSnapshots(
    params: Record<string, any>,
    layerCountsPerSnapshot: number[],
): SimulatorSnapshot[] {
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const nz = Number(params.nz);
    const total = nx * ny * nz;
    const swc = Number(params.s_wc ?? 0.1);
    const pressure = Number(params.initialPressure ?? 300);
    const offInjectorColumn = nx + 1;

    return layerCountsPerSnapshot.map((layerCount, index) => {
        const satWater = new Float64Array(total).fill(swc);
        const satOil = new Float64Array(total).fill(Math.max(0, 1 - swc));
        const satGas = new Float64Array(total).fill(0);
        for (let k = 0; k < Math.min(nz, layerCount); k += 1) {
            const cellIndex = k * nx * ny + offInjectorColumn;
            satWater[cellIndex] = swc + 0.2;
            satOil[cellIndex] = Math.max(0, 1 - satWater[cellIndex]);
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
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'both' as const };
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

    it('suppresses generic BL rate overlays for combined sweep variants that only change heterogeneity', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'both' as const, analyticalOverlayMode: 'shared' as const };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);

        const heterogeneityPreviewVariants = [
            {
                label: 'Uniform layers',
                variantKey: 'uniform_layers',
                params: {
                    ...baseSpec.params,
                    permMode: 'uniform',
                    uniformPermX: 100,
                    uniformPermY: 100,
                    uniformPermZ: 10,
                    nx: 48,
                    ny: 1,
                    nz: 5,
                    producerI: 47,
                    producerJ: 0,
                },
            },
            {
                label: 'Layered contrast',
                variantKey: 'layered_contrast',
                params: {
                    ...baseSpec.params,
                    permMode: 'perLayer',
                    layerPermsX: [200, 150, 100, 60, 40],
                    layerPermsY: [200, 150, 100, 60, 40],
                    layerPermsZ: [20, 15, 10, 6, 4],
                    nx: 48,
                    ny: 1,
                    nz: 5,
                    producerI: 47,
                    producerJ: 0,
                },
            },
        ];

        const previewModel = buildReferenceComparisonModel({
            family,
            results: [],
            xAxisMode: 'pvi',
            analyticalPerVariant: true,
            previewVariantParams: heterogeneityPreviewVariants,
            previewAnalyticalMethod: family.analyticalMethod,
        });

        expect(previewModel.panels.rates.curves.filter((curve) => curve.curveKey === 'water-cut-reference')).toHaveLength(0);
        expect(previewModel.sweepPanels.combined?.curves.length).toBe(2);

        const resultSpecs = heterogeneityPreviewVariants.map((variant) => ({
            ...baseSpec,
            key: variant.variantKey,
            caseKey: variant.variantKey,
            variantKey: variant.variantKey,
            variantLabel: variant.label,
            label: variant.label,
            params: variant.params,
        }));
        const results = resultSpecs.map((spec) => buildSweepRunResult(spec));

        const completedModel = buildReferenceComparisonModel({
            family,
            results,
            xAxisMode: 'pvi',
            analyticalPerVariant: true,
        });

        expect(completedModel.panels.rates.curves.filter((curve) => curve.curveKey === 'water-cut-reference')).toHaveLength(0);
        expect(completedModel.sweepPanels.combined?.curves.length).toBeGreaterThan(1);
    });

    it('suppresses generic BL breakthrough references for completed combined sweep mobility runs', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'both' as const, analyticalOverlayMode: 'per-result' as const };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);

        const mobilitySpecs = [
            {
                ...baseSpec,
                key: 'sweep_mobility_favorable',
                caseKey: 'sweep_mobility_favorable',
                variantKey: 'sweep_mobility_favorable',
                variantLabel: 'Favorable mobility',
                label: 'Sweep mobility favorable',
                params: {
                    ...baseSpec.params,
                    mu_o: 0.5,
                    nx: 21,
                    ny: 21,
                    nz: 5,
                    producerI: 20,
                    producerJ: 20,
                },
            },
            {
                ...baseSpec,
                key: 'sweep_mobility_base',
                caseKey: 'sweep_mobility_base',
                variantKey: 'sweep_mobility_base',
                variantLabel: 'Base mobility',
                label: 'Sweep mobility base',
                params: {
                    ...baseSpec.params,
                    mu_o: 1.0,
                    nx: 21,
                    ny: 21,
                    nz: 5,
                    producerI: 20,
                    producerJ: 20,
                },
            },
            {
                ...baseSpec,
                key: 'sweep_mobility_unfavorable',
                caseKey: 'sweep_mobility_unfavorable',
                variantKey: 'sweep_mobility_unfavorable',
                variantLabel: 'Unfavorable mobility',
                label: 'Sweep mobility unfavorable',
                params: {
                    ...baseSpec.params,
                    mu_o: 5.0,
                    nx: 21,
                    ny: 21,
                    nz: 5,
                    producerI: 20,
                    producerJ: 20,
                },
            },
        ];

        const results = mobilitySpecs.map((spec) => buildSweepRunResult(spec));
        const model = buildReferenceComparisonModel({
            family,
            results,
            xAxisMode: 'pvi',
        });

        const referenceCurves = model.panels.rates.curves.filter((curve) => curve.curveKey === 'water-cut-reference');
        expect(referenceCurves).toHaveLength(0);
        expect(model.sweepPanels.combined?.curves.filter((curve) => curve.curveKey === 'sweep-combined-reference')).toHaveLength(3);
    });

    it('builds nonzero completed sweep BL references on PVI even when injection-rate history is missing', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'vertical' as const, analyticalOverlayMode: 'per-result' as const };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);

        const results = [0.5, 5.0].map((mu_o, index) => {
            const spec = {
                ...baseSpec,
                key: `vertical_mobility_${index}`,
                caseKey: `vertical_mobility_${index}`,
                variantKey: `vertical_mobility_${index}`,
                variantLabel: `mobility_${index}`,
                label: `Vertical mobility ${index}`,
                params: {
                    ...baseSpec.params,
                    mu_o,
                    nx: 48,
                    ny: 1,
                    nz: 5,
                    permMode: 'perLayer',
                    layerPermsX: [200, 150, 100, 60, 40],
                    layerPermsY: [200, 150, 100, 60, 40],
                    producerI: 47,
                    producerJ: 0,
                },
            };
            const result = buildSweepRunResult(spec);
            result.rateHistory = result.rateHistory.map((point) => ({
                ...point,
                total_injection: 0,
            }));
            result.pviSeries = [0.15, 0.4, 0.8];
            return result;
        });

        const model = buildReferenceComparisonModel({
            family,
            results,
            xAxisMode: 'pvi',
        });

        const referenceSeries = model.panels.rates.series.filter((_, index) => (
            model.panels.rates.curves[index]?.curveKey === 'water-cut-reference'
        ));

        expect(referenceSeries).toHaveLength(2);
        expect(referenceSeries.every((series) => series.some((point) => (point.y ?? 0) > 0))).toBe(true);
    });

    it('keeps pending sweep variants visible as dashed overlays while completed runs show solid sweep curves', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'vertical' as const };
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
        const combinedMobileOilPanel = model.sweepPanels.combinedMobileOil;
        expect(combinedPanel).not.toBeNull();
        expect(combinedMobileOilPanel).toBeNull();

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
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'vertical' as const };
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
        expect(model.sweepPanels.combinedMobileOil).toBeNull();
    });

    it('keeps uniform variants on the vertical sweep decomposition when scenario geometry is vertical', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'vertical' as const };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);
        const uniformVerticalSpec = {
            ...baseSpec,
            key: 'vertical_uniform_case',
            caseKey: 'vertical_uniform_case',
            label: 'Vertical uniform case',
            params: {
                ...baseSpec.params,
                nx: 24,
                ny: 1,
                nz: 3,
                cellDx: 10,
                cellDy: 10,
                cellDz: 4,
                permMode: 'uniform',
                uniformPermX: 100,
                uniformPermY: 100,
                uniformPermZ: 10,
                producerI: 23,
                producerJ: 0,
            },
        };

        const result = buildSweepRunResult(uniformVerticalSpec);
        const model = buildReferenceComparisonModel({
            family,
            results: [result],
            xAxisMode: 'pvi',
        });

        expect(model.sweepPanels.areal).toBeNull();
        expect(model.sweepPanels.vertical?.curves.length).toBeGreaterThan(0);
        expect(model.sweepPanels.combined?.curves.length).toBeGreaterThan(0);
        expect(model.sweepPanels.combinedMobileOil).toBeNull();
    });

    it('hides the vertical sweep panel for areal sweep geometry', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'areal' as const };
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
        expect(model.sweepPanels.combinedMobileOil).toBeNull();
    });

    it('starts sweep simulation series at zero and uses volumetric sweep for vertical simulation E_V', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'vertical' as const };
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

    it('preserves the zero origin when the first sweep snapshot also remaps to x=0', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'both' as const };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);
        const result = buildSweepRunResult(baseSpec);
        result.pviSeries = result.pviSeries.map((value, index) => (index === 0 ? 0 : value));

        const model = buildReferenceComparisonModel({
            family,
            results: [result],
            xAxisMode: 'pvi',
        });

        const verticalPanel = model.sweepPanels.vertical;
        const verticalSimIndex = verticalPanel!.curves.findIndex((curve) => curve.curveKey === 'sweep-vertical-sim');
        const combinedPanel = model.sweepPanels.combined;
        const combinedEvolIndex = combinedPanel!.curves.findIndex((curve) => curve.curveKey === 'sweep-combined-sim');
        const combinedMobileOilPanel = model.sweepPanels.combinedMobileOil;
        const combinedSimIndex = combinedMobileOilPanel!.curves.findIndex((curve) => curve.curveKey === 'sweep-combined-mobile-oil-sim');

        expect(verticalSimIndex).toBe(-1);
        expect(verticalPanel!.curves.some((curve) => curve.curveKey === 'sweep-vertical-reference')).toBe(true);
        expect(combinedPanel!.series[combinedEvolIndex]?.[0]).toEqual({ x: 0, y: 0 });
        expect(combinedMobileOilPanel!.series[combinedSimIndex]?.[0]).toEqual({ x: 0, y: 0 });
        expect(combinedPanel!.curves[combinedEvolIndex]?.label).toContain('E_vol');
        expect(combinedMobileOilPanel!.curves[combinedSimIndex]?.label).toContain('Mobile Oil Recovered');
    });

    it('remaps completed sweep panels onto the selected time axis', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'both' as const };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);
        const result = buildSweepRunResult(baseSpec);

        const model = buildReferenceComparisonModel({
            family,
            results: [result],
            xAxisMode: 'time',
        });

        const recoverySimSeries = model.panels.recovery.series[0];
        const sweepRfPanel = model.sweepPanels.rf;
        const simRfIndex = sweepRfPanel!.curves.findIndex((curve) => curve.curveKey === 'sweep-rf-sim');
        const analyticalCombinedPanel = model.sweepPanels.combined;
        const analyticalCombinedMobileOilPanel = model.sweepPanels.combinedMobileOil;
        const combinedEvolIndex = analyticalCombinedPanel!.curves.findIndex((curve) => curve.curveKey === 'sweep-combined-sim');
        const combinedSimIndex = analyticalCombinedMobileOilPanel!.curves.findIndex((curve) => curve.curveKey === 'sweep-combined-mobile-oil-sim');
        const analyticalCombinedIndex = analyticalCombinedPanel!.curves.findIndex((curve) => curve.curveKey === 'sweep-combined-reference');

        expect(sweepRfPanel!.series[simRfIndex]).toEqual(recoverySimSeries);
        expect(combinedEvolIndex).toBeGreaterThanOrEqual(0);
        expect(combinedSimIndex).toBeGreaterThanOrEqual(0);
        expect(analyticalCombinedPanel!.series[analyticalCombinedIndex]?.[0]?.x).toBe(0);
        expect(analyticalCombinedPanel!.series[analyticalCombinedIndex]?.at(-1)?.x).toBeCloseTo(recoverySimSeries.at(-1)?.x ?? NaN, 10);
        expect(analyticalCombinedPanel!.series[analyticalCombinedIndex]?.[1]?.x).toBeGreaterThan(0);
        expect(analyticalCombinedPanel!.series[analyticalCombinedIndex]?.[1]?.x).toBeLessThanOrEqual(recoverySimSeries.at(-1)?.x ?? NaN);
        expect(analyticalCombinedMobileOilPanel!.series[combinedSimIndex]?.[0]?.x).toBe(0);
    });

    it('suppresses generic BL analytical overlays for combined sweep scenarios', () => {
        const baseFamily = getBenchmarkFamily('bl_case_a_refined');
        const family = { ...baseFamily!, showSweepPanel: true, sweepGeometry: 'both' as const };
        const [baseSpec] = buildBenchmarkRunSpecs(baseFamily!);
        const result = buildSweepRunResult(baseSpec);

        const model = buildReferenceComparisonModel({
            family,
            results: [result],
            xAxisMode: 'pvi',
        });

        expect(model.panels.recovery.curves.some((curve) => curve.curveKey === 'recovery-factor-reference')).toBe(false);
        expect(model.panels.rates.curves.some((curve) => curve.curveKey === 'water-cut-reference')).toBe(false);
        expect(model.sweepPanels.combined?.curves.some((curve) => curve.curveKey === 'sweep-combined-reference')).toBe(true);
        expect(model.sweepPanels.combinedMobileOil?.curves.some((curve) => curve.curveKey === 'sweep-combined-reference')).toBe(true);
    });

    it('keeps gas-oil BL overlays shared for variants that only change permeability', () => {
        const family = {
            key: 'gas_injection',
            analyticalMethod: 'gas-oil-bl',
            showSweepPanel: false,
            sweepGeometry: null,
        } as any;
        const baseSpec = {
            key: 'gas_base',
            caseKey: 'gas_base',
            familyKey: 'gas_injection',
            analyticalMethod: 'gas-oil-bl' as const,
            variantKey: null,
            variantLabel: null,
            label: 'Gas Base',
            description: 'Base gas-oil case',
            params: {
                analyticalSolutionMode: 'waterflood',
                nx: 50, ny: 1, nz: 1,
                cellDx: 20, cellDy: 50, cellDz: 10,
                initialPressure: 250,
                initialSaturation: 0.2,
                initialGasSaturation: 0,
                reservoirPorosity: 0.2,
                uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10,
                permMode: 'uniform',
                s_wc: 0.2, s_or: 0.15,
                s_gc: 0.05, s_gr: 0.05, s_org: 0.20,
                n_w: 2.0, n_o: 2.0, n_g: 1.5,
                k_rw_max: 0.4, k_ro_max: 1.0, k_rg_max: 0.8,
                mu_w: 0.5, mu_o: 2.0, mu_g: 0.02,
                c_w: 3e-6, c_o: 1e-5, c_g: 1e-4,
                rho_w: 1000, rho_o: 800, rho_g: 10,
                depth_reference: 0,
                volume_expansion_o: 1.0, volume_expansion_w: 1.0,
                rock_compressibility: 1e-6,
                injectorEnabled: true,
                injectorControlMode: 'pressure',
                producerControlMode: 'pressure',
                injectorBhp: 350, producerBhp: 100,
                injectorI: 0, injectorJ: 0,
                producerI: 49, producerJ: 0,
                well_radius: 0.1, well_skin: 0,
                capillaryEnabled: false,
                capillaryPEntry: 0, capillaryLambda: 2,
                gravityEnabled: false,
                threePhaseModeEnabled: true,
                injectedFluid: 'gas',
                pcogEnabled: false, pcogPEntry: 3, pcogLambda: 2,
                delta_t_days: 2,
                steps: 150,
                max_sat_change_per_step: 0.05,
                max_pressure_change_per_step: 75,
                max_well_rate_change_fraction: 0.75,
            },
            steps: 150,
            deltaTDays: 2,
            historyInterval: 3,
            reference: { kind: 'analytical' as const, source: 'gas_injection:analytical' },
            comparisonMetric: null,
            breakthroughCriterion: null,
            comparisonMeaning: 'Base gas-oil case',
        };

        const permeabilityPreviewVariants = [
            {
                label: '10 mD',
                variantKey: 'perm_low',
                params: {
                    ...baseSpec.params,
                    uniformPermX: 10,
                    uniformPermY: 10,
                    uniformPermZ: 1,
                },
            },
            {
                label: '1000 mD',
                variantKey: 'perm_high',
                params: {
                    ...baseSpec.params,
                    uniformPermX: 1000,
                    uniformPermY: 1000,
                    uniformPermZ: 100,
                },
            },
        ];

        const previewModel = buildReferenceComparisonModel({
            family,
            results: [],
            xAxisMode: 'pvi',
            analyticalPerVariant: true,
            previewVariantParams: permeabilityPreviewVariants,
            previewAnalyticalMethod: 'gas-oil-bl',
        });

        expect(previewModel.panels.rates.curves.filter((curve) => curve.curveKey === 'gas-cut-reference')).toHaveLength(1);

        const resultSpecs = permeabilityPreviewVariants.map((variant) => ({
            ...baseSpec,
            key: variant.variantKey,
            caseKey: variant.variantKey,
            variantKey: variant.variantKey,
            variantLabel: variant.label,
            label: variant.label,
            params: variant.params,
        }));
        const results = resultSpecs.map((spec) => buildGasOilRunResult(spec));

        const completedModel = buildReferenceComparisonModel({
            family,
            results,
            xAxisMode: 'pvi',
            analyticalPerVariant: true,
        });

        expect(completedModel.panels.rates.curves.filter((curve) => curve.curveKey === 'gas-cut-reference')).toHaveLength(1);
        expect(completedModel.panels.recovery.curves.filter((curve) => curve.curveKey === 'recovery-factor-reference')).toHaveLength(1);
    });
});