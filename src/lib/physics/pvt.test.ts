import { describe, expect, it } from 'vitest';
import { PSI_PER_BAR, SCF_PER_BBL_TO_M3_PER_M3, cToF, generateBlackOilTable, standingBubblePoint, standingRs } from './pvt';

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
