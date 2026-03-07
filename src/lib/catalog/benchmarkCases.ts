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

export type BenchmarkScenarioClass = 'buckley-leverett' | 'depletion';

export type BenchmarkSensitivityAxisKey =
    | 'grid-refinement'
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

type BenchmarkFamilyDefinition = {
    key: string;
    baseCaseKey: string;
    scenarioClass: BenchmarkScenarioClass;
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
};

export type BenchmarkDimensionOption = {
    value: string;
    label: string;
    default?: boolean;
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

const benchmarkFamilyDefinitions: BenchmarkFamilyDefinition[] = [
    {
        key: 'bl_case_a_refined',
        baseCaseKey: 'bl_case_a_refined',
        scenarioClass: 'buckley-leverett',
        sensitivityAxes: ['grid-refinement', 'timestep-refinement', 'heterogeneity'],
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
        sensitivityAxes: ['grid-refinement', 'timestep-refinement', 'heterogeneity'],
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

export function getBenchmarkDimensionOptions(): BenchmarkDimensionOption[] {
    return benchmarkFamilies.map((family, index) => ({
        value: family.key,
        label: family.label,
        default: index === 0,
    }));
}