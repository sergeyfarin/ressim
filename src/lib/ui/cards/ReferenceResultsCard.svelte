<script lang="ts">
  import Card from "../controls/Card.svelte";
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
</script>

{#if family && orderedResults.length > 0}
  <Card>
    <div class="p-3 md:p-4 space-y-3">
      <div class="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
            Reference Run Results
          </div>
          <div class="mt-2 text-[11px] text-muted-foreground">
            <strong>{family.label}:</strong>
            {orderedResults.length} stored run{orderedResults.length === 1 ? "" : "s"} ready for output review.
          </div>
          <div class="mt-2 flex flex-wrap items-center gap-2 text-[10px] text-muted-foreground">
            <span>
              Output focus: <strong class="text-foreground">{selectedResult?.label ?? "All runs"}</strong>
            </span>
            {#if selectedResult}
              <button
                type="button"
                class="rounded-md border border-border/70 bg-background px-2 py-1 text-muted-foreground transition-colors hover:bg-muted/30 hover:text-foreground"
                onclick={onClearSelection}
              >
                Show all runs
              </button>
            {/if}
          </div>
        </div>
        {#if baseResult}
          <div class="rounded-md border border-border/70 bg-muted/10 px-3 py-2 text-[10px] text-muted-foreground lg:max-w-xs">
            <div class="font-semibold uppercase tracking-[0.14em]">Base Case</div>
            <div class="mt-1 text-foreground">{baseResult.label}</div>
            <div class="mt-1">{baseResult.referencePolicy.referenceLabel}</div>
          </div>
        {/if}
      </div>

      <div class="grid gap-2 xl:grid-cols-2">
        {#each orderedResults as result}
          <button
            type="button"
            class={`rounded-md border px-3 py-3 text-left text-[10px] text-muted-foreground transition-colors ${selectedResultKey === result.key
              ? "border-primary/60 bg-primary/10"
              : "border-border/70 bg-muted/20 hover:bg-muted/30"}`}
            onclick={() => onSelectResult(result.key)}
          >
            <div class="flex flex-wrap items-start justify-between gap-2">
              <div>
                <div class="font-semibold text-foreground">{result.label}</div>
                <div class="mt-1">
                  {result.variantKey === null ? "Base case" : `Variant: ${result.variantLabel ?? result.variantKey}`}
                </div>
                <div class="mt-2 inline-flex rounded-md border border-border/70 bg-background/80 px-2 py-1 text-[10px] font-medium text-muted-foreground">
                  {selectedResultKey === result.key ? "Focused in outputs" : "Focus in outputs"}
                </div>
              </div>
              <span class={`rounded-md border px-2 py-1 font-medium ${comparisonStatusTone(result.referenceComparison.status)}`}>
                {formatComparisonStatus(result.referenceComparison.status)}
              </span>
            </div>

            <div class="mt-2 grid gap-2 sm:grid-cols-2">
              <div>
                <div>Breakthrough PVI: <strong class="text-foreground">{formatNullableMetric(result.breakthroughPvi)}</strong></div>
                <div class="mt-1">Reference: <strong class="text-foreground">{result.referencePolicy.referenceLabel}</strong></div>
              </div>
              <div>
                <div>{result.referenceComparison.summary}</div>
                <div class="mt-1">{result.comparisonOutputs.errorSummary}</div>
              </div>
            </div>

            <div class="mt-2 flex flex-wrap gap-x-3 gap-y-1">
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
          </button>
        {/each}
      </div>
    </div>
  </Card>
{/if}