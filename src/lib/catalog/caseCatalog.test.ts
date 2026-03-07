import fs from 'node:fs';
import path from 'node:path';
import { describe, it, expect } from 'vitest';
import {
  benchmarkCases,
  buildCaseKey,
  catalog,
  getBenchmarkEntry,
  getDefaultToggles,
  getDisabledOptions,
} from './caseCatalog';

describe('caseCatalog Dynamic Catalog', () => {
  it('has a valid catalog loaded', () => {
    expect(catalog.version).toBeDefined();
    expect(Object.keys(catalog.modes).length).toBeGreaterThan(0);
    expect(catalog.modes.dep.dimensions.length).toBeGreaterThan(0);
  });

  it('generates a case key deterministically', () => {
    const toggles = getDefaultToggles('dep');
    const key = buildCaseKey(toggles);
    expect(key).toContain('mode-dep');
  });

  it('evaluates disability rules correctly for 1D gravity', () => {
    const toggles = getDefaultToggles('dep');
    toggles.geo = '1d';
    toggles.grav = 'off';

    const disabled = getDisabledOptions(toggles);
    expect(disabled['grav']).toBeDefined();
    // Since it's a 1D case, setting gravity to "on" should be disabled
    expect(disabled['grav']['on']).toBeDefined();
  });

  it('keeps refined BL benchmark catalog entries aligned with public case presets', () => {
    const repoRoot = path.resolve(__dirname, '../../..');
    const caseA = JSON.parse(
      fs.readFileSync(path.join(repoRoot, 'public/cases/bl_case_a_refined.json'), 'utf8'),
    );
    const caseB = JSON.parse(
      fs.readFileSync(path.join(repoRoot, 'public/cases/bl_case_b_refined.json'), 'utf8'),
    );

    const catalogCaseA = getBenchmarkEntry('bl_case_a_refined');
    const catalogCaseB = getBenchmarkEntry('bl_case_b_refined');

    expect(catalogCaseA).toBeDefined();
    expect(catalogCaseB).toBeDefined();
    expect(catalogCaseA?.label).toBe(caseA.label);
    expect(catalogCaseB?.label).toBe(caseB.label);
    expect(catalogCaseA?.description).toBe(caseA.description);
    expect(catalogCaseB?.description).toBe(caseB.description);
    expect(catalogCaseA?.params).toEqual(caseA.params);
    expect(catalogCaseB?.params).toEqual(caseB.params);
  });

  it('builds benchmark registry from public case files only', () => {
    expect(benchmarkCases.map((entry) => entry.key)).toEqual([
      'bl_case_a_refined',
      'bl_case_b_refined',
      'dietz_sq_center',
      'dietz_sq_corner',
      'fetkovich_exp',
    ]);
    expect(catalog.benchmarks).toEqual(benchmarkCases);
  });
});
