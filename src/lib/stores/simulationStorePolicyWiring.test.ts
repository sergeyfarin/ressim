import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const storePath = path.join(__dirname, 'simulationStore.svelte.ts');
const storeSource = fs.readFileSync(storePath, 'utf8');

describe('simulation store policy wiring', () => {
  it('uses the shared auto-clear policy for modified-state reset behavior', () => {
    expect(storeSource).toMatch(/shouldAutoClearModifiedState/);
    expect(storeSource).toMatch(/benchmarkProvenance/);
    expect(storeSource).toMatch(/parameterOverrideCount/);
  });

  it('uses the shared clone policy for benchmark clone-to-custom flow', () => {
    expect(storeSource).toMatch(/shouldAllowBenchmarkClone/);
    expect(storeSource).toMatch(/function cloneActiveBenchmarkToCustom\(\): boolean/);
  });

  it('wires benchmark sweep actions and normalized benchmark results through the store', () => {
    expect(storeSource).toMatch(/buildBenchmarkRunSpecs/);
    expect(storeSource).toMatch(/runActiveBenchmarkSelection/);
    expect(storeSource).toMatch(/benchmarkRunResults/);
  });
});