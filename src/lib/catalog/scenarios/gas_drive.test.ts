import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../scenarios';
import { integrateRunSeries } from '@ressim/quantities/runSeries';
import { getStockTankOilInPlace } from '@ressim/quantities/reservoirVolumes';
import { buildBenchmarkCreatePayload } from '../../benchmarkRunModel';
import { configureReservoirSimulator } from '../../workers/configureSimulator';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    if (!wasmReady) {
        wasmReady = readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url)).then(
            (wasmBytes) => initWasm({ module_or_path: wasmBytes }),
        );
    }
    await wasmReady;
}

type Params = Record<string, unknown>;

/**
 * 300 days at the scenario's own 10 d step — enough for every rung of the
 * critical-gas-saturation ladder to fall to the 130 bar comparison pressure
 * (the slowest, s_gc = 0.15, reaches it at 250 d). The shipped run continues
 * to 600 d.
 */
const STEPS = 30;

/** Average reservoir pressure the rungs are compared at. */
const COMPARISON_PRESSURE_BAR = 130;

/** A simulator for `params`, built the way a scenario run builds it (payload + worker setup). */
function configure(params: Params, steps: number): ReservoirSimulator {
    const payload = buildBenchmarkCreatePayload({ ...params, steps });
    const sim = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));
    configureReservoirSimulator(sim, payload);
    return sim;
}

function buildAndRun(params: Params, steps: number): any[] {
    const sim = configure(params, steps);
    const dt = Number(params.delta_t_days);
    for (let i = 0; i < steps; i++) sim.step(dt);
    const history = sim.getRateHistorySince(0) as any[];
    sim.free();
    return history;
}

/** Saturated Rs at `pressureBar`, linearly interpolated on the scenario's own PVT table. */
function saturatedRs(params: Params, pressureBar: number): number {
    const rows = (params.pvtTable as Array<{ p_bar: number; rs_m3m3: number }>)
        .slice()
        .sort((a, b) => a.p_bar - b.p_bar);
    for (let i = 1; i < rows.length; i += 1) {
        if (rows[i].p_bar >= pressureBar) {
            const t = (pressureBar - rows[i - 1].p_bar) / (rows[i].p_bar - rows[i - 1].p_bar);
            return rows[i - 1].rs_m3m3 + t * (rows[i].rs_m3m3 - rows[i - 1].rs_m3m3);
        }
    }
    return rows[rows.length - 1].rs_m3m3;
}

type AtPressure = { time: number; gor: number; gasSaturation: number; oilRecovery: number };

/**
 * The state of a run when the *reservoir* first reaches a given average
 * pressure. Sampling at matched pressure rather than matched time is the whole
 * point: the discarded permeability ladder looked like three different curves
 * on a time axis and turned out to be one curve on three clocks.
 */
function sampleAtPressure(params: Params, history: any[], pressureBar: number): AtPressure | null {
    const index = history.findIndex((h) => Number(h.avg_reservoir_pressure) <= pressureBar);
    if (index < 0) return null;
    const cumulative = integrateRunSeries(history);
    const stockTankOilInPlace = getStockTankOilInPlace(params);
    expect(stockTankOilInPlace).not.toBeNull();
    return {
        time: Number(history[index].time),
        gor: Number(history[index].producing_gor),
        gasSaturation: Number(history[index].avg_gas_saturation),
        oilRecovery: cumulative.oil[index] / stockTankOilInPlace!,
    };
}

describe('gas_drive — critical gas saturation changes the drive, not the clock', () => {
    /**
     * Measured at a matched 130 bar average pressure on the corrected Standing
     * fluid (#60, solution GOR 132 m3/m3):
     *
     *   s_gc   GOR [m3/m3]   Sg      oil RF
     *   0.02   1326          0.183   3.5 %
     *   0.05   1094          0.187   4.2 %
     *   0.15    520          0.220   9.4 %
     *
     * On the lean fluid before #60 (solution GOR 28) it was GOR 685 / 474 / 103,
     * Sg 0.114 / 0.120 / 0.160, RF 2.2 / 3.0 / 8.9 %.
     *
     * Contrast the ladder this replaced, on the same axis: GOR 459/463/460 and
     * Sg 0.106/0.109/0.110 across a 100x permeability range.
     */
    it('the rungs separate at matched average pressure', async () => {
        await ensureWasmReady();

        const samples = ['sgc_low', 'sgc_base', 'sgc_high'].map((variantKey) => {
            const params = getScenarioWithVariantParams('gas_drive', 's_gc', variantKey);
            const sample = sampleAtPressure(params, buildAndRun(params, STEPS), COMPARISON_PRESSURE_BAR);
            expect(sample, `${variantKey} never reached ${COMPARISON_PRESSURE_BAR} bar`).not.toBeNull();
            return { variantKey, s_gc: Number(params.s_gc), ...sample! };
        });

        const [low, base, high] = samples;
        expect([low.s_gc, base.s_gc, high.s_gc]).toEqual([0.02, 0.05, 0.15]);

        // Trapping liberated gas keeps it in the reservoir instead of the
        // tubing: GOR falls monotonically as s_gc rises…
        expect(low.gor).toBeGreaterThan(base.gor);
        expect(base.gor).toBeGreaterThan(high.gor);
        // …and the separation is large, not a rounding difference. The
        // permeability ladder this replaced spanned 1.01x here. Measured 2.55x
        // (6.6x before #60: with 4.7x more gas in solution, liberation adds
        // gas on every rung, so trapping it moves the GOR by a smaller factor).
        expect(low.gor / high.gor).toBeGreaterThan(2);

        // Retained gas is gas still in place doing work, so Sg is higher…
        expect(high.gasSaturation).toBeGreaterThan(base.gasSaturation);
        expect(base.gasSaturation).toBeGreaterThan(low.gasSaturation);

        // …and it produces more oil for the same reservoir pressure drop,
        // which is the reservoir-engineering point of the dimension.
        expect(high.oilRecovery).toBeGreaterThan(base.oilRecovery);
        expect(base.oilRecovery).toBeGreaterThan(low.oilRecovery);
        expect(high.oilRecovery / low.oilRecovery).toBeGreaterThan(2);
    }, 300000);
});

/**
 * #12: the solution-gas-drive story the scenario tells, checked on its base case through the
 * same payload and worker setup a run uses.
 */
describe('gas_drive — a saturated start that liberates gas and raises the GOR', () => {
    it('starts on the saturated curve with the declared free gas', async () => {
        await ensureWasmReady();
        const params = getScenarioWithVariantParams('gas_drive', 'sg_init', 'sg_base');
        const sim = configure(params, 1);
        const initialPressure = Number(params.initialPressure);
        const rsBubble = saturatedRs(params, initialPressure);
        const rs = Array.from(sim.getRs());
        const sg = Array.from(sim.getSatGas());
        sim.free();

        // Initial pressure is the bubble point: every cell holds the maximum dissolved gas…
        for (const value of rs) expect(value).toBeCloseTo(rsBubble, 6);
        // …and the declared free gas sits alongside it.
        for (const value of sg) expect(value).toBeCloseTo(Number(params.initialGasSaturation), 12);
    });

    it('liberates solution gas as pressure falls', async () => {
        await ensureWasmReady();
        const params = getScenarioWithVariantParams('gas_drive', 'sg_init', 'sg_base');
        const sim = configure(params, STEPS);
        for (let i = 0; i < STEPS; i++) sim.step(Number(params.delta_t_days));
        const history = sim.getRateHistorySince(0) as any[];
        const rs = Array.from(sim.getRs());
        const pressures = Array.from(sim.getPressures());
        sim.free();

        const rsBubble = saturatedRs(params, Number(params.initialPressure));
        const meanRs = rs.reduce((sum, value) => sum + value, 0) / rs.length;
        const finalPressure = Number(history.at(-1).avg_reservoir_pressure);
        // Pressure is well below the bubble point, and the oil holds only what the saturated
        // curve allows at the pressure each cell is at: dissolved gas has come out of solution.
        expect(finalPressure).toBeLessThan(Number(params.initialPressure) - 50);
        rs.forEach((value, cell) => expect(value).toBeCloseTo(saturatedRs(params, pressures[cell]), 3));
        expect(meanRs).toBeLessThan(0.8 * rsBubble);
    }, 300000);

    it('produces far above the solution GOR, and the GOR keeps rising', async () => {
        await ensureWasmReady();
        const params = getScenarioWithVariantParams('gas_drive', 'sg_init', 'sg_base');
        const history = buildAndRun(params, STEPS);
        const rsBubble = saturatedRs(params, Number(params.initialPressure));
        const gors = history.map((point) => Number(point.producing_gor));

        // Measured on the corrected Standing fluid (#60; base rung, solution GOR 132 m³/m³): 806
        // at the first step, rising every step to 1237 at 300 d, i.e. 6.1-9.4x the solution GOR.
        // The declared free gas (Sg 0.08) is above critical (0.05), so it flows from the start, and
        // liberation keeps adding to it. The bound was 10x on the lean fluid before #60 (solution
        // GOR 28, producing 386-514, 14-18x): the free-gas share is the same physics, but the oil
        // now carries 4.7x more dissolved gas, so the ratio to it is smaller.
        for (const gor of gors) expect(gor).toBeGreaterThan(5 * rsBubble);
        for (let i = 1; i < gors.length; i += 1) {
            expect(gors[i], `GOR fell at step ${i}`).toBeGreaterThan(gors[i - 1] * (1 - 0.005));
        }
        expect(gors.at(-1)!).toBeGreaterThan(1.25 * gors[0]);
    }, 300000);
});
