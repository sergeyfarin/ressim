import { computeWelgeMetrics } from './analytical/fractionalFlow';
import { buildCreatePayloadFromState } from './buildCreatePayload';
import type {
    BenchmarkBreakthroughCriterion,
    BenchmarkComparisonMetric,
    BenchmarkFamily,
    BenchmarkReferenceDefinition,
    BenchmarkVariant,
} from './catalog/benchmarkCases';
import type { RateHistoryPoint, SimulatorCreatePayload, SimulatorSnapshot } from './simulator-types';

export type BenchmarkRunSpec = {
    key: string;
    caseKey: string;
    familyKey: string;
    variantKey: string | null;
    variantLabel: string | null;
    label: string;
    description: string;
    params: Record<string, any>;
    steps: number;
    deltaTDays: number;
    historyInterval: number;
    reference: BenchmarkReferenceDefinition;
    comparisonMetric: BenchmarkComparisonMetric | null;
    breakthroughCriterion: BenchmarkBreakthroughCriterion | null;
    comparisonMeaning: string;
};

export type BenchmarkReferenceComparisonStatus =
    | 'not-applicable'
    | 'pending-reference'
    | 'within-tolerance'
    | 'outside-tolerance';

export type BenchmarkReferenceComparison = {
    status: BenchmarkReferenceComparisonStatus;
    referenceKind: BenchmarkReferenceDefinition['kind'] | null;
    referenceSource: string | null;
    metricKind: BenchmarkComparisonMetric['kind'] | null;
    measuredValue: number | null;
    referenceValue: number | null;
    relativeError: number | null;
    tolerance: number | null;
    summary: string;
};

export type BenchmarkRunResult = {
    key: string;
    caseKey: string;
    familyKey: string;
    variantKey: string | null;
    variantLabel: string | null;
    label: string;
    description: string;
    params: Record<string, any>;
    rateHistory: RateHistoryPoint[];
    history: SimulatorSnapshot[];
    finalSnapshot: SimulatorSnapshot | null;
    breakthroughPvi: number | null;
    breakthroughTime: number | null;
    watercutSeries: Array<number | null>;
    pressureSeries: Array<number | null>;
    recoverySeries: Array<number | null>;
    pviSeries: Array<number | null>;
    referenceComparison: BenchmarkReferenceComparison;
    comparisonMeaning: string;
};

function toFiniteNumber(value: unknown, fallback: number): number {
    const numeric = Number(value);
    return Number.isFinite(numeric) ? numeric : fallback;
}

function getPoreVolume(params: Record<string, any>): number {
    return toFiniteNumber(params.nx, 1)
        * toFiniteNumber(params.ny, 1)
        * toFiniteNumber(params.nz, 1)
        * toFiniteNumber(params.cellDx, 10)
        * toFiniteNumber(params.cellDy, 10)
        * toFiniteNumber(params.cellDz, 1)
        * toFiniteNumber(params.reservoirPorosity ?? params.porosity, 0.2);
}

function getOoip(params: Record<string, any>): number {
    const poreVolume = getPoreVolume(params);
    const initialSaturation = toFiniteNumber(params.initialSaturation, 0.3);
    return poreVolume * Math.max(0, 1 - initialSaturation);
}

function buildComparisonStatus(input: {
    measuredValue: number | null;
    referenceValue: number | null;
    tolerance: number | null;
    referenceKind: BenchmarkReferenceDefinition['kind'];
    referenceSource: string;
    metric: BenchmarkComparisonMetric | null;
}): BenchmarkReferenceComparison {
    const { measuredValue, referenceValue, tolerance, referenceKind, referenceSource, metric } = input;

    if (!metric) {
        return {
            status: 'not-applicable',
            referenceKind,
            referenceSource,
            metricKind: null,
            measuredValue,
            referenceValue,
            relativeError: null,
            tolerance: null,
            summary: 'Reference declared, but no scored comparison metric is configured.',
        };
    }

    if (!Number.isFinite(measuredValue) || !Number.isFinite(referenceValue) || Math.abs(referenceValue ?? 0) <= 1e-12) {
        return {
            status: 'pending-reference',
            referenceKind,
            referenceSource,
            metricKind: metric.kind,
            measuredValue,
            referenceValue,
            relativeError: null,
            tolerance: metric.tolerance,
            summary: 'Reference comparison is pending because the benchmark metric is not yet measurable.',
        };
    }

    const relativeError = Math.abs(((measuredValue as number) - (referenceValue as number)) / (referenceValue as number));
    const withinTolerance = relativeError <= metric.tolerance;

    return {
        status: withinTolerance ? 'within-tolerance' : 'outside-tolerance',
        referenceKind,
        referenceSource,
        metricKind: metric.kind,
        measuredValue,
        referenceValue,
        relativeError,
        tolerance: metric.tolerance,
        summary: withinTolerance
            ? `Relative error ${(relativeError * 100).toFixed(1)}% is within tolerance ${(metric.tolerance * 100).toFixed(1)}%.`
            : `Relative error ${(relativeError * 100).toFixed(1)}% exceeds tolerance ${(metric.tolerance * 100).toFixed(1)}%.`,
    };
}

function buildInitialReferenceComparison(spec: BenchmarkRunSpec, breakthroughPvi: number | null): BenchmarkReferenceComparison {
    if (spec.reference.kind === 'analytical') {
        const reference = computeWelgeMetrics(
            {
                s_wc: toFiniteNumber(spec.params.s_wc, 0.1),
                s_or: toFiniteNumber(spec.params.s_or, 0.1),
                n_w: toFiniteNumber(spec.params.n_w, 2),
                n_o: toFiniteNumber(spec.params.n_o, 2),
                k_rw_max: toFiniteNumber(spec.params.k_rw_max, 1),
                k_ro_max: toFiniteNumber(spec.params.k_ro_max, 1),
            },
            {
                mu_w: toFiniteNumber(spec.params.mu_w, 0.5),
                mu_o: toFiniteNumber(spec.params.mu_o, 1),
            },
            toFiniteNumber(spec.params.initialSaturation, toFiniteNumber(spec.params.s_wc, 0.1)),
        );

        return buildComparisonStatus({
            measuredValue: breakthroughPvi,
            referenceValue: reference.breakthroughPvi,
            tolerance: spec.comparisonMetric?.tolerance ?? null,
            referenceKind: spec.reference.kind,
            referenceSource: spec.reference.source,
            metric: spec.comparisonMetric,
        });
    }

    return {
        status: 'pending-reference',
        referenceKind: spec.reference.kind,
        referenceSource: spec.reference.source,
        metricKind: spec.comparisonMetric?.kind ?? null,
        measuredValue: breakthroughPvi,
        referenceValue: null,
        relativeError: null,
        tolerance: spec.comparisonMetric?.tolerance ?? null,
        summary: 'Waiting for a numerical reference run to resolve the benchmark comparison.',
    };
}

export function buildBenchmarkRunSpecs(
    family: BenchmarkFamily,
    variants: BenchmarkVariant[] = [],
): BenchmarkRunSpec[] {
    const baseParams = family.baseCase.params;
    const baseSteps = Math.max(1, Math.round(toFiniteNumber(baseParams.steps, 1)));
    const baseDeltaTDays = Math.max(1e-6, toFiniteNumber(baseParams.delta_t_days, 0.25));

    return [
        {
            key: family.baseCase.key,
            caseKey: family.baseCase.key,
            familyKey: family.key,
            variantKey: null,
            variantLabel: null,
            label: family.label,
            description: family.description,
            params: family.baseCase.params,
            steps: baseSteps,
            deltaTDays: baseDeltaTDays,
            historyInterval: Math.max(1, Math.ceil(baseSteps / 50)),
            reference: family.reference,
            comparisonMetric: family.comparisonMetric ?? null,
            breakthroughCriterion: family.breakthroughCriterion ?? null,
            comparisonMeaning: 'Base benchmark run for the selected benchmark family.',
        },
        ...variants.map((variant) => {
            const steps = Math.max(1, Math.round(toFiniteNumber(variant.params.steps, baseSteps)));
            const deltaTDays = Math.max(1e-6, toFiniteNumber(variant.params.delta_t_days, baseDeltaTDays));

            return {
                key: variant.key,
                caseKey: variant.key,
                familyKey: family.key,
                variantKey: variant.variantKey,
                variantLabel: variant.label,
                label: variant.label,
                description: variant.description,
                params: variant.params,
                steps,
                deltaTDays,
                historyInterval: Math.max(1, Math.ceil(steps / 50)),
                reference: variant.reference,
                comparisonMetric: variant.comparisonMetric,
                breakthroughCriterion: family.breakthroughCriterion ?? null,
                comparisonMeaning: variant.comparisonMeaning,
            };
        }),
    ];
}

export function buildBenchmarkCreatePayload(params: Record<string, any>): SimulatorCreatePayload {
    return buildCreatePayloadFromState({
        ...params,
        porosity: toFiniteNumber(params.reservoirPorosity ?? params.porosity, 0.2),
        rateControlledWells: params.rateControlledWells
            ?? (String(params.injectorControlMode ?? 'pressure') === 'rate'
                && String(params.producerControlMode ?? 'pressure') === 'rate'),
    });
}

export function buildBenchmarkRunResult(input: {
    spec: BenchmarkRunSpec;
    rateHistory: RateHistoryPoint[];
    history?: SimulatorSnapshot[];
    finalSnapshot?: SimulatorSnapshot | null;
}): BenchmarkRunResult {
    const { spec } = input;
    const rateHistory = Array.isArray(input.rateHistory) ? [...input.rateHistory] : [];
    const history = Array.isArray(input.history) ? [...input.history] : [];
    const finalSnapshot = input.finalSnapshot ?? history.at(-1) ?? null;
    const watercutThreshold = spec.breakthroughCriterion?.value ?? 0.01;
    const poreVolume = getPoreVolume(spec.params);
    const ooip = getOoip(spec.params);

    let cumulativeInjection = 0;
    let cumulativeOil = 0;
    let previousTime = 0;
    let breakthroughPvi: number | null = null;
    let breakthroughTime: number | null = null;

    const watercutSeries: Array<number | null> = [];
    const pressureSeries: Array<number | null> = [];
    const recoverySeries: Array<number | null> = [];
    const pviSeries: Array<number | null> = [];

    for (const point of rateHistory) {
        const time = Math.max(0, toFiniteNumber(point.time, previousTime));
        const dt = Math.max(0, time - previousTime);
        previousTime = time;

        const oilRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0)));
        const liquidRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_liquid, 0)));
        const waterRate = Math.max(0, liquidRate - oilRate);
        const injectionRate = Math.max(0, toFiniteNumber(point.total_injection, 0));

        cumulativeInjection += injectionRate * dt;
        cumulativeOil += oilRate * dt;

        const watercut = liquidRate > 1e-12 ? Math.max(0, Math.min(1, waterRate / liquidRate)) : 0;
        const pvi = poreVolume > 1e-12 ? cumulativeInjection / poreVolume : null;
        const recovery = ooip > 1e-12 ? Math.max(0, Math.min(1, cumulativeOil / ooip)) : null;
        const pressure = [
            point.avg_reservoir_pressure,
            point.avg_pressure,
            (point as Record<string, unknown>).average_reservoir_pressure,
        ].find((value) => Number.isFinite(value)) as number | undefined;

        watercutSeries.push(watercut);
        pressureSeries.push(Number.isFinite(pressure) ? Number(pressure) : null);
        recoverySeries.push(recovery);
        pviSeries.push(pvi);

        if (breakthroughPvi === null && watercut >= watercutThreshold) {
            breakthroughPvi = pvi;
            breakthroughTime = time;
        }
    }

    return {
        key: spec.key,
        caseKey: spec.caseKey,
        familyKey: spec.familyKey,
        variantKey: spec.variantKey,
        variantLabel: spec.variantLabel,
        label: spec.label,
        description: spec.description,
        params: spec.params,
        rateHistory,
        history,
        finalSnapshot,
        breakthroughPvi,
        breakthroughTime,
        watercutSeries,
        pressureSeries,
        recoverySeries,
        pviSeries,
        referenceComparison: buildInitialReferenceComparison(spec, breakthroughPvi),
        comparisonMeaning: spec.comparisonMeaning,
    };
}

export function resolveBenchmarkReferenceComparisons(results: BenchmarkRunResult[]): BenchmarkRunResult[] {
    const numericalReferenceMap = new Map<string, BenchmarkRunResult>();

    for (const result of results) {
        if (result.variantKey === null) {
            numericalReferenceMap.set(`${result.familyKey}:refined-numerical-reference`, result);
        }
    }

    return results.map((result) => {
        if (result.referenceComparison.status !== 'pending-reference') return result;
        if (result.referenceComparison.referenceKind !== 'numerical-refined') return result;

        const numericalReference = numericalReferenceMap.get(result.referenceComparison.referenceSource ?? '');
        if (!numericalReference) return result;

        return {
            ...result,
            referenceComparison: buildComparisonStatus({
                measuredValue: result.breakthroughPvi,
                referenceValue: numericalReference.breakthroughPvi,
                tolerance: result.referenceComparison.tolerance,
                referenceKind: 'numerical-refined',
                referenceSource: result.referenceComparison.referenceSource ?? numericalReference.key,
                metric: result.referenceComparison.metricKind
                    ? {
                        kind: result.referenceComparison.metricKind,
                        target: 'numerical-reference',
                        tolerance: result.referenceComparison.tolerance ?? 0,
                    }
                    : null,
            }),
        };
    });
}