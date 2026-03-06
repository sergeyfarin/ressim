<script lang="ts">
  import ScenarioSectionsPanel from "./ScenarioSectionsPanel.svelte";
  import type { ScenarioModePanelContentProps } from "./modePanelTypes";

  let {
    toggles,
    disabledOptions,
    onToggleChange,
    onParamEdit = () => {},
    params,
    validationErrors = {},
  }: ScenarioModePanelContentProps = $props();
</script>

<ScenarioSectionsPanel
  activeMode="wf"
  {toggles}
  {disabledOptions}
  {onToggleChange}
  {onParamEdit}
  {params}
  {validationErrors}
/>
