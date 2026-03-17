<script lang="ts">
  import {
    getModeDimensions,
    type Dimension,
  } from "../../catalog/caseCatalog";
  import FilterCard from "../controls/FilterCard.svelte";
  import {
    getModePanelSections,
  } from "../modePanelSections";
  import type { ScenarioModePanelProps } from "../modePanelTypes";
  import AnalyticalSection from "./AnalyticalSection.svelte";
  import GasFluidSection from "./GasFluidSection.svelte";
  import GeometrySection from "./GeometrySection.svelte";
  import RelativeCapillarySection from "./RelativeCapillarySection.svelte";
  import ReservoirSection from "./ReservoirSection.svelte";
  import TimestepSection from "./TimestepSection.svelte";
  import WellsSection from "./WellsSection.svelte";

  let {
    activeMode,
    toggles,
    disabledOptions,
    onToggleChange,
    onParamEdit = () => {},
    params,
    validationErrors = {},
  }: ScenarioModePanelProps = $props();

  // Adjust section order or mode-specific visibility in `modePanelSections.ts`.
  const sections = $derived(getModePanelSections());

  // Adjust these header chips in `catalog/caseCatalog.ts` when a case needs different quick toggles.
  const modeDimensions = $derived(getModeDimensions(activeMode));

  function getSectionDims(dimKeys: readonly string[]): Dimension[] {
    return modeDimensions.filter((dim) => dimKeys.includes(dim.key));
  }

</script>

<div class="space-y-1.5 px-2 py-2">
  {#each sections as section}
    {@const dims = getSectionDims(section.dims)}
    {#if (dims.length > 0 || section.dims.length === 0) && (section.key !== "gasfluid" || activeMode === "3p") && (section.key !== "analytical" || activeMode !== "3p")}
      <div>
        {#if dims.length > 0}
          <div class="flex flex-wrap items-center gap-1.5 px-1 pb-1" role="group">
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
        <div oninput={onParamEdit} onchange={onParamEdit}>
          {#if section.key === "geometry"}
            <GeometrySection
              bindings={params}
              fieldErrors={validationErrors}
              {onParamEdit}
              showHeader={false}
              hideQuickPickOptions={false}
            />
          {:else if section.key === "reservoir"}
            <ReservoirSection
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
              bind:nz={params.nz}
              bind:layerPermsX={params.layerPermsX}
              bind:layerPermsY={params.layerPermsY}
              bind:layerPermsZ={params.layerPermsZ}
              onNzOrPermModeChange={params.handleNzOrPermModeChange}
              fieldErrors={validationErrors}
            />
          {:else if section.key === "wells"}
            <WellsSection
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
            <TimestepSection
              bind:delta_t_days={params.delta_t_days}
              bind:max_sat_change_per_step={params.max_sat_change_per_step}
              bind:max_pressure_change_per_step={params.max_pressure_change_per_step}
              bind:max_well_rate_change_fraction={params.max_well_rate_change_fraction}
              fieldErrors={validationErrors}
            />
          {:else if section.key === "analytical"}
            <AnalyticalSection
              bind:analyticalSolutionMode={params.analyticalSolutionMode}
              bind:analyticalDepletionRateScale={params.analyticalDepletionRateScale}
              onAnalyticalSolutionModeChange={params.handleAnalyticalSolutionModeChange}
            />
          {:else if section.key === "scal"}
            <RelativeCapillarySection
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
          {:else if section.key === "gasfluid"}
            <GasFluidSection
              bind:s_gc={params.s_gc}
              bind:s_gr={params.s_gr}
              bind:n_g={params.n_g}
              bind:k_rg_max={params.k_rg_max}
              bind:mu_g={params.mu_g}
              bind:c_g={params.c_g}
              bind:rho_g={params.rho_g}
              bind:pcogEnabled={params.pcogEnabled}
              bind:pcogPEntry={params.pcogPEntry}
              bind:pcogLambda={params.pcogLambda}
              bind:injectedFluid={params.injectedFluid}
              fieldErrors={validationErrors}
            />
          {/if}
        </div>
      </div>
    {/if}
  {/each}
</div>
