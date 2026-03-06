<script lang="ts">
  import GeometryGridQuickEditor from "./GeometryGridQuickEditor.svelte";
  import { GEOMETRY_GRID_QUICK_EDITOR } from "./geometryGridQuickEditor";
  import type { ModePanelParameterBindings } from "./modePanelTypes";

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

<GeometryGridQuickEditor
  editor={GEOMETRY_GRID_QUICK_EDITOR}
  bindings={params}
  fieldErrors={validationErrors}
  {onParamEdit}
  showHeader={false}
  hideQuickPickOptions={false}
/>