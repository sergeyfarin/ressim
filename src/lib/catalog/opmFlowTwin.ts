import { readFile } from 'node:fs/promises';
import { expect } from 'vitest';
import initWasm, { ReservoirSimulator } from '../ressim/pkg/simulator.js';
import { buildCreatePayloadForRun, buildScenarioRunSpecs } from '../scenario/runModel';
import { integrateRunSeries } from '@ressim/quantities/runSeries';
import { configureReservoirSimulator } from '../workers/configureSimulator';
import type { RateHistoryPoint } from '../simulator-types';
import { listOpmFlowArtifacts } from './opmFlowArtifacts';

/**
 * Test support for grading a scenario against the Flow run of its small-direct twin: the deck
 * `opm_small_direct.rs` writes from the same simulator setup the scenario ships. Node only; the
 * scenario tests import it, the app does not.
 *
 * The run goes through the worker's own path (`buildCreatePayloadForRun` then
 * `configureReservoirSimulator`), so a scenario edit that the twin does not mirror moves the
 * answer away from Flow and fails the scenario's test.
 */

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

export type FlowTwinComparison = {
    /**
     * Cumulative oil and cumulative water injected against FOPT and FWIT: the largest difference
     * over the Flow report times as a fraction of Flow's final value (a relative error on the
     * first few report steps' small volumes says little), and the relative error at the end.
     */
    oil: { worstOfFinal: number; final: number };
    injection: { worstOfFinal: number; final: number };
    /** First report time with a water cut above 1 %, days: the run's and Flow's (FWCT). Both are
     * read on the same report times, so the difference is a whole number of report steps. */
    breakthroughDays: { run: number; flow: number };
    /** Water cut at the last report time graded: the run's and Flow's. */
    finalWaterCut: { run: number; flow: number };
    /** The engine's last solver warning, '' when there was none. */
    warning: string;
};

const BREAKTHROUGH_WATER_CUT = 0.01;

/**
 * Run one variant of a scenario over its own schedule and compare it with a Flow artifact at
 * every Flow report time.
 */
export async function compareWithFlowTwin(input: {
    scenarioKey: string;
    dimensionKey: string;
    variantKey: string;
    artifactKey: string;
    /** Report steps to run and grade; the scenario's own count when absent. */
    steps?: number;
}): Promise<FlowTwinComparison> {
    await ensureWasmReady();
    const [spec] = buildScenarioRunSpecs({
        scenarioKey: input.scenarioKey,
        dimensionKey: input.dimensionKey,
        variantKeys: [input.variantKey],
    });
    const payload = buildCreatePayloadForRun(spec);
    const sim = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));
    const dt = Number(spec.params.delta_t_days);
    const steps = input.steps ?? Number(spec.params.steps);
    let history: RateHistoryPoint[];
    let warning: string;
    try {
        configureReservoirSimulator(sim, payload);
        for (let step = 0; step < steps; step += 1) sim.step(dt);
        history = sim.getRateHistorySince(0) as RateHistoryPoint[];
        warning = sim.getLastSolverWarning();
    } finally {
        sim.free();
    }

    const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === input.artifactKey);
    const xAxis = artifact?.xAxis;
    if (!artifact || !xAxis?.cumulativeInjectionSm3) {
        throw new Error(`no parsed Flow artifact '${input.artifactKey}' with a cumulative-injection axis`);
    }
    const flowSeries = (mnemonic: string) => {
        const series = artifact.series.find((candidate) => candidate.mnemonic === mnemonic);
        if (!series) throw new Error(`${input.artifactKey}: no ${mnemonic} series`);
        return series.data.map((point) => point.y);
    };
    const flowOil = flowSeries('FOPT');
    const flowWaterCut = flowSeries('FWCT');
    const flowInjection = xAxis.cumulativeInjectionSm3;

    // Flow's summary also carries its own internal timesteps; compare where it reports on one of
    // the run's report times, which every report step ends on.
    const cumulative = integrateRunSeries(history);
    const pairs = xAxis.timeDays.flatMap((t, flowIndex) => {
        const runIndex = history.findIndex((point) => Math.abs(Number(point.time) - t) < 1e-6);
        return runIndex < 0 ? [] : [{ t, flowIndex, runIndex }];
    });
    const horizon = steps * dt;
    if (Math.abs((pairs.at(-1)?.t ?? 0) - horizon) > 1e-6) {
        throw new Error(`${input.scenarioKey}: the Flow run has no report at ${horizon} d`);
    }
    const runWaterCut = pairs.map(({ runIndex }) => {
        const point = history[runIndex];
        const liquid = Math.abs(Number(point.total_production_liquid));
        return liquid > 0 ? 1 - Math.abs(Number(point.total_production_oil)) / liquid : 0;
    });
    const errors = (run: number[], flow: number[]) => {
        const final = flow[pairs.at(-1)!.flowIndex];
        const diffs = pairs.map(({ flowIndex, runIndex }) => Math.abs(run[runIndex] - flow[flowIndex]));
        return { worstOfFinal: Math.max(...diffs) / final, final: diffs.at(-1)! / final };
    };
    const breakthrough = (cuts: number[]) => {
        const index = cuts.findIndex((cut) => cut > BREAKTHROUGH_WATER_CUT);
        return index < 0 ? Number.POSITIVE_INFINITY : pairs[index].t;
    };
    const flowWaterCutAtReports = pairs.map(({ flowIndex }) => flowWaterCut[flowIndex]);
    return {
        oil: errors(cumulative.oil, flowOil),
        injection: errors(cumulative.injection, flowInjection),
        breakthroughDays: { run: breakthrough(runWaterCut), flow: breakthrough(flowWaterCutAtReports) },
        finalWaterCut: { run: runWaterCut.at(-1)!, flow: flowWaterCutAtReports.at(-1)! },
        warning,
    };
}

/** How far a scenario's run may sit from its Flow twin; each scenario test states its own. */
export type FlowTwinBands = {
    /** Worst cumulative-oil difference over the report times, as a fraction of Flow's final value. */
    oilWorstOfFinal: number;
    /** Relative cumulative-oil error at the last report time. */
    oilFinal: number;
    /** Worst cumulative-injection difference, as a fraction of Flow's final value. */
    injection: number;
    /** Breakthrough (1 % water cut) time difference, days. */
    breakthroughDays: number;
    /** Water-cut difference at the last report time, absolute. */
    finalWaterCut: number;
};

export function expectWithinFlowTwinBands(comparison: FlowTwinComparison, bands: FlowTwinBands): void {
    expect(comparison.warning).toBe('');
    expect(comparison.oil.worstOfFinal, 'cumulative oil, worst report').toBeLessThan(bands.oilWorstOfFinal);
    expect(comparison.oil.final, 'cumulative oil, last report').toBeLessThan(bands.oilFinal);
    expect(comparison.injection.worstOfFinal, 'cumulative injection, worst report').toBeLessThan(bands.injection);
    expect(
        Math.abs(comparison.breakthroughDays.run - comparison.breakthroughDays.flow),
        `breakthrough ${comparison.breakthroughDays.run} d against Flow's ${comparison.breakthroughDays.flow} d`,
    ).toBeLessThanOrEqual(bands.breakthroughDays);
    expect(
        Math.abs(comparison.finalWaterCut.run - comparison.finalWaterCut.flow),
        'water cut, last report',
    ).toBeLessThan(bands.finalWaterCut);
}
