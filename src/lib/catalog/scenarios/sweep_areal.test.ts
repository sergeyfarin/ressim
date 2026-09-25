import { describe, it } from 'vitest';
import { compareWithFlowTwin, expectWithinFlowTwinBands } from '../opmFlowTwin';

describe('sweep_areal against its OPM Flow twin', () => {
    /**
     * The shipped run, through the worker's own setup, against Flow on
     * `opm/reference-decks/small-direct/sweep-areal`, the deck the chart's Flow curves come from.
     * Craig's five-spot correlation is empirical; Flow solves the same 21 x 21 quarter five-spot.
     *
     * The run is the scenario's default solver; FIM on the same deck is closer still (see the
     * cross-solver scorecard). Measured 2026-09-25: oil 2.6 % worst / 0.53 % final,
     * injection 2.0 %, breakthrough 105 d against 100 d (one report step), final water cut
     * 0.9578 against 0.9540.
     */
    it('runs the shipped case to the Flow run of the same model', async () => {
        const comparison = await compareWithFlowTwin({
            scenarioKey: 'sweep_areal',
            dimensionKey: 'mobility',
            variantKey: 'mob_unit',
            artifactKey: 'sweep_areal',
        });
        expectWithinFlowTwinBands(comparison, {
            oilWorstOfFinal: 0.04,
            oilFinal: 0.01,
            injection: 0.03,
            breakthroughDays: 5,
            finalWaterCut: 0.01,
        });
    }, 300_000);
});
