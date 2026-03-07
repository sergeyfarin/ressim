import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const modePanelPath = path.join(__dirname, 'modes', 'ModePanel.svelte');
const modePanelSource = fs.readFileSync(modePanelPath, 'utf8');

const benchmarkPanelPath = path.join(__dirname, 'modes', 'BenchmarkPanel.svelte');
const benchmarkPanelSource = fs.readFileSync(benchmarkPanelPath, 'utf8');

describe('mode panel flows', () => {
  it('shows the status row when clone provenance or tracked overrides exist', () => {
    expect(modePanelSource).toMatch(/shouldShowModePanelStatusRow/);
    expect(modePanelSource).toMatch(/parameterOverrideCount: Number\(params\.parameterOverrideCount \?\? 0\)/);
    expect(modePanelSource).toMatch(/\{params\.parameterOverrideCount\} changed field/);
    expect(modePanelSource).toMatch(/Cloned from <strong class="text-foreground">\{benchmarkProvenance\.sourceLabel\}<\/strong>/);
  });

  it('keeps benchmark clone flow explicit in the benchmark panel', () => {
    expect(benchmarkPanelSource).toMatch(/Clone to Custom/);
    expect(benchmarkPanelSource).toMatch(/disabled=\{isModified \|\| benchmarkSweepRunning\}/);
    expect(benchmarkPanelSource).toMatch(/Cloned source: <strong class="text-foreground">\{benchmarkProvenance\.sourceLabel\}<\/strong>/);
    expect(benchmarkPanelSource).toMatch(/Customized without clone provenance/);
  });
  
  it('uses a single execution-set selector for benchmark runs instead of one button per axis', () => {
    expect(benchmarkPanelSource).toMatch(/Execution Set/);
    expect(benchmarkPanelSource).toMatch(/selectedExecutionRow/);
    expect(benchmarkPanelSource).toMatch(/selectedVariantKeys/);
    expect(benchmarkPanelSource).toMatch(/onRunBenchmarkSelection/);
    expect(benchmarkPanelSource).toMatch(/type="checkbox"/);
    expect(benchmarkPanelSource).toMatch(/Run Base/);
  });
  
  it('scopes stored benchmark result cards to the active family', () => {
    expect(benchmarkPanelSource).toMatch(/activeBenchmarkResults/);
    expect(benchmarkPanelSource).toMatch(/result\.familyKey === activeFamily\.key/);
  });

  it('keeps validation warnings scoped to the mode panel warning surface', () => {
    expect(modePanelSource).toMatch(/<WarningPolicyPanel/);
    expect(modePanelSource).toMatch(/groups=\{\["blockingValidation", "nonPhysical", "advisory"\]\}/);
    expect(modePanelSource).toMatch(/blockingValidation: \["validation"\]/);
    expect(modePanelSource).toMatch(/nonPhysical: \["validation"\]/);
    expect(modePanelSource).toMatch(/advisory: \["validation"\]/);
  });
});