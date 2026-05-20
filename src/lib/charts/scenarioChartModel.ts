import type { BenchmarkFamily } from '../catalog/benchmarkCases';
import {
    getScenarioChartLayout,
    resolveCapabilities,
    suppressesPrimaryAnalyticalOverlays,
    type Scenario,
    type ScenarioAnalyticalOption,
} from '../catalog/scenarios';
import { getOpmFlowPublishedReferenceSeries } from '../catalog/opmFlowArtifacts';
import type { CurveConfig } from './chartTypes';
import type { RateChartLayoutConfig, RateChartPanelId, RateChartXAxisMode } from './rateChartLayoutConfig';

export type ChartCurveModel = CurveConfig & {
    sourceType: 'simulation' | 'analytical' | 'published-reference' | 'opm-flow-precomputed';
    sourceId: string;
};

export type ChartPanelModel = {
    key: RateChartPanelId;
    title: string;
    visible: boolean;
    expanded: boolean;
    curves: ChartCurveModel[];
    series: Array<{ x: number; y: number | null }[]>;
};

export type ChartModel = {
    xAxisMode: RateChartXAxisMode;
    panels: ChartPanelModel[];
    warnings: string[];
};

export function buildScenarioComparisonFamily(input: {
    scenario: Scenario | null | undefined;
    activeDimensionKey?: string | null;
    analyticalOption?: ScenarioAnalyticalOption | null;
    layoutConfig?: RateChartLayoutConfig;
}): BenchmarkFamily | null {
    const scenario = input.scenario ?? null;
    if (!scenario) return null;

    const resolved = resolveCapabilities(scenario.capabilities);
    const activeDimension = scenario.sensitivities.find((dimension) => dimension.key === input.activeDimensionKey) ?? null;
    const chartLayout = input.layoutConfig ?? getScenarioChartLayout(scenario, input.activeDimensionKey);
    const xAxis = resolved.analyticalNativeXAxis as BenchmarkFamily['displayDefaults']['xAxis'];
    const panels = (resolved.primaryRateCurve === 'oil-rate'
        ? ['oil-rate', 'cumulative-oil', 'decline-diagnostics']
        : ['watercut-breakthrough', 'recovery', 'pressure']) as BenchmarkFamily['displayDefaults']['panels'][number][];

    return {
        key: scenario.key,
        baseCaseKey: scenario.key,
        analyticalMethod: resolved.analyticalMethod,
        sensitivityAxes: [],
        reference: {
            kind: 'analytical' as const,
            source: resolved.analyticalMethod === 'digitized-reference'
                ? `${scenario.key}:digitized-reference`
                : `${scenario.key}:analytical`,
        },
        displayDefaults: { xAxis, panels },
        stylePolicy: {
            colorBy: 'case' as const,
            lineStyleBy: 'quantity-or-reference' as const,
            separatePressurePanel: true,
        },
        runPolicy: 'compare-to-reference' as const,
        label: scenario.label,
        description: scenario.description,
        baseCase: {
            key: scenario.key,
            label: scenario.label,
            description: scenario.description,
            params: scenario.params,
        },
        suppressPrimaryAnalyticalOverlays: suppressesPrimaryAnalyticalOverlays(chartLayout),
        showSweepPanel: resolved.showSweepPanel,
        sweepGeometry: resolved.sweepGeometry,
        sweepAnalyticalMethod: input.analyticalOption?.sweepMethod,
        analyticalOverlayMode: activeDimension?.analyticalOverlayMode ?? 'auto',
        publishedReferenceSeries: [
            ...(scenario.publishedReferenceSeries ?? []),
            ...getOpmFlowPublishedReferenceSeries(scenario.key),
        ],
    } as BenchmarkFamily;
}
