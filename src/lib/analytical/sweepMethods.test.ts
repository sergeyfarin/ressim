import { describe, expect, it } from 'vitest';
import {
    DEFAULT_SWEEP_METHOD,
    SWEEP_METHODS,
    describeSweepMethod,
} from './sweepMethods';
import {
    computeCombinedSweep,
    computeSweepRecoveryFactor,
    type SweepAnalyticalMethod,
    type SweepGeometry,
} from './sweepEfficiency';
import { getScenario, getScenarioAnalyticalOptions, listScenarios } from '../catalog/scenarios';

const ROCK = { s_wc: 0.15, s_or: 0.2, n_w: 2, n_o: 2, k_rw_max: 0.4, k_ro_max: 0.9 };
const FLUID = { mu_w: 0.5, mu_o: 1.0 };

function sweepCurves(
    perms: number[],
    geometry: SweepGeometry,
    method: SweepAnalyticalMethod,
) {
    const combined = computeCombinedSweep(ROCK, FLUID, perms, 5, 3.0, 200, geometry, method);
    const rf = computeSweepRecoveryFactor(ROCK, FLUID, perms, 5, 3.0, 200, geometry, method);
    return {
        eA: combined.arealSweep.curve.map((p) => p.efficiency),
        eV: combined.verticalSweep.curve.map((p) => p.efficiency),
        eVol: combined.combined.map((p) => p.efficiency),
        rf: rf.curve.map((p) => p.rfSweep),
    };
}

function maxAbsDiff(a: number[], b: number[]): number {
    const diffs = a.map((v, i) => Math.abs(v - (b[i] ?? NaN))).filter(Number.isFinite);
    return diffs.length ? Math.max(...diffs) : NaN;
}

describe('sweepMethods', () => {
    it('describes every selectable correlation', () => {
        for (const method of Object.keys(SWEEP_METHODS) as SweepAnalyticalMethod[]) {
            const descriptor = SWEEP_METHODS[method];
            expect(descriptor.method).toBe(method);
            expect(descriptor.label.length).toBeGreaterThan(0);
            expect(descriptor.reference).toMatch(/\(\d{4}\)/);
        }
    });

    it('states the decomposition caveat for the components the geometry shows', () => {
        // ROADMAP 2.2: improving the total-recovery comparison must not promote
        // the per-component panels to predictions.
        expect(describeSweepMethod('stiles', 'both').summary)
            .toContain('E_A and E_V remain analytical diagnostic decomposition views');
        expect(describeSweepMethod('stiles', 'areal').summary)
            .toContain('E_A remains an analytical diagnostic decomposition view');
        expect(describeSweepMethod('stiles', 'vertical').summary)
            .toContain('E_V remains an analytical diagnostic decomposition view');
    });

    it('keeps each correlation named after what it does to the total', () => {
        expect(describeSweepMethod('stiles', 'both').summary).toContain('layer-by-layer');
        expect(describeSweepMethod('dykstra-parsons', 'both').summary).toContain('Craig areal');
    });

    it('never claims a sweep component the geometry holds at 1', () => {
        // A vertical-only scenario has no areal sweep, and vice versa. A single
        // geometry-blind summary would tell users their answer came from a
        // factor that is identically 1.
        const vertical = describeSweepMethod('dykstra-parsons', 'vertical').summary;
        expect(vertical).toContain('E_A = 1 at this geometry');
        expect(vertical).not.toContain('Craig areal ×');

        const areal = describeSweepMethod('dykstra-parsons', 'areal').summary;
        expect(areal).toContain('E_V = 1 at this geometry');

        // Stiles has nothing to order without layers, which is *why* the two
        // correlations are numerically identical at areal geometry.
        expect(describeSweepMethod('stiles', 'areal').summary)
            .toContain('identical to Dykstra-Parsons at this geometry');
    });
});

describe('sweep method sensitivity by geometry', () => {
    // These measurements are why `sweepMethods` is opt-in per scenario rather
    // than offered automatically wherever analyticalMethod is 'sweep'.

    it('is numerically identical at areal-only geometry', () => {
        // One layer, no vertical component: both correlations defer to the same
        // Craig five-spot curve, so a Stiles/Dykstra-Parsons toggle on an areal
        // scenario would be a control that changes nothing on screen.
        const dp = sweepCurves([100], 'areal', 'dykstra-parsons');
        const st = sweepCurves([100], 'areal', 'stiles');
        expect(maxAbsDiff(dp.eA, st.eA)).toBe(0);
        expect(maxAbsDiff(dp.eVol, st.eVol)).toBe(0);
        expect(maxAbsDiff(dp.rf, st.rf)).toBe(0);
    });

    it('diverges materially once layers are resolved', () => {
        const perms = [500, 200, 100, 50, 20];
        for (const geometry of ['vertical', 'both'] as const) {
            const dp = sweepCurves(perms, geometry, 'dykstra-parsons');
            const st = sweepCurves(perms, geometry, 'stiles');
            expect(maxAbsDiff(dp.eV, st.eV), geometry).toBeGreaterThan(0.01);
            expect(maxAbsDiff(dp.rf, st.rf), geometry).toBeGreaterThan(0.001);
        }
    });
});

describe('scenario sweep-method options', () => {
    it('offers no toggle where the correlations cannot differ', () => {
        expect(getScenarioAnalyticalOptions(getScenario('sweep_areal'))).toEqual([]);
    });

    it('offers both correlations on the layered sweep scenarios', () => {
        for (const key of ['sweep_vertical', 'sweep_combined']) {
            const options = getScenarioAnalyticalOptions(getScenario(key));
            expect(options.map((o) => o.sweepMethod), key)
                .toEqual(expect.arrayContaining(['stiles', 'dykstra-parsons']));
            expect(options.filter((o) => o.default), key).toHaveLength(1);
        }
    });

    it('preserves each scenario default: Stiles on combined, Dykstra-Parsons on vertical', () => {
        expect(getScenarioAnalyticalOptions(getScenario('sweep_combined'))[0].sweepMethod).toBe('stiles');
        expect(getScenarioAnalyticalOptions(getScenario('sweep_vertical'))[0].sweepMethod).toBe('dykstra-parsons');
    });

    it('derives option prose from the method table, not from scenario files', () => {
        const option = getScenarioAnalyticalOptions(getScenario('sweep_combined'))
            .find((o) => o.sweepMethod === 'stiles')!;
        const expected = describeSweepMethod('stiles', 'both');
        expect(option.label).toBe(expected.label);
        expect(option.summary).toBe(expected.summary);
        expect(option.reference).toBe(expected.reference);
    });

    it('offers analytical options only on sweep scenarios', () => {
        for (const scenario of listScenarios()) {
            if (scenario.capabilities.analyticalMethod === 'sweep') continue;
            expect(getScenarioAnalyticalOptions(scenario), scenario.key).toEqual([]);
        }
    });

    it('falls back to the shared default for scenarios that declare no methods', () => {
        expect(DEFAULT_SWEEP_METHOD).toBe('dykstra-parsons');
        // sweep_areal declares none, so it takes the shared default.
        const areal = getScenario('sweep_areal')!;
        expect((areal.capabilities as { sweepMethods?: readonly string[] }).sweepMethods).toBeUndefined();
    });
});
