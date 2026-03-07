type PublicCaseFile = {
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

export type BenchmarkDimensionOption = {
    value: string;
    label: string;
    default?: boolean;
};

const caseModules = import.meta.glob('../../../public/cases/*.json', {
    eager: true,
    import: 'default',
}) as Record<string, unknown>;

function isBenchmarkCaseFile(value: unknown): value is PublicCaseFile {
    if (!value || typeof value !== 'object') return false;

    const candidate = value as Partial<PublicCaseFile>;
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

export const benchmarkCases: BenchmarkEntry[] = orderedBenchmarkFiles.map((entry) => ({
    key: entry.key,
    label: entry.label,
    description: entry.description,
    params: entry.params,
}));

const benchmarkCaseMap = new Map(benchmarkCases.map((entry) => [entry.key, entry]));

export function getBenchmarkEntry(key: string | null | undefined): BenchmarkEntry | null {
    if (!key) return null;
    return benchmarkCaseMap.get(key) ?? null;
}

export function getBenchmarkDimensionOptions(): BenchmarkDimensionOption[] {
    return benchmarkCases.map((entry, index) => ({
        value: entry.key,
        label: entry.label,
        default: index === 0,
    }));
}