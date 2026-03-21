import { calculateDepletionAnalyticalProduction } from './analytical/depletionAnalytical';
import { calculateAnalyticalProduction, computeWelgeMetrics } from './analytical/fractionalFlow';
import { buildCreatePayloadFromState } from './buildCreatePayload';
import type {
    BenchmarkBreakthroughCriterion,
    BenchmarkComparisonMetric,
    BenchmarkFamily,
    BenchmarkReferenceDefinition,
    BenchmarkVariant,
} from './catalog/benchmarkCases';
import type { AnalyticalMethod } from './catalog/scenarios';
import type { RateHistoryPoint, SimulatorCreatePayload, SimulatorSnapshot } from './simulator-types';

export type BenchmarkRunSpec = {
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

export type BenchmarkReferencePolicy = {
    analyticalMethod: AnalyticalMethod;
    referenceKind: BenchmarkReferenceDefinition['kind'];
    referenceSource: string;
    referenceLabel: string;
    primaryTruthLabel: string;
    analyticalOverlayRole: 'primary' | 'secondary' | 'not-applicable';
    summary: string;
};

export type BenchmarkComparisonOutputs = {
    referenceCoordinateLabel: string | null;
    finalCoordinateLabel: string | null;
    breakthroughShiftPvi: number | null;
    recoveryDifferenceAtReferenceCoordinate: number | null;
    recoveryDifferenceAtFinalCoordinate: number | null;
    oilRateRelativeErrorAtFinalTime: number | null;
    cumulativeOilRelativeErrorAtFinalTime: number | null;
    pressureDifferenceAtFinalTime: number | null;
    errorSummary: string;
};

export type BenchmarkRunResult = {
    key: string;
    caseKey: string;
    familyKey: string;
    analyticalMethod: AnalyticalMethod;
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
    referencePolicy: BenchmarkReferencePolicy;
    referenceComparison: BenchmarkReferenceComparison;
    comparisonOutputs: BenchmarkComparisonOutputs;
    comparisonMeaning: string;
};

const EMPTY_COMPARISON_OUTPUTS: BenchmarkComparisonOutputs = {
    referenceCoordinateLabel: null,
    finalCoordinateLabel: null,
    breakthroughShiftPvi: null,
    recoveryDifferenceAtReferenceCoordinate: null,
    recoveryDifferenceAtFinalCoordinate: null,
    oilRateRelativeErrorAtFinalTime: null,
    cumulativeOilRelativeErrorAtFinalTime: null,
    pressureDifferenceAtFinalTime: null,
    errorSummary: 'Reference review details are not available yet.',
};

function clonePlainValue(value: unknown): unknown {
    if (Array.isArray(value)) {
        return value.map((item) => clonePlainValue(item));
    }

    if (value instanceof Float64Array) {
        return new Float64Array(value);
    }

    if (value instanceof Float32Array) {
        return new Float32Array(value);
    }

    if (value instanceof Int32Array) {
        return new Int32Array(value);
    }

    if (value instanceof Uint32Array) {
        return new Uint32Array(value);
    }

    if (value instanceof Uint16Array) {
        return new Uint16Array(value);
    }

    if (value instanceof Uint8Array) {
        return new Uint8Array(value);
    }

    if (value && typeof value === 'object') {
        return Object.fromEntries(
            Object.entries(value as Record<string, unknown>).map(([key, entryValue]) => [key, clonePlainValue(entryValue)]),
        );
    }

    return value;
}

function cloneParams(params: Record<string, any>): Record<string, any> {
    return Object.fromEntries(
        Object.entries(params).map(([key, value]) => [key, clonePlainValue(value)]),
    );
}

function cloneRateHistoryPoint(point: RateHistoryPoint): RateHistoryPoint {
    return Object.fromEntries(
        Object.entries(point).map(([key, value]) => [key, clonePlainValue(value)]),
    ) as RateHistoryPoint;
}

function cloneRateHistory(rateHistory: RateHistoryPoint[]): RateHistoryPoint[] {
    return rateHistory.map((point) => cloneRateHistoryPoint(point));
}

function cloneSimulatorSnapshot(snapshot: SimulatorSnapshot | null | undefined): SimulatorSnapshot | null {
    if (!snapshot) return null;

    return {
        grid: {
            pressure: new Float64Array(snapshot.grid.pressure),
            sat_water: new Float64Array(snapshot.grid.sat_water),
            sat_oil: new Float64Array(snapshot.grid.sat_oil),
            sat_gas: new Float64Array(snapshot.grid.sat_gas),
        },
        wells: snapshot.wells.map((well) => Object.fromEntries(
            Object.entries(well).map(([key, value]) => [key, clonePlainValue(value)]),
        ) as typeof well),
        time: snapshot.time,
        rateHistory: Array.isArray(snapshot.rateHistory) ? cloneRateHistory(snapshot.rateHistory) : undefined,
        solverWarning: snapshot.solverWarning ?? null,
        recordHistory: snapshot.recordHistory,
        stepIndex: snapshot.stepIndex,
        profile: snapshot.profile ? { ...snapshot.profile } : undefined,
    };
}

function cloneSimulatorHistory(history: SimulatorSnapshot[]): SimulatorSnapshot[] {
    return history
        .map((snapshot) => cloneSimulatorSnapshot(snapshot))
        .filter((snapshot): snapshot is SimulatorSnapshot => Boolean(snapshot));
}

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

function formatPercent(value: number | null | undefined, digits = 1): string {
    if (!Number.isFinite(value)) return 'n/a';
    return `${(Number(value) * 100).toFixed(digits)}%`;
}

function formatSignedNumber(value: number | null | undefined, digits = 3): string {
    if (!Number.isFinite(value)) return 'n/a';
    const numeric = Number(value);
    return `${numeric >= 0 ? '+' : ''}${numeric.toFixed(digits)}`;
}

function buildReferencePolicy(spec: BenchmarkRunSpec): BenchmarkReferencePolicy {
    if (spec.analyticalMethod === 'buckley-leverett' && spec.reference.kind === 'analytical') {
        return {
            analyticalMethod: spec.analyticalMethod,
            referenceKind: spec.reference.kind,
            referenceSource: spec.reference.source,
            referenceLabel: 'Buckley-Leverett reference solution',
            primaryTruthLabel: 'Reference arrival-PVI comparison',
            analyticalOverlayRole: 'primary',
            summary: 'The Buckley-Leverett reference solution is the primary review baseline for this run.',
        };
    }

    if (spec.analyticalMethod === 'buckley-leverett' && spec.reference.kind === 'numerical-refined') {
        return {
            analyticalMethod: spec.analyticalMethod,
            referenceKind: spec.reference.kind,
            referenceSource: spec.reference.source,
            referenceLabel: 'Refined numerical reference',
            primaryTruthLabel: 'Refined numerical arrival-PVI comparison',
            analyticalOverlayRole: 'secondary',
            summary: 'A refined numerical reference is the primary review baseline; the reference-solution overlay is contextual rather than an equality target.',
        };
    }

    return {
        analyticalMethod: spec.analyticalMethod,
        referenceKind: spec.reference.kind,
        referenceSource: spec.reference.source,
        referenceLabel: 'Depletion reference solution',
        primaryTruthLabel: 'Reference trend comparison',
        analyticalOverlayRole: 'primary',
        summary: 'The depletion reference solution is the primary review baseline for decline, cumulative oil, and pressure diagnostics.',
    };
}

function interpolateSeriesValue(
    xValues: Array<number | null>,
    yValues: Array<number | null>,
    target: number | null,
): number | null {
    if (!Number.isFinite(target)) return null;

    let previousIndex = -1;
    for (let index = 0; index < xValues.length; index += 1) {
        const x = xValues[index];
        const y = yValues[index];
        if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
        if (x === target) return Number(y);
        if ((x as number) > (target as number)) {
            if (previousIndex < 0) return Number(y);
            const x0 = Number(xValues[previousIndex]);
            const y0 = Number(yValues[previousIndex]);
            const x1 = Number(x);
            const y1 = Number(y);
            if (Math.abs(x1 - x0) <= 1e-12) return y1;
            const fraction = ((target as number) - x0) / (x1 - x0);
            return y0 + fraction * (y1 - y0);
        }
        previousIndex = index;
    }

    return previousIndex >= 0 && Number.isFinite(yValues[previousIndex])
        ? Number(yValues[previousIndex])
        : null;
}

function safeRelativeError(measured: number | null, reference: number | null): number | null {
    if (!Number.isFinite(measured) || !Number.isFinite(reference) || Math.abs(reference as number) <= 1e-12) {
        return null;
    }
    return Math.abs(((measured as number) - (reference as number)) / (reference as number));
}

function buildBuckleyLeverettAnalyticalDiagnostics(input: {
    spec: BenchmarkRunSpec;
    rateHistory: RateHistoryPoint[];
    pviSeries: Array<number | null>;
    recoverySeries: Array<number | null>;
    breakthroughPvi: number | null;
    poreVolume: number;
}): {
    referenceComparison: BenchmarkReferenceComparison;
    comparisonOutputs: BenchmarkComparisonOutputs;
} {
    const { spec, rateHistory, pviSeries, recoverySeries, breakthroughPvi, poreVolume } = input;
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

    const analyticalProduction = calculateAnalyticalProduction(
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
        rateHistory.map((point) => toFiniteNumber(point.time, 0)),
        rateHistory.map((point) => Math.max(0, toFiniteNumber(point.total_injection, 0))),
        poreVolume,
    );
    const ooip = getOoip(spec.params);
    const analyticalRecoverySeries = analyticalProduction.map((point) => (
        ooip > 1e-12 ? Math.max(0, Math.min(1, point.cumulativeOil / ooip)) : null
    ));
    const referenceBreakthroughRecovery = interpolateSeriesValue(
        pviSeries,
        recoverySeries,
        reference.breakthroughPvi,
    );
    const analyticalBreakthroughRecovery = interpolateSeriesValue(
        pviSeries,
        analyticalRecoverySeries,
        reference.breakthroughPvi,
    );
    const finalPvi = pviSeries.at(-1) ?? null;
    const simFinalRecovery = interpolateSeriesValue(pviSeries, recoverySeries, finalPvi);
    const analyticalFinalRecovery = interpolateSeriesValue(pviSeries, analyticalRecoverySeries, finalPvi);
    const breakthroughShiftPvi = Number.isFinite(breakthroughPvi) ? (breakthroughPvi as number) - reference.breakthroughPvi : null;

    return {
        referenceComparison: buildComparisonStatus({
            measuredValue: breakthroughPvi,
            referenceValue: reference.breakthroughPvi,
            tolerance: spec.comparisonMetric?.tolerance ?? null,
            referenceKind: spec.reference.kind,
            referenceSource: spec.reference.source,
            metric: spec.comparisonMetric,
        }),
        comparisonOutputs: {
            referenceCoordinateLabel: 'Reference breakthrough PVI',
            finalCoordinateLabel: 'Final simulated PVI',
            breakthroughShiftPvi,
            recoveryDifferenceAtReferenceCoordinate: (
                Number.isFinite(referenceBreakthroughRecovery) && Number.isFinite(analyticalBreakthroughRecovery)
                    ? (referenceBreakthroughRecovery as number) - (analyticalBreakthroughRecovery as number)
                    : null
            ),
            recoveryDifferenceAtFinalCoordinate: (
                Number.isFinite(simFinalRecovery) && Number.isFinite(analyticalFinalRecovery)
                    ? (simFinalRecovery as number) - (analyticalFinalRecovery as number)
                    : null
            ),
            oilRateRelativeErrorAtFinalTime: safeRelativeError(
                Math.max(0, Math.abs(toFiniteNumber(rateHistory.at(-1)?.total_production_oil, 0))),
                analyticalProduction.at(-1)?.oilRate ?? null,
            ),
            cumulativeOilRelativeErrorAtFinalTime: safeRelativeError(
                Number.isFinite(simFinalRecovery) ? simFinalRecovery : null,
                Number.isFinite(analyticalFinalRecovery) ? analyticalFinalRecovery : null,
            ),
            pressureDifferenceAtFinalTime: null,
            errorSummary: `Breakthrough shift ${formatSignedNumber(breakthroughShiftPvi)} PVI; final recovery delta ${formatSignedNumber(
                Number.isFinite(simFinalRecovery) && Number.isFinite(analyticalFinalRecovery)
                    ? (simFinalRecovery as number) - (analyticalFinalRecovery as number)
                    : null,
            )}.`,
        },
    };
}

function buildDepletionAnalyticalDiagnostics(input: {
    spec: BenchmarkRunSpec;
    rateHistory: RateHistoryPoint[];
    recoverySeries: Array<number | null>;
    pressureSeries: Array<number | null>;
}): {
    referenceComparison: BenchmarkReferenceComparison;
    comparisonOutputs: BenchmarkComparisonOutputs;
} {
    const { spec, rateHistory, recoverySeries, pressureSeries } = input;
    const depletionReference = calculateDepletionAnalyticalProduction({
        reservoir: {
            length: toFiniteNumber(spec.params.nx, 1) * toFiniteNumber(spec.params.cellDx, 10),
            area: toFiniteNumber(spec.params.ny, 1)
                * toFiniteNumber(spec.params.cellDy, 10)
                * toFiniteNumber(spec.params.nz, 1)
                * toFiniteNumber(spec.params.cellDz, 1),
            porosity: toFiniteNumber(spec.params.reservoirPorosity ?? spec.params.porosity, 0.2),
        },
        timeHistory: rateHistory.map((point) => toFiniteNumber(point.time, 0)),
        initialSaturation: toFiniteNumber(spec.params.initialSaturation, 0.3),
        nz: toFiniteNumber(spec.params.nz, 1),
        permMode: String(spec.params.permMode ?? 'uniform'),
        uniformPermX: toFiniteNumber(spec.params.uniformPermX, 100),
        uniformPermY: toFiniteNumber(spec.params.uniformPermY ?? spec.params.uniformPermX, 100),
        layerPermsX: Array.isArray(spec.params.layerPermsX) ? spec.params.layerPermsX.map(Number) : [],
        layerPermsY: Array.isArray(spec.params.layerPermsY) ? spec.params.layerPermsY.map(Number) : [],
        cellDx: toFiniteNumber(spec.params.cellDx, 10),
        cellDy: toFiniteNumber(spec.params.cellDy, 10),
        cellDz: toFiniteNumber(spec.params.cellDz, 1),
        wellRadius: toFiniteNumber(spec.params.well_radius, 0.1),
        wellSkin: toFiniteNumber(spec.params.well_skin, 0),
        muO: toFiniteNumber(spec.params.mu_o, 1),
        sWc: toFiniteNumber(spec.params.s_wc, 0.1),
        sOr: toFiniteNumber(spec.params.s_or, 0.1),
        nO: toFiniteNumber(spec.params.n_o, 2),
        c_o: toFiniteNumber(spec.params.c_o, 1e-5),
        c_w: toFiniteNumber(spec.params.c_w, 3e-6),
        cRock: toFiniteNumber(spec.params.rock_compressibility, 1e-6),
        initialPressure: toFiniteNumber(spec.params.initialPressure, 300),
        producerBhp: toFiniteNumber(spec.params.producerBhp, 100),
        depletionRateScale: toFiniteNumber(spec.params.analyticalDepletionRateScale, 1),
        nx: toFiniteNumber(spec.params.nx, 1),
        ny: toFiniteNumber(spec.params.ny, 1),
        producerI: spec.params.producerI != null ? toFiniteNumber(spec.params.producerI, 0) : undefined,
        producerJ: spec.params.producerJ != null ? toFiniteNumber(spec.params.producerJ, 0) : undefined,
    });

    const ooip = getOoip(spec.params);
    const analyticalRecoverySeries = depletionReference.production.map((point) => (
        ooip > 1e-12 ? Math.max(0, Math.min(1, point.cumulativeOil / ooip)) : null
    ));
    const finalTime = rateHistory.at(-1)?.time ?? null;
    const simFinalRecovery = recoverySeries.at(-1) ?? null;
    const analyticalFinalRecovery = analyticalRecoverySeries.at(-1) ?? null;
    const simFinalOilRate = Math.max(0, Math.abs(toFiniteNumber(rateHistory.at(-1)?.total_production_oil, 0)));
    const analyticalFinalOilRate = depletionReference.production.at(-1)?.oilRate ?? null;
    const simFinalPressure = pressureSeries.at(-1) ?? null;
    const analyticalFinalPressure = depletionReference.production.at(-1)?.avgPressure ?? null;

    return {
        referenceComparison: {
            status: 'not-applicable',
            referenceKind: spec.reference.kind,
            referenceSource: spec.reference.source,
            metricKind: null,
            measuredValue: null,
            referenceValue: null,
            relativeError: null,
            tolerance: null,
            summary: 'The depletion reference solution is descriptive and trend-based for this reference family.',
        },
        comparisonOutputs: {
            referenceCoordinateLabel: 'Final simulated time',
            finalCoordinateLabel: Number.isFinite(finalTime) ? `t = ${Number(finalTime).toFixed(2)} d` : 'Final simulated time',
            breakthroughShiftPvi: null,
            recoveryDifferenceAtReferenceCoordinate: null,
            recoveryDifferenceAtFinalCoordinate: (
                Number.isFinite(simFinalRecovery) && Number.isFinite(analyticalFinalRecovery)
                    ? (simFinalRecovery as number) - (analyticalFinalRecovery as number)
                    : null
            ),
            oilRateRelativeErrorAtFinalTime: safeRelativeError(simFinalOilRate, analyticalFinalOilRate),
            cumulativeOilRelativeErrorAtFinalTime: safeRelativeError(simFinalRecovery, analyticalFinalRecovery),
            pressureDifferenceAtFinalTime: (
                Number.isFinite(simFinalPressure) && Number.isFinite(analyticalFinalPressure)
                    ? (simFinalPressure as number) - (analyticalFinalPressure as number)
                    : null
            ),
            errorSummary: `Final oil-rate error ${formatPercent(safeRelativeError(simFinalOilRate, analyticalFinalOilRate))}; final recovery delta ${formatSignedNumber(
                Number.isFinite(simFinalRecovery) && Number.isFinite(analyticalFinalRecovery)
                    ? (simFinalRecovery as number) - (analyticalFinalRecovery as number)
                    : null,
            )}; pressure delta ${formatSignedNumber(
                Number.isFinite(simFinalPressure) && Number.isFinite(analyticalFinalPressure)
                    ? (simFinalPressure as number) - (analyticalFinalPressure as number)
                    : null,
            )} bar.`,
        },
    };
}

function buildPendingNumericalDiagnostics(spec: BenchmarkRunSpec, breakthroughPvi: number | null): {
    referenceComparison: BenchmarkReferenceComparison;
    comparisonOutputs: BenchmarkComparisonOutputs;
} {
    return {
        referenceComparison: {
            status: 'pending-reference',
            referenceKind: spec.reference.kind,
            referenceSource: spec.reference.source,
            metricKind: spec.comparisonMetric?.kind ?? null,
            measuredValue: breakthroughPvi,
            referenceValue: null,
            relativeError: null,
            tolerance: spec.comparisonMetric?.tolerance ?? null,
            summary: 'Waiting for the refined numerical reference run before scoring this reference variant.',
        },
        comparisonOutputs: {
            ...EMPTY_COMPARISON_OUTPUTS,
            referenceCoordinateLabel: 'Reference breakthrough PVI',
            finalCoordinateLabel: 'Shared final PVI',
            errorSummary: 'Reference review details will populate after the refined numerical reference run completes.',
        },
    };
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
            summary: 'Reference declared, but no scored review metric is configured.',
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
            summary: 'Reference comparison is pending because the review metric is not yet measurable.',
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
            analyticalMethod: family.analyticalMethod,
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
            comparisonMeaning: 'Base reference run for the selected family.',
        },
        ...variants.map((variant) => {
            const steps = Math.max(1, Math.round(toFiniteNumber(variant.params.steps, baseSteps)));
            const deltaTDays = Math.max(1e-6, toFiniteNumber(variant.params.delta_t_days, baseDeltaTDays));

            return {
                key: variant.key,
                caseKey: variant.key,
                familyKey: family.key,
                analyticalMethod: family.analyticalMethod,
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
    const rateHistory = Array.isArray(input.rateHistory) ? cloneRateHistory(input.rateHistory) : [];
    const history = Array.isArray(input.history) ? cloneSimulatorHistory(input.history) : [];
    const finalSnapshot = cloneSimulatorSnapshot(input.finalSnapshot ?? history.at(-1) ?? null);
    const watercutThreshold = spec.breakthroughCriterion?.value ?? 0.01;
    const poreVolume = getPoreVolume(spec.params);
    const ooip = getOoip(spec.params);
    const referencePolicy = buildReferencePolicy(spec);

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

    const referenceDetails = spec.analyticalMethod === 'buckley-leverett'
        ? (spec.reference.kind === 'analytical'
            ? buildBuckleyLeverettAnalyticalDiagnostics({
                spec,
                rateHistory,
                pviSeries,
                recoverySeries,
                breakthroughPvi,
                poreVolume,
            })
            : buildPendingNumericalDiagnostics(spec, breakthroughPvi))
        : buildDepletionAnalyticalDiagnostics({
            spec,
            rateHistory,
            recoverySeries,
            pressureSeries,
        });

    return {
        key: spec.key,
        caseKey: spec.caseKey,
        familyKey: spec.familyKey,
        analyticalMethod: spec.analyticalMethod,
        variantKey: spec.variantKey,
        variantLabel: spec.variantLabel,
        label: spec.label,
        description: spec.description,
        params: cloneParams(spec.params),
        rateHistory,
        history,
        finalSnapshot,
        breakthroughPvi,
        breakthroughTime,
        watercutSeries,
        pressureSeries,
        recoverySeries,
        pviSeries,
        referencePolicy,
        referenceComparison: referenceDetails.referenceComparison,
        comparisonOutputs: referenceDetails.comparisonOutputs,
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

        const referenceBreakthroughShift = (
            Number.isFinite(result.breakthroughPvi) && Number.isFinite(numericalReference.breakthroughPvi)
                ? (result.breakthroughPvi as number) - (numericalReference.breakthroughPvi as number)
                : null
        );
        const referenceRecovery = interpolateSeriesValue(
            numericalReference.pviSeries,
            numericalReference.recoverySeries,
            numericalReference.breakthroughPvi,
        );
        const resultRecoveryAtReference = interpolateSeriesValue(
            result.pviSeries,
            result.recoverySeries,
            numericalReference.breakthroughPvi,
        );
        const sharedFinalPvi = Math.min(
            Number(result.pviSeries.at(-1) ?? 0),
            Number(numericalReference.pviSeries.at(-1) ?? 0),
        );
        const resultRecoveryAtSharedFinalPvi = interpolateSeriesValue(
            result.pviSeries,
            result.recoverySeries,
            sharedFinalPvi,
        );
        const referenceRecoveryAtSharedFinalPvi = interpolateSeriesValue(
            numericalReference.pviSeries,
            numericalReference.recoverySeries,
            sharedFinalPvi,
        );
        const updatedReferenceComparison = buildComparisonStatus({
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
        });

        return {
            ...result,
            referenceComparison: updatedReferenceComparison,
            comparisonOutputs: {
                referenceCoordinateLabel: 'Reference breakthrough PVI',
                finalCoordinateLabel: 'Shared final PVI',
                breakthroughShiftPvi: referenceBreakthroughShift,
                recoveryDifferenceAtReferenceCoordinate: (
                    Number.isFinite(resultRecoveryAtReference) && Number.isFinite(referenceRecovery)
                        ? (resultRecoveryAtReference as number) - (referenceRecovery as number)
                        : null
                ),
                recoveryDifferenceAtFinalCoordinate: (
                    Number.isFinite(resultRecoveryAtSharedFinalPvi) && Number.isFinite(referenceRecoveryAtSharedFinalPvi)
                        ? (resultRecoveryAtSharedFinalPvi as number) - (referenceRecoveryAtSharedFinalPvi as number)
                        : null
                ),
                oilRateRelativeErrorAtFinalTime: null,
                cumulativeOilRelativeErrorAtFinalTime: null,
                pressureDifferenceAtFinalTime: null,
                errorSummary: `Breakthrough shift ${formatSignedNumber(referenceBreakthroughShift)} PVI; recovery delta at shared final PVI ${formatSignedNumber(
                    Number.isFinite(resultRecoveryAtSharedFinalPvi) && Number.isFinite(referenceRecoveryAtSharedFinalPvi)
                        ? (resultRecoveryAtSharedFinalPvi as number) - (referenceRecoveryAtSharedFinalPvi as number)
                        : null,
                )}; ${updatedReferenceComparison.summary}`,
            },
        };
    });
}