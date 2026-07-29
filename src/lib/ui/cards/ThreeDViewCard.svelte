<script lang="ts">
    import type { Output3DSelection, OutputSelectionProfile } from '../../stores/navigationStore.svelte';
    import type { BenchmarkRunResult } from '../../benchmarkRunModel';
    import Button from '../controls/Button.svelte';
    import SpatialProfileChart from '../../visualization/SpatialProfileChart.svelte';

    type ThreeDViewComponentType = typeof import('../../visualization/3dview.svelte').default;

    type Props = {
        ThreeDViewComponent: ThreeDViewComponentType | null;
        loadingThreeDView: boolean;
        selectedOutput3D: Output3DSelection;
        selectedOutputProfile: OutputSelectionProfile;
        activeReferenceResults: BenchmarkRunResult[];
        activePrimaryComparisonResultKey: string | null;
        theme: 'dark' | 'light';
        vizRevision: number;
        showProperty: 'pressure' | 'saturation_water' | 'saturation_oil' | 'saturation_gas' | 'saturation_ternary';
        legendFixedMin: number;
        legendFixedMax: number;
        onApplyHistoryIndex: (index: number) => void;
        onLoadThreeDView: () => void;
        onSelectResult: (key: string) => void;
        onClearResult: () => void;
    };

    let {
        ThreeDViewComponent,
        loadingThreeDView,
        selectedOutput3D,
        selectedOutputProfile,
        activeReferenceResults,
        activePrimaryComparisonResultKey,
        theme,
        vizRevision,
        showProperty = $bindable(),
        legendFixedMin = $bindable(),
        legendFixedMax = $bindable(),
        onApplyHistoryIndex,
        onLoadThreeDView,
        onSelectResult,
        onClearResult,
    }: Props = $props();

    function applyHistorySlider(event: Event): void {
        const nextIndex = Number((event.currentTarget as HTMLInputElement).value);
        if (Number.isFinite(nextIndex)) onApplyHistoryIndex(nextIndex);
    }
</script>

<div class="p-4 md:p-5">
    {#if ThreeDViewComponent}
        {#key `${selectedOutput3D.nx}-${selectedOutput3D.ny}-${selectedOutput3D.nz}-${selectedOutput3D.cellDz}-${selectedOutput3D.cellDzPerLayer.join(",")}-${vizRevision}-${activePrimaryComparisonResultKey ?? "live"}`}
            <ThreeDViewComponent
                nx={selectedOutput3D.nx}
                ny={selectedOutput3D.ny}
                nz={selectedOutput3D.nz}
                cellDx={selectedOutput3D.cellDx}
                cellDy={selectedOutput3D.cellDy}
                cellDz={selectedOutput3D.cellDz}
                cellDzPerLayer={selectedOutput3D.cellDzPerLayer}
                {theme}
                sourceLabel={selectedOutput3D.sourceLabel}
                gridState={selectedOutput3D.gridState}
                bind:showProperty
                bind:legendFixedMin
                bind:legendFixedMax
                s_wc={selectedOutputProfile.rockProps.s_wc}
                s_or={selectedOutputProfile.rockProps.s_or}
                currentIndex={selectedOutput3D.currentIndex}
                history={selectedOutput3D.history}
                wellState={selectedOutput3D.wellState}
            />
        {/key}

        <div class="mt-3 flex flex-col gap-3 px-1">
            <div class="flex flex-wrap items-center gap-1.5">
                <button
                    type="button"
                    class={`px-2 py-1 text-[11px] font-medium rounded-md border transition-colors ${
                        activePrimaryComparisonResultKey === null
                            ? "bg-primary text-primary-foreground border-primary"
                            : "bg-transparent text-muted-foreground border-border hover:bg-muted/50 hover:text-foreground"
                    }`}
                    onclick={onClearResult}
                >
                    Live runtime
                </button>
                {#each activeReferenceResults as result}
                    <button
                        type="button"
                        class={`px-2 py-1 text-[11px] font-medium rounded-md border transition-colors ${
                            activePrimaryComparisonResultKey === result.key
                                ? "bg-primary text-primary-foreground border-primary"
                                : "bg-transparent text-muted-foreground border-border hover:bg-muted/50 hover:text-foreground"
                        }`}
                        onclick={() => onSelectResult(result.key)}
                    >
                        {result.variantKey === null ? "Base" : (result.variantLabel ?? result.label)}
                    </button>
                {/each}
            </div>

            <div class="flex w-full items-center gap-4">
                <input
                    type="range"
                    class="time-slider flex-1"
                    min="0"
                    max={Math.max(0, selectedOutput3D.history.length - 1)}
                    value={selectedOutput3D.currentIndex}
                    disabled={selectedOutput3D.history.length === 0}
                    oninput={applyHistorySlider}
                    onchange={applyHistorySlider}
                />
                <div class="min-w-35 select-none text-right text-[12px] font-mono font-medium text-foreground">
                    Snapshot <span class="text-primary">{selectedOutput3D.currentIndex}</span><span class="text-muted-foreground">
                        / {Math.max(0, selectedOutput3D.history.length - 1)}</span
                    >
                    {#if selectedOutput3D.replayTime !== null}
                        {@const hrs = selectedOutput3D.replayTime * 24}
                        {@const yrs = selectedOutput3D.replayTime / 365.25}
                        <span class="ml-1 text-muted-foreground">
                            ({selectedOutput3D.replayTime < 1
                                ? `${hrs.toFixed(1)} hrs`
                                : selectedOutput3D.replayTime > 365
                                  ? `${yrs.toFixed(1)} yrs`
                                  : `${selectedOutput3D.replayTime.toFixed(1)} days`})
                        </span>
                    {/if}
                </div>
            </div>
        </div>

        <!--
            Cross-section of the same snapshot the 3D view is rendering. Reads
            `selectedOutput3D.gridState` (the selected timestep) rather than the
            profile selection's final grid, and shares `showProperty`, so the
            timestep and property selectors drive both views together.
        -->
        <SpatialProfileChart
            gridState={selectedOutput3D.gridState}
            grid={{
                nx: selectedOutput3D.nx,
                ny: selectedOutput3D.ny,
                nz: selectedOutput3D.nz,
                cellDx: selectedOutput3D.cellDx,
                cellDy: selectedOutput3D.cellDy,
                cellDz: selectedOutput3D.cellDz,
                cellDzPerLayer: selectedOutput3D.cellDzPerLayer,
            }}
            property={showProperty}
            simTime={selectedOutput3D.replayTime ?? selectedOutputProfile.simTime}
            sourceLabel={selectedOutput3D.sourceLabel}
            {theme}
            rockProps={selectedOutputProfile.rockProps}
            fluidProps={selectedOutputProfile.fluidProps}
            initialSaturation={selectedOutputProfile.initialSaturation}
            injectionRate={selectedOutputProfile.injectionRate}
            defaultJ={selectedOutputProfile.producerJ}
        />
    {:else}
        <div
            class="flex items-center justify-center rounded border border-border bg-muted/20"
            style="height: clamp(240px, 35vh, 420px);"
        >
            {#if loadingThreeDView}
                <div class="flex items-center space-x-2">
                    <div class="h-4 w-4 animate-spin rounded-full border-b-2 border-primary"></div>
                    <span class="text-sm font-medium">Loading 3D output...</span>
                </div>
            {:else}
                <Button size="sm" variant="default" onclick={onLoadThreeDView}>Open 3D View</Button>
            {/if}
        </div>
    {/if}
</div>
