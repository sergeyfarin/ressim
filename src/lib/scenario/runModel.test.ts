import { describe, expect, it } from 'vitest';
import { listScenarios, getScenarioWithVariantParams } from '../catalog/scenarios';
import { listOpmFlowArtifacts } from '../catalog/opmFlowArtifacts';
import { buildCreatePayloadForRun, buildScenarioRunSpecs } from './runModel';

function firstDimensionAndVariant(scenarioKey: string) {
    const scenario = listScenarios().find((candidate) => candidate.key === scenarioKey);
    const dimension = scenario?.sensitivities[0] ?? null;
    const variant = dimension?.variants[0] ?? null;
    return { scenario, dimension, variant };
}

function assertWellBounds(params: Record<string, unknown>) {
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const injectorI = Number(params.injectorI ?? 0);
    const injectorJ = Number(params.injectorJ ?? 0);
    const producerI = Number(params.producerI ?? 0);
    const producerJ = Number(params.producerJ ?? 0);

    expect(Number.isFinite(nx) && nx > 0).toBe(true);
    expect(Number.isFinite(ny) && ny > 0).toBe(true);
    expect(injectorI).toBeGreaterThanOrEqual(0);
    expect(injectorI).toBeLessThan(nx);
    expect(injectorJ).toBeGreaterThanOrEqual(0);
    expect(injectorJ).toBeLessThan(ny);
    expect(producerI).toBeGreaterThanOrEqual(0);
    expect(producerI).toBeLessThan(nx);
    expect(producerJ).toBeGreaterThanOrEqual(0);
    expect(producerJ).toBeLessThan(ny);
}

describe('scenario-first run model', () => {
    it('keeps every predefined scenario on the IMPES product path by default', () => {
        for (const scenario of listScenarios()) {
            expect(scenario.params.fimEnabled, scenario.key).toBe(false);
        }
    });

    it('builds scenario-native run specs and simulator payloads for each predefined scenario', () => {
        for (const scenario of listScenarios()) {
            const dimension = scenario.sensitivities[0];
            if (!dimension) continue;
            const variant = dimension.variants[0];
            const specs = buildScenarioRunSpecs({
                scenarioKey: scenario.key,
                dimensionKey: dimension.key,
                variantKeys: [variant.key],
            });

            expect(specs, scenario.key).toHaveLength(1);
            expect(specs[0]).toMatchObject({
                caseKey: scenario.key,
                familyKey: scenario.key,
                variantKey: variant.key,
                referenceSource: expect.objectContaining({ source: expect.any(String) }),
            });
            const payload = buildCreatePayloadForRun(specs[0]);
            expect(payload.fimEnabled, scenario.key).toBe(false);
            expect(payload.nx, scenario.key).toBeGreaterThan(0);
            expect(payload.ny, scenario.key).toBeGreaterThan(0);
            expect(payload.nz, scenario.key).toBeGreaterThan(0);
        }
    });

    it('keeps base and variant well locations inside grid bounds', () => {
        for (const scenario of listScenarios()) {
            assertWellBounds(scenario.params);
            for (const dimension of scenario.sensitivities) {
                for (const variant of dimension.variants) {
                    assertWellBounds(getScenarioWithVariantParams(scenario.key, dimension.key, variant.key));
                }
            }
        }
    });

    it('links declared OPM Flow artifact keys to tracked artifacts for the same scenario', () => {
        const artifactsByCase = new Map(listOpmFlowArtifacts().map((artifact) => [artifact.caseKey, artifact]));
        for (const scenario of listScenarios()) {
            for (const artifactKey of scenario.opmFlowReferenceArtifactKeys ?? []) {
                const artifact = artifactsByCase.get(artifactKey);
                expect(artifact, `${scenario.key}:${artifactKey}`).toBeTruthy();
                expect(artifact?.scenarioKey).toBe(scenario.key);
                expect(artifact?.sourceType).toBe('opm-flow-precomputed');
            }
        }
    });

    it('can build an overridden run policy without mutating scenario params', () => {
        const { scenario, dimension, variant } = firstDimensionAndVariant('wf_bl1d');
        expect(scenario && dimension && variant).toBeTruthy();
        const specs = buildScenarioRunSpecs({
            scenarioKey: scenario!.key,
            dimensionKey: dimension!.key,
            variantKeys: [variant!.key],
            stepsOverride: 7,
            deltaTDaysOverride: 0.5,
        });

        expect(specs[0].steps).toBe(7);
        expect(specs[0].deltaTDays).toBe(0.5);
        expect(scenario!.params.steps).not.toBe(7);
    });
});
