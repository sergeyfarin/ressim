import fs from "fs";
import path from "path";
import { describe, expect, it } from "vitest";

const modePanelPath = path.join(__dirname, "ModePanel.svelte");
const modePanelSource = fs.readFileSync(modePanelPath, "utf8");

const scenarioSectionsPath = path.join(__dirname, "ScenarioSectionsPanel.svelte");
const scenarioSectionsSource = fs.readFileSync(scenarioSectionsPath, "utf8");

describe("Mode panel composition", () => {
  it("routes each top-level mode through a dedicated component", () => {
    expect(modePanelSource).toMatch(/import\s+BenchmarkPanel\s+from\s+"\.\/BenchmarkPanel\.svelte"/);
    expect(modePanelSource).toMatch(/import\s+DepletionPanel\s+from\s+"\.\/DepletionPanel\.svelte"/);
    expect(modePanelSource).toMatch(/import\s+WaterfloodPanel\s+from\s+"\.\/WaterfloodPanel\.svelte"/);
    expect(modePanelSource).toMatch(/import\s+SimulationPanel\s+from\s+"\.\/SimulationPanel\.svelte"/);
    expect(modePanelSource).toMatch(/activeMode === "benchmark"[\s\S]*<BenchmarkPanel/);
    expect(modePanelSource).toMatch(/activeMode === "dep"[\s\S]*<DepletionPanel/);
    expect(modePanelSource).toMatch(/activeMode === "wf"[\s\S]*<WaterfloodPanel/);
    expect(modePanelSource).toMatch(/<SimulationPanel/);
  });

  it("keeps the shared scenario renderer focused on section composition", () => {
    expect(scenarioSectionsSource).toMatch(/getModePanelSections\(activeMode\)/);
    expect(scenarioSectionsSource).toMatch(/<SchemaSectionRenderer/);
    expect(scenarioSectionsSource).toMatch(/<ReservoirPropertiesPanel/);
    expect(scenarioSectionsSource).toMatch(/<RelativeCapillaryPanel/);
    expect(scenarioSectionsSource).toMatch(/<WellPropertiesPanel/);
    expect(scenarioSectionsSource).toMatch(/<TimestepControlsPanel/);
    expect(scenarioSectionsSource).toMatch(/<AnalyticalInputsPanel/);
  });
});
