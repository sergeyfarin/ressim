import { describe, it, expect } from 'vitest';
import { resolveParams, findCaseByKey, caseCatalog, FACET_OPTIONS } from './caseCatalog';

describe('caseCatalog properties and resolvers', () => {
  it('resolveParams merges depletion defaults properly', () => {
    const sparse = { nx: 10 };
    const merged = resolveParams(sparse, 'depletion');
    expect(merged.nx).toBe(10);
    // Depletion base is 48
    expect(merged.ny).toBe(1);
    expect(merged.analyticalSolutionMode).toBe('depletion');
    // Injector is disabled in depletion base
    expect(merged.injectorEnabled).toBe(false);
  });

  it('resolveParams merges waterflood defaults properly', () => {
    const sparse = { nx: 96, targetInjectorRate: 500 };
    const merged = resolveParams(sparse, 'waterflood');
    expect(merged.nx).toBe(96);
    expect(merged.targetInjectorRate).toBe(500);
    expect(merged.analyticalSolutionMode).toBe('waterflood');
    expect(merged.injectorEnabled).toBe(true);
  });

  it('findCaseByKey returns the correct case entry', () => {
    const entry = findCaseByKey('wf_pub_a');
    expect(entry).not.toBeNull();
    expect(entry!.key).toBe('wf_pub_a');
    expect(entry!.facets.mode).toBe('waterflood');
  });

  it('findCaseByKey returns null for unknown keys', () => {
    expect(findCaseByKey('nonexistent_case')).toBeNull();
  });

  it('caseCatalog contains exactly 92 cases as planned', () => {
    expect(caseCatalog.length).toBe(92);
  });

  it('all cases have valid properties and facets', () => {
    for (const entry of caseCatalog) {
      expect(entry.key).toBeDefined();
      expect(entry.label).toBeDefined();
      expect(entry.facets.mode).toBeDefined();
      expect(entry.params).toBeDefined();

      // Verify facets are part of valid options
      expect(FACET_OPTIONS.mode.includes(entry.facets.mode)).toBe(true);
      expect(FACET_OPTIONS.geometry.includes(entry.facets.geometry)).toBe(true);
      expect(FACET_OPTIONS.permeability.includes(entry.facets.permeability)).toBe(true);
    }
  });
});
