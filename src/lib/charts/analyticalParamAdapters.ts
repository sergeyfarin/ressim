/**
 * analyticalParamAdapters.ts — bridges raw scenario params → typed analytical
 * inputs and results for the chart layer.
 *
 * All functions here are pure: they take a `params: Record<string, any>` (or a
 * BenchmarkRunResult) and return typed data. No Chart.js, no curve config, no
 * DOM. Every function is independently testable.
 *
 * Consumed by buildChartData.ts (orchestrator), referenceOverlayBuilders.ts,
 * and sweepPanelBuilder.ts.
 */

import { calculateAnalyticalProduction, calculateGasOilAnalyticalProduction } from '../analytical/fractionalFlow';
import type { RockProps, FluidProps, GasOilRockProps, GasOilFluidProps } from '../analytical/fractionalFlow';
import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { calculateMaterialBalance } from '../analytical/materialBalance';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import type { AnalyticalOverlayMode } from '../catalog/scenarios';
import type { DerivedRunSeries } from './axisAdapters';
import type { RateChartXAxisMode } from './rateChartLayoutConfig';

// ─── Numeric utilities ────────────────────────────────────────────────────────

/** Coerces `value` to a finite number, returning `fallback` for NaN/Infinity/null/undefined. */
export function toFiniteNumber(value: unknown, fallback: number): number {
    const numeric = Number(value);
    return Number.isFinite(numeric) ? numeric : fallback;
}

// ─── Geometry / reservoir volume helpers ──────────────────────────────────────

/**
 * Returns per-layer thicknesses (m). Falls back to `cellDz` when
 * `cellDzPerLayer` is absent or empty.
 */
export function getLayerThicknesses(params: Record<string, any>): number[] {
    const nz = Math.max(1, Math.round(toFiniteNumber(params.nz, 1)));
    const fallback = Math.max(1e-12, toFiniteNumber(params.cellDz, 1));
    if (!Array.isArray(params.cellDzPerLayer) || params.cellDzPerLayer.length === 0) {
        return Array.from({ length: nz }, () => fallback);
    }
    return Array.from({ length: nz }, (_, index) => {
        const thickness = toFiniteNumber(params.cellDzPerLayer[index], fallback);
        return thickness > 0 ? thickness : fallback;
    });
}

export function getTotalThickness(params: Record<string, any>): number {
    return getLayerThicknesses(params).reduce((sum, t) => sum + t, 0);
}

export function getAverageLayerThickness(params: Record<string, any>): number {
    const layers = getLayerThicknesses(params);
    return layers.reduce((sum, t) => sum + t, 0) / layers.length;
}

/** Bulk pore volume (m³). Supports per-layer cellDz via getLayerThicknesses. */
export function getPoreVolume(params: Record<string, any>): number {
    return toFiniteNumber(params.nx, 1)
        * toFiniteNumber(params.ny, 1)
        * toFiniteNumber(params.cellDx, 10)
        * toFiniteNumber(params.cellDy, 10)
        * getTotalThickness(params)
        * toFiniteNumber(params.reservoirPorosity ?? params.porosity, 0.2);
}

/** Original oil in place (m³). */
export function getOoip(params: Record<string, any>): number {
    return getPoreVolume(params) * Math.max(0, 1 - toFiniteNumber(params.initialSaturation, 0.3));
}

/**
 * Returns permeability values per layer (mD). Uses `layerPermsX` in perLayer
 * mode, otherwise fills all layers with `uniformPermX`.
 */
export function getLayerPermeabilities(params: Record<string, any>): number[] {
    const nz = toFiniteNumber(params.nz, 1);
    if (
        String(params.permMode) === 'perLayer' &&
        Array.isArray(params.layerPermsX) &&
        params.layerPermsX.length > 1
    ) {
        return params.layerPermsX.map(Number);
    }
    if (nz > 1) {
        return Array.from({ length: nz }, () => toFiniteNumber(params.uniformPermX, 100));
    }
    return [toFiniteNumber(params.uniformPermX, 100)];
}

// ─── Rock / fluid property extraction ────────────────────────────────────────

export function extractRockProps(params: Record<string, any>): RockProps {
    return {
        s_wc:     toFiniteNumber(params.s_wc, 0.1),
        s_or:     toFiniteNumber(params.s_or, 0.1),
        n_w:      toFiniteNumber(params.n_w, 2),
        n_o:      toFiniteNumber(params.n_o, 2),
        k_rw_max: toFiniteNumber(params.k_rw_max, 1),
        k_ro_max: toFiniteNumber(params.k_ro_max, 1),
    };
}

export function extractFluidProps(params: Record<string, any>): FluidProps {
    return {
        mu_w: toFiniteNumber(params.mu_w, 0.5),
        mu_o: toFiniteNumber(params.mu_o, 1),
    };
}

export function extractGasOilRockProps(params: Record<string, any>): GasOilRockProps {
    return {
        s_wc:     toFiniteNumber(params.s_wc, 0.2),
        s_gc:     toFiniteNumber(params.s_gc, 0.05),
        s_gr:     toFiniteNumber(params.s_gr, 0.05),
        s_org:    toFiniteNumber(params.s_org, 0.20),
        n_o:      toFiniteNumber(params.n_o, 2),
        n_g:      toFiniteNumber(params.n_g, 1.5),
        k_ro_max: toFiniteNumber(params.k_ro_max, 1),
        k_rg_max: toFiniteNumber(params.k_rg_max, 0.8),
    };
}

export function extractGasOilFluidProps(params: Record<string, any>): GasOilFluidProps {
    return {
        mu_o: toFiniteNumber(params.mu_o, 2),
        mu_g: toFiniteNumber(params.mu_g, 0.02),
    };
}

// ─── Depletion param extraction ───────────────────────────────────────────────

/**
 * Builds the full `DepletionAnalyticalParams` object from raw scenario params,
 * using a caller-supplied `timeHistory` array.
 */
function buildDepletionParams(
    params: Record<string, any>,
    timeHistory: number[],
): Parameters<typeof calculateDepletionAnalyticalProduction>[0] {
    return {
        reservoir: {
            length: toFiniteNumber(params.nx, 1) * toFiniteNumber(params.cellDx, 10),
            area: toFiniteNumber(params.ny, 1)
                * toFiniteNumber(params.cellDy, 10)
                * getTotalThickness(params),
            porosity: toFiniteNumber(params.reservoirPorosity ?? params.porosity, 0.2),
        },
        timeHistory,
        minTimeDays: toFiniteNumber(params.analyticalDepletionStartDays, 0),
        initialSaturation: toFiniteNumber(params.initialSaturation, 0.3),
        nz: toFiniteNumber(params.nz, 1),
        permMode: String(params.permMode ?? 'uniform'),
        uniformPermX: toFiniteNumber(params.uniformPermX, 100),
        uniformPermY: toFiniteNumber(params.uniformPermY ?? params.uniformPermX, 100),
        layerPermsX: Array.isArray(params.layerPermsX) ? params.layerPermsX.map(Number) : [],
        layerPermsY: Array.isArray(params.layerPermsY) ? params.layerPermsY.map(Number) : [],
        cellDx: toFiniteNumber(params.cellDx, 10),
        cellDy: toFiniteNumber(params.cellDy, 10),
        cellDz: getAverageLayerThickness(params),
        wellRadius: toFiniteNumber(params.well_radius, 0.1),
        wellSkin: toFiniteNumber(params.well_skin, 0),
        muO: toFiniteNumber(params.mu_o, 1),
        sWc: toFiniteNumber(params.s_wc, 0.1),
        sOr: toFiniteNumber(params.s_or, 0.1),
        nO: toFiniteNumber(params.n_o, 2),
        c_o: toFiniteNumber(params.c_o, 1e-5),
        c_w: toFiniteNumber(params.c_w, 3e-6),
        cRock: toFiniteNumber(params.rock_compressibility, 1e-6),
        initialPressure: toFiniteNumber(params.initialPressure, 300),
        producerBhp: toFiniteNumber(params.producerBhp, 100),
        depletionRateScale: toFiniteNumber(params.analyticalDepletionRateScale, 1),
        arpsB: toFiniteNumber(params.analyticalArpsB, 0),
        nx: toFiniteNumber(params.nx, 1),
        ny: toFiniteNumber(params.ny, 1),
        producerI: params.producerI != null ? toFiniteNumber(params.producerI, 0) : undefined,
        producerJ: params.producerJ != null ? toFiniteNumber(params.producerJ, 0) : undefined,
    };
}

// ─── Analytical overlay signature / deduplication ────────────────────────────

/** Stable key for BL physics — used to decide shared vs. per-result overlays. */
export function getBuckleyLeverettOverlaySignature(params: Record<string, any>): string {
    return JSON.stringify({
        rock: extractRockProps(params),
        fluid: extractFluidProps(params),
        initialSaturation: toFiniteNumber(params.initialSaturation, toFiniteNumber(params.s_wc, 0.1)),
    });
}

/** Returns true when any two param sets have different BL physics. */
export function hasDistinctBuckleyLeverettOverlays(paramSets: Array<Record<string, any>>): boolean {
    if (paramSets.length <= 1) return false;
    return new Set(paramSets.map(getBuckleyLeverettOverlaySignature)).size > 1;
}

export function getGasOilBLOverlaySignature(params: Record<string, any>): string {
    return JSON.stringify({
        rock: extractGasOilRockProps(params),
        fluid: extractGasOilFluidProps(params),
        initialGasSaturation: toFiniteNumber(params.initialGasSaturation, 0),
    });
}

export function hasDistinctGasOilBLOverlays(paramSets: Array<Record<string, any>>): boolean {
    if (paramSets.length <= 1) return false;
    return new Set(paramSets.map(getGasOilBLOverlaySignature)).size > 1;
}

/** Resolves the effective overlay mode from scenario config + physics. */
export function resolveOverlayMode(input: {
    requested: AnalyticalOverlayMode | null | undefined;
    distinctByPhysics: boolean;
    analyticalPerVariant?: boolean;
}): 'shared' | 'per-result' {
    if (input.requested === 'shared') return 'shared';
    if (input.requested === 'per-result') return 'per-result';
    if (input.analyticalPerVariant) return 'per-result';
    return input.distinctByPhysics ? 'per-result' : 'shared';
}

// ─── BL analytical computation ────────────────────────────────────────────────

const BL_PVI_GRID_N = 150;
const BL_PVI_MAX = 3.0;

/** Default PVI grid [0 .. 3.0] with 150 points used by BL preview overlays. */
export function defaultBLPviGrid(): number[] {
    return Array.from({ length: BL_PVI_GRID_N }, (_, i) => (i / (BL_PVI_GRID_N - 1)) * BL_PVI_MAX);
}

/**
 * Computes the Buckley-Leverett analytical solution from raw scenario params.
 *
 * When `options` is omitted the solution is computed on the default PVI grid
 * and returned with PVI as x-values (suitable for overlay on PVI axis).
 *
 * When `options` is provided the solution is evaluated at the supplied
 * `timeHistory` / `injectionRateSeries` and returned with `xValues` as the
 * x-axis (suitable for overlay on any other axis after axis conversion).
 */
export function computeBLAnalyticalFromParams(
    params: Record<string, any>,
    options?: {
        xValues?: Array<number | null>;
        timeHistory?: number[];
        injectionRateSeries?: number[];
        poreVolume?: number;
        recoveryDenominator?: number;
    },
): {
    xValues: Array<number | null>;
    waterCut: Array<number | null>;
    recovery: Array<number | null>;
    cumulativeOil: Array<number | null>;
    oilRate: Array<number | null>;
} | null {
    const defaultPvi = defaultBLPviGrid();
    const xValues = options?.xValues ?? defaultPvi;
    const timeHistory = options?.timeHistory ?? defaultPvi;
    const injRates = options?.injectionRateSeries ?? new Array(timeHistory.length).fill(1);

    let analyticalProduction: Array<{ oilRate: number; waterRate: number; cumulativeOil: number }>;
    try {
        analyticalProduction = calculateAnalyticalProduction(
            extractRockProps(params),
            extractFluidProps(params),
            toFiniteNumber(params.initialSaturation, toFiniteNumber(params.s_wc, 0.1)),
            timeHistory,
            injRates,
            options?.poreVolume ?? 1,
        );
    } catch {
        return null;
    }

    const initialSaturation = toFiniteNumber(params.initialSaturation, toFiniteNumber(params.s_wc, 0.1));
    const ooip = Math.max(1e-12, options?.recoveryDenominator ?? (1 - initialSaturation));

    const waterCut = analyticalProduction.map((pt) => {
        const total = Math.max(0, pt.oilRate + pt.waterRate);
        return total > 1e-12 ? pt.waterRate / total : 0;
    });
    const recovery = analyticalProduction.map((pt) => Math.max(0, Math.min(1, pt.cumulativeOil / ooip)));
    const cumulativeOil = analyticalProduction.map((pt) => pt.cumulativeOil);
    const oilRate = analyticalProduction.map((pt) => pt.oilRate);

    return { xValues, waterCut, recovery, cumulativeOil, oilRate };
}

// ─── Gas-Oil BL analytical computation ───────────────────────────────────────

const GAS_OIL_PVI_GRID_N = 150;
const GAS_OIL_PVI_MAX = 3.0;

/** Default PVI grid [0 .. 3.0] for gas-oil BL preview overlays. */
export function defaultGasOilBLPviGrid(): number[] {
    return Array.from({ length: GAS_OIL_PVI_GRID_N }, (_, i) => (i / (GAS_OIL_PVI_GRID_N - 1)) * GAS_OIL_PVI_MAX);
}

/**
 * Computes the gas-oil Buckley-Leverett analytical solution from raw scenario
 * params over the default PVI grid (for PVI-axis overlay).
 */
export function computeGasOilBLAnalyticalFromParams(params: Record<string, any>): {
    pviValues: number[];
    gasCut: Array<number | null>;
    recovery: Array<number | null>;
    cumulativeOil: Array<number | null>;
} | null {
    const pviValues = defaultGasOilBLPviGrid();
    const injRates = new Array(pviValues.length).fill(1);

    let analyticalProduction: Array<{ oilRate: number; gasRate: number; cumulativeOil: number }>;
    try {
        analyticalProduction = calculateGasOilAnalyticalProduction(
            extractGasOilRockProps(params),
            extractGasOilFluidProps(params),
            toFiniteNumber(params.initialGasSaturation, 0),
            pviValues,
            injRates,
            1, // unit pore volume — PVI = cumulative injection directly
        );
    } catch {
        return null;
    }
    if (!analyticalProduction.length) return null;

    const s_wc = toFiniteNumber(params.s_wc, 0.2);
    const ooip = 1 - s_wc; // oil initially in place as fraction of PV

    const gasCut = analyticalProduction.map((pt) => {
        const total = Math.max(0, pt.oilRate + pt.gasRate);
        return total > 1e-12 ? pt.gasRate / total : 0;
    });
    const recovery = analyticalProduction.map((pt) =>
        ooip > 1e-12 ? Math.max(0, Math.min(1, pt.cumulativeOil / ooip)) : null,
    );
    const cumulativeOil = analyticalProduction.map((pt) => pt.cumulativeOil);

    return { pviValues, gasCut, recovery, cumulativeOil };
}

// ─── Depletion analytical computation ────────────────────────────────────────

/**
 * Computes the characteristic time constant τ (days) for a depletion scenario.
 * Returns null on any numerical failure (used as the tD x-axis denominator).
 */
export function computeDepletionTau(params: Record<string, any>): number | null {
    try {
        const result = calculateDepletionAnalyticalProduction(buildDepletionParams(params, [1]));
        return result.meta.tau ?? null;
    } catch {
        return null;
    }
}

/**
 * Computes the depletion analytical solution on a caller-supplied time axis.
 * Used by reference overlay builders that already have the simulation time vector
 * (as opposed to `computeDepletionAnalyticalFromParams` which builds a synthetic grid).
 */
export function computeDepletionOnTimeAxis(
    params: Record<string, any>,
    timeHistory: number[],
): ReturnType<typeof calculateDepletionAnalyticalProduction> {
    return calculateDepletionAnalyticalProduction(buildDepletionParams(params, timeHistory));
}

/**
 * Computes the depletion analytical solution over a synthetic time grid built
 * from `params.steps` × `params.delta_t_days`. Used for preview overlays before
 * any simulation result (and therefore no real time axis) is available.
 *
 * Returns null on numerical failure so callers can skip the curve.
 */
export function computeDepletionAnalyticalFromParams(
    params: Record<string, any>,
    xAxisMode: RateChartXAxisMode,
): {
    xValues: (number | null)[];
    oilRates: (number | null)[];
    recoveryValues: (number | null)[];
    cumulativeOilValues: (number | null)[];
    avgPressureValues: (number | null)[];
} | null {
    const steps = toFiniteNumber(params.steps, 200);
    const dt = toFiniteNumber(params.delta_t_days, 5);
    const timeHistory = Array.from({ length: steps }, (_, i) => (i + 1) * dt);

    let analyticalResult: ReturnType<typeof calculateDepletionAnalyticalProduction>;
    try {
        analyticalResult = calculateDepletionAnalyticalProduction(
            buildDepletionParams(params, timeHistory),
        );
    } catch {
        return null;
    }

    const ooip = getOoip(params);
    const tau = analyticalResult.meta.tau ?? null;
    const xValues = analyticalResult.production.map((pt) => {
        if (xAxisMode === 'logTime') return pt.time > 0 ? Math.log10(pt.time) : null;
        if (xAxisMode === 'tD' && Number.isFinite(tau) && (tau as number) > 0) return pt.time / (tau as number);
        return pt.time;
    });

    return {
        xValues,
        oilRates: analyticalResult.production.map((pt) => pt.oilRate),
        recoveryValues: analyticalResult.production.map((pt) =>
            ooip > 1e-12 ? Math.max(0, Math.min(1, pt.cumulativeOil / ooip)) : null,
        ),
        cumulativeOilValues: analyticalResult.production.map((pt) => pt.cumulativeOil),
        avgPressureValues: analyticalResult.production.map((pt) => pt.avgPressure),
    };
}

// ─── Material balance diagnostics ─────────────────────────────────────────────

export type MbeDiagnostics = {
    ooipRatio: Array<number | null>;
    driveOilExpansion: Array<number | null>;
    driveGasCap: Array<number | null>;
    driveCompaction: Array<number | null>;
};

/**
 * Computes Havlena-Odeh MBE diagnostics from simulation output:
 * - OOIP ratio (N_mbe / N_volumetric) — should converge to ~1.0
 * - Drive indices (oil expansion, gas cap, compaction) — fractional, sum to 1.0
 */
export function computeMbeDiagnostics(
    result: BenchmarkRunResult,
    derived: DerivedRunSeries,
): MbeDiagnostics {
    const params = result.params;
    const poreVolume = getPoreVolume(params);
    const pvtMode = String(params.pvtMode ?? 'constant');

    const n = result.rateHistory.length;
    const cumOil: number[] = [];
    const cumGas: number[] = [];
    const cumWater: number[] = [];
    let co = 0, cg = 0, cw = 0;

    for (let i = 0; i < n; i++) {
        const point = result.rateHistory[i];
        const dt = i > 0
            ? Math.max(0, toFiniteNumber(point.time, 0) - toFiniteNumber(result.rateHistory[i - 1]?.time, 0))
            : Math.max(0, toFiniteNumber(point.time, 0));
        co += Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0))) * dt;
        const gasRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_gas, 0)));
        cg += gasRate * dt;
        const liqRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_liquid, 0)));
        const oilRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0)));
        cw += Math.max(0, liqRate - oilRate) * dt;
        cumOil.push(co);
        cumGas.push(cg);
        cumWater.push(cw);
    }

    const mbeResult = calculateMaterialBalance({
        initialPressure: toFiniteNumber(params.initialPressure, 300),
        initialWaterSaturation: toFiniteNumber(params.initialSaturation, 0.3),
        initialGasSaturation: toFiniteNumber(params.initialGasSaturation, 0),
        porosity: toFiniteNumber(params.reservoirPorosity ?? params.porosity, 0.2),
        poreVolume,
        c_w: toFiniteNumber(params.c_w, 3e-6),
        c_rock: toFiniteNumber(params.rock_compressibility, 1e-6),
        pvtMode: pvtMode === 'black-oil' ? 'black-oil' : 'constant',
        Bo_constant: toFiniteNumber(params.volume_expansion_o, 1.0),
        Bw_constant: toFiniteNumber(params.volume_expansion_w, 1.0),
        c_o: toFiniteNumber(params.c_o, 1e-5),
        apiGravity: toFiniteNumber(params.apiGravity, 30),
        gasSpecificGravity: toFiniteNumber(params.gasSpecificGravity, 0.7),
        reservoirTemperature: toFiniteNumber(params.reservoirTemperature, 80),
        bubblePoint: toFiniteNumber(params.bubblePoint, 150),
        pressureHistory: derived.pressure.map((v) => toFiniteNumber(v, 0)),
        cumulativeOilSC: cumOil,
        cumulativeGasSC: cumGas,
        cumulativeWaterSC: cumWater,
        timeHistory: derived.time,
    });

    const Nvol = mbeResult.volumetricOoip;
    return {
        ooipRatio: mbeResult.points.map((pt) =>
            pt.N_mbe === null || Nvol < 1e-12 ? null : pt.N_mbe / Nvol,
        ),
        driveOilExpansion: mbeResult.points.map((pt) =>
            pt.Et > 1e-15 ? pt.driveIndex_oilExpansion : null,
        ),
        driveGasCap: mbeResult.points.map((pt) =>
            pt.Et > 1e-15 ? pt.driveIndex_gasCap : null,
        ),
        driveCompaction: mbeResult.points.map((pt) =>
            pt.Et > 1e-15 ? pt.driveIndex_compaction : null,
        ),
    };
}

// ─── Run series construction ──────────────────────────────────────────────────

/** Minimum oil rate (m³/day) below which GOR is suppressed to avoid division noise. */
export const MIN_GOR_OIL_RATE_SM3_DAY = 10.0;

function extractWellBhpHistory(result: BenchmarkRunResult): {
    historyTime: number[];
    producerBhp: Array<number | null>;
    injectorBhp: Array<number | null>;
} {
    const historyTime: number[] = [];
    const producerBhp: Array<number | null> = [];
    const injectorBhp: Array<number | null> = [];

    for (const snapshot of result.history) {
        historyTime.push(toFiniteNumber(snapshot.time, 0));
        const wells = Array.isArray(snapshot.wells) ? snapshot.wells : [];
        const producer = wells.find((well) => well?.injector === false && Number.isFinite(well?.bhp));
        const injector = wells.find((well) => well?.injector === true && Number.isFinite(well?.bhp));
        producerBhp.push(Number.isFinite(producer?.bhp) ? Number(producer?.bhp) : null);
        injectorBhp.push(Number.isFinite(injector?.bhp) ? Number(injector?.bhp) : null);
    }

    return { historyTime, producerBhp, injectorBhp };
}

/**
 * Builds the full `DerivedRunSeries` for a completed simulation result.
 * This is the single constructor for `DerivedRunSeries` — all other code reads
 * from the derived object rather than recomputing from `rateHistory` directly.
 */
export function buildDerivedRunSeries(result: BenchmarkRunResult): DerivedRunSeries {
    const poreVolume = getPoreVolume(result.params);
    const ooip = getOoip(result.params);
    const wellBhpHistory = extractWellBhpHistory(result);

    let cumulativeInjection = 0;
    let cumulativeLiquid = 0;
    let cumulativeGas = 0;

    const cumulativeInjectionSeries: Array<number | null> = [];
    const cumulativeLiquidSeries: Array<number | null> = [];
    const cumulativeGasSeries: Array<number | null> = [];
    const pZSeries: Array<number | null> = [];

    for (let index = 0; index < result.rateHistory.length; index += 1) {
        const point = result.rateHistory[index];
        const dt = index > 0
            ? Math.max(0, toFiniteNumber(point.time, 0) - toFiniteNumber(result.rateHistory[index - 1]?.time, 0))
            : Math.max(0, toFiniteNumber(point.time, 0));
        cumulativeInjection += Math.max(0, toFiniteNumber(point.total_injection, 0)) * dt;
        cumulativeLiquid += Math.max(0, Math.abs(toFiniteNumber(point.total_production_liquid, 0))) * dt;
        cumulativeGas += Math.max(0, Math.abs(toFiniteNumber(point.total_production_gas, 0))) * dt;

        cumulativeInjectionSeries.push(cumulativeInjection);
        cumulativeLiquidSeries.push(cumulativeLiquid);
        cumulativeGasSeries.push(cumulativeGas);

        const pressure = toFiniteNumber(point.avg_reservoir_pressure, 0);
        // Simple z-factor = 1 for now (real gas correlation deferred).
        pZSeries.push(pressure > 0 ? pressure : null);
    }

    return {
        time: result.rateHistory.map((point) => toFiniteNumber(point.time, 0)),
        historyTime: wellBhpHistory.historyTime || [],
        oilRate: result.rateHistory.map((point) => Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0)))),
        injectionRate: result.rateHistory.map((point) => Math.max(0, toFiniteNumber(point.total_injection, 0))),
        waterCut: [...result.watercutSeries],
        gasCut: result.rateHistory.map((point) => {
            const gasRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_gas, 0)));
            const oilRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0)));
            const total = gasRate + oilRate;
            return total > 1e-12 ? gasRate / total : 0;
        }),
        avgWaterSat: result.rateHistory.map((point) => {
            const value = point.avg_water_saturation;
            return Number.isFinite(value) ? Number(value) : null;
        }),
        pressure: [...result.pressureSeries],
        producerBhp: wellBhpHistory.producerBhp,
        injectorBhp: wellBhpHistory.injectorBhp,
        recovery: [...result.recoverySeries],
        cumulativeOil: result.recoverySeries.map((value) =>
            Number.isFinite(value) && ooip > 1e-12 ? Number(value) * ooip : null,
        ),
        cumulativeInjection: cumulativeInjectionSeries,
        cumulativeLiquid: cumulativeLiquidSeries,
        cumulativeGas: cumulativeGasSeries,
        p_z: pZSeries,
        pvi: [...result.pviSeries],
        pvp: cumulativeLiquidSeries.map((value) =>
            poreVolume > 1e-12 && Number.isFinite(value) ? Number(value) / poreVolume : null,
        ),
        gor: result.rateHistory.map((point) => {
            const oilRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0)));
            if (oilRate <= MIN_GOR_OIL_RATE_SM3_DAY) return null;
            const value = toFiniteNumber(point.producing_gor as number, 0);
            return value > 0 ? value : null;
        }),
        producerBhpLimitedFraction: result.rateHistory.map((point) => {
            const value = point.producer_bhp_limited_fraction;
            return Number.isFinite(value) ? Number(value) : null;
        }),
        injectorBhpLimitedFraction: result.rateHistory.map((point) => {
            const value = point.injector_bhp_limited_fraction;
            return Number.isFinite(value) ? Number(value) : null;
        }),
    };
}
