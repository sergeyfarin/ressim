import { describe, expect, it } from 'vitest';
import fs from 'fs';
import path from 'path';

const appPath = path.join(__dirname, '..', 'App.svelte');
const appSource = fs.readFileSync(appPath, 'utf8');

describe('App store domain wiring', () => {
  it('consumes domain objects from the simulation store', () => {
    expect(appSource).toMatch(/const\s+scenario\s*=\s*store\.scenarioSelection\s*;/);
    expect(appSource).toMatch(/const\s+params\s*=\s*store\.parameterState\s*;/);
    expect(appSource).toMatch(/const\s+runtime\s*=\s*store\.runtimeState\s*;/);
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
    expect(appSource).toMatch(/import ReferenceResultsCard from "\.\/lib\/ui\/cards\/ReferenceResultsCard\.svelte"/);
    expect(appSource).toMatch(/<ReferenceResultsCard/);
    expect(appSource).toMatch(/family=\{activeReferenceFamily\}/);
    expect(appSource).toMatch(/results=\{activeReferenceResults\}/);
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
    expect(appSource).toMatch(/const outputProfileGridState = \$derived\.by/);
    expect(appSource).toMatch(/const outputProfileNx = \$derived\.by/);
    expect(appSource).toMatch(/const outputProfileScenarioMode = \$derived\.by/);
    expect(appSource).toMatch(/gridState=\{outputProfileGridState\}/);
    expect(appSource).toMatch(/nx=\{outputProfileNx\}/);
    expect(appSource).toMatch(/simTime=\{outputProfileSimTime\}/);
    expect(appSource).toMatch(/sourceLabel=\{outputProfileSourceLabel\}/);
    expect(appSource).toMatch(/rockProps=\{outputProfileRockProps\}/);
    expect(appSource).toMatch(/fluidProps=\{outputProfileFluidProps\}/);
  });

  it('extends outputs-owned comparison focus into the 3D surface', () => {
    expect(appSource).toMatch(/const output3DHistory = \$derived\.by/);
    expect(appSource).toMatch(/const output3DNx = \$derived\.by/);
    expect(appSource).toMatch(/const output3DCurrentIndex = \$derived\.by/);
    expect(appSource).toMatch(/const output3DReplayTime = \$derived\.by/);
    expect(appSource).toMatch(/const output3DSourceLabel = \$derived\.by/);
    expect(appSource).toMatch(/function handleApplyOutputHistoryIndex\(index: number\)/);
    expect(appSource).toMatch(/sourceLabel=\{output3DSourceLabel\}/);
    expect(appSource).toMatch(/gridState=\{output3DGridState\}/);
    expect(appSource).toMatch(/nx=\{output3DNx\}/);
    expect(appSource).toMatch(/cellDx=\{output3DCellDx\}/);
    expect(appSource).toMatch(/currentIndex=\{output3DCurrentIndex\}/);
    expect(appSource).toMatch(/replayTime=\{output3DReplayTime\}/);
    expect(appSource).toMatch(/onApplyHistoryIndex=\{handleApplyOutputHistoryIndex\}/);
    expect(appSource).toMatch(/history=\{output3DHistory\}/);
    expect(appSource).toMatch(/wellState=\{output3DWellState\}/);
  });

  it('routes the centralized warning policy into runtime warning surfaces', () => {
    expect(appSource).toMatch(/warningPolicy=\{runtime\.warningPolicy\}/);
    expect(appSource).toMatch(/<ScenarioPicker/);
    expect(appSource).toMatch(/<RunControls/);
    expect(appSource).not.toMatch(/<WarningPolicyPanel/);
  });

  it('avoids transitional App-side contract assembly logic', () => {
    expect(appSource).not.toMatch(/buildReferenceCloneProvenance/);
    expect(appSource).not.toMatch(/buildOverrideResetPlan/);
    expect(appSource).not.toMatch(/import\s*\{\s*catalog\s*\}\s*from\s*"\.\/lib\/catalog\/caseCatalog"/);
  });
});
