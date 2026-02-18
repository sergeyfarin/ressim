<script lang="ts">
  import StaticPropertiesPanel from './StaticPropertiesPanel.svelte';
  import ReservoirPropertiesPanel from './ReservoirPropertiesPanel.svelte';
  import RelativeCapillaryPanel from './RelativeCapillaryPanel.svelte';
  import WellPropertiesPanel from './WellPropertiesPanel.svelte';
  import TimestepControlsPanel from './TimestepControlsPanel.svelte';

  // All the same bindings that the sidebar panels used to have
  export let nx: number;
  export let ny: number;
  export let nz: number;
  export let cellDx: number;
  export let cellDy: number;
  export let cellDz: number;
  export let initialPressure: number;
  export let initialSaturation: number;
  export let mu_w: number;
  export let mu_o: number;
  export let c_o: number;
  export let c_w: number;
  export let rho_w: number;
  export let rho_o: number;
  export let rock_compressibility: number;
  export let depth_reference: number;
  export let volume_expansion_o: number;
  export let volume_expansion_w: number;
  export let gravityEnabled: boolean;
  export let permMode: 'uniform' | 'random' | 'perLayer';
  export let uniformPermX: number;
  export let uniformPermY: number;
  export let uniformPermZ: number;
  export let useRandomSeed: boolean;
  export let randomSeed: number;
  export let minPerm: number;
  export let maxPerm: number;
  export let layerPermsX: number[];
  export let layerPermsY: number[];
  export let layerPermsZ: number[];
  export let s_wc: number;
  export let s_or: number;
  export let n_w: number;
  export let n_o: number;
  export let capillaryEnabled: boolean;
  export let capillaryPEntry: number;
  export let capillaryLambda: number;
  export let well_radius: number;
  export let well_skin: number;
  export let injectorEnabled: boolean;
  export let injectorControlMode: 'rate' | 'pressure';
  export let producerControlMode: 'rate' | 'pressure';
  export let injectorBhp: number;
  export let producerBhp: number;
  export let targetInjectorRate: number;
  export let targetProducerRate: number;
  export let injectorI: number;
  export let injectorJ: number;
  export let producerI: number;
  export let producerJ: number;
  export let delta_t_days: number;
  export let max_sat_change_per_step: number;
  export let max_pressure_change_per_step: number;
  export let max_well_rate_change_fraction: number;

  export let validationErrors: Record<string, string> = {};
  export let validationWarnings: string[] = [];
  export let readOnly: boolean = false;
</script>

<div class="grid grid-cols-1 gap-4 lg:grid-cols-2 xl:grid-cols-3">
  <div class="space-y-3">
    <StaticPropertiesPanel
      bind:nx bind:ny bind:nz
      bind:cellDx bind:cellDy bind:cellDz
    />
    <TimestepControlsPanel
      bind:delta_t_days
      bind:max_sat_change_per_step
      bind:max_pressure_change_per_step
      bind:max_well_rate_change_fraction
      fieldErrors={validationErrors}
    />
  </div>

  <div class="space-y-3">
    <ReservoirPropertiesPanel
      bind:initialPressure bind:initialSaturation
      bind:mu_w bind:mu_o
      bind:c_o bind:c_w
      bind:rho_w bind:rho_o
      bind:rock_compressibility
      bind:depth_reference
      bind:volume_expansion_o bind:volume_expansion_w
      bind:gravityEnabled
      bind:permMode
      bind:uniformPermX bind:uniformPermY bind:uniformPermZ
      bind:useRandomSeed bind:randomSeed
      bind:minPerm bind:maxPerm
      bind:nz
      bind:layerPermsX bind:layerPermsY bind:layerPermsZ
      fieldErrors={validationErrors}
    />
  </div>

  <div class="space-y-3">
    <RelativeCapillaryPanel
      bind:s_wc bind:s_or bind:n_w bind:n_o
      bind:capillaryEnabled
      bind:capillaryPEntry bind:capillaryLambda
    />
    <WellPropertiesPanel
      bind:well_radius bind:well_skin
      bind:nx bind:ny
      bind:injectorEnabled
      bind:injectorControlMode bind:producerControlMode
      bind:injectorBhp bind:producerBhp
      bind:targetInjectorRate bind:targetProducerRate
      bind:injectorI bind:injectorJ
      bind:producerI bind:producerJ
      fieldErrors={validationErrors}
    />
  </div>
</div>

{#if validationWarnings.length > 0}
  <div class="mt-3 card border border-warning bg-base-100 shadow-sm">
    <div class="card-body p-3 text-xs">
      {#each validationWarnings as warning}
        <div class="text-warning">⚠ {warning}</div>
      {/each}
    </div>
  </div>
{/if}

{#if readOnly}
  <div class="mt-3 text-xs opacity-60 text-center">
    Viewing pre-run case parameters. Switch to <strong>Custom</strong> mode to edit.
  </div>
{/if}
