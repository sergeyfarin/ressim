import { describe, expect, it } from 'vitest';
import { getBenchmarkFamily } from '../catalog/benchmarkCases';
import { getReferenceChartLayoutConfig } from './referenceChartConfig';

describe('referenceChartConfig', () => {
    it.skip('builds breakthrough-centric BL chart defaults with PVI x-axis' + ' [ARCHIVED FIXTURE — see .archive/README.md, TODO.md]', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const config = getReferenceChartLayoutConfig({
            family,
            referencePolicy: {
                analyticalMethod: 'buckley-leverett',
                referenceKind: 'analytical',
                referenceSource: 'buckley-leverett-shock-reference',
                referenceLabel: 'Buckley-Leverett reference solution',
                primaryTruthLabel: 'Reference arrival-PVI comparison',
                analyticalOverlayRole: 'primary',
                summary: 'The Buckley-Leverett reference solution is the primary review baseline for this run.',
            },
        });

        expect(config).toMatchObject({
            chart: {
                xAxisMode: 'pvi',
                panelOrder: ['rates', 'cumulative', 'diagnostics'],
                panels: {
                    rates: {
                        title: 'Breakthrough',
                        scalePreset: 'breakthrough',
                        expanded: true,
                    },
                    cumulative: {
                        title: 'Recovery',
                        expanded: true,
                    },
                    diagnostics: {
                        title: 'Pressure',
                        expanded: false,
                    },
                },
            },
        });
        expect(config.chart?.panels?.rates?.curveKeys).toEqual(['water-cut-sim', 'water-cut-reference', 'avg-water-sat']);
    });

    it.skip('drops the reference-solution BL overlay from primary panel defaults when numerical reference is primary' + ' [ARCHIVED FIXTURE — see .archive/README.md, TODO.md]', () => {
        const family = getBenchmarkFamily('bl_case_b_refined');
        const config = getReferenceChartLayoutConfig({
            family,
            referencePolicy: {
                analyticalMethod: 'buckley-leverett',
                referenceKind: 'numerical-refined',
                referenceSource: 'bl_case_b_refined:refined-numerical-reference',
                referenceLabel: 'Refined numerical reference',
                primaryTruthLabel: 'Refined numerical arrival-PVI comparison',
                analyticalOverlayRole: 'secondary',
                summary: 'A refined numerical reference is the primary review baseline; the reference-solution overlay is contextual rather than an equality target.',
            },
        });

        expect(config.chart?.panels?.rates?.curveKeys).toEqual(['water-cut-sim', 'avg-water-sat']);
        expect(config.chart?.panels?.cumulative?.curveKeys).toEqual(['recovery-factor', 'cum-oil-sim', 'cum-injection']);
    });

    it.skip('builds depletion-focused chart defaults and log-time Fetkovich preference' + ' [ARCHIVED FIXTURE — see .archive/README.md, TODO.md]', () => {
        const dietz = getBenchmarkFamily('dietz_sq_center');
        const fetkovich = getBenchmarkFamily('fetkovich_exp');
        const dietzConfig = getReferenceChartLayoutConfig({ family: dietz });
        const fetkovichConfig = getReferenceChartLayoutConfig({ family: fetkovich });

        expect(dietzConfig).toMatchObject({
            chart: {
                xAxisMode: 'time',
                panels: {
                    rates: { title: 'Oil Rate' },
                    cumulative: { title: 'Cumulative Oil / Recovery' },
                    diagnostics: { title: 'Pressure / Decline' },
                },
            },
        });
        expect(fetkovichConfig.chart?.xAxisMode).toBe('logTime');
        expect(fetkovichConfig.chart?.logScale).toBe(true);
        expect(dietzConfig.chart?.panels?.rates?.curveKeys).toEqual(['oil-rate-sim', 'oil-rate-reference', 'oil-rate-error']);
    });
});