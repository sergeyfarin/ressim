<script lang="ts">
    import RateChart from './RateChart.svelte';
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
        isCustom = false,
        activeDimensionKey = null,
        analyticalOption = null,
        runResults = [],
        layoutConfig = {},
        analyticalPerVariant = false,
        previewVariantParams = undefined,
        pendingPreviewVariants = undefined,
        previewBaseParams = undefined,
        theme = 'dark',
        rateHistory = [],
        analyticalProductionData = [],
        avgReservoirPressureSeries = [],
        avgWaterSaturationSeries = [],
        ooipM3 = 0,
        poreVolumeM3 = 0,
        activeMode = '',
        activeCase = '',
        analyticalMeta = undefined,
        rockProps = undefined,
        fluidProps = undefined,
        layerPermeabilities = [],
        layerThickness = 1,
        showSweepPanel = false,
        sweepGeometry = 'both',
        sweepAnalyticalMethod = 'dykstra-parsons',
        sweepEfficiencySimSeries = null,
        sweepRFAnalytical = null,
    }: {
        scenario?: Scenario | null;
        isCustom?: boolean;
        activeDimensionKey?: string | null;
        analyticalOption?: ScenarioAnalyticalOption | null;
        runResults?: RunResult[];
        layoutConfig?: RateChartLayoutConfig;
        analyticalPerVariant?: boolean;
        previewVariantParams?: AnalyticalPreviewVariant[];
        pendingPreviewVariants?: AnalyticalPreviewVariant[];
        previewBaseParams?: Record<string, any>;
        theme?: 'dark' | 'light';
        rateHistory?: RateHistoryPoint[];
        analyticalProductionData?: AnalyticalProductionPoint[];
        avgReservoirPressureSeries?: Array<number | null>;
        avgWaterSaturationSeries?: Array<number | null>;
        ooipM3?: number;
        poreVolumeM3?: number;
        activeMode?: string;
        activeCase?: string;
        analyticalMeta?: any;
        rockProps?: RockProps;
        fluidProps?: FluidProps;
        layerPermeabilities?: number[];
        layerThickness?: number;
        showSweepPanel?: boolean;
        sweepGeometry?: SweepGeometry;
        sweepAnalyticalMethod?: SweepAnalyticalMethod;
        sweepEfficiencySimSeries?: Array<{
            time: number;
            eA: number | null;
            eV: number | null;
            eVol: number;
            mobileOilRecovered: number | null;
        }> | null;
        sweepRFAnalytical?: SweepRFResult | null;
    } = $props();

    const comparisonFamily = $derived(buildScenarioComparisonFamily({
        scenario,
        activeDimensionKey,
        analyticalOption,
        layoutConfig,
    }));
    const shouldRenderComparison = $derived(Boolean(!isCustom && scenario && comparisonFamily));
</script>

{#if shouldRenderComparison && comparisonFamily}
    <ReferenceComparisonChart
        results={runResults}
        family={comparisonFamily}
        {layoutConfig}
        {analyticalPerVariant}
        {theme}
        previewVariantParams={runResults.length === 0 ? previewVariantParams : undefined}
        pendingPreviewVariants={runResults.length > 0 ? pendingPreviewVariants : undefined}
        previewBaseParams={runResults.length === 0 ? previewBaseParams : undefined}
        previewAnalyticalMethod={comparisonFamily.analyticalMethod}
    />
{:else}
    <RateChart
        panelDefs={scenario?.liveChartPanels ?? []}
        {rateHistory}
        {analyticalProductionData}
        {avgReservoirPressureSeries}
        {avgWaterSaturationSeries}
        {ooipM3}
        {poreVolumeM3}
        {activeMode}
        {activeCase}
        {theme}
        {analyticalMeta}
        {layoutConfig}
        {rockProps}
        {fluidProps}
        {layerPermeabilities}
        {layerThickness}
        {showSweepPanel}
        {sweepGeometry}
        {sweepAnalyticalMethod}
        {sweepEfficiencySimSeries}
        {sweepRFAnalytical}
    />
{/if}
