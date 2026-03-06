<script lang="ts">
  import Button from "../../components/ui/Button.svelte";
  import Card from "../../components/ui/Card.svelte";
  import type { CaseMode } from "../../catalog/caseCatalog";
  import { shouldShowModePanelStatusRow } from "../../stores/phase2PresetContract";
  import WarningPolicyPanel from "../feedback/WarningPolicyPanel.svelte";
  import type { ModePanelProps } from "../modePanelTypes";
  import ScenarioSectionsPanel from "../sections/ScenarioSectionsPanel.svelte";
  import BenchmarkPanel from "./BenchmarkPanel.svelte";

  let {
    activeMode = "dep",
    isModified = false,
    toggles = {},
    disabledOptions = {},
    onModeChange,
    onParamEdit = () => {},
    onToggleChange,
    basePreset = null,
    benchmarkProvenance = null,
    onCloneBenchmarkToCustom = () => {},
    params,
    validationErrors = {},
    warningPolicy = undefined,
  }: ModePanelProps = $props();

  const sourceTone = $derived(
    basePreset?.source === "custom"
      ? "border-amber-300 bg-amber-50 text-amber-700 dark:border-amber-700/70 dark:bg-amber-950/40 dark:text-amber-300"
      : basePreset?.source === "benchmark"
        ? "border-sky-300 bg-sky-50 text-sky-700 dark:border-sky-700/70 dark:bg-sky-950/40 dark:text-sky-300"
        : "border-emerald-300 bg-emerald-50 text-emerald-700 dark:border-emerald-700/70 dark:bg-emerald-950/40 dark:text-emerald-300",
  );

  const shouldShowStatusRow = $derived(
    shouldShowModePanelStatusRow({
      benchmarkProvenance,
      parameterOverrideCount: Number(params.parameterOverrideCount ?? 0),
    }),
  );
</script>

<Card class="p-3 md:p-4">
  <!-- Mode tabs -->
  <div class="flex flex-wrap items-center gap-2">
    {#each [["dep", "Depletion"], ["wf", "Waterflood"], ["sim", "Simulation"], ["benchmark", "Benchmarks"]] as [mode, label]}
      <Button
        size="sm"
        variant={activeMode === mode && !isModified ? "default" : "outline"}
        onclick={() => onModeChange(mode as CaseMode)}
      >
        {label}
      </Button>
    {/each}
    {#if isModified}
      <span
        class="ml-2 inline-flex items-center gap-1 rounded-md border border-amber-300 bg-amber-50 px-2 py-1 text-[11px] font-medium text-amber-700 dark:border-amber-600 dark:bg-amber-900/30 dark:text-amber-400"
      >
        Customized
      </span>
    {/if}
  </div>

  {#if shouldShowStatusRow}
    <div class="mt-3 flex flex-wrap items-center gap-2 text-[11px]">
      {#if Number(params.parameterOverrideCount ?? 0) > 0}
        <span class={`rounded-md border px-2 py-1 font-medium ${sourceTone}`}>
          {params.parameterOverrideCount} changed field{params.parameterOverrideCount === 1 ? "" : "s"}
        </span>
      {/if}
      {#if benchmarkProvenance}
        <span class="rounded-md border border-border/70 bg-muted/25 px-2 py-1 text-muted-foreground">
          Cloned from <strong class="text-foreground">{benchmarkProvenance.sourceLabel}</strong>
        </span>
      {/if}
    </div>
  {/if}

  <div class="mt-4">
    {#if activeMode === "benchmark"}
      <BenchmarkPanel
        {toggles}
        {disabledOptions}
        {isModified}
        {benchmarkProvenance}
        {onToggleChange}
        {onCloneBenchmarkToCustom}
      />
    {:else}
      <ScenarioSectionsPanel
        activeMode={activeMode}
        {toggles}
        {disabledOptions}
        {onToggleChange}
        {onParamEdit}
        {params}
        {validationErrors}
      />
    {/if}
  </div>

  {#if warningPolicy}
    <WarningPolicyPanel
      policy={warningPolicy}
      groups={["blockingValidation", "nonPhysical", "advisory"]}
      groupSources={{
        blockingValidation: ["validation"],
        nonPhysical: ["validation"],
        advisory: ["validation"],
      }}
    />
  {/if}
</Card>
