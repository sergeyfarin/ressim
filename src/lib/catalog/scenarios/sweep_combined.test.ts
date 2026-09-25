import { describe, it } from 'vitest';
import { compareWithFlowTwin, expectWithinFlowTwinBands } from '../opmFlowTwin';

describe('sweep_combined against its OPM Flow twin', () => {
    /**
     * The shipped run, through the worker's own setup, against Flow on
     * `opm/reference-decks/small-direct/sweep-combined`, the deck the chart's Flow curves
     * come from.
     * The base case, which both dimensions run ("Favorable (M = 1) + layered" and "Layered").
     * The layered correlations assume sealed layers; these are sealed (k_v = 0.001 mD), and
     * Flow solves them as the scenario does.
     *
     * The run is the scenario's default solver; FIM on the same deck is closer still (see the
     * cross-solver scorecard). Measured 2026-09-25 over the shipped 75 report steps (375 d,
     * ~3.2 PVI): oil 1.9 % worst / 0.42 % final, injection 1.3 %, breakthrough 20 d against
     * 15 d (one report step), final water cut 0.9506 against 0.9499.
     */
    it('runs the shipped case to the Flow run of the same model', async () => {
        const comparison = await compareWithFlowTwin({
            scenarioKey: 'sweep_combined',
            dimensionKey: 'interaction_core',
            variantKey: 'interaction_favorable_layered',
            artifactKey: 'sweep_combined',
        });
        expectWithinFlowTwinBands(comparison, {
            oilWorstOfFinal: 0.04,
            oilFinal: 0.015,
            injection: 0.03,
            breakthroughDays: 5,
            finalWaterCut: 0.01,
        });
    }, 300_000);
});
