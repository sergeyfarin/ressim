import type { GridState, SimulatorSnapshot, WellState } from '../simulator-types';

export function getExpectedCellCount(input: { nx: number; ny: number; nz: number }): number {
    return Math.max(0, Math.round(Number(input.nx)) * Math.round(Number(input.ny)) * Math.round(Number(input.nz)));
}

export function isCompatibleGridState(grid: GridState | null | undefined, expectedCellCount: number): grid is GridState {
    return Boolean(
        grid
        && expectedCellCount > 0
        && grid.pressure
        && grid.pressure.length === expectedCellCount,
    );
}

export function clampHistoryIndex(index: number, historyLength: number): number {
    if (historyLength <= 0) return -1;
    return Math.max(0, Math.min(Math.round(Number(index)), historyLength - 1));
}

export function selectActiveGrid(input: {
    history: SimulatorSnapshot[];
    currentIndex: number;
    gridState: GridState | null;
    expectedCellCount: number;
}): GridState | null {
    const safeIndex = clampHistoryIndex(input.currentIndex, input.history.length);
    const historyGrid = safeIndex >= 0 ? input.history[safeIndex]?.grid ?? null : null;
    if (isCompatibleGridState(historyGrid, input.expectedCellCount)) return historyGrid;
    if (isCompatibleGridState(input.gridState, input.expectedCellCount)) return input.gridState;
    return null;
}

export function selectActiveWellState(input: {
    history: SimulatorSnapshot[];
    currentIndex: number;
    wellState: WellState | null;
}): WellState {
    const safeIndex = clampHistoryIndex(input.currentIndex, input.history.length);
    const historyWells = safeIndex >= 0 ? input.history[safeIndex]?.wells ?? null : null;
    if (Array.isArray(historyWells)) return historyWells;
    return Array.isArray(input.wellState) ? input.wellState : [];
}

export function getPressureMaxFromHistorySlice(input: {
    history: SimulatorSnapshot[];
    startIndex: number;
    expectedCellCount: number;
}): number | null {
    if (input.expectedCellCount <= 0 || input.history.length === 0) return null;

    const safeStart = Math.max(0, Math.min(input.history.length, Math.round(Number(input.startIndex))));
    let max = Number.NEGATIVE_INFINITY;

    for (let index = safeStart; index < input.history.length; index += 1) {
        const grid = input.history[index]?.grid;
        if (!isCompatibleGridState(grid, input.expectedCellCount)) continue;
        for (const value of grid.pressure) {
            const numeric = Number(value);
            if (Number.isFinite(numeric) && numeric > max) max = numeric;
        }
    }

    return Number.isFinite(max) ? max : null;
}

export function buildLayerThicknesses(input: {
    nz: number;
    cellDz: number;
    cellDzPerLayer?: number[];
    visualExaggeration?: number;
}): number[] {
    const count = Math.max(0, Math.round(Number(input.nz)));
    const fallback = Math.max(0.001, Number(input.cellDz) || 1);
    const exaggeration = Math.max(0.001, Number(input.visualExaggeration ?? 1));
    return Array.from({ length: count }, (_, index) => {
        const raw = Array.isArray(input.cellDzPerLayer) ? Number(input.cellDzPerLayer[index]) : Number.NaN;
        const thickness = Number.isFinite(raw) && raw > 0 ? raw : fallback;
        return thickness * exaggeration;
    });
}
