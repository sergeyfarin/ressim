import { describe, expect, it } from 'vitest';
import {
    getOpmFlowArtifactsForScenario,
    listDeclaredOpmFlowArtifactKeys,
    resolveScenarioReferenceSeries,
    listOpmFlowArtifacts,
} from './opmFlowArtifacts';
import { getScenario, listScenarios } from './scenarios';

// Case keys that have been confirmed to reach `status: "parsed"` with real
// Flow-run series (see docs/FRONTEND_EXECUTION_PLAN_2026-07.md Wave 1). This
// list must only grow — a committed artifact regressing from `parsed` back
// to `deck-ready`/`flow-run`/`error` means someone regenerated it without a
// real Flow run behind it, which is exactly the silent-stub failure mode
// this pipeline used to have.
const PARSED_BASELINE = ['wf_bl1d', 'spe1_gas_injection', 'gas_drive', 'wf_gravity', 'wf_numerics', 'wf_numerics_fine'];

describe('OPM Flow precomputed artifacts', () => {
    it('ships explicit metadata for predefined OPM Flow artifact targets', () => {
        const artifacts = listOpmFlowArtifacts();

        expect(artifacts.map((artifact) => artifact.scenarioKey).sort()).toEqual([
            'gas_drive',
            'spe1_gas_injection',
            'wf_bl1d',
            'wf_gravity',
            'wf_numerics',
            'wf_numerics',
        ]);
        for (const artifact of artifacts) {
            expect(artifact.schemaVersion).toBe(1);
            expect(artifact.sourceType).toBe('opm-flow-precomputed');
            expect(artifact.deckHash.length).toBeGreaterThan(8);
            expect(artifact.units.time).toBe('days');
            expect(artifact.supportedCurves.length).toBeGreaterThan(0);
        }
    });

    it('publishes a time to PVI mapping wherever the case injects', () => {
        const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === 'wf_gravity');
        const xAxis = artifact?.xAxis;

        expect(xAxis).toBeDefined();
        // Reservoir-volume injection, not surface: PVI is a reservoir quantity
        // and the two differ by Bw.
        expect(xAxis!.cumulativeInjectionCurve).toBe('FVIT');
        expect(xAxis!.timeDays).toHaveLength(artifact!.series[0].data.length);
        expect(xAxis!.pvi).toHaveLength(xAxis!.timeDays.length);
        expect(xAxis!.cumulativeInjectionM3).toHaveLength(xAxis!.timeDays.length);
        // Monotone, as a cumulative injection must be.
        expect(xAxis!.pvi.every((value, index) => index === 0 || value >= xAxis!.pvi[index - 1])).toBe(true);

        // Cross-check of the deck's declared pore volume against the ResSim
        // scenario it mirrors: same grid, same injection rate, same schedule,
        // so the artifact's final PVI must be the scenario's own.
        const params = getScenario('wf_gravity')!.params as Record<string, number>;
        const poreVolume = params.nx * params.cellDx * params.ny * params.cellDy
            * params.nz * params.cellDz * params.reservoirPorosity;
        expect(xAxis!.poreVolumeM3).toBe(poreVolume);
        const scenarioPvi = (params.steps * params.delta_t_days * params.targetInjectorRate) / poreVolume;
        expect(xAxis!.pvi.at(-1)).toBeCloseTo(scenarioPvi, 2);
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

    it('resolves a declared opm-flow source into stamped chart curves', () => {
        expect(getOpmFlowArtifactsForScenario('wf_bl1d')).toHaveLength(1);
        const series = resolveScenarioReferenceSeries([{ kind: 'opm-flow', artifactKeys: ['wf_bl1d'] }]);
        expect(series.length).toBeGreaterThan(0);
        for (const s of series) {
            expect(s.sourceType).toBe('opm-flow-precomputed');
            expect(s.sourceArtifactKey).toBe('wf_bl1d');
            // Overlay role leaves `primary` absent so the chart's `=== true` test reads false.
            expect(s.primary).toBeUndefined();
        }
    });

    it('stamps primary only on the primary role', () => {
        const primary = resolveScenarioReferenceSeries([
            { kind: 'opm-flow', artifactKeys: ['wf_bl1d'], role: 'primary' },
        ]);
        expect(primary.length).toBeGreaterThan(0);
        for (const s of primary) expect(s.primary).toBe(true);
    });

    it('resolves sources in declaration order and passes published series through untouched', () => {
        const published = { panelKey: 'rates', label: 'Paper', curveKey: 'published-x', data: [{ x: 0, y: 1 }] };
        const series = resolveScenarioReferenceSeries([
            { kind: 'published', series: [published] },
            { kind: 'opm-flow', artifactKeys: ['wf_bl1d'] },
        ]);
        expect(series[0]).toEqual(published);
        expect(series[1].sourceType).toBe('opm-flow-precomputed');
    });

    it('never resolves an artifact a scenario did not declare', () => {
        // The old path matched any artifact whose scenarioKey equalled the
        // scenario key, so a chart could gain curves no scenario file mentioned.
        expect(resolveScenarioReferenceSeries([])).toEqual([]);
        expect(resolveScenarioReferenceSeries(undefined)).toEqual([]);
        expect(resolveScenarioReferenceSeries([{ kind: 'opm-flow', artifactKeys: ['not-an-artifact'] }])).toEqual([]);
    });

    it("every scenario's declared opm-flow artifact keys resolve to an artifact owned by that scenario", () => {
        for (const scenario of listScenarios()) {
            const keys = listDeclaredOpmFlowArtifactKeys(scenario.referenceSources);
            // Prerun-artifacts scenarios (E7) intentionally reuse an artifact owned
            // by a different (live) scenario's deck — the artifact IS the exhibit —
            // so the scenarioKey-match invariant only applies to live-worker scenarios.
            const isPrerun = scenario.capabilities.runMode === 'prerun-artifacts';
            for (const caseKey of keys) {
                const artifact = listOpmFlowArtifacts().find((a) => a.caseKey === caseKey);
                expect(
                    artifact,
                    `scenario '${scenario.key}' declares opm-flow artifactKey '${caseKey}' with no matching bundled artifact`,
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
