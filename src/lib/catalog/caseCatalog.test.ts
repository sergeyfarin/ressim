import fs from 'node:fs';
import path from 'node:path';
import { describe, it, expect } from 'vitest';
import {
  benchmarkCases,
  benchmarkFamilies,
  buildCaseKey,
  catalog,
  getBenchmarkEntry,
  getBenchmarkFamily,
  getDefaultToggles,
  getDisabledOptions,
  getPresetEntry,
  presetCases,
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

  it('keeps refined BL benchmark catalog entries aligned with source benchmark case files', () => {
    const repoRoot = path.resolve(__dirname, '../../..');
    const caseA = JSON.parse(
      fs.readFileSync(path.join(repoRoot, 'src/lib/catalog/benchmark-case-data/bl_case_a_refined.json'), 'utf8'),
    );
    const caseB = JSON.parse(
      fs.readFileSync(path.join(repoRoot, 'src/lib/catalog/benchmark-case-data/bl_case_b_refined.json'), 'utf8'),
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

  it('builds benchmark registry from source benchmark case files only', () => {
    expect(benchmarkCases.map((entry) => entry.key)).toEqual([
      'bl_case_a_refined',
      'bl_case_b_refined',
      'dietz_sq_center',
      'dietz_sq_corner',
      'fetkovich_exp',
    ]);
    expect(catalog.benchmarks).toEqual(benchmarkCases);
  });

  it('builds named preset registry from source definitions only', () => {
    expect(presetCases.map((entry) => entry.key)).toEqual([
      'depletion_corner_producer',
      'depletion_center_producer',
      'depletion_1d_clean',
      'depletion_2d_radial_clean',
      'bl_aligned_homogeneous',
      'bl_aligned_mild_capillary',
      'bl_aligned_mobility_balanced',
      'waterflood_bl_clean',
      'waterflood_unfavorable_mobility',
      'baseline_waterflood',
      'high_contrast_layers',
      'viscous_fingering_risk',
    ]);
    expect(catalog.presets).toEqual(presetCases);
  });

  it('extracts preset layout overrides into typed registry metadata', () => {
    const cornerProducer = getPresetEntry('depletion_corner_producer');
    const baseline = getPresetEntry('baseline_waterflood');

    expect(cornerProducer).toMatchObject({
      mode: 'dep',
      category: 'depletion',
      layoutConfig: {
        rateChart: {
          logScale: true,
        },
      },
    });
    expect(cornerProducer?.params).not.toHaveProperty('layout');
    expect(baseline).toMatchObject({
      mode: 'sim',
      category: 'exploration',
    });
    expect(baseline?.layoutConfig ?? null).toBeNull();
  });

  it('defines benchmark families with explicit ownership metadata', () => {
    const caseA = getBenchmarkFamily('bl_case_a_refined');
    const dietz = getBenchmarkFamily('dietz_sq_center');

    expect(benchmarkFamilies.map((family) => family.key)).toEqual([
      'bl_case_a_refined',
      'bl_case_b_refined',
      'dietz_sq_center',
      'dietz_sq_corner',
      'fetkovich_exp',
    ]);

    expect(caseA).toMatchObject({
      scenarioClass: 'buckley-leverett',
      reference: {
        kind: 'analytical',
        source: 'buckley-leverett-shock-reference',
      },
      displayDefaults: {
        xAxis: 'pvi',
        panels: ['watercut-breakthrough', 'recovery', 'pressure'],
      },
      stylePolicy: {
        colorBy: 'case',
        lineStyleBy: 'quantity-or-reference',
        separatePressurePanel: true,
      },
      runPolicy: 'compare-to-reference',
      sensitivityAxes: ['grid-refinement', 'timestep-refinement', 'heterogeneity'],
    });
    expect(caseA?.baseCase.key).toBe('bl_case_a_refined');
    expect(caseA?.label).toBe(caseA?.baseCase.label);
    expect(caseA?.description).toBe(caseA?.baseCase.description);

    expect(dietz).toMatchObject({
      scenarioClass: 'depletion',
      reference: {
        kind: 'analytical',
        source: 'dietz-shape-factor-reference',
      },
      displayDefaults: {
        xAxis: 'time',
        panels: ['oil-rate', 'cumulative-oil', 'decline-diagnostics'],
      },
      runPolicy: 'compare-to-reference',
      sensitivityAxes: [],
    });
    expect(dietz?.baseCase.key).toBe('dietz_sq_center');
  });

  it('derives benchmark runtime entries from family base cases without duplicating params', () => {
    for (const family of benchmarkFamilies) {
      const entry = getBenchmarkEntry(family.key);
      expect(entry).toBeDefined();
      expect(entry?.params).toBe(family.baseCase.params);
      expect(entry?.label).toBe(family.label);
      expect(entry?.description).toBe(family.description);
    }
  });
});
