<script lang="ts">
  import Button from "../controls/Button.svelte";
  import {
    getBenchmarkEntry,
    getBenchmarkFamily,
    getBenchmarkSensitivityAxisLabel,
    getBenchmarkVariantsForFamily,
    getModeDimensions,
  } from "../../catalog/caseCatalog";
  import type { BenchmarkSensitivityAxisKey } from "../../catalog/caseCatalog";
  import FilterCard from "../controls/FilterCard.svelte";
  import type { BenchmarkModePanelProps } from "../modePanelTypes";

  let {
    toggles,
    disabledOptions,
    isModified = false,
    benchmarkProvenance = null,
    benchmarkSweepRunning = false,
    benchmarkSweepProgressLabel = "",
    benchmarkSweepError = "",
    benchmarkRunResults = [],
    onToggleChange,
    onCloneBenchmarkToCustom = () => {},
    onRunBenchmarkBase = () => {},
    onRunBenchmarkSensitivityAxis = () => {},
    onStopBenchmarkSweep = () => {},
  }: BenchmarkModePanelProps = $props();

  function formatNullableMetric(value: number | null | undefined, digits = 3) {
    return Number.isFinite(value) ? Number(value).toFixed(digits) : "n/a";
  }

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
  const sensitivityAxes = $derived.by(() => {
    const orderedAxes: BenchmarkSensitivityAxisKey[] = [];
    for (const variant of generatedVariants) {
      if (!orderedAxes.includes(variant.axis)) orderedAxes.push(variant.axis);
    }

    return orderedAxes.map((axis) => ({
      axis,
      label: getBenchmarkSensitivityAxisLabel(axis),
      count: generatedVariants.filter((variant) => variant.axis === axis).length,
    }));
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
          disabled={!activeBenchmark || isModified || benchmarkSweepRunning}
          onclick={onRunBenchmarkBase}
        >
          Run Base
        </Button>
        {#each sensitivityAxes as sensitivityAxis}
          <Button
            size="sm"
            variant="secondary"
            disabled={!activeBenchmark || isModified || benchmarkSweepRunning}
            onclick={() => onRunBenchmarkSensitivityAxis(sensitivityAxis.axis)}
          >
            Run {sensitivityAxis.label}
          </Button>
        {/each}
        {#if benchmarkSweepRunning}
          <Button
            size="sm"
            variant="outline"
            onclick={onStopBenchmarkSweep}
          >
            Stop Sweep
          </Button>
        {/if}
      </div>
      {#if benchmarkSweepProgressLabel}
        <div class="mt-2 text-[10px] text-muted-foreground">
          {benchmarkSweepProgressLabel}
        </div>
      {/if}
      {#if benchmarkSweepError}
        <div class="mt-2 rounded-md border border-destructive/40 bg-destructive/10 px-2 py-1 text-[10px] text-destructive">
          {benchmarkSweepError}
        </div>
      {/if}
      <div class="mt-2 flex flex-wrap items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={isModified || benchmarkSweepRunning}
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
      {#if benchmarkRunResults.length > 0}
        <div class="mt-3 space-y-2 border-t border-border/50 pt-2">
          <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
            Stored Benchmark Results
          </div>
          <div class="space-y-2">
            {#each benchmarkRunResults as result}
              <div class="rounded-md border border-border/70 bg-muted/20 px-2 py-2 text-[10px]">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <strong class="text-foreground">{result.label}</strong>
                  <span class="text-muted-foreground">
                    Breakthrough PVI: {formatNullableMetric(result.breakthroughPvi)}
                  </span>
                </div>
                <div class="mt-1 text-muted-foreground">
                  {result.referenceComparison.summary}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>
