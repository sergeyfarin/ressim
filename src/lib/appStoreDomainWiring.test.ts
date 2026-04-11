import { describe, expect, it } from 'vitest';
import fs from 'fs';
import path from 'path';

const appPath = path.join(__dirname, '..', 'App.svelte');
const appSource = fs.readFileSync(appPath, 'utf8');

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
    expect(appSource).toMatch(/import ReferenceExecutionCard from "\.\/lib\/ui\/cards\/ReferenceExecutionCard\.svelte"/);
    expect(appSource).toMatch(/<ReferenceExecutionCard/);
    expect(appSource).toMatch(/referenceFamilyKey=\{scenario\.activeReferenceFamily\?\.key \?\? null\}/);
    expect(appSource).toMatch(/onRunReferenceSelection=\{\(keys\) =\> runtime\.runActiveReferenceSelection\(keys\)\}/);
    expect(appSource).toMatch(/const activeRunManifest = \$derived\.by/);
    expect(appSource).toMatch(/scenario\.activeLibraryEntry\?\.referencePolicySummary/);
    expect(appSource).toMatch(/scenario\.referenceProvenance/);
    expect(appSource).toMatch(/Reference Run Status/);
  });

  it('uses resolved library metadata for non-benchmark layout config', () => {
    expect(appSource).toMatch(/scenario\.activeLibraryEntry\?\.layoutConfig \?\? \{\}/);
  });

  it('filters reference outputs by active reference family rather than the benchmark tab', () => {
    expect(appSource).not.toMatch(/ReferenceResultsCard/);
    expect(appSource).not.toMatch(/Reference Run Results/);
    expect(appSource).toMatch(/scenario\.setComparisonSelection\(/);
    expect(appSource).not.toMatch(/primaryResultKey=\{activePrimaryComparisonResultKey\}/);
    expect(appSource).not.toMatch(/comparedResultKeys=\{activeComparedResultKeys\}/);
    expect(appSource).toMatch(/scenario\.activeReferenceFamily\?\.key/);
    expect(appSource).toMatch(/getReferenceRateChartLayoutConfig/);
    expect(appSource).toMatch(/ReferenceComparisonChartComponent/);
    expect(appSource).toMatch(/activeReferenceFamily && ReferenceComparisonChartComponent/);
    expect(appSource).toMatch(/Results/);
  });

  it('extends outputs-owned comparison focus into the saturation-profile surface', () => {
    expect(appSource).toMatch(/const activeSelectedReferenceResult = \$derived\.by/);
    expect(appSource).toMatch(/type OutputSelectionProfile = \{/);
    expect(appSource).toMatch(/const selectedOutputProfile = \$derived\.by/);
    expect(appSource).toMatch(/computeSweepRecoveryFactor\(selectedOutputProfile\.rockProps, selectedOutputProfile\.fluidProps/);
    expect(appSource).toMatch(/rockProps=\{selectedOutputProfile\.rockProps\}/);
    expect(appSource).toMatch(/fluidProps=\{selectedOutputProfile\.fluidProps\}/);
  });

  it('extends outputs-owned comparison focus into the 3D surface', () => {
    expect(appSource).toMatch(/type Output3DSelection = \{/);
    expect(appSource).toMatch(/const selectedOutput3D = \$derived\.by/);
    expect(appSource).toMatch(/function handleApplyOutputHistoryIndex\(index: number\)/);
    expect(appSource).toMatch(/sourceLabel=\{selectedOutput3D\.sourceLabel\}/);
    expect(appSource).toMatch(/gridState=\{selectedOutput3D\.gridState\}/);
    expect(appSource).toMatch(/nx=\{selectedOutput3D\.nx\}/);
    expect(appSource).toMatch(/cellDx=\{selectedOutput3D\.cellDx\}/);
    expect(appSource).toMatch(/currentIndex=\{selectedOutput3D\.currentIndex\}/);
    expect(appSource).toMatch(/replayTime=\{selectedOutput3D\.replayTime\}/);
    expect(appSource).toMatch(/onApplyHistoryIndex=\{handleApplyOutputHistoryIndex\}/);
    expect(appSource).toMatch(/history=\{selectedOutput3D\.history\}/);
    expect(appSource).toMatch(/wellState=\{selectedOutput3D\.wellState\}/);
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
