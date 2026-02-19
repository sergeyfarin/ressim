import { describe, it, expect } from 'vitest'
import { resolveParams, findCaseByKey, categoryKeys, caseCatalog } from './caseCatalog'

describe('caseCatalog helpers', () => {
  it('resolveParams merges defaults and fills missing well positions', () => {
    const sparse = { nx: 5, ny: 3 }
    const merged = resolveParams(sparse)
    expect(merged.nx).toBe(5)
    expect(merged.ny).toBe(3)
    // injector defaults to 0,0
    expect(merged.injectorI).toBe(0)
    expect(merged.injectorJ).toBe(0)
    // producer defaults to nx-1,0
    expect(merged.producerI).toBe(4)
    expect(merged.producerJ).toBe(0)
    // default properties are present
    expect(typeof merged.mu_w).toBe('number')
    expect(typeof merged.injectorBhp).toBe('number')
  })

  it('resolveParams preserves explicit well positions', () => {
    const sparse = { nx: 6, injectorI: 2, injectorJ: 1, producerI: 3, producerJ: 2 }
    const merged = resolveParams(sparse)
    expect(merged.injectorI).toBe(2)
    expect(merged.injectorJ).toBe(1)
    expect(merged.producerI).toBe(3)
    expect(merged.producerJ).toBe(2)
  })

  it('findCaseByKey returns the correct case and category', () => {
    const entry = findCaseByKey('bl_case_a_refined')
    expect(entry).not.toBeNull()
    expect(entry!.categoryKey).toBe('waterflood')
    expect(entry!.case.key).toBe('bl_case_a_refined')
  })

  it('findCaseByKey returns null for unknown keys', () => {
    expect(findCaseByKey('nonexistent_case')).toBeNull()
  })

  it('categoryKeys includes all top-level categories from caseCatalog', () => {
    const keys = categoryKeys
    for (const expected of Object.keys(caseCatalog)) {
      expect(keys).toContain(expected)
    }
  })
})
