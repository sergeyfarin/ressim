type SourceBenchmarkCaseFile = {
    key: string;
    category: string;
    label: string;
    description: string;
    benchmark?: boolean;
    benchmarkOrder?: number;
    params: Record<string, any>;
};

export type BenchmarkEntry = {
    key: string;
    label: string;
    description: string;
    params: Record<string, any>;
};

/**
 * @deprecated Use AnalyticalMethod from scenarios.ts instead. Kept as alias during migration.
 */
export type BenchmarkScenarioClass = 'buckley-leverett' | 'depletion' | 'gas-oil-bl';

export type BenchmarkSensitivityAxisKey =
    | 'grid-refinement'
    | '2d-grid-refinement'
    | 'timestep-refinement'
    | 'heterogeneity';

export type BenchmarkReferenceKind = 'analytical' | 'numerical-refined';

export type BenchmarkXAxisKey = 'time' | 'pvi' | 'tD';

export type BenchmarkPanelKey =
    | 'watercut-breakthrough'
    | 'recovery'
    | 'pressure'
    | 'rates'
    | 'oil-rate'
    | 'cumulative-oil'
    | 'decline-diagnostics';

export type BenchmarkRunPolicy = 'single' | 'sweep' | 'compare-to-reference';

export type BenchmarkReferenceDefinition = {
    kind: BenchmarkReferenceKind;
    source: string;
};

export type BenchmarkBreakthroughCriterion = {
    kind: 'watercut-threshold';
    value: number;
};

export type BenchmarkComparisonMetric = {
    kind: 'breakthrough-pv-relative-error';
    target: 'analytical-reference' | 'numerical-reference';
    tolerance: number;
};

export type BenchmarkDisplayDefaults = {
    xAxis: BenchmarkXAxisKey;
    panels: BenchmarkPanelKey[];
};

export type BenchmarkStylePolicy = {
    colorBy: 'case';
    lineStyleBy: 'quantity-or-reference';
    separatePressurePanel: boolean;
};

export type BenchmarkVariantAnalyticalValidity =
    | 'same-reference'
    | 'numerical-reference-required';

type BenchmarkFamilyDefinition = {
    key: string;
    baseCaseKey: string;
    /** @deprecated Use analyticalMethod instead. */
    scenarioClass: BenchmarkScenarioClass;
    /** Which analytical reference model to use — unified vocabulary with ScenarioCapabilities. */
    analyticalMethod: import('../catalog/scenarios').AnalyticalMethod;
    sensitivityAxes: BenchmarkSensitivityAxisKey[];
    reference: BenchmarkReferenceDefinition;
    breakthroughCriterion?: BenchmarkBreakthroughCriterion;
    comparisonMetric?: BenchmarkComparisonMetric;
    displayDefaults: BenchmarkDisplayDefaults;
    stylePolicy: BenchmarkStylePolicy;
    runPolicy: BenchmarkRunPolicy;
};

export type BenchmarkFamily = BenchmarkFamilyDefinition & {
    label: string;
    description: string;
    baseCase: BenchmarkEntry;
    /** True only for sweep-domain scenarios where E_A/E_V/E_vol panels are physically meaningful. */
    showSweepPanel?: boolean;
    sweepGeometry?: import('../analytical/sweepEfficiency').SweepGeometry | null;
    analyticalOverlayMode?: import('../catalog/scenarios').AnalyticalOverlayMode;
};

type BenchmarkVariantTemplate = {
    variantKey: string;
    axis: BenchmarkSensitivityAxisKey;
    label: string;
    description: string;
    paramsDelta: Record<string, any> | ((baseCase: BenchmarkEntry) => Record<string, any>);
    comparisonMeaning: string;
    analyticalValidity: BenchmarkVariantAnalyticalValidity;
    reference?: BenchmarkReferenceDefinition;
    comparisonMetric?: BenchmarkComparisonMetric;
};

export type BenchmarkVariant = {
    key: string;
    familyKey: string;
    variantKey: string;
    axis: BenchmarkSensitivityAxisKey;
    label: string;
    description: string;
    params: Record<string, any>;
    paramsDelta: Record<string, any>;
    baseCaseKey: string;
    reference: BenchmarkReferenceDefinition;
    comparisonMetric: BenchmarkComparisonMetric | null;
    comparisonMeaning: string;
    analyticalValidity: BenchmarkVariantAnalyticalValidity;
};

export type BenchmarkDimensionOption = {
    value: string;
    label: string;
    default?: boolean;
};

const BENCHMARK_SENSITIVITY_AXIS_LABELS: Record<BenchmarkSensitivityAxisKey, string> = {
    'grid-refinement': 'Grid refinement',
    '2d-grid-refinement': '2D grid refinement',
    'timestep-refinement': 'Timestep refinement',
    heterogeneity: 'Heterogeneity',
};

const caseModules = import.meta.glob('./benchmark-case-data/*.json', {
    eager: true,
    import: 'default',
}) as Record<string, unknown>;

function isBenchmarkCaseFile(value: unknown): value is SourceBenchmarkCaseFile {
    if (!value || typeof value !== 'object') return false;

    const candidate = value as Partial<SourceBenchmarkCaseFile>;
    return (
        candidate.benchmark === true
        && typeof candidate.key === 'string'
        && typeof candidate.label === 'string'
        && typeof candidate.description === 'string'
        && !!candidate.params
        && typeof candidate.params === 'object'
    );
}

const orderedBenchmarkFiles = Object.values(caseModules)
    .filter(isBenchmarkCaseFile)
    .sort((left, right) => {
        const orderDelta = (left.benchmarkOrder ?? Number.MAX_SAFE_INTEGER)
            - (right.benchmarkOrder ?? Number.MAX_SAFE_INTEGER);
        if (orderDelta !== 0) return orderDelta;
        return left.label.localeCompare(right.label);
    });

export const sourceBenchmarkCases: BenchmarkEntry[] = orderedBenchmarkFiles.map((entry) => ({
    key: entry.key,
    label: entry.label,
    description: entry.description,
    params: entry.params,
}));

const benchmarkCaseMap = new Map(sourceBenchmarkCases.map((entry) => [entry.key, entry]));

function buildTimestepVariantStepCount(baseCase: BenchmarkEntry, dtDays: number): number {
    const baseDtDays = Number(baseCase.params.delta_t_days ?? 0.125);
    const baseSteps = Number(baseCase.params.steps ?? 240);
    const horizonDays = baseDtDays * baseSteps;
    return Math.max(1, Math.round(horizonDays / dtDays));
}

function buildBuckleyVariantTemplates(
    familyKey: string,
    tolerance: number,
): BenchmarkVariantTemplate[] {
    const heterogeneityReference: BenchmarkReferenceDefinition = {
        kind: 'numerical-refined',
        source: `${familyKey}:refined-numerical-reference`,
    };

    const heterogeneityMetric: BenchmarkComparisonMetric = {
        kind: 'breakthrough-pv-relative-error',
        target: 'numerical-reference',
        tolerance,
    };

    return [
        {
            variantKey: 'grid_24',
            axis: 'grid-refinement',
            label: 'Grid 24 cells',
            description: 'Coarse 24-cell 1D discretization using the same Rust-parity physics.',
            paramsDelta: {
                nx: 24,
                producerI: 23,
                cellDx: 10 * 96 / 24,
            },
            comparisonMeaning: 'Measure breakthrough-PV discretization error against the same analytical shock reference.',
            analyticalValidity: 'same-reference',
        },
        {
            variantKey: 'grid_48',
            axis: 'grid-refinement',
            label: 'Grid 48 cells',
            description: 'Intermediate 48-cell 1D discretization using the same Rust-parity physics.',
            paramsDelta: {
                nx: 48,
                producerI: 47,
                cellDx: 10 * 96 / 48,
            },
            comparisonMeaning: 'Measure convergence toward the same analytical shock reference as the grid is refined.',
            analyticalValidity: 'same-reference',
        },
        {
            variantKey: 'grid_2',
            axis: '2d-grid-refinement',
            label: 'Grid 2 cells',
            description: '2 layers.',
            paramsDelta: {
                ny: 2,
                cellDy: 10 / 2,
            },
            comparisonMeaning: 'Measure breakthrough-PV discretization error against the same analytical shock reference.',
            analyticalValidity: 'same-reference',
        },
        {
            variantKey: 'grid_3',
            axis: '2d-grid-refinement',
            label: 'Grid 10 cells',
            description: '10 layers.',
            paramsDelta: {
                ny: 10,
                cellDy: 10 / 10,
            },
            comparisonMeaning: 'Measure convergence toward the same analytical shock reference as the grid is refined.',
            analyticalValidity: 'same-reference',
        },
        {
            variantKey: 'dt_0_25',
            axis: 'timestep-refinement',
            label: 'dt = 0.25 days',
            description: 'Coarser timestep using the same physical horizon and Rust-parity BHP controls.',
            paramsDelta: (baseCase) => ({
                delta_t_days: 0.25,
                steps: buildTimestepVariantStepCount(baseCase, 0.25),
            }),
            comparisonMeaning: 'Measure timestep sensitivity at fixed physics and horizon against the same analytical shock reference.',
            analyticalValidity: 'same-reference',
        },
        {
            variantKey: 'dt_0_50',
            axis: 'timestep-refinement',
            label: 'dt = 0.50 days',
            description: 'Coarse timestep using the same physical horizon and Rust-parity BHP controls.',
            paramsDelta: (baseCase) => ({
                delta_t_days: 0.5,
                steps: buildTimestepVariantStepCount(baseCase, 0.5),
            }),
            comparisonMeaning: 'Measure timestep coarsening error against the same analytical shock reference.',
            analyticalValidity: 'same-reference',
        },
        {
            variantKey: 'heterogeneity_mild_random',
            axis: 'heterogeneity',
            label: 'Mild Heterogeneity',
            description: 'Seeded random permeability with the same target mean permeability and modest contrast.',
            paramsDelta: {
                permMode: 'random',
                minPerm: 1500,
                maxPerm: 2500,
                useRandomSeed: true,
                randomSeed: familyKey === 'bl_case_a_refined' ? 4201 : 4202,
            },
            comparisonMeaning: 'Compare heterogeneous displacement against a refined numerical reference, not directly against analytical equality.',
            analyticalValidity: 'numerical-reference-required',
            reference: heterogeneityReference,
            comparisonMetric: heterogeneityMetric,
        },
        {
            variantKey: 'heterogeneity_strong_random',
            axis: 'heterogeneity',
            label: 'Strong Heterogeneity',
            description: 'Seeded random permeability with the same target mean permeability and stronger contrast.',
            paramsDelta: {
                permMode: 'random',
                minPerm: 10,
                maxPerm: 10000,
                useRandomSeed: true,
                randomSeed: familyKey === 'bl_case_a_refined' ? 4301 : 4302,
            },
            comparisonMeaning: 'Stress heterogeneous breakthrough behavior against a refined numerical reference rather than the homogeneous analytical solution.',
            analyticalValidity: 'numerical-reference-required',
            reference: heterogeneityReference,
            comparisonMetric: heterogeneityMetric,
        },
    ];
}

const benchmarkFamilyDefinitions: BenchmarkFamilyDefinition[] = [
    {
        key: 'bl_case_a_refined',
        baseCaseKey: 'bl_case_a_refined',
        scenarioClass: 'buckley-leverett',
        analyticalMethod: 'buckley-leverett',
        sensitivityAxes: ['grid-refinement', '2d-grid-refinement', 'timestep-refinement', 'heterogeneity'],
        reference: {
            kind: 'analytical',
            source: 'buckley-leverett-shock-reference',
        },
        breakthroughCriterion: {
            kind: 'watercut-threshold',
            value: 0.01,
        },
        comparisonMetric: {
            kind: 'breakthrough-pv-relative-error',
            target: 'analytical-reference',
            tolerance: 0.25,
        },
        displayDefaults: {
            xAxis: 'pvi',
            panels: ['watercut-breakthrough', 'recovery', 'pressure'],
        },
        stylePolicy: {
            colorBy: 'case',
            lineStyleBy: 'quantity-or-reference',
            separatePressurePanel: true,
        },
        runPolicy: 'compare-to-reference',
    },
    {
        key: 'bl_case_b_refined',
        baseCaseKey: 'bl_case_b_refined',
        scenarioClass: 'buckley-leverett',
        analyticalMethod: 'buckley-leverett',
        sensitivityAxes: ['grid-refinement', '2d-grid-refinement', 'timestep-refinement', 'heterogeneity'],
        reference: {
            kind: 'analytical',
            source: 'buckley-leverett-shock-reference',
        },
        breakthroughCriterion: {
            kind: 'watercut-threshold',
            value: 0.01,
        },
        comparisonMetric: {
            kind: 'breakthrough-pv-relative-error',
            target: 'analytical-reference',
            tolerance: 0.30,
        },
        displayDefaults: {
            xAxis: 'pvi',
            panels: ['watercut-breakthrough', 'recovery', 'pressure'],
        },
        stylePolicy: {
            colorBy: 'case',
            lineStyleBy: 'quantity-or-reference',
            separatePressurePanel: true,
        },
        runPolicy: 'compare-to-reference',
    },
    {
        key: 'dietz_sq_center',
        baseCaseKey: 'dietz_sq_center',
        scenarioClass: 'depletion',
        analyticalMethod: 'depletion',
        sensitivityAxes: [],
        reference: {
            kind: 'analytical',
            source: 'dietz-shape-factor-reference',
        },
        displayDefaults: {
            xAxis: 'time',
            panels: ['oil-rate', 'cumulative-oil', 'decline-diagnostics'],
        },
        stylePolicy: {
            colorBy: 'case',
            lineStyleBy: 'quantity-or-reference',
            separatePressurePanel: true,
        },
        runPolicy: 'compare-to-reference',
    },
    {
        key: 'dietz_sq_corner',
        baseCaseKey: 'dietz_sq_corner',
        scenarioClass: 'depletion',
        analyticalMethod: 'depletion',
        sensitivityAxes: [],
        reference: {
            kind: 'analytical',
            source: 'dietz-shape-factor-reference',
        },
        displayDefaults: {
            xAxis: 'time',
            panels: ['oil-rate', 'cumulative-oil', 'decline-diagnostics'],
        },
        stylePolicy: {
            colorBy: 'case',
            lineStyleBy: 'quantity-or-reference',
            separatePressurePanel: true,
        },
        runPolicy: 'compare-to-reference',
    },
    {
        key: 'fetkovich_exp',
        baseCaseKey: 'fetkovich_exp',
        scenarioClass: 'depletion',
        analyticalMethod: 'depletion',
        sensitivityAxes: [],
        reference: {
            kind: 'analytical',
            source: 'fetkovich-decline-reference',
        },
        displayDefaults: {
            xAxis: 'time',
            panels: ['oil-rate', 'cumulative-oil', 'decline-diagnostics'],
        },
        stylePolicy: {
            colorBy: 'case',
            lineStyleBy: 'quantity-or-reference',
            separatePressurePanel: true,
        },
        runPolicy: 'compare-to-reference',
    },
];

export const benchmarkFamilies: BenchmarkFamily[] = benchmarkFamilyDefinitions.map((definition) => {
    const baseCase = benchmarkCaseMap.get(definition.baseCaseKey);
    if (!baseCase) {
        throw new Error(`Benchmark family '${definition.key}' references missing case '${definition.baseCaseKey}'`);
    }

    return {
        ...definition,
        label: baseCase.label,
        description: baseCase.description,
        baseCase,
    };
});

const benchmarkFamilyMap = new Map(benchmarkFamilies.map((family) => [family.key, family]));

const benchmarkVariantTemplatesByFamily = new Map<string, BenchmarkVariantTemplate[]>([
    ['bl_case_a_refined', buildBuckleyVariantTemplates('bl_case_a_refined', 0.25)],
    ['bl_case_b_refined', buildBuckleyVariantTemplates('bl_case_b_refined', 0.30)],
]);

function resolveVariantParamsDelta(
    template: BenchmarkVariantTemplate,
    baseCase: BenchmarkEntry,
): Record<string, any> {
    return typeof template.paramsDelta === 'function'
        ? template.paramsDelta(baseCase)
        : template.paramsDelta;
}

export const benchmarkVariants: BenchmarkVariant[] = benchmarkFamilies.flatMap((family) => {
    const templates = benchmarkVariantTemplatesByFamily.get(family.key) ?? [];

    return templates.map((template) => {
        const paramsDelta = resolveVariantParamsDelta(template, family.baseCase);

        return {
            key: `${family.key}__${template.variantKey}`,
            familyKey: family.key,
            variantKey: template.variantKey,
            axis: template.axis,
            label: template.label,
            description: template.description,
            params: {
                ...family.baseCase.params,
                ...paramsDelta,
            },
            paramsDelta,
            baseCaseKey: family.baseCase.key,
            reference: template.reference ?? family.reference,
            comparisonMetric: template.comparisonMetric ?? family.comparisonMetric ?? null,
            comparisonMeaning: template.comparisonMeaning,
            analyticalValidity: template.analyticalValidity,
        };
    });
});

const benchmarkVariantMap = new Map(benchmarkVariants.map((variant) => [variant.key, variant]));
const benchmarkVariantsByFamily = new Map(
    benchmarkFamilies.map((family) => [
        family.key,
        benchmarkVariants.filter((variant) => variant.familyKey === family.key),
    ]),
);

export const benchmarkCases: BenchmarkEntry[] = benchmarkFamilies.map((family) => ({
    key: family.key,
    label: family.label,
    description: family.description,
    params: family.baseCase.params,
}));

const benchmarkEntryMap = new Map(benchmarkCases.map((entry) => [entry.key, entry]));

export function getBenchmarkEntry(key: string | null | undefined): BenchmarkEntry | null {
    if (!key) return null;
    return benchmarkEntryMap.get(key) ?? null;
}

export function getBenchmarkFamily(key: string | null | undefined): BenchmarkFamily | null {
    if (!key) return null;
    return benchmarkFamilyMap.get(key) ?? null;
}

export function getBenchmarkVariantsForFamily(familyKey: string | null | undefined): BenchmarkVariant[] {
    if (!familyKey) return [];
    return benchmarkVariantsByFamily.get(familyKey) ?? [];
}

export function getBenchmarkVariant(key: string | null | undefined): BenchmarkVariant | null {
    if (!key) return null;
    return benchmarkVariantMap.get(key) ?? null;
}

export function getBenchmarkSensitivityAxisLabel(axis: BenchmarkSensitivityAxisKey): string {
    return BENCHMARK_SENSITIVITY_AXIS_LABELS[axis] ?? axis;
}

export function getBenchmarkDimensionOptions(): BenchmarkDimensionOption[] {
    return benchmarkFamilies.map((family, index) => ({
        value: family.key,
        label: family.label,
        default: index === 0,
    }));
}