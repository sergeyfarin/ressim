/**
 * spatialProfileModel.ts — extracts a 1D profile of a grid property along one
 * axis at a single timestep.
 *
 * This is the spatial counterpart to the rate charts: those show one number per
 * report step across the whole run, this shows one number per cell along a line
 * at one instant. That difference is why it belongs to the 3D view group rather
 * than the run-results charts — it is a cross-section of the same snapshot the
 * 3D view is rendering, and it follows the same timestep and property selectors.
 *
 * Pure: no Chart.js, no Svelte, no DOM. `SpatialProfileChart.svelte` renders it.
 */

import type { GridState, RateHistoryPoint } from '../simulator-types';
import { buildLayerThicknesses } from './spatialViewModel';
import {
    computeWelgeMetrics,
    dfw_dSw,
    type FluidProps,
    type RockProps,
} from '../analytical/fractionalFlow';
import {
    arealSweepAtBreakthrough,
    arealSweepAtPvi,
    mobilityRatio,
    type SweepGeometry,
} from '../analytical/sweepEfficiency';

/** Which grid direction the profile runs along. */
export type SpatialProfileAxis = 'i' | 'j' | 'k' | 'well-path';
export type SpatialProfileLayerSelection = number | 'average';
export type SpatialProfileReference =
    | { kind: 'buckley-leverett' }
    | {
        kind: 'sweep';
        geometry: Extract<SweepGeometry, 'areal' | 'both'>;
        layerPermeabilities: number[];
    };

/**
 * The property to profile. Deliberately the same union as the 3D view's
 * `showProperty` so the two share one selector.
 */
export type SpatialProfileProperty =
    | 'pressure'
    | 'saturation_water'
    | 'saturation_oil'
    | 'saturation_gas'
    | 'saturation_ternary';

export type SpatialProfileSeries = {
    key: string;
    label: string;
    values: Array<number | null>;
};

export type SpatialProfileResult = {
    /** Cell-centre distance along the profile axis, in metres. */
    distances: number[];
    series: SpatialProfileSeries[];
    axisLabel: string;
    valueLabel: string;
    /** Fixed y-range for saturations; null lets the chart autoscale pressure. */
    valueRange: [number, number] | null;
};

export type SpatialProfileGrid = {
    nx: number;
    ny: number;
    nz: number;
    cellDx: number;
    cellDy: number;
    cellDz: number;
    cellDzPerLayer?: number[];
};

/** Pick the analytical displacement path appropriate to the active scenario. */
export function defaultSpatialProfileAxis(
    grid: SpatialProfileGrid,
    reference?: SpatialProfileReference | null,
): SpatialProfileAxis {
    if (reference?.kind === 'sweep' && grid.nx > 1 && grid.ny > 1) return 'well-path';
    return grid.nx > 1 ? 'i' : grid.ny > 1 ? 'j' : 'k';
}

const AXIS_LABELS: Record<SpatialProfileAxis, string> = {
    i: 'Distance along I (m)',
    j: 'Distance along J (m)',
    k: 'Depth along K (m)',
    'well-path': 'Distance from injector to producer (m)',
};

const SATURATION_SERIES: Record<
    Exclude<SpatialProfileProperty, 'pressure' | 'saturation_ternary'>,
    { key: keyof GridState; label: string }
> = {
    saturation_water: { key: 'sat_water', label: 'Water Saturation Sw' },
    saturation_oil: { key: 'sat_oil', label: 'Oil Saturation So' },
    saturation_gas: { key: 'sat_gas', label: 'Gas Saturation Sg' },
};

function clampIndex(value: number, count: number): number {
    if (!Number.isFinite(value)) return 0;
    return Math.max(0, Math.min(Math.max(0, count - 1), Math.round(value)));
}

/** Number of cells along an axis. */
export function axisLength(grid: SpatialProfileGrid, axis: SpatialProfileAxis): number {
    if (axis === 'well-path') return 0;
    const count = axis === 'i' ? grid.nx : axis === 'j' ? grid.ny : grid.nz;
    return Math.max(0, Math.round(Number(count) || 0));
}

/**
 * Cell-centre distances along an axis. Uses per-layer thicknesses for K, so a
 * grid with non-uniform dz (SPE1) profiles against true depth rather than
 * layer number.
 */
export function axisDistances(grid: SpatialProfileGrid, axis: SpatialProfileAxis): number[] {
    if (axis === 'well-path') return [];
    const count = axisLength(grid, axis);
    if (axis === 'k') {
        const thicknesses = buildLayerThicknesses({
            nz: count,
            cellDz: grid.cellDz,
            cellDzPerLayer: grid.cellDzPerLayer,
        });
        const centres: number[] = [];
        let top = 0;
        for (const thickness of thicknesses) {
            centres.push(top + thickness / 2);
            top += thickness;
        }
        return centres;
    }
    const size = Math.max(1e-9, Number(axis === 'i' ? grid.cellDx : grid.cellDy) || 1);
    return Array.from({ length: count }, (_, index) => (index + 0.5) * size);
}

/**
 * Walk one line of cells and read a property off each.
 *
 * `fixed` gives the two indices held constant; the third is swept. Out-of-range
 * fixed indices are clamped rather than rejected, so a profile survives a
 * scenario switch that shrinks the grid.
 */
export function buildSpatialProfile(input: {
    gridState: GridState | null | undefined;
    grid: SpatialProfileGrid;
    axis: SpatialProfileAxis;
    fixedI: number;
    fixedJ: number;
    fixedK: number;
    layerSelection?: SpatialProfileLayerSelection;
    injectorI?: number;
    injectorJ?: number;
    producerI?: number;
    producerJ?: number;
    property: SpatialProfileProperty;
}): SpatialProfileResult {
    const { grid, axis, property } = input;
    const nx = Math.max(0, Math.round(Number(grid.nx) || 0));
    const ny = Math.max(0, Math.round(Number(grid.ny) || 0));
    const nz = Math.max(0, Math.round(Number(grid.nz) || 0));
    const path = axis === 'well-path'
        ? buildWellPath(grid, input.injectorI ?? 0, input.injectorJ ?? 0,
            input.producerI ?? nx - 1, input.producerJ ?? ny - 1)
        : [];
    const count = axis === 'well-path' ? path.length : axisLength(grid, axis);
    const distances = axis === 'well-path'
        ? path.map((cell, index) => index === 0 ? 0 : Math.hypot(
            (cell.i - path[0].i) * grid.cellDx,
            (cell.j - path[0].j) * grid.cellDy,
        ))
        : axisDistances(grid, axis);

    const isPressure = property === 'pressure';
    const isTernary = property === 'saturation_ternary';
    const wanted: Array<{ key: keyof GridState; label: string; seriesKey: string }> = isPressure
        ? [{ key: 'pressure', label: 'Pressure', seriesKey: 'pressure' }]
        : isTernary
            // The 3D view blends all three saturations into one colour; a 1D
            // profile can just draw them, which is strictly more readable.
            ? (['saturation_water', 'saturation_oil', 'saturation_gas'] as const).map((name) => ({
                key: SATURATION_SERIES[name].key,
                label: SATURATION_SERIES[name].label,
                seriesKey: name,
            }))
            : [{
                key: SATURATION_SERIES[property].key,
                label: SATURATION_SERIES[property].label,
                seriesKey: property,
            }];

    const emptySeries = wanted.map((entry) => ({
        key: entry.seriesKey,
        label: entry.label,
        values: Array.from({ length: count }, () => null as number | null),
    }));

    const base = {
        distances,
        axisLabel: AXIS_LABELS[axis],
        valueLabel: isPressure ? 'Pressure (bar)' : isTernary ? 'Saturation' : wanted[0].label,
        valueRange: (isPressure ? null : [0, 1]) as [number, number] | null,
    };

    if (!input.gridState || count === 0 || nx === 0 || ny === 0 || nz === 0) {
        return { ...base, series: emptySeries };
    }

    const fixedI = clampIndex(input.fixedI, nx);
    const fixedJ = clampIndex(input.fixedJ, ny);
    const fixedK = clampIndex(input.fixedK, nz);

    const series = wanted.map((entry) => {
        const source = input.gridState?.[entry.key] as Float64Array | undefined;
        const values: Array<number | null> = [];
        for (let step = 0; step < count; step += 1) {
            const i = axis === 'well-path' ? path[step].i : axis === 'i' ? step : fixedI;
            const j = axis === 'well-path' ? path[step].j : axis === 'j' ? step : fixedJ;
            const averageLayers = input.layerSelection === 'average' && axis !== 'k';
            const layers = averageLayers ? Array.from({ length: nz }, (_, k) => k) : [
                axis === 'k' ? step : clampIndex(
                    typeof input.layerSelection === 'number' ? input.layerSelection : fixedK,
                    nz,
                ),
            ];
            const samples = layers
                .map((k) => source?.[k * nx * ny + j * nx + i])
                .filter((raw): raw is number => Number.isFinite(raw));
            values.push(samples.length > 0
                ? samples.reduce((sum, raw) => sum + Number(raw), 0) / samples.length
                : null);
        }
        return { key: entry.seriesKey, label: entry.label, values };
    });

    return { ...base, series };
}

/** Rasterized cell path joining two wells in the XY plane. */
export function buildWellPath(
    grid: SpatialProfileGrid,
    injectorI: number,
    injectorJ: number,
    producerI: number,
    producerJ: number,
): Array<{ i: number; j: number }> {
    let i = clampIndex(injectorI, grid.nx);
    let j = clampIndex(injectorJ, grid.ny);
    const endI = clampIndex(producerI, grid.nx);
    const endJ = clampIndex(producerJ, grid.ny);
    const di = Math.abs(endI - i);
    const dj = Math.abs(endJ - j);
    const stepI = i < endI ? 1 : -1;
    const stepJ = j < endJ ? 1 : -1;
    let error = di - dj;
    const cells: Array<{ i: number; j: number }> = [];
    while (true) {
        cells.push({ i, j });
        if (i === endI && j === endJ) break;
        const twiceError = 2 * error;
        if (twiceError > -dj) { error -= dj; i += stepI; }
        if (twiceError < di) { error += di; j += stepJ; }
    }
    return cells;
}

// ─── Buckley-Leverett flood-front overlay ────────────────────────────────────

export type FloodFrontOverlay = {
    /** BL rarefaction behind the shock and initial saturation ahead of it. */
    values: Array<number | null>;
    /** Front position along the axis, metres. */
    frontDistance: number;
    shockSw: number;
    initialSw: number;
    label: string;
};

/** Injected reservoir volume through a selected replay time, using report-step rates. */
export function cumulativeInjectedVolume(
    rateHistory: RateHistoryPoint[],
    simTime: number,
): number {
    if (!(simTime > 0)) return 0;
    let volume = 0;
    let previousTime = 0;
    for (const point of rateHistory) {
        const pointTime = Math.max(previousTime, Math.min(simTime, Number(point.time) || 0));
        const rate = Math.max(0, Number(point.total_injection) || 0);
        volume += rate * Math.max(0, pointTime - previousTime);
        previousTime = pointTime;
        if (previousTime >= simTime) break;
    }
    return volume;
}

/**
 * Buckley–Leverett water-saturation profile at the snapshot time.
 *
 * Only meaningful along I in a scenario being flooded — it is the same Welge
 * construction the rate charts use for breakthrough, evaluated in space instead
 * of time. Returns null whenever that does not hold, which is what hides the
 * overlay rather than drawing a misleading flat line.
 *
 * The fractional-flow math is imported, not reimplemented: the old
 * `SwProfileChart` carried its own copies of k_rw, k_ro, fractionalFlow and the
 * Welge tangent search, which could drift from `analytical/fractionalFlow.ts`.
 */
export function buildFloodFrontOverlay(input: {
    grid: SpatialProfileGrid;
    axis: SpatialProfileAxis;
    property: SpatialProfileProperty;
    rock: RockProps;
    fluid: FluidProps;
    initialSaturation: number;
    porosity: number;
    injectedVolume: number;
}): FloodFrontOverlay | null {
    if (input.axis !== 'i' || input.property !== 'saturation_water') return null;
    if (!(input.injectedVolume > 0) || !(input.porosity > 0)) return null;

    const nx = Math.max(0, Math.round(Number(input.grid.nx) || 0));
    if (nx === 0) return null;

    const { shockSw, initialSw, breakthroughPvi } = computeWelgeMetrics(
        input.rock,
        input.fluid,
        input.initialSaturation,
    );
    // breakthroughPvi = 1 / (dfw/dSw at the shock); recover the slope.
    if (!(breakthroughPvi > 1e-12)) return null;
    const dfwShock = 1 / breakthroughPvi;

    const thicknesses = buildLayerThicknesses({
        nz: input.grid.nz,
        cellDz: input.grid.cellDz,
        cellDzPerLayer: input.grid.cellDzPerLayer,
    });
    const totalThickness = thicknesses.reduce((sum, t) => sum + t, 0);
    const crossSection = Math.max(
        1e-9,
        Math.max(0, Number(input.grid.ny) || 0) * (Number(input.grid.cellDy) || 0) * totalThickness,
    );

    const length = nx * Math.max(1e-9, Number(input.grid.cellDx) || 1);
    const poreVolumesInjected = input.injectedVolume
        / Math.max(1e-12, input.porosity * crossSection * length);
    const unclampedFrontDistance = length * poreVolumesInjected * Math.max(0, dfwShock);
    const frontDistance = Math.max(0, Math.min(length, unclampedFrontDistance));

    const maxSw = 1 - input.rock.s_or;
    const slopeAtMaxSw = dfw_dSw(maxSw, input.rock, input.fluid, 1e-5);
    function saturationBehindShock(targetSlope: number): number {
        if (targetSlope >= dfwShock) return shockSw;
        if (targetSlope <= slopeAtMaxSw) return maxSw;
        let lo = shockSw;
        let hi = maxSw;
        for (let iteration = 0; iteration < 60; iteration += 1) {
            const mid = 0.5 * (lo + hi);
            if (dfw_dSw(mid, input.rock, input.fluid, 1e-5) > targetSlope) lo = mid;
            else hi = mid;
        }
        return 0.5 * (lo + hi);
    }

    const distances = axisDistances(input.grid, 'i');
    return {
        values: distances.map((x) => {
            if (x > unclampedFrontDistance) return initialSw;
            const dimensionlessDistance = x / length;
            return saturationBehindShock(dimensionlessDistance / poreVolumesInjected);
        }),
        frontDistance,
        shockSw,
        initialSw,
        label: 'Buckley–Leverett reference',
    };
}

/**
 * Craig-contacted five-spot diagonal with Buckley–Leverett displacement inside it.
 *
 * Craig predicts contacted area, not a unique saturation contour. We map its
 * pre-breakthrough E_A/E_A(BT) progression onto the injector→producer diagonal,
 * then evaluate the BL rarefaction in that contacted distance. For a combined
 * layered flood, Stiles' permeability-weighted layer allocation supplies each
 * layer's local PVI before selected-layer/column-average presentation.
 */
export function buildSweepDiagonalOverlay(input: {
    grid: SpatialProfileGrid;
    axis: SpatialProfileAxis;
    property: SpatialProfileProperty;
    layerSelection: SpatialProfileLayerSelection;
    geometry: Extract<SweepGeometry, 'areal' | 'both'>;
    rock: RockProps;
    fluid: FluidProps;
    initialSaturation: number;
    porosity: number;
    injectedVolume: number;
    layerPermeabilities: number[];
    injectorI: number;
    injectorJ: number;
    producerI: number;
    producerJ: number;
}): FloodFrontOverlay | null {
    if (input.axis !== 'well-path' || input.property !== 'saturation_water') return null;
    if (!(input.injectedVolume > 0) || !(input.porosity > 0)) return null;

    const thicknesses = buildLayerThicknesses({
        nz: input.grid.nz,
        cellDz: input.grid.cellDz,
        cellDzPerLayer: input.grid.cellDzPerLayer,
    });
    const bulkVolume = Math.max(1e-12,
        input.grid.nx * input.grid.cellDx
        * input.grid.ny * input.grid.cellDy
        * thicknesses.reduce((sum, value) => sum + value, 0));
    const globalPvi = input.injectedVolume / Math.max(1e-12, input.porosity * bulkVolume);
    if (!(globalPvi > 0)) return null;

    const metrics = computeWelgeMetrics(input.rock, input.fluid, input.initialSaturation);
    if (!(metrics.breakthroughPvi > 1e-12)) return null;
    const eaBt = arealSweepAtBreakthrough(mobilityRatio(input.rock, input.fluid));
    const eA = arealSweepAtPvi(
        mobilityRatio(input.rock, input.fluid),
        globalPvi,
        metrics.breakthroughPvi,
    );
    if (!(eA > 1e-12) || !(eaBt > 1e-12)) return null;

    const contactedFraction = Math.min(1, eA / eaBt);
    const path = buildWellPath(
        input.grid,
        input.injectorI,
        input.injectorJ,
        input.producerI,
        input.producerJ,
    );
    const distances = path.map((cell, index) => index === 0 ? 0 : Math.hypot(
        (cell.i - path[0].i) * input.grid.cellDx,
        (cell.j - path[0].j) * input.grid.cellDy,
    ));
    const pathLength = Math.max(1e-12, distances.at(-1) ?? 0);
    const zonePvi = globalPvi / eA;

    const nz = Math.max(1, Math.round(input.grid.nz));
    const rawPerms = Array.from({ length: nz }, (_, k) =>
        Math.max(0, Number(input.layerPermeabilities[k] ?? input.layerPermeabilities[0] ?? 1) || 0));
    const permTotal = rawPerms.reduce((sum, value) => sum + value, 0);
    const flowFractions = permTotal > 1e-12
        ? rawPerms.map((value) => value / permTotal)
        : rawPerms.map(() => 1 / nz);

    const selectedLayers = input.layerSelection === 'average'
        ? Array.from({ length: nz }, (_, k) => k)
        : [Math.max(0, Math.min(nz - 1, Math.round(input.layerSelection)))];
    const maxSw = 1 - input.rock.s_or;
    const dfwShock = 1 / metrics.breakthroughPvi;
    const slopeAtMaxSw = dfw_dSw(maxSw, input.rock, input.fluid, 1e-5);
    function saturationAtSlope(targetSlope: number): number {
        if (targetSlope >= dfwShock) return metrics.shockSw;
        if (targetSlope <= slopeAtMaxSw) return maxSw;
        let lo = metrics.shockSw;
        let hi = maxSw;
        for (let iteration = 0; iteration < 60; iteration += 1) {
            const mid = 0.5 * (lo + hi);
            if (dfw_dSw(mid, input.rock, input.fluid, 1e-5) > targetSlope) lo = mid;
            else hi = mid;
        }
        return 0.5 * (lo + hi);
    }

    const layerValues = selectedLayers.map((layer) => {
        const localPvi = zonePvi * nz * flowFractions[layer];
        return distances.map((distance) => {
            const contactedCoordinate = (distance / pathLength) / contactedFraction;
            if (contactedCoordinate > localPvi * dfwShock) return metrics.initialSw;
            return saturationAtSlope(contactedCoordinate / Math.max(1e-12, localPvi));
        });
    });
    const values = distances.map((_, index) =>
        layerValues.reduce((sum, layer) => sum + layer[index], 0) / layerValues.length);
    const leadingLocalPvi = Math.max(...selectedLayers.map((layer) => zonePvi * nz * flowFractions[layer]));

    return {
        values,
        frontDistance: pathLength * contactedFraction * Math.min(1, leadingLocalPvi * dfwShock),
        shockSw: metrics.shockSw,
        initialSw: metrics.initialSw,
        label: input.geometry === 'both'
            ? 'Craig + Stiles + BL reference'
            : 'Craig + BL reference',
    };
}
