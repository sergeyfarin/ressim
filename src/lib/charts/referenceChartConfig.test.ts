import { describe, expect, it } from 'vitest';
import { getBenchmarkFamily } from '../catalog/caseCatalog';
import { getReferenceRateChartLayoutConfig } from './referenceChartConfig';

describe('referenceChartConfig', () => {
    it('builds breakthrough-centric BL chart defaults with PVI x-axis', () => {
        const family = getBenchmarkFamily('bl_case_a_refined');
        const config = getReferenceRateChartLayoutConfig({
            family,
            referencePolicy: {
                scenarioClass: 'buckley-leverett',
                referenceKind: 'analytical',
                referenceSource: 'buckley-leverett-shock-reference',
                referenceLabel: 'Analytical Buckley-Leverett shock reference',
                primaryTruthLabel: 'Analytical breakthrough-PV comparison',
                analyticalOverlayRole: 'primary',
                summary: 'Analytical Buckley-Leverett reference is the primary truth source for this run.',
            },
        });

        expect(config).toMatchObject({
            rateChart: {
                xAxisMode: 'pvi',
                ratesExpanded: true,
                cumulativeExpanded: true,
                diagnosticsExpanded: false,
                panels: {
                    rates: {
                        title: 'Breakthrough',
                        scalePreset: 'breakthrough',
                    },
                    cumulative: {
                        title: 'Recovery',
                    },
                    diagnostics: {
                        title: 'Pressure',
                    },
                },
            },
        });
    });

    it('drops analytical BL overlay from primary panel defaults when numerical reference is primary', () => {
        const family = getBenchmarkFamily('bl_case_b_refined');
        const config = getReferenceRateChartLayoutConfig({
            family,
            referencePolicy: {
                scenarioClass: 'buckley-leverett',
                referenceKind: 'numerical-refined',
                referenceSource: 'bl_case_b_refined:refined-numerical-reference',
                referenceLabel: 'Refined numerical benchmark reference',
                primaryTruthLabel: 'Refined numerical breakthrough-PV comparison',
                analyticalOverlayRole: 'secondary',
                summary: 'A refined numerical benchmark run is the primary truth source; analytical overlay is not a strict equality test here.',
            },
        });

        expect(config.rateChart?.panels?.rates?.curveLabels).toEqual(['Water Cut (Sim)', 'Avg Water Sat']);
        expect(config.rateChart?.panels?.cumulative?.curveLabels).toEqual(['Recovery Factor', 'Cum Oil', 'Cum Injection']);
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
    });
});