import { describe, expect, it } from 'vitest';
import {
    PSI_PER_BAR,
    SCF_PER_BBL_TO_M3_PER_M3,
    cToF,
    cToR,
    findThermodynamicallyUnstableRanges,
    generateBlackOilTable,
    saturatedRsFraction,
    standingBubblePoint,
    standingRs,
    type SaturatedRsCorrelation,
} from './pvt';

describe('Standing (1947) solution GOR (#60)', () => {
    const tempF = cToF(80);

    it('is the inverse of standingBubblePoint', () => {
        for (const api of [22, 35, 45]) {
            for (const pPsia of [200, 1000, 2175.6, 3000, 4350]) {
                const rs = standingRs(pPsia, 0.75, api, tempF);
                expect(standingBubblePoint(rs, 0.75, api, tempF)).toBeCloseTo(pPsia, 6);
            }
        }
    });

    it('dissolves more gas in lighter oil and at lower temperature', () => {
        const at = (api: number, tF: number) => standingRs(2900, 0.75, api, tF);
        expect(at(45, tempF)).toBeGreaterThan(at(35, tempF));
        expect(at(35, tempF)).toBeGreaterThan(at(22, tempF));
        expect(at(35, cToF(60))).toBeGreaterThan(at(35, tempF));
    });

    /**
     * A generated table's bubble-point row must be the fluid the correlation says has that bubble
     * point. The sign error gave 20.0 m3/m3 here (112 scf/STB, whose own Standing bubble point is
     * 607 psia, not 2176).
     */
    it('generates a table whose bubble-point row has that bubble point', () => {
        const table = generateBlackOilTable(35, 0.75, 80, 150, 300, 20, 1e-4);
        const pb = table.find((row) => row.p_bar === 150)!;
        const rsScf = pb.rs_m3m3 / SCF_PER_BBL_TO_M3_PER_M3;
        expect(standingBubblePoint(rsScf, 0.75, 35, tempF) / PSI_PER_BAR).toBeCloseTo(150, 6);
        expect(pb.rs_m3m3).toBeCloseTo(93.04, 1);
    });
});

describe('saturated Rs(p) below one calibration point (#26)', () => {
    const tempF = cToF(80);
    const pbPsia = 150 * PSI_PER_BAR;
    const correlations: SaturatedRsCorrelation[] = ['petrosky-farshad', 'standing', 'al-marhoun'];

    /**
     * Each correlation in full, at 35 API, 0.75 gas gravity and 80 C, divided by its own value at
     * the bubble point. The gravity, API and temperature factors must cancel, which is the claim
     * `saturatedRsFraction` is built on.
     */
    it('is each published correlation normalised through the bubble point', () => {
        const sgOil = 141.5 / (131.5 + 35);
        const petroskyFarshad = (p: number) => {
            const x = 7.916e-4 * Math.pow(35, 1.5410) - 4.561e-5 * Math.pow(tempF, 1.3911);
            return Math.pow((p / 112.727 + 12.340) * Math.pow(0.75, 0.8439) * Math.pow(10, x), 1.73184);
        };
        const alMarhoun = (p: number) => Math.pow(
            p / (5.38088e-3 * Math.pow(0.75, -1.87784) * Math.pow(sgOil, 3.1437) * Math.pow(cToR(80), 1.32657)),
            1 / 0.715082,
        );
        const standing = (p: number) => standingRs(p, 0.75, 35, tempF);
        for (const pPsia of [300, 1000, 1600, 2000, pbPsia]) {
            expect(saturatedRsFraction('petrosky-farshad', pPsia, pbPsia)).toBeCloseTo(petroskyFarshad(pPsia) / petroskyFarshad(pbPsia), 12);
            expect(saturatedRsFraction('al-marhoun', pPsia, pbPsia)).toBeCloseTo(alMarhoun(pPsia) / alMarhoun(pbPsia), 12);
            expect(saturatedRsFraction('standing', pPsia, pbPsia)).toBeCloseTo(standing(pPsia) / standing(pbPsia), 12);
        }
    });

    /**
     * Fraction of Rs_b still dissolved at 110 bar. Petrosky–Farshad and Al-Marhoun as tabulated in
     * the #26 research; Standing is the 18-and-no-offset form #60 settled on, 0.688 where the
     * textbook (p/18.2 + 1.4) form gives 0.692.
     */
    it('orders the correlations by how long the oil holds its gas', () => {
        const at110 = (c: SaturatedRsCorrelation) => saturatedRsFraction(c, 110 * PSI_PER_BAR, pbPsia);
        expect(at110('petrosky-farshad')).toBeCloseTo(0.735, 3);
        expect(at110('standing')).toBeCloseTo(0.688, 3);
        expect(at110('al-marhoun')).toBeCloseTo(0.648, 3);
    });

    it('keeps the calibration point and the undersaturated branch, and reshapes only below it', () => {
        const [petrosky, standing, alMarhoun] = correlations.map((c) => generateBlackOilTable(35, 0.75, 80, 150, 300, 20, 1e-4, c));
        expect(standing).toEqual(generateBlackOilTable(35, 0.75, 80, 150, 300, 20, 1e-4));
        standing.forEach((row, i) => {
            if (row.p_bar >= 150) {
                expect(petrosky[i]).toEqual(row);
                expect(alMarhoun[i]).toEqual(row);
            } else {
                expect(petrosky[i].rs_m3m3).toBeGreaterThan(row.rs_m3m3);
                expect(alMarhoun[i].rs_m3m3).toBeLessThan(row.rs_m3m3);
                expect(petrosky[i].bg_m3m3).toBe(row.bg_m3m3);
            }
        });
        for (const table of [petrosky, standing, alMarhoun]) {
            expect(findThermodynamicallyUnstableRanges(table)).toEqual([]);
        }
    });
});
