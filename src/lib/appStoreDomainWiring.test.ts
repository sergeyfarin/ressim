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

  it('routes clone and override-reset flows via domain APIs', () => {
    expect(appSource).toMatch(/scenario\.cloneActiveBenchmarkToCustom\(\)/);
    expect(appSource).toMatch(/params\.resetOverrideGroupsToBase\(groupKeys\)/);
  });

  it('avoids transitional App-side contract assembly logic', () => {
    expect(appSource).not.toMatch(/buildBenchmarkCloneProvenance/);
    expect(appSource).not.toMatch(/buildOverrideResetPlan/);
    expect(appSource).not.toMatch(/import\s*\{\s*catalog\s*\}\s*from\s*"\.\/lib\/caseCatalog"/);
  });
});
