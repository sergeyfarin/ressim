import { describe, expect, it } from 'vitest';
import {
    getOpmFlowArtifactsForScenario,
    getOpmFlowPublishedReferenceSeries,
    listOpmFlowArtifacts,
} from './opmFlowArtifacts';
import { getScenario, listScenarios } from './scenarios';

// Case keys that have been confirmed to reach `status: "parsed"` with real
// Flow-run series (see docs/FRONTEND_EXECUTION_PLAN_2026-07.md Wave 1). This
// list must only grow — a committed artifact regressing from `parsed` back
// to `deck-ready`/`flow-run`/`error` means someone regenerated it without a
// real Flow run behind it, which is exactly the silent-stub failure mode
// this pipeline used to have.
const PARSED_BASELINE = ['wf_bl1d', 'spe1_gas_injection'];

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

    it('never regresses a case that has already reached a real parsed run', () => {
        const artifacts = listOpmFlowArtifacts();
        for (const caseKey of PARSED_BASELINE) {
            const artifact = artifacts.find((a) => a.caseKey === caseKey);
            expect(artifact, `expected a bundled artifact for case '${caseKey}'`).toBeDefined();
            expect(artifact!.status).toBe('parsed');
            expect(artifact!.series.length).toBeGreaterThan(0);
            for (const series of artifact!.series) {
                expect(series.data.length).toBeGreaterThan(0);
                // Sanity floor: a "dead well" deck bug (e.g. the 2026-07-17
                // EQUIL water-oil-contact bug) silently parses to a real but
                // physically-zero series — status:"parsed" alone doesn't
                // catch that. Every parsed curve must show *some* movement.
                const hasNonZeroValue = series.data.some((point) => point.y !== 0);
                expect(
                    hasNonZeroValue,
                    `artifact '${caseKey}' series '${series.curveKey}' is all-zero — likely a dead-well/no-flow deck bug`,
                ).toBe(true);
            }
        }
    });

    it('exposes real parsed series as chart curves for scenarios with a parsed artifact', () => {
        expect(getOpmFlowArtifactsForScenario('wf_bl1d')).toHaveLength(1);
        const series = getOpmFlowPublishedReferenceSeries('wf_bl1d');
        expect(series.length).toBeGreaterThan(0);
        for (const s of series) {
            expect(s.sourceType).toBe('opm-flow-precomputed');
            expect(s.sourceArtifactKey).toBe('wf_bl1d');
        }
    });

    it('every scenario opmFlowReferenceArtifactKeys entry resolves to an artifact whose scenarioKey matches the owning scenario', () => {
        for (const scenario of listScenarios()) {
            const keys = scenario.opmFlowReferenceArtifactKeys ?? [];
            // Prerun-artifacts scenarios (E7) intentionally reuse an artifact owned
            // by a different (live) scenario's deck — the artifact IS the exhibit —
            // so the scenarioKey-match invariant only applies to live-worker scenarios.
            const isPrerun = scenario.capabilities.runMode === 'prerun-artifacts';
            for (const caseKey of keys) {
                const artifact = listOpmFlowArtifacts().find((a) => a.caseKey === caseKey);
                expect(
                    artifact,
                    `scenario '${scenario.key}' declares opmFlowReferenceArtifactKeys '${caseKey}' with no matching bundled artifact`,
                ).toBeDefined();
                if (isPrerun) continue;
                expect(
                    artifact!.scenarioKey,
                    `artifact '${caseKey}' has scenarioKey '${artifact!.scenarioKey}', expected '${scenario.key}'`,
                ).toBe(scenario.key);
            }
        }
    });

    it('every artifact scenarioKey resolves to a registered scenario', () => {
        for (const artifact of listOpmFlowArtifacts()) {
            expect(
                getScenario(artifact.scenarioKey),
                `artifact '${artifact.caseKey}' references unknown scenarioKey '${artifact.scenarioKey}'`,
            ).not.toBeNull();
        }
    });
});
