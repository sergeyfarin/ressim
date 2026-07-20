import { describe, it, expect } from 'vitest';
import {
  buildCaseKey,
  catalog,
  resolveCaseLibraryEntryFromScenario,
  getDefaultToggles,
  getDisabledOptions,
  composeCaseParams,
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

  it('resolves exact non-benchmark scenario params back to real library preset entries', () => {
    const cornerProducer = getPresetEntry('depletion_corner_producer');
    const baseline = getPresetEntry('baseline_waterflood');

    expect(resolveCaseLibraryEntryFromScenario({
      activeMode: 'dep',
      scenarioParams: { ...catalog.defaults, ...cornerProducer!.params },
    })?.key).toBe('depletion_corner_producer');

    expect(resolveCaseLibraryEntryFromScenario({
      activeMode: 'sim',
      scenarioParams: { ...catalog.defaults, ...baseline!.params },
    })?.key).toBe('baseline_waterflood');
  });

  it('does not fabricate a library entry for unmatched non-benchmark facet scenarios', () => {
    const toggles = getDefaultToggles('dep');

    expect(resolveCaseLibraryEntryFromScenario({
      activeMode: 'dep',
      scenarioParams: composeCaseParams(toggles),
    })).toBeNull();
  });
});
