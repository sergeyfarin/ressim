import { describe, expect, it } from 'vitest';
import { calculateMaterialBalance } from './materialBalance';

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
        const Efw = 1.0 * ct / 0.8 * dP; // 1.2e-5 × 100 = 1.2e-3
        const N_vol = 16000;
        const Np = N_vol * Efw; // 16000 × 1.2e-3 = 19.2

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
        expect(pt.Eo).toBe(0);
        expect(pt.Eg).toBe(0);
        expect(pt.Et).toBeCloseTo(Efw, 12);
        expect(pt.N_mbe).toBeCloseTo(N_vol, 6);
    });

    it('drive indices sum to 1 for constant PVT (100% compaction drive)', () => {
        const result = calculateMaterialBalance({
            ...CONSTANT_PVT_BASE,
            timeHistory: [0, 50],
            pressureHistory: [300, 200],
            cumulativeOilSC: [0, 10],
            cumulativeGasSC: [0, 0],
            cumulativeWaterSC: [0, 0],
        });

        const pt = result.points[1];
        expect(pt.driveIndex_compaction).toBeCloseTo(1.0, 10);
        expect(pt.driveIndex_oilExpansion).toBe(0);
        expect(pt.driveIndex_gasCap).toBe(0);
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
});
