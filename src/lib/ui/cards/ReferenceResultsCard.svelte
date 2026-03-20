<script lang="ts">
  import Card from "../controls/Card.svelte";
  import type { BenchmarkFamily } from "../../catalog/benchmarkCases";
  import type { BenchmarkRunResult } from "../../benchmarkRunModel";

  let {
    family = null,
    results = [],
  }: {
    family?: BenchmarkFamily | null;
    results?: BenchmarkRunResult[];
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
    if (status === "within-tolerance") return "Pass";
    if (status === "outside-tolerance") return "Fail";
    if (status === "pending-reference") return "Pending";
    return "n/a";
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

  const orderedResults = $derived.by(() => ([...results]));
</script>

{#if family && orderedResults.length > 0}
  <Card>
    <div class="p-3 md:p-4 space-y-3">
      <div>
        <div class="ui-panel-kicker">Reference Run Results</div>
        <div class="ui-support-copy mt-2">
          <strong>{family.label}:</strong>
          {orderedResults.length} stored run{orderedResults.length === 1 ? "" : "s"} ready for output review.
        </div>
      </div>

      <div class="ui-microcopy rounded-md border border-border/70 bg-muted/10 p-3">
        <div class="mt-2 overflow-x-auto">
          <table class="compact-table w-full min-w-120 text-left text-[10px] text-muted-foreground">
            <thead>
              <tr class="border-b border-border/60 text-[9px] uppercase tracking-[0.12em]">
                <th class="px-2 py-2 font-semibold">Run</th>
                <th class="px-2 py-2 font-semibold">BT PVI</th>
                <th class="px-2 py-2 font-semibold">BT Time (d)</th>
                <th class="px-2 py-2 font-semibold">Delta vs reference</th>
                <th class="px-2 py-2 font-semibold">Status</th>
              </tr>
            </thead>
            <tbody>
              {#each orderedResults as result, index}
                <tr class="transition-colors">
                  <td class={`px-2 py-2 ${index > 0 ? "border-t border-border/60" : ""}`}>
                    <div class="font-semibold text-foreground">{result.label}</div>
                    <div class="mt-1 text-[9px] text-muted-foreground">
                      {result.variantKey === null ? "Base case" : (result.variantLabel ?? "Variant")}
                    </div>
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
      </div>
    </div>
  </Card>
{/if}
