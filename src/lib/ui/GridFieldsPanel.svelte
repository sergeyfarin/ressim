<script lang="ts">
  import GeometryGridQuickEditor from "./GeometryGridQuickEditor.svelte";
  import { GEOMETRY_GRID_QUICK_EDITOR } from "./geometryGridQuickEditor";
  import type { ModePanelParameterBindings } from "./modePanelTypes";

  // Keep this wrapper shared across modes.
  // If Depletion and Waterflood need different geometry toggles or extra inputs,
  // keep one `GridFieldsPanel` and swap the editor config here by mode instead of
  // splitting `ScenarioSectionsPanel` again.
  // Typical next step: pass `activeMode` in, choose a mode-specific editor
  // definition, and only add inline conditionals here for truly small deltas.

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

<!-- Shared geometry shell: vary editor definitions here before creating a second grid panel. -->
<GeometryGridQuickEditor
  editor={GEOMETRY_GRID_QUICK_EDITOR}
  bindings={params}
  fieldErrors={validationErrors}
  {onParamEdit}
  showHeader={false}
  hideQuickPickOptions={false}
/>