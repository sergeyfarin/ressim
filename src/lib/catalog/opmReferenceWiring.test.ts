import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { mapReferenceTimesToXAxis } from '@ressim/charts/axisAdapters';
import { getScenario, getScenarioChartLayout, listScenarios } from './scenarios';
import {
    listDeclaredOpmFlowArtifactKeys,
    listOpmFlowArtifacts,
    resolveScenarioReferenceSeries,
} from './opmFlowArtifacts';

/**
 * Where each scenario's OPM Flow reference lands, checked from the scenario side (#20).
 *
 * `tools/opm_flow` checks the artifact against the summary it came from: units, phases,
 * provenance. This file checks it against the chart that draws it: the curve is on a panel the
 * scenario's layout has, it says which run it is, and it can be placed on the axis the chart
 * opens on. A reference that is silently dropped on the default axis is as absent as one never
 * declared.
 */

const REPO_ROOT = new URL('../../../', import.meta.url);

/** The OPM curves each scenario is meant to show, by curve key. */
const EXPECTED: Record<string, { artifacts: string[]; curveKeys: string[]; variantLabel?: string }> = {
    wf_bl1d: {
        artifacts: ['wf_bl1d'],
        curveKeys: ['opm-water-cut', 'opm-oil-rate', 'opm-cum-oil', 'opm-avg-pressure', 'opm-cum-injection'],
        variantLabel: 'base',
    },
    gas_injection: {
        artifacts: ['gas_injection'],
        curveKeys: ['opm-oil-rate', 'opm-cum-oil', 'opm-avg-pressure', 'opm-cum-injection'],
        variantLabel: 'base',
    },
    wf_capillary: {
        artifacts: ['wf_capillary'],
        curveKeys: ['opm-water-cut', 'opm-oil-rate', 'opm-cum-oil', 'opm-avg-pressure', 'opm-cum-injection'],
        variantLabel: 'base',
    },
    wf_gravity_stability: {
        artifacts: ['wf_gravity_stability'],
        curveKeys: ['opm-water-cut', 'opm-oil-rate', 'opm-cum-oil', 'opm-avg-pressure', 'opm-cum-injection'],
        variantLabel: 'base',
    },
    // The sweep layout has no oil-rate or cumulative-injection panel.
    sweep_areal: {
        artifacts: ['sweep_areal'],
        curveKeys: ['opm-water-cut', 'opm-cum-oil', 'opm-avg-pressure'],
        variantLabel: 'base',
    },
    sweep_vertical: {
        artifacts: ['sweep_vertical'],
        curveKeys: ['opm-water-cut', 'opm-cum-oil', 'opm-avg-pressure'],
        variantLabel: 'base',
    },
    sweep_crossflow: {
        artifacts: ['sweep_crossflow'],
        curveKeys: ['opm-water-cut', 'opm-cum-oil', 'opm-avg-pressure'],
        variantLabel: 'base',
    },
    // Its own parameters are no variant it runs; the Flow run is the one both dimensions draw.
    sweep_combined: {
        artifacts: ['sweep_combined'],
        curveKeys: ['opm-water-cut', 'opm-cum-oil', 'opm-avg-pressure'],
        variantLabel: 'Favorable + layered',
    },
    dep_pvt: {
        artifacts: ['dep_pvt_correlation', 'dep_pvt_lab_report'],
        curveKeys: [
            'opm-avg-pressure', 'opm-gor', 'opm-gas-rate', 'opm-cum-gas',
            'opm-lab-avg-pressure', 'opm-lab-gor', 'opm-lab-gas-rate', 'opm-lab-cum-gas',
        ],
    },
};

/**
 * Every scenario that can show an OPM reference: the picker's, plus withheld ones (`dep_pvt`),
 * which stay resolvable by key and keep their references.
 */
function scenariosWithReferences() {
    const keys = new Set([
        ...listScenarios().map((scenario) => scenario.key),
        ...listOpmFlowArtifacts().map((artifact) => artifact.scenarioKey),
    ]);
    return [...keys].map((key) => getScenario(key)!);
}

function panelsOf(scenarioKey: string): Set<string> {
    const chart = getScenarioChartLayout(getScenario(scenarioKey)!).chart ?? {};
    return new Set([...(chart.panelOrder ?? []), ...Object.keys(chart.panels ?? {})]);
}

describe('OPM Flow references on their scenarios (#20)', () => {
    it.each(Object.entries(EXPECTED))('%s shows the intended reference, correctly labelled', (scenarioKey, expected) => {
        const scenario = getScenario(scenarioKey)!;
        expect(listDeclaredOpmFlowArtifactKeys(scenario.referenceSources)).toEqual(expected.artifacts);

        const series = resolveScenarioReferenceSeries(scenario.referenceSources, scenario.params);
        expect(series.map((curve) => curve.curveKey)).toEqual(expected.curveKeys);
        for (const curve of series) {
            expect(curve.label.startsWith('OPM Flow — '), curve.label).toBe(true);
            expect(curve.sourceType).toBe('opm-flow-precomputed');
            expect(curve.variantLabel).toBe(expected.variantLabel);
        }
    });

    it('labels the two dep_pvt runs by the rung each one is', () => {
        const scenario = getScenario('dep_pvt')!;
        const series = resolveScenarioReferenceSeries(scenario.referenceSources, scenario.params);
        for (const curve of series) {
            const rung = curve.sourceArtifactKey === 'dep_pvt_lab_report' ? 'c_o = 2.5e-4' : 'c_o = 1.0e-4';
            expect(curve.label, curve.curveKey).toContain(`(${rung})`);
        }
        // Distinct legend toggles, since neither run carries a variant suffix.
        expect(new Set(series.map((curve) => curve.sourceArtifactLabel)).size).toBe(2);
    });

    it('every declared reference lands on a panel its scenario draws', () => {
        for (const scenario of scenariosWithReferences()) {
            const panels = panelsOf(scenario.key);
            for (const curve of resolveScenarioReferenceSeries(scenario.referenceSources, scenario.params)) {
                expect(panels.has(curve.panelKey), `${scenario.key}: ${curve.curveKey} on missing panel '${curve.panelKey}'`)
                    .toBe(true);
            }
        }
    });

    it('every declared reference can be placed on the axis its chart opens on', () => {
        for (const scenario of scenariosWithReferences()) {
            const mode = getScenarioChartLayout(scenario).chart?.xAxisMode ?? 'time';
            for (const curve of resolveScenarioReferenceSeries(scenario.referenceSources, scenario.params)) {
                const times = curve.data.map((point) => point.x);
                const mapped = mapReferenceTimesToXAxis(times, mode, curve.xAxisMap);
                expect(mapped, `${scenario.key}: ${curve.curveKey} is dropped on its default '${mode}' axis`).not.toBeNull();
            }
        }
    });

    it('drops a reference on an axis its run cannot place it on, instead of misplacing it', () => {
        const scenario = getScenario('gas_injection')!;
        const [curve] = resolveScenarioReferenceSeries(scenario.referenceSources, scenario.params);
        const times = curve.data.map((point) => point.x);
        // Produced gas and dimensionless time: the Flow run publishes no mapping for either.
        expect(mapReferenceTimesToXAxis(times, 'cumGas', curve.xAxisMap)).toBeNull();
        expect(mapReferenceTimesToXAxis(times, 'tD', curve.xAxisMap)).toBeNull();
        // The cumulative-injection axis is surface volume and uses FGIT, not the FVIT behind PVI.
        expect(curve.xAxisMap?.cumulativeInjectionSm3?.at(-1)).toBeCloseTo(49351.664, 2);
        const pvi = mapReferenceTimesToXAxis(times, 'pvi', curve.xAxisMap)!;
        expect(pvi.at(-1)).toBeGreaterThan(0.4);
        expect(pvi.at(-1)).toBeLessThan(0.6);
    });
});

describe('committed OPM artifacts (#20)', () => {
    it('are every one declared by the scenario they were run for', () => {
        for (const artifact of listOpmFlowArtifacts()) {
            const declared = listDeclaredOpmFlowArtifactKeys(getScenario(artifact.scenarioKey)?.referenceSources);
            expect(declared, `artifact '${artifact.caseKey}' is bundled but '${artifact.scenarioKey}' does not show it`)
                .toContain(artifact.caseKey);
        }
    });

    it('are parsed, versioned and source-attributed', () => {
        for (const artifact of listOpmFlowArtifacts()) {
            expect(artifact.status, artifact.caseKey).toBe('parsed');
            expect(artifact.schemaVersion).toBe(2);
            expect(artifact.flowVersion, artifact.caseKey).toMatch(/^flow \d{4}\.\d{2}/);
            expect(artifact.provenance.deckSource, artifact.caseKey).toBeTruthy();
            expect(artifact.provenance.replay, artifact.caseKey).toContain(`run-flow ${artifact.caseKey}`);
            expect(artifact.provenance.origin, artifact.caseKey).toContain('Distributed under the repository licence');
            for (const series of artifact.series) {
                expect(series.mnemonic, `${artifact.caseKey}/${series.curveKey}`).toBeTruthy();
                expect(typeof series.unit).toBe('string');
            }
        }
    });

    it('were generated from the committed deck they name', () => {
        // Decks held inline in tools/opm_flow are hashed by its own test suite; a committed deck
        // file can be hashed here, so an edit that is not followed by a Flow run fails in CI.
        const fileBacked = listOpmFlowArtifacts().filter((artifact) => artifact.provenance.deckSource.endsWith('.DATA'));
        expect(fileBacked.map((artifact) => artifact.caseKey).sort())
            .toEqual([
                'dep_pvt_correlation', 'dep_pvt_lab_report', 'gas_drive', 'gas_injection',
                'sweep_areal', 'sweep_combined', 'sweep_crossflow', 'sweep_vertical',
                'wf_capillary', 'wf_gravity_stability',
            ]);
        for (const artifact of fileBacked) {
            const deck = readFileSync(new URL(artifact.provenance.deckSource, REPO_ROOT));
            expect(createHash('sha256').update(deck).digest('hex'), artifact.caseKey).toBe(artifact.deckHash);
        }
    });

    it('put one unit on each panel of a layout', () => {
        // A water rate on a water-cut panel, or a reservoir volume beside surface volumes, shows
        // up as two units on one axis.
        const units = new Map<string, Set<string>>();
        for (const scenario of scenariosWithReferences()) {
            const layout = getScenarioChartLayout(scenario);
            for (const key of listDeclaredOpmFlowArtifactKeys(scenario.referenceSources)) {
                const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === key)!;
                for (const series of artifact.series) {
                    const slot = `${scenario.chartLayoutKey}/${series.panelKey}`;
                    if (!units.has(slot)) units.set(slot, new Set());
                    units.get(slot)!.add(series.unit);
                }
            }
            expect(layout).toBeDefined();
        }
        for (const [slot, found] of units) {
            expect([...found], slot).toHaveLength(1);
        }
    });
});
