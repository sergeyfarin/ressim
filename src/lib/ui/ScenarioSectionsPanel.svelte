<script lang="ts">
  import FilterCard from "./FilterCard.svelte";
  import ReservoirPropertiesPanel from "./ReservoirPropertiesPanel.svelte";
  import RelativeCapillaryPanel from "./RelativeCapillaryPanel.svelte";
  import WellPropertiesPanel from "./WellPropertiesPanel.svelte";
  import TimestepControlsPanel from "./TimestepControlsPanel.svelte";
  import AnalyticalInputsPanel from "./AnalyticalInputsPanel.svelte";
  import SchemaSectionRenderer from "./SchemaSectionRenderer.svelte";
  import {
    getModeDimensions,
    type Dimension,
  } from "../caseCatalog";
  import {
    GEOMETRY_GRID_SECTION_SCHEMA,
    getModePanelSections,
    type ModePanelSectionDefinition,
  } from "./modePanelSchema";
  import type { ScenarioModePanelProps } from "./modePanelTypes";

  let {
    activeMode,
    toggles,
    disabledOptions,
    onToggleChange,
    onParamEdit = () => {},
    params,
    validationErrors = {},
  }: ScenarioModePanelProps = $props();

  let expandedSections = $state<Record<string, boolean>>({});

  const sections = $derived(getModePanelSections(activeMode));
  const modeDimensions = $derived(getModeDimensions(activeMode));

  function toggleSection(key: string) {
    expandedSections = { ...expandedSections, [key]: !expandedSections[key] };
  }

  function getSectionDims(dimKeys: readonly string[]): Dimension[] {
    return modeDimensions.filter((dim) => dimKeys.includes(dim.key));
  }

  function sectionHasSchema(section: ModePanelSectionDefinition): boolean {
    return section.schemaKey === "geometry-grid";
  }

  function handleManualFieldEdit() {
    onParamEdit();
  }
</script>

<div class="space-y-2">
  {#each sections as section}
    {@const dims = getSectionDims(section.dims)}
    {@const isExpanded = !!expandedSections[section.key]}
    {#if dims.length > 0 || section.dims.length === 0}
      <div class="section-card">
        <div class="section-header">
          <button
            class="expand-btn"
            onclick={() => toggleSection(section.key)}
            title={isExpanded ? "Collapse parameters" : "Expand parameters"}
          >
            <span class="chevron">{isExpanded ? "▾" : "▸"}</span>
            <span class="section-label">{section.label}</span>
          </button>
          {#if dims.length > 0}
            <div class="flex flex-wrap gap-1.5" role="group">
              {#each dims as dim}
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
            </div>
          {/if}
        </div>

        {#if isExpanded}
          <div class="section-body" oninput={handleManualFieldEdit} onchange={handleManualFieldEdit}>
            {#if sectionHasSchema(section)}
              <SchemaSectionRenderer
                section={GEOMETRY_GRID_SECTION_SCHEMA}
                bindings={params}
                fieldErrors={validationErrors}
                {onParamEdit}
                showHeader={false}
                hideQuickPickOptions={true}
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

<style>
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
