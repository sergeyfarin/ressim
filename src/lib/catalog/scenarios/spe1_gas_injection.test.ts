import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import type { RateHistoryPoint } from '../../simulator-types';
import { buildCreatePayloadForRun, buildScenarioRunSpecs } from '../../scenario/runModel';
import { configureReservoirSimulator } from '../../workers/configureSimulator';
import { listDeclaredOpmFlowArtifactKeys, listOpmFlowArtifacts } from '../opmFlowArtifacts';
import { getScenario } from '../scenarios';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

/** Worst relative gap against the Flow run of the same deck, by reported quantity. */
const FLOW_BANDS: Record<string, number> = {
    FPR: 0.005,
    FOPR: 0.01,
    'WGOR:PROD': 0.025,
};

function runValue(point: RateHistoryPoint, mnemonic: string): number {
    if (mnemonic === 'FPR') return Number(point.avg_reservoir_pressure);
    if (mnemonic === 'FOPR') return Number(point.total_production_oil);
    if (mnemonic === 'WGOR:PROD') return Number(point.producing_gor);
    throw new Error(`no scenario quantity for ${mnemonic}`);
}

describe('spe1_gas_injection scenario definition', () => {
    it('declares the published series and the OPM Flow run over the shipped horizon', () => {
        const scenario = getScenario('spe1_gas_injection');

        expect(scenario?.referenceSources?.map((source) => source.kind)).toEqual(['published', 'opm-flow']);
        expect(listDeclaredOpmFlowArtifactKeys(scenario?.referenceSources)).toEqual(['spe1_gas_injection']);

        const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === 'spe1_gas_injection');
        expect(artifact?.status).toBe('parsed');
        expect(artifact?.scenarioKey).toBe('spe1_gas_injection');
        const horizon = Number(scenario?.params.steps) * Number(scenario?.params.delta_t_days);
        expect(horizon).toBe(3600);
        for (const series of artifact!.series) {
            expect(series.data.at(-1)!.x, series.mnemonic).toBe(horizon);
        }
    });
});

describe('spe1_gas_injection against its OPM Flow twin', () => {
    /**
     * The shipped run, through the worker's own setup, against Flow on the same
     * `spe1-10x10x3` deck (#55). The comparison covers every 30-day report the two share;
     * Flow's four fractional startup reports are not on the scenario's report grid.
     *
     * Measured 2026-11-02 over all 120 shared reports: pressure 0.078 % worst, oil rate
     * 0.275 % (t=990 d), GOR 1.18 % (t=690 d).
     */
    it('runs the shipped case to the Flow run of the same model', async () => {
        await ensureWasmReady();
        const [spec] = buildScenarioRunSpecs({
            scenarioKey: 'spe1_gas_injection',
            dimensionKey: null,
            variantKeys: [],
        });
        expect(spec.steps).toBe(120);

        const payload = buildCreatePayloadForRun(spec);
        const sim = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));
        let history: RateHistoryPoint[];
        let warning: string;
        try {
            configureReservoirSimulator(sim, payload);
            for (let step = 0; step < Number(spec.steps); step += 1) sim.step(Number(spec.deltaTDays));
            history = sim.getRateHistorySince(0) as RateHistoryPoint[];
            warning = sim.getLastSolverWarning();
        } finally {
            sim.free();
        }
        expect(warning).toBe('');

        const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === 'spe1_gas_injection')!;
        for (const [mnemonic, band] of Object.entries(FLOW_BANDS)) {
            const series = artifact.series.find((candidate) => candidate.mnemonic === mnemonic);
            expect(series, `${mnemonic} in the Flow artifact`).toBeDefined();

            let compared = 0;
            let worst = 0;
            let worstAt = 0;
            for (const flowPoint of series!.data) {
                const runPoint = history.find((point) => Math.abs(Number(point.time) - flowPoint.x) < 1e-6);
                if (!runPoint) continue;
                const error = Math.abs(runValue(runPoint, mnemonic) - flowPoint.y) / Math.abs(flowPoint.y);
                if (error > worst) {
                    worst = error;
                    worstAt = flowPoint.x;
                }
                compared += 1;
            }

            expect(compared, `${mnemonic} shared reports`).toBeGreaterThanOrEqual(Number(spec.steps));
            expect(worst, `${mnemonic} worst at t=${worstAt} d`).toBeLessThan(band);
        }
    }, 300_000);
});
