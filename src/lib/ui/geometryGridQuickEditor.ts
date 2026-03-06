export type GeometryGridParamKey =
  | "nx"
  | "ny"
  | "nz"
  | "cellDx"
  | "cellDy"
  | "cellDz";

export type GeometryGridPatch = Partial<Record<GeometryGridParamKey, number>>;

export type GeometryGridChangeBehavior = "none" | "sync-layer-arrays";

export type GeometryGridQuickPickOption = {
  key: string;
  label: string;
  description?: string;
  patch: GeometryGridPatch;
};

export type GeometryGridCustomNumberEntry = {
  type: "inline-number";
  param: GeometryGridParamKey;
  label: string;
  description?: string;
  min?: number;
  max?: number;
  step?: number;
  integer?: boolean;
  changeBehavior?: GeometryGridChangeBehavior;
};

export type GeometryGridQuickPickControl = {
  type: "quick-picks";
  key: string;
  label: string;
  description?: string;
  param: GeometryGridParamKey;
  options: readonly GeometryGridQuickPickOption[];
  custom: GeometryGridCustomNumberEntry;
  fieldErrorKeys?: readonly string[];
};

export type GeometryGridNumberControl = {
  type: "number";
  key: string;
  label: string;
  description?: string;
  param: GeometryGridParamKey;
  min?: number;
  max?: number;
  step?: number;
  integer?: boolean;
  unit?: string;
  fieldErrorKeys?: readonly string[];
  changeBehavior?: GeometryGridChangeBehavior;
};

export type GeometryGridControlDefinition =
  | GeometryGridQuickPickControl
  | GeometryGridNumberControl;

export type GeometryGridQuickEditorDefinition = {
  key: "geometry-grid-quick-editor";
  label: string;
  description?: string;
  controls: readonly GeometryGridControlDefinition[];
};

export const GEOMETRY_GRID_QUICK_EDITOR: GeometryGridQuickEditorDefinition = {
  key: "geometry-grid-quick-editor",
  label: "Geometry + Grid Overrides",
  description:
    "Preset facets choose a baseline. Use quick picks for common cell counts, then open Custom for exact grid edits.",
  controls: [
    {
      type: "quick-picks",
      key: "nx-quick-picks",
      label: "X Cells",
      description:
        "Fast presets for 1D/grid-density exploration. Use Custom when you need a non-catalog value.",
      param: "nx",
      fieldErrorKeys: ["nx"],
      options: [
        { key: "12", label: "12", patch: { nx: 12 } },
        { key: "24", label: "24", patch: { nx: 24 } },
        { key: "48", label: "48", patch: { nx: 48 } },
        { key: "96", label: "96", patch: { nx: 96 } },
      ],
      custom: {
        type: "inline-number",
        param: "nx",
        label: "Custom X cells",
        min: 1,
        step: 1,
        integer: true,
      },
    },
    {
      type: "quick-picks",
      key: "ny",
      label: "Y Cells",
      description:
        "Use a slim areal count quickly, then switch to Custom when you need an exact side resolution.",
      param: "ny",
      fieldErrorKeys: ["ny"],
      options: [
        { key: "1", label: "1", patch: { ny: 1 } },
        { key: "11", label: "11", patch: { ny: 11 } },
        { key: "21", label: "21", patch: { ny: 21 } },
        { key: "41", label: "41", patch: { ny: 41 } },
      ],
      custom: {
        type: "inline-number",
        param: "ny",
        label: "Custom Y cells",
        min: 1,
        step: 1,
        integer: true,
      },
    },
    {
      type: "quick-picks",
      key: "nz",
      label: "Z Cells",
      description:
        "Pick a common layer count first. Use Custom when you need a specific layering depth.",
      param: "nz",
      fieldErrorKeys: ["nz"],
      options: [
        { key: "1", label: "1", patch: { nz: 1 } },
        { key: "3", label: "3", patch: { nz: 3 } },
        { key: "5", label: "5", patch: { nz: 5 } },
        { key: "10", label: "10", patch: { nz: 10 } },
      ],
      custom: {
        type: "inline-number",
        param: "nz",
        label: "Custom Z cells",
        min: 1,
        step: 1,
        integer: true,
        changeBehavior: "sync-layer-arrays",
      },
    },
    {
      type: "quick-picks",
      key: "cellDx",
      label: "dX",
      description:
        "Common x-direction cell lengths for fast coarse-to-fine sweeps.",
      param: "cellDx",
      fieldErrorKeys: ["cellDx"],
      options: [
        { key: "5", label: "5 m", patch: { cellDx: 5 } },
        { key: "10", label: "10 m", patch: { cellDx: 10 } },
        { key: "20", label: "20 m", patch: { cellDx: 20 } },
        { key: "40", label: "40 m", patch: { cellDx: 40 } },
      ],
      custom: {
        type: "inline-number",
        param: "cellDx",
        label: "Custom dX",
        min: 0.1,
        step: 0.1,
      },
    },
    {
      type: "quick-picks",
      key: "cellDy",
      label: "dY",
      description:
        "Common y-direction cell lengths for quick areal spacing adjustments.",
      param: "cellDy",
      fieldErrorKeys: ["cellDy"],
      options: [
        { key: "5", label: "5 m", patch: { cellDy: 5 } },
        { key: "10", label: "10 m", patch: { cellDy: 10 } },
        { key: "20", label: "20 m", patch: { cellDy: 20 } },
        { key: "40", label: "40 m", patch: { cellDy: 40 } },
      ],
      custom: {
        type: "inline-number",
        param: "cellDy",
        label: "Custom dY",
        min: 0.1,
        step: 0.1,
      },
    },
    {
      type: "quick-picks",
      key: "cellDz",
      label: "dZ",
      description:
        "Common vertical cell thickness presets for thin-bed and coarse-layer cases.",
      param: "cellDz",
      fieldErrorKeys: ["cellDz"],
      options: [
        { key: "1", label: "1 m", patch: { cellDz: 1 } },
        { key: "5", label: "5 m", patch: { cellDz: 5 } },
        { key: "10", label: "10 m", patch: { cellDz: 10 } },
        { key: "20", label: "20 m", patch: { cellDz: 20 } },
      ],
      custom: {
        type: "inline-number",
        param: "cellDz",
        label: "Custom dZ",
        min: 0.1,
        step: 0.1,
      },
    },
  ],
} as const;

export function isGeometryGridQuickPickControl(
  control: GeometryGridControlDefinition,
): control is GeometryGridQuickPickControl {
  return control.type === "quick-picks";
}

export function getGeometryGridQuickPickMatch(
  control: GeometryGridQuickPickControl,
  currentValue: unknown,
): GeometryGridQuickPickOption | null {
  return (
    control.options.find((option) => {
      const patchValue = option.patch[control.param];
      return typeof patchValue === "number" && Number(currentValue) === patchValue;
    }) ?? null
  );
}

export function getGeometryGridControlErrorMessage(
  fieldErrors: Record<string, string>,
  fieldErrorKeys: readonly string[] | undefined,
): string | null {
  if (!fieldErrorKeys?.length) return null;
  for (const key of fieldErrorKeys) {
    if (fieldErrors[key]) return fieldErrors[key];
  }
  return null;
}
