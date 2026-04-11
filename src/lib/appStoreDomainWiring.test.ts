import { describe, expect, it } from 'vitest';
import fs from 'fs';
import path from 'path';

const appPath = path.join(__dirname, '..', 'App.svelte');
const appSource = fs.readFileSync(appPath, 'utf8');

const navStorePath = path.join(__dirname, 'stores', 'navigationStore.svelte.ts');
const navStoreSource = fs.readFileSync(navStorePath, 'utf8');

describe('App store domain wiring', () => {
  it('consumes domain objects from the simulation store', () => {
    expect(appSource).toMatch(/const \{ params, runtime, nav: scenario \} = createSimulationStore\(\)/);
  });

  it('routes clone flow via domain API', () => {
    expect(appSource).toMatch(/scenario\.cloneActiveReferenceToCustom\(\)/);
  });

  it('passes preset-customize domain state into ModePanel', () => {
    expect(appSource).toMatch(/onParamEdit=\{\(\) =\> scenario\.handleParamEdit\(\)\}/);
    expect(appSource).toMatch(/basePreset=\{scenario\.basePreset\}/);
    expect(appSource).toMatch(/navigationState=\{scenario\.navigationState\}/);
    expect(appSource).toMatch(/onActivateLibraryEntry=\{\(key\) =\> scenario\.activateLibraryEntry\(key\)\}/);
    expect(appSource).toMatch(/referenceProvenance=\{scenario\.referenceProvenance\}/);
    expect(appSource).toMatch(/referenceSweepRunning=\{runtime\.referenceSweepRunning\}/);
    expect(appSource).toMatch(/warningPolicy=\{runtime\.warningPolicy\}/);
    expect(appSource).not.toMatch(/<ModePanel[^>]*referenceSweepProgressLabel=/);
    expect(appSource).not.toMatch(/<ModePanel[^>]*onRunReferenceSelection=/);
    expect(appSource).not.toMatch(/<ModePanel[^>]*referenceRunResults=/);
  });

  it('builds a run-region manifest from active case-library and custom-state metadata', () => {
    // App.svelte orchestrates the UI; the run-manifest derived state lives in the nav store.
    expect(appSource).toMatch(/import ReferenceExecutionCard from "\.\/lib\/ui\/cards\/ReferenceExecutionCard\.svelte"/);
    expect(appSource).toMatch(/<ReferenceExecutionCard/);
    expect(appSource).toMatch(/referenceFamilyKey=\{scenario\.activeReferenceFamily\?\.key \?\? null\}/);
    expect(appSource).toMatch(/onRunReferenceSelection=\{\(keys\) =\> runtime\.runActiveReferenceSelection\(keys\)\}/);
    expect(appSource).toMatch(/Reference Run Status/);
  });

  it('uses resolved library metadata for non-benchmark layout config', () => {
    // Phase 7: chart layout config computation moved to nav store; App.svelte delegates.
    expect(navStoreSource).toMatch(/activeLibraryEntry\?\.layoutConfig \?\? \{\}/);
    expect(appSource).toMatch(/scenario\.activeRateChartLayoutConfig/);
  });

  it('filters reference outputs by active reference family rather than the benchmark tab', () => {
    expect(appSource).not.toMatch(/ReferenceResultsCard/);
    expect(appSource).not.toMatch(/Reference Run Results/);
    expect(appSource).toMatch(/scenario\.setComparisonSelection\(/);
    expect(appSource).not.toMatch(/primaryResultKey=\{activePrimaryComparisonResultKey\}/);
    expect(appSource).not.toMatch(/comparedResultKeys=\{activeComparedResultKeys\}/);
    expect(appSource).toMatch(/scenario\.activeReferenceFamily\?\.key/);
    // Phase 7: getReferenceRateChartLayoutConfig lives in nav store; App.svelte uses the derived result.
    expect(navStoreSource).toMatch(/getReferenceRateChartLayoutConfig/);
    expect(appSource).toMatch(/ReferenceComparisonChartComponent/);
    expect(appSource).toMatch(/scenario\.activeChartFamily && ReferenceComparisonChartComponent/);
    expect(appSource).toMatch(/Results/);
  });

  it('extends outputs-owned comparison focus into the saturation-profile surface', () => {
    // Phase 7: derived state moved to nav store; App.svelte delegates via scenario.xxx.
    expect(navStoreSource).toMatch(/activeSelectedReferenceResult = \$derived\.by/);
    expect(navStoreSource).toMatch(/export type OutputSelectionProfile = \{/);
    expect(navStoreSource).toMatch(/selectedOutputProfile = \$derived\.by/);
    expect(navStoreSource).toMatch(/computeSweepRecoveryFactor\(/);
    expect(appSource).toMatch(/rockProps=\{scenario\.selectedOutputProfile\.rockProps\}/);
    expect(appSource).toMatch(/fluidProps=\{scenario\.selectedOutputProfile\.fluidProps\}/);
  });

  it('extends outputs-owned comparison focus into the 3D surface', () => {
    // Phase 7: type and derived state moved to nav store; 3D props wired via ThreeDViewCard.
    expect(navStoreSource).toMatch(/export type Output3DSelection = \{/);
    expect(navStoreSource).toMatch(/selectedOutput3D = \$derived\.by/);
    expect(appSource).toMatch(/function handleApplyOutputHistoryIndex\(index: number\)/);
    expect(appSource).toMatch(/selectedOutput3D=\{scenario\.selectedOutput3D\}/);
    expect(appSource).toMatch(/onApplyHistoryIndex=\{handleApplyOutputHistoryIndex\}/);
  });

  it('routes the centralized warning policy into runtime warning surfaces', () => {
    expect(appSource).toMatch(/warningPolicy=\{runtime\.warningPolicy\}/);
    expect(appSource).toMatch(/<ScenarioPicker/);
    expect(appSource).toMatch(/<RunControls/);
    expect(appSource).toMatch(/showStepsInput=\{scenario\.isCustomMode\}/);
    expect(appSource).toMatch(/onStepsEdit=\{\(\) =\> params\.markStepsOverride\(\)\}/);
    expect(appSource).not.toMatch(/<WarningPolicyPanel/);
  });

  it('avoids transitional App-side contract assembly logic', () => {
    expect(appSource).not.toMatch(/buildReferenceCloneProvenance/);
    expect(appSource).not.toMatch(/buildOverrideResetPlan/);
    expect(appSource).not.toMatch(/import\s*\{\s*catalog\s*\}\s*from\s*"\.\/lib\/catalog\/caseCatalog"/);
  });
});
