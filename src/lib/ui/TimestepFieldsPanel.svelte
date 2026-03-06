<script lang="ts">
  import TimestepControlsPanel from "./TimestepControlsPanel.svelte";
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

<TimestepControlsPanel
  bind:delta_t_days={params.delta_t_days}
  bind:max_sat_change_per_step={params.max_sat_change_per_step}
  bind:max_pressure_change_per_step={params.max_pressure_change_per_step}
  bind:max_well_rate_change_fraction={params.max_well_rate_change_fraction}
  fieldErrors={validationErrors}
/>