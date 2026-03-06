<script lang="ts">
  import FilterCard from "./FilterCard.svelte";
  import StaticPropertiesPanel from "./StaticPropertiesPanel.svelte";
  import ReservoirPropertiesPanel from "./ReservoirPropertiesPanel.svelte";
  import RelativeCapillaryPanel from "./RelativeCapillaryPanel.svelte";
  import WellPropertiesPanel from "./WellPropertiesPanel.svelte";
  import TimestepControlsPanel from "./TimestepControlsPanel.svelte";
  import AnalyticalInputsPanel from "./AnalyticalInputsPanel.svelte";
  import Button from "../components/ui/Button.svelte";
  import {
    type CaseMode,
    catalog,
    getModeDimensions,
    type Dimension,
    type ToggleState,
  } from "../caseCatalog";

  /**
   * Maps each panel section to the catalog dimension keys it covers.
   * Sections with empty dims (e.g. "analytical") always render.
   */
  const SECTIONS = [
    { key: "geometry", label: "Geometry & Grid", dims: ["geo", "grid"] },
    {
      key: "reservoir",
      label: "Reservoir & Rock",
      dims: ["rock", "fluid", "grav"],
    },
    { key: "scal", label: "Relative Perm & Capillary", dims: ["cap"] },
    { key: "wells", label: "Wells", dims: ["well"] },
    { key: "timestep", label: "Timestep & Stability", dims: ["dt"] },
    { key: "analytical", label: "Analytical", dims: [] },
  ];

  let {
    activeMode = "dep" as CaseMode,
    isModified = false,
    toggles = {} as ToggleState,
    disabledOptions = {} as Record<string, Record<string, string>>,
    onModeChange,
    onToggleChange,
    benchmarkProvenance = null as {
      sourceBenchmarkId: string;
      sourceCaseKey: string;
      sourceLabel: string;
      clonedAtIso: string;
    } | null,
    onCloneBenchmarkToCustom = () => {},
    params,
    validationErrors = {} as Record<string, string>,
    validationWarnings = [] as string[],
  }: {
    activeMode: CaseMode;
    isModified?: boolean;
    toggles: ToggleState;
    disabledOptions: Record<string, Record<string, string>>;
    onModeChange: (mode: CaseMode) => void;
    onToggleChange: (key: string, value: any) => void;
    benchmarkProvenance?: {
      sourceBenchmarkId: string;
      sourceCaseKey: string;
      sourceLabel: string;
      clonedAtIso: string;
    } | null;
    onCloneBenchmarkToCustom?: () => void;
    params: any;
    validationErrors?: Record<string, string>;
    validationWarnings?: string[];
  } = $props();

  let expandedSections = $state<Record<string, boolean>>({});

  const modeDimensions = $derived(getModeDimensions(activeMode));

  const activeBenchmark = $derived(
    activeMode === "benchmark"
      ? catalog.benchmarks.find((b) => b.key === toggles.benchmarkId)
      : null,
  );

  function toggleSection(key: string) {
    expandedSections = { ...expandedSections, [key]: !expandedSections[key] };
  }

  function getSectionDims(dimKeys: readonly string[]): Dimension[] {
    return modeDimensions.filter((d) => dimKeys.includes(d.key));
  }
</script>

<div class="mode-panel">
  <!-- Mode tabs -->
  <div class="flex items-center gap-2 flex-wrap">
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

  {#if activeMode === "benchmark"}
    <!-- Benchmark mode: single selector + details -->
    <div class="mt-4 space-y-3">
      {#each modeDimensions as dim}
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
      {/each}
      {#if activeBenchmark}
        <div class="border-t border-border/50 pt-2">
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
                Cloned source: <strong class="text-foreground"
                  >{benchmarkProvenance.sourceLabel}</strong
                >
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
  {:else}
    <!-- Non-benchmark: grouped sections with sub-selectors + inline parameters -->
    <div class="mt-4 space-y-2">
      {#each SECTIONS as section}
        {@const dims = getSectionDims(section.dims)}
        {@const isExpanded = !!expandedSections[section.key]}
        {#if dims.length > 0 || section.dims.length === 0}
          <div class="section-card">
            <div class="section-header">
              <button
                class="expand-btn"
                onclick={() => toggleSection(section.key)}
                title={isExpanded
                  ? "Collapse parameters"
                  : "Expand parameters"}
              >
                <span class="chevron">{isExpanded ? "▾" : "▸"}</span>
                <span class="section-label">{section.label}</span>
              </button>
              {#if dims.length > 0}
                <div class="flex flex-wrap gap-1.5" role="group">
                  {#each dims as dim}
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
                  {/each}
                </div>
              {/if}
            </div>

            {#if isExpanded}
              <div class="section-body">
                {#if section.key === "geometry"}
                  <StaticPropertiesPanel
                    bind:nx={params.nx}
                    bind:ny={params.ny}
                    bind:nz={params.nz}
                    bind:cellDx={params.cellDx}
                    bind:cellDy={params.cellDy}
                    bind:cellDz={params.cellDz}
                    fieldErrors={validationErrors}
                    onNzOrPermModeChange={params.handleNzOrPermModeChange}
                  />
                {:else if section.key === "reservoir"}
                  <ReservoirPropertiesPanel
                    bind:initialPressure={params.initialPressure}
                    bind:initialSaturation={params.initialSaturation}
                    bind:reservoirPorosity={params.reservoirPorosity}
                    bind:mu_w={params.mu_w}
                    bind:mu_o={params.mu_o}
                    bind:c_o={params.c_o}
                    bind:c_w={params.c_w}
                    bind:rho_w={params.rho_w}
                    bind:rho_o={params.rho_o}
                    bind:rock_compressibility={params.rock_compressibility}
                    bind:depth_reference={params.depth_reference}
                    bind:volume_expansion_o={params.volume_expansion_o}
                    bind:volume_expansion_w={params.volume_expansion_w}
                    bind:gravityEnabled={params.gravityEnabled}
                    bind:permMode={params.permMode}
                    bind:uniformPermX={params.uniformPermX}
                    bind:uniformPermY={params.uniformPermY}
                    bind:uniformPermZ={params.uniformPermZ}
                    bind:useRandomSeed={params.useRandomSeed}
                    bind:randomSeed={params.randomSeed}
                    bind:minPerm={params.minPerm}
                    bind:maxPerm={params.maxPerm}
                    bind:nz={params.nz}
                    bind:layerPermsX={params.layerPermsX}
                    bind:layerPermsY={params.layerPermsY}
                    bind:layerPermsZ={params.layerPermsZ}
                    onNzOrPermModeChange={params.handleNzOrPermModeChange}
                    fieldErrors={validationErrors}
                  />
                {:else if section.key === "scal"}
                  <RelativeCapillaryPanel
                    bind:s_wc={params.s_wc}
                    bind:s_or={params.s_or}
                    bind:n_w={params.n_w}
                    bind:n_o={params.n_o}
                    bind:k_rw_max={params.k_rw_max}
                    bind:k_ro_max={params.k_ro_max}
                    bind:capillaryEnabled={params.capillaryEnabled}
                    bind:capillaryPEntry={params.capillaryPEntry}
                    bind:capillaryLambda={params.capillaryLambda}
                    fieldErrors={validationErrors}
                  />
                {:else if section.key === "wells"}
                  <WellPropertiesPanel
                    bind:well_radius={params.well_radius}
                    bind:well_skin={params.well_skin}
                    bind:nx={params.nx}
                    bind:ny={params.ny}
                    bind:injectorEnabled={params.injectorEnabled}
                    bind:injectorControlMode={params.injectorControlMode}
                    bind:producerControlMode={params.producerControlMode}
                    bind:injectorBhp={params.injectorBhp}
                    bind:producerBhp={params.producerBhp}
                    bind:targetInjectorRate={params.targetInjectorRate}
                    bind:targetProducerRate={params.targetProducerRate}
                    bind:injectorI={params.injectorI}
                    bind:injectorJ={params.injectorJ}
                    bind:producerI={params.producerI}
                    bind:producerJ={params.producerJ}
                    fieldErrors={validationErrors}
                  />
                {:else if section.key === "timestep"}
                  <TimestepControlsPanel
                    bind:delta_t_days={params.delta_t_days}
                    bind:max_sat_change_per_step={params.max_sat_change_per_step}
                    bind:max_pressure_change_per_step={params.max_pressure_change_per_step}
                    bind:max_well_rate_change_fraction={params.max_well_rate_change_fraction}
                    fieldErrors={validationErrors}
                  />
                {:else if section.key === "analytical"}
                  <AnalyticalInputsPanel
                    bind:analyticalSolutionMode={params.analyticalSolutionMode}
                    bind:analyticalDepletionRateScale={params.analyticalDepletionRateScale}
                    onAnalyticalSolutionModeChange={params.handleAnalyticalSolutionModeChange}
                  />
                {/if}
              </div>
            {/if}
          </div>
        {/if}
      {/each}
    </div>
  {/if}

  {#if validationWarnings.length > 0}
    <div
      class="mt-3 rounded-xl border border-warning bg-card text-card-foreground shadow-sm"
    >
      <div class="p-3 text-xs">
        {#each validationWarnings as warning}
          <div class="text-warning font-medium">⚠ {warning}</div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .mode-panel {
    background-color: hsl(var(--card));
    border: 1px solid hsl(var(--border) / 0.8);
    border-radius: var(--radius);
    padding: 12px 16px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .section-card {
    border: 1px solid hsl(var(--border) / 0.6);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: hsl(var(--muted) / 0.3);
    flex-wrap: wrap;
  }

  .expand-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    border-radius: var(--radius);
    color: hsl(var(--muted-foreground));
    transition:
      color 0.15s,
      background-color 0.15s;
    cursor: pointer;
    white-space: nowrap;
    border: none;
    background: none;
    font-family: inherit;
  }

  .expand-btn:hover {
    color: hsl(var(--foreground));
    background-color: hsl(var(--muted) / 0.5);
  }

  .chevron {
    font-size: 10px;
    width: 10px;
    text-align: center;
  }

  .section-label {
    font-size: 12px;
    font-weight: 600;
  }

  .section-body {
    border-top: 1px solid hsl(var(--border) / 0.4);
  }
</style>
