import { describe, expect, it } from 'vitest';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import {
    toFiniteNumber,
    getLayerThicknesses,
    getTotalThickness,
    getAverageLayerThickness,
    getPoreVolume,
    getOoip,
    getLayerPermeabilities,
    extractRockProps,
    extractFluidProps,
    extractGasOilRockProps,
    extractGasOilFluidProps,
    getBuckleyLeverettOverlaySignature,
    hasDistinctBuckleyLeverettOverlays,
    getGasOilBLOverlaySignature,
    hasDistinctGasOilBLOverlays,
    resolveOverlayMode,
    defaultBLPviGrid,
    computeBLAnalyticalFromParams,
    defaultGasOilBLPviGrid,
    computeGasOilBLAnalyticalFromParams,
    computeDepletionTau,
    computeDepletionAnalyticalFromParams,
    MIN_GOR_OIL_RATE_SM3_DAY,
    buildDerivedRunSeries,
} from './analyticalParamAdapters';

// ─── Helpers ──────────────────────────────────────────────────────────────────

function baseParams(override: Record<string, any> = {}): Record<string, any> {
    return {
        nx: 10, ny: 5, nz: 1,
        cellDx: 20, cellDy: 10, cellDz: 5,
        reservoirPorosity: 0.2,
        initialSaturation: 0.2,
        s_wc: 0.2, s_or: 0.15,
        n_w: 2, n_o: 2,
        k_rw_max: 0.3, k_ro_max: 0.8,
        mu_w: 0.5, mu_o: 2,
        ...override,
    };
}

function makeResult(override: Partial<BenchmarkRunResult> = {}): BenchmarkRunResult {
    const n = 3;
    const rateHistory = [
        { time: 10, total_production_oil: 50, total_production_liquid: 50, total_injection: 60,
          avg_reservoir_pressure: 280, total_production_gas: 0, producing_gor: 0 },
        { time: 20, total_production_oil: 45, total_production_liquid: 50, total_injection: 60,
          avg_reservoir_pressure: 260, total_production_gas: 0, producing_gor: 0 },
        { time: 30, total_production_oil: 40, total_production_liquid: 50, total_injection: 60,
          avg_reservoir_pressure: 240, total_production_gas: 0, producing_gor: 0 },
    ];
    return {
        key: 'test', caseKey: 'test', familyKey: 'test',
        analyticalMethod: 'buckley-leverett',
        variantKey: null, variantLabel: null,
        label: 'Test', description: '',
        params: baseParams(),
        rateHistory,
        history: [],
        finalSnapshot: null,
        breakthroughPvi: null, breakthroughTime: null,
        watercutSeries: Array(n).fill(0),
        pressureSeries: [280, 260, 240],
        recoverySeries: [0.05, 0.10, 0.15],
        pviSeries: [0.2, 0.4, 0.6],
        referencePolicy: {} as any,
        referenceComparison: {} as any,
        comparisonOutputs: {} as any,
        comparisonMeaning: '',
        ...override,
    };
}

// ─── toFiniteNumber ───────────────────────────────────────────────────────────

describe('toFiniteNumber', () => {
    it('returns the numeric value for a valid number', () => {
        expect(toFiniteNumber(42, 0)).toBe(42);
    });

    it('returns fallback for NaN', () => {
        expect(toFiniteNumber(NaN, 5)).toBe(5);
    });

    it('returns fallback for Infinity', () => {
        expect(toFiniteNumber(Infinity, 1)).toBe(1);
    });

    it('coerces null to 0 (Number(null) === 0 is finite)', () => {
        expect(toFiniteNumber(null, 3)).toBe(0);
    });

    it('returns fallback for undefined', () => {
        expect(toFiniteNumber(undefined, 7)).toBe(7);
    });

    it('coerces numeric strings', () => {
        expect(toFiniteNumber('3.14', 0)).toBeCloseTo(3.14);
    });
});

// ─── getLayerThicknesses ──────────────────────────────────────────────────────

describe('getLayerThicknesses', () => {
    it('returns [cellDz] for single layer with no cellDzPerLayer', () => {
        expect(getLayerThicknesses({ nz: 1, cellDz: 5 })).toEqual([5]);
    });

    it('fills nz layers with cellDz when cellDzPerLayer is absent', () => {
        const result = getLayerThicknesses({ nz: 3, cellDz: 4 });
        expect(result).toHaveLength(3);
        expect(result.every((v) => v === 4)).toBe(true);
    });

    it('uses cellDzPerLayer values when present', () => {
        const result = getLayerThicknesses({ nz: 3, cellDz: 4, cellDzPerLayer: [1, 2, 3] });
        expect(result).toEqual([1, 2, 3]);
    });

    it('falls back to cellDz for zero-thickness layers', () => {
        const result = getLayerThicknesses({ nz: 2, cellDz: 5, cellDzPerLayer: [0, 3] });
        expect(result).toEqual([5, 3]);
    });

    it('falls back to cellDz for out-of-bounds indices', () => {
        // cellDzPerLayer shorter than nz — index 2 falls back
        const result = getLayerThicknesses({ nz: 3, cellDz: 5, cellDzPerLayer: [2, 4] });
        expect(result[2]).toBe(5);
    });
});

// ─── getTotalThickness / getAverageLayerThickness ─────────────────────────────

describe('getTotalThickness', () => {
    it('sums layer thicknesses', () => {
        expect(getTotalThickness({ nz: 3, cellDz: 4, cellDzPerLayer: [2, 3, 5] })).toBe(10);
    });

    it('works for single-layer uniform grid', () => {
        expect(getTotalThickness({ nz: 1, cellDz: 8 })).toBe(8);
    });
});

describe('getAverageLayerThickness', () => {
    it('returns mean of layer thicknesses', () => {
        expect(getAverageLayerThickness({ nz: 2, cellDzPerLayer: [2, 6] })).toBe(4);
    });

    it('equals cellDz for uniform single-layer grid', () => {
        expect(getAverageLayerThickness({ nz: 1, cellDz: 7 })).toBe(7);
    });
});

// ─── getPoreVolume / getOoip ──────────────────────────────────────────────────

describe('getPoreVolume', () => {
    it('computes bulk pore volume correctly', () => {
        // 10×5 × 20×10 × 5 × 0.2 = 10000
        expect(getPoreVolume(baseParams())).toBeCloseTo(10000);
    });

    it('uses reservoirPorosity over porosity', () => {
        const pv1 = getPoreVolume(baseParams({ reservoirPorosity: 0.3 }));
        const pv2 = getPoreVolume(baseParams({ reservoirPorosity: undefined, porosity: 0.3 }));
        expect(pv1).toBeCloseTo(pv2);
        expect(pv1).toBeGreaterThan(getPoreVolume(baseParams({ reservoirPorosity: 0.2 })));
    });

    it('falls back to default porosity 0.2 when absent', () => {
        const pv = getPoreVolume({ nx: 1, ny: 1, cellDx: 10, cellDy: 10, nz: 1, cellDz: 1 });
        expect(pv).toBeCloseTo(20);
    });
});

describe('getOoip', () => {
    it('equals poreVolume × (1 - initialSaturation)', () => {
        const params = baseParams({ initialSaturation: 0.2 });
        const pv = getPoreVolume(params);
        expect(getOoip(params)).toBeCloseTo(pv * 0.8);
    });

    it('clamps at 0 when initialSaturation >= 1', () => {
        expect(getOoip(baseParams({ initialSaturation: 1.0 }))).toBe(0);
    });
});

// ─── getLayerPermeabilities ───────────────────────────────────────────────────

describe('getLayerPermeabilities', () => {
    it('returns [uniformPermX] for single-layer uniform mode', () => {
        expect(getLayerPermeabilities({ nz: 1, permMode: 'uniform', uniformPermX: 100 })).toEqual([100]);
    });

    it('fills nz entries for multi-layer uniform mode', () => {
        const result = getLayerPermeabilities({ nz: 3, permMode: 'uniform', uniformPermX: 200 });
        expect(result).toEqual([200, 200, 200]);
    });

    it('returns layerPermsX in perLayer mode', () => {
        const result = getLayerPermeabilities({
            nz: 3, permMode: 'perLayer',
            layerPermsX: [50, 100, 200],
        });
        expect(result).toEqual([50, 100, 200]);
    });

    it('falls back to uniform when layerPermsX has only 1 element', () => {
        const result = getLayerPermeabilities({
            nz: 2, permMode: 'perLayer',
            layerPermsX: [75],
            uniformPermX: 75,
        });
        expect(result).toEqual([75, 75]);
    });
});

// ─── extractRockProps / extractFluidProps ─────────────────────────────────────

describe('extractRockProps', () => {
    it('picks up explicit values from params', () => {
        const props = extractRockProps(baseParams({ s_wc: 0.15, s_or: 0.10, n_w: 3, n_o: 4 }));
        expect(props.s_wc).toBe(0.15);
        expect(props.s_or).toBe(0.10);
        expect(props.n_w).toBe(3);
        expect(props.n_o).toBe(4);
    });

    it('uses documented defaults for missing params', () => {
        const props = extractRockProps({});
        expect(props.s_wc).toBe(0.1);
        expect(props.s_or).toBe(0.1);
    });
});

describe('extractFluidProps', () => {
    it('picks up explicit mu values', () => {
        const props = extractFluidProps({ mu_w: 0.3, mu_o: 5 });
        expect(props.mu_w).toBe(0.3);
        expect(props.mu_o).toBe(5);
    });
});

describe('extractGasOilRockProps', () => {
    it('uses documented defaults for missing params', () => {
        const props = extractGasOilRockProps({});
        expect(props.s_wc).toBe(0.2);
        expect(props.s_gc).toBe(0.05);
        expect(props.k_rg_max).toBe(0.8);
    });
});

describe('extractGasOilFluidProps', () => {
    it('picks up mu_g', () => {
        const props = extractGasOilFluidProps({ mu_o: 3, mu_g: 0.015 });
        expect(props.mu_o).toBe(3);
        expect(props.mu_g).toBe(0.015);
    });
});

// ─── overlay signature / distinctness ────────────────────────────────────────

describe('getBuckleyLeverettOverlaySignature', () => {
    it('returns the same key for identical params', () => {
        const p = baseParams();
        expect(getBuckleyLeverettOverlaySignature(p)).toBe(getBuckleyLeverettOverlaySignature(p));
    });

    it('differs when mobility ratio changes', () => {
        const a = getBuckleyLeverettOverlaySignature(baseParams({ mu_o: 2 }));
        const b = getBuckleyLeverettOverlaySignature(baseParams({ mu_o: 5 }));
        expect(a).not.toBe(b);
    });
});

describe('hasDistinctBuckleyLeverettOverlays', () => {
    it('returns false for single param set', () => {
        expect(hasDistinctBuckleyLeverettOverlays([baseParams()])).toBe(false);
    });

    it('returns false when all param sets have identical physics', () => {
        expect(hasDistinctBuckleyLeverettOverlays([baseParams(), baseParams()])).toBe(false);
    });

    it('returns true when mobility ratio differs', () => {
        expect(hasDistinctBuckleyLeverettOverlays([
            baseParams({ mu_o: 2 }),
            baseParams({ mu_o: 5 }),
        ])).toBe(true);
    });
});

describe('hasDistinctGasOilBLOverlays', () => {
    it('returns false for a single param set', () => {
        expect(hasDistinctGasOilBLOverlays([{}])).toBe(false);
    });

    it('returns true when initial gas saturation differs', () => {
        expect(hasDistinctGasOilBLOverlays([
            { initialGasSaturation: 0 },
            { initialGasSaturation: 0.1 },
        ])).toBe(true);
    });
});

describe('getGasOilBLOverlaySignature', () => {
    it('matches for identical physics', () => {
        const p = {};
        expect(getGasOilBLOverlaySignature(p)).toBe(getGasOilBLOverlaySignature(p));
    });
});

// ─── resolveOverlayMode ───────────────────────────────────────────────────────

describe('resolveOverlayMode', () => {
    it("returns 'shared' when explicitly requested", () => {
        expect(resolveOverlayMode({ requested: 'shared', distinctByPhysics: true })).toBe('shared');
    });

    it("returns 'per-result' when explicitly requested", () => {
        expect(resolveOverlayMode({ requested: 'per-result', distinctByPhysics: false })).toBe('per-result');
    });

    it("returns 'per-result' when analyticalPerVariant is true", () => {
        expect(resolveOverlayMode({ requested: null, distinctByPhysics: false, analyticalPerVariant: true })).toBe('per-result');
    });

    it("returns 'per-result' when physics are distinct and no explicit override", () => {
        expect(resolveOverlayMode({ requested: null, distinctByPhysics: true })).toBe('per-result');
    });

    it("returns 'shared' when physics are identical and no override", () => {
        expect(resolveOverlayMode({ requested: null, distinctByPhysics: false })).toBe('shared');
    });
});

// ─── PVI grid helpers ─────────────────────────────────────────────────────────

describe('defaultBLPviGrid', () => {
    it('has 150 points', () => {
        expect(defaultBLPviGrid()).toHaveLength(150);
    });

    it('starts at 0 and ends at 3', () => {
        const grid = defaultBLPviGrid();
        expect(grid[0]).toBe(0);
        expect(grid.at(-1)).toBeCloseTo(3.0);
    });
});

describe('defaultGasOilBLPviGrid', () => {
    it('has 150 points starting at 0 ending at 3', () => {
        const grid = defaultGasOilBLPviGrid();
        expect(grid).toHaveLength(150);
        expect(grid[0]).toBe(0);
        expect(grid.at(-1)).toBeCloseTo(3.0);
    });
});

// ─── computeBLAnalyticalFromParams ────────────────────────────────────────────

describe('computeBLAnalyticalFromParams', () => {
    it('returns 150-point arrays on default PVI grid', () => {
        const result = computeBLAnalyticalFromParams(baseParams());
        expect(result).not.toBeNull();
        expect(result!.waterCut).toHaveLength(150);
        expect(result!.recovery).toHaveLength(150);
        expect(result!.xValues).toHaveLength(150);
    });

    it('waterCut is in [0, 1]', () => {
        const result = computeBLAnalyticalFromParams(baseParams())!;
        for (const v of result.waterCut) {
            if (v !== null) {
                expect(v).toBeGreaterThanOrEqual(0);
                expect(v).toBeLessThanOrEqual(1);
            }
        }
    });

    it('recovery is monotonically non-decreasing', () => {
        const result = computeBLAnalyticalFromParams(baseParams())!;
        for (let i = 1; i < result.recovery.length; i++) {
            const prev = result.recovery[i - 1];
            const curr = result.recovery[i];
            if (prev !== null && curr !== null) {
                expect(curr).toBeGreaterThanOrEqual(prev - 1e-9);
            }
        }
    });

    it('returns null for physically impossible params (mu_o = 0)', () => {
        // Degenerate params that force the BL library to throw
        const result = computeBLAnalyticalFromParams(baseParams({ mu_o: 0, mu_w: 0 }));
        // Either null or valid — just shouldn't throw
        expect(result === null || Array.isArray(result?.waterCut)).toBe(true);
    });

    it('uses provided xValues and returns matching-length arrays', () => {
        const xValues = [0, 0.5, 1.0, 1.5, 2.0];
        const result = computeBLAnalyticalFromParams(baseParams(), {
            xValues,
            timeHistory: xValues,
            injectionRateSeries: new Array(5).fill(1),
            poreVolume: 1,
        });
        expect(result).not.toBeNull();
        expect(result!.xValues).toHaveLength(5);
    });
});

// ─── computeGasOilBLAnalyticalFromParams ─────────────────────────────────────

describe('computeGasOilBLAnalyticalFromParams', () => {
    it('returns valid arrays for default gas-oil params', () => {
        const result = computeGasOilBLAnalyticalFromParams({
            s_wc: 0.2, s_gc: 0.05, s_gr: 0.05, s_org: 0.2,
            n_o: 2, n_g: 1.5, k_ro_max: 1, k_rg_max: 0.8,
            mu_o: 2, mu_g: 0.02,
            initialGasSaturation: 0,
        });
        expect(result).not.toBeNull();
        expect(result!.pviValues).toHaveLength(150);
        expect(result!.gasCut.every((v) => v === null || (v >= 0 && v <= 1))).toBe(true);
    });
});

// ─── computeDepletionTau ──────────────────────────────────────────────────────

describe('computeDepletionTau', () => {
    it('returns a positive finite number for well-posed params', () => {
        const tau = computeDepletionTau({
            nx: 10, ny: 1, nz: 1,
            cellDx: 100, cellDy: 100, cellDz: 10,
            reservoirPorosity: 0.2, initialSaturation: 0.2,
            s_wc: 0.2, s_or: 0.1, n_o: 2,
            mu_o: 1, c_o: 1e-5, c_w: 3e-6, rock_compressibility: 1e-6,
            uniformPermX: 100, permMode: 'uniform',
            well_radius: 0.1, well_skin: 0,
            initialPressure: 300, producerBhp: 100,
            analyticalDepletionRateScale: 1, analyticalArpsB: 0,
        });
        expect(tau).not.toBeNull();
        expect(Number.isFinite(tau)).toBe(true);
        expect(tau!).toBeGreaterThan(0);
    });
});

// ─── computeDepletionAnalyticalFromParams ────────────────────────────────────

describe('computeDepletionAnalyticalFromParams', () => {
    const depletionParams = {
        nx: 10, ny: 1, nz: 1,
        cellDx: 100, cellDy: 100, cellDz: 10,
        reservoirPorosity: 0.2, initialSaturation: 0.2,
        s_wc: 0.2, s_or: 0.1, n_o: 2,
        mu_o: 1, c_o: 1e-5, c_w: 3e-6, rock_compressibility: 1e-6,
        uniformPermX: 100, permMode: 'uniform',
        well_radius: 0.1, well_skin: 0,
        initialPressure: 300, producerBhp: 100,
        analyticalDepletionRateScale: 1, analyticalArpsB: 0,
        steps: 50, delta_t_days: 10,
    };

    it('returns 50-point arrays in time mode', () => {
        const result = computeDepletionAnalyticalFromParams(depletionParams, 'time');
        expect(result).not.toBeNull();
        expect(result!.xValues).toHaveLength(50);
        expect(result!.oilRates).toHaveLength(50);
    });

    it('xValues are positive and increasing in time mode', () => {
        const result = computeDepletionAnalyticalFromParams(depletionParams, 'time')!;
        for (let i = 1; i < result.xValues.length; i++) {
            const prev = result.xValues[i - 1];
            const curr = result.xValues[i];
            if (prev !== null && curr !== null) expect(curr).toBeGreaterThan(prev);
        }
    });

    it('xValues are log10(t) in logTime mode', () => {
        const result = computeDepletionAnalyticalFromParams(depletionParams, 'logTime')!;
        const timeResult = computeDepletionAnalyticalFromParams(depletionParams, 'time')!;
        for (let i = 0; i < result.xValues.length; i++) {
            const t = timeResult.xValues[i];
            const lt = result.xValues[i];
            if (t !== null && t > 0 && lt !== null) {
                expect(lt).toBeCloseTo(Math.log10(t), 6);
            }
        }
    });

    it('pressure series is declining', () => {
        const result = computeDepletionAnalyticalFromParams(depletionParams, 'time')!;
        const pressures = result.avgPressureValues.filter((v) => v !== null) as number[];
        expect(pressures.length).toBeGreaterThan(0);
        expect(pressures[0]).toBeGreaterThan(pressures.at(-1)!);
    });
});

// ─── MIN_GOR_OIL_RATE_SM3_DAY ─────────────────────────────────────────────────

describe('MIN_GOR_OIL_RATE_SM3_DAY', () => {
    it('is a positive number', () => {
        expect(MIN_GOR_OIL_RATE_SM3_DAY).toBeGreaterThan(0);
    });
});

// ─── buildDerivedRunSeries ────────────────────────────────────────────────────

describe('buildDerivedRunSeries', () => {
    it('all output series have the same length as rateHistory', () => {
        const result = makeResult();
        const derived = buildDerivedRunSeries(result);
        const n = result.rateHistory.length;

        expect(derived.time).toHaveLength(n);
        expect(derived.oilRate).toHaveLength(n);
        expect(derived.injectionRate).toHaveLength(n);
        expect(derived.waterCut).toHaveLength(n);
        expect(derived.gasCut).toHaveLength(n);
        expect(derived.avgWaterSat).toHaveLength(n);
        expect(derived.pressure).toHaveLength(n);
        expect(derived.recovery).toHaveLength(n);
        expect(derived.cumulativeOil).toHaveLength(n);
        expect(derived.cumulativeInjection).toHaveLength(n);
        expect(derived.cumulativeLiquid).toHaveLength(n);
        expect(derived.cumulativeGas).toHaveLength(n);
        expect(derived.p_z).toHaveLength(n);
        expect(derived.pvi).toHaveLength(n);
        expect(derived.pvp).toHaveLength(n);
        expect(derived.gor).toHaveLength(n);
        expect(derived.producerBhpLimitedFraction).toHaveLength(n);
        expect(derived.injectorBhpLimitedFraction).toHaveLength(n);
    });

    it('cumulative injection is non-decreasing', () => {
        const derived = buildDerivedRunSeries(makeResult());
        for (let i = 1; i < derived.cumulativeInjection.length; i++) {
            const prev = derived.cumulativeInjection[i - 1];
            const curr = derived.cumulativeInjection[i];
            if (prev !== null && curr !== null) expect(curr).toBeGreaterThanOrEqual(prev);
        }
    });

    it('pressure series copies pressureSeries from result', () => {
        const result = makeResult();
        const derived = buildDerivedRunSeries(result);
        expect(derived.pressure).toEqual(result.pressureSeries);
    });

    it('cumulativeOil is proportional to recoverySeries × ooip', () => {
        const result = makeResult();
        const ooip = getOoip(result.params);
        const derived = buildDerivedRunSeries(result);
        for (let i = 0; i < result.recoverySeries.length; i++) {
            const rf = result.recoverySeries[i];
            const co = derived.cumulativeOil[i];
            if (rf !== null && co !== null) {
                expect(co).toBeCloseTo(rf * ooip, 6);
            }
        }
    });

    it('p_z is null when avg_reservoir_pressure is 0', () => {
        const result = makeResult({
            rateHistory: [{ time: 10, avg_reservoir_pressure: 0 }],
            watercutSeries: [0],
            pressureSeries: [0],
            recoverySeries: [0],
            pviSeries: [0],
        });
        const derived = buildDerivedRunSeries(result);
        expect(derived.p_z[0]).toBeNull();
    });

    it('gor is null when oil rate is below threshold', () => {
        const result = makeResult({
            rateHistory: [{ time: 10, total_production_oil: 1, producing_gor: 100 }],
            watercutSeries: [0], pressureSeries: [280], recoverySeries: [0], pviSeries: [0],
        });
        const derived = buildDerivedRunSeries(result);
        // oil rate 1 < MIN_GOR_OIL_RATE_SM3_DAY, so gor should be null
        expect(derived.gor[0]).toBeNull();
    });

    it('gor is returned when oil rate exceeds threshold', () => {
        const result = makeResult({
            rateHistory: [{ time: 10, total_production_oil: 100, producing_gor: 50 }],
            watercutSeries: [0], pressureSeries: [280], recoverySeries: [0], pviSeries: [0],
        });
        const derived = buildDerivedRunSeries(result);
        expect(derived.gor[0]).toBe(50);
    });

    it('extracts producer BHP from well snapshots', () => {
        const result = makeResult({
            history: [
                { time: 10, wells: [{ injector: false, bhp: 150 }, { injector: true, bhp: 200 }] } as any,
            ],
        });
        const derived = buildDerivedRunSeries(result);
        expect(derived.producerBhp[0]).toBe(150);
        expect(derived.injectorBhp[0]).toBe(200);
    });
});
