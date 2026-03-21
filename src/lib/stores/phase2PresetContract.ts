import type { CaseMode, ToggleState } from '../catalog/caseCatalog';
import type { AnalyticalMethod } from '../catalog/scenarios';

export type PresetSource = 'facet' | 'reference' | 'custom';

export type ProductFamily =
    | 'waterflood'
    | 'depletion-analysis'
    | 'type-curves'
    | 'scenario-builder';

export type ScenarioSource = 'case-library' | 'custom';

export type LibraryCaseGroup =
    | 'literature-reference'
    | 'internal-reference'
    | 'curated-starter';

export type ScenarioEditabilityPolicyKind =
    | 'library-reference'
    | 'custom-editable';

export type ScenarioEditabilityPolicy = {
    kind: ScenarioEditabilityPolicyKind;
    allowDirectInputEditing: boolean;
    allowSensitivitySelection: boolean;
    allowCustomizeAction: boolean;
    transitionsToCustomOnEdit: boolean;
};

export type ComparisonSelection = {
    primaryResultKey: string | null;
    comparedResultKeys: string[];
};

export type ScenarioNavigationState = {
    activeFamily: ProductFamily;
    activeSource: ScenarioSource;
    activeLibraryCaseKey: string | null;
    activeLibraryGroup: LibraryCaseGroup | null;
    sourceLabel: string | null;
    referenceSourceLabel: string | null;
    provenanceSummary: string | null;
    activeComparisonSelection: ComparisonSelection;
    editabilityPolicy: ScenarioEditabilityPolicy;
};

export type BasePresetProfile = {
    key: string;
    mode: CaseMode;
    source: PresetSource;
    label: string;
    toggles: ToggleState;
    benchmarkId: string | null;
    family: ProductFamily;
    caseSource: ScenarioSource;
    libraryCaseKey: string | null;
    libraryCaseGroup: LibraryCaseGroup | null;
    editabilityPolicy: ScenarioEditabilityPolicy;
};

function isFetkovichBenchmarkCase(benchmarkId: string | null | undefined): boolean {
    return benchmarkId === 'fetkovich_exp';
}

export function resolveProductFamily(input: {
    activeMode: CaseMode;
    activeLibraryFamily?: ProductFamily | null;
    benchmarkScenarioClass?: AnalyticalMethod | null;
    benchmarkId?: string | null;
}): ProductFamily {
    if (input.activeLibraryFamily) return input.activeLibraryFamily;
    if (input.benchmarkScenarioClass === 'buckley-leverett' || input.benchmarkScenarioClass === 'gas-oil-bl') {
        return 'waterflood';
    }
    if (isFetkovichBenchmarkCase(input.benchmarkId ?? null)) {
        return 'type-curves';
    }
    if (input.benchmarkScenarioClass === 'depletion') {
        return 'depletion-analysis';
    }
    if (input.activeMode === 'wf') return 'waterflood';
    if (input.activeMode === 'sim') return 'scenario-builder';
    return 'depletion-analysis';
}

export function resolveScenarioSource(input: {
    isModified: boolean;
}): ScenarioSource {
    return input.isModified ? 'custom' : 'case-library';
}

export function resolveLibraryCaseKey(input: {
    caseKey: string | null | undefined;
    benchmarkId?: string | null;
    caseSource: ScenarioSource;
}): string | null {
    if (input.caseSource === 'custom') return null;
    if (input.benchmarkId) return input.benchmarkId ?? null;
    return input.caseKey ?? null;
}

export function resolveLibraryCaseGroup(input: {
    benchmarkId?: string | null;
    caseSource: ScenarioSource;
}): LibraryCaseGroup | null {
    if (input.caseSource === 'custom') return null;
    if (input.benchmarkId === 'bl_case_a_refined' || input.benchmarkId === 'bl_case_b_refined') {
        return 'internal-reference';
    }
    if (input.benchmarkId) {
        return 'literature-reference';
    }
    return 'curated-starter';
}

export function buildScenarioEditabilityPolicy(input: {
    activeMode: CaseMode;
    caseSource: ScenarioSource;
    activeLibraryGroup?: LibraryCaseGroup | null;
}): ScenarioEditabilityPolicy {
    if (input.caseSource === 'custom') {
        return {
            kind: 'custom-editable',
            allowDirectInputEditing: true,
            allowSensitivitySelection: false,
            allowCustomizeAction: false,
            transitionsToCustomOnEdit: false,
        };
    }

    return {
        kind: 'library-reference',
        allowDirectInputEditing: false,
        allowSensitivitySelection: input.activeLibraryGroup === 'literature-reference'
            || input.activeLibraryGroup === 'internal-reference',
        allowCustomizeAction: true,
        transitionsToCustomOnEdit: false,
    };
}

export function buildComparisonSelection(
    input: Partial<ComparisonSelection> = {},
): ComparisonSelection {
    const comparedResultKeys = Array.isArray(input.comparedResultKeys)
        ? [...new Set(input.comparedResultKeys.filter((key): key is string => typeof key === 'string' && key.length > 0))]
        : [];

    return {
        primaryResultKey: typeof input.primaryResultKey === 'string' && input.primaryResultKey.length > 0
            ? input.primaryResultKey
            : null,
        comparedResultKeys,
    };
}

export function buildScenarioNavigationState(input: {
    activeMode: CaseMode;
    isModified: boolean;
    activeCaseKey?: string | null;
    activeLibraryCaseKey?: string | null;
    activeLibraryFamily?: ProductFamily | null;
    activeLibraryGroup?: LibraryCaseGroup | null;
    sourceLabel?: string | null;
    referenceSourceLabel?: string | null;
    provenanceSummary?: string | null;
    benchmarkId?: string | null;
    benchmarkScenarioClass?: AnalyticalMethod | null;
    activeComparisonSelection?: ComparisonSelection;
}): ScenarioNavigationState {
    const hasResolvedLibraryCaseKey = Object.prototype.hasOwnProperty.call(input, 'activeLibraryCaseKey');
    const hasResolvedLibraryGroup = Object.prototype.hasOwnProperty.call(input, 'activeLibraryGroup');
    const hasSourceLabel = Object.prototype.hasOwnProperty.call(input, 'sourceLabel');
    const hasReferenceSourceLabel = Object.prototype.hasOwnProperty.call(input, 'referenceSourceLabel');
    const hasProvenanceSummary = Object.prototype.hasOwnProperty.call(input, 'provenanceSummary');

    const activeFamily = resolveProductFamily({
        activeMode: input.activeMode,
        activeLibraryFamily: input.activeLibraryFamily ?? null,
        benchmarkScenarioClass: input.benchmarkScenarioClass ?? null,
        benchmarkId: input.benchmarkId ?? null,
    });
    const activeSource = resolveScenarioSource({
        isModified: input.isModified,
    });
    const activeLibraryCaseKey = activeSource === 'custom'
        ? null
        : hasResolvedLibraryCaseKey
            ? input.activeLibraryCaseKey ?? null
            : resolveLibraryCaseKey({
                caseKey: input.activeCaseKey ?? null,
                benchmarkId: input.benchmarkId ?? null,
                caseSource: activeSource,
            });
    const activeLibraryGroup = activeSource === 'custom'
        ? null
        : hasResolvedLibraryGroup
            ? input.activeLibraryGroup ?? null
            : resolveLibraryCaseGroup({
                benchmarkId: input.benchmarkId ?? null,
                caseSource: activeSource,
            });

    return {
        activeFamily,
        activeSource,
        activeLibraryCaseKey,
        activeLibraryGroup,
        sourceLabel: activeSource === 'custom' ? null : hasSourceLabel ? input.sourceLabel ?? null : null,
        referenceSourceLabel: activeSource === 'custom' ? null : hasReferenceSourceLabel ? input.referenceSourceLabel ?? null : null,
        provenanceSummary: activeSource === 'custom' ? null : hasProvenanceSummary ? input.provenanceSummary ?? null : null,
        activeComparisonSelection: buildComparisonSelection(input.activeComparisonSelection),
        editabilityPolicy: buildScenarioEditabilityPolicy({
            activeMode: input.activeMode,
            caseSource: activeSource,
            activeLibraryGroup,
        }),
    };
}

export type ParameterOverrideValue = {
    base: unknown;
    current: unknown;
};

export type ParameterOverrides = Record<string, ParameterOverrideValue>;

export type OverrideResetPlanEntry = {
    key: string;
    base: unknown;
};

export type ParameterOverrideGroupKey =
    | 'grid'
    | 'initial'
    | 'fluids'
    | 'permeability'
    | 'relperm'
    | 'wells'
    | 'stability'
    | 'physics'
    | 'analytical';

export const PHASE2_TRACKED_PARAMETER_KEYS = [
    'nx', 'ny', 'nz',
    'cellDx', 'cellDy', 'cellDz',
    'initialPressure', 'initialSaturation', 'reservoirPorosity',
    'mu_w', 'mu_o', 'c_o', 'c_w',
    'rock_compressibility', 'depth_reference',
    'volume_expansion_o', 'volume_expansion_w',
    'rho_w', 'rho_o',
    'permMode',
    'uniformPermX', 'uniformPermY', 'uniformPermZ',
    'minPerm', 'maxPerm', 'useRandomSeed', 'randomSeed',
    'layerPermsX', 'layerPermsY', 'layerPermsZ',
    's_wc', 's_or', 'n_w', 'n_o', 'k_rw_max', 'k_ro_max',
    'well_radius', 'well_skin',
    'injectorBhp', 'producerBhp',
    'injectorControlMode', 'producerControlMode',
    'injectorEnabled', 'targetInjectorRate', 'targetProducerRate',
    'injectorI', 'injectorJ', 'producerI', 'producerJ',
    'max_sat_change_per_step', 'max_pressure_change_per_step', 'max_well_rate_change_fraction',
    'gravityEnabled', 'capillaryEnabled', 'capillaryPEntry', 'capillaryLambda',
    'analyticalSolutionMode', 'analyticalDepletionRateScale',
] as const;

export type TrackedParameterKey = (typeof PHASE2_TRACKED_PARAMETER_KEYS)[number];

export const PHASE2_PARAMETER_GROUPS: Record<ParameterOverrideGroupKey, readonly TrackedParameterKey[]> = {
    grid: ['nx', 'ny', 'nz', 'cellDx', 'cellDy', 'cellDz'],
    initial: ['initialPressure', 'initialSaturation', 'reservoirPorosity'],
    fluids: [
        'mu_w', 'mu_o', 'c_o', 'c_w',
        'rock_compressibility', 'depth_reference',
        'volume_expansion_o', 'volume_expansion_w',
        'rho_w', 'rho_o',
    ],
    permeability: [
        'permMode',
        'uniformPermX', 'uniformPermY', 'uniformPermZ',
        'minPerm', 'maxPerm', 'useRandomSeed', 'randomSeed',
        'layerPermsX', 'layerPermsY', 'layerPermsZ',
    ],
    relperm: ['s_wc', 's_or', 'n_w', 'n_o', 'k_rw_max', 'k_ro_max'],
    wells: [
        'well_radius', 'well_skin',
        'injectorBhp', 'producerBhp',
        'injectorControlMode', 'producerControlMode',
        'injectorEnabled', 'targetInjectorRate', 'targetProducerRate',
        'injectorI', 'injectorJ', 'producerI', 'producerJ',
    ],
    stability: ['max_sat_change_per_step', 'max_pressure_change_per_step', 'max_well_rate_change_fraction'],
    physics: ['gravityEnabled', 'capillaryEnabled', 'capillaryPEntry', 'capillaryLambda'],
    analytical: ['analyticalSolutionMode', 'analyticalDepletionRateScale'],
};

export type ParameterOverrideGroups = Record<ParameterOverrideGroupKey, string[]>;

export type CustomizeSectionTarget =
    | 'shell'
    | 'static'
    | 'timestep'
    | 'reservoir'
    | 'relcap'
    | 'well'
    | 'analytical';

export const FACET_TO_SECTION_TARGET: Record<string, CustomizeSectionTarget> = {
    geo: 'static',
    grid: 'static',
    dt: 'timestep',
    fluid: 'reservoir',
    rock: 'reservoir',
    grav: 'reservoir',
    cap: 'relcap',
    well: 'well',
    benchmarkId: 'shell',
};

export const FACET_TO_OVERRIDE_GROUPS: Record<string, readonly ParameterOverrideGroupKey[]> = {
    geo: ['grid'],
    grid: ['grid'],
    dt: ['stability'],
    fluid: ['fluids'],
    rock: ['permeability'],
    grav: ['physics'],
    cap: ['physics'],
    well: ['wells'],
    benchmarkId: [],
};

export const OVERRIDE_GROUP_TO_SECTION_TARGET: Record<ParameterOverrideGroupKey, CustomizeSectionTarget> = {
    grid: 'static',
    initial: 'reservoir',
    fluids: 'reservoir',
    permeability: 'reservoir',
    relperm: 'relcap',
    wells: 'well',
    stability: 'timestep',
    physics: 'reservoir',
    analytical: 'analytical',
};

export function getFacetCustomizeSectionTarget(dimensionKey: string): CustomizeSectionTarget {
    return FACET_TO_SECTION_TARGET[dimensionKey] ?? 'shell';
}

export function getFacetOverrideGroups(dimensionKey: string): ParameterOverrideGroupKey[] {
    const groups = FACET_TO_OVERRIDE_GROUPS[dimensionKey] ?? [];
    return [...groups];
}

export function getOverrideGroupSectionTarget(groupKey: string): CustomizeSectionTarget {
    const key = groupKey as ParameterOverrideGroupKey;
    return OVERRIDE_GROUP_TO_SECTION_TARGET[key] ?? 'shell';
}

export type ReferenceProvenance = {
    sourceBenchmarkId: string | null;
    sourceCaseKey: string;
    sourceLabel: string;
    clonedAtIso: string;
};

export function buildReferenceCloneProvenance(input: {
    benchmarkId: string | null | undefined;
    sourceCaseKey: string | null | undefined;
    sourceLabel: string | null | undefined;
    nowIso?: string;
}): ReferenceProvenance | null {
    const benchmarkId = input.benchmarkId ?? null;
    const sourceCaseKey = input.sourceCaseKey ?? null;
    const sourceLabel = input.sourceLabel ?? null;
    if (!sourceCaseKey || !sourceLabel) return null;

    return {
        sourceBenchmarkId: benchmarkId,
        sourceCaseKey,
        sourceLabel,
        clonedAtIso: input.nowIso ?? new Date().toISOString(),
    };
}

export function shouldAutoClearModifiedState(input: {
    isModified: boolean;
    activeMode: CaseMode;
    referenceProvenance: ReferenceProvenance | null;
    parameterOverrideCount: number;
}): boolean {
    if (!input.isModified) return false;
    if (input.referenceProvenance) return false;
    return Number(input.parameterOverrideCount) === 0;
}

export function shouldAllowReferenceClone(input: {
    activeMode: CaseMode;
    isModified: boolean;
    hasReferenceLibraryCase?: boolean;
}): boolean {
    return input.hasReferenceLibraryCase === true && !input.isModified;
}

export function shouldShowModePanelStatusRow(input: {
    referenceProvenance: ReferenceProvenance | null;
    parameterOverrideCount: number;
}): boolean {
    return !!input.referenceProvenance || Number(input.parameterOverrideCount) > 0;
}

// Analytical status types and evaluator live in warningPolicy.ts — re-exported here for backward compatibility.
export type {
    AnalyticalStatusLevel,
    AnalyticalStatusMode,
    AnalyticalReasonSeverity,
    AnalyticalStatusWarningSeverity,
    AnalyticalStatusReason,
    AnalyticalStatus,
    AnalyticalStatusInput,
} from '../warningPolicy';
export { evaluateAnalyticalStatus } from '../warningPolicy';

function toComparableScalar(value: unknown): unknown {
    if (typeof value === 'number') {
        return Number.isFinite(value) ? Number(value) : value;
    }
    return value;
}

function valuesEqual(base: unknown, current: unknown): boolean {
    if (Array.isArray(base) || Array.isArray(current)) {
        if (!Array.isArray(base) || !Array.isArray(current)) return false;
        if (base.length !== current.length) return false;
        for (let i = 0; i < base.length; i++) {
            if (!valuesEqual(base[i], current[i])) return false;
        }
        return true;
    }

    return Object.is(toComparableScalar(base), toComparableScalar(current));
}

export function buildBasePresetProfile(input: {
    key: string;
    mode: CaseMode;
    toggles: ToggleState;
    isModified: boolean;
    benchmarkId?: string | null;
    benchmarkLabel?: string | null;
    benchmarkScenarioClass?: AnalyticalMethod | null;
    activeLibraryCaseKey?: string | null;
    activeLibraryGroup?: LibraryCaseGroup | null;
}): BasePresetProfile {
    const {
        key,
        mode,
        toggles,
        isModified,
        benchmarkId,
        benchmarkLabel,
        benchmarkScenarioClass,
        activeLibraryCaseKey,
        activeLibraryGroup,
    } = input;
    const isReferenceLibraryCase = activeLibraryGroup === 'literature-reference'
        || activeLibraryGroup === 'internal-reference';

    let source: PresetSource = 'facet';
    if (isReferenceLibraryCase) source = 'reference';
    if (isModified) source = 'custom';

    const label = isReferenceLibraryCase
        ? (benchmarkLabel || 'Reference Preset')
        : `${mode.toUpperCase()} preset`;

    const navigationState = buildScenarioNavigationState({
        activeMode: mode,
        isModified,
        activeCaseKey: key,
        activeLibraryCaseKey,
        activeLibraryGroup,
        benchmarkId: benchmarkId ?? null,
        benchmarkScenarioClass: benchmarkScenarioClass ?? null,
    });

    return {
        key,
        mode,
        source,
        label,
        toggles: { ...toggles },
        benchmarkId: isReferenceLibraryCase ? (benchmarkId ?? null) : null,
        family: navigationState.activeFamily,
        caseSource: navigationState.activeSource,
        libraryCaseKey: navigationState.activeLibraryCaseKey,
        libraryCaseGroup: navigationState.activeLibraryGroup,
        editabilityPolicy: navigationState.editabilityPolicy,
    };
}

export function buildParameterOverrides(input: {
    currentParams: Record<string, unknown>;
    baseParams: Record<string, unknown>;
    trackedKeys?: readonly string[];
}): ParameterOverrides {
    const { currentParams, baseParams, trackedKeys = PHASE2_TRACKED_PARAMETER_KEYS } = input;
    const overrides: ParameterOverrides = {};

    for (const key of trackedKeys) {
        if (!(key in currentParams)) continue;
        if (!(key in baseParams)) continue;

        const currentValue = currentParams[key];
        const baseValue = baseParams[key];

        if (!valuesEqual(baseValue, currentValue)) {
            overrides[key] = {
                base: baseValue,
                current: currentValue,
            };
        }
    }

    return overrides;
}

export function groupParameterOverrides(overrides: ParameterOverrides): ParameterOverrideGroups {
    const grouped: ParameterOverrideGroups = {
        grid: [],
        initial: [],
        fluids: [],
        permeability: [],
        relperm: [],
        wells: [],
        stability: [],
        physics: [],
        analytical: [],
    };

    const keys = new Set(Object.keys(overrides));

    (Object.keys(PHASE2_PARAMETER_GROUPS) as ParameterOverrideGroupKey[]).forEach((group) => {
        PHASE2_PARAMETER_GROUPS[group].forEach((key) => {
            if (keys.has(key)) grouped[group].push(key);
        });
    });

    return grouped;
}

export function buildOverrideResetPlan(input: {
    groupKeys: readonly string[];
    groupedOverrides: Record<string, string[]>;
    overrides: ParameterOverrides;
}): OverrideResetPlanEntry[] {
    const { groupKeys, groupedOverrides, overrides } = input;
    const seen = new Set<string>();
    const plan: OverrideResetPlanEntry[] = [];

    for (const groupKey of groupKeys) {
        const keys = groupedOverrides[groupKey] ?? [];
        for (const key of keys) {
            if (seen.has(key)) continue;
            const entry = overrides[key];
            if (!entry) continue;
            seen.add(key);
            plan.push({
                key,
                base: entry.base,
            });
        }
    }

    return plan;
}

