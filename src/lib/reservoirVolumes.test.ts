import { describe, expect, it } from 'vitest';
import {
    getPoreVolume,
    getInitialSaturations,
    getInitialPvt,
    getStockTankOilInPlace,
    getGasInPlace,
    getDisplacementOilInPlace,
} from './reservoirVolumes';
import { generateBlackOilTable } from './physics/pvt';

const GRID = {
    nx: 10, ny: 1, nz: 2,
    cellDx: 10, cellDy: 10, cellDz: 5,
    reservoirPorosity: 0.2,
};
// 10 x 1 x 2 cells of 10 x 10 x 5 m at phi = 0.2
const PORE_VOLUME = 10 * 1 * 10 * 10 * (5 + 5) * 0.2;

describe('in-place volumes', () => {
    it('computes pore volume from the grid and porosity', () => {
        expect(getPoreVolume(GRID)).toBeCloseTo(PORE_VOLUME, 9);
    });

    it('splits initial saturations, with oil as the remainder', () => {
        const s = getInitialSaturations({ ...GRID, initialSaturation: 0.2, initialGasSaturation: 0.1 });
        expect(s.water).toBeCloseTo(0.2, 12);
        expect(s.gas).toBeCloseTo(0.1, 12);
        expect(s.oil).toBeCloseTo(0.7, 12);
    });

    it('weights per-layer saturations by layer thickness', () => {
        // A gas cap in the top layer only. The scalar would report no gas cap at
        // all, which is the geometry `dep_gas_cap` is built to vary — and the
        // material balance's m comes from this number.
        const s = getInitialSaturations({
            ...GRID,
            cellDzPerLayer: [15, 5],
            initialGasSaturation: 0,
            initialGasSaturationPerLayer: [0.6, 0],
            initialSaturation: 0.2,
        });
        expect(s.gas).toBeCloseTo(0.6 * 15 / 20, 12);
        expect(s.oil).toBeCloseTo(1 - 0.2 - 0.45, 12);
    });

    it('is a surface volume: STOIIP divides the reservoir volume by B_oi', () => {
        const constantPvt = { ...GRID, initialSaturation: 0.2, volume_expansion_o: 1.25 };
        expect(getStockTankOilInPlace(constantPvt)).toBeCloseTo(PORE_VOLUME * 0.8 / 1.25, 9);
    });

    it('takes B_oi from the run\'s own PVT table for a black-oil case', () => {
        const pvtTable = generateBlackOilTable(35, 0.75, 80, 200, 300, 20, 1e-5);
        const params = {
            ...GRID, pvtMode: 'black-oil', pvtTable, c_o: 1e-5,
            initialPressure: 200, initialSaturation: 0.2, initialGasSaturation: 0.08,
            // Deliberately inconsistent scalar: the table must win.
            volume_expansion_o: 1.0,
        };
        const { Boi } = getInitialPvt(params);
        expect(Boi).toBeGreaterThan(1.05);
        expect(getStockTankOilInPlace(params)).toBeCloseTo(PORE_VOLUME * 0.72 / Boi, 6);
    });

    it('reports no oil in place for a dry-gas case rather than zero or a wrong denominator', () => {
        const pvtTable = generateBlackOilTable(35, 0.65, 90, 20, 450, 40, 1e-4);
        const dryGas = {
            ...GRID, pvtMode: 'black-oil', pvtTable,
            initialPressure: 400, initialSaturation: 0.2, initialGasSaturation: 0.8,
        };
        expect(getStockTankOilInPlace(dryGas)).toBeNull();
        // ...and all of its hydrocarbon is gas, free with nothing dissolved.
        const { Bgi } = getInitialPvt(dryGas);
        expect(getGasInPlace(dryGas)).toBeCloseTo(PORE_VOLUME * 0.8 / (Bgi as number), 6);
    });

    it('counts dissolved gas in GIIP, not just free gas', () => {
        const pvtTable = generateBlackOilTable(35, 0.75, 80, 200, 300, 20, 1e-5);
        const params = {
            ...GRID, pvtMode: 'black-oil', pvtTable, c_o: 1e-5,
            initialPressure: 200, initialSaturation: 0.2, initialGasSaturation: 0.08,
        };
        const { Boi, Bgi, Rsi } = getInitialPvt(params);
        expect(Rsi).toBeGreaterThan(0);
        const free = PORE_VOLUME * 0.08 / (Bgi as number);
        const dissolved = PORE_VOLUME * 0.72 / Boi * Rsi;
        expect(getGasInPlace(params)).toBeCloseTo(free + dissolved, 6);
    });

    it('has no gas in place without a gas PVT table', () => {
        expect(getGasInPlace({ ...GRID, initialSaturation: 0.2 })).toBeNull();
    });

    it('keeps the displacement denominator a reservoir volume', () => {
        // Buckley-Leverett and the sweep correlations produce displaced
        // reservoir volume, so their denominator must not carry 1/B_oi.
        const params = { ...GRID, initialSaturation: 0.2, volume_expansion_o: 1.25 };
        expect(getDisplacementOilInPlace(params)).toBeCloseTo(PORE_VOLUME * 0.8, 9);
    });

    it('agrees with the displacement denominator for every B_o = 1 case', () => {
        // Which is every displacement scenario in the catalog — the guarantee
        // that this split changes no waterflood or sweep comparison.
        const params = { ...GRID, initialSaturation: 0.2, volume_expansion_o: 1 };
        expect(getStockTankOilInPlace(params)).toBeCloseTo(getDisplacementOilInPlace(params), 9);
    });
});
