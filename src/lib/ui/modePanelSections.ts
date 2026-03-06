import type { CaseMode } from "../caseCatalog";

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
};

export const MODE_PANEL_SECTIONS: readonly ModePanelSectionDefinition[] = [
  {
    key: "geometry",
    label: "Geometry & Grid",
    dims: ["geo", "grid"],
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

export function getModePanelSections(
  _mode: CaseMode,
): readonly ModePanelSectionDefinition[] {
  return MODE_PANEL_SECTIONS;
}
