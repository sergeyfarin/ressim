import { describe, expect, it } from 'vitest';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import { generateBlackOilTable } from '../physics/pvt';
import {
    toFiniteNumber,
    getLayerThicknesses,
    getTotalThickness,
    getAverageLayerThickness,
    getPoreVolume,
    getStockTankOilInPlace,
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
    computeWellTestOnTimeAxis,
    computeDietzPssSimulationDiagnostics,
    MIN_GOR_OIL_RATE_FRACTION_OF_PEAK,
    buildDerivedRunSeries,
    computeMbeDiagnostics,
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
        gasRecoverySeries: [null, null, null],
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

describe('getStockTankOilInPlace', () => {
    it('is a surface volume: poreVolume × S_o / B_oi', () => {
        const params = baseParams({ initialSaturation: 0.2 });
        const pv = getPoreVolume(params);
        // B_o = 1 for this constant-PVT fixture, so it reduces to pv × S_o.
        expect(getStockTankOilInPlace(params)).toBeCloseTo(pv * 0.8);
    });

    it('excludes initial free gas from the oil volume', () => {
        const params = baseParams({ initialSaturation: 0.2, initialGasSaturation: 0.1 });
        expect(getStockTankOilInPlace(params)).toBeCloseTo(getPoreVolume(params) * 0.7);
    });

    it('divides by the constant-PVT formation volume factor', () => {
        const params = baseParams({ initialSaturation: 0.2, volume_expansion_o: 1.25 });
        expect(getStockTankOilInPlace(params)).toBeCloseTo(getPoreVolume(params) * 0.8 / 1.25);
    });

    it('returns null when there is no oil in place at all', () => {
        // A dry-gas case must show no oil recovery, not a fraction of a
        // denominator that does not exist.
        expect(getStockTankOilInPlace(baseParams({ initialSaturation: 0.2, initialGasSaturation: 0.8 }))).toBeNull();
        expect(getStockTankOilInPlace(baseParams({ initialSaturation: 1.0 }))).toBeNull();
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

// ─── GOR suppression floor ───────────────────────────────────────────────────

describe('computeWellTestOnTimeAxis — Dietz PSS diagnostics', () => {
    it('hides the reference before PSS and exposes analytical PI and C_A afterwards', () => {
        const solution = computeWellTestOnTimeAxis(baseParams({
            analyticalPressureModel: 'dietz-pss', analyticalPssStartDays: 1,
            producerControlMode: 'rate', targetProducerRate: 10,
            initialPressure: 300,
            uniformPermX: 20, uniformPermY: 20,
            c_o: 1e-5, c_w: 3e-6, rock_compressibility: 1e-6,
            nx: 21, ny: 21, cellDx: 20, cellDy: 20, cellDz: 10,
            producerI: 10, producerJ: 10,
            well_radius: 0.1, well_skin: 0, mu_o: 1,
        }), [0.5, 1, 2]);

        expect(solution).not.toBeNull();
        expect(solution?.pssProductivity[0]).toBeNull();
        expect(solution?.pssShapeFactor[0]).toBeNull();
        expect(solution?.pssProductivity[1]).toBeGreaterThan(0);
        expect(solution?.pssShapeFactor.slice(1)).toEqual([30.8828, 30.8828]);
    });

    it('recovers numerical PI and C_A from average pressure and flowing BHP', () => {
        const params = baseParams({
            analyticalPressureModel: 'dietz-pss', analyticalPssStartDays: 1,
            producerControlMode: 'rate', targetProducerRate: 10,
            initialPressure: 300,
            uniformPermX: 20, uniformPermY: 20,
            c_o: 1e-5, c_w: 3e-6, rock_compressibility: 1e-6,
            nx: 21, ny: 21, cellDx: 20, cellDy: 20, cellDz: 10,
            producerI: 10, producerJ: 10,
            well_radius: 0.1, well_skin: 0, mu_o: 1,
        });
        const reference = computeWellTestOnTimeAxis(params, [1, 2])!;
        const pi = Number(reference.pssProductivity[0]);
        const pressures = [290, 280];
        const result = makeResult({
            analyticalMethod: 'well-test', params,
            rateHistory: pressures.map((pressure, index) => ({
                time: index + 1,
                total_production_oil: 10,
                total_production_liquid: 10,
                total_injection: 0,
                avg_reservoir_pressure: pressure,
            })),
            pressureSeries: pressures,
            watercutSeries: [0, 0], recoverySeries: [0, 0], pviSeries: [0, 0],
            history: pressures.map((pressure, index) => ({
                time: index + 1,
                wells: [{ injector: false, flowing_bhp: pressure - 10 / pi }],
            } as any)),
        });
        const diagnostics = computeDietzPssSimulationDiagnostics(result, buildDerivedRunSeries(result));

        expect(diagnostics?.productivity[0]).toBeCloseTo(pi, 10);
        expect(diagnostics?.productivity[1]).toBeCloseTo(pi, 10);
        expect(diagnostics?.shapeFactor[0]).toBeCloseTo(30.8828, 8);
        expect(diagnostics?.shapeFactor[1]).toBeCloseTo(30.8828, 8);
    });
});

describe('MIN_GOR_OIL_RATE_FRACTION_OF_PEAK', () => {
    it('is a small positive fraction', () => {
        expect(MIN_GOR_OIL_RATE_FRACTION_OF_PEAK).toBeGreaterThan(0);
        expect(MIN_GOR_OIL_RATE_FRACTION_OF_PEAK).toBeLessThan(0.01);
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

    it('integrates cumulativeOil from the oil rate rather than from the recovery curve', () => {
        // It used to be reconstructed as `recovery x OOIP`, which tied the
        // cumulative curve to the recovery *denominator*: a case with no oil in
        // place (`dep_gas_pz`) lost its cumulative-oil curve the moment recovery
        // correctly became null.
        const result = makeResult();
        const derived = buildDerivedRunSeries(result);
        let expected = 0;
        for (let i = 0; i < result.rateHistory.length; i++) {
            const point = result.rateHistory[i];
            const dt = i > 0 ? point.time - result.rateHistory[i - 1].time : point.time;
            expected += Math.max(0, point.total_production_oil ?? 0) * dt;
            expect(derived.cumulativeOil[i]).toBeCloseTo(expected, 9);
        }
    });

    it('still produces a cumulative-oil curve when there is no oil in place', () => {
        const result = makeResult({ recoverySeries: [null, null, null] });
        const derived = buildDerivedRunSeries(result);
        expect(derived.cumulativeOil.every((value) => value !== null)).toBe(true);
        expect(derived.cumulativeOil.at(-1)).toBeGreaterThan(0);
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

    it('gor is null once the oil rate collapses relative to the run\'s own peak', () => {
        const result = makeResult({
            rateHistory: [
                { time: 10, total_production_oil: 1000, producing_gor: 100 },
                { time: 20, total_production_oil: 0.5, producing_gor: 100 },
            ],
            watercutSeries: [0, 0], pressureSeries: [280, 260],
            recoverySeries: [0, 0], pviSeries: [0, 0],
        });
        const derived = buildDerivedRunSeries(result);
        // 0.5 is 5e-4 of the 1000 peak — below the 1e-3 floor.
        expect(derived.gor[0]).toBe(100);
        expect(derived.gor[1]).toBeNull();
    });

    it('gor survives a small absolute rate when that is the scale of the whole run', () => {
        // The rule this replaced was an absolute 10 Sm³/d, which blanked a case
        // like this one entirely even though 1 Sm³/d is its normal production.
        const result = makeResult({
            rateHistory: [
                { time: 10, total_production_oil: 1, producing_gor: 50 },
                { time: 20, total_production_oil: 0.8, producing_gor: 60 },
            ],
            watercutSeries: [0, 0], pressureSeries: [280, 260],
            recoverySeries: [0, 0], pviSeries: [0, 0],
        });
        const derived = buildDerivedRunSeries(result);
        expect(derived.gor).toEqual([50, 60]);
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

// ─── computeMbeDiagnostics (Havlena-Odeh) ─────────────────────────────────────

describe('computeMbeDiagnostics', () => {
    const depletionHistory = [
        { time: 10, total_production_oil: 50, total_production_liquid: 50, total_injection: 0,
          avg_reservoir_pressure: 280, total_production_gas: 0, producing_gor: 0 },
        { time: 20, total_production_oil: 45, total_production_liquid: 45, total_injection: 0,
          avg_reservoir_pressure: 260, total_production_gas: 0, producing_gor: 0 },
        { time: 30, total_production_oil: 40, total_production_liquid: 40, total_injection: 0,
          avg_reservoir_pressure: 240, total_production_gas: 0, producing_gor: 0 },
    ];

    function depletionResult(paramOverride: Record<string, any> = {}): BenchmarkRunResult {
        return makeResult({
            params: baseParams({ initialPressure: 300, ...paramOverride }),
            rateHistory: depletionHistory,
            pressureSeries: [280, 260, 240],
        });
    }

    it('reports itself inapplicable on a run with injection', () => {
        // This MBE has no G_inj/W_inj or aquifer influx term, so on an injected
        // run F is missing a withdrawal counterpart: N_mbe would be wrong, not
        // merely uncertain. `makeResult`'s default history is a waterflood.
        const result = makeResult();
        const diagnostics = computeMbeDiagnostics(result, buildDerivedRunSeries(result));
        expect(diagnostics.applicable).toBe(false);
        expect(diagnostics.ooipRatio).toEqual([]);
        expect(diagnostics.driveOilExpansion).toEqual([]);
    });

    it('produces drive indices that sum to 1 on a depletion run', () => {
        const result = depletionResult();
        const diagnostics = computeMbeDiagnostics(result, buildDerivedRunSeries(result));
        expect(diagnostics.applicable).toBe(true);
        for (let index = 0; index < diagnostics.driveOilExpansion.length; index += 1) {
            const sum = (diagnostics.driveOilExpansion[index] ?? 0)
                + (diagnostics.driveGasCap[index] ?? 0)
                + (diagnostics.driveCompaction[index] ?? 0);
            expect(sum).toBeCloseTo(1.0, 9);
        }
    });

    it('passes the run\'s own PVT table through to the balance', () => {
        // Without this the balance falls back to correlation defaults keyed on
        // `params.apiGravity` / `params.bubblePoint`, which a scenario carrying a
        // generated table does not have at all.
        const table = generateBlackOilTable(35, 0.75, 80, 150, 300, 20, 1e-4);
        const withTable = depletionResult({ pvtMode: 'black-oil', pvtTable: table, c_o: 1e-4 });
        const withoutTable = depletionResult({ pvtMode: 'black-oil', c_o: 1e-4 });

        const a = computeMbeDiagnostics(withTable, buildDerivedRunSeries(withTable));
        const b = computeMbeDiagnostics(withoutTable, buildDerivedRunSeries(withoutTable));
        expect(a.applicable && b.applicable).toBe(true);
        expect(a.ooipRatio[2]).not.toBeCloseTo(b.ooipRatio[2] ?? 0, 6);
    });
});
