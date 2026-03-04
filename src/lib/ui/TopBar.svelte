<script lang="ts">
  import Button from "../components/ui/Button.svelte";
  import FilterCard from "./FilterCard.svelte";
  import {
    type CaseMode,
    caseCatalog,
    FACET_OPTIONS,
    type CaseEntry,
  } from "../caseCatalog";

  let {
    activeMode = "depletion" as CaseMode,
    activeCase = "",
    isCustomMode = false,
    customSubCase = null,
    onModeChange,
    onCaseChange,
    onCustomMode,
  } = $props<{
    activeMode: CaseMode;
    activeCase: string;
    isCustomMode?: boolean;
    customSubCase?: { key: string; label: string } | null;
    onModeChange: (mode: CaseMode) => void;
    onCaseChange: (key: string) => void;
    onCustomMode: () => void;
  }>();

  let isCollapsed = $state(false);

  let filters = $state({
    geometry: [] as string[],
    permeability: [] as string[],
    physics: [] as string[],
    fluids: [] as string[],
    study: [] as string[],
  });

  // Reset filters when mode changes
  $effect(() => {
    activeMode;
    filters.geometry = [];
    filters.permeability = [];
    filters.physics = [];
    filters.fluids = [];
    filters.study = [];
  });

  const matchingCases = $derived(
    caseCatalog.filter((c: CaseEntry) => {
      if (c.facets.mode !== activeMode) return false;
      if (
        filters.geometry.length &&
        !filters.geometry.includes(c.facets.geometry)
      )
        return false;
      if (
        filters.permeability.length &&
        !filters.permeability.includes(c.facets.permeability)
      )
        return false;
      if (filters.physics.includes("Gravity") && !c.facets.gravity)
        return false;
      if (filters.physics.includes("Capillary") && !c.facets.capillary)
        return false;
      if (
        filters.fluids.length &&
        !filters.fluids.some((f) => c.facets.fluidVariation.includes(f))
      )
        return false;
      if (
        filters.study.length &&
        !filters.study.some((s) => c.facets.studyType.includes(s))
      )
        return false;
      return true;
    }),
  );

  const activeModeLabel = $derived(
    activeMode === "depletion"
      ? "Depletion"
      : activeMode === "waterflood"
        ? "Waterflood"
        : "Simulation",
  );
</script>

<div class="case-selector" class:collapsed={isCollapsed}>
  <!-- Row 1: Mode tabs + collapse toggle -->
  <div class="flex items-center gap-2 flex-wrap">
    <Button
      size="sm"
      variant={activeMode === "depletion" && !isCustomMode
        ? "default"
        : "outline"}
      onclick={() => {
        onModeChange("depletion");
        isCollapsed = false;
      }}
    >
      Depletion
    </Button>
    <Button
      size="sm"
      variant={activeMode === "waterflood" && !isCustomMode
        ? "default"
        : "outline"}
      onclick={() => {
        onModeChange("waterflood");
        isCollapsed = false;
      }}
    >
      Waterflood
    </Button>
    <Button
      size="sm"
      variant={activeMode === "simulation" && !isCustomMode
        ? "default"
        : "outline"}
      onclick={() => {
        onModeChange("simulation");
        isCollapsed = false;
      }}
    >
      Simulation
    </Button>
    <Button
      size="sm"
      variant={isCustomMode ? "default" : "outline"}
      onclick={onCustomMode}
    >
      ⚙ Custom
    </Button>

    <button
      class="ml-auto text-xs text-muted-foreground hover:text-foreground transition-colors flex items-center gap-1 font-medium px-2 py-1 rounded"
      onclick={() => (isCollapsed = !isCollapsed)}
    >
      {#if isCollapsed}
        ▸ Expand Filters
      {:else}
        ▾ Collapse
      {/if}
    </button>
  </div>

  {#if isCollapsed && !isCustomMode}
    <!-- Collapsed summary strip -->
    <div
      class="mt-2 flex items-center gap-2 text-xs text-muted-foreground flex-wrap"
    >
      <span class="font-medium text-foreground">{activeModeLabel}</span>

      {#each Object.entries(filters) as [key, vals]}
        {#if vals.length > 0}
          <span>&middot;</span>
          <span class="capitalize">{vals.join(", ")}</span>
        {/if}
      {/each}

      <span class="ml-2 font-medium bg-muted px-1.5 py-0.5 rounded text-[10px]"
        >{matchingCases.length} cases</span
      >

      <span class="ml-auto flex items-center gap-1">
        Active: <span class="text-foreground font-semibold"
          >{caseCatalog.find((c) => c.key === activeCase)?.label ??
            "Select a case"}</span
        >
      </span>
    </div>
  {:else if !isCollapsed && !isCustomMode}
    <!-- Row 2: Filter cards -->
    <div
      class="flex flex-wrap gap-2 mt-3 animate-in fade-in slide-in-from-top-1 duration-200"
    >
      <FilterCard
        label="Geometry"
        options={FACET_OPTIONS.geometry}
        bind:selected={filters.geometry}
      />
      <FilterCard
        label="Rock"
        options={FACET_OPTIONS.permeability}
        bind:selected={filters.permeability}
      />
      <FilterCard
        label="Physics"
        options={["Gravity", "Capillary"]}
        bind:selected={filters.physics}
      />
      <FilterCard
        label="Fluids"
        options={FACET_OPTIONS.fluidVariation}
        bind:selected={filters.fluids}
      />
      <FilterCard
        label="Study"
        options={FACET_OPTIONS.studyType}
        bind:selected={filters.study}
      />

      <!-- Clear filters button if any active -->
      {#if filters.geometry.length || filters.permeability.length || filters.physics.length || filters.fluids.length || filters.study.length}
        <button
          class="text-[10px] uppercase font-bold text-muted-foreground hover:text-destructive transition-colors px-2 self-start mt-2"
          onclick={() => {
            filters.geometry = [];
            filters.permeability = [];
            filters.physics = [];
            filters.fluids = [];
            filters.study = [];
          }}>Clear</button
        >
      {/if}
    </div>

    <!-- Row 3: Matching cases strip -->
    <div class="flex flex-col gap-2 mt-4 pt-3 border-t border-border/50">
      <div class="flex flex-wrap gap-1.5 items-center">
        <span class="text-xs font-semibold text-muted-foreground mr-1 w-16">
          {matchingCases.length}
          {matchingCases.length === 1 ? "case" : "cases"}:
        </span>
        {#if matchingCases.length === 0}
          <span class="text-xs text-muted-foreground italic"
            >No cases match this combination of filters.</span
          >
        {:else}
          {#each matchingCases as c}
            <Button
              size="xs"
              variant={activeCase === c.key ? "default" : "outline"}
              onclick={() => onCaseChange(c.key)}
              title={c.description}
              class={activeCase === c.key
                ? "ring-1 ring-primary overflow-hidden"
                : "opacity-80 hover:opacity-100"}
              style="max-width: 180px;"
            >
              <div
                class="truncate text-[11px] font-medium flex items-center gap-1.5 w-full"
              >
                <!-- Runtime indicator dot -->
                <div
                  class="w-1.5 h-1.5 rounded-full shrink-0
                                  {c.runTimeEstimate === 'fast'
                    ? 'bg-emerald-500'
                    : c.runTimeEstimate === 'medium'
                      ? 'bg-amber-500'
                      : 'bg-rose-500'}"
                  title="Est. runtime: {c.runTimeEstimate}"
                ></div>
                <span class="truncate">{c.label}</span>
              </div>
            </Button>
          {/each}
        {/if}
      </div>

      <!-- Active case description -->
      {#if activeCase}
        {@const activeDetails = caseCatalog.find((c) => c.key === activeCase)}
        {#if activeDetails}
          <div
            class="text-[11px] text-muted-foreground ml-[70px] bg-muted/40 px-2 py-1 rounded inline-block max-w-fit"
          >
            <span class="font-bold mr-1">▸ {activeDetails.label}:</span>
            {activeDetails.description}
          </div>
        {/if}
      {/if}
    </div>
  {:else if isCustomMode}
    <div class="mt-3 text-sm text-muted-foreground px-2">
      You are editing simulation parameters directly. Select a preset mode above
      to return to defined cases.
      {#if customSubCase}
        <div class="mt-1 font-medium text-foreground">
          {customSubCase.label} active
        </div>
      {/if}
    </div>
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
