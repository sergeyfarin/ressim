import fs from "fs";
import path from "path";
import { describe, expect, it } from "vitest";

const scenarioPickerSource = fs.readFileSync(
  path.join(__dirname, "modes", "ScenarioPicker.svelte"),
  "utf8",
);

const scenarioSectionsPath = path.join(
  __dirname,
  "sections",
  "ScenarioSectionsPanel.svelte",
);
const scenarioSectionsSource = fs.readFileSync(scenarioSectionsPath, "utf8");

const geometrySectionPath = path.join(__dirname, "sections", "GeometrySection.svelte");
const geometrySectionSource = fs.readFileSync(geometrySectionPath, "utf8");

describe("Scenario picker composition", () => {
  it("keeps the customize flow inside the scenario picker via ScenarioSectionsPanel", () => {
    expect(scenarioPickerSource).toMatch(/import\s+ScenarioSectionsPanel\s+from\s+"\.\.\/sections\/ScenarioSectionsPanel\.svelte"/);
    expect(scenarioPickerSource).toMatch(/<ScenarioSectionsPanel/);
    expect(scenarioPickerSource).toMatch(/Customize/);
    expect(scenarioPickerSource).toMatch(/onEnterCustomMode/);
    expect(scenarioPickerSource).not.toMatch(/BenchmarkPanel/);
    expect(scenarioPickerSource).not.toMatch(/ScenarioModePanel|DepletionPanel|WaterfloodPanel|SimulationPanel/);
  });

  it("keeps execution-set controls out of the scenario picker", () => {
    expect(scenarioPickerSource).not.toMatch(/onRunReferenceSelection/);
    expect(scenarioPickerSource).not.toMatch(/Execution Set/);
  });

  it("keeps the shared scenario renderer focused on section composition", () => {
    expect(scenarioSectionsSource).toMatch(/getModePanelSections\(\)/);
    expect(scenarioSectionsSource).toMatch(/<GeometrySection/);
    expect(scenarioSectionsSource).toMatch(/<ReservoirSection/);
    expect(scenarioSectionsSource).toMatch(/<WellsSection/);
    expect(scenarioSectionsSource).toMatch(/<TimestepSection/);
    expect(scenarioSectionsSource).toMatch(/<AnalyticalSection/);
    expect(scenarioSectionsSource).toMatch(/<RelativeCapillarySection/);
    expect(scenarioSectionsSource).toMatch(/section.key === "geometry"/);
    expect(scenarioSectionsSource).toMatch(/section.key === "scal"/);
  });

  it("keeps geometry grid editing behind a focused section component", () => {
    expect(geometrySectionSource).toMatch(/<Collapsible title="Grid"/);
    expect(geometrySectionSource).not.toMatch(/geometryGridQuickEditor/);
    expect(geometrySectionSource).toMatch(/function setInt\(param: "nx" \| "ny" \| "nz", raw: string\)/);
    expect(geometrySectionSource).toMatch(/function setFloat\(param: "cellDx" \| "cellDy" \| "cellDz", raw: string\)/);
  });

  it("binds section components directly instead of routing through wrapper-only field panels", () => {
    expect(scenarioSectionsSource).toMatch(/bind:well_radius=\{params\.well_radius\}/);
    expect(scenarioSectionsSource).toMatch(/bind:delta_t_days=\{params\.delta_t_days\}/);
    expect(scenarioSectionsSource).toMatch(/bindings=\{params\}/);
    expect(scenarioSectionsSource).toMatch(/bind:analyticalSolutionMode=\{params\.analyticalSolutionMode\}/);
    expect(scenarioSectionsSource).not.toMatch(/GridFieldsPanel|ReservoirFieldsPanel|WellsFieldsPanel|TimestepFieldsPanel|AnalyticalFieldsPanel/);
  });
});
