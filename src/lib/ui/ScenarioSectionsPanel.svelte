<script lang="ts">
  import FilterCard from "./FilterCard.svelte";
  import RelativeCapillaryPanel from "./RelativeCapillaryPanel.svelte";
  import ReservoirFieldsPanel from "./ReservoirFieldsPanel.svelte";
  import AnalyticalFieldsPanel from "./AnalyticalFieldsPanel.svelte";
  import GridFieldsPanel from "./GridFieldsPanel.svelte";
  import TimestepFieldsPanel from "./TimestepFieldsPanel.svelte";
  import WellsFieldsPanel from "./WellsFieldsPanel.svelte";
  import {
    getModeDimensions,
    type Dimension,
  } from "../caseCatalog";
  import {
    getModePanelSections,
    type ModePanelSectionDefinition,
  } from "./modePanelSections";
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
  const WRAPPED_SECTION_COMPONENTS = {
    geometry: GridFieldsPanel,
    reservoir: ReservoirFieldsPanel,
    wells: WellsFieldsPanel,
    timestep: TimestepFieldsPanel,
    analytical: AnalyticalFieldsPanel,
  } as const;

  function toggleSection(key: string) {
    expandedSections = { ...expandedSections, [key]: !expandedSections[key] };
  }

  function getSectionDims(dimKeys: readonly string[]): Dimension[] {
    return modeDimensions.filter((dim) => dimKeys.includes(dim.key));
  }

  // Keep wrapper-based dispatch local so this shared panel stays a section orchestrator.
  function getWrappedSectionComponent(section: ModePanelSectionDefinition) {
    if (section.key === "scal") return null;
    return WRAPPED_SECTION_COMPONENTS[section.key];
  }

  function handleManualFieldEdit() {
    onParamEdit();
  }
</script>

<div class="space-y-2">
  {#each sections as section}
    {@const dims = getSectionDims(section.dims)}
    {@const isExpanded = !!expandedSections[section.key]}
    {@const WrappedSectionComponent = getWrappedSectionComponent(section)}
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
            {#if WrappedSectionComponent}
              <WrappedSectionComponent
                {params}
                validationErrors={validationErrors}
                {onParamEdit}
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
