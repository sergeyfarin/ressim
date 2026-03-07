import { describe, expect, it } from 'vitest';
import {
    buildBenchmarkCloneProvenance,
    buildComparisonSelection,
    buildOverrideResetPlan,
    buildBasePresetProfile,
    buildParameterOverrides,
    buildScenarioEditabilityPolicy,
    buildScenarioNavigationState,
    evaluateAnalyticalStatus,
    getFacetCustomizeSectionTarget,
    getFacetOverrideGroups,
    getOverrideGroupSectionTarget,
    groupParameterOverrides,
    resolveProductFamily,
    resolveScenarioSource,
    shouldAllowBenchmarkClone,
    shouldAutoClearModifiedState,
    shouldShowModePanelStatusRow,
} from './phase2PresetContract';

describe('phase2PresetContract', () => {
    it('marks modified presets as custom source', () => {
        const profile = buildBasePresetProfile({
            key: 'mode-dep_geo-1d',
            mode: 'dep',
            toggles: { mode: 'dep', geo: '1d', well: 'e2e' },
            isModified: true,
        });

        expect(profile.source).toBe('custom');
        expect(profile.mode).toBe('dep');
        expect(profile.family).toBe('depletion-analysis');
        expect(profile.caseSource).toBe('custom');
        expect(profile.libraryCaseKey).toBeNull();
        expect(profile.editabilityPolicy.kind).toBe('custom-editable');
    });

    it('maps benchmark families into compatibility product families', () => {
        expect(resolveProductFamily({
            activeMode: 'benchmark',
            benchmarkScenarioClass: 'buckley-leverett',
            benchmarkId: 'bl_case_a_refined',
        })).toBe('waterflood');

        expect(resolveProductFamily({
            activeMode: 'benchmark',
            benchmarkScenarioClass: 'depletion',
            benchmarkId: 'dietz_sq_center',
        })).toBe('depletion-analysis');

        expect(resolveProductFamily({
            activeMode: 'benchmark',
            benchmarkScenarioClass: 'depletion',
            benchmarkId: 'fetkovich_exp',
        })).toBe('type-curves');
    });

    it('derives scenario source from modified state', () => {
        expect(resolveScenarioSource({ isModified: false })).toBe('case-library');
        expect(resolveScenarioSource({ isModified: true })).toBe('custom');
    });

    it('builds compatibility navigation state for benchmark references', () => {
        const navigation = buildScenarioNavigationState({
            activeMode: 'benchmark',
            isModified: false,
            activeCaseKey: 'bench_bl-case-a-refined',
            activeLibraryCaseKey: 'bl_case_a_refined',
            activeLibraryGroup: 'internal-reference',
            sourceLabel: 'Internal Rust-parity validation family',
            referenceSourceLabel: 'Buckley-Leverett analytical shock reference',
            provenanceSummary: 'Homogeneous Rust-parity Buckley-Leverett base family maintained as an internal validation case.',
            benchmarkId: 'bl_case_a_refined',
            benchmarkScenarioClass: 'buckley-leverett',
            activeComparisonSelection: {
                primaryResultKey: 'base',
                comparedResultKeys: ['grid_48'],
            },
        });

        expect(navigation).toMatchObject({
            activeFamily: 'waterflood',
            activeSource: 'case-library',
            activeLibraryCaseKey: 'bl_case_a_refined',
            activeLibraryGroup: 'internal-reference',
            sourceLabel: 'Internal Rust-parity validation family',
            referenceSourceLabel: 'Buckley-Leverett analytical shock reference',
            provenanceSummary: 'Homogeneous Rust-parity Buckley-Leverett base family maintained as an internal validation case.',
            editabilityPolicy: {
                kind: 'library-reference',
                allowDirectInputEditing: false,
                allowSensitivitySelection: true,
                allowCustomizeAction: true,
            },
        });
        expect(navigation.activeComparisonSelection).toEqual({
            primaryResultKey: 'base',
            comparedResultKeys: ['grid_48'],
        });
    });

    it('clears threaded library metadata when navigation becomes custom', () => {
        const navigation = buildScenarioNavigationState({
            activeMode: 'benchmark',
            isModified: true,
            activeLibraryCaseKey: 'bl_case_a_refined',
            activeLibraryGroup: 'internal-reference',
            sourceLabel: 'Internal Rust-parity validation family',
            referenceSourceLabel: 'Buckley-Leverett analytical shock reference',
            provenanceSummary: 'Homogeneous Rust-parity Buckley-Leverett base family maintained as an internal validation case.',
            benchmarkId: 'bl_case_a_refined',
            benchmarkScenarioClass: 'buckley-leverett',
        });

        expect(navigation).toMatchObject({
            activeSource: 'custom',
            activeLibraryCaseKey: null,
            activeLibraryGroup: null,
            sourceLabel: null,
            referenceSourceLabel: null,
            provenanceSummary: null,
        });
    });

    it('preserves explicit null library metadata for unresolved non-benchmark selections', () => {
        const navigation = buildScenarioNavigationState({
            activeMode: 'dep',
            isModified: false,
            activeCaseKey: 'mode-dep_geo-1d_well-e2e',
            activeLibraryCaseKey: null,
            activeLibraryGroup: null,
            sourceLabel: null,
            referenceSourceLabel: null,
            provenanceSummary: null,
        });

        expect(navigation).toMatchObject({
            activeSource: 'case-library',
            activeLibraryCaseKey: null,
            activeLibraryGroup: null,
            sourceLabel: null,
            referenceSourceLabel: null,
            provenanceSummary: null,
        });
    });

    it('builds editable starter policy for non-benchmark library cases', () => {
        const policy = buildScenarioEditabilityPolicy({
            activeMode: 'wf',
            caseSource: 'case-library',
        });

        expect(policy).toEqual({
            kind: 'library-starter',
            allowDirectInputEditing: true,
            allowSensitivitySelection: false,
            allowCustomizeAction: false,
            transitionsToCustomOnEdit: true,
        });
    });

    it('treats non-benchmark reference library cases as locked reference flows', () => {
        const policy = buildScenarioEditabilityPolicy({
            activeMode: 'dep',
            caseSource: 'case-library',
            activeLibraryGroup: 'literature-reference',
        });

        expect(policy).toEqual({
            kind: 'library-reference',
            allowDirectInputEditing: false,
            allowSensitivitySelection: true,
            allowCustomizeAction: true,
            transitionsToCustomOnEdit: false,
        });
    });

    it('builds reference base profiles from family-owned library references outside benchmark mode', () => {
        const profile = buildBasePresetProfile({
            key: 'dietz_sq_center',
            mode: 'dep',
            toggles: { mode: 'dep', geo: '1d', well: 'e2e' },
            isModified: false,
            benchmarkId: 'dietz_sq_center',
            benchmarkLabel: 'Dietz Square Center',
            benchmarkScenarioClass: 'depletion',
            activeLibraryCaseKey: 'dietz_sq_center',
            activeLibraryGroup: 'literature-reference',
        });

        expect(profile.source).toBe('benchmark');
        expect(profile.label).toBe('Dietz Square Center');
        expect(profile.benchmarkId).toBe('dietz_sq_center');
        expect(profile.editabilityPolicy.kind).toBe('library-reference');
        expect(profile.libraryCaseKey).toBe('dietz_sq_center');
        expect(profile.libraryCaseGroup).toBe('literature-reference');
    });

    it('normalizes comparison selection keys', () => {
        expect(buildComparisonSelection({
            primaryResultKey: '',
            comparedResultKeys: ['run-a', 'run-a', '', 'run-b'],
        })).toEqual({
            primaryResultKey: null,
            comparedResultKeys: ['run-a', 'run-b'],
        });
    });

    it('detects scalar and array overrides', () => {
        const overrides = buildParameterOverrides({
            currentParams: {
                nx: 20,
                gravityEnabled: true,
                layerPermsX: [50, 100],
            },
            baseParams: {
                nx: 10,
                gravityEnabled: false,
                layerPermsX: [50, 200],
            },
            trackedKeys: ['nx', 'gravityEnabled', 'layerPermsX'],
        });

        expect(Object.keys(overrides)).toEqual(['nx', 'gravityEnabled', 'layerPermsX']);
        expect(overrides.nx.base).toBe(10);
        expect(overrides.nx.current).toBe(20);
    });

    it('preserves tracked-key order when building overrides', () => {
        const overrides = buildParameterOverrides({
            currentParams: {
                a: 2,
                b: 3,
            },
            baseParams: {
                a: 1,
                b: 2,
            },
            trackedKeys: ['b', 'a'],
        });

        expect(Object.keys(overrides)).toEqual(['b', 'a']);
    });

    it('groups overrides by configured group keys', () => {
        const grouped = groupParameterOverrides({
            nx: { base: 10, current: 20 },
            gravityEnabled: { base: false, current: true },
            injectorBhp: { base: 400, current: 450 },
        });

        expect(grouped.grid).toEqual(['nx']);
        expect(grouped.physics).toEqual(['gravityEnabled']);
        expect(grouped.wells).toEqual(['injectorBhp']);
    });

    it('returns reference analytical status for ideal waterflood assumptions', () => {
        const status = evaluateAnalyticalStatus({
            activeMode: 'wf',
            analyticalMode: 'waterflood',
            injectorEnabled: true,
            gravityEnabled: false,
            capillaryEnabled: false,
            permMode: 'uniform',
            toggles: { mode: 'wf', geo: '1d', well: 'e2e' },
        });

        expect(status.level).toBe('reference');
        expect(status.warningSeverity).toBe('none');
        expect(status.reasonDetails).toEqual([]);
        expect(status.reasons).toEqual([]);
    });

    it('returns approximate analytical status with reasons for non-ideal setup', () => {
        const status = evaluateAnalyticalStatus({
            activeMode: 'sim',
            analyticalMode: 'waterflood',
            injectorEnabled: true,
            gravityEnabled: true,
            capillaryEnabled: true,
            permMode: 'random',
            toggles: { mode: 'sim', geo: '2dxy', well: 'corner' },
        });

        expect(status.level).toBe('approximate');
        expect(status.warningSeverity).toBe('warning');
        expect(status.reasonDetails.some((r) => r.code === 'sim-mode-exploratory')).toBe(true);
        expect(status.reasons.length).toBeGreaterThan(0);
    });

    it('returns critical warning severity for contradictory injector assumptions', () => {
        const status = evaluateAnalyticalStatus({
            activeMode: 'wf',
            analyticalMode: 'waterflood',
            injectorEnabled: false,
            gravityEnabled: false,
            capillaryEnabled: false,
            permMode: 'uniform',
            toggles: { mode: 'wf', geo: '1d', well: 'e2e' },
        });

        expect(status.level).toBe('approximate');
        expect(status.warningSeverity).toBe('critical');
        expect(status.reasonDetails.some((r) => r.code === 'wf-injector-disabled')).toBe(true);
    });

    it('returns off status with non-warning severity when analytical mode is none', () => {
        const status = evaluateAnalyticalStatus({
            activeMode: 'sim',
            analyticalMode: 'none',
            injectorEnabled: true,
            gravityEnabled: false,
            capillaryEnabled: false,
            permMode: 'uniform',
            toggles: { mode: 'sim', geo: '2dxy', well: 'corner' },
        });

        expect(status.level).toBe('off');
        expect(status.mode).toBe('none');
        expect(status.warningSeverity).toBe('none');
        expect(status.reasonDetails).toEqual([
            {
                code: 'analytical-disabled',
                message: 'Analytical overlay is disabled for this scenario.',
                severity: 'notice',
            },
        ]);
    });

    it('uses centralized facet section mapping with safe fallback', () => {
        expect(getFacetCustomizeSectionTarget('geo')).toBe('static');
        expect(getFacetCustomizeSectionTarget('dt')).toBe('timestep');
        expect(getFacetCustomizeSectionTarget('unknown-dim')).toBe('shell');
    });

    it('uses centralized facet override-group mapping with safe fallback', () => {
        expect(getFacetOverrideGroups('rock')).toEqual(['permeability']);
        expect(getFacetOverrideGroups('well')).toEqual(['wells']);
        expect(getFacetOverrideGroups('unknown-dim')).toEqual([]);
    });

    it('maps override groups to section targets with safe fallback', () => {
        expect(getOverrideGroupSectionTarget('grid')).toBe('static');
        expect(getOverrideGroupSectionTarget('wells')).toBe('well');
        expect(getOverrideGroupSectionTarget('analytical')).toBe('analytical');
        expect(getOverrideGroupSectionTarget('unknown-group')).toBe('shell');
    });

    it('builds benchmark clone provenance from benchmark context', () => {
        const provenance = buildBenchmarkCloneProvenance({
            benchmarkId: 'bl_case_a_refined',
            sourceCaseKey: 'bench_bl-case-a-refined',
            sourceLabel: 'BL Case A Refined',
            nowIso: '2026-03-05T18:00:00.000Z',
        });

        expect(provenance).toEqual({
            sourceBenchmarkId: 'bl_case_a_refined',
            sourceCaseKey: 'bench_bl-case-a-refined',
            sourceLabel: 'BL Case A Refined',
            clonedAtIso: '2026-03-05T18:00:00.000Z',
        });
    });

    it('returns null clone provenance when benchmark context is incomplete', () => {
        expect(buildBenchmarkCloneProvenance({
            benchmarkId: null,
            sourceCaseKey: 'bench_x',
            sourceLabel: 'X',
        })).toBeNull();

        expect(buildBenchmarkCloneProvenance({
            benchmarkId: 'bl_case_a_refined',
            sourceCaseKey: '',
            sourceLabel: 'X',
        })).toBeNull();

        expect(buildBenchmarkCloneProvenance({
            benchmarkId: 'bl_case_a_refined',
            sourceCaseKey: 'bench_x',
            sourceLabel: '',
        })).toBeNull();
    });

    it('auto-clears modified state only for non-benchmark cases with no provenance and no overrides', () => {
        expect(shouldAutoClearModifiedState({
            isModified: true,
            activeMode: 'dep',
            benchmarkProvenance: null,
            parameterOverrideCount: 0,
        })).toBe(true);

        expect(shouldAutoClearModifiedState({
            isModified: true,
            activeMode: 'benchmark',
            benchmarkProvenance: null,
            parameterOverrideCount: 0,
        })).toBe(false);

        expect(shouldAutoClearModifiedState({
            isModified: true,
            activeMode: 'dep',
            benchmarkProvenance: {
                sourceBenchmarkId: 'bl_case_a_refined',
                sourceCaseKey: 'bench_bl-case-a-refined',
                sourceLabel: 'BL Case A Refined',
                clonedAtIso: '2026-03-06T20:00:00.000Z',
            },
            parameterOverrideCount: 0,
        })).toBe(false);

        expect(shouldAutoClearModifiedState({
            isModified: true,
            activeMode: 'dep',
            benchmarkProvenance: null,
            parameterOverrideCount: 2,
        })).toBe(false);
    });

    it('only allows clone-to-custom from unmodified reference-capable state', () => {
        expect(shouldAllowBenchmarkClone({ activeMode: 'benchmark', isModified: false })).toBe(true);
        expect(shouldAllowBenchmarkClone({ activeMode: 'benchmark', isModified: true })).toBe(false);
        expect(shouldAllowBenchmarkClone({ activeMode: 'dep', isModified: false, hasReferenceLibraryCase: true })).toBe(true);
        expect(shouldAllowBenchmarkClone({ activeMode: 'dep', isModified: false })).toBe(false);
    });

    it('shows mode-panel status row for provenance or tracked overrides', () => {
        expect(shouldShowModePanelStatusRow({
            benchmarkProvenance: null,
            parameterOverrideCount: 0,
        })).toBe(false);

        expect(shouldShowModePanelStatusRow({
            benchmarkProvenance: null,
            parameterOverrideCount: 1,
        })).toBe(true);

        expect(shouldShowModePanelStatusRow({
            benchmarkProvenance: {
                sourceBenchmarkId: 'bl_case_a_refined',
                sourceCaseKey: 'bench_bl-case-a-refined',
                sourceLabel: 'BL Case A Refined',
                clonedAtIso: '2026-03-06T20:00:00.000Z',
            },
            parameterOverrideCount: 0,
        })).toBe(true);
    });

    it('builds deterministic reset plan with de-duplication across groups', () => {
        const plan = buildOverrideResetPlan({
            groupKeys: ['fluids', 'grid'],
            groupedOverrides: {
                fluids: ['mu_w', 'ny'],
                grid: ['nx', 'ny'],
            },
            overrides: {
                nx: { base: 10, current: 20 },
                ny: { base: 1, current: 2 },
                mu_w: { base: 0.5, current: 0.8 },
            },
        });

        expect(plan).toEqual([
            { key: 'mu_w', base: 0.5 },
            { key: 'ny', base: 1 },
            { key: 'nx', base: 10 },
        ]);
    });

    it('ignores unknown groups and stale override keys in reset plan', () => {
        const plan = buildOverrideResetPlan({
            groupKeys: ['unknown', 'wells'],
            groupedOverrides: {
                unknown: ['missing'],
                wells: ['injectorBhp', 'missing'],
            },
            overrides: {
                injectorBhp: { base: 400, current: 450 },
            },
        });

        expect(plan).toEqual([{ key: 'injectorBhp', base: 400 }]);
    });
});
