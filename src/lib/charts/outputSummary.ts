
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

