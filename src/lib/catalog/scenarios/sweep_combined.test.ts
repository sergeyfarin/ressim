import { describe, it } from 'vitest';
import { compareWithFlowTwin, expectWithinFlowTwinBands } from '../opmFlowTwin';

describe('sweep_combined against its OPM Flow twin', () => {
    /**
     * The shipped run, through the worker's own setup, against Flow on
     * `opm/reference-decks/small-direct/sweep-combined`, the deck the chart's Flow curves
     * come from.
     * The scenario's own parameters (mu_o = 1) are no variant it runs, so the twin is the
     * favorable layered variant both dimensions draw. The layered correlations assume sealed
     * layers; these are nearly sealed (k_v = 0.001 mD), and Flow solves them as the scenario
     * does.
     *
     * The run is the scenario's default solver; FIM on the same deck is closer still (see the
     * cross-solver scorecard). Measured 2026-09-25: over the first 40 report steps (200 d,
     * 1.8 PVI): oil 2.4 % worst / 0.75 % final, injection 2.0 %, breakthrough 20 d against
     * 15 d (one report step), water cut 0.8991 against 0.8965 at 200 d.
     *
     * Graded over the first 40 report steps: they cover breakthrough and most of the recovery,
     * and the full 1,000-day run costs another ~17 s of IMPES substeps for a tail at 99 %
     * water cut.
     */
    it('runs the shipped case to the Flow run of the same model', async () => {
        const comparison = await compareWithFlowTwin({
            scenarioKey: 'sweep_combined',
            dimensionKey: 'interaction_core',
            variantKey: 'interaction_favorable_layered',
            artifactKey: 'sweep_combined',
            steps: 40,
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
