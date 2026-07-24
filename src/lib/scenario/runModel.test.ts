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
    it('routes gas scenarios to FIM and oil/water scenarios to IMPES by default', () => {
        for (const scenario of listScenarios()) {
            const expectedFim = scenario.capabilities.requiresThreePhaseMode;
            expect(scenario.params.fimEnabled, scenario.key).toBe(expectedFim);
            expect(scenario.solverPolicy.defaultSolver, scenario.key).toBe(expectedFim ? 'fim' : 'impes');
        }
    });

    it('adds an explicit FIM-vs-IMPES sensitivity to every oil/water scenario only', () => {
        for (const scenario of listScenarios()) {
            const dimension = scenario.sensitivities.find((candidate) => candidate.key === 'solver_comparison');
            if (scenario.capabilities.requiresThreePhaseMode) {
                expect(dimension, scenario.key).toBeUndefined();
                continue;
            }
            expect(dimension?.variants.map((variant) => [variant.label, variant.paramPatch.fimEnabled]), scenario.key).toEqual([
                ['IMPES', false],
                ['FIM', true],
            ]);
        }
    });

    it('keeps each scenario default solver across ordinary sensitivities', () => {
        for (const scenario of listScenarios()) {
            const expectedFim = scenario.solverPolicy.defaultSolver === 'fim';
            for (const dimension of scenario.sensitivities) {
                if (dimension.key === 'solver_comparison') continue;
                for (const variant of dimension.variants) {
                    const params = getScenarioWithVariantParams(scenario.key, dimension.key, variant.key);
                    expect(params.fimEnabled, `${scenario.key}/${dimension.key}/${variant.key}`).toBe(expectedFim);
                }
            }
        }
    });

    it('includes the numerical solver in scenario run metadata', () => {
        const specs = buildScenarioRunSpecs({
            scenarioKey: 'wf_bl1d',
            dimensionKey: 'solver_comparison',
            variantKeys: ['solver_impes', 'solver_fim'],
        });
        expect(specs.map((spec) => [spec.solver, spec.variantLabel, spec.label])).toEqual([
            ['impes', 'IMPES', '1D Waterflood — IMPES [IMPES]'],
            ['fim', 'FIM', '1D Waterflood — FIM [FIM]'],
        ]);
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
            expect(payload.fimEnabled, scenario.key).toBe(scenario.capabilities.requiresThreePhaseMode);
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
            // Prerun-artifacts scenarios (E7) reuse another scenario's bundled
            // artifact by caseKey, so the same-scenario invariant is skipped for them.
            const isPrerun = scenario.capabilities.runMode === 'prerun-artifacts';
            for (const artifactKey of scenario.opmFlowReferenceArtifactKeys ?? []) {
                const artifact = artifactsByCase.get(artifactKey);
                expect(artifact, `${scenario.key}:${artifactKey}`).toBeTruthy();
                expect(artifact?.sourceType).toBe('opm-flow-precomputed');
                if (isPrerun) continue;
                expect(artifact?.scenarioKey).toBe(scenario.key);
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
