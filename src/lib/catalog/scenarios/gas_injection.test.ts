import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { buildCreatePayloadForRun, buildScenarioRunSpecs } from '../../scenario/runModel';
import { buildBenchmarkRunResult } from '../../benchmarkRunModel';
import { integrateRunSeries } from '@ressim/quantities/runSeries';
import { configureReservoirSimulator } from '../../workers/configureSimulator';
import type { RateHistoryPoint } from '../../simulator-types';
import { listOpmFlowArtifacts } from '../opmFlowArtifacts';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

/**
 * `flow 2026.04` on `opm/reference-decks/small-direct/go-1d-50`: [t (d), FOPT, FGPT, FGIT] in Sm³.
 * The same numbers grade the engine-side twin in `three_phase_acceptance.rs`. This test grades
 * the scenario itself, so an edit that decouples `gas_injection.ts` from the twin, or from the
 * deck, fails here.
 */
const OPM_GAS_INJECTION: Array<[number, number, number, number]> = [
    [100, 6167.628, 0, 6226.662],
    [200, 15379.979, 3657.553, 19077.826],
    [300, 20331.672, 28986.195, 49359.785],
];

describe('gas_injection against its OPM Flow twin (#12)', () => {
    it('runs the shipped base case to the Flow cumulatives', async () => {
        await ensureWasmReady();
        const [spec] = buildScenarioRunSpecs({
            scenarioKey: 'gas_injection',
            dimensionKey: 'mobility',
            variantKeys: ['mob_base'],
        });
        const payload = buildCreatePayloadForRun(spec);
        const sim = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));
        configureReservoirSimulator(sim, payload);
        const dt = Number(spec.params.delta_t_days);
        expect(dt).toBe(2);
        for (let step = 0; step < 150; step += 1) sim.step(dt);
        const history = sim.getRateHistorySince(0) as RateHistoryPoint[];
        const warning = sim.getLastSolverWarning();
        sim.free();
        expect(warning).toBe('');

        const cumulative = integrateRunSeries(history);
        for (const [t, flowOil, flowGasProduced, flowGasInjected] of OPM_GAS_INJECTION) {
            const index = history.findIndex((point) => Math.abs(Number(point.time) - t) < 1e-6);
            expect(index, `no report at ${t} d`).toBeGreaterThanOrEqual(0);
            // Oil produced and gas injected: measured ≤ 0.046 % (2026-09-24, after #42).
            expect(Math.abs(cumulative.oil[index] - flowOil) / flowOil, `FOPT at ${t} d`).toBeLessThan(0.002);
            expect(Math.abs(cumulative.injection[index] - flowGasInjected) / flowGasInjected, `FGIT at ${t} d`)
                .toBeLessThan(0.002);
            // Gas produced starts at breakthrough (170-180 d in both): 0.29 % at 200 d, 0.03 % at 300 d.
            if (flowGasProduced > 0) {
                expect(Math.abs(cumulative.gas[index] - flowGasProduced) / flowGasProduced, `FGPT at ${t} d`)
                    .toBeLessThan(0.015);
            } else {
                expect(cumulative.gas[index], `gas produced before breakthrough at ${t} d`).toBeLessThan(1);
            }
        }

        // #43: the chart's PVI, built the way the chart builds it, is Flow's FVIT over the same
        // pore volume: both convert at the average reservoir pressure. Converting at the injector
        // cell instead reads 0.8-1.0 % low here; the surface series happens to agree only because
        // this fluid's Bg is 1 at the 250 bar reference, which the average stays near.
        const { pviSeries } = buildBenchmarkRunResult({ spec, rateHistory: history });
        const flowAxis = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === 'gas_injection')!.xAxis!;
        for (const [t] of OPM_GAS_INJECTION) {
            const simPvi = pviSeries[history.findIndex((point) => Math.abs(Number(point.time) - t) < 1e-6)]!;
            const flowPvi = flowAxis.pvi![flowAxis.timeDays.indexOf(t)];
            expect(Math.abs(simPvi - flowPvi) / flowPvi, `PVI at ${t} d`).toBeLessThan(0.001);
        }
    }, 120_000);

    it('draws the same Flow run on its chart (#20)', () => {
        // The bundled artifact is a separate run of the same committed deck. It has to carry these
        // numbers, or the chart would grade the scenario against a different Flow result than the
        // engine gate does.
        const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === 'gas_injection')!;
        expect(artifact.provenance.deckSource).toBe('opm/reference-decks/small-direct/go-1d-50/CASE.DATA');
        const cumOil = artifact.series.find((series) => series.mnemonic === 'FOPT')!;
        const xAxis = artifact.xAxis!;
        // The text summary the artifact reads prints 7 significant digits; the constants above
        // come from the binary summary.
        const sameRun = (value: number, flow: number) => Math.abs(value - flow) / flow;
        for (const [t, flowOil, , flowGasInjected] of OPM_GAS_INJECTION) {
            const index = xAxis.timeDays.indexOf(t);
            expect(cumOil.data[index].x).toBe(t);
            expect(sameRun(cumOil.data[index].y, flowOil), `FOPT at ${t} d`).toBeLessThan(1e-6);
            expect(sameRun(xAxis.cumulativeInjectionSm3![index], flowGasInjected), `FGIT at ${t} d`).toBeLessThan(1e-6);
        }
    });
});
