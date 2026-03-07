<script lang="ts">
  import Button from "../controls/Button.svelte";
  import {
    getBenchmarkEntry,
    getBenchmarkFamily,
    getBenchmarkSensitivityAxisLabel,
    getBenchmarkVariantsForFamily,
    getModeDimensions,
  } from "../../catalog/caseCatalog";
  import FilterCard from "../controls/FilterCard.svelte";
  import type { BenchmarkModePanelProps } from "../modePanelTypes";

  let {
    toggles,
    disabledOptions,
    isModified = false,
    benchmarkProvenance = null,
    onToggleChange,
    onCloneBenchmarkToCustom = () => {},
  }: BenchmarkModePanelProps = $props();

  const modeDimensions = $derived(getModeDimensions("benchmark"));
  const activeBenchmark = $derived(
    getBenchmarkEntry(toggles.benchmarkId),
  );
  const activeFamily = $derived(
    getBenchmarkFamily(toggles.benchmarkId),
  );
  const generatedVariants = $derived(
    activeFamily ? getBenchmarkVariantsForFamily(activeFamily.key) : [],
  );
  const sensitivitySummary = $derived.by(() => {
    if (!generatedVariants.length) return null;

    const orderedAxes: string[] = [];
    for (const axis of generatedVariants.map((variant) => variant.axis)) {
      if (!orderedAxes.includes(axis)) orderedAxes.push(axis);
    }

    return orderedAxes.map((axis) => getBenchmarkSensitivityAxisLabel(axis as any)).join(", ");
  });
</script>

<div class="space-y-3">
  {#each modeDimensions as dim}
    <FilterCard
      label={dim.label}
      options={dim.options.map((option) => option.value)}
      customLabels={dim.options.reduce(
        (acc, option) => ({ ...acc, [option.value]: option.label }),
        {},
      )}
      selected={toggles[dim.key]}
      disabled={Object.keys(disabledOptions[dim.key] || {})}
      disabledReasons={disabledOptions[dim.key] || {}}
      onchange={(value) => onToggleChange(dim.key, value)}
    />
  {/each}

  {#if activeBenchmark}
    <div class="border-t border-border/50 pt-2">
      <div class="text-[11px] text-muted-foreground">
        <strong>{activeBenchmark.label}:</strong>
        {activeBenchmark.description}
      </div>
      {#if generatedVariants.length > 0}
        <div class="mt-2 text-[10px] text-muted-foreground">
          Generated sensitivity suite: <strong class="text-foreground">{generatedVariants.length}</strong>
          variants across {sensitivitySummary}.
        </div>
      {/if}
      <div class="mt-2 flex flex-wrap items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={isModified}
          onclick={onCloneBenchmarkToCustom}
        >
          Clone to Custom
        </Button>
        {#if benchmarkProvenance}
          <span class="text-[10px] text-muted-foreground">
            Cloned source: <strong class="text-foreground">{benchmarkProvenance.sourceLabel}</strong>
          </span>
        {:else if isModified}
          <span class="text-[10px] text-muted-foreground">
            Customized without clone provenance
          </span>
        {/if}
      </div>
    </div>
  {/if}
</div>
