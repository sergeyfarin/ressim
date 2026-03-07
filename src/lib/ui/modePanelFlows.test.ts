import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const modePanelPath = path.join(__dirname, 'modes', 'ModePanel.svelte');
const modePanelSource = fs.readFileSync(modePanelPath, 'utf8');

describe('mode panel flows', () => {
  it('shows the status row when clone provenance or tracked overrides exist', () => {
    expect(modePanelSource).toMatch(/shouldShowModePanelStatusRow/);
    expect(modePanelSource).toMatch(/parameterOverrideCount: Number\(params\.parameterOverrideCount \?\? 0\)/);
    expect(modePanelSource).toMatch(/\{params\.parameterOverrideCount\} changed field/);
    expect(modePanelSource).toMatch(/Seeded from <strong class="text-foreground">\{referenceProvenance\.sourceLabel\}<\/strong>/);
  });

  it('surfaces current library identity and a grouped family-local case-library selector in the mode panel', () => {
    expect(modePanelSource).toMatch(/Inputs/);
    expect(modePanelSource).toMatch(/Type Curves/);
    expect(modePanelSource).toMatch(/Source/);
    expect(modePanelSource).toMatch(/Case Library/);
    expect(modePanelSource).toMatch(/Custom/);
    expect(modePanelSource).toMatch(/handleSourceSelect/);
    expect(modePanelSource).toMatch(/Library Context/);
    expect(modePanelSource).toMatch(/Case Disclosure/);
    expect(modePanelSource).toMatch(/Citation \/ Source/);
    expect(modePanelSource).toMatch(/Fixed Settings/);
    expect(modePanelSource).toMatch(/Allowed Sensitivities/);
    expect(modePanelSource).toMatch(/Reference Policy/);
    expect(modePanelSource).toMatch(/Current facet combination is not mapped to a curated library case yet\./);
    expect(modePanelSource).toMatch(/Case Library/);
    expect(modePanelSource).toMatch(/Literature References/);
    expect(modePanelSource).toMatch(/Curated Starters/);
    expect(modePanelSource).toMatch(/onActivateLibraryEntry\(entry\.key\)/);
    expect(modePanelSource).toMatch(/handleFamilySelect/);
    expect(modePanelSource).toMatch(/variant=\{activeFamily === family \? "default" : "outline"\}/);
    expect(modePanelSource).toMatch(/activeLibraryEntry\.referencePolicySummary/);
    expect(modePanelSource).toMatch(/activeLibraryEntry\.sensitivitySummary/);
  });

  it('keeps reference customize flow explicit in the inputs shell without a benchmark-named wrapper panel', () => {
    expect(modePanelSource).toMatch(/Customize/);
    expect(modePanelSource).toMatch(/disabled=\{isModified \|\| referenceSweepRunning\}/);
    expect(modePanelSource).toMatch(/Seeded source: <strong class="text-foreground">\{referenceProvenance\.sourceLabel\}<\/strong>/);
    expect(modePanelSource).toMatch(/Customized without source provenance/);
    expect(modePanelSource).toMatch(/navigationState\?\.activeLibraryCaseKey \?\? null/);
    expect(modePanelSource).not.toMatch(/BenchmarkPanel/);
    expect(modePanelSource).not.toMatch(/Reference Sweep Status/);
    expect(modePanelSource).not.toMatch(/Execution Set/);
    expect(modePanelSource).not.toMatch(/Stored Reference Results/);
  });

  it('keeps validation warnings scoped to the mode panel warning surface', () => {
    expect(modePanelSource).toMatch(/<WarningPolicyPanel/);
    expect(modePanelSource).toMatch(/groups=\{\["blockingValidation", "nonPhysical", "advisory"\]\}/);
    expect(modePanelSource).toMatch(/blockingValidation: \["validation"\]/);
    expect(modePanelSource).toMatch(/nonPhysical: \["validation"\]/);
    expect(modePanelSource).toMatch(/advisory: \["validation"\]/);
  });
});