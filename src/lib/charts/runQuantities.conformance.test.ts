/**
 * The Python quantities module must compute what the application computes.
 *
 * Sharing a contract fixes the *names*: `contracts/run-quantities.json` is generated from
 * `RUN_QUANTITIES`, so an id, label or unit cannot drift. It does not fix the *numbers* — two
 * implementations of a cumulative integral can agree on what the curve is called and still
 * disagree about its values, which is the worse failure because a chart still renders.
 *
 * So this runs both on one rate history and compares them. The fixture deliberately includes the
 * cases the formulas actually differ on: an uneven time step (the rectangle rule is exact for a
 * step-average rate and a trapezoid is not), a step with no oil production (GOR is null, not
 * zero), a non-finite saturation (null, not a substitute), and a negative reported rate (taken as
 * a magnitude). A fixture of smooth positive numbers would pass against almost any arithmetic.
 *
 * Skipped, not failed, when python3 is unavailable: this asserts agreement between two languages
 * and a machine without one of them cannot answer the question either way.
 */
import { describe, expect, it } from 'vitest';
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { integrateRunSeries } from '@ressim/quantities/runSeries';

const PY_MODULE_DIR = path.resolve('crates/ressim-py');

/** Chosen for their edge cases; see the module docs. */
const RATE_HISTORY = [
    {
        time: 0.5,
        total_production_oil: 120.5,
        total_production_gas: 4000,
        total_production_liquid: 150.25,
        total_injection: 200,
        avg_water_saturation: 0.31,
        producing_gor: 33.2,
    },
    {
        time: 2.0, // uneven step
        total_production_oil: -90.0, // reported negative; magnitude is the rate
        total_production_gas: 5200.75,
        total_production_liquid: 140.0,
        total_injection: 210.5,
        avg_water_saturation: 0.44,
        producing_gor: 57.8,
    },
    {
        time: 2.25,
        total_production_oil: 0, // no oil this step -> GOR is null, not zero
        total_production_gas: 6100,
        total_production_liquid: 130.0,
        total_injection: 0,
        avg_water_saturation: null, // not finite -> null, not a substitute
        producing_gor: 91.4,
    },
];

function pythonAvailable(): boolean {
    try {
        execFileSync('python3', ['--version'], { stdio: 'ignore' });
        return existsSync(path.join(PY_MODULE_DIR, 'ressim_quantities', '__init__.py'));
    } catch {
        return false;
    }
}

function pythonDerive(): Record<string, Array<number | null>> {
    const script = `
import json, sys
sys.path.insert(0, ${JSON.stringify(PY_MODULE_DIR)})
import ressim_quantities as rq
print(json.dumps(rq.derive(json.loads(sys.argv[1]))))
`;
    const out = execFileSync('python3', ['-c', script, JSON.stringify(RATE_HISTORY)], {
        encoding: 'utf8',
    });
    return JSON.parse(out);
}

describe('python run-quantities conformance', () => {
    const available = pythonAvailable();

    it.runIf(available)('agrees with the TypeScript cumulative integration', () => {
        const py = pythonDerive();
        const ts = integrateRunSeries(RATE_HISTORY as never);

        expect(py.cumulativeOil).toEqual(ts.oil);
        expect(py.cumulativeGas).toEqual(ts.gas);
        expect(py.cumulativeLiquid).toEqual(ts.liquid);
        expect(py.cumulativeInjection).toEqual(ts.injection);
        expect(py.time).toEqual(ts.time);

        // The fixture has to exercise accumulation, or equality is vacuous.
        expect(ts.oil.at(-1)).toBeGreaterThan(0);
    });

    it.runIf(available)('agrees on the per-point transforms, including their edge cases', () => {
        const py = pythonDerive();

        // Magnitudes, floored at zero — the second point reports a negative oil rate.
        expect(py.oilRate).toEqual([120.5, 90.0, 0]);
        expect(py.gasRate).toEqual([4000, 5200.75, 6100]);
        expect(py.injectionRate).toEqual([200, 210.5, 0]);

        // Gas cut is gas / (gas + oil), and 0 rather than NaN when nothing is produced.
        expect(py.gasCut![0]).toBeCloseTo(4000 / (4000 + 120.5), 12);
        expect(py.gasCut![2]).toBe(1);

        // A non-finite saturation reports null; it does not become zero.
        expect(py.avgWaterSat).toEqual([0.31, 0.44, null]);

        // GOR is null where no oil is produced, even though the engine reported a value.
        expect(py.gor).toEqual([33.2, 57.8, null]);
    });

    it.runIf(available)('exposes every contract quantity it can compute, with contract labels', () => {
        const script = `
import json, sys
sys.path.insert(0, ${JSON.stringify(PY_MODULE_DIR)})
import ressim_quantities as rq
print(json.dumps({
    "quantities": rq.quantities(json.loads(sys.argv[1])),
    "missing": rq.missing_inputs(),
    "ids": rq.quantity_ids(),
}))
`;
        const out = JSON.parse(
            execFileSync('python3', ['-c', script, JSON.stringify(RATE_HISTORY)], { encoding: 'utf8' }),
        );

        // Computed plus explicitly-unavailable must account for the whole contract: a quantity
        // that is neither is one nobody decided about.
        const accounted = new Set([...Object.keys(out.quantities), ...Object.keys(out.missing)]);
        expect([...accounted].sort()).toEqual([...out.ids].sort());

        expect(out.quantities['oil-rate'].label).toBe('Oil Rate');
        expect(out.quantities['oil-rate'].unit).toBe('Sm³/day');
        expect(out.quantities['cumulative-gas'].unit).toBe('Sm³');
        expect(out.missing['recovery-oil']).toContain('in place');
    });
});
