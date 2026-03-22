/**
 * Pure-function implementations of sweep efficiency analytics.
 *
 * Areal sweep: Craig (1971) five-spot pattern correlations — polynomial fits
 *   to published graphical data.  E_A(M) at breakthrough, then Dyes-Caudle-
 *   Erickson post-breakthrough growth with PVI.
 *
 * Vertical sweep: Dykstra-Parsons (1950) piston-like displacement in non-
 *   communicating layers of varying permeability.
 */

import type { RockProps, FluidProps } from './fractionalFlow';
import { computeBLRecoveryVsPVI, computeWelgeMetrics } from './fractionalFlow';

// ────────────────────────────────────────────────────────────────────────────
// Types
// ────────────────────────────────────────────────────────────────────────────

export type SweepPoint = { pvi: number; efficiency: number };

export type ArealSweepResult = {
    curve: SweepPoint[];
    mobilityRatio: number;
    eaAtBreakthrough: number;
    pviAtBreakthrough: number;
};

export type VerticalSweepResult = {
    curve: SweepPoint[];
    vdp: number;
};

export type CombinedSweepResult = {
    arealSweep: ArealSweepResult;
    verticalSweep: VerticalSweepResult;
    combined: SweepPoint[];
};

/**
 * Which sweep components are physically meaningful for a given grid geometry.
 *
 *  'areal'    — 2D XY (nz=1 or uniform layers).  E_A from Craig; E_V = 1.
 *  'vertical' — 2D XZ line drive (ny=1, layered).  E_V from Dykstra-Parsons; E_A = 1.
 *  'both'     — 3D five-spot + layered.  E_vol = E_A × E_V.
 */
export type SweepGeometry = 'areal' | 'vertical' | 'both';

export type SweepComponentVisibility = {
    showAreal: boolean;
    showVertical: boolean;
};

export type SweepRFPoint = {
    pvi: number;
    /** RF from sweep model: E_vol(PVI) × E_D_BL(PVI_local). Primary analytical prediction. */
    rfSweep: number;
    /** RF from 1D BL alone (perfect sweep, E_vol = 1). Upper-bound reference. */
    rfBL1D: number;
    /** Volumetric sweep efficiency at this PVI. */
    eVol: number;
    /** Displacement efficiency within swept zone at this PVI. */
    eD: number;
};

export type SweepRFResult = {
    curve: SweepRFPoint[];
    /** Maximum possible displacement efficiency (piston model): (1−Sor−Swc)/(1−Swc). */
    edPiston: number;
    eAAtBreakthrough: number;
    vdp: number;
};

export type SimSweepPoint = {
    eA: number;
    eV: number;
    eVol: number;
};

export type SimSweepDiagnostics = {
    eA: number | null;
    eV: number | null;
    eVol: number;
    mobileOilRecovered: number | null;
};

type SimSweepGeometryContext = {
    geometry: SweepGeometry;
    injectorI?: number;
    injectorJ?: number;
    producerI?: number;
    producerJ?: number;
    cellDx?: number;
    cellDy?: number;
};

export function getSweepComponentVisibility(geometry: SweepGeometry): SweepComponentVisibility {
    return {
        showAreal: geometry !== 'vertical',
        showVertical: geometry !== 'areal',
    };
}

export function normalizeSimSweepPointForGeometry(
    point: SimSweepPoint,
    geometry: SweepGeometry,
): SimSweepPoint {
    if (geometry === 'vertical') {
        return {
            eA: 1,
            eV: point.eVol,
            eVol: point.eVol,
        };
    }

    if (geometry === 'areal') {
        return {
            eA: point.eA,
            eV: 1,
            eVol: point.eVol,
        };
    }

    return point;
}

function clamp01(value: number): number {
    return Math.max(0, Math.min(1, value));
}

function cellCenterCoordinate(index: number, spacing: number): number {
    return (index + 0.5) * Math.max(1e-12, spacing);
}

function projectedSweepPathCoordinate(
    i: number,
    j: number,
    nx: number,
    ny: number,
    context: SimSweepGeometryContext,
): number {
    const injectorI = Number.isFinite(context.injectorI) ? Number(context.injectorI) : 0;
    const injectorJ = Number.isFinite(context.injectorJ) ? Number(context.injectorJ) : 0;
    const producerI = Number.isFinite(context.producerI) ? Number(context.producerI) : Math.max(0, nx - 1);
    const producerJ = Number.isFinite(context.producerJ) ? Number(context.producerJ) : Math.max(0, ny - 1);
    const cellDx = Math.max(1e-12, Number(context.cellDx ?? 1));
    const cellDy = Math.max(1e-12, Number(context.cellDy ?? 1));

    const injectorX = cellCenterCoordinate(injectorI, cellDx);
    const injectorY = cellCenterCoordinate(injectorJ, cellDy);
    const producerX = cellCenterCoordinate(producerI, cellDx);
    const producerY = cellCenterCoordinate(producerJ, cellDy);
    const cellX = cellCenterCoordinate(i, cellDx);
    const cellY = cellCenterCoordinate(j, cellDy);

    const dirX = producerX - injectorX;
    const dirY = producerY - injectorY;
    const denom = dirX * dirX + dirY * dirY;
    if (denom <= 1e-12) return 0;

    const relX = cellX - injectorX;
    const relY = cellY - injectorY;
    return clamp01((relX * dirX + relY * dirY) / denom);
}

function saturationFrontWeight(sw: number, sweptThreshold: number): number {
    if (!Number.isFinite(sw) || sw <= sweptThreshold) return 0;
    return clamp01((sw - sweptThreshold) / Math.max(1e-12, 1 - sweptThreshold));
}

function weightedQuantile(entries: Array<{ coordinate: number; weight: number }>, quantile: number): number {
    const filtered = entries
        .filter((entry) => entry.weight > 1e-12 && Number.isFinite(entry.coordinate))
        .sort((a, b) => a.coordinate - b.coordinate);
    if (filtered.length === 0) return 0;

    const totalWeight = filtered.reduce((sum, entry) => sum + entry.weight, 0);
    if (totalWeight <= 1e-12) return 0;

    const target = clamp01(quantile) * totalWeight;
    let cumulative = 0;
    for (const entry of filtered) {
        cumulative += entry.weight;
        if (cumulative >= target - 1e-12) {
            return clamp01(entry.coordinate);
        }
    }
    return clamp01(filtered.at(-1)?.coordinate ?? 0);
}

function computeCombinedSimVerticalSweep(
    satWater: Float64Array | number[],
    nx: number,
    ny: number,
    nz: number,
    sweptThreshold: number,
    context: SimSweepGeometryContext,
): number {
    if (nx <= 1 || ny <= 1 || nz <= 1) {
        return 0;
    }

    const columnUnionEntries: Array<{ coordinate: number; weight: number }> = [];

    const columnCoordinates = new Float64Array(nx * ny);
    const columnUnionWeights = new Float64Array(nx * ny);
    for (let j = 0; j < ny; j += 1) {
        for (let i = 0; i < nx; i += 1) {
            const columnIndex = j * nx + i;
            columnCoordinates[columnIndex] = projectedSweepPathCoordinate(i, j, nx, ny, context);
        }
    }

    const layerFronts = new Float64Array(nz);
    for (let k = 0; k < nz; k += 1) {
        const entries: Array<{ coordinate: number; weight: number }> = [];
        for (let j = 0; j < ny; j += 1) {
            for (let i = 0; i < nx; i += 1) {
                const index = k * nx * ny + j * nx + i;
                const columnIndex = j * nx + i;
                const weight = saturationFrontWeight(Number(satWater[index]), sweptThreshold);
                entries.push({
                    coordinate: columnCoordinates[columnIndex],
                    weight,
                });
                columnUnionWeights[columnIndex] = Math.max(columnUnionWeights[columnIndex], weight);
            }
        }
        layerFronts[k] = weightedQuantile(entries, 0.5);
    }

    for (let columnIndex = 0; columnIndex < columnUnionWeights.length; columnIndex += 1) {
        columnUnionEntries.push({
            coordinate: columnCoordinates[columnIndex],
            weight: columnUnionWeights[columnIndex],
        });
    }

    const unionFront = weightedQuantile(columnUnionEntries, 0.5);
    if (unionFront <= 1e-9) return 0;

    const combinedFront = clamp01(layerFronts.reduce((sum, value) => sum + value, 0) / nz);
    return clamp01(combinedFront / unionFront);
}

export function computeSimSweepPointForGeometry(
    satWater: Float64Array | number[],
    nx: number,
    ny: number,
    nz: number,
    sweptThreshold: number,
    context: SimSweepGeometryContext,
): SimSweepPoint {
    const raw = computeSimSweepPoint(satWater, nx, ny, nz, sweptThreshold);
    if (context.geometry === 'both') {
        return {
            eA: raw.eA,
            eV: computeCombinedSimVerticalSweep(satWater, nx, ny, nz, sweptThreshold, context),
            eVol: raw.eVol,
        };
    }
    return normalizeSimSweepPointForGeometry(raw, context.geometry);
}

export function computeMobileOilRecoveredFraction(
    satOil: Float64Array | number[] | null | undefined,
    nx: number,
    ny: number,
    nz: number,
    initialOilSaturation: number,
    residualOilSaturation: number,
): number {
    const totalCells = nx * ny * nz;
    if (!satOil || satOil.length === 0 || totalCells <= 0) return 0;

    const initialMobileOilPerCell = Math.max(0, initialOilSaturation - residualOilSaturation);
    const initialMobileOil = totalCells * initialMobileOilPerCell;
    if (initialMobileOil <= 1e-12) return 0;

    let remainingMobileOil = 0;
    for (let index = 0; index < Math.min(totalCells, satOil.length); index += 1) {
        remainingMobileOil += Math.max(0, Number(satOil[index]) - residualOilSaturation);
    }

    return clamp01(1 - remainingMobileOil / initialMobileOil);
}

export function computeSimSweepDiagnosticsForGeometry(
    satWater: Float64Array | number[],
    satOil: Float64Array | number[] | null | undefined,
    nx: number,
    ny: number,
    nz: number,
    sweptThreshold: number,
    context: SimSweepGeometryContext,
    initialOilSaturation: number,
    residualOilSaturation: number,
): SimSweepDiagnostics {
    const raw = computeSimSweepPoint(satWater, nx, ny, nz, sweptThreshold);

    if (context.geometry === 'both') {
        return {
            eA: null,
            eV: null,
            eVol: raw.eVol,
            mobileOilRecovered: computeMobileOilRecoveredFraction(
                satOil,
                nx,
                ny,
                nz,
                initialOilSaturation,
                residualOilSaturation,
            ),
        };
    }

    const point = computeSimSweepPointForGeometry(satWater, nx, ny, nz, sweptThreshold, context);
    return {
        eA: point.eA,
        eV: point.eV,
        eVol: point.eVol,
        mobileOilRecovered: null,
    };
}

// ────────────────────────────────────────────────────────────────────────────
// Mobility ratio
// ────────────────────────────────────────────────────────────────────────────

/**
 * End-point mobility ratio M = (k_rw_max / μ_w) / (k_ro_max / μ_o).
 * M > 1 → unfavourable (water more mobile), M ≤ 1 → favourable.
 */
export function mobilityRatio(rock: RockProps, fluid: FluidProps): number {
    const lambdaW = rock.k_rw_max / Math.max(1e-12, fluid.mu_w);
    const lambdaO = rock.k_ro_max / Math.max(1e-12, fluid.mu_o);
    if (lambdaO <= 0) return Infinity;
    return lambdaW / lambdaO;
}

// ────────────────────────────────────────────────────────────────────────────
// Areal sweep – five-spot pattern (Craig 1971)
// ────────────────────────────────────────────────────────────────────────────

/**
 * Areal sweep efficiency at breakthrough for a confined five-spot pattern.
 *
 * Polynomial regression fit to Craig's (1971) graphical data:
 *   E_A(M=0.1) ≈ 0.95,  E_A(M=1) ≈ 0.70,  E_A(M=10) ≈ 0.38,  E_A(M=100) → ~0.15
 *
 * Uses log10(M) as the independent variable for a smooth 3rd-order fit.
 */
export function arealSweepAtBreakthrough(M: number): number {
    if (!Number.isFinite(M) || M <= 0) return 0;
    const x = Math.log10(Math.max(1e-3, Math.min(1e3, M)));
    // Regression coefficients fitted to Craig's five-spot breakthrough data:
    //   E_A = a0 + a1·x + a2·x² + a3·x³   where x = log10(M)
    // Anchored at: E_A(0.1)≈0.95, E_A(1)≈0.70, E_A(10)≈0.38, E_A(100)≈0.15
    const ea = 0.7 - 0.2238 * x - 0.0540 * x * x + 0.0091 * x * x * x;
    return Math.max(0, Math.min(1, ea));
}

function computeLocalDisplacementFrontPvi(rock: RockProps, fluid: FluidProps): number {
    const welge = computeWelgeMetrics(rock, fluid, rock.s_wc);
    return Math.max(0.01, welge.breakthroughPvi);
}

/**
 * Areal sweep efficiency vs cumulative PVI for a five-spot pattern.
 *
 * Before breakthrough (PVI < PVI_bt):  linear ramp from 0 to E_A_bt.
 * After breakthrough:  exponential growth toward 1.0, with the growth rate
 * depending on mobility ratio (unfavourable M → slower post-BT growth).
 *
 * Based on Dyes, Caudle & Erickson (1954) graphical correlations.
 *
 * @param localFrontPvi — 1D Buckley-Leverett breakthrough PVI in the locally
 *   swept zone. PVI at pattern breakthrough is E_A_bt × PVI_bt,local. Default
 *   1 keeps the function usable as a standalone correlation helper.
 */
export function arealSweepAtPvi(M: number, pvi: number, localFrontPvi: number = 1): number {
    if (pvi <= 0) return 0;
    const eaBt = arealSweepAtBreakthrough(M);
    if (eaBt <= 0) return 0;

    // Pattern breakthrough occurs when the average swept area reaches E_A_bt
    // and the local front in that swept area reaches its 1D BL breakthrough.
    const pviBt = eaBt * Math.max(0.01, localFrontPvi);

    if (pvi <= pviBt) {
        return Math.max(0, (pvi / pviBt) * eaBt);
    }

    // Post-breakthrough exponential growth rate.
    // Lower M → faster approach to E_A=1; higher M → slower.
    const alpha = Math.max(0.3, 1.5 - 0.5 * Math.log10(Math.max(1e-3, M)));
    const excessPvi = pvi / pviBt - 1;
    const ea = eaBt + (1 - eaBt) * (1 - Math.exp(-alpha * excessPvi));
    return Math.max(0, Math.min(1, ea));
}

/**
 * Generate an areal sweep curve  { pvi, efficiency }[]  for charting.
 */
export function arealSweepCurve(
    M: number,
    pviMax: number = 3.0,
    nPoints: number = 200,
    localFrontPvi: number = 1,
): SweepPoint[] {
    const result: SweepPoint[] = [];
    for (let i = 0; i <= nPoints; i++) {
        const pvi = (i / nPoints) * pviMax;
        result.push({ pvi, efficiency: arealSweepAtPvi(M, pvi, localFrontPvi) });
    }
    return result;
}

/**
 * Full areal sweep result including key metrics.
 */
export function computeArealSweep(
    rock: RockProps,
    fluid: FluidProps,
    pviMax: number = 3.0,
    nPoints: number = 200,
): ArealSweepResult {
    const M = mobilityRatio(rock, fluid);
    const eaBt = arealSweepAtBreakthrough(M);
    const localFrontPvi = computeLocalDisplacementFrontPvi(rock, fluid);
    const pviBt = eaBt * localFrontPvi;
    return {
        curve: arealSweepCurve(M, pviMax, nPoints, localFrontPvi),
        mobilityRatio: M,
        eaAtBreakthrough: eaBt,
        pviAtBreakthrough: pviBt,
    };
}

// ────────────────────────────────────────────────────────────────────────────
// Vertical sweep – Dykstra-Parsons (1950)
// ────────────────────────────────────────────────────────────────────────────

/**
 * Dykstra-Parsons coefficient   VDP = (k_50 − k_84.1) / k_50.
 * Assumes a log-normal permeability distribution.
 *
 * @param permeabilities — array of layer permeability values (any order).
 *   Returns 0 for empty / single-layer input.
 */
export function dykstraParsonsCoefficient(permeabilities: number[]): number {
    if (permeabilities.length <= 1) return 0;

    const sorted = [...permeabilities].sort((a, b) => b - a); // descending
    const n = sorted.length;

    // Percentile by linear interpolation of ranks
    function percentileValue(p: number): number {
        const rank = (p / 100) * (n - 1);
        const lo = Math.floor(rank);
        const hi = Math.ceil(rank);
        const frac = rank - lo;
        return sorted[lo] * (1 - frac) + sorted[hi] * frac;
    }

    const k50 = percentileValue(50);
    const k84 = percentileValue(84.1);
    if (k50 <= 0) return 0;

    return Math.max(0, Math.min(1, (k50 - k84) / k50));
}

/**
 * Dykstra-Parsons vertical sweep efficiency for N non-communicating layers
 * with piston-like displacement (constant pressure drop across all layers).
 *
 * Each layer has a permeability k_i and thickness h_i.  All layers share the
 * same total pressure drop ΔP.  The water front in each layer advances at a
 * rate that depends on its permeability and the mobility contrast between the
 * swept (water) zone behind the front and the un-swept (oil) zone ahead.
 *
 * **Mobility ratio effect** — After the fastest layer breaks through, it
 * flows entirely water at endpoint mobility λ_w = k_rw_max/μ_w.  For M > 1
 * (unfavourable), this high-mobility channel steals injection from remaining
 * layers, significantly slowing their front advance and reducing E_V.  The
 * model integrates this flow redistribution at each PVI step.
 *
 * @param localFrontPvi — 1D Buckley-Leverett breakthrough PVI in the local
 *   swept zone. Determines how quickly each layer front advances per unit PVI.
 *   Default 1 preserves the old standalone helper behaviour.
 *
 * Returns a   { pvi, efficiency }[]   curve.
 */
export function verticalSweep(
    layers: Array<{ perm: number; thickness: number }>,
    M: number,
    pviMax: number = 3.0,
    nPoints: number = 200,
    localFrontPvi: number = 1,
): SweepPoint[] {
    if (layers.length === 0) return [{ pvi: 0, efficiency: 0 }];

    const totalH = layers.reduce((s, l) => s + Math.max(0, l.thickness), 0);
    if (totalH <= 0) return [{ pvi: 0, efficiency: 0 }];

    const clampedFrontPvi = Math.max(0.01, localFrontPvi);
    const nLayers = layers.length;
    const h = layers.map(l => Math.max(0, l.thickness));
    const k = layers.map(l => Math.max(1e-9, l.perm));

    // Front positions x[i] ∈ [0, 1] — fraction of layer length swept
    const x = new Float64Array(nLayers);

    // Time-step in PVI space with sub-stepping for accuracy
    const subStepsPerPoint = 20;
    const totalSteps = nPoints * subStepsPerPoint;
    const dPVI = pviMax / totalSteps;

    const result: SweepPoint[] = [{ pvi: 0, efficiency: 0 }];

    for (let step = 1; step <= totalSteps; step++) {
        // Compute effective mobility for each layer (normalised to λ_o = 1):
        //   Un-flooded layer at front position x_i:
        //     Series resistance → λ_eff = 1 / [x_i/M + (1 − x_i)]
        //   Flooded layer (x_i ≥ 1): all water → λ_eff = M
        let totalEffRate = 0;
        const effRates = new Float64Array(nLayers);

        for (let i = 0; i < nLayers; i++) {
            const lambdaEff = x[i] >= 1
                ? M
                : 1 / (x[i] / Math.max(1e-12, M) + (1 - x[i]));
            effRates[i] = h[i] * k[i] * lambdaEff;
            totalEffRate += effRates[i];
        }

        if (totalEffRate <= 0) break;

        // Advance each un-flooded layer's front:
        //   dx_i/dPVI = k_i × λ_eff_i × totalH / (Σ(h_j × k_j × λ_eff_j) × PVI_bt,local)
        //
        // Derivation: from q_i ∝ h_i × k_i × λ_eff_i and the relation
        //   dPVI = Σ q_j × dt / totalPV,  dx_i = q_i × dt / (h_i × PVI_bt,local × PV_layer_i/h_i)
        // the h_i cancels, leaving dx_i ∝ k_i × λ_eff_i.
        for (let i = 0; i < nLayers; i++) {
            if (x[i] >= 1) continue;
            const lambdaEff = 1 / (x[i] / Math.max(1e-12, M) + (1 - x[i]));
            const dx = k[i] * lambdaEff * totalH * dPVI / (totalEffRate * clampedFrontPvi);
            x[i] = Math.min(1, x[i] + dx);
        }

        // Record at output points
        if (step % subStepsPerPoint === 0) {
            let sweptH = 0;
            for (let i = 0; i < nLayers; i++) {
                sweptH += h[i] * Math.min(1, x[i]);
            }
            const pvi = (step / totalSteps) * pviMax;
            result.push({ pvi, efficiency: Math.max(0, Math.min(1, sweptH / totalH)) });
        }
    }

    return result;
}

/**
 * Generate a log-normal permeability distribution for N layers.
 *
 * @param nLayers — number of layers
 * @param vdp — target Dykstra-Parsons coefficient (0–1)
 * @param kMean — geometric mean permeability [mD]
 */
export function generateLayerPermDistribution(
    nLayers: number,
    vdp: number,
    kMean: number,
): number[] {
    if (nLayers <= 0) return [];
    if (nLayers === 1) return [kMean];

    // VDP relates to σ_ln:  VDP = 1 − exp(−σ_ln)   →   σ_ln = −ln(1 − VDP)
    const clampedVdp = Math.max(0, Math.min(0.999, vdp));
    const sigmaLn = clampedVdp > 0 ? -Math.log(1 - clampedVdp) : 0;
    const muLn = Math.log(Math.max(1e-6, kMean));

    // Use evenly-spaced quantiles of the log-normal distribution
    const perms: number[] = [];
    for (let i = 0; i < nLayers; i++) {
        // Percentile for layer i (exclude extremes 0 and 1)
        const p = (i + 0.5) / nLayers;
        // Inverse CDF of standard normal (rational approximation)
        const z = inverseCdfStdNormal(p);
        perms.push(Math.exp(muLn + sigmaLn * z));
    }
    return perms;
}

/**
 * Rational approximation of the inverse CDF (quantile function) of the
 * standard normal distribution.  Abramowitz & Stegun 26.2.23, |ε| < 4.5e-4.
 */
function inverseCdfStdNormal(p: number): number {
    if (p <= 0) return -Infinity;
    if (p >= 1) return Infinity;
    if (p === 0.5) return 0;

    const sign = p < 0.5 ? -1 : 1;
    const pp = p < 0.5 ? p : 1 - p;
    const t = Math.sqrt(-2 * Math.log(pp));

    const c0 = 2.515517;
    const c1 = 0.802853;
    const c2 = 0.010328;
    const d1 = 1.432788;
    const d2 = 0.189269;
    const d3 = 0.001308;

    return sign * (t - (c0 + c1 * t + c2 * t * t) / (1 + d1 * t + d2 * t * t + d3 * t * t * t));
}

/**
 * Compute vertical sweep result from a permeability array.
 */
export function computeVerticalSweep(
    permeabilities: number[],
    layerThickness: number,
    M: number,
    pviMax: number = 3.0,
    nPoints: number = 200,
    localFrontPvi: number = 1,
): VerticalSweepResult {
    const layers = permeabilities.map((perm) => ({ perm, thickness: layerThickness }));
    return {
        curve: verticalSweep(layers, M, pviMax, nPoints, localFrontPvi),
        vdp: dykstraParsonsCoefficient(permeabilities),
    };
}

// ────────────────────────────────────────────────────────────────────────────
// Simulation sweep efficiency (from per-cell saturation data)
// ────────────────────────────────────────────────────────────────────────────

/**
 * Compute a physically meaningful "swept" threshold from the Buckley-Leverett
 * shock-front saturation.  A cell is considered swept when its water saturation
 * reaches the midpoint between Swc and the BL shock front — i.e. the
 * displacement front has meaningfully passed through it.
 *
 * This avoids the pitfall of a tiny fixed epsilon (e.g. 0.01) that triggers
 * on numerical diffusion far ahead of the actual front.
 */
export function computeSweptThreshold(rock: RockProps, fluid: FluidProps, initialSw: number): number {
    const welge = computeWelgeMetrics(rock, fluid, initialSw);
    // Midpoint between initial Sw and BL shock-front Sw.
    // If Welge fails (shockSw ≈ initialSw), fall back to 20% of movable range.
    const movable = 1 - rock.s_or - rock.s_wc;
    if (welge.shockSw - initialSw < 0.01 * movable) {
        return initialSw + 0.2 * movable;
    }
    return (initialSw + welge.shockSw) / 2;
}

/**
 * Compute simulation sweep efficiencies from a per-cell saturation array.
 *
 * Cell layout: flat index = k * nx * ny + j * nx + i  (k=layer, j=col, i=row).
 * A cell is considered "swept" if its water saturation exceeds the given threshold.
 *
 *  E_vol — volumetric: fraction of all cells that are swept.
 *  E_A   — areal:      fraction of (i,j) columns that contain ≥1 swept cell.
 *  E_V   — vertical:   E_vol / E_A  (layers swept within the swept area).
 *
 * @param sweptThreshold — absolute Sw threshold for a cell to count as swept.
 *   Use {@link computeSweptThreshold} to derive from BL shock-front saturation.
 *
 * Returns {eA, eV, eVol} in [0, 1].
 */
export function computeSimSweepPoint(
    satWater: Float64Array | number[],
    nx: number,
    ny: number,
    nz: number,
    sweptThreshold: number,
): { eA: number; eV: number; eVol: number } {
    if (!satWater || satWater.length === 0 || nx <= 0 || ny <= 0 || nz <= 0) {
        return { eA: 0, eV: 0, eVol: 0 };
    }
    let sweptCells = 0;
    let sweptColumns = 0;

    for (let j = 0; j < ny; j++) {
        for (let i = 0; i < nx; i++) {
            let colSwept = false;
            for (let k = 0; k < nz; k++) {
                if (satWater[k * nx * ny + j * nx + i] > sweptThreshold) {
                    sweptCells++;
                    colSwept = true;
                }
            }
            if (colSwept) sweptColumns++;
        }
    }

    const total = nx * ny * nz;
    const eVol = sweptCells / total;
    const eA = sweptColumns / (nx * ny);
    const eV = eA > 1e-9 ? eVol / eA : 0;
    return { eA, eV, eVol };
}

// ────────────────────────────────────────────────────────────────────────────
// Combined volumetric sweep
// ────────────────────────────────────────────────────────────────────────────

/**
 * Compute combined volumetric sweep: E_vol(PVI) = E_A(PVI) × E_V(PVI).
 *
 * @param geometry — which sweep components to apply.  'areal' sets E_V = 1
 *   (single-layer or uniform XY five-spot);  'vertical' sets E_A = 1
 *   (XZ line drive);  'both' applies the full product.
 */
export function computeCombinedSweep(
    rock: RockProps,
    fluid: FluidProps,
    permeabilities: number[],
    layerThickness: number,
    pviMax: number = 3.0,
    nPoints: number = 200,
    geometry: SweepGeometry = 'both',
): CombinedSweepResult {
    const M = mobilityRatio(rock, fluid);
    const visibility = getSweepComponentVisibility(geometry);
    const localFrontPvi = computeLocalDisplacementFrontPvi(rock, fluid);

    const areal = computeArealSweep(rock, fluid, pviMax, nPoints);
    const vertical = computeVerticalSweep(permeabilities, layerThickness, M, pviMax, nPoints, localFrontPvi);
    const maskedArealCurve = areal.curve.map((point) => ({
        pvi: point.pvi,
        efficiency: visibility.showAreal ? point.efficiency : 1,
    }));
    const maskedVerticalCurve = vertical.curve.map((point) => ({
        pvi: point.pvi,
        efficiency: visibility.showVertical ? point.efficiency : 1,
    }));

    // Build pvi grid from the areal curve (always computed for metrics)
    const combined: SweepPoint[] = [];
    for (let i = 0; i < maskedArealCurve.length && i < maskedVerticalCurve.length; i++) {
        const ea = maskedArealCurve[i].efficiency;
        const ev = maskedVerticalCurve[i].efficiency;
        combined.push({
            pvi: maskedArealCurve[i].pvi,
            efficiency: ea * ev,
        });
    }

    return {
        arealSweep: { ...areal, curve: maskedArealCurve },
        verticalSweep: { ...vertical, curve: maskedVerticalCurve },
        combined,
    };
}

// ────────────────────────────────────────────────────────────────────────────
// Sweep recovery factor: RF = E_vol(PVI) × E_D_BL(PVI_local)
// ────────────────────────────────────────────────────────────────────────────

/**
 * Combine Craig/Dykstra-Parsons sweep efficiency with Buckley-Leverett
 * displacement efficiency to produce an analytical recovery factor curve.
 *
 *   RF_sweep(PVI) = E_vol(PVI) × E_D_BL(PVI_local)
 *
 * where:
 *   E_vol(PVI) = E_A_Craig(PVI) × E_V_DykstraParsons(PVI)  [volumetric sweep]
 *   PVI_local  = PVI / E_vol(PVI)                           [local PVI in swept zone]
 *   E_D_BL(x)  = RF_1D_BL(x)                               [1D Welge RF at PVI = x]
 *
 * --- APPROXIMATION & LIMITATIONS ---
 *
 * 1. Local-PVI approximation: assumes displacement quality within the swept zone
 *    is uniform and equal to what a 1D BL system receives at PVI_local. In
 *    reality, cells near the injector are over-displaced while frontier cells are
 *    under-displaced. This underestimates early RF and may slightly overestimate
 *    late RF. A rigorous treatment uses stream-tube integration (see TODO F11).
 *
 * 2. Craig (1971) five-spot correlation accuracy: ±10–15% based on original
 *    lab data scatter. Valid for M ∈ [0.1, 10]; confidence degrades outside.
 *    Only applies to confined five-spot geometry (not line drives or nine-spots).
 *
 * 3. Dykstra-Parsons non-communicating layers: assumes zero vertical cross-flow.
 *    Full cross-flow (good Kv/Kh) → E_V → 1 and the DP model overpredicts
 *    vertical heterogeneity impact. See Warren-Root or layered BL for cross-flow.
 *
 * 4. Independence of E_A and E_V: E_vol = E_A × E_V is an approximation.
 *    In reality, vertical heterogeneity reshapes areal flow paths.
 *
 * 5. Expansion terms ignored: Bo ≈ 1, incompressible fluids. Error < 3% for
 *    typical waterfloods; increases near the bubble point.
 *
 * 6. Constant injection rate assumed by Craig's correlation. BHP-controlled
 *    wells see variable rates; timing of E_A(PVI) may shift accordingly.
 *
 * Better models (see TODO F11): Stiles (1949) layer-by-layer BL integration;
 * stream-tube models; capacitance-resistance models fitted to production data.
 */
export function computeSweepRecoveryFactor(
    rock: RockProps,
    fluid: FluidProps,
    permeabilities: number[],
    layerThickness: number,
    pviMax: number = 3.0,
    nPoints: number = 200,
    geometry: SweepGeometry = 'both',
): SweepRFResult {
    // Build a dense 1D BL RF lookup table (extended range to handle PVI_local >> pviMax)
    const blLookupMax = pviMax * 5;
    const blLookupN = nPoints * 5;
    const blCurve = computeBLRecoveryVsPVI(rock, fluid, blLookupMax, blLookupN);

    function interpBL1D(pvi: number): number {
        if (pvi <= 0) return 0;
        if (pvi >= blCurve[blCurve.length - 1].pvi) return blCurve[blCurve.length - 1].rf;
        let lo = 0, hi = blCurve.length - 1;
        while (hi - lo > 1) {
            const mid = (lo + hi) >> 1;
            if (blCurve[mid].pvi <= pvi) lo = mid; else hi = mid;
        }
        const t = (pvi - blCurve[lo].pvi) / Math.max(1e-12, blCurve[hi].pvi - blCurve[lo].pvi);
        return blCurve[lo].rf + t * (blCurve[hi].rf - blCurve[lo].rf);
    }

    const edPiston = Math.max(0, (1 - rock.s_or - rock.s_wc) / Math.max(1e-12, 1 - rock.s_wc));
    const sweep = computeCombinedSweep(rock, fluid, permeabilities, layerThickness, pviMax, nPoints, geometry);

    const curve: SweepRFPoint[] = sweep.combined.map(({ pvi, efficiency: eVol }) => {
        const rfBL1D = interpBL1D(pvi);
        // PVI_local: effective PVI in the swept zone. Clamp at blLookupMax so
        // interpBL1D returns E_D_piston when E_vol is very small (swept zone ~empty).
        const pviLocal = eVol > 1e-3 ? Math.min(pvi / eVol, blLookupMax) : blLookupMax;
        const eD = interpBL1D(pviLocal);
        return { pvi, rfSweep: Math.min(eVol * eD, edPiston), rfBL1D, eVol, eD };
    });

    return {
        curve,
        edPiston,
        eAAtBreakthrough: sweep.arealSweep.eaAtBreakthrough,
        vdp: sweep.verticalSweep.vdp,
    };
}
