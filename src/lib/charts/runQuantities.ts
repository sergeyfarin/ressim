/**
 * runQuantities.ts — the named quantities a run can plot.
 *
 * The gap this closed: ResSim could not plot its own gas rate. `dep_gas_pz`'s
 * panel titled "Gas Rate" drew `oil-rate-sim` — identically ~0 for a dry-gas
 * reservoir — beside OPM's real gas rates, because `DerivedRunSeries` was a
 * closed struct of named fields and nobody had added one for gas. Adding a
 * quantity meant editing the derived-series type, the builder, the panel
 * defaults and the layout unions: the nine-file ripple the chart review
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
 * A quantity says *what* it is and where its values come from. Where it is
 * plotted is a separate question, answered by `simulationCurves.ts`.
 */

import type { DerivedRunSeries } from './axisAdapters';

export type RunQuantity = {
    /** Stable identity. */
    id: string;
    /** Human label, used to build the curve's legend entry. */
    label: string;
    /** Physical unit, for axis and tooltip copy. Empty for dimensionless. */
    unit: string;
    /** Classification for the single-property-per-panel rule. */
    property: string;
    /** Where the values come from, given a run's derived series. */
    source: (derived: DerivedRunSeries) => Array<number | null>;
};

export const RUN_QUANTITIES = {
    'oil-rate': {
        id: 'oil-rate', label: 'Oil Rate', unit: 'Sm³/day', property: 'oil-rate',
        source: (derived) => derived.oilRate,
    },
    'gas-rate': {
        id: 'gas-rate', label: 'Gas Rate', unit: 'Sm³/day', property: 'gas-rate',
        source: (derived) => derived.gasRate,
    },
    'injection-rate': {
        id: 'injection-rate', label: 'Injection Rate', unit: 'Sm³/day', property: 'injection-rate',
        source: (derived) => derived.injectionRate,
    },
    'water-cut': {
        id: 'water-cut', label: 'Water Cut', unit: '', property: 'water-cut',
        source: (derived) => derived.waterCut,
    },
    'gas-cut': {
        id: 'gas-cut', label: 'Gas Cut', unit: '', property: 'gas-cut',
        source: (derived) => derived.gasCut,
    },
    'gor': {
        id: 'gor', label: 'GOR', unit: 'Sm³/Sm³', property: 'gor',
        source: (derived) => derived.gor,
    },
    'average-water-saturation': {
        id: 'average-water-saturation', label: 'Avg Water Sat', unit: '', property: 'average-water-saturation',
        source: (derived) => derived.avgWaterSat,
    },
    'average-pressure': {
        id: 'average-pressure', label: 'Avg Pressure', unit: 'bar', property: 'average-pressure',
        source: (derived) => derived.pressure,
    },
    'p-over-z': {
        id: 'p-over-z', label: 'p/z', unit: 'bar', property: 'p-over-z',
        source: (derived) => derived.p_z,
    },
    'producer-bhp': {
        id: 'producer-bhp', label: 'Producer WBHP', unit: 'bar', property: 'producer-bhp',
        source: (derived) => derived.producerBhp,
    },
    'injector-bhp': {
        id: 'injector-bhp', label: 'Injector WBHP', unit: 'bar', property: 'injector-bhp',
        source: (derived) => derived.injectorBhp,
    },
    // Oil and gas recovery are fractions of different volumes in place, so the
    // label names both the phase and its denominator. See `reservoirVolumes.ts`.
    'recovery-oil': {
        id: 'recovery-oil', label: 'Recovery — Oil (of STOIIP)', unit: '', property: 'recovery-factor',
        source: (derived) => derived.recovery,
    },
    'recovery-gas': {
        id: 'recovery-gas', label: 'Recovery — Gas (of GIIP)', unit: '', property: 'recovery-factor',
        source: (derived) => derived.recoveryGas,
    },
    'cumulative-oil': {
        id: 'cumulative-oil', label: 'Cum Oil', unit: 'Sm³', property: 'cumulative-oil',
        source: (derived) => derived.cumulativeOil,
    },
    'cumulative-gas': {
        id: 'cumulative-gas', label: 'Cum Gas', unit: 'Sm³', property: 'cumulative-gas',
        source: (derived) => derived.cumulativeGas,
    },
    'cumulative-injection': {
        id: 'cumulative-injection', label: 'Cum Injection', unit: 'Sm³', property: 'cumulative-injection',
        source: (derived) => derived.cumulativeInjection,
    },
} as const satisfies Record<string, RunQuantity>;

export type RunQuantityId = keyof typeof RUN_QUANTITIES;

export function getRunQuantity(id: RunQuantityId): RunQuantity {
    return RUN_QUANTITIES[id];
}
