import type { CaseMode, ToggleState } from '../catalog/caseCatalog';

export type PresetSource = 'facet' | 'benchmark' | 'custom';

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
    | 'library-starter'
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
    benchmarkScenarioClass?: 'buckley-leverett' | 'depletion' | null;
    benchmarkId?: string | null;
}): ProductFamily {
    if (input.activeLibraryFamily) return input.activeLibraryFamily;
    if (input.activeMode === 'wf') return 'waterflood';
    if (input.activeMode === 'sim') return 'scenario-builder';
    if (input.activeMode === 'dep') return 'depletion-analysis';

    if (input.benchmarkScenarioClass === 'buckley-leverett') {
        return 'waterflood';
    }
    if (isFetkovichBenchmarkCase(input.benchmarkId ?? null)) {
        return 'type-curves';
    }
    return 'depletion-analysis';
}

export function resolveScenarioSource(input: {
    isModified: boolean;
}): ScenarioSource {
    return input.isModified ? 'custom' : 'case-library';
}

export function resolveLibraryCaseKey(input: {
    activeMode: CaseMode;
    caseKey: string | null | undefined;
    benchmarkId?: string | null;
    caseSource: ScenarioSource;
}): string | null {
    if (input.caseSource === 'custom') return null;
    if (input.activeMode === 'benchmark') return input.benchmarkId ?? null;
    return input.caseKey ?? null;
}

export function resolveLibraryCaseGroup(input: {
    activeMode: CaseMode;
    benchmarkId?: string | null;
    caseSource: ScenarioSource;
}): LibraryCaseGroup | null {
    if (input.caseSource === 'custom') return null;
    if (input.activeMode === 'benchmark') {
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

    if (
        input.activeMode === 'benchmark'
        || input.activeLibraryGroup === 'literature-reference'
        || input.activeLibraryGroup === 'internal-reference'
    ) {
        return {
            kind: 'library-reference',
            allowDirectInputEditing: false,
            allowSensitivitySelection: true,
            allowCustomizeAction: true,
            transitionsToCustomOnEdit: false,
        };
    }

    return {
        kind: 'library-starter',
        allowDirectInputEditing: true,
        allowSensitivitySelection: false,
        allowCustomizeAction: false,
        transitionsToCustomOnEdit: true,
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
    benchmarkScenarioClass?: 'buckley-leverett' | 'depletion' | null;
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
                activeMode: input.activeMode,
                caseKey: input.activeCaseKey ?? null,
                benchmarkId: input.benchmarkId ?? null,
                caseSource: activeSource,
            });
    const activeLibraryGroup = activeSource === 'custom'
        ? null
        : hasResolvedLibraryGroup
            ? input.activeLibraryGroup ?? null
            : resolveLibraryCaseGroup({
                activeMode: input.activeMode,
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

export type BenchmarkProvenance = {
    sourceBenchmarkId: string;
    sourceCaseKey: string;
    sourceLabel: string;
    clonedAtIso: string;
};

export function buildBenchmarkCloneProvenance(input: {
    benchmarkId: string | null | undefined;
    sourceCaseKey: string | null | undefined;
    sourceLabel: string | null | undefined;
    nowIso?: string;
}): BenchmarkProvenance | null {
    const benchmarkId = input.benchmarkId ?? null;
    const sourceCaseKey = input.sourceCaseKey ?? null;
    const sourceLabel = input.sourceLabel ?? null;
    if (!benchmarkId || !sourceCaseKey || !sourceLabel) return null;

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
    benchmarkProvenance: BenchmarkProvenance | null;
    parameterOverrideCount: number;
}): boolean {
    if (!input.isModified) return false;
    if (input.activeMode === 'benchmark') return false;
    if (input.benchmarkProvenance) return false;
    return Number(input.parameterOverrideCount) === 0;
}

export function shouldAllowBenchmarkClone(input: {
    activeMode: CaseMode;
    isModified: boolean;
    hasReferenceLibraryCase?: boolean;
}): boolean {
    return (input.activeMode === 'benchmark' || input.hasReferenceLibraryCase === true) && !input.isModified;
}

export function shouldShowModePanelStatusRow(input: {
    benchmarkProvenance: BenchmarkProvenance | null;
    parameterOverrideCount: number;
}): boolean {
    return !!input.benchmarkProvenance || Number(input.parameterOverrideCount) > 0;
}

export type AnalyticalStatusLevel = 'reference' | 'approximate' | 'off';

export type AnalyticalStatusMode = 'waterflood' | 'depletion' | 'none';

export type AnalyticalReasonSeverity = 'notice' | 'warning' | 'critical';

export type AnalyticalStatusWarningSeverity = 'none' | AnalyticalReasonSeverity;

export type AnalyticalStatusReason = {
    code: string;
    message: string;
    severity: AnalyticalReasonSeverity;
};

export type AnalyticalStatus = {
    level: AnalyticalStatusLevel;
    mode: AnalyticalStatusMode;
    warningSeverity: AnalyticalStatusWarningSeverity;
    reasonDetails: AnalyticalStatusReason[];
    reasons: string[];
};

export type AnalyticalStatusInput = {
    activeMode: CaseMode;
    analyticalMode: AnalyticalStatusMode;
    injectorEnabled: boolean;
    gravityEnabled: boolean;
    capillaryEnabled: boolean;
    permMode: 'uniform' | 'random' | 'perLayer';
    toggles: ToggleState;
};

const ANALYTICAL_SEVERITY_RANK: Record<AnalyticalStatusWarningSeverity, number> = {
    none: 0,
    notice: 1,
    warning: 2,
    critical: 3,
};

function maxAnalyticalSeverity(
    reasons: readonly AnalyticalStatusReason[],
): AnalyticalStatusWarningSeverity {
    if (!reasons.length) return 'none';
    let max: AnalyticalStatusWarningSeverity = 'none';
    for (const reason of reasons) {
        const severity = reason.severity;
        if (ANALYTICAL_SEVERITY_RANK[severity] > ANALYTICAL_SEVERITY_RANK[max]) {
            max = severity;
        }
    }
    return max;
}

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
    benchmarkScenarioClass?: 'buckley-leverett' | 'depletion' | null;
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
    const isReferenceLibraryCase = mode === 'benchmark'
        || activeLibraryGroup === 'literature-reference'
        || activeLibraryGroup === 'internal-reference';

    let source: PresetSource = 'facet';
    if (isReferenceLibraryCase) source = 'benchmark';
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

export function evaluateAnalyticalStatus(input: AnalyticalStatusInput): AnalyticalStatus {
    const {
        activeMode,
        analyticalMode,
        injectorEnabled,
        gravityEnabled,
        capillaryEnabled,
        permMode,
        toggles,
    } = input;

    if (analyticalMode !== 'waterflood' && analyticalMode !== 'depletion') {
        const reasonDetails: AnalyticalStatusReason[] = [
            {
                code: 'analytical-disabled',
                message: 'Analytical overlay is disabled for this scenario.',
                severity: 'notice',
            },
        ];
        return {
            level: 'off',
            mode: 'none',
            warningSeverity: 'none',
            reasonDetails,
            reasons: reasonDetails.map((r) => r.message),
        };
    }

    const reasonDetails: AnalyticalStatusReason[] = [];

    const addReason = (
        code: string,
        message: string,
        severity: AnalyticalReasonSeverity,
    ) => {
        reasonDetails.push({ code, message, severity });
    };

    if (analyticalMode === 'waterflood') {
        if (!injectorEnabled) {
            addReason(
                'wf-injector-disabled',
                'Injector is disabled for waterflood analytical assumptions.',
                'critical',
            );
        }
        if (toggles.geo !== '1d') {
            addReason(
                'wf-geometry-not-1d',
                'Reference waterflood analytical comparison expects 1D geometry.',
                'warning',
            );
        }
        if (toggles.well !== 'e2e') {
            addReason(
                'wf-well-not-e2e',
                'Reference waterflood analytical comparison expects end-to-end wells.',
                'warning',
            );
        }
    } else {
        if (injectorEnabled) {
            addReason(
                'dep-injector-enabled',
                'Injector is enabled for depletion analytical assumptions.',
                'critical',
            );
        }
        if (!(toggles.geo === '1d' || toggles.well === 'center')) {
            addReason(
                'dep-geometry-well-mismatch',
                'Reference depletion analytical comparison expects 1D or center-producer assumptions.',
                'warning',
            );
        }
    }

    if (permMode !== 'uniform') {
        addReason(
            'perm-nonuniform',
            'Permeability is non-uniform, so analytical match is approximate.',
            'warning',
        );
    }
    if (gravityEnabled) {
        addReason(
            'gravity-enabled',
            'Gravity is enabled, which deviates from reference analytical assumptions.',
            'warning',
        );
    }
    if (capillaryEnabled) {
        addReason(
            'capillary-enabled',
            'Capillary pressure is enabled, which deviates from reference analytical assumptions.',
            'warning',
        );
    }

    if (activeMode === 'sim') {
        addReason(
            'sim-mode-exploratory',
            'Simulation mode is exploratory; analytical overlay is treated as approximate guidance.',
            'notice',
        );
    }

    const warningSeverity = maxAnalyticalSeverity(reasonDetails);

    return {
        level: reasonDetails.length === 0 ? 'reference' : 'approximate',
        mode: analyticalMode,
        warningSeverity,
        reasonDetails,
        reasons: reasonDetails.map((r) => r.message),
    };
}
