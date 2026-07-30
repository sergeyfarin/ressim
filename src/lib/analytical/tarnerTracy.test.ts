import { describe, expect, it } from 'vitest';
import { generateBlackOilTable } from '../physics/pvt';
import { calculateTarnerTracy } from './tarnerTracy';

const rock = {
    s_wc: 0.2, s_or: 0.15, s_gc: 0.05, s_gr: 0.05, s_org: 0.2,
    n_o: 2, n_g: 1.5, k_ro_max: 1, k_rg_max: 0.8,
};

describe('Tarner–Tracy solution-gas-drive model', () => {
    it('predicts monotonic recovery and finite producing GOR during depletion', () => {
        const points = calculateTarnerTracy({
            pressureBar: [200, 180, 150, 120, 100],
            pvtTable: generateBlackOilTable(35, 0.75, 80, 200, 300, 20),
            initialPressureBar: 200,
            poreVolumeM3: 100_000,
            initialWaterSaturation: 0.2,
            initialGasSaturation: 0.08,
            rock,
        });

        expect(points).toHaveLength(5);
        expect(points.every((point) => Number.isFinite(point.producingGorM3M3))).toBe(true);
        expect(points.every((point, index) => index === 0 || point.recoveryFactor >= points[index - 1].recoveryFactor)).toBe(true);
        expect(points.at(-1)!.recoveryFactor).toBeGreaterThan(0);
        expect(points.at(-1)!.gasSaturation).toBeGreaterThan(points[0].gasSaturation);
    });

    it('responds to initial free-gas volume and black-oil PVT', () => {
        const run = (initialGasSaturation: number, api: number) => calculateTarnerTracy({
            pressureBar: [200, 150, 100],
            pvtTable: generateBlackOilTable(api, 0.75, 80, 200, 300, 20),
            initialPressureBar: 200,
            poreVolumeM3: 100_000,
            initialWaterSaturation: 0.2,
            initialGasSaturation,
            rock,
        });
        expect(run(0.05, 35).at(-1)!.producingGorM3M3).not.toBeCloseTo(run(0.15, 35).at(-1)!.producingGorM3M3, 6);
        expect(run(0.08, 22).at(-1)!.recoveryFactor).not.toBeCloseTo(run(0.08, 45).at(-1)!.recoveryFactor, 6);
    });
});
