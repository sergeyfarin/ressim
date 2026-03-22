import { describe, expect, it } from 'vitest';
import { getBenchmarkFamily } from '../catalog/caseCatalog';
import { getReferenceRateChartLayoutConfig } from './referenceChartConfig';

describe('referenceChartConfig', () => {
    it('builds breakthrough-centric BL chart defaults with PVI x-axis', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const config = getReferenceRateChartLayoutConfig({
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
            rateChart: {
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
        expect(config.rateChart?.panels?.rates?.curveKeys).toEqual(['water-cut-sim', 'water-cut-reference', 'avg-water-sat']);
    });

    it('drops the reference-solution BL overlay from primary panel defaults when numerical reference is primary', () => {
        const family = getBenchmarkFamily('bl_case_b_refined');
        const config = getReferenceRateChartLayoutConfig({
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

        expect(config.rateChart?.panels?.rates?.curveKeys).toEqual(['water-cut-sim', 'avg-water-sat']);
        expect(config.rateChart?.panels?.cumulative?.curveKeys).toEqual(['recovery-factor', 'cum-oil-sim', 'cum-injection']);
    });

    it('builds depletion-focused chart defaults and log-time Fetkovich preference', () => {
        const dietz = getBenchmarkFamily('dietz_sq_center');
        const fetkovich = getBenchmarkFamily('fetkovich_exp');
        const dietzConfig = getReferenceRateChartLayoutConfig({ family: dietz });
        const fetkovichConfig = getReferenceRateChartLayoutConfig({ family: fetkovich });

        expect(dietzConfig).toMatchObject({
            rateChart: {
                xAxisMode: 'time',
                panels: {
                    rates: { title: 'Oil Rate' },
                    cumulative: { title: 'Cumulative Oil / Recovery' },
                    diagnostics: { title: 'Pressure / Decline' },
                },
            },
        });
        expect(fetkovichConfig.rateChart?.xAxisMode).toBe('logTime');
        expect(fetkovichConfig.rateChart?.logScale).toBe(true);
        expect(dietzConfig.rateChart?.panels?.rates?.curveKeys).toEqual(['oil-rate-sim', 'oil-rate-reference', 'oil-rate-error']);
    });
});