<script lang="ts">
  import type { ScenarioModePanelProps } from "../modePanelTypes";
  import ScenarioSectionsPanel from "../sections/ScenarioSectionsPanel.svelte";

  let {
    activeMode,
    toggles,
    disabledOptions,
    onToggleChange,
    onParamEdit = () => {},
    params,
    validationErrors = {},
  }: ScenarioModePanelProps = $props();
</script>

<ScenarioSectionsPanel
  {activeMode}
  {toggles}
  {disabledOptions}
  {onToggleChange}
  {onParamEdit}
  {params}
  {validationErrors}
/>