/**
 * simulationCurves.ts — which curves a run contributes, as data.
 *
 * Step 4 of `docs/CHART_ARCHITECTURE_REVIEW_2026-08-02.md`, and the end of the
 * emit-everything-then-filter inversion for the simulation family.
 *
 * `buildChartData` used to carry three near-identical branches — one per
 * `simulationCurveSet` — of roughly 20 hand-written `appendSeries(panels.X, …)`
 * blocks each, naming a panel and a derived-series field in code. Adding a
 * curve meant editing all three; the branches had already drifted (the recovery
 * curve was relabelled in one of them and not the others). They are now one
 * table: each row says which quantity goes in which panel, for which curve
 * sets, and the builder loops over it.
 *
 * The quantity itself — label, unit, property, where the values come from —
 * lives in `runQuantities.ts`. A row here only places it.
 */

import type { DerivedRunSeries } from './axisAdapters';
import type { PrimaryRateCurve } from '../catalog/scenarios';
import type { RateChartPanelKey } from './rateChartLayoutConfig';
import { RUN_QUANTITIES, type RunQuantityId } from './runQuantities';

export type SimulationCurveDescriptor = {
    /** The quantity plotted, from the run-quantity registry. */
    quantity: RunQuantityId;
    /** Where it goes. */
    panel: RateChartPanelKey;
    /** Curve key, kept explicit because layouts and artifacts reference it. */
    curveKey: string;
    /**
     * Which simulation curve sets include this curve. The set is chosen by the
     * scenario's analytical method (`AnalyticalMethodDescriptor.simulationCurveSet`),
     * so a waterflood leads with water cut and a depletion case with oil rate.
     */
    sets: readonly PrimaryRateCurve[];
    /**
     * Which x values to plot against. `run` is the report-time axis every rate
     * uses; `history` is the well-history axis, which is sampled separately and
     * so needs its own mapping.
     */
    axis?: 'run' | 'history';
};

const ALL_SETS = ['water-cut', 'gas-cut', 'oil-rate'] as const;

/**
 * The order here is the order curves are appended, which is legend order and
 * draw order within a panel. It reproduces what the three branches did.
 */
export const SIMULATION_CURVES: readonly SimulationCurveDescriptor[] = [
    { quantity: 'water-cut', panel: 'rates', curveKey: 'water-cut-sim', sets: ['water-cut'] },
    { quantity: 'gas-cut', panel: 'rates', curveKey: 'gas-cut-sim', sets: ['gas-cut'] },
    { quantity: 'oil-rate', panel: 'rates', curveKey: 'oil-rate-sim', sets: ['oil-rate'] },
    { quantity: 'average-water-saturation', panel: 'avg_water_sat', curveKey: 'avg-water-sat', sets: ['water-cut'] },
    { quantity: 'recovery-oil', panel: 'recovery', curveKey: 'recovery-factor-primary', sets: ALL_SETS },
    { quantity: 'recovery-gas', panel: 'recovery', curveKey: 'recovery-factor-gas', sets: ['oil-rate'] },
    { quantity: 'cumulative-oil', panel: 'cumulative', curveKey: 'cum-oil-sim', sets: ALL_SETS },
    { quantity: 'oil-rate', panel: 'oil_rate', curveKey: 'oil-rate-sim', sets: ALL_SETS },
    { quantity: 'gas-rate', panel: 'gas_rate', curveKey: 'gas-rate-sim', sets: ALL_SETS },
    { quantity: 'cumulative-gas', panel: 'cumulative_gas', curveKey: 'cum-gas-sim', sets: ALL_SETS },
    { quantity: 'injection-rate', panel: 'injection_rate', curveKey: 'injection-rate-sim', sets: ALL_SETS },
    { quantity: 'cumulative-injection', panel: 'volumes', curveKey: 'cum-injection', sets: ['water-cut', 'gas-cut'] },
    { quantity: 'average-pressure', panel: 'diagnostics', curveKey: 'avg-pressure-sim', sets: ALL_SETS },
    { quantity: 'p-over-z', panel: 'pz', curveKey: 'p-over-z-sim', sets: ALL_SETS },
    { quantity: 'gor', panel: 'gor', curveKey: 'gor-sim', sets: ALL_SETS },
    { quantity: 'producer-bhp', panel: 'producer_bhp', curveKey: 'producer-bhp-sim', sets: ['gas-cut', 'oil-rate'], axis: 'history' },
    { quantity: 'injector-bhp', panel: 'injector_bhp', curveKey: 'injector-bhp-sim', sets: ['gas-cut', 'oil-rate'], axis: 'history' },
];

/** The curves a given simulation curve set contributes, in append order. */
export function simulationCurvesForSet(set: PrimaryRateCurve): readonly SimulationCurveDescriptor[] {
    return SIMULATION_CURVES.filter((curve) => curve.sets.includes(set));
}

/** Label and values for one descriptor, resolved against a run. */
export function resolveSimulationCurve(
    descriptor: SimulationCurveDescriptor,
    derived: DerivedRunSeries,
): { label: string; property: string; values: Array<number | null> } {
    const quantity = RUN_QUANTITIES[descriptor.quantity];
    return { label: quantity.label, property: quantity.property, values: quantity.source(derived) };
}
