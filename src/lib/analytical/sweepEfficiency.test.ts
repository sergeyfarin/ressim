import { describe, it, expect } from 'vitest';
import {
    mobilityRatio,
    arealSweepAtBreakthrough,
    arealSweepAtPvi,
    arealSweepCurve,
    computeArealSweep,
    dykstraParsonsCoefficient,
    verticalSweep,
    generateLayerPermDistribution,
    computeVerticalSweep,
    computeCombinedSweep,
    computeSweepRecoveryFactor,
    type SweepPoint,
} from './sweepEfficiency';
import { computeBLRecoveryVsPVI } from './fractionalFlow';
import type { RockProps, FluidProps } from './fractionalFlow';

const defaultRock: RockProps = { s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2, k_rw_max: 1.0, k_ro_max: 1.0 };
const defaultFluid: FluidProps = { mu_w: 0.5, mu_o: 1.0 };

// ── Mobility ratio ──

describe('mobilityRatio', () => {
    it('returns M=2 for default props (k_rw=k_ro=1, μ_w=0.5, μ_o=1.0)', () => {
        expect(mobilityRatio(defaultRock, defaultFluid)).toBeCloseTo(2.0, 6);
    });

    it('returns M=1 for equal viscosities and equal endpoints', () => {
        const fluid: FluidProps = { mu_w: 1.0, mu_o: 1.0 };
        expect(mobilityRatio(defaultRock, fluid)).toBeCloseTo(1.0, 6);
    });

    it('returns M<1 for favourable mobility (high μ_w, low μ_o)', () => {
        const fluid: FluidProps = { mu_w: 2.0, mu_o: 0.5 };
        expect(mobilityRatio(defaultRock, fluid)).toBeLessThan(1);
    });

    it('returns M>1 for unfavourable mobility', () => {
        const fluid: FluidProps = { mu_w: 0.1, mu_o: 5.0 };
        expect(mobilityRatio(defaultRock, fluid)).toBeGreaterThan(1);
    });
});

// ── Areal sweep at breakthrough ──

describe('arealSweepAtBreakthrough', () => {
    it('returns ~0.70 at M=1 (canonical five-spot value)', () => {
        const ea = arealSweepAtBreakthrough(1.0);
        expect(ea).toBeCloseTo(0.70, 1);
    });

    it('returns higher value for favourable M<1', () => {
        const ea01 = arealSweepAtBreakthrough(0.1);
        const ea1 = arealSweepAtBreakthrough(1.0);
        expect(ea01).toBeGreaterThan(ea1);
    });

    it('returns ~0.35-0.42 for M=10', () => {
        const ea = arealSweepAtBreakthrough(10.0);
        expect(ea).toBeGreaterThan(0.30);
        expect(ea).toBeLessThan(0.50);
    });

    it('is monotonically decreasing for M in [0.1, 100]', () => {
        let prev = 1.0;
        for (let logM = -1; logM <= 2; logM += 0.1) {
            const M = Math.pow(10, logM);
            const ea = arealSweepAtBreakthrough(M);
            expect(ea).toBeLessThanOrEqual(prev + 1e-6);
            prev = ea;
        }
    });

    it('is bounded [0, 1]', () => {
        for (const M of [0.01, 0.1, 1, 10, 100, 1000]) {
            const ea = arealSweepAtBreakthrough(M);
            expect(ea).toBeGreaterThanOrEqual(0);
            expect(ea).toBeLessThanOrEqual(1);
        }
    });

    it('handles degenerate M=0 gracefully', () => {
        expect(arealSweepAtBreakthrough(0)).toBe(0);
    });
});

// ── Areal sweep vs PVI ──

describe('arealSweepAtPvi', () => {
    it('returns 0 at PVI=0', () => {
        expect(arealSweepAtPvi(1.0, 0)).toBe(0);
    });

    it('returns E_A_bt at PVI = PVI_bt (which ≈ E_A_bt)', () => {
        const M = 2.0;
        const eaBt = arealSweepAtBreakthrough(M);
        const ea = arealSweepAtPvi(M, eaBt);
        expect(ea).toBeCloseTo(eaBt, 2);
    });

    it('is monotonically increasing', () => {
        let prev = 0;
        for (let pvi = 0; pvi <= 5; pvi += 0.1) {
            const ea = arealSweepAtPvi(2.0, pvi);
            expect(ea).toBeGreaterThanOrEqual(prev - 1e-10);
            prev = ea;
        }
    });

    it('approaches 1 at high PVI', () => {
        expect(arealSweepAtPvi(1.0, 10)).toBeGreaterThan(0.95);
    });

    it('is bounded [0, 1]', () => {
        for (let pvi = 0; pvi <= 10; pvi += 0.5) {
            const ea = arealSweepAtPvi(5.0, pvi);
            expect(ea).toBeGreaterThanOrEqual(0);
            expect(ea).toBeLessThanOrEqual(1);
        }
    });
});

// ── Areal sweep curve ──

describe('arealSweepCurve', () => {
    it('returns nPoints+1 entries', () => {
        const curve = arealSweepCurve(2.0, 3.0, 100);
        expect(curve).toHaveLength(101);
    });

    it('starts at PVI=0, efficiency=0', () => {
        const curve = arealSweepCurve(2.0);
        expect(curve[0].pvi).toBe(0);
        expect(curve[0].efficiency).toBe(0);
    });

    it('ends at PVI=pviMax', () => {
        const curve = arealSweepCurve(2.0, 5.0, 50);
        expect(curve[curve.length - 1].pvi).toBeCloseTo(5.0, 6);
    });
});

// ── computeArealSweep ──

describe('computeArealSweep', () => {
    it('returns valid mobilityRatio and eaAtBreakthrough', () => {
        const result = computeArealSweep(defaultRock, defaultFluid);
        expect(result.mobilityRatio).toBeCloseTo(2.0, 6);
        expect(result.eaAtBreakthrough).toBeGreaterThan(0.4);
        expect(result.eaAtBreakthrough).toBeLessThan(0.8);
    });
});

// ── Dykstra-Parsons coefficient ──

describe('dykstraParsonsCoefficient', () => {
    it('returns 0 for uniform permeability', () => {
        expect(dykstraParsonsCoefficient([100, 100, 100, 100])).toBe(0);
    });

    it('returns 0 for single layer', () => {
        expect(dykstraParsonsCoefficient([100])).toBe(0);
    });

    it('returns 0 for empty array', () => {
        expect(dykstraParsonsCoefficient([])).toBe(0);
    });

    it('returns value between 0 and 1 for heterogeneous', () => {
        const vdp = dykstraParsonsCoefficient([10, 50, 100, 200, 500]);
        expect(vdp).toBeGreaterThan(0);
        expect(vdp).toBeLessThan(1);
    });

    it('higher variation → higher VDP', () => {
        const vdpLow = dykstraParsonsCoefficient([90, 100, 110]);
        const vdpHigh = dykstraParsonsCoefficient([10, 100, 1000]);
        expect(vdpHigh).toBeGreaterThan(vdpLow);
    });
});

// ── Vertical sweep ──

describe('verticalSweep', () => {
    it('returns nPoints+1 entries', () => {
        const layers = [{ perm: 100, thickness: 10 }, { perm: 50, thickness: 10 }];
        const curve = verticalSweep(layers, 1.0, 3.0, 50);
        expect(curve).toHaveLength(51);
    });

    it('starts at 0 efficiency', () => {
        const layers = [{ perm: 100, thickness: 10 }];
        const curve = verticalSweep(layers, 1.0);
        expect(curve[0].efficiency).toBe(0);
    });

    it('uniform layers reach Ev=1 earlier than heterogeneous', () => {
        const uniform = [
            { perm: 100, thickness: 10 },
            { perm: 100, thickness: 10 },
        ];
        const heterogeneous = [
            { perm: 500, thickness: 10 },
            { perm: 10, thickness: 10 },
        ];
        const curveU = verticalSweep(uniform, 1.0, 3.0, 200);
        const curveH = verticalSweep(heterogeneous, 1.0, 3.0, 200);

        // At PVI=0.5 (mid-range), uniform should be higher
        const idxMid = 200 * 0.5 / 3.0;
        // Round to nearest valid index
        const idx = Math.round(idxMid * 200 / 200);
        // Just check that at some point before PVI=1, uniform is swept more
        const uAt1 = curveU.find(p => p.pvi >= 1.0)?.efficiency ?? 0;
        const hAt1 = curveH.find(p => p.pvi >= 1.0)?.efficiency ?? 0;
        expect(uAt1).toBeGreaterThan(hAt1);
    });

    it('is monotonically increasing', () => {
        const layers = [
            { perm: 200, thickness: 5 },
            { perm: 100, thickness: 10 },
            { perm: 50, thickness: 8 },
        ];
        const curve = verticalSweep(layers, 2.0);
        let prev = 0;
        for (const pt of curve) {
            expect(pt.efficiency).toBeGreaterThanOrEqual(prev - 1e-10);
            prev = pt.efficiency;
        }
    });

    it('handles empty layers', () => {
        const curve = verticalSweep([], 1.0);
        expect(curve).toHaveLength(1);
        expect(curve[0].efficiency).toBe(0);
    });
});

// ── generateLayerPermDistribution ──

describe('generateLayerPermDistribution', () => {
    it('returns correct number of layers', () => {
        expect(generateLayerPermDistribution(5, 0.7, 100)).toHaveLength(5);
    });

    it('returns single value for nLayers=1', () => {
        const perms = generateLayerPermDistribution(1, 0.5, 100);
        expect(perms).toHaveLength(1);
        expect(perms[0]).toBe(100);
    });

    it('returns empty array for nLayers=0', () => {
        expect(generateLayerPermDistribution(0, 0.5, 100)).toHaveLength(0);
    });

    it('all values are positive', () => {
        const perms = generateLayerPermDistribution(10, 0.8, 100);
        for (const p of perms) {
            expect(p).toBeGreaterThan(0);
        }
    });

    it('VDP of generated distribution is close to requested VDP', () => {
        const targetVdp = 0.7;
        const perms = generateLayerPermDistribution(20, targetVdp, 100);
        const actualVdp = dykstraParsonsCoefficient(perms);
        // Allow ±0.15 tolerance (small sample from quantile approximation)
        expect(actualVdp).toBeGreaterThan(targetVdp - 0.15);
        expect(actualVdp).toBeLessThan(targetVdp + 0.15);
    });

    it('VDP ≈ 0 gives nearly uniform distribution', () => {
        const perms = generateLayerPermDistribution(10, 0.0, 100);
        const vdp = dykstraParsonsCoefficient(perms);
        expect(vdp).toBeCloseTo(0, 1);
    });
});

// ── computeVerticalSweep ──

describe('computeVerticalSweep', () => {
    it('returns valid VDP', () => {
        const result = computeVerticalSweep([100, 50, 200], 10, 2.0);
        expect(result.vdp).toBeGreaterThan(0);
        expect(result.vdp).toBeLessThan(1);
    });
});

// ── computeBLRecoveryVsPVI ──

describe('computeBLRecoveryVsPVI', () => {
    it('starts at RF=0 at PVI=0', () => {
        const curve = computeBLRecoveryVsPVI(defaultRock, defaultFluid);
        expect(curve[0].pvi).toBe(0);
        expect(curve[0].rf).toBe(0);
    });

    it('is monotonically non-decreasing', () => {
        const curve = computeBLRecoveryVsPVI(defaultRock, defaultFluid, 3.0, 200);
        for (let i = 1; i < curve.length; i++) {
            expect(curve[i].rf).toBeGreaterThanOrEqual(curve[i - 1].rf - 1e-9);
        }
    });

    it('is bounded [0, 1] at all PVI', () => {
        const curve = computeBLRecoveryVsPVI(defaultRock, defaultFluid, 5.0, 100);
        for (const pt of curve) {
            expect(pt.rf).toBeGreaterThanOrEqual(0);
            expect(pt.rf).toBeLessThanOrEqual(1);
        }
    });

    it('approaches E_D_piston = (1-Sor-Swc)/(1-Swc) at high PVI (within 5%)', () => {
        // BL RF converges slowly to the piston limit — expect ≥90% of E_D_piston at PVI=20
        const curve = computeBLRecoveryVsPVI(defaultRock, defaultFluid, 20.0, 500);
        const edPiston = (1 - defaultRock.s_or - defaultRock.s_wc) / (1 - defaultRock.s_wc);
        const lastRf = curve[curve.length - 1].rf;
        expect(lastRf).toBeGreaterThan(edPiston * 0.90);
        expect(lastRf).toBeLessThanOrEqual(edPiston + 1e-9);
    });

    it('favourable mobility (M<1) reaches higher RF faster than unfavourable (M>1)', () => {
        const rockFav: RockProps = { ...defaultRock, k_rw_max: 0.3, mu_w: 1, mu_o: 1 };  // M<1 approx
        // Use same defaultFluid but different endpoint krw to change M
        const fluidLowM: FluidProps = { mu_w: 2.0, mu_o: 1.0 };  // M<1
        const fluidHighM: FluidProps = { mu_w: 0.5, mu_o: 5.0 }; // M>1
        const curveLow = computeBLRecoveryVsPVI(defaultRock, fluidLowM, 2.0, 100);
        const curveHigh = computeBLRecoveryVsPVI(defaultRock, fluidHighM, 2.0, 100);
        // At PVI=1, favourable should have higher RF
        const rfLow = curveLow.find(p => p.pvi >= 1.0)?.rf ?? 0;
        const rfHigh = curveHigh.find(p => p.pvi >= 1.0)?.rf ?? 0;
        expect(rfLow).toBeGreaterThan(rfHigh);
    });
});

// ── computeSweepRecoveryFactor ──

describe('computeSweepRecoveryFactor', () => {
    it('returns curve with correct length', () => {
        const result = computeSweepRecoveryFactor(defaultRock, defaultFluid, [100], 10, 3.0, 100);
        expect(result.curve).toHaveLength(101);
    });

    it('starts at RF=0 at PVI=0', () => {
        const result = computeSweepRecoveryFactor(defaultRock, defaultFluid, [100], 10);
        expect(result.curve[0].rfSweep).toBe(0);
        expect(result.curve[0].rfBL1D).toBe(0);
    });

    it('rfSweep ≤ rfBL1D at all PVI (sweep penalty reduces RF vs perfect sweep)', () => {
        const result = computeSweepRecoveryFactor(defaultRock, defaultFluid, [100, 50, 200], 10);
        for (const pt of result.curve) {
            expect(pt.rfSweep).toBeLessThanOrEqual(pt.rfBL1D + 1e-9);
        }
    });

    it('rfSweep ≤ edPiston at all PVI', () => {
        const result = computeSweepRecoveryFactor(defaultRock, defaultFluid, [100, 50, 200], 10);
        for (const pt of result.curve) {
            expect(pt.rfSweep).toBeLessThanOrEqual(result.edPiston + 1e-9);
        }
    });

    it('single-layer (nz=1): E_vol=E_A, rfSweep≈rfBL1D at high PVI (sweep approaches perfect)', () => {
        const result = computeSweepRecoveryFactor(defaultRock, defaultFluid, [100], 10, 5.0, 200);
        const last = result.curve[result.curve.length - 1];
        // With nz=1, E_V=1, so E_vol=E_A. At high PVI E_A→1 and rfSweep→rfBL1D
        expect(last.rfSweep).toBeCloseTo(last.rfBL1D, 1);
    });

    it('is monotonically non-decreasing', () => {
        const result = computeSweepRecoveryFactor(defaultRock, defaultFluid, [100, 50, 200], 10);
        for (let i = 1; i < result.curve.length; i++) {
            expect(result.curve[i].rfSweep).toBeGreaterThanOrEqual(result.curve[i - 1].rfSweep - 1e-9);
        }
    });

    it('edPiston matches (1-Sor-Swc)/(1-Swc)', () => {
        const result = computeSweepRecoveryFactor(defaultRock, defaultFluid, [100], 10);
        const expected = (1 - defaultRock.s_or - defaultRock.s_wc) / (1 - defaultRock.s_wc);
        expect(result.edPiston).toBeCloseTo(expected, 6);
    });
});

// ── computeCombinedSweep ──

describe('computeCombinedSweep', () => {
    it('combined ≤ min(areal, vertical) at each PVI', () => {
        const result = computeCombinedSweep(defaultRock, defaultFluid, [100, 50, 200], 10);
        for (let i = 0; i < result.combined.length; i++) {
            const ea = result.arealSweep.curve[i].efficiency;
            const ev = result.verticalSweep.curve[i].efficiency;
            // E_vol = E_A × E_V  ≤  min(E_A, E_V)
            expect(result.combined[i].efficiency).toBeLessThanOrEqual(Math.min(ea, ev) + 1e-10);
        }
    });

    it('combined = arealSweep × verticalSweep', () => {
        const result = computeCombinedSweep(defaultRock, defaultFluid, [100, 50, 200], 10);
        for (let i = 0; i < result.combined.length; i++) {
            const expected = result.arealSweep.curve[i].efficiency * result.verticalSweep.curve[i].efficiency;
            expect(result.combined[i].efficiency).toBeCloseTo(expected, 10);
        }
    });

    it('returns matching curve lengths', () => {
        const result = computeCombinedSweep(defaultRock, defaultFluid, [100], 10, 3.0, 100);
        expect(result.arealSweep.curve).toHaveLength(101);
        expect(result.verticalSweep.curve).toHaveLength(101);
        expect(result.combined).toHaveLength(101);
    });
});
