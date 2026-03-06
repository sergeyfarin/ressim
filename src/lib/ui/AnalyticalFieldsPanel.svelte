<script lang="ts">
  import AnalyticalInputsPanel from "./AnalyticalInputsPanel.svelte";
  import type { ModePanelParameterBindings } from "./modePanelTypes";

  let {
    params,
    validationErrors: _validationErrors = {},
    onParamEdit: _onParamEdit = () => {},
  }: {
    params: ModePanelParameterBindings;
    validationErrors?: Record<string, string>;
    onParamEdit?: () => void;
  } = $props();
</script>

<AnalyticalInputsPanel
  bind:analyticalSolutionMode={params.analyticalSolutionMode}
  bind:analyticalDepletionRateScale={params.analyticalDepletionRateScale}
  onAnalyticalSolutionModeChange={params.handleAnalyticalSolutionModeChange}
/>