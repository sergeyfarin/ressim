import {
    buildScenarioEditabilityPolicy,
    resolveProductFamily,
    type LibraryCaseGroup,
    type ProductFamily,
    type ScenarioEditabilityPolicy,
    type ScenarioSource,
} from '../stores/phase2PresetContract';
import {
    benchmarkFamilies,
    getBenchmarkSensitivityAxisLabel,
    getBenchmarkVariantsForFamily,
    type BenchmarkComparisonMetric,
    type BenchmarkDisplayDefaults,
    type BenchmarkFamily,
    type BenchmarkReferenceDefinition,
    type BenchmarkRunPolicy,
    type BenchmarkSensitivityAxisKey,
} from './benchmarkCases';
import {
    presetCases,
    type PresetCategory,
    type PresetEntry,
    type PresetLayoutConfig,
    type PresetMode,
} from './presetCases';

export type CaseLibraryEntryKind = 'benchmark-family' | 'preset';

export type CaseLibraryActivationMode = PresetMode | 'benchmark';

export type CaseLibraryActivation = {
    activeMode: CaseLibraryActivationMode;
    benchmarkId: string | null;
    presetKey: string | null;
};

export type CaseLibrarySensitivityAxis = {
    key: BenchmarkSensitivityAxisKey;
    label: string;
    variantCount: number;
    variantKeys: string[];
};

export type CaseLibraryEntry = {
    key: string;
    entryKind: CaseLibraryEntryKind;
    family: ProductFamily;
    group: LibraryCaseGroup;
    caseSource: ScenarioSource;
    activation: CaseLibraryActivation;
    label: string;
    description: string;
    params: Record<string, any>;
    sourceLabel: string;
    referencePolicySummary: string;
    sensitivitySummary: string;
    sensitivityAxes: CaseLibrarySensitivityAxis[];
    editabilityPolicy: ScenarioEditabilityPolicy;
    benchmarkFamilyKey: string | null;
    benchmarkReference: BenchmarkReferenceDefinition | null;
    comparisonMetric: BenchmarkComparisonMetric | null;
    runPolicy: BenchmarkRunPolicy | null;
    displayDefaults: BenchmarkDisplayDefaults | null;
    presetCategory: PresetCategory | null;
    layoutConfig: PresetLayoutConfig | null;
};

const CASE_LIBRARY_SOURCE: ScenarioSource = 'case-library';

const FAMILY_SORT_ORDER: Record<ProductFamily, number> = {
    waterflood: 0,
    'depletion-analysis': 1,
    'type-curves': 2,
    'scenario-builder': 3,
};

const GROUP_SORT_ORDER: Record<LibraryCaseGroup, number> = {
    'literature-reference': 0,
    'internal-reference': 1,
    'curated-starter': 2,
};

const BENCHMARK_SOURCE_LABELS: Record<string, string> = {
    'buckley-leverett-shock-reference': 'Buckley-Leverett analytical shock reference',
    'dietz-shape-factor-reference': 'Dietz (1965) shape-factor reference',
    'fetkovich-decline-reference': 'Fetkovich decline-curve reference',
};

function buildBenchmarkSensitivityAxes(family: BenchmarkFamily): CaseLibrarySensitivityAxis[] {
    return family.sensitivityAxes.map((axis) => {
        const variants = getBenchmarkVariantsForFamily(family.key)
            .filter((variant) => variant.axis === axis)
            .map((variant) => variant.key);

        return {
            key: axis,
            label: getBenchmarkSensitivityAxisLabel(axis),
            variantCount: variants.length,
            variantKeys: variants,
        };
    });
}

function buildBenchmarkSourceLabel(family: BenchmarkFamily): string {
    return BENCHMARK_SOURCE_LABELS[family.reference.source] ?? family.reference.source;
}

function buildBenchmarkReferencePolicySummary(
    family: BenchmarkFamily,
    sourceLabel: string,
    sensitivityAxes: CaseLibrarySensitivityAxis[],
): string {
    const sensitivityClause = sensitivityAxes.length > 0
        ? `Allowed library sensitivities: ${sensitivityAxes.map((axis) => axis.label).join(', ')}.`
        : 'No library sensitivity sweep is exposed for this reference case.';

    return `Locked literature reference case. Runs compare directly against the ${sourceLabel}. ${sensitivityClause} Use Customize to branch into a writable custom scenario when you need to edit fixed inputs.`;
}

function buildPresetFamily(entry: PresetEntry): ProductFamily {
    if (entry.mode === 'wf') return 'waterflood';
    if (entry.mode === 'sim') return 'scenario-builder';
    return 'depletion-analysis';
}

function buildPresetSourceLabel(entry: PresetEntry): string {
    if (entry.category === 'exploration') {
        return 'Curated exploratory starter';
    }
    return 'Curated internal starter';
}

function buildPresetReferencePolicySummary(entry: PresetEntry): string {
    if (entry.category === 'exploration') {
        return 'Curated exploratory starter case. It is editable immediately and becomes part of the custom workflow as you tune parameters.';
    }
    return 'Curated starter case from the internal library. It is editable immediately and transitions into the custom workflow on first input edit.';
}

function buildPresetSensitivitySummary(): string {
    return 'No locked library sensitivity sweep is defined for this starter case.';
}

function buildBenchmarkLibraryEntry(family: BenchmarkFamily): CaseLibraryEntry {
    const sensitivityAxes = buildBenchmarkSensitivityAxes(family);
    const sourceLabel = buildBenchmarkSourceLabel(family);

    return {
        key: family.key,
        entryKind: 'benchmark-family',
        family: resolveProductFamily({
            activeMode: 'benchmark',
            benchmarkScenarioClass: family.scenarioClass,
            benchmarkId: family.key,
        }),
        group: 'literature-reference',
        caseSource: CASE_LIBRARY_SOURCE,
        activation: {
            activeMode: 'benchmark',
            benchmarkId: family.key,
            presetKey: null,
        },
        label: family.label,
        description: family.description,
        params: family.baseCase.params,
        sourceLabel,
        referencePolicySummary: buildBenchmarkReferencePolicySummary(family, sourceLabel, sensitivityAxes),
        sensitivitySummary: sensitivityAxes.length > 0
            ? `Available sensitivities: ${sensitivityAxes.map((axis) => axis.label).join(', ')}.`
            : 'No library sensitivities available.',
        sensitivityAxes,
        editabilityPolicy: buildScenarioEditabilityPolicy({
            activeMode: 'benchmark',
            caseSource: CASE_LIBRARY_SOURCE,
        }),
        benchmarkFamilyKey: family.key,
        benchmarkReference: family.reference,
        comparisonMetric: family.comparisonMetric ?? null,
        runPolicy: family.runPolicy,
        displayDefaults: family.displayDefaults,
        presetCategory: null,
        layoutConfig: null,
    };
}

function buildPresetLibraryEntry(entry: PresetEntry): CaseLibraryEntry {
    const family = buildPresetFamily(entry);

    return {
        key: entry.key,
        entryKind: 'preset',
        family,
        group: 'curated-starter',
        caseSource: CASE_LIBRARY_SOURCE,
        activation: {
            activeMode: entry.mode,
            benchmarkId: null,
            presetKey: entry.key,
        },
        label: entry.label,
        description: entry.description,
        params: entry.params,
        sourceLabel: buildPresetSourceLabel(entry),
        referencePolicySummary: buildPresetReferencePolicySummary(entry),
        sensitivitySummary: buildPresetSensitivitySummary(),
        sensitivityAxes: [],
        editabilityPolicy: buildScenarioEditabilityPolicy({
            activeMode: entry.mode,
            caseSource: CASE_LIBRARY_SOURCE,
        }),
        benchmarkFamilyKey: null,
        benchmarkReference: null,
        comparisonMetric: null,
        runPolicy: null,
        displayDefaults: null,
        presetCategory: entry.category,
        layoutConfig: entry.layoutConfig ?? null,
    };
}

function compareCaseLibraryEntries(left: CaseLibraryEntry, right: CaseLibraryEntry): number {
    const familyDelta = FAMILY_SORT_ORDER[left.family] - FAMILY_SORT_ORDER[right.family];
    if (familyDelta !== 0) return familyDelta;

    const groupDelta = GROUP_SORT_ORDER[left.group] - GROUP_SORT_ORDER[right.group];
    if (groupDelta !== 0) return groupDelta;

    return left.label.localeCompare(right.label);
}

export const caseLibraryEntries: CaseLibraryEntry[] = [
    ...benchmarkFamilies.map(buildBenchmarkLibraryEntry),
    ...presetCases.map(buildPresetLibraryEntry),
].sort(compareCaseLibraryEntries);

const caseLibraryEntryMap = new Map(caseLibraryEntries.map((entry) => [entry.key, entry]));

export function getCaseLibraryEntry(key: string | null | undefined): CaseLibraryEntry | null {
    if (!key) return null;
    return caseLibraryEntryMap.get(key) ?? null;
}

export function getCaseLibraryEntries(input: {
    family?: ProductFamily | null;
    group?: LibraryCaseGroup | null;
} = {}): CaseLibraryEntry[] {
    return caseLibraryEntries.filter((entry) => {
        if (input.family && entry.family !== input.family) return false;
        if (input.group && entry.group !== input.group) return false;
        return true;
    });
}

export function getCaseLibraryEntriesForFamily(family: ProductFamily | null | undefined): CaseLibraryEntry[] {
    if (!family) return [];
    return getCaseLibraryEntries({ family });
}

export function getCaseLibraryEntriesForGroup(group: LibraryCaseGroup | null | undefined): CaseLibraryEntry[] {
    if (!group) return [];
    return getCaseLibraryEntries({ group });
}

export function getCaseLibraryEntriesForFamilyAndGroup(
    family: ProductFamily | null | undefined,
    group: LibraryCaseGroup | null | undefined,
): CaseLibraryEntry[] {
    if (!family || !group) return [];
    return getCaseLibraryEntries({ family, group });
}

export function getCaseLibraryGroupsForFamily(family: ProductFamily | null | undefined): LibraryCaseGroup[] {
    if (!family) return [];

    return [...new Set(
        getCaseLibraryEntriesForFamily(family)
            .map((entry) => entry.group),
    )].sort((left, right) => GROUP_SORT_ORDER[left] - GROUP_SORT_ORDER[right]);
}