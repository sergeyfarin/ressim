<script lang="ts">
  import type { ScenarioModePanelProps } from "../modePanelTypes";
  import GasFluidSection from "./GasFluidSection.svelte";
  import GeometrySection from "./GeometrySection.svelte";
  import RelativeCapillarySection from "./RelativeCapillarySection.svelte";
  import ReservoirSection from "./ReservoirSection.svelte";
  import FluidPropertiesSection from "./FluidPropertiesSection.svelte";
  import TimestepSection from "./TimestepSection.svelte";
  import WellsSection from "./WellsSection.svelte";

  let {
    activeMode,
    toggles: _toggles,
    disabledOptions: _disabledOptions,
    onToggleChange: _onToggleChange,
    onParamEdit = () => {},
    params,
    validationErrors = {},
  }: ScenarioModePanelProps = $props();
</script>

<div class="sections-flow" oninput={onParamEdit} onchange={onParamEdit}>
  <div class="flow-item">
    <GeometrySection
      bindings={params}
      fieldErrors={validationErrors}
      {onParamEdit}
      showHeader={false}
      hideQuickPickOptions={false}
    />
  </div>

  <div class="flow-item">
    <ReservoirSection
      bindings={params}
      onNzOrPermModeChange={params.handleNzOrPermModeChange}
      fieldErrors={validationErrors}
    />
  </div>

  <div class="flow-item {params.pvtMode === 'black-oil' ? 'col-span-all' : ''}">
    <FluidPropertiesSection
      bindings={params}
      fieldErrors={validationErrors}
    />
  </div>

  <div class="flow-item">
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
  </div>

  {#if activeMode === "3p"}
    <div class="flow-item">
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
    </div>
  {/if}

  <div class="flow-item">
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
  </div>

  <div class="flow-item">
    <TimestepSection
      bind:delta_t_days={params.delta_t_days}
      bind:max_sat_change_per_step={params.max_sat_change_per_step}
      bind:max_pressure_change_per_step={params.max_pressure_change_per_step}
      bind:max_well_rate_change_fraction={params.max_well_rate_change_fraction}
      onDeltaTDaysEdit={params.markDeltaTDaysOverride}
      fieldErrors={validationErrors}
    />
  </div>

</div>

<style>
  .sections-flow {
    columns: 300px;
    column-gap: 0.375rem;
    padding: 0.25rem 0.25rem 0.5rem;
  }

  .flow-item {
    break-inside: avoid;
    margin-bottom: 0.375rem;
  }

  .col-span-all {
    column-span: all;
    width: 100%;
  }
</style>
