import { describe, expect, it } from 'vitest';
import {
    axisDistances,
    axisLength,
    buildFloodFrontOverlay,
    buildSpatialProfile,
    type SpatialProfileGrid,
} from './spatialProfileModel';
import type { GridState } from '../simulator-types';

const GRID: SpatialProfileGrid = { nx: 4, ny: 3, nz: 2, cellDx: 10, cellDy: 20, cellDz: 5 };

/** Cell value encodes its own (i, j, k) so a mis-indexed walk is obvious. */
function makeGrid(grid: SpatialProfileGrid): GridState {
    const total = grid.nx * grid.ny * grid.nz;
    const encode = (fn: (i: number, j: number, k: number) => number) => {
        const out = new Float64Array(total);
        for (let k = 0; k < grid.nz; k += 1) {
            for (let j = 0; j < grid.ny; j += 1) {
                for (let i = 0; i < grid.nx; i += 1) {
                    out[k * grid.nx * grid.ny + j * grid.nx + i] = fn(i, j, k);
                }
            }
        }
        return out;
    };
    return {
        pressure: encode((i, j, k) => 100 * k + 10 * j + i),
        sat_water: encode((i) => i / 10),
        sat_oil: encode((_i, j) => j / 10),
        sat_gas: encode((_i, _j, k) => k / 10),
    };
}

const ROCK = { s_wc: 0.2, s_or: 0.2, n_w: 2, n_o: 2, k_rw_max: 1, k_ro_max: 1 };
const FLUID = { mu_w: 1, mu_o: 1 };

describe('axis geometry', () => {
    it('reports the cell count along each axis', () => {
        expect(axisLength(GRID, 'i')).toBe(4);
        expect(axisLength(GRID, 'j')).toBe(3);
        expect(axisLength(GRID, 'k')).toBe(2);
    });

    it('places samples at cell centres in metres, not cell indices', () => {
        expect(axisDistances(GRID, 'i')).toEqual([5, 15, 25, 35]);
        expect(axisDistances(GRID, 'j')).toEqual([10, 30, 50]);
    });

    it('uses per-layer thickness for depth so non-uniform dz profiles against true depth', () => {
        // SPE1-style: 20 / 30 / 50 ft-ish layers rather than a uniform cellDz.
        const layered = { ...GRID, nz: 3, cellDzPerLayer: [20, 30, 50] };
        expect(axisDistances(layered, 'k')).toEqual([10, 35, 75]);
    });
});

describe('buildSpatialProfile', () => {
    const gridState = makeGrid(GRID);

    it('walks the I line at the fixed J and K', () => {
        const profile = buildSpatialProfile({
            gridState, grid: GRID, axis: 'i', fixedI: 0, fixedJ: 2, fixedK: 1,
            property: 'pressure',
        });
        expect(profile.series).toHaveLength(1);
        expect(profile.series[0].values).toEqual([120, 121, 122, 123]);
        expect(profile.distances).toEqual([5, 15, 25, 35]);
    });

    it('walks the J and K lines from the same snapshot', () => {
        const alongJ = buildSpatialProfile({
            gridState, grid: GRID, axis: 'j', fixedI: 3, fixedJ: 0, fixedK: 0,
            property: 'pressure',
        });
        expect(alongJ.series[0].values).toEqual([3, 13, 23]);

        const alongK = buildSpatialProfile({
            gridState, grid: GRID, axis: 'k', fixedI: 1, fixedJ: 1, fixedK: 0,
            property: 'pressure',
        });
        expect(alongK.series[0].values).toEqual([11, 111]);
    });

    it('clamps out-of-range fixed indices instead of returning holes', () => {
        // Survives a scenario switch that shrinks the grid under a stale selector.
        const profile = buildSpatialProfile({
            gridState, grid: GRID, axis: 'i', fixedI: 0, fixedJ: 99, fixedK: -4,
            property: 'pressure',
        });
        expect(profile.series[0].values).toEqual([20, 21, 22, 23]);
    });

    it('expands the ternary property into all three saturations', () => {
        // The 3D view blends them into one colour; a 1D profile can just draw them.
        const profile = buildSpatialProfile({
            gridState, grid: GRID, axis: 'i', fixedI: 0, fixedJ: 1, fixedK: 1,
            property: 'saturation_ternary',
        });
        expect(profile.series.map((s) => s.key))
            .toEqual(['saturation_water', 'saturation_oil', 'saturation_gas']);
        expect(profile.series[0].values).toEqual([0, 0.1, 0.2, 0.3]);
        expect(profile.series[1].values).toEqual([0.1, 0.1, 0.1, 0.1]);
        expect(profile.series[2].values).toEqual([0.1, 0.1, 0.1, 0.1]);
    });

    it('fixes saturations to [0,1] and lets pressure autoscale', () => {
        const sat = buildSpatialProfile({
            gridState, grid: GRID, axis: 'i', fixedI: 0, fixedJ: 0, fixedK: 0,
            property: 'saturation_water',
        });
        expect(sat.valueRange).toEqual([0, 1]);
        const pressure = buildSpatialProfile({
            gridState, grid: GRID, axis: 'i', fixedI: 0, fixedJ: 0, fixedK: 0,
            property: 'pressure',
        });
        expect(pressure.valueRange).toBeNull();
        expect(pressure.valueLabel).toBe('Pressure (bar)');
    });

    it('returns a null-filled profile of the right length with no snapshot', () => {
        const profile = buildSpatialProfile({
            gridState: null, grid: GRID, axis: 'i', fixedI: 0, fixedJ: 0, fixedK: 0,
            property: 'saturation_water',
        });
        expect(profile.series[0].values).toEqual([null, null, null, null]);
        expect(profile.distances).toEqual([5, 15, 25, 35]);
    });
});

describe('buildFloodFrontOverlay', () => {
    const base = {
        grid: GRID, axis: 'i' as const, property: 'saturation_water' as const,
        rock: ROCK, fluid: FLUID, initialSaturation: 0.2,
        injectionRate: 100, simTime: 10,
    };

    it('draws a step from shock saturation behind the front to initial ahead of it', () => {
        const overlay = buildFloodFrontOverlay(base)!;
        expect(overlay).not.toBeNull();
        expect(overlay.shockSw).toBeGreaterThan(overlay.initialSw);
        const behind = overlay.values.filter((v) => v === overlay.shockSw).length;
        const ahead = overlay.values.filter((v) => v === overlay.initialSw).length;
        expect(behind + ahead).toBe(GRID.nx);
    });

    it('advances the front monotonically with time and stops at the outlet', () => {
        const early = buildFloodFrontOverlay({ ...base, simTime: 1 })!;
        const later = buildFloodFrontOverlay({ ...base, simTime: 5 })!;
        const huge = buildFloodFrontOverlay({ ...base, simTime: 1e6 })!;
        expect(later.frontDistance).toBeGreaterThan(early.frontDistance);
        expect(huge.frontDistance).toBeCloseTo(GRID.nx * GRID.cellDx, 6);
    });

    it('is hidden wherever a flood front is not the right reference', () => {
        // Not a displacement axis, not the water saturation, not injecting, not started.
        expect(buildFloodFrontOverlay({ ...base, axis: 'k' })).toBeNull();
        expect(buildFloodFrontOverlay({ ...base, property: 'pressure' })).toBeNull();
        expect(buildFloodFrontOverlay({ ...base, injectionRate: 0 })).toBeNull();
        expect(buildFloodFrontOverlay({ ...base, simTime: 0 })).toBeNull();
    });

    it('scales the front position by the true cross-section, including per-layer dz', () => {
        // Same rate through a thicker column ⇒ slower front.
        const thin = buildFloodFrontOverlay(base)!;
        const thick = buildFloodFrontOverlay({
            ...base,
            grid: { ...GRID, cellDzPerLayer: [50, 50] },
        })!;
        expect(thick.frontDistance).toBeLessThan(thin.frontDistance);
    });
});
