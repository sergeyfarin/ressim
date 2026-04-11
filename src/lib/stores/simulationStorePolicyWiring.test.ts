import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

// Concatenate all three stores so patterns can span whichever sub-file they landed in.
const storeSource = [
    'parameterStore.svelte.ts',
    'runtimeStore.svelte.ts',
    'navigationStore.svelte.ts',
].map((f) => fs.readFileSync(path.join(__dirname, f), 'utf8')).join('\n');

describe('simulation store policy wiring', () => {
  it('uses the shared auto-clear policy for modified-state reset behavior', () => {
    expect(storeSource).toMatch(/shouldAutoClearModifiedState/);
    expect(storeSource).toMatch(/referenceProvenance/);
    expect(storeSource).toMatch(/referenceProvenance,\s*parameterOverrideCount/);
    expect(storeSource).toMatch(/parameterOverrideCount/);
    expect(storeSource).not.toMatch(/get benchmarkProvenance\(\)/);
  });

  it('uses the shared clone policy for benchmark clone-to-custom flow', () => {
    expect(storeSource).toMatch(/shouldAllowReferenceClone/);
    expect(storeSource).toMatch(/cloneActiveReferenceToCustom\(\): boolean/);
    expect(storeSource).toMatch(/hasReferenceLibraryCase: Boolean\(this\.activeNavigationLibraryEntry\)/);
    expect(storeSource).not.toMatch(/cloneActiveBenchmarkToCustom:/);
  });

  it('derives reference metadata from the active library entry instead of the legacy benchmark tab alone', () => {
    expect(storeSource).toMatch(/benchmarkId = this\.activeReferenceFamily\?\.key/);
    expect(storeSource).toMatch(/activeLibraryEntry\?\.group/);
    expect(storeSource).toMatch(/activeLibraryEntry\?\.label/);
  });

  it('wires benchmark sweep actions and normalized benchmark results through the store', () => {
    expect(storeSource).toMatch(/buildBenchmarkRunSpecs/);
    expect(storeSource).toMatch(/runActiveReferenceSelection\(variantKeys: string\[\] = \[\]\): boolean/);
    expect(storeSource).toMatch(/referenceRunResults/);
    expect(storeSource).toMatch(/activeReferenceFamily/);
    expect(storeSource).toMatch(/referenceRunResults = \$state/);
    expect(storeSource).not.toMatch(/get benchmarkRunResults\(\)/);
    expect(storeSource).not.toMatch(/runActiveBenchmarkSelection:/);
    expect(storeSource).toMatch(/activateLibraryEntry\(entryKey: string\): boolean/);
    expect(storeSource).toMatch(/explicitLibraryEntryKey/);
  });

  it('treats scenario runtime controls as explicit overrides for sensitivity sweeps', () => {
    expect(storeSource).toMatch(/hasUserDeltaTDaysOverride = \$state\(false\)/);
    expect(storeSource).toMatch(/hasUserStepsOverride = \$state\(false\)/);
    expect(storeSource).toMatch(/markDeltaTDaysOverride\(\)/);
    expect(storeSource).toMatch(/markStepsOverride\(\)/);
    expect(storeSource).toMatch(/const runSteps = this\.#params\.hasUserStepsOverride/);
    expect(storeSource).toMatch(/variantParams\.steps \?\? baseParams\.steps/);
    expect(storeSource).toMatch(/const runDeltaTDays = this\.#params\.hasUserDeltaTDaysOverride/);
    expect(storeSource).toMatch(/steps: runSteps/);
    expect(storeSource).toMatch(/deltaTDays: runDeltaTDays/);
    expect(storeSource).toMatch(/clearRuntimeOverrides\(\)/);
  });

  it('persists solver selection from scenario params into runtime payload state', () => {
    expect(storeSource).toMatch(/fimEnabled = \$state\(true\)/);
    expect(storeSource).toMatch(/this\.fimEnabled = resolved\.fimEnabled !== false;/);
  });

  it('persists depletion analytical late-time clipping settings from scenario params', () => {
    expect(storeSource).toMatch(/analyticalDepletionStartDays = \$state\(0\.0\)/);
    expect(storeSource).toMatch(/this\.analyticalDepletionStartDays = fin\(resolved\.analyticalDepletionStartDays, 0\.0\);/);
  });

  it('clears stale reference comparisons when sensitivity selection changes', () => {
    expect(storeSource).toMatch(/selectSensitivityDimension\(dimensionKey: string\)/);
    expect(storeSource).toMatch(/if \(this\.#runtime\.referenceSweepRunning \|\| this\.#runtime\.activeReferenceRunSpec\) return;/);
    expect(storeSource).toMatch(/this\.activeComparisonSelection = buildComparisonSelection\(\);[\s\S]*this\.#runtime\.clearReferenceRunnerState\(true\);[\s\S]*this\.activeSensitivityDimensionKey = dimensionKey;/);
    expect(storeSource).toMatch(/toggleScenarioVariant\(variantKey: string\)/);
    expect(storeSource).toMatch(/toggleScenarioVariant\(variantKey: string\)[\s\S]*this\.activeComparisonSelection = buildComparisonSelection\(\);[\s\S]*this\.#runtime\.clearReferenceRunnerState\(true\);/);
  });

  it('exposes compatibility navigation state alongside legacy mode state', () => {
    expect(storeSource).not.toMatch(/buildScenarioNavigationState/);
    expect(storeSource).toMatch(/resolveProductFamily/);
    expect(storeSource).toMatch(/resolveScenarioSource/);
    expect(storeSource).toMatch(/resolveCaseLibraryEntryFromScenario/);
    expect(storeSource).toMatch(/get activeFamily\(\) \{ return this\.navigationState\.activeFamily; \}/);
    expect(storeSource).toMatch(/get activeSource\(\) \{ return this\.navigationState\.activeSource; \}/);
    expect(storeSource).toMatch(/get sourceLabel\(\) \{ return this\.navigationState\.sourceLabel; \}/);
    expect(storeSource).toMatch(/get referenceSourceLabel\(\) \{ return this\.navigationState\.referenceSourceLabel; \}/);
    expect(storeSource).toMatch(/get provenanceSummary\(\) \{ return this\.navigationState\.provenanceSummary; \}/);
    expect(storeSource).toMatch(/navigationState = \$derived\.by/);
    expect(storeSource).toMatch(/setComparisonSelection\(selection: Partial<ComparisonSelection>\)/);
  });
});
