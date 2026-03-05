<script lang="ts">
  import Button from "../components/ui/Button.svelte";
  import FilterCard from "./FilterCard.svelte";
  import { type CaseMode, catalog, type ToggleState } from "../caseCatalog";
  import { getFacetOverrideGroups } from "../stores/phase2PresetContract";

  let {
    activeMode = "dep" as CaseMode,
    isModified = false,
    toggles = {},
    disabledOptions = {},
    onModeChange,
    onParamEdit,
    onToggleChange,
    onCustomizeFacet = (_dimKey: string) => {},
    onResetFacet = (_dimKey: string) => {},
    onCloneBenchmarkToCustom = () => {},
    activeCustomizeGroup = null,
    parameterOverrideGroups = {},
    benchmarkProvenance = null,
  } = $props<{
    activeMode: CaseMode;
    isModified?: boolean;
    toggles: ToggleState;
    disabledOptions: Record<string, Record<string, string>>;
    onModeChange: (mode: CaseMode) => void;
    onParamEdit: () => void;
    onToggleChange: (key: string, value: any) => void;
    onCustomizeFacet?: (dimensionKey: string) => void;
    onResetFacet?: (dimensionKey: string) => void;
    onCloneBenchmarkToCustom?: () => void;
    activeCustomizeGroup?: string | null;
    parameterOverrideGroups?: Record<string, string[]>;
    benchmarkProvenance?: {
      sourceBenchmarkId: string;
      sourceCaseKey: string;
      sourceLabel: string;
      clonedAtIso: string;
    } | null;
  }>();

  let isCollapsed = $state(false);

  const activeModeLabel = $derived(
    activeMode === "dep"
      ? "Depletion"
      : activeMode === "wf"
        ? "Waterflood"
        : activeMode === "benchmark"
          ? "Benchmarks"
          : "Simulation",
  );

  const activeBenchmark = $derived(
    activeMode === "benchmark"
      ? catalog.benchmarks.find((b) => b.key === toggles.benchmarkId)
      : null,
  );

  const activeModeDimensions = $derived(
    catalog.dimensions.filter((dim) => {
      // Benchmark mode only shows benchmarks picker
      if (activeMode === "benchmark" && dim.key !== "benchmarkId") return false;
      if (dim.key === "benchmarkId" && activeMode !== "benchmark") return false;
      // Dimensions shouldn't include 'mode', it's handled via the top bar tabs
      if (dim.key === "mode") return false;

      // Filter out options that belong to disabled options globally if they are totally disabled?
      // Not globally, keep the dimension card even if some are disabled.
      return true;
    }),
  );

  function groupsForFacet(dimKey: string): string[] {
    return getFacetOverrideGroups(dimKey);
  }

  function facetOverrideCount(dimKey: string): number {
    const groups = groupsForFacet(dimKey);
    return groups.reduce(
      (sum, group) => sum + (parameterOverrideGroups[group]?.length ?? 0),
      0,
    );
  }

  function facetCustomizeActive(dimKey: string): boolean {
    const groups = groupsForFacet(dimKey);
    return !!activeCustomizeGroup && groups.includes(activeCustomizeGroup);
  }
</script>

<div class="case-selector" class:collapsed={isCollapsed}>
  <!-- Row 1: Mode tabs + collapse toggle -->
  <div class="flex items-center justify-between gap-2 flex-wrap">
    <div class="flex items-center gap-2">
      {#each [["dep", "Depletion"], ["wf", "Waterflood"], ["sim", "Simulation"], ["benchmark", "Benchmarks"]] as [mode, label]}
        <Button
          size="sm"
          variant={activeMode === mode && !isModified ? "default" : "outline"}
          onclick={() => {
            onModeChange(mode as CaseMode);
            isCollapsed = false;
          }}
        >
          {label}
        </Button>
      {/each}
      {#if isModified}
        <span
          class="ml-2 inline-flex items-center gap-1 rounded-md border border-amber-300 bg-amber-50 px-2 py-1 text-[11px] font-medium text-amber-700"
          title="Current inputs include manual overrides over the selected preset"
        >
          Customized
        </span>
      {/if}
    </div>

    <button
      class="text-xs text-muted-foreground hover:text-foreground transition-colors flex items-center gap-1 font-medium px-2 py-1 rounded"
      onclick={() => (isCollapsed = !isCollapsed)}
    >
      {isCollapsed ? "▸ Expand" : "▾ Collapse"}
    </button>
  </div>

  {#if isCollapsed && !isModified}
    <!-- Collapsed summary -->
    <div
      class="mt-2 flex items-center gap-2 text-xs text-muted-foreground flex-wrap"
    >
      <span class="font-medium text-foreground">{activeModeLabel}</span>
      {#if activeMode === "benchmark" && activeBenchmark}
        <span>·</span>
        <span class="text-foreground font-semibold"
          >{activeBenchmark.label}</span
        >
      {:else}
        {#each activeModeDimensions as dim}
          {#if toggles[dim.key]}
            <span
              class="px-1.5 py-0.5 bg-muted rounded border border-border/50"
            >
              {dim.label}:
              <strong class="text-foreground"
                >{dim.options.find((o) => o.value === toggles[dim.key])
                  ?.label || toggles[dim.key]}</strong
              >
            </span>
          {/if}
        {/each}
      {/if}
    </div>
  {:else}
    {#if isModified}
      <div
        class="mt-3 text-sm text-amber-600 dark:text-amber-400/90 font-medium px-2 flex items-center gap-2"
      >
        <span>Parameter overrides active. </span>
        <span class="text-xs font-normal text-muted-foreground"
          >Use per-facet Customize actions below, or select a mode above to reset to predefined cases.</span
        >
      </div>
    {/if}
    <!-- Row 2: Toggle cards mapping directly from catalog -->
    <div
      class="flex flex-wrap gap-2 mt-3 animate-in fade-in slide-in-from-top-1 duration-200"
    >
      {#each activeModeDimensions as dim}
        <div class="flex flex-col gap-1">
          <FilterCard
            label={dim.label}
            options={dim.options.map((o) => o.value)}
            customLabels={dim.options.reduce(
              (acc, o) => ({ ...acc, [o.value]: o.label }),
              {},
            )}
            selected={toggles[dim.key]}
            disabled={Object.keys(disabledOptions[dim.key] || {})}
            disabledReasons={disabledOptions[dim.key] || {}}
            onchange={(val) => onToggleChange(dim.key, val)}
          />
          <button
            class={`self-start rounded px-1.5 py-0.5 text-[11px] font-medium transition-colors ${
              facetCustomizeActive(dim.key)
                ? "bg-primary/15 text-primary"
                : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
            }`}
            onclick={() => {
              onParamEdit();
              onCustomizeFacet(dim.key);
            }}
            title={`Customize fields related to ${dim.label}`}
          >
            Customize {dim.label}
          </button>
          {#if groupsForFacet(dim.key).length > 0}
            <button
              class="self-start rounded px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground hover:text-foreground hover:bg-muted/50 disabled:opacity-40 disabled:pointer-events-none transition-colors"
              onclick={() => onResetFacet(dim.key)}
              disabled={facetOverrideCount(dim.key) === 0}
              title={`Reset customized ${dim.label} fields back to preset values`}
            >
              Reset {dim.label}
            </button>
          {/if}
          {#if facetOverrideCount(dim.key) > 0}
            <div class="self-start text-[10px] font-medium text-muted-foreground px-1">
              {facetOverrideCount(dim.key)} changed field{facetOverrideCount(dim.key) === 1 ? "" : "s"}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <!-- Benchmark details -->
    {#if activeMode === "benchmark" && activeBenchmark}
      <div
        class="mt-2 border-t border-border/50 pt-2 shrink-0"
      >
        <div class="text-[11px] text-muted-foreground">
          <strong>{activeBenchmark.label}:</strong>
          {activeBenchmark.description}
        </div>
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
  {/if}
</div>

<style>
  .case-selector {
    background-color: hsl(var(--card));
    border: 1px solid hsl(var(--border) / 0.8);
    border-radius: var(--radius);
    padding: 12px 16px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
    transition: all 0.2s ease-in-out;
  }
</style>
