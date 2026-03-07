import fs from "fs";
import path from "path";
import { describe, expect, it } from "vitest";

const modePanelPath = path.join(__dirname, "modes", "ModePanel.svelte");
const modePanelSource = fs.readFileSync(modePanelPath, "utf8");

const scenarioSectionsPath = path.join(
  __dirname,
  "sections",
  "ScenarioSectionsPanel.svelte",
);
const scenarioSectionsSource = fs.readFileSync(scenarioSectionsPath, "utf8");

const geometrySectionPath = path.join(__dirname, "sections", "GeometrySection.svelte");
const geometrySectionSource = fs.readFileSync(geometrySectionPath, "utf8");

describe("Mode panel composition", () => {
  it("keeps the reference customize flow inline instead of routing through a benchmark-named inputs panel", () => {
    expect(modePanelSource).toMatch(/import\s+ScenarioSectionsPanel\s+from\s+"\.\.\/sections\/ScenarioSectionsPanel\.svelte"/);
    expect(modePanelSource).toMatch(/const FAMILY_LABELS =/);
    expect(modePanelSource).toMatch(/handleFamilySelect/);
    expect(modePanelSource).toMatch(/const RUN_POLICY_LABELS =/);
    expect(modePanelSource).toMatch(/const PANEL_LABELS =/);
    expect(modePanelSource).toMatch(/const caseDisclosure = \$derived\.by/);
    expect(modePanelSource).not.toMatch(/\[\["dep", "Depletion"\], \["wf", "Waterflood"\], \["sim", "Simulation"\], \["benchmark", "Benchmarks"\]\]/);
    expect(modePanelSource).toMatch(/showReferencePanel/);
    expect(modePanelSource).toMatch(/Customize/);
    expect(modePanelSource).toMatch(/Seeded source: <strong class="text-foreground">\{referenceProvenance\.sourceLabel\}<\/strong>/);
    expect(modePanelSource).toMatch(/<ScenarioSectionsPanel/);
    expect(modePanelSource).not.toMatch(/BenchmarkPanel/);
    expect(modePanelSource).not.toMatch(/ScenarioModePanel|DepletionPanel|WaterfloodPanel|SimulationPanel/);
  });

  it("uses the family-local Case Library as the only reference entry selector in the inputs shell", () => {
    expect(modePanelSource).toMatch(/Case Library/);
    expect(modePanelSource).toMatch(/onActivateLibraryEntry\(entry.key\)/);
    expect(modePanelSource).not.toMatch(/FilterCard|getModeDimensions|showLegacyReferenceSelector/);
  });

  it("keeps execution-set controls out of the inputs shell now that run-region execution is explicit", () => {
    expect(modePanelSource).not.toMatch(/onRunReferenceSelection/);
    expect(modePanelSource).not.toMatch(/Execution Set/);
  });

  it("keeps the shared scenario renderer focused on section composition", () => {
    expect(scenarioSectionsSource).toMatch(/getModePanelSections\(activeMode\)/);
    expect(scenarioSectionsSource).toMatch(/<GeometrySection/);
    expect(scenarioSectionsSource).toMatch(/<ReservoirSection/);
    expect(scenarioSectionsSource).toMatch(/<WellsSection/);
    expect(scenarioSectionsSource).toMatch(/<TimestepSection/);
    expect(scenarioSectionsSource).toMatch(/<AnalyticalSection/);
    expect(scenarioSectionsSource).toMatch(/<RelativeCapillarySection/);
    expect(scenarioSectionsSource).toMatch(/section.key === "geometry"/);
    expect(scenarioSectionsSource).toMatch(/section.key === "scal"/);
  });

  it("keeps geometry quick-edit wiring behind a focused section component", () => {
    expect(geometrySectionSource).toMatch(/const GEOMETRY_GRID_CONTROLS/);
    expect(geometrySectionSource).not.toMatch(/geometryGridQuickEditor/);
    expect(geometrySectionSource).toMatch(/function getQuickPickMatch/);
  });

  it("binds section components directly instead of routing through wrapper-only field panels", () => {
    expect(scenarioSectionsSource).toMatch(/bind:well_radius=\{params\.well_radius\}/);
    expect(scenarioSectionsSource).toMatch(/bind:delta_t_days=\{params\.delta_t_days\}/);
    expect(scenarioSectionsSource).toMatch(/bind:initialPressure=\{params\.initialPressure\}/);
    expect(scenarioSectionsSource).toMatch(/bind:analyticalSolutionMode=\{params\.analyticalSolutionMode\}/);
    expect(scenarioSectionsSource).not.toMatch(/GridFieldsPanel|ReservoirFieldsPanel|WellsFieldsPanel|TimestepFieldsPanel|AnalyticalFieldsPanel/);
  });
});
