<script lang="ts">
  import ReservoirPropertiesPanel from "./ReservoirPropertiesPanel.svelte";
  import type { ModePanelParameterBindings } from "./modePanelTypes";

  let {
    params,
    validationErrors = {},
    onParamEdit: _onParamEdit = () => {},
  }: {
    params: ModePanelParameterBindings;
    validationErrors?: Record<string, string>;
    onParamEdit?: () => void;
  } = $props();
</script>

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