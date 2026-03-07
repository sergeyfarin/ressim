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
  import type { BenchmarkVariant } from "../../catalog/benchmarkCases";
  import FilterCard from "../controls/FilterCard.svelte";
  import type { BenchmarkModePanelProps } from "../modePanelTypes";

  let {
    navigationState = undefined,
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
    onRunBenchmarkSelection = () => {},
    onStopBenchmarkSweep = () => {},
  }: BenchmarkModePanelProps = $props();

  type BenchmarkExecutionRow = "base" | BenchmarkSensitivityAxisKey;

  function formatNullableMetric(value: number | null | undefined, digits = 3) {
    return Number.isFinite(value) ? Number(value).toFixed(digits) : "n/a";
  }

  function formatPercent(value: number | null | undefined, digits = 1) {
    return Number.isFinite(value) ? `${(Number(value) * 100).toFixed(digits)}%` : "n/a";
  }

  const modeDimensions = $derived(getModeDimensions("benchmark"));
  const activeReferenceKey = $derived(
    navigationState?.activeLibraryCaseKey ?? toggles.benchmarkId ?? null,
  );
  const showLegacyBenchmarkSelector = $derived(Boolean(toggles.benchmarkId));
  const activeBenchmark = $derived(
    getBenchmarkEntry(activeReferenceKey),
  );
  const activeFamily = $derived(
    getBenchmarkFamily(activeReferenceKey),
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
  const variantsByAxis = $derived.by(() => {
    const grouped: Partial<Record<BenchmarkSensitivityAxisKey, BenchmarkVariant[]>> = {};
    for (const variant of generatedVariants) {
      grouped[variant.axis] = [...(grouped[variant.axis] ?? []), variant];
    }
    return grouped;
  });
  const activeBenchmarkResults = $derived.by(() => {
    if (!activeFamily) return benchmarkRunResults;
    return benchmarkRunResults.filter((result) => result.familyKey === activeFamily.key);
  });

  let selectedExecutionRow = $state<BenchmarkExecutionRow>("base");
  let selectedVariantKeys = $state<string[]>([]);
  let selectionSignature = $state("");

  function buildSelectionSignature() {
    return [
      activeFamily?.key ?? "none",
      ...generatedVariants.map((variant) => `${variant.axis}:${variant.variantKey}`),
    ].join("|");
  }

  $effect(() => {
    const nextSignature = buildSelectionSignature();
    if (selectionSignature !== nextSignature) {
      selectionSignature = nextSignature;
      selectedExecutionRow = "base";
      selectedVariantKeys = [];
    }
  });

  function getAxisVariants(axis: BenchmarkSensitivityAxisKey): BenchmarkVariant[] {
    return variantsByAxis[axis] ?? [];
  }

  function selectExecutionRow(row: BenchmarkExecutionRow) {
    selectedExecutionRow = row;
    if (row === "base") {
      selectedVariantKeys = [];
      return;
    }
    selectedVariantKeys = getAxisVariants(row).map((variant) => variant.variantKey);
  }

  function toggleVariantSelection(axis: BenchmarkSensitivityAxisKey, variantKey: string) {
    if (selectedExecutionRow !== axis) {
      selectedExecutionRow = axis;
      selectedVariantKeys = [variantKey];
      return;
    }

    selectedVariantKeys = selectedVariantKeys.includes(variantKey)
      ? selectedVariantKeys.filter((key) => key !== variantKey)
      : [...selectedVariantKeys, variantKey];
  }

  const activeAxisSelectionLabel = $derived.by(() => {
    if (selectedExecutionRow === "base") return "Base only";
    return getBenchmarkSensitivityAxisLabel(selectedExecutionRow);
  });
  const selectedAxisVariants = $derived.by(() => (
    selectedExecutionRow === "base"
      ? []
      : getAxisVariants(selectedExecutionRow).filter((variant) => selectedVariantKeys.includes(variant.variantKey))
  ));
  const runButtonLabel = $derived.by(() => {
    if (selectedExecutionRow === "base") return "Run Base";
    if (selectedAxisVariants.length === 0) return `Run ${activeAxisSelectionLabel}`;
    if (selectedAxisVariants.length === getAxisVariants(selectedExecutionRow).length) {
      return `Run ${activeAxisSelectionLabel}`;
    }
    return `Run ${selectedAxisVariants.length} ${activeAxisSelectionLabel} Variant${selectedAxisVariants.length === 1 ? "" : "s"}`;
  });
  const runSelectionDisabled = $derived(
    !activeBenchmark
      || isModified
      || benchmarkSweepRunning
      || (selectedExecutionRow !== "base" && selectedAxisVariants.length === 0),
  );

  function runSelectedBenchmarkSet() {
    onRunBenchmarkSelection(
      selectedExecutionRow === "base"
        ? []
        : selectedAxisVariants.map((variant) => variant.variantKey),
    );
  }
</script>

<div class="space-y-3">
  {#if showLegacyBenchmarkSelector}
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
  {/if}

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
      <div class="mt-3 rounded-md border border-border/70 bg-muted/10 p-3">
        <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
          Execution Set
        </div>
        <div class="mt-2 space-y-2">
          <button
            type="button"
            class={`w-full rounded-md border px-3 py-2 text-left text-[11px] transition-colors ${selectedExecutionRow === "base"
              ? "border-primary/60 bg-primary/10 text-foreground"
              : "border-border/70 bg-background text-muted-foreground hover:bg-muted/30"}`}
            onclick={() => selectExecutionRow("base")}
          >
            <div class="flex items-center justify-between gap-2">
              <strong class="font-semibold">Base case</strong>
              <span>1 run</span>
            </div>
            <div class="mt-1 text-[10px] opacity-80">
              Run only the active reference-family base case with its primary reference policy.
            </div>
          </button>

          {#each sensitivityAxes as sensitivityAxis}
            <div
              class={`rounded-md border px-3 py-2 ${selectedExecutionRow === sensitivityAxis.axis
                ? "border-primary/60 bg-primary/10"
                : "border-border/70 bg-background"}`}
            >
              <button
                type="button"
                class="w-full text-left"
                onclick={() => selectExecutionRow(sensitivityAxis.axis)}
              >
                <div class="flex items-center justify-between gap-2 text-[11px]">
                  <strong class="font-semibold text-foreground">{sensitivityAxis.label}</strong>
                  <span class="text-muted-foreground">{selectedExecutionRow === sensitivityAxis.axis ? selectedAxisVariants.length : sensitivityAxis.count} / {sensitivityAxis.count}</span>
                </div>
                <div class="mt-1 text-[10px] text-muted-foreground">
                  {selectedExecutionRow === sensitivityAxis.axis
                    ? "Select the variants to run in this axis."
                    : `Click to stage the ${sensitivityAxis.label.toLowerCase()} set.`}
                </div>
              </button>

              <div class="mt-2 flex flex-wrap gap-2">
                {#each getAxisVariants(sensitivityAxis.axis) as variant}
                  <label
                    class={`inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-[10px] transition-colors ${selectedExecutionRow === sensitivityAxis.axis && selectedVariantKeys.includes(variant.variantKey)
                      ? "border-primary/60 bg-background text-foreground"
                      : "border-border/70 bg-muted/20 text-muted-foreground"}`}
                  >
                    <input
                      type="checkbox"
                      checked={selectedExecutionRow === sensitivityAxis.axis && selectedVariantKeys.includes(variant.variantKey)}
                      disabled={isModified || benchmarkSweepRunning}
                      onchange={() => toggleVariantSelection(sensitivityAxis.axis, variant.variantKey)}
                      class="h-3.5 w-3.5 rounded border-border text-primary focus:ring-primary"
                    />
                    <span>{variant.label}</span>
                  </label>
                {/each}
              </div>
            </div>
          {/each}
        </div>

        <div class="mt-3 flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            disabled={runSelectionDisabled}
            onclick={runSelectedBenchmarkSet}
          >
            {runButtonLabel}
          </Button>
          {#if benchmarkSweepRunning}
            <Button
              size="sm"
              variant="outline"
              onclick={onStopBenchmarkSweep}
            >
              Stop Sweep
            </Button>
          {/if}
          <span class="text-[10px] text-muted-foreground">
            Selected: <strong class="text-foreground">{activeAxisSelectionLabel}</strong>{#if selectedExecutionRow !== "base"} ({selectedAxisVariants.length} variant{selectedAxisVariants.length === 1 ? "" : "s"}){/if}
          </span>
        </div>
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
      {#if activeBenchmarkResults.length > 0}
        <div class="mt-3 space-y-2 border-t border-border/50 pt-2">
          <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
            Stored Reference Results
          </div>
          <div class="space-y-2">
            {#each activeBenchmarkResults as result}
              <div class="rounded-md border border-border/70 bg-muted/20 px-2 py-2 text-[10px]">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <strong class="text-foreground">{result.label}</strong>
                  <span class="text-muted-foreground">
                    Breakthrough PVI: {formatNullableMetric(result.breakthroughPvi)}
                  </span>
                </div>
                <div class="mt-1 text-muted-foreground">
                  Reference: <strong class="text-foreground">{result.referencePolicy.referenceLabel}</strong>
                </div>
                <div class="mt-1 text-muted-foreground">
                  Policy: {result.referencePolicy.summary}
                </div>
                <div class="mt-1 text-muted-foreground">
                  {result.referenceComparison.summary}
                </div>
                <div class="mt-1 text-muted-foreground">
                  {result.comparisonOutputs.errorSummary}
                </div>
                <div class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-muted-foreground">
                  {#if result.comparisonOutputs.breakthroughShiftPvi !== null}
                    <span>BT shift: <strong class="text-foreground">{formatNullableMetric(result.comparisonOutputs.breakthroughShiftPvi)}</strong></span>
                  {/if}
                  {#if result.comparisonOutputs.recoveryDifferenceAtFinalCoordinate !== null}
                    <span>Recovery delta: <strong class="text-foreground">{formatPercent(result.comparisonOutputs.recoveryDifferenceAtFinalCoordinate)}</strong></span>
                  {/if}
                  {#if result.comparisonOutputs.oilRateRelativeErrorAtFinalTime !== null}
                    <span>Final oil-rate error: <strong class="text-foreground">{formatPercent(result.comparisonOutputs.oilRateRelativeErrorAtFinalTime)}</strong></span>
                  {/if}
                  {#if result.comparisonOutputs.pressureDifferenceAtFinalTime !== null}
                    <span>Final pressure delta: <strong class="text-foreground">{formatNullableMetric(result.comparisonOutputs.pressureDifferenceAtFinalTime)}</strong> bar</span>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>
