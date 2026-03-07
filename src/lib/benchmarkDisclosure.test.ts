import { describe, expect, it } from 'vitest';
import {
    buildBenchmarkCaseSnapshot,
    buildBenchmarkPrimaryVariation,
    buildBenchmarkReferenceGuidance,
    buildBenchmarkVariantDeltaSummary,
    deriveBenchmarkParamsDelta,
} from './benchmarkDisclosure';

describe('benchmarkDisclosure', () => {
    it('builds a compact base-case snapshot for run and output disclosures', () => {
        const snapshot = buildBenchmarkCaseSnapshot({
            nx: 96,
            ny: 1,
            nz: 1,
            cellDx: 10,
            cellDy: 10,
            cellDz: 10,
            steps: 240,
            delta_t_days: 0.5,
            injectorEnabled: true,
            injectorControlMode: 'rate',
            targetInjectorRate: 350,
            producerControlMode: 'pressure',
            producerBhp: 100,
            reservoirPorosity: 0.2,
            permMode: 'uniform',
            uniformPermX: 2000,
            uniformPermY: 2000,
            uniformPermZ: 2000,
            mu_w: 0.5,
            mu_o: 1,
        });

        expect(snapshot.grid).toBe('96 x 1 x 1 cells · 10 x 10 x 10 m');
        expect(snapshot.horizon).toBe('240 steps × 0.5 d = 120 d');
        expect(snapshot.controls).toContain('Injector rate 350 m3/d');
        expect(snapshot.controls).toContain('Producer BHP 100 bar');
        expect(snapshot.reservoir).toContain('φ 0.2');
        expect(snapshot.reservoir).toContain('uniform 2000 mD');
    });

    it('builds reference guidance for comparison-driven families', () => {
        const guidance = buildBenchmarkReferenceGuidance({
            scenarioClass: 'buckley-leverett',
            referenceKind: 'analytical',
            comparisonMetric: {
                kind: 'breakthrough-pv-relative-error',
                target: 'analytical-reference',
                tolerance: 0.25,
            },
            displayDefaults: {
                xAxis: 'pvi',
                panels: ['watercut-breakthrough', 'recovery', 'pressure'],
            },
            runPolicy: 'compare-to-reference',
        });

        expect(guidance.reference).toContain('Buckley-Leverett reference solution');
        expect(guidance.metric).toContain('arrival-PVI relative error');
        expect(guidance.metric).toContain('25% tolerance');
        expect(guidance.outputs).toContain('pore volume injected x-axis');
        expect(guidance.outputs).toContain('breakthrough, recovery, pressure');
        expect(guidance.runApproach).toContain('Reference review run');
    });

    it('derives and summarizes meaningful variant deltas', () => {
        const delta = deriveBenchmarkParamsDelta(
            {
                nx: 96,
                producerI: 95,
                delta_t_days: 0.5,
                steps: 240,
                permMode: 'uniform',
            },
            {
                nx: 48,
                producerI: 47,
                delta_t_days: 0.25,
                steps: 480,
                permMode: 'random',
                minPerm: 1500,
                maxPerm: 2500,
                randomSeed: 4201,
            },
        );

        const summary = buildBenchmarkVariantDeltaSummary(delta);
        expect(summary).toContain('Grid nx=48');
        expect(summary).toContain('Timestep 0.25 d · 480 steps');
        expect(summary).toContain('Permeability 1500-2500 mD · seed 4201');
    });

    it('picks one primary varied input for compact result tables', () => {
        expect(buildBenchmarkPrimaryVariation({})).toEqual({
            label: 'Base case',
            value: 'Locked reference case',
        });

        expect(buildBenchmarkPrimaryVariation({ delta_t_days: 0.25, steps: 480 })).toEqual({
            label: 'dt',
            value: '0.25 d',
        });

        expect(buildBenchmarkPrimaryVariation({ nx: 48, producerI: 47 })).toEqual({
            label: 'Grid',
            value: 'nx 48',
        });

        expect(buildBenchmarkPrimaryVariation({ permMode: 'random', minPerm: 1500, maxPerm: 2500 })).toEqual({
            label: 'k range',
            value: '1500-2500 mD',
        });
    });
});