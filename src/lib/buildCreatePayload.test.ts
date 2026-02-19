import { describe, it, expect } from 'vitest'
import { buildCreatePayloadFromState } from './buildCreatePayload'
import type { SimulatorCreatePayload } from './simulator-types'

describe('buildCreatePayloadFromState', () => {
  it('produces perms arrays of length nz for uniform permMode', () => {
    const payload = buildCreatePayloadFromState({
      nx: 2, ny: 2, nz: 3,
      permMode: 'uniform',
      uniformPermX: 123,
      uniformPermY: 456,
      uniformPermZ: 7,
    }) as SimulatorCreatePayload

    expect(payload.permsX).toHaveLength(3)
    expect(payload.permsY).toHaveLength(3)
    expect(payload.permsZ).toHaveLength(3)
    expect(payload.permsX.every((v) => v === 123)).toBe(true)
    expect(payload.permsY.every((v) => v === 456)).toBe(true)
    expect(payload.permsZ.every((v) => v === 7)).toBe(true)
  })

  it('accepts perLayer arrays and preserves values', () => {
    const payload = buildCreatePayloadFromState({
      nx: 2, ny: 2, nz: 2,
      permMode: 'perLayer',
      layerPermsX: [1, 2],
      layerPermsY: [3, 4],
      layerPermsZ: [5, 6],
    }) as SimulatorCreatePayload

    expect(payload.permsX).toEqual([1, 2])
    expect(payload.permsY).toEqual([3, 4])
    expect(payload.permsZ).toEqual([5, 6])
  })

  it('coerces string/undefined inputs into numeric defaults', () => {
    const payload = buildCreatePayloadFromState({
      nx: '4' as unknown as number,
      ny: '3' as unknown as number,
      nz: '2' as unknown as number,
      permMode: 'uniform',
      uniformPermX: '10' as unknown as number,
    }) as SimulatorCreatePayload

    expect(payload.nx).toBe(4)
    expect(payload.ny).toBe(3)
    expect(payload.nz).toBe(2)
    expect(payload.permsX.every((v) => v === 10)).toBe(true)
  })

  it('returns required numeric and boolean fields', () => {
    const payload = buildCreatePayloadFromState({ nx: 1, ny: 1, nz: 1 }) as SimulatorCreatePayload
    expect(typeof payload.initialPressure).toBe('number')
    expect(typeof payload.initialSaturation).toBe('number')
    expect(typeof payload.capillaryEnabled).toBe('boolean')
    expect(Array.isArray(payload.permsX)).toBe(true)
  })
})
