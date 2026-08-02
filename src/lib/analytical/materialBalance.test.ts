import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import { calculateMaterialBalance, evaluateBlackOilPvt, type MaterialBalanceParams } from './materialBalance';
import { generateBlackOilTable, DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR } from '../physics/pvt';

const CONSTANT_PVT_BASE = {
    initialPressure: 300,
    initialWaterSaturation: 0.2,
    initialGasSaturation: 0,
    porosity: 0.2,
    poreVolume: 20000, // 100m × 100m × 10m × 0.2
    c_w: 3e-6,
    c_rock: 1e-6,
    pvtMode: 'constant' as const,
    Bo_constant: 1.0,
    Bw_constant: 1.0,
    c_o: 1e-5,
    apiGravity: 30,
    gasSpecificGravity: 0.7,
    reservoirTemperature: 80,
    bubblePoint: 150,
};

describe('materialBalance', () => {

    it('returns volumetric OOIP consistent with pore volume and saturations', () => {
        const result = calculateMaterialBalance({
            ...CONSTANT_PVT_BASE,
            timeHistory: [0],
            pressureHistory: [300],
            cumulativeOilSC: [0],
            cumulativeGasSC: [0],
            cumulativeWaterSC: [0],
        });

        // OOIP = Vp × (1 - Swi) / Boi = 20000 × 0.8 / 1.0 = 16000
        expect(result.volumetricOoip).toBeCloseTo(16000, 6);
        expect(result.gasCapRatio).toBe(0);
    });

    it('computes correct MBE OOIP for constant-PVT single-phase depletion', () => {
        // Simulate a simple depletion: known OOIP = 16000 m³
        // With ct = So×co + Sw×cw + cf = 0.8×1e-5 + 0.2×3e-6 + 1e-6 = 9.6e-6
        // Efw = Boi × ct/(1-Swi) × ΔP = 1.0 × 9.6e-6/0.8 × ΔP = 1.2e-5 × ΔP
        // F = Np × Bo = Np × 1.0
        // N_mbe = F / Efw = Np / (1.2e-5 × ΔP)
        //
        // For consistency: Np should equal N × Efw = 16000 × 1.2e-5 × ΔP
        // At ΔP = 100 bar: Np = 16000 × 1.2e-5 × 100 = 19.2 m³
        const dP = 100;
        const ct = 0.8 * 1e-5 + 0.2 * 3e-6 + 1e-6; // 9.6e-6
        const Et = 1.0 * ct / 0.8 * dP; // 1.2e-5 × 100 = 1.2e-3
        // Split into the two mechanisms it is actually made of. The sum is the
        // invariant: this is the same total expansion the combined c_t form gave
        // before the attribution was corrected.
        const Eo = 1.0 * 1e-5 * dP;                          // oil expansion
        const Efw = 1.0 * (0.2 * 3e-6 + 1e-6) / 0.8 * dP;    // water + rock
        const N_vol = 16000;
        const Np = N_vol * Et; // 16000 × 1.2e-3 = 19.2

        const result = calculateMaterialBalance({
            ...CONSTANT_PVT_BASE,
            timeHistory: [0, 50],
            pressureHistory: [300, 200],
            cumulativeOilSC: [0, Np],
            cumulativeGasSC: [0, 0],
            cumulativeWaterSC: [0, 0],
        });

        // At t=0: no production, N_mbe = null (F=0, Et=0)
        expect(result.points[0].N_mbe).toBeNull();

        // At t=50: N_mbe should equal N_volumetric
        const pt = result.points[1];
        expect(pt.F).toBeCloseTo(Np, 9);
        expect(pt.Efw).toBeCloseTo(Efw, 12);
        expect(pt.Eo).toBeCloseTo(Eo, 12);
        expect(pt.Eg).toBe(0);
        expect(pt.Et).toBeCloseTo(Et, 12);
        expect(pt.N_mbe).toBeCloseTo(N_vol, 6);
    });

    it('attributes constant-PVT depletion energy to oil expansion, not to compaction', () => {
        // This case is an undersaturated oil reservoir: c_o = 1e-5/bar against
        // S_w c_w + c_f = 1.6e-6/bar, so roughly five sixths of the energy is
        // the oil expanding. The balance used to report it as 100 % compaction
        // because Eo was pinned at 0 and c_o folded into the compaction term —
        // the total was right, the label on the chart was not.
        const result = calculateMaterialBalance({
            ...CONSTANT_PVT_BASE,
            timeHistory: [0, 50],
            pressureHistory: [300, 200],
            cumulativeOilSC: [0, 10],
            cumulativeGasSC: [0, 0],
            cumulativeWaterSC: [0, 0],
        });

        const pt = result.points[1];
        // c_o / (c_o + (S_w c_w + c_f)/(1 - S_wi)) = 1e-5 / 1.2e-5
        expect(pt.driveIndex_oilExpansion).toBeCloseTo(1e-5 / 1.2e-5, 10);
        expect(pt.driveIndex_compaction).toBeCloseTo(0.2e-5 / 1.2e-5, 10);
        expect(pt.driveIndex_gasCap).toBe(0);
        expect(
            pt.driveIndex_oilExpansion + pt.driveIndex_compaction + pt.driveIndex_gasCap,
        ).toBeCloseTo(1.0, 10);
    });

    it('gas cap ratio m is correctly computed from initial saturations', () => {
        const result = calculateMaterialBalance({
            ...CONSTANT_PVT_BASE,
            initialGasSaturation: 0.1,
            // Soi = 1 - 0.2 - 0.1 = 0.7
            // m = Sgi / Soi = 0.1 / 0.7 ≈ 0.1429
            timeHistory: [0],
            pressureHistory: [300],
            cumulativeOilSC: [0],
            cumulativeGasSC: [0],
            cumulativeWaterSC: [0],
        });

        expect(result.gasCapRatio).toBeCloseTo(0.1 / 0.7, 10);
        // OOIP = Vp × Soi / Boi = 20000 × 0.7 = 14000
        expect(result.volumetricOoip).toBeCloseTo(14000, 6);
    });

    it('handles black-oil PVT with pressure-dependent Bo and Rs', () => {
        // Use black-oil PVT: API=30, gg=0.7, T=80°C, Pb=150 bar
        // At Pi=300 bar (above Pb): undersaturated, Rs = Rs_max, Bo < Bo_pb
        // At P=100 bar (below Pb): saturated, Rs < Rs_max, Bo < Bo_pb
        const result = calculateMaterialBalance({
            ...CONSTANT_PVT_BASE,
            pvtMode: 'black-oil',
            initialPressure: 200, // above Pb=150
            timeHistory: [0, 100],
            pressureHistory: [200, 100],
            cumulativeOilSC: [0, 500],
            cumulativeGasSC: [0, 50000],
            cumulativeWaterSC: [0, 0],
        });

        const pt = result.points[1];
        // Below bubble point: Eo should be > 0 (oil expansion + gas liberation)
        expect(pt.Eo).toBeGreaterThan(0);
        // Efw should be > 0 (pressure drop)
        expect(pt.Efw).toBeGreaterThan(0);
        // F should be > 0
        expect(pt.F).toBeGreaterThan(0);
        // N_mbe should be finite and positive
        expect(pt.N_mbe).toBeGreaterThan(0);
        // Drive indices should sum to ~1
        const sum = pt.driveIndex_oilExpansion + pt.driveIndex_gasCap + pt.driveIndex_compaction;
        expect(sum).toBeCloseTo(1.0, 10);
    });

    it('N_mbe converges to N_vol for a self-consistent depletion history', () => {
        // Create a synthetic depletion history where MBE should close perfectly.
        // For constant PVT: Np(t) = N × Efw(t) = N × Boi × ct/(1-Swi) × (Pi-P(t))
        const N_vol = 16000;
        const ct = 0.8 * 1e-5 + 0.2 * 3e-6 + 1e-6;
        const efwFactor = 1.0 * ct / 0.8; // 1.2e-5

        const times = [0, 10, 20, 50, 100];
        const pressures = [300, 280, 260, 220, 180]; // declining
        const cumOil = pressures.map((P) => {
            const dP = 300 - P;
            return N_vol * efwFactor * dP;
        });

        const result = calculateMaterialBalance({
            ...CONSTANT_PVT_BASE,
            timeHistory: times,
            pressureHistory: pressures,
            cumulativeOilSC: cumOil,
            cumulativeGasSC: [0, 0, 0, 0, 0],
            cumulativeWaterSC: [0, 0, 0, 0, 0],
        });

        // Skip t=0 (no production), all subsequent points should give N_mbe ≈ N_vol
        for (let i = 1; i < result.points.length; i++) {
            const ratio = (result.points[i].N_mbe ?? 0) / N_vol;
            expect(ratio).toBeCloseTo(1.0, 6);
        }
    });

    it('reads the run\'s own PVT table in preference to the correlations', () => {
        // The scenario's table is the fluid the simulator integrated. A balance
        // built from correlations grades the run against a different fluid — and
        // for `dep_pvt`, whose two variants differ *only* by their table, it
        // would return byte-identical diagnostics for both.
        const table = generateBlackOilTable(35, 0.75, 80, 150, 300, 20, 1e-4);
        const shared = {
            ...CONSTANT_PVT_BASE,
            pvtMode: 'black-oil' as const,
            initialPressure: 300,
            // Deliberately wrong correlation inputs: if they were consulted the
            // table result would move with them.
            apiGravity: 12,
            gasSpecificGravity: 0.55,
            bubblePoint: 40,
            timeHistory: [0, 100],
            pressureHistory: [300, 200],
            cumulativeOilSC: [0, 400],
            cumulativeGasSC: [0, 20000],
            cumulativeWaterSC: [0, 0],
        };

        const fromTable = calculateMaterialBalance({ ...shared, pvtTable: table });
        const fromCorrelation = calculateMaterialBalance(shared);
        expect(fromTable.points[1].Eo).not.toBeCloseTo(fromCorrelation.points[1].Eo, 6);

        // And it reads it the way the engine does: at a table pressure the
        // balance's Bo/Rs must be that row, not an interpolation of a different curve.
        const row = table.find((r) => r.p_bar > 150 && r.p_bar < 300)!;
        const atRow = calculateMaterialBalance({
            ...shared,
            pvtTable: table,
            initialPressure: row.p_bar,
            pressureHistory: [row.p_bar, row.p_bar],
        });
        // Eo at the initial pressure is identically zero, so probe Boi through N.
        const Boi = (CONSTANT_PVT_BASE.poreVolume * 0.8) / atRow.volumetricOoip;
        expect(Boi).toBeCloseTo(row.bo_m3m3, 9);
    });

    it('applies Dake\'s (1 + m) factor to the connate-water and pore term', () => {
        // Dake (1978) Eq. 3.20: the gas cap has connate water and pore volume
        // that expand too. Identical to the old formula at m = 0, which is why
        // no shipped scenario moves; it exists for the gas-cap case.
        const withCap = calculateMaterialBalance({
            ...CONSTANT_PVT_BASE,
            initialGasSaturation: 0.1, // Soi = 0.7, m = 1/7
            timeHistory: [0, 50],
            pressureHistory: [300, 200],
            cumulativeOilSC: [0, 10],
            cumulativeGasSC: [0, 0],
            cumulativeWaterSC: [0, 0],
        });
        const noCap = calculateMaterialBalance({
            ...CONSTANT_PVT_BASE,
            timeHistory: [0, 50],
            pressureHistory: [300, 200],
            cumulativeOilSC: [0, 10],
            cumulativeGasSC: [0, 0],
            cumulativeWaterSC: [0, 0],
        });

        expect(withCap.gasCapRatio).toBeCloseTo(1 / 7, 12);
        expect(withCap.points[1].Efw / noCap.points[1].Efw).toBeCloseTo(1 + 1 / 7, 12);
    });
});

describe('undersaturated Bo shrinkage stays consistent between pvt.ts and materialBalance.ts', () => {
    it('uses the same default compressibility constant in both undersaturated-Bo formulas', () => {
        // pvt.ts's generateBlackOilTable and materialBalance.ts's evaluateBlackOilPvt each
        // independently shrink Bo above the bubble point via Bo = Bo_pb * exp(-c_o * dP).
        // They must import the same DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR
        // constant rather than each carrying their own hardcoded `1e-5` literal, or the
        // live simulator's PVT table and the analytical material-balance reference would
        // silently diverge above the bubble point.
        const api = 35;
        const sgGas = 0.75;
        const tempC = 80;
        const bubblePoint = 150;
        const testPressure = 250; // above bubble point

        const table = generateBlackOilTable(api, sgGas, tempC, bubblePoint, testPressure, 10);
        const tableRowAtTestPressure = table[table.length - 1];
        expect(tableRowAtTestPressure.p_bar).toBeCloseTo(testPressure, 9);

        const params: MaterialBalanceParams = {
            initialPressure: bubblePoint,
            initialWaterSaturation: 0.2,
            initialGasSaturation: 0,
            porosity: 0.2,
            poreVolume: 20000,
            c_w: 3e-6,
            c_rock: 1e-6,
            pvtMode: 'black-oil',
            Bo_constant: 1.0,
            Bw_constant: 1.0,
            c_o: 1e-5,
            apiGravity: api,
            gasSpecificGravity: sgGas,
            reservoirTemperature: tempC,
            bubblePoint,
            pressureHistory: [],
            cumulativeOilSC: [],
            cumulativeGasSC: [],
            cumulativeWaterSC: [],
            timeHistory: [],
        };

        const analyticalBo = evaluateBlackOilPvt(params, testPressure).Bo;

        expect(analyticalBo).toBeCloseTo(tableRowAtTestPressure.bo_m3m3, 9);
        expect(DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR).toBe(1e-5);
    });

    it('keeps both undersaturated-Bo call sites on the shared constant, with no re-introduced literal', async () => {
        // Regression guard: the two files previously each carried their own `1e-5`. The numeric
        // agreement above would still hold the moment a literal is pasted back in, so also assert
        // the source shape: exactly one definition of the constant, and no bare literal next to a
        // `c_o` in either consumer.
        const [pvtSource, mbSource] = await Promise.all([
            readFile(new URL('../physics/pvt.ts', import.meta.url), 'utf8'),
            readFile(new URL('./materialBalance.ts', import.meta.url), 'utf8'),
        ]);

        const definitions = pvtSource.match(/export const DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR/g) ?? [];
        expect(definitions).toHaveLength(1);
        expect(mbSource).not.toMatch(/export const DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR/);
        expect(mbSource).toMatch(/DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR/);

        // `const c_o = 1e-5` / `c_o: 1e-5` style re-hardcoding in either file.
        const hardcoded = /\bc_o\b\s*[:=]\s*1(\.0+)?e-0?5/;
        expect(pvtSource).not.toMatch(hardcoded);
        expect(mbSource).not.toMatch(hardcoded);
    });

    it('matches the Rust engine default oil compressibility', async () => {
        // The engine's FluidProperties::default_pvt() and the frontend constant are independent
        // declarations of the same assumption (docs/BLACK_OIL_VALIDATION.md §3). If they drift,
        // an analytical material-balance overlay silently grades a simulation run against a
        // different undersaturated PVT than the simulator used.
        const engineSource = await readFile(new URL('../ressim/src/lib.rs', import.meta.url), 'utf8');
        const match = engineSource.match(/fn default_pvt\(\) -> Self \{[\s\S]*?c_o:\s*([0-9eE.+-]+)\s*,/);
        expect(match, 'FluidProperties::default_pvt() c_o not found in src/lib/ressim/src/lib.rs').not.toBeNull();
        expect(Number(match![1])).toBe(DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR);
    });
});
