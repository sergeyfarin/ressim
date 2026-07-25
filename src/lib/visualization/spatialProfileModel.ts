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

import type { GridState } from '../simulator-types';
import { buildLayerThicknesses } from './spatialViewModel';
import { computeWelgeMetrics, type FluidProps, type RockProps } from '../analytical/fractionalFlow';

/** Which grid direction the profile runs along. */
export type SpatialProfileAxis = 'i' | 'j' | 'k';

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

const AXIS_LABELS: Record<SpatialProfileAxis, string> = {
    i: 'Distance along I (m)',
    j: 'Distance along J (m)',
    k: 'Depth along K (m)',
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
    const count = axis === 'i' ? grid.nx : axis === 'j' ? grid.ny : grid.nz;
    return Math.max(0, Math.round(Number(count) || 0));
}

/**
 * Cell-centre distances along an axis. Uses per-layer thicknesses for K, so a
 * grid with non-uniform dz (SPE1) profiles against true depth rather than
 * layer number.
 */
export function axisDistances(grid: SpatialProfileGrid, axis: SpatialProfileAxis): number[] {
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
    property: SpatialProfileProperty;
}): SpatialProfileResult {
    const { grid, axis, property } = input;
    const nx = Math.max(0, Math.round(Number(grid.nx) || 0));
    const ny = Math.max(0, Math.round(Number(grid.ny) || 0));
    const nz = Math.max(0, Math.round(Number(grid.nz) || 0));
    const count = axisLength(grid, axis);
    const distances = axisDistances(grid, axis);

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
            const i = axis === 'i' ? step : fixedI;
            const j = axis === 'j' ? step : fixedJ;
            const k = axis === 'k' ? step : fixedK;
            const cell = k * nx * ny + j * nx + i;
            const raw = source?.[cell];
            values.push(Number.isFinite(raw) ? Number(raw) : null);
        }
        return { key: entry.seriesKey, label: entry.label, values };
    });

    return { ...base, series };
}

// ─── Buckley-Leverett flood-front overlay ────────────────────────────────────

export type FloodFrontOverlay = {
    /** Step profile: shock saturation behind the front, initial ahead of it. */
    values: Array<number | null>;
    /** Front position along the axis, metres. */
    frontDistance: number;
    shockSw: number;
    initialSw: number;
};

/**
 * Piston-like BL front for the water-saturation profile, at the snapshot time.
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
    injectionRate: number;
    simTime: number;
}): FloodFrontOverlay | null {
    if (input.axis !== 'i' || input.property !== 'saturation_water') return null;
    if (!(input.injectionRate > 0) || !(input.simTime > 0)) return null;

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
    const frontVelocity = (input.injectionRate / crossSection) * Math.max(0, dfwShock);
    const frontDistance = Math.max(0, Math.min(length, frontVelocity * input.simTime));

    const distances = axisDistances(input.grid, 'i');
    return {
        values: distances.map((x) => (x <= frontDistance ? shockSw : initialSw)),
        frontDistance,
        shockSw,
        initialSw,
    };
}
