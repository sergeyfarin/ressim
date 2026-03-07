<script lang="ts">
  import Button from "../controls/Button.svelte";
  import Card from "../controls/Card.svelte";
  import {
    buildBenchmarkCaseSnapshot,
    buildBenchmarkReferenceGuidance,
    buildBenchmarkVariantDeltaSummary,
  } from "../../benchmarkDisclosure";
  import {
    getBenchmarkFamily,
    getBenchmarkSensitivityAxisLabel,
    getBenchmarkVariantsForFamily,
  } from "../../catalog/caseCatalog";
  import type { BenchmarkSensitivityAxisKey } from "../../catalog/caseCatalog";
  import type { BenchmarkVariant } from "../../catalog/benchmarkCases";

  let {
    referenceFamilyKey = null,
    isModified = false,
    referenceSweepRunning = false,
    onRunReferenceSelection = () => {},
    onStopReferenceSweep = () => {},
  }: {
    referenceFamilyKey?: string | null;
    isModified?: boolean;
    referenceSweepRunning?: boolean;
    onRunReferenceSelection?: (variantKeys: string[]) => void;
    onStopReferenceSweep?: () => void;
  } = $props();

  type ReferenceExecutionRow = "base" | BenchmarkSensitivityAxisKey;

  const activeFamily = $derived(getBenchmarkFamily(referenceFamilyKey));
  const generatedVariants = $derived(
    activeFamily ? getBenchmarkVariantsForFamily(activeFamily.key) : [],
  );
  const baseCaseSnapshot = $derived.by(() => (
    activeFamily ? buildBenchmarkCaseSnapshot(activeFamily.baseCase.params) : null
  ));
  const referenceGuidance = $derived.by(() => (
    activeFamily
      ? buildBenchmarkReferenceGuidance({
        scenarioClass: activeFamily.scenarioClass,
        referenceKind: activeFamily.reference.kind,
        comparisonMetric: activeFamily.comparisonMetric ?? null,
        displayDefaults: activeFamily.displayDefaults ?? null,
        runPolicy: activeFamily.runPolicy ?? null,
      })
      : null
  ));
  const variantDeltaSummaryByKey = $derived.by(() => {
    const summaryByKey = new Map<string, string>();
    for (const variant of generatedVariants) {
      summaryByKey.set(variant.variantKey, buildBenchmarkVariantDeltaSummary(variant.paramsDelta));
    }
    return summaryByKey;
  });
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

  let selectedExecutionRow = $state<ReferenceExecutionRow>("base");
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

  function selectExecutionRow(row: ReferenceExecutionRow) {
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
    if (selectedExecutionRow === "base") return "Base case";
    return getBenchmarkSensitivityAxisLabel(selectedExecutionRow);
  });
  const selectedAxisVariants = $derived.by(() => (
    selectedExecutionRow === "base"
      ? []
      : getAxisVariants(selectedExecutionRow).filter((variant) => selectedVariantKeys.includes(variant.variantKey))
  ));
  const runButtonLabel = $derived.by(() => {
    if (selectedExecutionRow === "base") return "Run Base Case";
    if (selectedAxisVariants.length === 0) return `Run ${activeAxisSelectionLabel} Set`;
    if (selectedAxisVariants.length === getAxisVariants(selectedExecutionRow).length) {
      return `Run ${activeAxisSelectionLabel} Set`;
    }
    return `Run ${selectedAxisVariants.length} ${activeAxisSelectionLabel} Variant${selectedAxisVariants.length === 1 ? "" : "s"}`;
  });
  const runSelectionDisabled = $derived(
    !activeFamily
      || isModified
      || referenceSweepRunning
      || (selectedExecutionRow !== "base" && selectedAxisVariants.length === 0),
  );

  function runSelectedReferenceSet() {
    onRunReferenceSelection(
      selectedExecutionRow === "base"
        ? []
        : selectedAxisVariants.map((variant) => variant.variantKey),
    );
  }
</script>

{#if activeFamily}
  <Card>
    <div class="p-3 md:p-4 space-y-3">
      <div>
        <div class="ui-panel-kicker">
          Reference Runs
        </div>
        <div class="ui-support-copy mt-2">
          <strong>{activeFamily.label}:</strong>
          {activeFamily.description}
        </div>
        {#if generatedVariants.length > 0}
          <div class="ui-microcopy mt-2">
            Generated sensitivity suite: <strong class="text-foreground">{generatedVariants.length}</strong>
            variants across {sensitivitySummary}.
          </div>
        {/if}
      </div>

      {#if baseCaseSnapshot && referenceGuidance}
        <div class="grid gap-2 xl:grid-cols-2">
          <div class="ui-microcopy rounded-md border border-border/70 bg-muted/10 p-3">
            <div class="ui-subsection-kicker">Case Snapshot</div>
            <div class="mt-2">Grid: <strong class="text-foreground">{baseCaseSnapshot.grid}</strong></div>
            <div class="mt-1">Horizon: <strong class="text-foreground">{baseCaseSnapshot.horizon}</strong></div>
            <div class="mt-1">Controls: <strong class="text-foreground">{baseCaseSnapshot.controls}</strong></div>
            <div class="mt-1">Reservoir: <strong class="text-foreground">{baseCaseSnapshot.reservoir}</strong></div>
          </div>
          <div class="ui-microcopy rounded-md border border-border/70 bg-muted/10 p-3">
            <div class="ui-subsection-kicker">Reference Guidance</div>
            <div class="mt-2">{referenceGuidance.reference}</div>
            <div class="mt-1">{referenceGuidance.metric}</div>
            <div class="mt-1">{referenceGuidance.outputs}</div>
            <div class="mt-1">{referenceGuidance.runApproach}</div>
          </div>
        </div>
      {/if}

      <div class="rounded-md border border-border/70 bg-muted/10 p-3">
        <div class="ui-panel-kicker">
          Run Set
        </div>
        <div class="mt-2 space-y-2">
          <button
            type="button"
            class={`w-full rounded-md border px-3 py-2 text-left transition-colors ${selectedExecutionRow === "base"
              ? "border-primary/60 bg-primary/10 text-foreground"
              : "border-border/70 bg-background text-muted-foreground hover:bg-muted/30"}`}
            onclick={() => selectExecutionRow("base")}
          >
            <div class="flex items-center justify-between gap-2">
              <strong class="font-semibold">Base case</strong>
              <span>1 run</span>
            </div>
            <div class="ui-microcopy mt-1 opacity-80">
              Run only the active library reference base case with its primary reference solution.
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
                <div class="flex items-center justify-between gap-2">
                  <strong class="font-semibold text-foreground">{sensitivityAxis.label}</strong>
                  <span class="ui-support-copy">{selectedExecutionRow === sensitivityAxis.axis ? selectedAxisVariants.length : sensitivityAxis.count} / {sensitivityAxis.count}</span>
                </div>
                <div class="ui-microcopy mt-1">
                  {selectedExecutionRow === sensitivityAxis.axis
                    ? "Select the variants to include in this run set."
                    : `Click to stage the ${sensitivityAxis.label.toLowerCase()} run set.`}
                </div>
              </button>

              <div class="mt-2 flex flex-wrap gap-2">
                {#each getAxisVariants(sensitivityAxis.axis) as variant}
                  <label
                    class={`ui-chip items-start rounded-md border transition-colors ${selectedExecutionRow === sensitivityAxis.axis && selectedVariantKeys.includes(variant.variantKey)
                      ? "border-primary/60 bg-background text-foreground"
                      : "border-border/70 bg-muted/20 text-muted-foreground"}`}
                  >
                    <input
                      type="checkbox"
                      checked={selectedExecutionRow === sensitivityAxis.axis && selectedVariantKeys.includes(variant.variantKey)}
                      disabled={isModified || referenceSweepRunning}
                      onchange={() => toggleVariantSelection(sensitivityAxis.axis, variant.variantKey)}
                      class="h-3.5 w-3.5 rounded border-border text-primary focus:ring-primary"
                    />
                    <span class="flex flex-col">
                      <span>{variant.label}</span>
                      <span class="ui-microcopy opacity-70">Change from base: {variantDeltaSummaryByKey.get(variant.variantKey) ?? variant.description}</span>
                    </span>
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
            onclick={runSelectedReferenceSet}
          >
            {runButtonLabel}
          </Button>
          {#if referenceSweepRunning}
            <Button
              size="sm"
              variant="outline"
              onclick={onStopReferenceSweep}
            >
              Stop Reference Runs
            </Button>
          {/if}
          <span class="ui-microcopy">
            Selected run set: <strong class="text-foreground">{activeAxisSelectionLabel}</strong>{#if selectedExecutionRow !== "base"} ({selectedAxisVariants.length} variant{selectedAxisVariants.length === 1 ? "" : "s"}){/if}
          </span>
        </div>
      </div>
    </div>
  </Card>
{/if}