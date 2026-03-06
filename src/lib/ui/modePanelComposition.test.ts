import fs from "fs";
import path from "path";
import { describe, expect, it } from "vitest";

const modePanelPath = path.join(__dirname, "ModePanel.svelte");
const modePanelSource = fs.readFileSync(modePanelPath, "utf8");

const scenarioSectionsPath = path.join(__dirname, "ScenarioSectionsPanel.svelte");
const scenarioSectionsSource = fs.readFileSync(scenarioSectionsPath, "utf8");

const gridFieldsPanelPath = path.join(__dirname, "GridFieldsPanel.svelte");
const gridFieldsPanelSource = fs.readFileSync(gridFieldsPanelPath, "utf8");

const timestepFieldsPanelPath = path.join(__dirname, "TimestepFieldsPanel.svelte");
const timestepFieldsPanelSource = fs.readFileSync(timestepFieldsPanelPath, "utf8");

const wellsFieldsPanelPath = path.join(__dirname, "WellsFieldsPanel.svelte");
const wellsFieldsPanelSource = fs.readFileSync(wellsFieldsPanelPath, "utf8");

const reservoirFieldsPanelPath = path.join(__dirname, "ReservoirFieldsPanel.svelte");
const reservoirFieldsPanelSource = fs.readFileSync(reservoirFieldsPanelPath, "utf8");

const analyticalFieldsPanelPath = path.join(__dirname, "AnalyticalFieldsPanel.svelte");
const analyticalFieldsPanelSource = fs.readFileSync(analyticalFieldsPanelPath, "utf8");

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
    expect(scenarioSectionsSource).toMatch(/const WRAPPED_SECTION_COMPONENTS =/);
    expect(scenarioSectionsSource).toMatch(/geometry:\s*GridFieldsPanel/);
    expect(scenarioSectionsSource).toMatch(/reservoir:\s*ReservoirFieldsPanel/);
    expect(scenarioSectionsSource).toMatch(/wells:\s*WellsFieldsPanel/);
    expect(scenarioSectionsSource).toMatch(/timestep:\s*TimestepFieldsPanel/);
    expect(scenarioSectionsSource).toMatch(/analytical:\s*AnalyticalFieldsPanel/);
    expect(scenarioSectionsSource).toMatch(/getWrappedSectionComponent\(section\)/);
    expect(scenarioSectionsSource).toMatch(/<WrappedSectionComponent/);
    expect(scenarioSectionsSource).toMatch(/<RelativeCapillaryPanel/);
    expect(scenarioSectionsSource).toMatch(/section.key === "scal"/);
  });

  it("keeps grid-field quick-edit wiring behind a focused subcomponent", () => {
    expect(gridFieldsPanelSource).toMatch(/import\s+GeometryGridQuickEditor\s+from\s+"\.\/GeometryGridQuickEditor\.svelte"/);
    expect(gridFieldsPanelSource).toMatch(/GEOMETRY_GRID_QUICK_EDITOR/);
    expect(gridFieldsPanelSource).toMatch(/<GeometryGridQuickEditor/);
  });

  it("keeps timestep and wells wiring behind focused section-body components", () => {
    expect(timestepFieldsPanelSource).toMatch(/import\s+TimestepControlsPanel\s+from\s+"\.\/TimestepControlsPanel\.svelte"/);
    expect(timestepFieldsPanelSource).toMatch(/<TimestepControlsPanel/);
    expect(wellsFieldsPanelSource).toMatch(/import\s+WellPropertiesPanel\s+from\s+"\.\/WellPropertiesPanel\.svelte"/);
    expect(wellsFieldsPanelSource).toMatch(/<WellPropertiesPanel/);
  });

  it("keeps reservoir and analytical wiring behind focused section-body components", () => {
    expect(reservoirFieldsPanelSource).toMatch(/import\s+ReservoirPropertiesPanel\s+from\s+"\.\/ReservoirPropertiesPanel\.svelte"/);
    expect(reservoirFieldsPanelSource).toMatch(/<ReservoirPropertiesPanel/);
    expect(analyticalFieldsPanelSource).toMatch(/import\s+AnalyticalInputsPanel\s+from\s+"\.\/AnalyticalInputsPanel\.svelte"/);
    expect(analyticalFieldsPanelSource).toMatch(/<AnalyticalInputsPanel/);
  });
});
