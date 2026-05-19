import { describe, expect, it } from 'vitest';
import {
    getOpmFlowArtifactsForScenario,
    getOpmFlowPublishedReferenceSeries,
    listOpmFlowArtifacts,
} from './opmFlowArtifacts';

describe('OPM Flow precomputed artifacts', () => {
    it('ships explicit metadata for predefined OPM Flow artifact targets', () => {
        const artifacts = listOpmFlowArtifacts();

        expect(artifacts.map((artifact) => artifact.scenarioKey).sort()).toEqual([
            'spe1_gas_injection',
            'wf_bl1d',
        ]);
        for (const artifact of artifacts) {
            expect(artifact.schemaVersion).toBe(1);
            expect(artifact.sourceType).toBe('opm-flow-precomputed');
            expect(artifact.deckHash.length).toBeGreaterThan(8);
            expect(artifact.units.time).toBe('days');
            expect(artifact.supportedCurves.length).toBeGreaterThan(0);
        }
    });

    it('does not expose deck-ready placeholders as chart curves', () => {
        expect(getOpmFlowArtifactsForScenario('wf_bl1d')).toHaveLength(1);
        expect(getOpmFlowPublishedReferenceSeries('wf_bl1d')).toEqual([]);
    });
});
