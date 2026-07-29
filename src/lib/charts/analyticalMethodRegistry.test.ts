import { describe, expect, it } from 'vitest';
import {
    ANALYTICAL_METHOD_DESCRIPTORS,
    getAnalyticalMethodDescriptor,
    slotsForContext,
} from './analyticalMethodRegistry';
import { ANALYTICAL_OUTPUT_CONTRACTS, type AnalyticalMethod } from '../catalog/scenarios';

const ALL_METHODS = Object.keys(ANALYTICAL_OUTPUT_CONTRACTS) as AnalyticalMethod[];

describe('analyticalMethodRegistry', () => {
    it('has a descriptor for every AnalyticalMethod', () => {
        for (const method of ALL_METHODS) {
            expect(ANALYTICAL_METHOD_DESCRIPTORS[method]?.method).toBe(method);
        }
    });

    it('agrees with each method output contract on the native x-axis', () => {
        // Two declarations of the same fact today: the scenario-facing contract
        // in scenarios.ts and the chart-facing descriptor here. They must not
        // drift while both exist.
        for (const method of ALL_METHODS) {
            expect(ANALYTICAL_METHOD_DESCRIPTORS[method].nativeXAxis)
                .toBe(ANALYTICAL_OUTPUT_CONTRACTS[method].nativeXAxis);
        }
    });

    it('marks the Buckley-Leverett family as pvi-native and the rest as time-native', () => {
        expect(getAnalyticalMethodDescriptor('buckley-leverett').nativeXAxis).toBe('pvi');
        expect(getAnalyticalMethodDescriptor('gas-oil-bl').nativeXAxis).toBe('pvi');
        expect(getAnalyticalMethodDescriptor('depletion').nativeXAxis).toBe('time');
        expect(getAnalyticalMethodDescriptor('well-test').nativeXAxis).toBe('time');
    });

    it('falls back to the no-reference descriptor for absent or unknown methods', () => {
        for (const method of [null, undefined, 'not-a-method' as AnalyticalMethod]) {
            const descriptor = getAnalyticalMethodDescriptor(method);
            expect(descriptor.method).toBe('none');
            expect(descriptor.slots).toEqual([]);
            expect(descriptor.fromResult).toBeNull();
            expect(descriptor.fromParams).toBeNull();
        }
    });

    it('gives methods without a reference solution no curve slots', () => {
        expect(getAnalyticalMethodDescriptor('none').slots).toEqual([]);
        expect(getAnalyticalMethodDescriptor('digitized-reference').slots).toEqual([]);
    });

    it('makes sweep a method with sweep panels and no primary curve slots', () => {
        // Sweep used to declare analyticalMethod 'buckley-leverett' plus a
        // showSweepPanel flag, so the stack built BL primary overlays and a
        // separate strip pass removed them again. Declaring no slots means they
        // are never built, which is what deleted the strip pass.
        const sweep = getAnalyticalMethodDescriptor('sweep');
        expect(sweep.producesSweepPanels).toBe(true);
        expect(sweep.slots).toEqual([]);
        expect(sweep.fromResult).toBeNull();
        expect(sweep.fromParams).toBeNull();
    });

    it('gives sweep panels to sweep alone', () => {
        for (const method of ALL_METHODS) {
            expect(ANALYTICAL_METHOD_DESCRIPTORS[method].producesSweepPanels).toBe(method === 'sweep');
        }
    });

    it('shows sweep the same simulation curves as a Buckley-Leverett waterflood', () => {
        // The reference side differs; the simulation side does not.
        expect(getAnalyticalMethodDescriptor('sweep').simulationCurveSet).toBe('water-cut');
        expect(getAnalyticalMethodDescriptor('buckley-leverett').simulationCurveSet).toBe('water-cut');
        expect(getAnalyticalMethodDescriptor('gas-oil-bl').simulationCurveSet).toBe('gas-cut');
        expect(getAnalyticalMethodDescriptor('depletion').simulationCurveSet).toBe('oil-rate');
    });

    it('gives every slot of a method with a reference solution a distinct curve key', () => {
        for (const method of ALL_METHODS) {
            const keys = ANALYTICAL_METHOD_DESCRIPTORS[method].slots.map((slot) => slot.curveKey);
            expect(new Set(keys).size).toBe(keys.length);
        }
    });

    it('routes the well-test flowing-BHP reference to the producer_bhp panel', () => {
        // Not `diagnostics`: a drawdown test measures flowing pressure at the
        // well, so its reference must never be drawn against average pressure.
        const slot = getAnalyticalMethodDescriptor('well-test').slots
            .find((candidate) => candidate.curveKey === 'producer-bhp-reference');
        expect(slot?.panelKey).toBe('producer_bhp');
    });

    it('honours a shared well-test reference for numerical-only studies', () => {
        // Skin/permeability studies request per-result curves, while a grid
        // study holds the analytical physics fixed and requests one shared
        // reference rather than three coincident copies.
        const descriptor = getAnalyticalMethodDescriptor('well-test');
        expect(descriptor.resolveOverlayMode({ requested: 'shared', paramSets: [] })).toBe('shared');
        expect(descriptor.resolveOverlayMode({ requested: 'per-result', paramSets: [] })).toBe('per-result');
        expect(slotsForContext(descriptor, 'shared').map((slot) => slot.curveKey))
            .toEqual(['producer-bhp-reference', 'oil-rate-reference']);
    });

    it('honours an explicit shared/per-result request for the BL family', () => {
        const descriptor = getAnalyticalMethodDescriptor('buckley-leverett');
        expect(descriptor.resolveOverlayMode({ requested: 'shared', paramSets: [] })).toBe('shared');
        expect(descriptor.resolveOverlayMode({ requested: 'per-result', paramSets: [] })).toBe('per-result');
    });

    it('infers per-result BL overlays when the variants differ in fractional-flow physics', () => {
        const descriptor = getAnalyticalMethodDescriptor('buckley-leverett');
        const base = { s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2, k_rw_max: 1, k_ro_max: 1, mu_w: 0.5, mu_o: 1.0 };
        expect(descriptor.resolveOverlayMode({
            requested: 'auto',
            paramSets: [base, { ...base }],
        })).toBe('shared');
        expect(descriptor.resolveOverlayMode({
            requested: 'auto',
            paramSets: [base, { ...base, mu_o: 5.0 }],
        })).toBe('per-result');
    });

    // These two slots are deliberately narrower than their siblings, preserving
    // pre-registry behavior. See the TODO.md "analytical slot-context
    // asymmetries" item — they are candidates to widen, not settled design.
    it('keeps the gas-oil cumulative-oil reference shared-only', () => {
        const descriptor = getAnalyticalMethodDescriptor('gas-oil-bl');
        const cumulativeKeys = (context: 'shared' | 'per-result' | 'pending' | 'preview') =>
            slotsForContext(descriptor, context).map((slot) => slot.curveKey);
        expect(cumulativeKeys('shared')).toContain('cum-oil-reference');
        expect(cumulativeKeys('per-result')).not.toContain('cum-oil-reference');
        expect(cumulativeKeys('preview')).not.toContain('cum-oil-reference');
    });

    it('omits the well-test oil-rate reference for still-pending variants', () => {
        const descriptor = getAnalyticalMethodDescriptor('well-test');
        expect(slotsForContext(descriptor, 'pending').map((slot) => slot.curveKey))
            .toEqual(['producer-bhp-reference']);
        expect(slotsForContext(descriptor, 'per-result').map((slot) => slot.curveKey))
            .toEqual(['producer-bhp-reference', 'oil-rate-reference']);
    });
});
