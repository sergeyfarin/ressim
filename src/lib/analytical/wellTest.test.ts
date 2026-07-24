import { describe, expect, it } from 'vitest';
import {
    EULER_GAMMA,
    EXP_EULER_GAMMA,
    SEMILOG_VALIDITY_U,
    buildupPressure,
    drawdownPressure,
    exponentialIntegralE1,
    fitSemilogLine,
    hornerTime,
    hydraulicDiffusivity,
    lineSourcePressure,
    permeabilityFromSemilogSlope,
    radialPressureGroup,
    radiusOfInvestigation,
    semilogSlope,
    semilogValidFromTime,
    skinFromSemilogIntercept,
    type ReservoirTestProps,
} from './wellTest';

/** A representative oil well: 50 mD, 20 m net, 0.2 cP-scale light oil. */
const props: ReservoirTestProps = {
    k: 50,
    h: 20,
    porosity: 0.2,
    mu: 1.0,
    c_t: 1.5e-4,
    r_w: 0.1,
    skin: 0,
};

/** Constant production rate [m3/day reservoir]. */
const Q = 100;

describe('exponentialIntegralE1', () => {
    // Reference values computed independently at 40 significant digits
    // (Python `decimal`, series below u = 1 and continued fraction above,
    // 200 terms) rather than transcribed from a printed table.
    it.each([
        [0.01, 4.0379295765381138],
        [0.1, 1.8229239584193907],
        [0.5, 0.5597735947761608],
        [1.0, 0.2193839343955203],
        [2.0, 0.0489005107080611],
        [5.0, 0.0011482955912753],
        [10.0, 4.1569689296853e-6],
    ])('matches the independently computed value at u = %f', (u, expected) => {
        expect(exponentialIntegralE1(u)).toBeCloseTo(expected, 12);
        // Relative accuracy, which is the meaningful bar across seven decades.
        expect(Math.abs(exponentialIntegralE1(u) - expected) / expected).toBeLessThan(1e-12);
    });

    it('approaches -ln(u) - gamma for small u, which is what the semilog form assumes', () => {
        const u = SEMILOG_VALIDITY_U;
        const approx = -Math.log(u) - EULER_GAMMA;
        const exact = exponentialIntegralE1(u);
        // The stated ~1% accuracy bound at the validity threshold.
        expect(Math.abs(exact - approx) / exact).toBeLessThan(0.01);

        // Two decades earlier it is far tighter still.
        const u2 = 1e-4;
        const exact2 = exponentialIntegralE1(u2);
        expect(Math.abs(exact2 - (-Math.log(u2) - EULER_GAMMA)) / exact2).toBeLessThan(1e-4);
    });

    it('is strictly decreasing and handles the singular and invalid arguments', () => {
        expect(exponentialIntegralE1(0)).toBe(Number.POSITIVE_INFINITY);
        expect(Number.isNaN(exponentialIntegralE1(-1))).toBe(true);
        let prev = Number.POSITIVE_INFINITY;
        for (const u of [1e-6, 1e-4, 1e-2, 0.5, 1, 2, 5, 10, 20]) {
            const v = exponentialIntegralE1(u);
            expect(v).toBeLessThan(prev);
            expect(v).toBeGreaterThan(0);
            prev = v;
        }
    });

    it('agrees across the series/continued-fraction branch boundary at u = 1', () => {
        const below = exponentialIntegralE1(1 - 1e-9);
        const above = exponentialIntegralE1(1 + 1e-9);
        expect(Math.abs(below - above)).toBeLessThan(1e-9);
    });
});

describe('hydraulicDiffusivity', () => {
    it('is derived from the engine Darcy constant, not a field-unit formula', () => {
        // eta = C.k/(phi.mu.c_t) with C = 8.5269888e-3
        const expected = (8.5269888e-3 * props.k) / (props.porosity * props.mu * props.c_t);
        expect(hydraulicDiffusivity(props)).toBeCloseTo(expected, 10);
    });

    it('scales linearly with k and inversely with porosity, viscosity and compressibility', () => {
        const base = hydraulicDiffusivity(props);
        expect(hydraulicDiffusivity({ ...props, k: props.k * 3 })).toBeCloseTo(3 * base, 6);
        expect(hydraulicDiffusivity({ ...props, porosity: props.porosity * 2 })).toBeCloseTo(base / 2, 6);
        expect(hydraulicDiffusivity({ ...props, mu: props.mu * 4 })).toBeCloseTo(base / 4, 6);
        expect(hydraulicDiffusivity({ ...props, c_t: props.c_t * 10 })).toBeCloseTo(base / 10, 6);
    });

    it('rejects non-physical inputs rather than returning a number', () => {
        expect(Number.isNaN(hydraulicDiffusivity({ ...props, porosity: 0 }))).toBe(true);
        expect(Number.isNaN(hydraulicDiffusivity({ ...props, c_t: 0 }))).toBe(true);
        expect(Number.isNaN(hydraulicDiffusivity({ ...props, mu: -1 }))).toBe(true);
    });
});

describe('drawdown — line-source vs semilog', () => {
    it('the semilog form converges to the full line-source solution once u < 0.01', () => {
        const tValid = semilogValidFromTime(props);
        for (const t of [tValid, tValid * 10, tValid * 1000, 1, 10]) {
            const exact = lineSourcePressure(300, Q, props.r_w, t, props);
            const approx = drawdownPressure(300, Q, t, props);
            // Both are drawdowns from 300 bar; compare the drawdown magnitudes.
            expect(Math.abs(exact - approx) / (300 - exact)).toBeLessThan(0.011);
        }
    });

    it('the two disagree before the semilog time, which is why that bound is exported', () => {
        const tEarly = semilogValidFromTime(props) / 1000;
        const exact = lineSourcePressure(300, Q, props.r_w, tEarly, props);
        const approx = drawdownPressure(300, Q, tEarly, props);
        expect(Math.abs(exact - approx)).toBeGreaterThan(0.01);
    });

    it('drops one semilog slope per decade of time', () => {
        const p1 = drawdownPressure(300, Q, 1, props);
        const p10 = drawdownPressure(300, Q, 10, props);
        const p100 = drawdownPressure(300, Q, 100, props);
        const m = semilogSlope(Q, props);
        expect(p1 - p10).toBeCloseTo(m, 10);
        expect(p10 - p100).toBeCloseTo(m, 10);
    });

    it('makes skin a constant offset that does not change the slope', () => {
        const damaged: ReservoirTestProps = { ...props, skin: 5 };
        const clean = drawdownPressure(300, Q, 10, props);
        const dirty = drawdownPressure(300, Q, 10, damaged);
        expect(clean - dirty).toBeCloseTo(2 * 5 * radialPressureGroup(Q, props), 10);
        expect(semilogSlope(Q, damaged)).toBeCloseTo(semilogSlope(Q, props), 12);

        // A stimulated well produces at a higher flowing pressure than a clean one.
        expect(drawdownPressure(300, Q, 10, { ...props, skin: -2 })).toBeGreaterThan(clean);
    });

    it('returns the initial pressure at and before t = 0', () => {
        expect(drawdownPressure(300, Q, 0, props)).toBe(300);
        expect(lineSourcePressure(300, Q, props.r_w, 0, props)).toBe(300);
    });

    it('applies skin only at the wellbore, not out in the reservoir', () => {
        const damaged: ReservoirTestProps = { ...props, skin: 5 };
        const atWell = lineSourcePressure(300, Q, props.r_w, 10, damaged);
        const inReservoir = lineSourcePressure(300, Q, 50, 10, damaged);
        const inReservoirClean = lineSourcePressure(300, Q, 50, 10, props);
        expect(atWell).toBeLessThan(lineSourcePressure(300, Q, props.r_w, 10, props));
        expect(inReservoir).toBeCloseTo(inReservoirClean, 12);
    });
});

describe('inverse problem — recovering k and s from a synthetic test', () => {
    /**
     * The round trip that matters: generate a pressure history from known
     * properties, analyse it exactly as an engineer would (straight line on a
     * semilog plot, slope for k, one-hour intercept for s), and check the
     * original inputs come back.
     */
    it('recovers permeability from a drawdown semilog slope', () => {
        const times = [0.05, 0.1, 0.2, 0.5, 1, 2, 5, 10];
        const points = times.map((t) => ({ x: t, y: drawdownPressure(300, Q, t, props) }));
        const fit = fitSemilogLine(points);

        expect(fit.count).toBe(times.length);
        expect(fit.slope).toBeCloseTo(semilogSlope(Q, props), 8);
        expect(permeabilityFromSemilogSlope(fit.slope, Q, props.h, props.mu)).toBeCloseTo(props.k, 6);
    });

    it.each([-3, -1, 0, 2, 5, 12])('recovers skin = %d from the one-hour intercept', (skin) => {
        const tested: ReservoirTestProps = { ...props, skin };
        const times = [0.05, 0.1, 0.2, 0.5, 1, 2, 5, 10];
        const points = times.map((t) => ({ x: t, y: drawdownPressure(300, Q, t, tested) }));
        const fit = fitSemilogLine(points);

        const k = permeabilityFromSemilogSlope(fit.slope, Q, tested.h, tested.mu);
        expect(k).toBeCloseTo(tested.k, 6);

        // Read the extrapolated straight line at one hour, as the method requires.
        const tRef = 1 / 24;
        const pAtRef = fit.intercept - fit.slope * Math.log10(tRef);
        const recovered = skinFromSemilogIntercept(300 - pAtRef, fit.slope, { ...tested, k }, tRef);
        expect(recovered).toBeCloseTo(skin, 6);
    });

    it('recovers permeability from a Horner buildup, where skin cancels', () => {
        const tp = 10; // days of production before shut-in
        const damaged: ReservoirTestProps = { ...props, skin: 7 };
        const shutIns = [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1, 2];

        const points = shutIns.map((dt) => ({
            x: hornerTime(tp, dt),
            y: buildupPressure(300, Q, tp, dt, damaged),
        }));
        const fit = fitSemilogLine(points);

        expect(fit.slope).toBeCloseTo(semilogSlope(Q, damaged), 8);
        expect(permeabilityFromSemilogSlope(fit.slope, Q, damaged.h, damaged.mu)).toBeCloseTo(props.k, 6);

        // The buildup is skin-independent: a clean well gives the identical line.
        const cleanFit = fitSemilogLine(shutIns.map((dt) => ({
            x: hornerTime(tp, dt),
            y: buildupPressure(300, Q, tp, dt, props),
        })));
        expect(cleanFit.slope).toBeCloseTo(fit.slope, 12);
    });

    it('extrapolates the Horner line to the initial pressure at infinite shut-in', () => {
        const tp = 10;
        const shutIns = [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1, 2];
        const fit = fitSemilogLine(shutIns.map((dt) => ({
            x: hornerTime(tp, dt),
            y: buildupPressure(300, Q, tp, dt, props),
        })));
        // Horner time 1 (log10 = 0) is infinite shut-in, where p* = p_i.
        expect(fit.intercept).toBeCloseTo(300, 6);
    });
});

describe('Horner time', () => {
    it('goes to infinity at shut-in and towards 1 as shut-in time grows', () => {
        expect(hornerTime(10, 0)).toBe(Number.POSITIVE_INFINITY);
        expect(hornerTime(10, 10)).toBeCloseTo(2, 12);
        expect(hornerTime(10, 1e6)).toBeCloseTo(1, 4);
    });

    it('makes the buildup pressure rise monotonically towards p_i', () => {
        let prev = -Infinity;
        for (const dt of [0.001, 0.01, 0.1, 1, 10, 100, 1000]) {
            const p = buildupPressure(300, Q, 10, dt, props);
            expect(p).toBeGreaterThan(prev);
            expect(p).toBeLessThanOrEqual(300);
            prev = p;
        }
        expect(buildupPressure(300, Q, 10, 1e7, props)).toBeCloseTo(300, 3);
    });

    it('falls back to the flowing pressure for a non-positive shut-in time', () => {
        expect(buildupPressure(300, Q, 10, 0, props)).toBeCloseTo(drawdownPressure(300, Q, 10, props), 12);
    });
});

describe('diagnostic quantities', () => {
    it('grows the radius of investigation as sqrt(t)', () => {
        const r1 = radiusOfInvestigation(1, props);
        const r4 = radiusOfInvestigation(4, props);
        expect(r4 / r1).toBeCloseTo(2, 10);
        expect(radiusOfInvestigation(0, props)).toBe(0);
        expect(r1).toBeCloseTo(2 * Math.sqrt(hydraulicDiffusivity(props)), 10);
    });

    it('reports a semilog start time consistent with the u threshold it is defined by', () => {
        const t = semilogValidFromTime(props);
        const eta = hydraulicDiffusivity(props);
        const u = (props.r_w * props.r_w) / (4 * eta * t);
        expect(u).toBeCloseTo(SEMILOG_VALIDITY_U, 12);
        // A tighter wellbore in a more diffusive rock is analysable sooner.
        expect(semilogValidFromTime({ ...props, k: props.k * 100 })).toBeLessThan(t);
        expect(semilogValidFromTime({ ...props, r_w: props.r_w * 10 })).toBeGreaterThan(t);
    });

    it('uses exp(gamma) = 1.781 inside the semilog log argument', () => {
        expect(EXP_EULER_GAMMA).toBeCloseTo(1.7810724, 6);
    });
});

describe('fitSemilogLine', () => {
    it('recovers an exact straight line in log10 space', () => {
        const fit = fitSemilogLine([1, 10, 100, 1000].map((x) => ({ x, y: 50 - 3 * Math.log10(x) })));
        expect(fit.slope).toBeCloseTo(3, 12);
        expect(fit.intercept).toBeCloseTo(50, 12);
    });

    it('skips non-positive x and non-finite y instead of poisoning the fit', () => {
        const fit = fitSemilogLine([
            { x: 0, y: 999 },
            { x: -5, y: 999 },
            { x: 1, y: 50 },
            { x: 10, y: 47 },
            { x: 100, y: Number.NaN },
        ]);
        expect(fit.count).toBe(2);
        expect(fit.slope).toBeCloseTo(3, 12);
    });

    it('returns NaN rather than a bogus line when there is too little data', () => {
        expect(Number.isNaN(fitSemilogLine([{ x: 1, y: 50 }]).slope)).toBe(true);
        expect(Number.isNaN(fitSemilogLine([]).slope)).toBe(true);
        // All points at the same x: no slope is determined.
        expect(Number.isNaN(fitSemilogLine([{ x: 5, y: 1 }, { x: 5, y: 2 }]).slope)).toBe(true);
    });
});
