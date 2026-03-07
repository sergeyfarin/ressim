import { describe, expect, it } from 'vitest';
import {
    caseLibraryEntries,
    catalog,
    getCaseLibraryEntriesForFamily,
    getCaseLibraryEntriesForFamilyAndGroup,
    getCaseLibraryEntry,
    getCaseLibraryGroupsForFamily,
} from './caseCatalog';

describe('caseLibrary adapter', () => {
    it('exposes a unified library registry through the catalog entrypoint', () => {
        expect(catalog.caseLibrary).toEqual(caseLibraryEntries);
        expect(caseLibraryEntries).toHaveLength(17);
    });

    it('normalizes Buckley-Leverett references into waterflood library entries', () => {
        const caseA = getCaseLibraryEntry('bl_case_a_refined');

        expect(caseA).toMatchObject({
            entryKind: 'benchmark-family',
            family: 'waterflood',
            group: 'literature-reference',
            caseSource: 'case-library',
            sourceLabel: 'Buckley-Leverett analytical shock reference',
            benchmarkFamilyKey: 'bl_case_a_refined',
            activation: {
                activeMode: 'benchmark',
                benchmarkId: 'bl_case_a_refined',
                presetKey: null,
            },
            editabilityPolicy: {
                kind: 'library-reference',
                allowDirectInputEditing: false,
                allowSensitivitySelection: true,
                allowCustomizeAction: true,
            },
        });
        expect(caseA?.sensitivityAxes.map((axis) => axis.key)).toEqual([
            'grid-refinement',
            'timestep-refinement',
            'heterogeneity',
        ]);
    });

    it('routes Fetkovich into the type-curves family while keeping literature-reference policy', () => {
        const fetkovich = getCaseLibraryEntry('fetkovich_exp');

        expect(fetkovich).toMatchObject({
            family: 'type-curves',
            group: 'literature-reference',
            sourceLabel: 'Fetkovich decline-curve reference',
            benchmarkFamilyKey: 'fetkovich_exp',
            runPolicy: 'compare-to-reference',
        });
        expect(fetkovich?.sensitivityAxes).toEqual([]);
    });

    it('normalizes curated starters separately from literature references', () => {
        const baseline = getCaseLibraryEntry('baseline_waterflood');
        const homogeneous = getCaseLibraryEntry('bl_aligned_homogeneous');

        expect(baseline).toMatchObject({
            entryKind: 'preset',
            family: 'scenario-builder',
            group: 'curated-starter',
            sourceLabel: 'Curated exploratory starter',
            activation: {
                activeMode: 'sim',
                benchmarkId: null,
                presetKey: 'baseline_waterflood',
            },
            editabilityPolicy: {
                kind: 'library-starter',
                allowDirectInputEditing: true,
                allowSensitivitySelection: false,
                allowCustomizeAction: false,
            },
        });
        expect(homogeneous).toMatchObject({
            family: 'waterflood',
            group: 'curated-starter',
            sourceLabel: 'Curated internal starter',
        });
        expect(baseline?.sensitivityAxes).toEqual([]);
    });

    it('filters the unified library by family and group for future inputs-region selectors', () => {
        expect(getCaseLibraryGroupsForFamily('waterflood')).toEqual([
            'literature-reference',
            'curated-starter',
        ]);

        expect(getCaseLibraryEntriesForFamily('depletion-analysis').map((entry) => entry.key)).toEqual([
            'dietz_sq_center',
            'dietz_sq_corner',
            'depletion_center_producer',
            'depletion_1d_clean',
            'depletion_2d_radial_clean',
            'depletion_corner_producer',
        ]);
    });

    it('returns the expected depletion references and starters when both filters are applied', () => {
        expect(
            getCaseLibraryEntriesForFamilyAndGroup('depletion-analysis', 'literature-reference')
                .map((entry) => entry.key),
        ).toEqual([
            'dietz_sq_center',
            'dietz_sq_corner',
        ]);

        expect(
            getCaseLibraryEntriesForFamilyAndGroup('scenario-builder', 'curated-starter')
                .map((entry) => entry.key),
        ).toEqual([
            'baseline_waterflood',
            'high_contrast_layers',
            'viscous_fingering_risk',
        ]);
    });
});