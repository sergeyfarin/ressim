/**
 * runQuantities.ts — the named quantities a run can plot.
 *
 * The gap this closes: ResSim could not plot its own gas rate. `dep_gas_pz`'s
 * panel titled "Gas Rate" drew `oil-rate-sim` — identically ~0 for a dry-gas
 * reservoir — beside OPM's real gas rates, because `DerivedRunSeries` was a
 * closed struct of named fields and nobody had added one for gas. Adding a
 * quantity meant editing the derived-series type, the builder, the panel
 * defaults and the layout unions: the same nine-file ripple the chart review
 * measured.
 *
 * **A quantity is data, not a component.** The alternative considered was one
 * component per plot — `GasRatePlot.svelte`, `SaturationPlot.svelte` — with
 * scenarios picking a component. That was rejected: `ChartSubPanel.svelte`
 * already *is* the generic plot (13 presentation props, no domain vocabulary),
 * and the difference between a gas-rate plot and a saturation plot is which
 * series, title, unit and scale — data, not behaviour. A component per quantity
 * would fork every presentation feature (log toggle, legend grouping, history
 * divider, outlier suppression, axis switching) N ways and would put reservoir
 * vocabulary back into the one layer that is currently free of it. Reasoning
 * recorded in `docs/CHART_ARCHITECTURE_REVIEW_2026-08-02.md` §10.
 *
 * This registry is the first slice of the accessor described in the review's
 * §5: the rate family is served from it, and step 4 migrates the rest.
 */

import type { DerivedRunSeries } from './axisAdapters';

export type RunQuantity = {
    /** Stable identity, and the stem of the curve keys built from it. */
    id: string;
    /** Human label, used to build the curve's legend entry. */
    label: string;
    /** Physical unit, for axis and tooltip copy. */
    unit: string;
    /** Classification for the single-property-per-panel rule. */
    property: string;
    /** Where the values come from, given a run's derived series. */
    source: (derived: DerivedRunSeries) => Array<number | null>;
};

/**
 * Quantities served from this registry today.
 *
 * Deliberately not the full 22-field derived contract yet: the rate family is
 * the pilot, so that adding gas rate proves the pattern rather than adding
 * another hard-coded block. The rest migrate with step 4, when panels declare
 * their own curves.
 */
export const RUN_QUANTITIES = {
    'oil-rate': {
        id: 'oil-rate',
        label: 'Oil Rate',
        unit: 'Sm³/day',
        property: 'oil-rate',
        source: (derived) => derived.oilRate,
    },
    'gas-rate': {
        id: 'gas-rate',
        label: 'Gas Rate',
        unit: 'Sm³/day',
        property: 'gas-rate',
        source: (derived) => derived.gasRate,
    },
} as const satisfies Record<string, RunQuantity>;

export type RunQuantityId = keyof typeof RUN_QUANTITIES;

export function getRunQuantity(id: RunQuantityId): RunQuantity {
    return RUN_QUANTITIES[id];
}
