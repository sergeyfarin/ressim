<script lang="ts">
    import type { Output3DSelection, OutputSelectionProfile } from '../../stores/navigationStore.svelte';
    import type { BenchmarkRunResult } from '../../benchmarkRunModel';
    import Button from '../controls/Button.svelte';

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
</script>

<div class="p-4 md:p-5">
    {#if activeReferenceResults.length > 0}
        <div class="mb-3 flex flex-wrap items-center gap-1.5">
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
    {:else}
        <div class="mb-3">
            <span class="ui-chip border border-border/70 bg-background text-muted-foreground">
                Live runtime
            </span>
        </div>
    {/if}

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
                replayTime={selectedOutput3D.replayTime}
                onApplyHistoryIndex={onApplyHistoryIndex}
                history={selectedOutput3D.history}
                wellState={selectedOutput3D.wellState}
            />
        {/key}
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
