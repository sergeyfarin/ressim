import {
    buildBenchmarkCreatePayload,
    buildBenchmarkRunResult,
    resolveBenchmarkReferenceComparisons,
} from '../benchmarkRunModel';
import type {
    BenchmarkBreakthroughCriterion,
    BenchmarkComparisonMetric,
    BenchmarkReferenceDefinition,
} from '../catalog/benchmarkCases';
import {
    getScenario,
    getScenarioWithVariantParams,
    type AnalyticalMethod,
    type Scenario,
    type ScenarioTerminationPolicy,
} from '../catalog/scenarios';
import { cloneTerminationPolicy } from '../workers/terminationPolicy';
import type { RateHistoryPoint, SimulatorCreatePayload, SimulatorSnapshot } from '../simulator-types';

export type ReferenceSourceType =
    | 'analytical'
    | 'published-reference'
    | 'opm-flow-precomputed'
    | 'simulation';

export type ScenarioReferenceSource = {
    type: ReferenceSourceType;
    source: string;
    label: string;
};

export type RunSpec = {
    key: string;
    caseKey: string;
    familyKey: string;
    analyticalMethod: AnalyticalMethod;
    variantKey: string | null;
    variantLabel: string | null;
    label: string;
    description: string;
    params: Record<string, any>;
    steps: number;
    deltaTDays: number;
    historyInterval: number;
    reference: BenchmarkReferenceDefinition;
    referenceSource: ScenarioReferenceSource;
    comparisonMetric: BenchmarkComparisonMetric | null;
    breakthroughCriterion: BenchmarkBreakthroughCriterion | null;
    terminationPolicy?: ScenarioTerminationPolicy | null;
    comparisonMeaning: string;
};

export type RunResult = ReturnType<typeof buildBenchmarkRunResult> & {
    referenceSource?: ScenarioReferenceSource;
};

export type ScenarioRunPolicy = {
    steps: number;
    deltaTDays: number;
    historyInterval: number;
    terminationPolicy: ScenarioTerminationPolicy | null;
};

function toFiniteNumber(value: unknown, fallback: number): number {
    const numeric = Number(value);
    return Number.isFinite(numeric) ? numeric : fallback;
}

function getReferenceForScenario(scenario: Scenario): {
    benchmarkReference: BenchmarkReferenceDefinition;
    source: ScenarioReferenceSource;
} {
    const method = scenario.capabilities.analyticalMethod;
    if (method === 'digitized-reference') {
        const source = `${scenario.key}:published-reference`;
        return {
            benchmarkReference: { kind: 'analytical', source },
            source: {
                type: 'published-reference',
                source,
                label: 'Published reference',
            },
        };
    }

    const source = method === 'none' ? `${scenario.key}:simulation` : `${scenario.key}:analytical`;
    return {
        benchmarkReference: { kind: 'analytical', source },
        source: {
            type: method === 'none' ? 'simulation' : 'analytical',
            source,
            label: method === 'none' ? 'Simulation baseline' : 'Analytical reference',
        },
    };
}

export function buildScenarioRunPolicy(input: {
    params: Record<string, any>;
    baseParams?: Record<string, any>;
    stepsOverride?: number | null;
    deltaTDaysOverride?: number | null;
    terminationPolicy?: ScenarioTerminationPolicy | null;
}): ScenarioRunPolicy {
    const params = input.params;
    const baseParams = input.baseParams ?? params;
    const steps = Math.max(
        1,
        Math.round(toFiniteNumber(input.stepsOverride ?? params.steps ?? baseParams.steps, 240)),
    );
    const deltaTDays = Math.max(
        1e-9,
        toFiniteNumber(input.deltaTDaysOverride ?? params.delta_t_days ?? baseParams.delta_t_days, 0.125),
    );
    return {
        steps,
        deltaTDays,
        historyInterval: Math.max(1, Math.ceil(steps / 25)),
        terminationPolicy: cloneTerminationPolicy(input.terminationPolicy) ?? null,
    };
}

export function buildScenarioRunSpecs(input: {
    scenarioKey: string;
    dimensionKey: string | null | undefined;
    variantKeys: string[];
    stepsOverride?: number | null;
    deltaTDaysOverride?: number | null;
}): RunSpec[] {
    const scenario = getScenario(input.scenarioKey);
    if (!scenario) return [];

    const dimension = input.dimensionKey
        ? scenario.sensitivities.find((candidate) => candidate.key === input.dimensionKey)
        : null;
    if (!dimension) return [];

    const reference = getReferenceForScenario(scenario);
    const specs: RunSpec[] = [];

    for (const variantKey of input.variantKeys) {
        const variant = dimension.variants.find((candidate) => candidate.key === variantKey);
        if (!variant) continue;
        const params = getScenarioWithVariantParams(scenario.key, dimension.key, variant.key);
        const runPolicy = buildScenarioRunPolicy({
            params,
            baseParams: scenario.params,
            stepsOverride: input.stepsOverride,
            deltaTDaysOverride: input.deltaTDaysOverride,
            terminationPolicy: scenario.terminationPolicy,
        });
        specs.push({
            key: `${scenario.key}__${dimension.key}__${variant.key}`,
            caseKey: scenario.key,
            familyKey: scenario.key,
            analyticalMethod: scenario.capabilities.analyticalMethod,
            variantKey: variant.key,
            variantLabel: variant.label,
            label: `${scenario.label} — ${variant.label}`,
            description: variant.description,
            params,
            steps: runPolicy.steps,
            deltaTDays: runPolicy.deltaTDays,
            historyInterval: runPolicy.historyInterval,
            reference: reference.benchmarkReference,
            referenceSource: reference.source,
            comparisonMetric: null,
            breakthroughCriterion: null,
            terminationPolicy: runPolicy.terminationPolicy,
            comparisonMeaning: variant.description,
        });
    }

    return specs;
}

export function buildCreatePayloadForRun(spec: RunSpec): SimulatorCreatePayload {
    return buildBenchmarkCreatePayload({
        ...spec.params,
        terminationPolicy: spec.terminationPolicy ?? undefined,
    });
}

export function buildRunResult(input: {
    spec: RunSpec;
    rateHistory: RateHistoryPoint[];
    history?: SimulatorSnapshot[];
    finalSnapshot?: SimulatorSnapshot | null;
}): RunResult {
    const result = buildBenchmarkRunResult(input as Parameters<typeof buildBenchmarkRunResult>[0]) as RunResult;
    return {
        ...result,
        referenceSource: input.spec.referenceSource,
    };
}

export function resolveRunComparisons(results: RunResult[]): RunResult[] {
    return resolveBenchmarkReferenceComparisons(results) as RunResult[];
}
