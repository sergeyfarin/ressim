import { describe, expect, it } from 'vitest';
import {
    buildOutput3D,
    buildOutputProfile,
    defaultOutput3DProperty,
    resolveOutputSource,
    type LiveRunOutput,
    type StoredRunOutput,
} from './outputSource';
import type { GridState, RateHistoryPoint, SimulatorSnapshot, WellState } from '../simulator-types';
import type { Scenario } from '../catalog/scenarios';

const grid = (tag: number) => ({ pressure: [tag] }) as unknown as GridState;
const wells = (tag: number) => [{ tag }] as unknown as WellState;
const snapshot = (time: number, tag: number) =>
    ({ time, grid: grid(tag), wells: wells(tag) }) as unknown as SimulatorSnapshot;
const rate = (time: number) => ({ time }) as unknown as RateHistoryPoint;

const baseParams = {
    nx: 10, ny: 1, nz: 3, cellDx: 5, cellDy: 6, cellDz: 7, cellDzPerLayer: [1, 2, 3],
    reservoirPorosity: 0.2, initialSaturation: 0.1,
    injectorI: 0, injectorJ: 0, producerI: 9, producerJ: 0,
    injectorKLayers: [2], producerKLayers: [0, 1],
    s_wc: 0.1, s_or: 0.2, n_w: 2, n_o: 2, k_rw_max: 1, k_ro_max: 1, mu_w: 0.5, mu_o: 1,
    initialPressure: 300, producerBhp: 100, injectorBhp: 500,
};

const live: LiveRunOutput = {
    params: { ...baseParams, nx: 99, injectorEnabled: true, injectedFluid: 'gas', threePhaseModeEnabled: false },
    history: [snapshot(1, 101), snapshot(2, 102)],
    rateHistory: [rate(1), rate(2)],
    gridState: grid(199),
    wellState: wells(199),
    simTime: 2,
    scenarioMode: 'depletion',
};

const stored: StoredRunOutput = {
    key: 'wf:variant-a',
    label: 'Variant A',
    params: { ...baseParams, injectorEnabled: false, injectedFluid: 'water' },
    history: [],
    rateHistory: [rate(10), rate(20)],
    finalSnapshot: null,
};

describe('resolveOutputSource (#14)', () => {
    it('is the live runtime when nothing is selected', () => {
        const source = resolveOutputSource({ selected: null, selectedScenarioMode: 'none', live });
        expect(source.kind).toBe('live');
        expect(source.label).toBe('Live runtime');
        expect(source.params.nx).toBe(99);
        expect(source.finalGrid).toBe(live.gridState);
        expect(source.scenarioMode).toBe('depletion');
    });

    it('is the stored result when one is selected, with its own end time', () => {
        const source = resolveOutputSource({ selected: stored, selectedScenarioMode: 'waterflood', live });
        expect(source).toMatchObject({ kind: 'result', resultKey: 'wf:variant-a', label: 'Variant A' });
        expect(source.params.nx).toBe(10);
        // No final snapshot: the end time comes from the result's own rate history.
        expect(source.finalTime).toBe(20);
        expect(source.scenarioMode).toBe('waterflood');
    });
});

describe('payloads never mix a stored result with the live model (#14)', () => {
    const source = resolveOutputSource({ selected: stored, selectedScenarioMode: 'waterflood', live });

    it('the profile reports the result\'s own state, or none, never the live grid', () => {
        const profile = buildOutputProfile(source, undefined);
        expect(profile.gridState).toBeNull();
        expect(profile.nx).toBe(10);
        expect(profile.simTime).toBe(20);
        expect(profile.sourceLabel).toBe('Variant A');
        // The live model has an injector and this result does not.
        expect(profile.spatialProfileWellPathLabel).toBe('Diagonal');
        expect(profile.injectorKLayers).toEqual([2]);
        expect(profile.spatialReference).toEqual({ kind: 'buckley-leverett' });
    });

    it('the 3D payload shows the result\'s end, not the live grid or the live replay time', () => {
        const output = buildOutput3D(source, 5, 2);
        expect(output.currentIndex).toBe(-1);
        expect(output.gridState).toBeNull();
        expect(output.wellState).toBeNull();
        expect(output.replayTime).toBe(20);
        expect(output.cellDzPerLayer).toEqual([1, 2, 3]);
    });

    it('the default 3D property follows the result\'s fluid, not the live one', () => {
        expect(defaultOutput3DProperty(source, { default3DScalar: 'saturation_water' } as Scenario['capabilities']))
            .toBe('saturation_water');
    });
});

describe('the live source', () => {
    const source = resolveOutputSource({ selected: null, selectedScenarioMode: 'none', live });

    it('replays its own history, clamped to its length', () => {
        const output = buildOutput3D(source, 7, 2);
        expect(output.currentIndex).toBe(1);
        expect(output.gridState).toEqual(grid(102));
        expect(output.replayTime).toBe(2);
    });

    it('falls back to the worker\'s latest state and replay time without history', () => {
        const noHistory = resolveOutputSource({
            selected: null,
            selectedScenarioMode: 'none',
            live: { ...live, history: [] },
        });
        const output = buildOutput3D(noHistory, 0, 1.5);
        expect(output.currentIndex).toBe(-1);
        expect(output.gridState).toEqual(grid(199));
        expect(output.replayTime).toBe(1.5);
    });

    it('opens gas saturation only for a three-phase gas flood', () => {
        const caps = { default3DScalar: 'pressure' } as Scenario['capabilities'];
        expect(defaultOutput3DProperty(source, caps)).toBe('pressure');
        const threePhase = resolveOutputSource({
            selected: null,
            selectedScenarioMode: 'none',
            live: { ...live, params: { ...live.params, threePhaseModeEnabled: true } },
        });
        expect(defaultOutput3DProperty(threePhase, caps)).toBe('saturation_gas');
    });

    it('declares a sweep reference only for an areal or combined sweep scenario', () => {
        const caps = { analyticalMethod: 'sweep', sweepGeometry: 'areal' } as unknown as Scenario['capabilities'];
        expect(buildOutputProfile(source, caps).spatialReference).toMatchObject({ kind: 'sweep', geometry: 'areal' });
        const vertical = { analyticalMethod: 'sweep', sweepGeometry: 'vertical' } as unknown as Scenario['capabilities'];
        expect(buildOutputProfile(source, vertical).spatialReference).toBeNull();
    });
});
