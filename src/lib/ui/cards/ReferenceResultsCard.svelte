<script lang="ts">
  import Card from "../controls/Card.svelte";
  import {
    buildBenchmarkPrimaryVariation,
    buildBenchmarkVariantDeltaSummary,
    deriveBenchmarkParamsDelta,
  } from "../../benchmarkDisclosure";
  import type { BenchmarkFamily } from "../../catalog/benchmarkCases";
  import type { BenchmarkRunResult } from "../../benchmarkRunModel";

  let {
    family = null,
    results = [],
    selectedResultKey = null,
    onSelectResult = () => {},
    onClearSelection = () => {},
  }: {
    family?: BenchmarkFamily | null;
    results?: BenchmarkRunResult[];
    selectedResultKey?: string | null;
    onSelectResult?: (resultKey: string) => void;
    onClearSelection?: () => void;
  } = $props();

  function formatNullableMetric(value: number | null | undefined, digits = 3) {
    return Number.isFinite(value) ? Number(value).toFixed(digits) : "n/a";
  }

  function formatSignedMetric(value: number | null | undefined, digits = 3) {
    if (!Number.isFinite(value)) return "n/a";
    const numeric = Number(value);
    return `${numeric >= 0 ? "+" : ""}${numeric.toFixed(digits)}`;
  }

  function formatPercent(value: number | null | undefined, digits = 1) {
    return Number.isFinite(value) ? `${(Number(value) * 100).toFixed(digits)}%` : "n/a";
  }

  function formatComparisonStatus(status: BenchmarkRunResult["referenceComparison"]["status"]) {
    if (status === "within-tolerance") return "Within tolerance";
    if (status === "outside-tolerance") return "Outside tolerance";
    if (status === "pending-reference") return "Pending reference";
    return "Not applicable";
  }

  function comparisonStatusTone(status: BenchmarkRunResult["referenceComparison"]["status"]) {
    if (status === "within-tolerance") {
      return "border-emerald-300/70 bg-emerald-50 text-emerald-700 dark:border-emerald-700/70 dark:bg-emerald-950/40 dark:text-emerald-300";
    }
    if (status === "outside-tolerance") {
      return "border-amber-300/70 bg-amber-50 text-amber-700 dark:border-amber-700/70 dark:bg-amber-950/40 dark:text-amber-300";
    }
    if (status === "pending-reference") {
      return "border-sky-300/70 bg-sky-50 text-sky-700 dark:border-sky-700/70 dark:bg-sky-950/40 dark:text-sky-300";
    }
    return "border-border/70 bg-muted/20 text-muted-foreground";
  }

  function handleRowKeydown(event: KeyboardEvent, resultKey: string) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onSelectResult(resultKey);
    }
  }

  function formatDeltaValue(result: BenchmarkRunResult) {
    if (Number.isFinite(result.comparisonOutputs.breakthroughShiftPvi)) {
      return `${formatSignedMetric(result.comparisonOutputs.breakthroughShiftPvi)} PVI`;
    }
    if (Number.isFinite(result.referenceComparison.relativeError)) {
      return formatPercent(result.referenceComparison.relativeError);
    }
    if (Number.isFinite(result.comparisonOutputs.oilRateRelativeErrorAtFinalTime)) {
      return formatPercent(result.comparisonOutputs.oilRateRelativeErrorAtFinalTime);
    }
    return "n/a";
  }

  const orderedResults = $derived.by(() => (
    [...results].sort((left, right) => {
      if (left.variantKey === null && right.variantKey !== null) return -1;
      if (left.variantKey !== null && right.variantKey === null) return 1;
      return left.label.localeCompare(right.label);
    })
  ));
  const baseResult = $derived.by(() => (
    orderedResults.find((result) => result.variantKey === null) ?? orderedResults[0] ?? null
  ));
  const selectedResult = $derived.by(() => (
    orderedResults.find((result) => result.key === selectedResultKey) ?? null
  ));
  const focusedResult = $derived.by(() => selectedResult ?? baseResult);
  const resultDeltaSummaryByKey = $derived.by(() => {
    const summaryByKey = new Map<string, string>();
    if (!family) return summaryByKey;

    for (const result of orderedResults) {
      const delta = deriveBenchmarkParamsDelta(family.baseCase.params, result.params);
      summaryByKey.set(result.key, buildBenchmarkVariantDeltaSummary(delta));
    }
    return summaryByKey;
  });
  const resultVariationByKey = $derived.by(() => {
    const variationByKey = new Map<string, ReturnType<typeof buildBenchmarkPrimaryVariation>>();
    if (!family) return variationByKey;

    for (const result of orderedResults) {
      const delta = deriveBenchmarkParamsDelta(family.baseCase.params, result.params);
      variationByKey.set(result.key, buildBenchmarkPrimaryVariation(delta));
    }
    return variationByKey;
  });
</script>

{#if family && orderedResults.length > 0}
  <Card>
    <div class="p-3 md:p-4 space-y-3">
      <div class="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <div class="ui-panel-kicker">
            Reference Run Results
          </div>
          <div class="ui-support-copy mt-2">
            <strong>{family.label}:</strong>
            {orderedResults.length} stored run{orderedResults.length === 1 ? "" : "s"} ready for output review.
          </div>
          <div class="mt-2 flex flex-wrap items-center gap-2">
            <span>
              Visualization focus: <strong class="text-foreground">{focusedResult?.label ?? "All runs"}</strong>
            </span>
            {#if selectedResult}
              <button
                type="button"
                class="ui-chip border border-border/70 bg-background text-muted-foreground transition-colors hover:bg-muted/30 hover:text-foreground"
                onclick={onClearSelection}
              >
                Show all runs
              </button>
            {/if}
          </div>
          <div class="ui-microcopy mt-2">
            Selecting a run updates the profile and 3D outputs. Charts keep their own case selectors.
          </div>
        </div>
      </div>

      <div class="ui-microcopy rounded-md border border-border/70 bg-muted/10 p-3">
        <div class="ui-subsection-kicker">Run Table</div>

        <div class="mt-2 overflow-x-auto">
          <table class="compact-table w-full min-w-184 text-left text-[10px] text-muted-foreground">
            <thead>
              <tr class="border-b border-border/60 text-[9px] uppercase tracking-[0.12em]">
                <th class="px-2 py-2 font-semibold">Run</th>
                <th class="px-2 py-2 font-semibold">Varied input</th>
                <th class="px-2 py-2 font-semibold">BT PVI</th>
                <th class="px-2 py-2 font-semibold">BT Time (d)</th>
                <th class="px-2 py-2 font-semibold">Delta vs reference</th>
                <th class="px-2 py-2 font-semibold">Status</th>
              </tr>
            </thead>
            <tbody>
              {#each orderedResults as result, index}
                <tr
                  class={`cursor-pointer transition-colors ${selectedResultKey === result.key
                    ? "bg-primary/10"
                    : result.variantKey === null && !selectedResultKey
                      ? "bg-primary/5"
                      : "hover:bg-muted/30"}`}
                  tabindex="0"
                  onclick={() => onSelectResult(result.key)}
                  onkeydown={(event) => handleRowKeydown(event, result.key)}
                >
                  <td class={`px-2 py-2 ${index > 0 ? "border-t border-border/60" : ""}`}>
                    <div class="font-semibold text-foreground">{result.label}</div>
                    <div class="mt-1 text-[9px] text-muted-foreground">
                      {result.variantKey === null ? "Base case" : (result.variantLabel ?? "Variant")}
                    </div>
                  </td>
                  <td class={`px-2 py-2 ${index > 0 ? "border-t border-border/60" : ""}`}>
                    <div class="text-foreground">{resultVariationByKey.get(result.key)?.value ?? "n/a"}</div>
                    <div class="mt-1 text-[9px] text-muted-foreground">{resultVariationByKey.get(result.key)?.label ?? "Variant"}</div>
                  </td>
                  <td class={`px-2 py-2 text-foreground ${index > 0 ? "border-t border-border/60" : ""}`}>
                    {formatNullableMetric(result.breakthroughPvi)}
                  </td>
                  <td class={`px-2 py-2 text-foreground ${index > 0 ? "border-t border-border/60" : ""}`}>
                    {formatNullableMetric(result.breakthroughTime, 2)}
                  </td>
                  <td class={`px-2 py-2 text-foreground ${index > 0 ? "border-t border-border/60" : ""}`}>
                    {formatDeltaValue(result)}
                  </td>
                  <td class={`px-2 py-2 ${index > 0 ? "border-t border-border/60" : ""}`}>
                    <span class={`inline-flex rounded-md border px-2 py-1 font-medium ${comparisonStatusTone(result.referenceComparison.status)}`}>
                      {formatComparisonStatus(result.referenceComparison.status)}
                    </span>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>

        {#if focusedResult}
          <div class="mt-3 flex flex-wrap items-start justify-between gap-2 rounded-md border border-border/60 bg-background/70 px-3 py-2">
            <div class="min-w-0 flex-1">
              <div class="ui-subsection-kicker">Selected Review</div>
              <div class="mt-1 text-foreground">{focusedResult.label}</div>
              <div class="ui-microcopy mt-1">{focusedResult.referenceComparison.summary}</div>
              {#if focusedResult.variantKey !== null}
                <div class="ui-microcopy mt-1">
                  Change from base: {resultDeltaSummaryByKey.get(focusedResult.key)}
                </div>
              {/if}
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <span class={`ui-chip border ${comparisonStatusTone(focusedResult.referenceComparison.status)}`}>
                {formatComparisonStatus(focusedResult.referenceComparison.status)}
              </span>
              {#if selectedResult && selectedResult.variantKey !== null}
                <button
                  type="button"
                  class="ui-chip border border-border/70 bg-background text-muted-foreground transition-colors hover:bg-muted/30 hover:text-foreground"
                  onclick={onClearSelection}
                >
                  Show base case
                </button>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  </Card>
{/if}