import { describe, expect, it } from 'vitest';
import {
    computeGasMaterialBalance,
    effectiveCompressibility,
    fitStraightLineGiip,
    gasDeviationFactor,
    gasFormationVolumeFactor,
    pressureForPOverZ,
    pressureOverZ,
    type GasPvtRow,
} from './gasMaterialBalance';
import { generateBlackOilTable } from '../physics/pvt';
import { PSI_PER_BAR, cToR } from '../physics/pvt';

const RESERVOIR_TEMPERATURE_C = 90;
const TABLE = generateBlackOilTable(35, 0.65, RESERVOIR_TEMPERATURE_C, 20, 450, 60, 1e-4) as GasPvtRow[];

/** An ideal gas: z = 1 everywhere, so B_g follows the gas law exactly. */
function idealGasTable(temperatureC: number): GasPvtRow[] {
    const rows: GasPvtRow[] = [];
    for (let p = 10; p <= 400; p += 10) {
        rows.push({
            p_bar: p,
            bg_m3m3: (14.7 / 519.67) * (1 * cToR(temperatureC) / (p * PSI_PER_BAR)),
        });
    }
    return rows;
}

describe('gas law inversion', () => {
    it('recovers z = 1 from an ideal-gas table at every pressure', () => {
        const table = idealGasTable(RESERVOIR_TEMPERATURE_C);
        for (const pressure of [15, 50, 130, 275, 390]) {
            expect(gasDeviationFactor(table, pressure, RESERVOIR_TEMPERATURE_C), `${pressure} bar`)
                .toBeCloseTo(1, 6);
        }
    });

    it('makes p/z equal to p for an ideal gas, and less than p for a real one', () => {
        const ideal = idealGasTable(RESERVOIR_TEMPERATURE_C);
        expect(pressureOverZ(ideal, 200, RESERVOIR_TEMPERATURE_C)).toBeCloseTo(200, 4);

        // A real gas below its z minimum has z < 1, so p/z exceeds p.
        const z = gasDeviationFactor(TABLE, 200, RESERVOIR_TEMPERATURE_C);
        expect(z).toBeGreaterThan(0.5);
        expect(z).toBeLessThan(1);
        expect(pressureOverZ(TABLE, 200, RESERVOIR_TEMPERATURE_C)).toBeGreaterThan(200);
    });

    it('round-trips p/z back to pressure', () => {
        for (const pressure of [40, 120, 260, 400]) {
            const pz = pressureOverZ(TABLE, pressure, RESERVOIR_TEMPERATURE_C);
            expect(pressureForPOverZ(pz, 400, TABLE, RESERVOIR_TEMPERATURE_C), `${pressure} bar`)
                .toBeCloseTo(pressure, 3);
        }
    });

    it('interpolates linearly in 1/B_g, not in B_g', () => {
        const table: GasPvtRow[] = [{ p_bar: 100, bg_m3m3: 0.01 }, { p_bar: 200, bg_m3m3: 0.005 }];
        // Midpoint: 1/Bg goes 100 -> 200, so the midpoint is 150 and Bg = 1/150.
        expect(gasFormationVolumeFactor(table, 150)).toBeCloseTo(1 / 150, 12);
        // Linear-in-Bg would have given 0.0075 = 1/133.3.
        expect(gasFormationVolumeFactor(table, 150)).not.toBeCloseTo(0.0075, 6);
    });
});

describe('the p/z straight line', () => {
    const GIIP = 1e8;
    const base = {
        pvtTable: TABLE,
        reservoirTemperature: RESERVOIR_TEMPERATURE_C,
        initialPressure: 400,
        giip: GIIP,
        initialWaterSaturation: 0.2,
        c_w: 0,
        c_f: 0,
    };

    it('is straight, and hits zero exactly at the gas initially in place', () => {
        const fractions = [0, 0.1, 0.25, 0.5, 0.75, 0.9, 1];
        const result = computeGasMaterialBalance({
            ...base,
            cumulativeGas: fractions.map((f) => f * GIIP),
        });

        expect(result.points[0].pOverZ).toBeCloseTo(result.initialPOverZ, 9);
        expect(result.points.at(-1)!.pOverZ).toBeCloseTo(0, 9);
        for (let index = 0; index < fractions.length; index += 1) {
            expect(result.points[index].pOverZ, `at ${fractions[index]} of GIIP`)
                .toBeCloseTo(result.initialPOverZ * (1 - fractions[index]), 9);
        }
    });

    it('leaves the corrected curve identical when there is nothing to correct', () => {
        const result = computeGasMaterialBalance({
            ...base,
            cumulativeGas: [0, 0.3 * GIIP, 0.7 * GIIP],
        });
        expect(result.effectiveCompressibility).toBe(0);
        for (const point of result.points) {
            expect(point.pOverZCompactionCorrected).toBeCloseTo(point.pOverZ, 9);
        }
    });

    it('holds the corrected curve above the straight line, by more as c_f grows', () => {
        // The extra energy from compaction and water expansion supports pressure,
        // so at the same cumulative production p/z is higher than the naive line.
        const cumulativeGas = [0, 0.25 * GIIP, 0.5 * GIIP, 0.75 * GIIP];
        const mild = computeGasMaterialBalance({ ...base, c_w: 4e-5, c_f: 5e-6, cumulativeGas });
        const strong = computeGasMaterialBalance({ ...base, c_w: 4e-5, c_f: 5e-4, cumulativeGas });

        expect(strong.effectiveCompressibility).toBeGreaterThan(mild.effectiveCompressibility);
        for (let index = 1; index < cumulativeGas.length; index += 1) {
            expect(mild.points[index].pOverZCompactionCorrected)
                .toBeGreaterThan(mild.points[index].pOverZ);
            expect(strong.points[index].pOverZCompactionCorrected)
                .toBeGreaterThan(mild.points[index].pOverZCompactionCorrected);
        }
    });

    it('computes c_e as the gas-volume-normalised sum of the two terms', () => {
        // Craft & Hawkins: c_e = (c_w S_wi + c_f) / (1 - S_wi).
        expect(effectiveCompressibility(4e-5, 5e-4, 0.2)).toBeCloseTo((4e-5 * 0.2 + 5e-4) / 0.8, 12);
        expect(effectiveCompressibility(4e-5, 0, 0)).toBeCloseTo(0, 12);
    });
});

describe('straight-line interpretation', () => {
    it('recovers the gas initially in place from a clean line', () => {
        const giip = 5e7;
        const initial = 300;
        const cumulativeGas = Array.from({ length: 20 }, (_, i) => (i / 19) * 0.8 * giip);
        const pOverZ = cumulativeGas.map((gp) => initial * (1 - gp / giip));

        const fit = fitStraightLineGiip(cumulativeGas, pOverZ, 1);
        expect(fit).not.toBeNull();
        expect(fit!.giip).toBeCloseTo(giip, 0);
        expect(fit!.initialPOverZ).toBeCloseTo(initial, 6);
        expect(fit!.slope).toBeLessThan(0);
    });

    it('over-estimates the gas in place when the plot is bending upward', () => {
        // A curve that sits above its own chord — the over-pressured signature.
        const giip = 5e7;
        const initial = 300;
        const cumulativeGas = Array.from({ length: 30 }, (_, i) => (i / 29) * 0.8 * giip);
        const pOverZ = cumulativeGas.map((gp) => {
            const x = gp / giip;
            return initial * (1 - x) * (1 + 0.25 * x);
        });

        const fit = fitStraightLineGiip(cumulativeGas, pOverZ, 1 / 3);
        expect(fit!.giip).toBeGreaterThan(giip);
    });

    it('refuses a record with no usable trend rather than inventing one', () => {
        expect(fitStraightLineGiip([], [])).toBeNull();
        expect(fitStraightLineGiip([1, 2, 3], [5, 5, 5])).toBeNull();
        // A rising p/z is not a depleting reservoir.
        expect(fitStraightLineGiip([1, 2, 3], [1, 2, 3])).toBeNull();
    });
});
