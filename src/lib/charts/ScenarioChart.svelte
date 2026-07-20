<script lang="ts">
    import ReferenceComparisonChart from './ReferenceComparisonChart.svelte';
    import { buildScenarioComparisonFamily } from './scenarioChartModel';
    import type { Scenario, ScenarioAnalyticalOption } from '../catalog/scenarios';
    import type { RunResult } from '../scenario/runModel';
    import type { RateChartLayoutConfig } from './rateChartLayoutConfig';
    import type { RateHistoryPoint, AnalyticalProductionPoint } from '../simulator-types';
    import type { SweepAnalyticalMethod, SweepGeometry, SweepRFResult } from '../analytical/sweepEfficiency';
    import type { RockProps, FluidProps } from '../analytical/fractionalFlow';
    import type { AnalyticalPreviewVariant } from './buildChartData';

    let {
        scenario = null,
        activeDimensionKey = null,
        analyticalOption = null,
        runResults = [],
        layoutConfig = {},
        analyticalPerVariant = false,
        previewVariantParams = undefined,
        pendingPreviewVariants = undefined,
        previewBaseParams = undefined,
        theme = 'dark',
    }: {
        scenario?: Scenario | null;
        activeDimensionKey?: string | null;
        analyticalOption?: ScenarioAnalyticalOption | null;
        runResults?: RunResult[];
        layoutConfig?: RateChartLayoutConfig;
        analyticalPerVariant?: boolean;
        previewVariantParams?: AnalyticalPreviewVariant[];
        pendingPreviewVariants?: AnalyticalPreviewVariant[];
        previewBaseParams?: Record<string, any>;
        theme?: 'dark' | 'light';
    } = $props();

    const comparisonFamily = $derived(buildScenarioComparisonFamily({
        scenario,
        activeDimensionKey,
        analyticalOption,
        layoutConfig,
    }));
    const shouldRenderComparison = $derived(Boolean(scenario && comparisonFamily));
</script>

{#if shouldRenderComparison && comparisonFamily}
    <ReferenceComparisonChart
        results={runResults}
        family={comparisonFamily}
        {layoutConfig}
        {analyticalPerVariant}
        {theme}
        historyWindow={scenario?.historyWindow ?? null}
        previewVariantParams={runResults.length === 0 ? previewVariantParams : undefined}
        pendingPreviewVariants={runResults.length > 0 ? pendingPreviewVariants : undefined}
        previewBaseParams={runResults.length === 0 ? previewBaseParams : undefined}
        previewAnalyticalMethod={comparisonFamily.analyticalMethod}
    />
{/if}
