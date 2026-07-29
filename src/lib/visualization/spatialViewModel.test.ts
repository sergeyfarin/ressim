import { describe, expect, it } from 'vitest';
import {
    buildLayerThicknesses,
    clampHistoryIndex,
    getExpectedCellCount,
    getPressureMaxFromHistorySlice,
    selectActiveGrid,
    selectActiveWellState,
    resolvePressureDisplayRange,
} from './spatialViewModel';
import type { GridState, SimulatorSnapshot } from '../simulator-types';

function grid(values: number[]): GridState {
    return {
        pressure: new Float64Array(values),
        sat_water: new Float64Array(values.length),
        sat_oil: new Float64Array(values.length),
        sat_gas: new Float64Array(values.length),
    };
}

describe('spatialViewModel', () => {
    it('selects a compatible history grid before static grid state', () => {
        const staticGrid = grid([1, 2]);
        const historyGrid = grid([3, 4]);
        const history: SimulatorSnapshot[] = [{ time: 1, grid: historyGrid, wells: [] }];

        expect(selectActiveGrid({ history, currentIndex: 0, gridState: staticGrid, expectedCellCount: 2 })).toBe(historyGrid);
    });

    it('falls back to static grid when history index is invalid', () => {
        const staticGrid = grid([1, 2]);
        expect(selectActiveGrid({ history: [], currentIndex: 3, gridState: staticGrid, expectedCellCount: 2 })).toBe(staticGrid);
    });

    it('scans object-shaped GridState history for pressure max', () => {
        const history: SimulatorSnapshot[] = [
            { time: 0, grid: grid([10, 20]), wells: [] },
            { time: 1, grid: grid([15, 35]), wells: [] },
        ];

        expect(getPressureMaxFromHistorySlice({ history, startIndex: 1, expectedCellCount: 2 })).toBe(35);
    });

    it('clamps invalid history indices and selects wells safely', () => {
        const history: SimulatorSnapshot[] = [{ time: 0, grid: grid([1]), wells: [{ i: 0, j: 0, k: 0 }] }];
        expect(clampHistoryIndex(10, history.length)).toBe(0);
        expect(clampHistoryIndex(0, 0)).toBe(-1);
        expect(selectActiveWellState({ history, currentIndex: 10, wellState: null })).toHaveLength(1);
    });

    it('builds per-layer visual thicknesses with fallback values', () => {
        expect(buildLayerThicknesses({ nz: 3, cellDz: 2, cellDzPerLayer: [1, 0, 4], visualExaggeration: 10 })).toEqual([10, 20, 40]);
        expect(getExpectedCellCount({ nx: 2, ny: 3, nz: 4 })).toBe(24);
    });

    it('sets the pressure display range from initial pressure and active pressure controls', () => {
        expect(resolvePressureDisplayRange({
            initialPressure: 1500,
            producerEnabled: true,
            producerControlMode: 'pressure',
            producerBhp: 50,
            injectorEnabled: false,
            injectorBhp: 2000,
        })).toEqual({ min: 50, max: 1500 });
    });

    it('uses configured well limits for rate-controlled wells', () => {
        expect(resolvePressureDisplayRange({
            initialPressure: 300,
            producerControlMode: 'rate',
            producerBhp: 100,
            injectorEnabled: true,
            injectorControlMode: 'rate',
            injectorBhp: 500,
        })).toEqual({ min: 100, max: 500 });
    });
});
