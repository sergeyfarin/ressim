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
            group: 'internal-reference',
            caseSource: 'case-library',
            sourceLabel: 'Internal Rust-parity reference family',
            referenceSourceLabel: 'Buckley-Leverett reference solution',
            benchmarkFamilyKey: 'bl_case_a_refined',
            activation: {
                activeMode: 'wf',
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
        expect(caseA?.provenanceSummary).toContain('Homogeneous Rust-parity Buckley-Leverett family');
        expect(caseA?.sensitivityAxes.map((axis) => axis.key)).toEqual([
            'grid-refinement',
            '2d-grid-refinement',
            'timestep-refinement',
            'heterogeneity',
        ]);
    });

    it('routes Fetkovich into the type-curves family while keeping literature reference guidance', () => {
        const fetkovich = getCaseLibraryEntry('fetkovich_exp');

        expect(fetkovich).toMatchObject({
            family: 'type-curves',
            group: 'literature-reference',
            sourceLabel: 'Literature reference solution',
            referenceSourceLabel: 'Fetkovich reference solution',
            benchmarkFamilyKey: 'fetkovich_exp',
            runPolicy: 'compare-to-reference',
        });
        expect(fetkovich?.provenanceSummary).toContain('Fetkovich exponential decline behavior');
        expect(fetkovich?.sensitivityAxes).toEqual([]);
    });

    it('normalizes curated starters as locked library cases with customize handoff', () => {
        const baseline = getCaseLibraryEntry('baseline_waterflood');
        const homogeneous = getCaseLibraryEntry('bl_aligned_homogeneous');

        expect(baseline).toMatchObject({
            entryKind: 'preset',
            family: 'scenario-builder',
            group: 'curated-starter',
            sourceLabel: 'Curated exploratory starter',
            referenceSourceLabel: null,
            activation: {
                activeMode: 'sim',
                benchmarkId: null,
                presetKey: 'baseline_waterflood',
            },
            editabilityPolicy: {
                kind: 'library-reference',
                allowDirectInputEditing: false,
                allowSensitivitySelection: false,
                allowCustomizeAction: true,
            },
        });
        expect(homogeneous).toMatchObject({
            family: 'waterflood',
            group: 'curated-starter',
            sourceLabel: 'Curated internal starter',
        });
        expect(baseline?.provenanceSummary).toContain('curated starting point rather than a literature reference');
        expect(baseline?.sensitivityAxes).toEqual([]);
    });

    it('filters the unified library by family and group for future inputs-region selectors', () => {
        expect(getCaseLibraryGroupsForFamily('waterflood')).toEqual([
            'internal-reference',
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

    it('returns internal validation families separately from literature references', () => {
        expect(
            getCaseLibraryEntriesForFamilyAndGroup('waterflood', 'internal-reference')
                .map((entry) => entry.key),
        ).toEqual([
            'bl_case_a_refined',
            'bl_case_b_refined',
        ]);

        expect(
            getCaseLibraryEntriesForFamilyAndGroup('type-curves', 'literature-reference')
                .map((entry) => entry.key),
        ).toEqual([
            'fetkovich_exp',
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