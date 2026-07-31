import { describe, expect, it } from 'vitest';
import { listScenarios, getScenarioWithVariantParams } from '../catalog/scenarios';
import { listDeclaredOpmFlowArtifactKeys, listOpmFlowArtifacts } from '../catalog/opmFlowArtifacts';
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
    it('routes every scenario through its declared default solver', () => {
        for (const scenario of listScenarios()) {
            const expectedFim = scenario.solverPolicy.defaultSolver === 'fim';
            expect(scenario.params.fimEnabled, scenario.key).toBe(expectedFim);
        }
    });

    it('runs the scenario-owned FIM-vs-IMPES comparison inside the 1D waterflood', () => {
        // Folded in from the standalone solver_fim_impes scenario 2026-07-31:
        // as a wf_bl1d dimension the four variants are judged against the
        // timestep-independent Buckley-Leverett reference rather than only
        // against each other.
        const scenario = listScenarios().find((candidate) => candidate.key === 'wf_bl1d');
        const dimension = scenario?.sensitivities.find((candidate) => candidate.variesSolver);
        expect(dimension?.variants.map((variant) => [
            variant.label,
            variant.paramPatch.fimEnabled ?? false,
            variant.paramPatch.delta_t_days ?? scenario?.params.delta_t_days,
        ])).toEqual([
            ['IMPES · 0.25-day steps', false, 0.25],
            ['FIM · 0.25-day steps', true, 0.25],
            ['IMPES · 5-day steps', false, 5],
            ['FIM · 5-day steps', true, 5],
        ]);
    });

    it('keeps each scenario default solver across ordinary sensitivities', () => {
        for (const scenario of listScenarios()) {
            const expectedFim = scenario.solverPolicy.defaultSolver === 'fim';
            for (const dimension of scenario.sensitivities) {
                if (dimension.variesSolver) continue;
                for (const variant of dimension.variants) {
                    const params = getScenarioWithVariantParams(scenario.key, dimension.key, variant.key);
                    expect(params.fimEnabled, `${scenario.key}/${dimension.key}/${variant.key}`).toBe(expectedFim);
                }
            }
        }
    });

    it('keeps solver names only when the sensitivity names them', () => {
        const specs = buildScenarioRunSpecs({
            scenarioKey: 'wf_bl1d',
            dimensionKey: 'solver_formulation',
            variantKeys: ['solver_impes_base', 'solver_fim_coarse'],
        });
        expect(specs.map((spec) => [spec.solver, spec.variantLabel, spec.label])).toEqual([
            ['impes', 'IMPES · 0.25-day steps', '1D Waterflood — IMPES · 0.25-day steps'],
            ['fim', 'FIM · 5-day steps', '1D Waterflood — FIM · 5-day steps'],
        ]);
    });

    it('does not append the default solver to ordinary sensitivity labels', () => {
        const { scenario, dimension, variant } = firstDimensionAndVariant('wf_bl1d');
        expect(dimension?.variesSolver).not.toBe(true);

        const [spec] = buildScenarioRunSpecs({
            scenarioKey: scenario!.key,
            dimensionKey: dimension!.key,
            variantKeys: [variant!.key],
        });

        expect(spec.variantLabel).toBe(variant!.label);
        expect(spec.label).toBe(`${scenario!.label} — ${variant!.label}`);
        expect(spec.label).not.toMatch(/\[(?:FIM|IMPES)\]$| · (?:FIM|IMPES)$/);
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
            expect(payload.fimEnabled, scenario.key)
                .toBe(scenario.solverPolicy.defaultSolver === 'fim');
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
            for (const artifactKey of listDeclaredOpmFlowArtifactKeys(scenario.referenceSources)) {
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
