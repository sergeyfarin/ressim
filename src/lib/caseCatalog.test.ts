import { describe, it, expect } from 'vitest';
import { catalog, buildCaseKey, getDefaultToggles, getDisabledOptions } from './caseCatalog';

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
});
