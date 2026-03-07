import type {
    RateChartPanelLayout,
    RateChartScalePreset,
    RateChartXAxisMode,
} from './rateChartLayoutConfig';

export type ChartXAxisOption = {
    value: RateChartXAxisMode;
    label: string;
    disabled?: boolean;
    title?: string;
};

export type ChartPanelEntry<TCurve, TSeries> = {
    curve: TCurve;
    series: TSeries;
};

export type ChartPanelDefinition<TCurve, TSeries> = {
    title: string;
    curves: TCurve[];
    series: TSeries[];
    scales: Record<string, any>;
    allowLogToggle: boolean;
};

export type ChartPanelFallback = {
    title: string;
    curveKeys?: string[];
    curveLabels?: string[];
    scalePreset: RateChartScalePreset;
    allowLogToggle?: boolean;
};

type SelectableCurve = {
    label: string;
    curveKey?: string | null;
};

function selectPanelEntries<TCurve extends SelectableCurve, TSeries>(input: {
    entries: Array<ChartPanelEntry<TCurve, TSeries>>;
    curveKeys?: string[];
    curveLabels?: string[];
}): Array<ChartPanelEntry<TCurve, TSeries>> {
    const { entries, curveKeys, curveLabels } = input;

    if (Array.isArray(curveKeys) && curveKeys.length > 0) {
        const allowedCurveKeys = new Set(curveKeys);
        return entries.filter((entry) => (
            Boolean(entry.curve.curveKey) && allowedCurveKeys.has(entry.curve.curveKey as string)
        ));
    }

    if (Array.isArray(curveLabels) && curveLabels.length > 0) {
        const entriesByLabel = new Map(entries.map((entry) => [entry.curve.label, entry]));
        return curveLabels
            .map((label) => entriesByLabel.get(label))
            .filter((entry): entry is ChartPanelEntry<TCurve, TSeries> => Boolean(entry));
    }

    return entries;
}

export function getConfiguredXAxisOptions(
    allOptions: ChartXAxisOption[],
    configured?: RateChartXAxisMode[],
): ChartXAxisOption[] {
    if (!Array.isArray(configured) || configured.length === 0) return allOptions;

    const allowed = new Set(configured);
    return allOptions.filter((option) => allowed.has(option.value));
}

export function coerceChartAxisState(input: {
    xAxisMode: RateChartXAxisMode;
    xAxisOptions: ChartXAxisOption[];
    logScale: boolean;
    allowLogScale?: boolean;
}): {
    xAxisMode: RateChartXAxisMode;
    logScale: boolean;
} {
    const allowedModes = input.xAxisOptions.map((option) => option.value);
    const nextXAxisMode = !allowedModes.includes(input.xAxisMode) && allowedModes.length > 0
        ? allowedModes[0]
        : input.xAxisMode;

    return {
        xAxisMode: nextXAxisMode,
        logScale: input.allowLogScale === false ? false : input.logScale,
    };
}

export function resolveChartPanelDefinition<
    TCurve extends SelectableCurve,
    TSeries,
>(input: {
    override?: RateChartPanelLayout;
    fallback: ChartPanelFallback;
    entries: Array<ChartPanelEntry<TCurve, TSeries>>;
    getScalePresetConfig: (scalePreset: RateChartScalePreset) => Record<string, any>;
}): ChartPanelDefinition<TCurve, TSeries> {
    const { override, fallback, entries, getScalePresetConfig } = input;
    const selectedEntries = selectPanelEntries({
        entries,
        curveKeys: override?.curveKeys ?? fallback.curveKeys,
        curveLabels: override?.curveLabels ?? fallback.curveLabels,
    });

    return {
        title: override?.title ?? fallback.title,
        curves: selectedEntries.map((entry) => entry.curve),
        series: selectedEntries.map((entry) => entry.series),
        scales: getScalePresetConfig(override?.scalePreset ?? fallback.scalePreset),
        allowLogToggle: override?.allowLogToggle ?? fallback.allowLogToggle ?? false,
    };
}