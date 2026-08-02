/**
 * reservoirVolumes.ts — geometry and hydrocarbon-in-place, defined once.
 *
 * These numbers were previously computed in three places — `benchmarkRunModel`,
 * `charts/analyticalParamAdapters` and the live `parameterStore` — with the same
 * defect in all three: "OOIP" was `V_p × (1 − S_w)`, a *reservoir* volume of
 * oil-plus-gas, used as the denominator of a *surface* cumulative-oil numerator.
 * It ignored initial gas saturation and never divided by B_oi, so recovery read
 * 10 % low on `dep_pvt`, 20 % on `gas_drive` and 44 % on SPE1, and `dep_gas_pz`
 * reported a recovery factor against 480,000 m³ of oil in a reservoir with no
 * oil in it at all.
 *
 * Two rules follow from that and are the reason this module exists:
 *
 * 1. **In place is a surface volume**, because production is. `STOIIP = V_p·S_o/B_oi`
 *    and `GIIP = V_p·S_g/B_gi + STOIIP·R_si` (free plus dissolved), both at
 *    standard conditions.
 * 2. **Oil and gas are different quantities.** A dry-gas case has no oil to
 *    recover and a black-oil case has both, so callers ask for the one they mean
 *    and get `null` where it does not exist — never a fraction of a denominator
 *    that isn't there.
 */

import { interpolatePvtAtPressure, DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR } from './physics/pvt';
import type { PvtRow } from './simulator-types';

/** Coerces `value` to a finite number, returning `fallback` for NaN/Infinity/null/undefined. */
export function toFiniteNumber(value: unknown, fallback: number): number {
    const numeric = Number(value);
    return Number.isFinite(numeric) ? numeric : fallback;
}

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

/**
 * Pore-volume-weighted initial saturations.
 *
 * Per-layer aware: the engine accepts `initialSaturationPerLayer` and
 * `initialGasSaturationPerLayer`, and reading only the scalars would report a
 * segregated gas cap as no gas cap at all — which is exactly the geometry the
 * planned `dep_gas_cap` case varies.
 */
export function getInitialSaturations(
    params: Record<string, any>,
): { water: number; gas: number; oil: number } {
    const thicknesses = getLayerThicknesses(params);
    const total = thicknesses.reduce((sum, t) => sum + t, 0);

    const weighted = (perLayer: unknown, scalar: number): number => {
        if (!Array.isArray(perLayer) || perLayer.length === 0 || total <= 0) return scalar;
        const sum = thicknesses.reduce(
            (acc, thickness, index) => acc + thickness * toFiniteNumber(perLayer[index], scalar),
            0,
        );
        return sum / total;
    };

    const water = Math.max(0, Math.min(1, weighted(
        params.initialSaturationPerLayer, toFiniteNumber(params.initialSaturation, 0.3),
    )));
    const gas = Math.max(0, Math.min(1, weighted(
        params.initialGasSaturationPerLayer, toFiniteNumber(params.initialGasSaturation, 0),
    )));
    return { water, gas, oil: Math.max(0, 1 - water - gas) };
}

/**
 * Formation volume factors and solution GOR at the initial pressure, read from
 * the run's own PVT table where it has one and from the constant-PVT scalars
 * otherwise. `B_g` is null when the case carries no gas PVT — there is then no
 * gas in place to express at surface conditions.
 */
export function getInitialPvt(
    params: Record<string, any>,
): { Boi: number; Bgi: number | null; Rsi: number } {
    const table = Array.isArray(params.pvtTable) ? (params.pvtTable as PvtRow[]) : null;
    if (String(params.pvtMode) === 'black-oil' && table && table.length > 0) {
        const row = interpolatePvtAtPressure(
            table,
            toFiniteNumber(params.initialPressure, 300),
            toFiniteNumber(params.c_o, DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR),
        );
        return {
            Boi: row.bo_m3m3 > 0 ? row.bo_m3m3 : 1,
            Bgi: row.bg_m3m3 > 0 ? row.bg_m3m3 : null,
            Rsi: Math.max(0, row.rs_m3m3),
        };
    }
    return {
        Boi: Math.max(1e-9, toFiniteNumber(params.volume_expansion_o, 1)),
        Bgi: null,
        Rsi: 0,
    };
}

/**
 * Stock-tank oil initially in place [Sm³], or null when the case has no oil.
 *
 * Null rather than zero is deliberate: a caller dividing by this must show
 * nothing, not a recovery factor of infinity or of a made-up denominator.
 */
export function getStockTankOilInPlace(params: Record<string, any>): number | null {
    const { oil } = getInitialSaturations(params);
    if (oil <= 1e-9) return null;
    const { Boi } = getInitialPvt(params);
    const value = getPoreVolume(params) * oil / Boi;
    return value > 1e-9 ? value : null;
}

/**
 * Gas initially in place at standard conditions [Sm³] — free gas plus gas
 * dissolved in the oil — or null when the case carries no gas PVT.
 */
export function getGasInPlace(params: Record<string, any>): number | null {
    const { gas, oil } = getInitialSaturations(params);
    const { Boi, Bgi, Rsi } = getInitialPvt(params);
    if (Bgi === null) return null;
    const poreVolume = getPoreVolume(params);
    const free = poreVolume * gas / Bgi;
    const dissolved = poreVolume * oil / Boi * Rsi;
    const value = free + dissolved;
    return value > 1e-9 ? value : null;
}

/**
 * Oil in place for an analytical *displacement* overlay, in reservoir volumes.
 *
 * Buckley-Leverett, the depletion model and the sweep correlations all produce a
 * displaced *reservoir* volume, so their recovery fraction takes a reservoir
 * denominator; `getStockTankOilInPlace` is the surface quantity and belongs with
 * produced volumes. The two coincide for every displacement scenario in the
 * catalog (B_o = 1, no initial gas) — this exists so that stays true by
 * construction rather than by luck.
 */
export function getDisplacementOilInPlace(params: Record<string, any>): number {
    return getPoreVolume(params) * getInitialSaturations(params).oil;
}
