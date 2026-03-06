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
    expect(appSource).toMatch(/scenario\.cloneActiveBenchmarkToCustom\(\)/);
  });

  it('passes preset-customize domain state into ModePanel', () => {
    expect(appSource).toMatch(/onParamEdit=\{scenario\.handleParamEdit\}/);
    expect(appSource).toMatch(/basePreset=\{scenario\.basePreset\}/);
  });

  it('avoids transitional App-side contract assembly logic', () => {
    expect(appSource).not.toMatch(/buildBenchmarkCloneProvenance/);
    expect(appSource).not.toMatch(/buildOverrideResetPlan/);
    expect(appSource).not.toMatch(/import\s*\{\s*catalog\s*\}\s*from\s*"\.\/lib\/caseCatalog"/);
  });
});
