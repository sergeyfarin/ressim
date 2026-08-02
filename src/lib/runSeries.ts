/**
 * runSeries.ts — cumulative production and injection, integrated once.
 *
 * This was four implementations of one loop: `benchmarkRunModel` (for recovery
 * and PVI), `charts/analyticalParamAdapters` twice (the derived series and the
 * material balance), and `ReferenceComparisonChart.svelte` inline. They agreed
 * by coincidence rather than by construction, and one of them had already
 * drifted — the component carried a private `getPoreVolume` that ignored
 * `cellDzPerLayer`.
 *
 * **The rule is a rectangle, and that is not an approximation.** The engine
 * reports a step-*average* rate over the interval that ends at `point.time`, so
 * `rate × dt` is exact and a trapezoid rule is wrong. Measured on `gas_drive`:
 * the rectangle rule closes the Havlena-Odeh balance to N_mbe/N_volumetric =
 * 1.0000, while a trapezoid gives 1.124 (final cumulative oil 2841.9 vs
 * 3224.6 Sm³, +13.5 %). Do not "improve" this loop.
 */

import { toFiniteNumber } from './reservoirVolumes';
import type { RateHistoryPoint } from './simulator-types';

export type CumulativeRunSeries = {
    /** Report times [days], one per rate-history point. */
    time: number[];
    /** Cumulative produced oil at surface conditions [Sm³]. */
    oil: number[];
    /** Cumulative produced gas at surface conditions [Sm³]. */
    gas: number[];
    /** Cumulative produced liquid (oil + water) at surface conditions [Sm³]. */
    liquid: number[];
    /** Cumulative produced water at surface conditions [Sm³], as liquid − oil. */
    water: number[];
    /** Cumulative injection at surface conditions [Sm³], whatever phase was injected. */
    injection: number[];
};

/**
 * Integrates every cumulative quantity a chart or balance needs, in one pass.
 *
 * The first interval runs from t = 0 to the first report time, matching how the
 * engine reports its own first step.
 */
export function integrateRunSeries(rateHistory: readonly RateHistoryPoint[]): CumulativeRunSeries {
    const series: CumulativeRunSeries = {
        time: [], oil: [], gas: [], liquid: [], water: [], injection: [],
    };

    let oil = 0, gas = 0, liquid = 0, water = 0, injection = 0;

    for (let index = 0; index < rateHistory.length; index += 1) {
        const point = rateHistory[index];
        const time = toFiniteNumber(point.time, 0);
        const previousTime = index > 0 ? toFiniteNumber(rateHistory[index - 1]?.time, 0) : 0;
        const dt = Math.max(0, time - previousTime);

        const oilRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_oil, 0)));
        const liquidRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_liquid, 0)));
        const gasRate = Math.max(0, Math.abs(toFiniteNumber(point.total_production_gas, 0)));
        const injectionRate = Math.max(0, toFiniteNumber(point.total_injection, 0));

        oil += oilRate * dt;
        gas += gasRate * dt;
        liquid += liquidRate * dt;
        water += Math.max(0, liquidRate - oilRate) * dt;
        injection += injectionRate * dt;

        series.time.push(time);
        series.oil.push(oil);
        series.gas.push(gas);
        series.liquid.push(liquid);
        series.water.push(water);
        series.injection.push(injection);
    }

    return series;
}
