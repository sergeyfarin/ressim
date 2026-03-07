import type { BenchmarkFamily } from '../catalog/benchmarkCases';
import type {
    BenchmarkReferenceComparisonStatus,
    BenchmarkRunResult,
} from '../benchmarkRunModel';

export type OutputSummaryTone = 'default' | 'positive' | 'warning';

export type OutputSummaryItem = {
    label: string;
    value: string;
    detail: string;
    tone?: OutputSummaryTone;
};

type LiveMismatchSummary = {
    pointsCompared: number;
    mae: number;
    rmse: number;
    mape: number;
};

function toFiniteNumber(value: unknown): number | null {
    const numeric = Number(value);
    return Number.isFinite(numeric) ? numeric : null;
}

function lastFinite(values: Array<number | null | undefined>): number | null {
    for (let index = values.length - 1; index >= 0; index -= 1) {
        const numeric = toFiniteNumber(values[index]);
        if (numeric !== null) return numeric;
    }
    return null;
}

function formatNumber(value: number | null | undefined, digits = 1): string {
    if (!Number.isFinite(value)) return 'n/a';

    const numeric = Number(value);
    if (Math.abs(numeric - Math.round(numeric)) <= 1e-9) {
        return String(Math.round(numeric));
    }

    return numeric
        .toFixed(digits)
        .replace(/\.0+$/, '')
        .replace(/(\.\d*?)0+$/, '$1');
}

function formatPercent(value: number | null | undefined, digits = 1): string {
    if (!Number.isFinite(value)) return 'n/a';
    return `${(Number(value) * 100).toFixed(digits)}%`;
}

function formatSigned(value: number | null | undefined, digits = 2): string {
    if (!Number.isFinite(value)) return 'n/a';

    const numeric = Number(value);
    return `${numeric >= 0 ? '+' : ''}${formatNumber(numeric, digits)}`;
}

function getOoip(params: Record<string, any>): number {
    const nx = toFiniteNumber(params.nx) ?? 1;
    const ny = toFiniteNumber(params.ny) ?? 1;
    const nz = toFiniteNumber(params.nz) ?? 1;
    const cellDx = toFiniteNumber(params.cellDx) ?? 10;
    const cellDy = toFiniteNumber(params.cellDy) ?? 10;
    const cellDz = toFiniteNumber(params.cellDz) ?? 1;
    const porosity = toFiniteNumber(params.reservoirPorosity ?? params.porosity) ?? 0.2;
    const initialSaturation = toFiniteNumber(params.initialSaturation) ?? 0.3;

    return nx * ny * nz * cellDx * cellDy * cellDz * porosity * Math.max(0, 1 - initialSaturation);
}

function detectLiveScenarioLabel(activeMode: string, activeCase: string): 'Waterflood' | 'Depletion Analysis' {
    const mode = activeMode.toLowerCase();
    const activeCaseLower = activeCase.toLowerCase();

    if (
        mode === 'waterflood' ||
        activeCaseLower.includes('waterflood') ||
        activeCaseLower.startsWith('bl_')
    ) {
        return 'Waterflood';
    }

    return 'Depletion Analysis';
}

function formatComparisonStatus(status: BenchmarkReferenceComparisonStatus): string {
    if (status === 'within-tolerance') return 'Within tolerance';
    if (status === 'outside-tolerance') return 'Outside tolerance';
    if (status === 'pending-reference') return 'Pending reference';
    return 'Trend-based review';
}

function getComparisonTone(status: BenchmarkReferenceComparisonStatus): OutputSummaryTone {
    if (status === 'within-tolerance') return 'positive';
    if (status === 'outside-tolerance' || status === 'pending-reference') return 'warning';
    return 'default';
}

function buildPrimaryReviewValue(result: BenchmarkRunResult, family: BenchmarkFamily | null): string {
    if (family?.scenarioClass === 'buckley-leverett') {
        return Number.isFinite(result.breakthroughPvi)
            ? `${formatNumber(result.breakthroughPvi, 3)} PVI`
            : 'n/a';
    }

    if (Number.isFinite(result.comparisonOutputs.oilRateRelativeErrorAtFinalTime)) {
        return formatPercent(result.comparisonOutputs.oilRateRelativeErrorAtFinalTime, 1);
    }

    return formatComparisonStatus(result.referenceComparison.status);
}

function buildPrimaryReviewDetail(result: BenchmarkRunResult, family: BenchmarkFamily | null): string {
    if (family?.scenarioClass === 'buckley-leverett') {
        const parts = [
            Number.isFinite(result.referenceComparison.referenceValue)
                ? `Ref ${formatNumber(result.referenceComparison.referenceValue, 3)} PVI`
                : null,
            Number.isFinite(result.comparisonOutputs.breakthroughShiftPvi)
                ? `Shift ${formatSigned(result.comparisonOutputs.breakthroughShiftPvi, 3)} PVI`
                : null,
            Number.isFinite(result.referenceComparison.relativeError)
                ? `${formatPercent(result.referenceComparison.relativeError, 1)} error`
                : null,
        ].filter(Boolean);

        if (parts.length > 0) return parts.join(' · ');
    }

    if (result.comparisonOutputs.errorSummary) return result.comparisonOutputs.errorSummary;
    return result.referenceComparison.summary;
}

export function buildLiveOutputSummaryItems(input: {
    activeMode: string;
    activeCase: string;
    timeValues: Array<number | null | undefined>;
    pviSeries: Array<number | null | undefined>;
    oilRateSeries: Array<number | null | undefined>;
    waterCutSeries: Array<number | null | undefined>;
    cumulativeOilSeries: Array<number | null | undefined>;
    recoverySeries: Array<number | null | undefined>;
    pressureSeries: Array<number | null | undefined>;
    mismatchSummary: LiveMismatchSummary;
}): OutputSummaryItem[] {
    const scenarioLabel = detectLiveScenarioLabel(input.activeMode, input.activeCase);
    const finalTime = lastFinite(input.timeValues);
    const finalPvi = lastFinite(input.pviSeries);
    const finalOilRate = lastFinite(input.oilRateSeries);
    const finalWaterCut = lastFinite(input.waterCutSeries);
    const finalRecovery = lastFinite(input.recoverySeries);
    const finalCumOil = lastFinite(input.cumulativeOilSeries);
    const finalPressure = lastFinite(input.pressureSeries);
    const reviewDetail = input.mismatchSummary.pointsCompared > 0
        ? `Reference MAPE ${input.mismatchSummary.mape.toFixed(1)}%`
        : 'Live outputs only';

    return [
        {
            label: 'Run Context',
            value: scenarioLabel,
            detail: Number.isFinite(finalTime)
                ? `Latest time ${formatNumber(finalTime, 1)} d`
                : 'Run history not available yet',
        },
        {
            label: 'Primary Output',
            value: scenarioLabel === 'Waterflood'
                ? formatPercent(finalWaterCut, 1)
                : `${formatNumber(finalOilRate, 1)} m3/d`,
            detail: scenarioLabel === 'Waterflood'
                ? [
                    Number.isFinite(finalPvi) ? `Final PVI ${formatNumber(finalPvi, 3)}` : null,
                    reviewDetail,
                ].filter(Boolean).join(' · ')
                : [
                    Number.isFinite(finalTime) ? `Latest rate at ${formatNumber(finalTime, 1)} d` : null,
                    reviewDetail,
                ].filter(Boolean).join(' · '),
        },
        {
            label: 'Recovery',
            value: formatPercent(finalRecovery, 1),
            detail: Number.isFinite(finalCumOil)
                ? `Cum oil ${formatNumber(finalCumOil, 0)} m3`
                : 'Recovery not available yet',
        },
        {
            label: 'Avg Pressure',
            value: Number.isFinite(finalPressure)
                ? `${formatNumber(finalPressure, 1)} bar`
                : 'n/a',
            detail: input.mismatchSummary.pointsCompared > 0
                ? `${input.mismatchSummary.pointsCompared} reference point${input.mismatchSummary.pointsCompared === 1 ? '' : 's'} compared`
                : 'Latest average reservoir pressure',
        },
    ];
}

export function buildReferenceComparisonSummaryItems(input: {
    family: BenchmarkFamily | null;
    results: BenchmarkRunResult[];
    primaryResultKey?: string | null;
}): OutputSummaryItem[] {
    const focusedResult = input.results.find((result) => result.key === input.primaryResultKey)
        ?? input.results.find((result) => result.variantKey === null)
        ?? input.results[0]
        ?? null;

    if (!focusedResult) return [];

    const finalRecovery = lastFinite(focusedResult.recoverySeries);
    const finalPressure = lastFinite(focusedResult.pressureSeries);
    const ooip = getOoip(focusedResult.params);
    const finalCumOil = Number.isFinite(finalRecovery) && ooip > 1e-12
        ? (finalRecovery as number) * ooip
        : null;

    return [
        {
            label: 'Focused Run',
            value: focusedResult.label,
            detail: `${input.results.length} stored run${input.results.length === 1 ? '' : 's'} · ${formatComparisonStatus(focusedResult.referenceComparison.status)}`,
            tone: getComparisonTone(focusedResult.referenceComparison.status),
        },
        {
            label: 'Primary Review',
            value: buildPrimaryReviewValue(focusedResult, input.family),
            detail: buildPrimaryReviewDetail(focusedResult, input.family),
            tone: getComparisonTone(focusedResult.referenceComparison.status),
        },
        {
            label: 'Recovery',
            value: formatPercent(finalRecovery, 1),
            detail: [
                Number.isFinite(finalCumOil) ? `Cum oil ${formatNumber(finalCumOil, 0)} m3` : null,
                Number.isFinite(focusedResult.comparisonOutputs.recoveryDifferenceAtFinalCoordinate)
                    ? `Delta ${formatSigned((focusedResult.comparisonOutputs.recoveryDifferenceAtFinalCoordinate as number) * 100, 1)}%`
                    : null,
            ].filter(Boolean).join(' · ') || 'Recovery review stays in the chart panels',
        },
        {
            label: 'Avg Pressure',
            value: Number.isFinite(finalPressure)
                ? `${formatNumber(finalPressure, 1)} bar`
                : 'n/a',
            detail: Number.isFinite(focusedResult.comparisonOutputs.pressureDifferenceAtFinalTime)
                ? `Delta ${formatSigned(focusedResult.comparisonOutputs.pressureDifferenceAtFinalTime, 1)} bar`
                : focusedResult.referencePolicy.referenceLabel,
        },
    ];
}