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
     * FIM is the shipped default after #63: the conservative IMPES closure needs many
     * small transport steps on this layered grid. Both solvers remain covered by the
     * native cross-solver matrix; this test exercises the interactive default.
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
