<script lang="ts">
  import WellPropertiesPanel from "./WellPropertiesPanel.svelte";
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