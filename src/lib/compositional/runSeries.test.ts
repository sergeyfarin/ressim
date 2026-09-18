import { describe, expect, it } from 'vitest';

import { compositionalRunQuantities, findCompositionalRunQuantity } from './runQuantities';
import { buildCompositionalRunSeries } from './runSeries';
import type { CompositionalSnapshot } from './types';

function snapshot(options: {
  time: number;
  pressure?: number[];
  inventory?: number[];
  wellMoles?: number[][];
  phases?: string[];
  vapour?: number[];
  ids?: string[];
}): CompositionalSnapshot {
  const ids = options.ids ?? ['CO2', 'C1', 'C10'];
  return {
    time_days: options.time,
    component_ids: ids,
    pressure: options.pressure ?? [100, 100],
    composition: ids.map(() => [0.3, 0.3]),
    phase_state: options.phases ?? ['two-phase', 'two-phase'],
    vapour_saturation: options.vapour ?? [0.2, 0.4],
    inventory: options.inventory ?? [100, 300, 600],
    cumulative_well_moles: options.wellMoles ?? [],
  };
}

describe('buildCompositionalRunSeries', () => {
  it('is empty for no snapshots', () => {
    const series = buildCompositionalRunSeries([]);
    expect(series.timeDays).toEqual([]);
    expect(series.componentIds).toEqual([]);
  });

  it('derives time, averages and the two-phase fraction', () => {
    const series = buildCompositionalRunSeries([
      snapshot({ time: 0, pressure: [100, 200], vapour: [0.2, 0.4] }),
      snapshot({
        time: 1,
        pressure: [90, 110],
        phases: ['two-phase', 'single-liquid'],
        vapour: [0.5, 0],
      }),
    ]);
    expect(series.timeDays).toEqual([0, 1]);
    expect(series.averagePressure).toEqual([150, 100]);
    expect(series.averageVapourSaturation[0]).toBeCloseTo(0.3, 12);
    expect(series.averageVapourSaturation[1]).toBeCloseTo(0.25, 12);
    expect(series.twoPhaseCellFraction).toEqual([1, 0.5]);
  });

  it('gives one inventory series per component, in the engine order', () => {
    const series = buildCompositionalRunSeries([
      snapshot({ time: 0, inventory: [100, 300, 600] }),
      snapshot({ time: 1, inventory: [150, 290, 580] }),
    ]);
    expect(series.componentIds).toEqual(['CO2', 'C1', 'C10']);
    expect(series.componentInventory).toHaveLength(3);
    expect(series.componentInventory[0]).toEqual([100, 150]);
    expect(series.componentInventory[2]).toEqual([600, 580]);
  });

  it('sums the well totals across wells', () => {
    const series = buildCompositionalRunSeries([
      snapshot({ time: 0, wellMoles: [[0, 0, 0]] }),
      snapshot({
        time: 1,
        wellMoles: [
          [50, 0, 0],
          [-5, -10, -20],
        ],
      }),
    ]);
    expect(series.componentNetWellMoles[0]).toEqual([0, 45]);
    expect(series.componentNetWellMoles[1]).toEqual([0, -10]);
  });

  it('reports a conservation residual near zero for a run that conserves', () => {
    // Started with [100, 300, 600]; a well injected 50 of component 0 and produced 10 and 20 of
    // the others, so the grid should hold [150, 290, 580] — and does.
    const series = buildCompositionalRunSeries([
      snapshot({ time: 0, inventory: [100, 300, 600], wellMoles: [[0, 0, 0]] }),
      snapshot({ time: 1, inventory: [150, 290, 580], wellMoles: [[50, -10, -20]] }),
    ]);
    expect(series.conservationResidual[1]).toBeLessThan(1e-12);
  });

  it('reports a conservation residual that grows when a run stops conserving', () => {
    const series = buildCompositionalRunSeries([
      snapshot({ time: 0, inventory: [100, 300, 600], wellMoles: [[0, 0, 0]] }),
      // The grid holds 10% more component 0 than the well moved.
      snapshot({ time: 1, inventory: [165, 290, 580], wellMoles: [[50, -10, -20]] }),
    ]);
    const residual = series.conservationResidual[1];
    expect(residual).not.toBeNull();
    expect(residual as number).toBeGreaterThan(0.05);
  });

  it('drops snapshots whose component set differs rather than interleaving two runs', () => {
    const series = buildCompositionalRunSeries([
      snapshot({ time: 0 }),
      snapshot({ time: 1, ids: ['C1', 'C10'], inventory: [10, 20] }),
      snapshot({ time: 2 }),
    ]);
    expect(series.timeDays).toEqual([0, 2]);
    expect(series.componentIds).toEqual(['CO2', 'C1', 'C10']);
  });

  it('nulls an average rather than propagating a NaN from an unresolved cell', () => {
    const series = buildCompositionalRunSeries([
      snapshot({ time: 0, vapour: [Number.NaN, Number.NaN], phases: ['unresolved', 'unresolved'] }),
    ]);
    expect(series.averageVapourSaturation[0]).toBeNull();
    expect(series.twoPhaseCellFraction[0]).toBe(0);
  });
});

describe('compositionalRunQuantities', () => {
  const series = buildCompositionalRunSeries([
    snapshot({ time: 0, inventory: [100, 300, 600], wellMoles: [[0, 0, 0]] }),
    snapshot({ time: 1, inventory: [150, 290, 580], wellMoles: [[50, -10, -20]] }),
  ]);

  it('builds one pair of per-component quantities for each component the run has', () => {
    const ids = compositionalRunQuantities(series).map((quantity) => quantity.id);
    expect(ids).toContain('comp-inventory-CO2');
    expect(ids).toContain('comp-net-well-moles-CO2');
    expect(ids).toContain('comp-inventory-C10');
    // And nothing for a component this case does not contain.
    expect(ids).not.toContain('comp-inventory-H2O');
  });

  it('builds nothing per-component for a run with no snapshots', () => {
    const empty = buildCompositionalRunSeries([]);
    const ids = compositionalRunQuantities(empty).map((quantity) => quantity.id);
    expect(ids.some((id) => id.startsWith('comp-inventory-'))).toBe(false);
    // The run-wide quantities still exist, so a panel can render an empty chart rather than crash.
    expect(ids).toContain('comp-average-pressure');
  });

  it('sources the values it names', () => {
    const quantity = findCompositionalRunQuantity(series, 'comp-inventory-CO2');
    expect(quantity?.source(series)).toEqual([100, 150]);
    expect(quantity?.unit).toBe('mol');
  });

  it('gives every quantity a distinct id', () => {
    const ids = compositionalRunQuantities(series).map((quantity) => quantity.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('keeps the compositional registry disjoint from the black-oil one', async () => {
    // The plan forbids compositional output reaching black-oil analytical curves. Separate
    // registries make that structural; this pins that they have not been merged. It has already
    // earned its place: `average-pressure` exists in both and means a different thing in each,
    // which is why the compositional ids are prefixed.
    const { RUN_QUANTITIES } = await import('../charts/runQuantities');
    const blackOil = new Set(Object.keys(RUN_QUANTITIES));
    for (const quantity of compositionalRunQuantities(series)) {
      expect(blackOil.has(quantity.id)).toBe(false);
    }
  });
});
