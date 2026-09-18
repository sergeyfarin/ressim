/**
 * The named quantities a compositional run can plot.
 *
 * Same idea as `charts/runQuantities.ts` — a quantity is data, not a component — with one
 * structural difference that the black-oil registry does not have: **the set is built, not
 * declared.** A compositional case has a variable number of components, so "CO2 in place" is a
 * quantity that exists only for a case that has CO2. A static `const` object cannot express that.
 *
 * It is also a **separate** registry rather than entries in the black-oil one. See
 * `runSeries.ts` for why: keeping the series types apart is what stops a compositional series
 * reaching a black-oil analytical overlay, which the execution plan forbids.
 */

import type { CompositionalRunSeries } from './runSeries';

export interface CompositionalRunQuantity {
  /** Stable identity. Per-component ids embed the component, e.g. `inventory-CO2`. */
  id: string;
  label: string;
  /** Physical unit, for axis and tooltip copy. Empty for dimensionless. */
  unit: string;
  /** Classification for the single-property-per-panel rule. */
  property: string;
  source: (series: CompositionalRunSeries) => Array<number | null>;
}

/**
 * Every compositional quantity id starts with this.
 *
 * Not decoration: `average-pressure` exists in both registries and means a different thing in each
 * — a black-oil run's mean cell pressure and a compositional one's. A test asserts the two id sets
 * are disjoint, and it caught exactly that collision. The prefix makes the separation visible
 * wherever an id is read, not only where the registries are.
 */
export const COMPOSITIONAL_QUANTITY_PREFIX = 'comp-';

/**
 * Build the quantities available for a run.
 *
 * Takes the series rather than a component list so that the per-component quantities cannot be
 * built for components the run does not have.
 */
export function compositionalRunQuantities(
  series: CompositionalRunSeries,
): CompositionalRunQuantity[] {
  const quantities: CompositionalRunQuantity[] = [
    {
      id: `${COMPOSITIONAL_QUANTITY_PREFIX}average-pressure`,
      label: 'Average Pressure',
      unit: 'bar',
      property: 'pressure',
      source: (s) => s.averagePressure,
    },
    {
      id: `${COMPOSITIONAL_QUANTITY_PREFIX}average-vapour-saturation`,
      label: 'Average Vapour Saturation',
      unit: '',
      property: 'saturation',
      source: (s) => s.averageVapourSaturation,
    },
    {
      id: `${COMPOSITIONAL_QUANTITY_PREFIX}two-phase-cell-fraction`,
      label: 'Two-Phase Cells',
      unit: '',
      property: 'phase-fraction',
      source: (s) => s.twoPhaseCellFraction,
    },
    {
      id: `${COMPOSITIONAL_QUANTITY_PREFIX}conservation-residual`,
      label: 'Conservation Residual',
      unit: '',
      property: 'diagnostic',
      source: (s) => s.conservationResidual,
    },
  ];

  series.componentIds.forEach((component, index) => {
    quantities.push({
      id: `${COMPOSITIONAL_QUANTITY_PREFIX}inventory-${component}`,
      label: `${component} In Place`,
      unit: 'mol',
      property: 'component-inventory',
      source: (s) => s.componentInventory[index] ?? [],
    });
    quantities.push({
      id: `${COMPOSITIONAL_QUANTITY_PREFIX}net-well-moles-${component}`,
      label: `${component} Net Injected`,
      unit: 'mol',
      property: 'component-well-moles',
      source: (s) => s.componentNetWellMoles[index] ?? [],
    });
  });

  return quantities;
}

/** Look one up by id, or `undefined`. */
export function findCompositionalRunQuantity(
  series: CompositionalRunSeries,
  id: string,
): CompositionalRunQuantity | undefined {
  return compositionalRunQuantities(series).find((quantity) => quantity.id === id);
}
