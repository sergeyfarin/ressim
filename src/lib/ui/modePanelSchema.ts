import type { CaseMode } from "../caseCatalog";

export type PermMode = "uniform" | "random" | "perLayer";

export type WellControlMode = "rate" | "pressure";

export type AnalyticalSolutionMode = "waterflood" | "depletion";

export type ModePanelSectionKey =
  | "geometry"
  | "reservoir"
  | "scal"
  | "wells"
  | "timestep"
  | "analytical";

export type ModePanelDimensionKey =
  | "geo"
  | "grid"
  | "rock"
  | "fluid"
  | "grav"
  | "cap"
  | "well"
  | "dt";

export type ModePanelSectionDefinition = {
  key: ModePanelSectionKey;
  label: string;
  dims: readonly ModePanelDimensionKey[];
  schemaKey?: "geometry-grid";
};

export const MODE_PANEL_SECTIONS: readonly ModePanelSectionDefinition[] = [
  {
    key: "geometry",
    label: "Geometry & Grid",
    dims: ["geo", "grid"],
    schemaKey: "geometry-grid",
  },
  {
    key: "reservoir",
    label: "Reservoir & Rock",
    dims: ["rock", "fluid", "grav"],
  },
  { key: "scal", label: "Relative Perm & Capillary", dims: ["cap"] },
  { key: "wells", label: "Wells", dims: ["well"] },
  { key: "timestep", label: "Timestep & Stability", dims: ["dt"] },
  { key: "analytical", label: "Analytical", dims: [] },
] as const;

export type GeometryGridParamKey =
  | "nx"
  | "ny"
  | "nz"
  | "cellDx"
  | "cellDy"
  | "cellDz";

export type ModePanelParameterBindings = {
  nx: number;
  ny: number;
  nz: number;
  cellDx: number;
  cellDy: number;
  cellDz: number;
  initialPressure: number;
  initialSaturation: number;
  reservoirPorosity: number;
  mu_w: number;
  mu_o: number;
  c_o: number;
  c_w: number;
  rho_w: number;
  rho_o: number;
  rock_compressibility: number;
  depth_reference: number;
  volume_expansion_o: number;
  volume_expansion_w: number;
  gravityEnabled: boolean;
  permMode: PermMode;
  uniformPermX: number;
  uniformPermY: number;
  uniformPermZ: number;
  useRandomSeed: boolean;
  randomSeed: number;
  minPerm: number;
  maxPerm: number;
  layerPermsX: number[];
  layerPermsY: number[];
  layerPermsZ: number[];
  s_wc: number;
  s_or: number;
  n_w: number;
  n_o: number;
  k_rw_max: number;
  k_ro_max: number;
  capillaryEnabled: boolean;
  capillaryPEntry: number;
  capillaryLambda: number;
  well_radius: number;
  well_skin: number;
  injectorEnabled: boolean;
  injectorControlMode: WellControlMode;
  producerControlMode: WellControlMode;
  injectorBhp: number;
  producerBhp: number;
  targetInjectorRate: number;
  targetProducerRate: number;
  injectorI: number;
  injectorJ: number;
  producerI: number;
  producerJ: number;
  delta_t_days: number;
  max_sat_change_per_step: number;
  max_pressure_change_per_step: number;
  max_well_rate_change_fraction: number;
  analyticalSolutionMode: AnalyticalSolutionMode;
  analyticalDepletionRateScale: number;
  parameterOverrideCount: number;
  handleNzOrPermModeChange: () => void;
  handleAnalyticalSolutionModeChange: (mode: AnalyticalSolutionMode) => void;
};

export type SchemaPatch<K extends string> = Partial<Record<K, number>>;

export type ControlChangeBehavior = "none" | "sync-layer-arrays";

export type QuickPickOption<K extends string> = {
  key: string;
  label: string;
  description?: string;
  patch: SchemaPatch<K>;
};

export type CustomEntryBehavior<K extends string> = {
  type: "inline-number";
  param: K;
  label: string;
  description?: string;
  min?: number;
  max?: number;
  step?: number;
  integer?: boolean;
  changeBehavior?: ControlChangeBehavior;
};

export type QuickPickControlDefinition<K extends string> = {
  type: "quick-picks";
  key: string;
  label: string;
  description?: string;
  param: K;
  options: readonly QuickPickOption<K>[];
  custom: CustomEntryBehavior<K>;
  fieldErrorKeys?: readonly string[];
};

export type NumberControlDefinition<K extends string> = {
  type: "number";
  key: string;
  label: string;
  description?: string;
  param: K;
  min?: number;
  max?: number;
  step?: number;
  integer?: boolean;
  unit?: string;
  fieldErrorKeys?: readonly string[];
  changeBehavior?: ControlChangeBehavior;
};

export type ControlDefinition<K extends string> =
  | QuickPickControlDefinition<K>
  | NumberControlDefinition<K>;

export type SchemaSectionDefinition<K extends string> = {
  key: string;
  label: string;
  description?: string;
  controls: readonly ControlDefinition<K>[];
};

export const GEOMETRY_GRID_SECTION_SCHEMA: SchemaSectionDefinition<GeometryGridParamKey> = {
  key: "geometry-grid",
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
      type: "number",
      key: "ny",
      label: "Y Cells",
      param: "ny",
      min: 1,
      step: 1,
      integer: true,
      unit: "cells",
      fieldErrorKeys: ["ny"],
    },
    {
      type: "number",
      key: "nz",
      label: "Z Cells",
      param: "nz",
      min: 1,
      step: 1,
      integer: true,
      unit: "cells",
      fieldErrorKeys: ["nz"],
      changeBehavior: "sync-layer-arrays",
    },
    {
      type: "number",
      key: "cellDx",
      label: "dX",
      param: "cellDx",
      min: 0.1,
      step: 0.1,
      unit: "m",
      fieldErrorKeys: ["cellDx"],
    },
    {
      type: "number",
      key: "cellDy",
      label: "dY",
      param: "cellDy",
      min: 0.1,
      step: 0.1,
      unit: "m",
      fieldErrorKeys: ["cellDy"],
    },
    {
      type: "number",
      key: "cellDz",
      label: "dZ",
      param: "cellDz",
      min: 0.1,
      step: 0.1,
      unit: "m",
      fieldErrorKeys: ["cellDz"],
    },
  ],
} as const;

export function getModePanelSections(_mode: CaseMode): readonly ModePanelSectionDefinition[] {
  return MODE_PANEL_SECTIONS;
}

export function getQuickPickMatch<K extends string>(
  control: QuickPickControlDefinition<K>,
  currentValue: unknown,
): QuickPickOption<K> | null {
  return (
    control.options.find((option) => {
      const patchValue = option.patch[control.param];
      return typeof patchValue === "number" && Number(currentValue) === patchValue;
    }) ?? null
  );
}

export function getControlErrorMessage(
  fieldErrors: Record<string, string>,
  fieldErrorKeys: readonly string[] | undefined,
): string | null {
  if (!fieldErrorKeys?.length) return null;
  for (const key of fieldErrorKeys) {
    if (fieldErrors[key]) return fieldErrors[key];
  }
  return null;
}
