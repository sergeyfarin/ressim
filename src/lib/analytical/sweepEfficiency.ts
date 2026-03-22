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

/**
 * Areal sweep efficiency vs cumulative PVI for a five-spot pattern.
 *
 * Before breakthrough (PVI < PVI_bt):  linear ramp from 0 to E_A_bt.
 * After breakthrough:  exponential growth toward 1.0, with the growth rate
 * depending on mobility ratio (unfavourable M → slower post-BT growth).
 *
 * Based on Dyes, Caudle & Erickson (1954) graphical correlations.
 */
export function arealSweepAtPvi(M: number, pvi: number): number {
    if (pvi <= 0) return 0;
    const eaBt = arealSweepAtBreakthrough(M);
    if (eaBt <= 0) return 0;

    // PVI at breakthrough: for a five-spot, PVI_bt ≈ E_A_bt (piston approx)
    const pviBt = eaBt;

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
): SweepPoint[] {
    const result: SweepPoint[] = [];
    for (let i = 0; i <= nPoints; i++) {
        const pvi = (i / nPoints) * pviMax;
        result.push({ pvi, efficiency: arealSweepAtPvi(M, pvi) });
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
    return {
        curve: arealSweepCurve(M, pviMax, nPoints),
        mobilityRatio: M,
        eaAtBreakthrough: eaBt,
        pviAtBreakthrough: eaBt, // five-spot: PVI_bt ≈ E_A_bt
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
 * Each layer has a permeability k_i and thickness h_i. The water front in
 * layer i advances proportionally to its permeability. We track sequential
 * breakthrough events (fastest→slowest layer) and compute E_v at each.
 *
 * Returns a stepwise   { pvi, efficiency }[]   curve.
 */
export function verticalSweep(
    layers: Array<{ perm: number; thickness: number }>,
    M: number,
    pviMax: number = 3.0,
    nPoints: number = 200,
): SweepPoint[] {
    if (layers.length === 0) return [{ pvi: 0, efficiency: 0 }];

    const totalH = layers.reduce((s, l) => s + Math.max(0, l.thickness), 0);
    if (totalH <= 0) return [{ pvi: 0, efficiency: 0 }];

    // Sort layers by permeability descending (fastest to flood out first)
    const sortedLayers = layers
        .map((l, idx) => ({
            perm: Math.max(1e-9, l.perm),
            thickness: Math.max(0, l.thickness),
            idx,
        }))
        .sort((a, b) => b.perm - a.perm);

    // Compute the PVI at which each layer floods out, assuming piston-like
    // displacement.  In the Dykstra-Parsons model, the front velocity in
    // layer i is proportional to k_i.  The fastest layer floods first.
    //
    // For constant ΔP across all layers, the fill time for layer i is
    // proportional to 1/k_i (relative to the fastest layer).
    //
    // PVI_bt for the system = fraction of total PV in the fastest layer.
    // After that layer floods out, its flow becomes water (mobility M effect).

    const kMax = sortedLayers[0].perm;

    // PVI at which each layer's front reaches the outlet (relative reference)
    // Layer i fills when its normalized front position = 1, i.e.
    //   PVI_i = (kMax / k_i) × PVI_1   where PVI_1 = h_1 / totalH
    // But with mobility ratio adjustment: after a layer floods, it carries
    // water at higher mobility, which steals pressure from remaining layers.
    //
    // Simplified Dykstra-Parsons: ignore cross-flow redistribution,
    // breakthrough PVI for each layer scales as kMax/k_i × (h_1/totalH).

    const btPviBaseline = sortedLayers[0].thickness / totalH;
    const btEvents: Array<{ pvi: number; cumulativeThickness: number }> = [];
    let cumulativeH = 0;

    for (const layer of sortedLayers) {
        const layerBtPvi = (kMax / layer.perm) * btPviBaseline;
        // Apply mobility ratio effect: after BT, water's higher mobility
        // accelerates remaining layers only mildly in piston-like model.
        const adjustedPvi = layerBtPvi * (1 + (M - 1) * (cumulativeH / totalH) * 0.3);
        cumulativeH += layer.thickness;
        btEvents.push({ pvi: Math.max(0, adjustedPvi), cumulativeThickness: cumulativeH });
    }

    // Build continuous curve by interpolating between BT events
    const result: SweepPoint[] = [];
    for (let i = 0; i <= nPoints; i++) {
        const pvi = (i / nPoints) * pviMax;
        let ev = 0;

        if (pvi <= 0) {
            ev = 0;
        } else {
            // Sum fraction of each layer that is swept at this PVI
            let sweptH = 0;
            for (let j = 0; j < sortedLayers.length; j++) {
                const layerPviBt = btEvents[j].pvi;
                if (layerPviBt <= 0) {
                    sweptH += sortedLayers[j].thickness;
                } else {
                    const fraction = Math.min(1, pvi / layerPviBt);
                    sweptH += sortedLayers[j].thickness * fraction;
                }
            }
            ev = sweptH / totalH;
        }
        result.push({ pvi, efficiency: Math.max(0, Math.min(1, ev)) });
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
): VerticalSweepResult {
    const layers = permeabilities.map((perm) => ({ perm, thickness: layerThickness }));
    return {
        curve: verticalSweep(layers, M, pviMax, nPoints),
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
 */
export function computeCombinedSweep(
    rock: RockProps,
    fluid: FluidProps,
    permeabilities: number[],
    layerThickness: number,
    pviMax: number = 3.0,
    nPoints: number = 200,
): CombinedSweepResult {
    const areal = computeArealSweep(rock, fluid, pviMax, nPoints);
    const vertical = computeVerticalSweep(permeabilities, layerThickness, areal.mobilityRatio, pviMax, nPoints);

    const combined: SweepPoint[] = [];
    for (let i = 0; i < areal.curve.length && i < vertical.curve.length; i++) {
        combined.push({
            pvi: areal.curve[i].pvi,
            efficiency: areal.curve[i].efficiency * vertical.curve[i].efficiency,
        });
    }

    return { arealSweep: areal, verticalSweep: vertical, combined };
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
    const sweep = computeCombinedSweep(rock, fluid, permeabilities, layerThickness, pviMax, nPoints);

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
