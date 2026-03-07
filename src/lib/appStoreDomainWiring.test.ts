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
    expect(appSource).toMatch(/onParamEdit=\{scenario\.handleParamEdit\}/);
    expect(appSource).toMatch(/basePreset=\{scenario\.basePreset\}/);
    expect(appSource).toMatch(/navigationState=\{scenario\.navigationState\}/);
    expect(appSource).toMatch(/onActivateLibraryEntry=\{scenario\.activateLibraryEntry\}/);
    expect(appSource).toMatch(/warningPolicy=\{runtime\.warningPolicy\}/);
  });

  it('uses resolved library metadata for non-benchmark layout config', () => {
    expect(appSource).toMatch(/scenario\.activeLibraryEntry\?\.layoutConfig \?\? \{\}/);
  });

  it('filters reference outputs by active reference family rather than the benchmark tab', () => {
    expect(appSource).toMatch(/scenario\.activeReferenceFamily\?\.key/);
    expect(appSource).toMatch(/activeReferenceFamily && activeReferenceResults.length > 0 && BenchmarkChartComponent/);
  });

  it('routes benchmark execution selection through the runtime domain API', () => {
    expect(appSource).toMatch(/onRunBenchmarkSelection=\{runtime\.runActiveReferenceSelection\}/);
  });

  it('routes the centralized warning policy into runtime warning surfaces', () => {
    expect(appSource).toMatch(/warningPolicy=\{runtime\.warningPolicy\}/);
    expect(appSource).toMatch(/<WarningPolicyPanel/);
    expect(appSource).toMatch(/groups=\{\["referenceCaveat"\]\}/);
    expect(appSource).toMatch(/referenceCaveat: \["analytical"\]/);
  });

  it('avoids transitional App-side contract assembly logic', () => {
    expect(appSource).not.toMatch(/buildBenchmarkCloneProvenance/);
    expect(appSource).not.toMatch(/buildOverrideResetPlan/);
    expect(appSource).not.toMatch(/import\s*\{\s*catalog\s*\}\s*from\s*"\.\/lib\/catalog\/caseCatalog"/);
  });
});
