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
  activeMode="sim"
  {toggles}
  {disabledOptions}
  {onToggleChange}
  {onParamEdit}
  {params}
  {validationErrors}
/>
