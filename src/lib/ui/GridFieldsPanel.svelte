<script lang="ts">
  import GeometryGridQuickEditor from "./GeometryGridQuickEditor.svelte";
  import type { ModePanelParameterBindings } from "./modePanelTypes";

  // Keep this wrapper shared across modes.
  // If Depletion and Waterflood need different geometry toggles or extra inputs,
  // keep those deltas in `GeometryGridQuickEditor.svelte` or pass small mode-aware
  // props down from here instead of reviving a separate TS control schema.

  let {
    params,
    validationErrors = {},
    onParamEdit = () => {},
  }: {
    params: ModePanelParameterBindings;
    validationErrors?: Record<string, string>;
    onParamEdit?: () => void;
  } = $props();
</script>

<!-- Shared geometry shell: keep geometry-specific control details inside the geometry component. -->
<GeometryGridQuickEditor
  bindings={params}
  fieldErrors={validationErrors}
  {onParamEdit}
  showHeader={false}
  hideQuickPickOptions={false}
/>