import { describe, expect, it } from 'vitest';
import { listDeclaredOpmFlowArtifactKeys, resolveScenarioReferenceSeries } from '../opmFlowArtifacts';
import { getScenario } from '../scenarios';
import { compareWithFlowTwin, expectWithinFlowTwinBands } from '../opmFlowTwin';

describe('wf_bl1d scenario definition', () => {
    it('declares its OPM Flow twin over the shipped schedule', () => {
        const scenario = getScenario('wf_bl1d');

        expect(listDeclaredOpmFlowArtifactKeys(scenario?.referenceSources)).toEqual(['wf_bl1d']);
        const series = resolveScenarioReferenceSeries(scenario?.referenceSources);
        expect(series.map((entry) => entry.curveKey)).toEqual([
            'opm-water-cut', 'opm-oil-rate', 'opm-cum-oil', 'opm-avg-pressure', 'opm-cum-injection',
        ]);
        // The deck and the scenario must cover the same flood, or the curves are compared
        // over different horizons.
        const lastDay = Math.max(...series.map((entry) => entry.data.at(-1)!.x));
        expect(lastDay).toBe(Number(scenario?.params.steps) * Number(scenario?.params.delta_t_days));
    });
});

describe('wf_bl1d against its OPM Flow twin', () => {
    /**
     * The shipped run, through the worker's own setup, against Flow on the deck
     * `opm_flow_tool/cases.py` writes from the same scenario parameters, the run whose curves
     * the chart overlays. Both solve the same 96-cell, 50-day waterflood with the same pressure
     * controls.
     *
     * Measured 2026-11-02: oil 2.65 % worst / 0.42 % final, injection 1.07 %, breakthrough
     * 15.0 d against Flow's 14.75 d (one report step), final water cut 0.9728 against 0.9686.
     */
    it('runs the shipped case to the Flow run of the same model', async () => {
        const comparison = await compareWithFlowTwin({
            scenarioKey: 'wf_bl1d',
            dimensionKey: 'mobility',
            variantKey: 'mob_base',
            artifactKey: 'wf_bl1d',
        });
        expectWithinFlowTwinBands(comparison, {
            oilWorstOfFinal: 0.04,
            oilFinal: 0.01,
            injection: 0.02,
            breakthroughDays: 0.5,
            finalWaterCut: 0.01,
        });
    }, 300_000);
});
