import type { CaseMode, ToggleState } from '../caseCatalog';

export type PresetSource = 'facet' | 'benchmark' | 'custom';

export type BasePresetProfile = {
    key: string;
    mode: CaseMode;
    source: PresetSource;
    label: string;
    toggles: ToggleState;
    benchmarkId: string | null;
};

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
}): boolean {
    return input.activeMode === 'benchmark' && !input.isModified;
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
    benchmarkLabel?: string | null;
}): BasePresetProfile {
    const { key, mode, toggles, isModified, benchmarkLabel } = input;
    const isBenchmark = mode === 'benchmark';

    let source: PresetSource = 'facet';
    if (isBenchmark) source = 'benchmark';
    if (isModified) source = 'custom';

    const label = isBenchmark
        ? (benchmarkLabel || 'Benchmark Preset')
        : `${mode.toUpperCase()} preset`;

    return {
        key,
        mode,
        source,
        label,
        toggles: { ...toggles },
        benchmarkId: isBenchmark ? (toggles.benchmarkId ?? null) : null,
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
