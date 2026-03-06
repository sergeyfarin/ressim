import fs from "fs";
import path from "path";
import { describe, expect, it } from "vitest";

const modePanelPath = path.join(__dirname, "modes", "ModePanel.svelte");
const modePanelSource = fs.readFileSync(modePanelPath, "utf8");

const scenarioModePanelPath = path.join(
  __dirname,
  "modes",
  "ScenarioModePanel.svelte",
);
const scenarioModePanelSource = fs.readFileSync(scenarioModePanelPath, "utf8");

const scenarioSectionsPath = path.join(
  __dirname,
  "sections",
  "ScenarioSectionsPanel.svelte",
);
const scenarioSectionsSource = fs.readFileSync(scenarioSectionsPath, "utf8");

const geometrySectionPath = path.join(__dirname, "sections", "GeometrySection.svelte");
const geometrySectionSource = fs.readFileSync(geometrySectionPath, "utf8");

describe("Mode panel composition", () => {
  it("routes each top-level mode through a dedicated component", () => {
    expect(modePanelSource).toMatch(/import\s+BenchmarkPanel\s+from\s+"\.\/BenchmarkPanel\.svelte"/);
    expect(modePanelSource).toMatch(/import\s+ScenarioModePanel\s+from\s+"\.\/ScenarioModePanel\.svelte"/);
    expect(modePanelSource).toMatch(/activeMode === "benchmark"[\s\S]*<BenchmarkPanel/);
    expect(modePanelSource).toMatch(/<ScenarioModePanel/);
    expect(modePanelSource).not.toMatch(/DepletionPanel|WaterfloodPanel|SimulationPanel/);
    expect(scenarioModePanelSource).toMatch(/<ScenarioSectionsPanel/);
    expect(scenarioModePanelSource).toMatch(/\{activeMode\}/);
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
