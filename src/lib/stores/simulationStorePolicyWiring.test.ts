import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const storePath = path.join(__dirname, 'simulationStore.svelte.ts');
const storeSource = fs.readFileSync(storePath, 'utf8');

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
    expect(storeSource).toMatch(/function cloneActiveReferenceToCustom\(\): boolean/);
    expect(storeSource).toMatch(/hasReferenceLibraryCase: Boolean\(activeReferenceBenchmarkFamily\)/);
    expect(storeSource).not.toMatch(/cloneActiveBenchmarkToCustom:/);
  });

  it('derives reference metadata from the active library entry instead of the legacy benchmark tab alone', () => {
    expect(storeSource).toMatch(/benchmarkId = activeReferenceBenchmarkFamily\?\.key/);
    expect(storeSource).toMatch(/activeLibraryEntry\?\.group/);
    expect(storeSource).toMatch(/activeLibraryEntry\?\.label/);
  });

  it('wires benchmark sweep actions and normalized benchmark results through the store', () => {
    expect(storeSource).toMatch(/buildBenchmarkRunSpecs/);
    expect(storeSource).toMatch(/function runActiveReferenceSelection\(variantKeys: string\[\] = \[\]\): boolean/);
    expect(storeSource).toMatch(/referenceRunResults/);
    expect(storeSource).toMatch(/activeReferenceBenchmarkFamily/);
    expect(storeSource).toMatch(/get referenceRunResults\(\) \{ return referenceRunResults; \}/);
    expect(storeSource).not.toMatch(/get benchmarkRunResults\(\)/);
    expect(storeSource).not.toMatch(/runActiveBenchmarkSelection:/);
    expect(storeSource).toMatch(/function activateLibraryEntry\(entryKey: string\): boolean/);
    expect(storeSource).toMatch(/explicitLibraryEntryKey/);
  });

  it('exposes compatibility navigation state alongside legacy mode state', () => {
    expect(storeSource).toMatch(/buildScenarioNavigationState/);
    expect(storeSource).toMatch(/resolveCaseLibraryEntryFromScenario/);
    expect(storeSource).toMatch(/get activeFamily\(\) \{ return navigationState\.activeFamily; \}/);
    expect(storeSource).toMatch(/get activeSource\(\) \{ return navigationState\.activeSource; \}/);
    expect(storeSource).toMatch(/get sourceLabel\(\) \{ return navigationState\.sourceLabel; \}/);
    expect(storeSource).toMatch(/get referenceSourceLabel\(\) \{ return navigationState\.referenceSourceLabel; \}/);
    expect(storeSource).toMatch(/get provenanceSummary\(\) \{ return navigationState\.provenanceSummary; \}/);
    expect(storeSource).toMatch(/get navigationState\(\) \{ return navigationState; \}/);
    expect(storeSource).toMatch(/setComparisonSelection\(selection: Partial<ComparisonSelection>\)/);
  });
});