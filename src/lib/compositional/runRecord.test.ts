import { describe, expect, it } from 'vitest';

import { comp_co2_1d } from '../catalog/scenarios/comp_co2_1d';
import {
    buildCompositionalRunRecord,
    compositionalWellState,
    toSpatialSnapshot,
} from './runRecord';
import type { CompositionalCaseConfig, CompositionalSnapshot } from './types';

const config = comp_co2_1d.params.compositional as CompositionalCaseConfig;

const snapshot: CompositionalSnapshot = {
    time_days: 2.5,
    component_ids: ['CO2', 'C1', 'C10'],
    pressure: [140, 110, 90, 70, 55],
    composition: [[0.9, 0.5, 0.2, 0.1, 0.1], [0.05, 0.2, 0.3, 0.3, 0.3], [0.05, 0.3, 0.5, 0.6, 0.6]],
    phase_state: ['single-vapour', 'two-phase', 'two-phase', 'single-liquid', 'unresolved'],
    vapour_saturation: [1, 0.6, 0.25, 0, Number.NaN],
    inventory: [1, 2, 3],
    cumulative_well_moles: [],
};

describe('compositionalWellState', () => {
    it('places each completion on its cell and marks the well with a stream as the injector', () => {
        const wells = compositionalWellState(config);
        expect(wells.map((w) => [w.physical_well_id, w.i, w.j, w.k, w.injector])).toEqual([
            ['INJ', 0, 0, 0, true],
            ['PROD', config.grid.cells - 1, 0, 0, false],
        ]);
        expect(wells.map((w) => w.bhp)).toEqual([150, 50]);
    });
});

describe('toSpatialSnapshot', () => {
    const spatial = toSpatialSnapshot(snapshot, compositionalWellState(config));

    it('carries pressure and time through unchanged', () => {
        expect(spatial.time).toBe(2.5);
        expect([...spatial.grid.pressure]).toEqual(snapshot.pressure);
    });

    it('relabels vapour as gas and liquid as oil, with no water', () => {
        expect([...spatial.grid.sat_gas].slice(0, 4)).toEqual([1, 0.6, 0.25, 0]);
        expect([...spatial.grid.sat_oil].slice(0, 4).map((v) => Number(v.toFixed(12)))).toEqual([0, 0.4, 0.75, 1]);
        expect([...spatial.grid.sat_water]).toEqual([0, 0, 0, 0, 0]);
    });

    it('leaves a cell whose flash did not resolve without a saturation, rather than inventing one', () => {
        expect(Number.isNaN(spatial.grid.sat_gas[4])).toBe(true);
        expect(Number.isNaN(spatial.grid.sat_oil[4])).toBe(true);
    });
});

describe('buildCompositionalRunRecord', () => {
    it('derives the series once and keeps the stop it was given', () => {
        const record = buildCompositionalRunRecord({
            config,
            snapshots: [snapshot],
            stop: { reason: 'failed', message: 'The solver could not converge.' },
        });
        expect(record.series.timeDays).toEqual([2.5]);
        expect(record.series.componentIds).toEqual(['CO2', 'C1', 'C10']);
        expect(record.stop).toEqual({ reason: 'failed', message: 'The solver could not converge.' });
    });
});
