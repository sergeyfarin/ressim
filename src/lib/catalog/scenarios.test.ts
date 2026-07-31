import { describe, expect, it } from 'vitest';
import { getAnalyticalMethodDescriptor } from '../charts/analyticalMethodRegistry';
import { listDeclaredOpmFlowArtifactKeys } from './opmFlowArtifacts';
import { calculateAnalyticalProduction } from '../analytical/fractionalFlow';
import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { computeCombinedSweep } from '../analytical/sweepEfficiency';
import { computeWellTestOnTimeAxis } from '../charts/analyticalParamAdapters';
import type { RockProps, FluidProps } from '../analytical/fractionalFlow';
import {
    getScenario,
    getScenarioAnalyticalOptions,
    getScenarioChartLayout,
    getScenarioWithVariantParams,
    getScenarioGroup,
    SCENARIO_GROUPS,
    listScenarios,
    resolveCapabilities,
    validateScenarioChartLayout,
    validateScenarioCapabilities,
    type ScenarioCapabilities,
    ANALYTICAL_OUTPUT_CONTRACTS,
    type AnalyticalMethod,
    CHART_LAYOUTS,
} from './scenarios';

describe('scenario sensitivities', () => {
    it('uses each scenario\'s declared solver policy without injecting sensitivities', () => {
        for (const scenario of listScenarios()) {
            expect(scenario.params.fimEnabled, scenario.key)
                .toBe(scenario.solverPolicy.defaultSolver === 'fim');
            expect(scenario.solverPolicy.rationale.trim().length, scenario.key).toBeGreaterThan(20);
        }

        const solverDimensions = listScenarios().flatMap((scenario) =>
            scenario.sensitivities
                .filter((dimension) => dimension.variesSolver)
                .map((dimension) => [scenario.key, dimension.key]),
        );
        expect(solverDimensions).toEqual([['wf_bl1d', 'solver_formulation']]);
    });

    it('provides analytical method metadata for every canonical scenario', () => {
        expect(listScenarios()).toHaveLength(15);
        for (const scenario of listScenarios()) {
            expect(scenario.analyticalMethodSummary.length, scenario.key).toBeGreaterThan(10);
            expect(scenario.analyticalMethodReference.length, scenario.key).toBeGreaterThan(5);
        }
    });

    it('adds a seeded areal heterogeneity axis for the areal sweep scenario', () => {
        const scenario = getScenario('sweep_areal');

        expect(scenario?.defaultSensitivityDimensionKey).toBe('mobility');
        expect(scenario?.terminationPolicy).toEqual({
            mode: 'any',
            conditions: [
                {
                    kind: 'watercut-threshold',
                    value: 0.98,
                    scope: 'producer',
                },
            ],
        });
        expect(scenario?.sensitivities.map((dimension) => dimension.key)).toEqual([
            'mobility',
            'areal_heterogeneity',
            'sor',
            'grid_resolution',
        ]);

        const arealAxis = scenario?.sensitivities.find((dimension) => dimension.key === 'areal_heterogeneity');
        expect(arealAxis?.variants.map((variant) => variant.key)).toEqual([
            'areal_uniform',
            'areal_mild_random',
            'areal_strong_random',
        ]);
        expect(arealAxis?.variants.every((variant) => variant.affectsAnalytical === false)).toBe(true);

        const randomParams = getScenarioWithVariantParams('sweep_areal', 'areal_heterogeneity', 'areal_mild_random');
        expect(randomParams).toMatchObject({
            permMode: 'random',
            minPerm: 50,
            maxPerm: 500,
            useRandomSeed: true,
            randomSeed: 4201,
        });
    });

    it('treats vertical heterogeneity variants as analytically varying sweep overlays', () => {
        const scenario = getScenario('sweep_vertical');
        const heterogeneityAxis = scenario?.sensitivities.find((dimension) => dimension.key === 'heterogeneity');

        expect(heterogeneityAxis?.variants.map((variant) => variant.affectsAnalytical)).toEqual([
            true,
            true,
            true,
        ]);
    });

    it('uses a coarser combined-sweep timestep while keeping a substantial run horizon', () => {
        const vertical = getScenario('sweep_vertical');
        const combined = getScenario('sweep_combined');

        expect(Number(combined?.params.delta_t_days ?? 0)).toBeGreaterThan(Number(vertical?.params.delta_t_days ?? 0));
        expect(Number(combined?.params.steps ?? 0)).toBeGreaterThan(0);
    });

    it('exposes Stiles and Dykstra-Parsons analytical options for the combined sweep scenario', () => {
        const scenario = getScenario('sweep_combined');

        expect(getScenarioAnalyticalOptions(scenario).map(
            (option) => [option.key, option.sweepMethod, option.default ?? false],
        )).toEqual([
            ['stiles', 'stiles', true],
            ['dykstra', 'dykstra-parsons', false],
        ]);
    });

    it('splits the combined sweep scenario into interaction and ideal-to-worst axes', () => {
        const scenario = getScenario('sweep_combined');

        expect(scenario?.defaultSensitivityDimensionKey).toBe('interaction_core');
        expect(scenario?.sensitivities.map((dimension) => dimension.key)).toEqual([
            'interaction_core',
            'sweep_ladder',
        ]);

        const interactionAxis = scenario?.sensitivities.find((dimension) => dimension.key === 'interaction_core');
        expect(interactionAxis?.variants.map((variant) => variant.key)).toEqual([
            'interaction_favorable_uniform',
            'interaction_unfavorable_uniform',
            'interaction_favorable_layered',
            'interaction_unfavorable_layered',
        ]);
        expect(interactionAxis?.variants.every((variant) => variant.affectsAnalytical)).toBe(true);

        const ladderAxis = scenario?.sensitivities.find((dimension) => dimension.key === 'sweep_ladder');
        expect(ladderAxis?.variants.map((variant) => variant.key)).toEqual([
            'ladder_ideal',
            'ladder_vertical',
            'ladder_full_het',
            'ladder_worst',
        ]);
        expect(ladderAxis?.variants.every((variant) => variant.affectsAnalytical === false)).toBe(true);

        const interactionParams = getScenarioWithVariantParams(
            'sweep_combined',
            'interaction_core',
            'interaction_unfavorable_uniform',
        );
        expect(interactionParams).toMatchObject({
            mu_o: 5,
            permMode: 'uniform',
        });

        const ladderParams = getScenarioWithVariantParams(
            'sweep_combined',
            'sweep_ladder',
            'ladder_full_het',
        );
        expect(ladderParams).toMatchObject({
            permMode: 'random',
            minPerm: 40,
            maxPerm: 500,
            useRandomSeed: true,
            randomSeed: 4301,
        });
    });
});

// ────────────────────────────────────────────────────────────────────────────
// Contract: affectsAnalytical accuracy
// ────────────────────────────────────────────────────────────────────────────

function toNum(v: unknown, fallback: number): number {
    const n = Number(v);
    return Number.isFinite(n) ? n : fallback;
}

function extractRock(p: Record<string, unknown>): RockProps {
    return {
        s_wc: toNum(p.s_wc, 0.1),
        s_or: toNum(p.s_or, 0.1),
        n_w: toNum(p.n_w, 2),
        n_o: toNum(p.n_o, 2),
        k_rw_max: toNum(p.k_rw_max, 1),
        k_ro_max: toNum(p.k_ro_max, 1),
    };
}

function extractFluid(p: Record<string, unknown>): FluidProps {
    return {
        mu_w: toNum(p.mu_w, 0.5),
        mu_o: toNum(p.mu_o, 1),
    };
}

function getLayerPerms(p: Record<string, unknown>): number[] {
    const nz = toNum(p.nz, 1);
    if (String(p.permMode) === 'perLayer' && Array.isArray(p.layerPermsX) && p.layerPermsX.length > 1) {
        return p.layerPermsX.map(Number);
    }
    return Array.from({ length: nz }, () => toNum(p.uniformPermX, 100));
}

/** Run the BL analytical and return a fingerprint array (recovery at fixed PVI points). */
function blFingerprint(params: Record<string, unknown>): number[] {
    const pvi = [0.1, 0.5, 1.0, 2.0];
    const inj = new Array(pvi.length).fill(1);
    const prod = calculateAnalyticalProduction(
        extractRock(params), extractFluid(params),
        toNum(params.initialSaturation, toNum(params.s_wc, 0.1)),
        pvi, inj, 1,
    );
    return prod.map((pt) => pt.cumulativeOil);
}

/** Run the sweep analytical and return a fingerprint. */
function sweepFingerprint(params: Record<string, unknown>): number[] {
    const result = computeCombinedSweep(
        extractRock(params), extractFluid(params),
        getLayerPerms(params), toNum(params.cellDz, 1), 3.0, 50,
    );
    return result.combined.filter((_, i) => i % 10 === 0).map((pt) => pt.efficiency);
}

/** Run the depletion analytical and return a fingerprint (q0, tau, and early-time rates). */
function depletionFingerprint(params: Record<string, unknown>): number[] {
    const dt = toNum(params.delta_t_days, 1);
    // Sample at very early times to capture differences before exponential decay
    const timeHistory = [dt * 0.01, dt * 0.1, dt, dt * 5, dt * 20];
    const result = calculateDepletionAnalyticalProduction({
        reservoir: {
            length: toNum(params.nx, 1) * toNum(params.cellDx, 10),
            area: toNum(params.ny, 1) * toNum(params.cellDy, 10) * toNum(params.nz, 1) * toNum(params.cellDz, 1),
            porosity: toNum(params.reservoirPorosity ?? params.porosity, 0.2),
        },
        timeHistory,
        initialSaturation: toNum(params.initialSaturation, 0.3),
        nz: toNum(params.nz, 1),
        permMode: String(params.permMode ?? 'uniform'),
        uniformPermX: toNum(params.uniformPermX, 100),
        uniformPermY: toNum(params.uniformPermY ?? params.uniformPermX, 100),
        layerPermsX: Array.isArray(params.layerPermsX) ? params.layerPermsX.map(Number) : [],
        layerPermsY: Array.isArray(params.layerPermsY) ? params.layerPermsY.map(Number) : [],
        cellDx: toNum(params.cellDx, 10),
        cellDy: toNum(params.cellDy, 10),
        cellDz: toNum(params.cellDz, 1),
        wellRadius: toNum(params.well_radius, 0.1),
        wellSkin: toNum(params.well_skin, 0),
        muO: toNum(params.mu_o, 1),
        sWc: toNum(params.s_wc, 0.1),
        sOr: toNum(params.s_or, 0.1),
        nO: toNum(params.n_o, 2),
        c_o: toNum(params.c_o, 1e-5),
        c_w: toNum(params.c_w, 3e-6),
        cRock: toNum(params.rock_compressibility, 1e-6),
        initialPressure: toNum(params.initialPressure, 300),
        producerBhp: toNum(params.producerBhp, 100),
        depletionRateScale: toNum(params.analyticalDepletionRateScale, 1),
        arpsB: toNum(params.analyticalArpsB, 0),
        layeredComposite: params.analyticalLayeredComposite === true,
        model: params.analyticalDepletionModel === 'finite-slab' ? 'finite-slab' : 'tank',
        nx: params.nx != null ? toNum(params.nx, 1) : undefined,
        ny: params.ny != null ? toNum(params.ny, 1) : undefined,
        producerI: params.producerI != null ? toNum(params.producerI, 0) : undefined,
        producerJ: params.producerJ != null ? toNum(params.producerJ, 0) : undefined,
    });
    return [result.meta.q0 ?? 0, result.meta.tau ?? 0, ...result.production.map((pt) => pt.oilRate)];
}

function analyticalFingerprint(analyticalMethod: string, params: Record<string, unknown>): number[] {
    if (analyticalMethod === 'depletion') return depletionFingerprint(params);
    if (analyticalMethod === 'well-test') {
        const result = computeWellTestOnTimeAxis(params, [0.5, 1, 2, 5]);
        return result ? [
            ...result.oilRate.map((value) => value ?? 0),
            ...result.flowingBhp.map((value) => value ?? 0),
        ] : [];
    }
    // waterflood class covers both BL and sweep scenarios
    return [...blFingerprint(params), ...sweepFingerprint(params)];
}

function arraysEqual(a: number[], b: number[], tol = 1e-12): boolean {
    if (a.length !== b.length) return false;
    return a.every((v, i) => Math.abs(v - b[i]) < tol);
}

describe('affectsAnalytical contract', () => {
    const analyticalScenarios = ['wf_bl1d', 'sweep_areal', 'sweep_vertical', 'sweep_combined', 'dep_pss', 'dep_decline', 'dep_arps'];

    for (const scenarioKey of analyticalScenarios) {
        const scenario = getScenario(scenarioKey)!;

        for (const dim of scenario.sensitivities) {
            const baseFingerprint = analyticalFingerprint(
                scenario.capabilities.analyticalMethod,
                scenario.params as Record<string, unknown>,
            );

            for (const variant of dim.variants) {
                const isBaseCase = Object.keys(variant.paramPatch).length === 0;
                const variantParams = getScenarioWithVariantParams(scenarioKey, dim.key, variant.key);
                const variantFp = analyticalFingerprint(scenario.capabilities.analyticalMethod, variantParams);
                if (isBaseCase) {
                    // Base-case variants (empty paramPatch) produce identical output by definition
                    it(`${scenarioKey} / ${dim.key} / ${variant.key}: base-case variant produces identical analytical output`, () => {
                        expect(arraysEqual(baseFingerprint, variantFp)).toBe(true);
                    });
                } else if (variant.affectsAnalytical) {
                    // Critical contract: if marked true, the variant MUST actually change
                    // analytical output. A false positive here causes the UI to render
                    // per-variant curves that are all identical — confusing and broken.
                    it(`${scenarioKey} / ${dim.key} / ${variant.key}: affectsAnalytical=true must change analytical output`, () => {
                        const same = arraysEqual(baseFingerprint, variantFp);
                        expect(same, `variant "${variant.key}" is marked affectsAnalytical but produces identical analytical output`).toBe(false);
                    });
                }
                // Note: affectsAnalytical=false is a UI decision (show shared analytical
                // reference), not a strict invariant. Some false-flagged variants do change
                // analytical inputs (e.g. sweep_ladder patches mu_o) but intentionally
                // share a single reference curve for pedagogical clarity.
            }
        }
    }
});

describe('scenario catalog taxonomy', () => {
    it('uses explicit product groups instead of inferring navigation from physics capabilities', () => {
        expect(getScenarioGroup(getScenario('wf_bl1d')!)).toBe('buckley-leverett-displacement');
        expect(getScenarioGroup(getScenario('sweep_areal')!)).toBe('sweep-efficiency');
        expect(getScenarioGroup(getScenario('dep_pss')!)).toBe('flow-regimes-decline');
        expect(getScenarioGroup(getScenario('dep_welltest')!)).toBe('flow-regimes-decline');
        expect(getScenarioGroup(getScenario('gas_injection')!)).toBe('buckley-leverett-displacement');
        expect(getScenarioGroup(getScenario('spe1_gas_injection')!)).toBe('validation-benchmarks');
        expect(getScenarioGroup(getScenario('dep_pvt')!)).toBe('simulation-only');
        expect(getScenarioGroup(getScenario('gas_drive')!)).toBe('simulation-only');
    });

    it('gives every scenario a recognized group, role, app mode, and scenario-owned picker summary', () => {
        const groupKeys = new Set(SCENARIO_GROUPS.map((group) => group.key));
        for (const scenario of listScenarios()) {
            expect(groupKeys.has(scenario.catalog.group), scenario.key).toBe(true);
            expect(['simulation', 'interpretation', 'benchmark'], scenario.key).toContain(scenario.catalog.role);
            expect(['wf', 'dep', '3p'], scenario.key).toContain(scenario.catalog.caseMode);
            expect(scenario.catalog.parameterSummary.trim().length, scenario.key).toBeGreaterThan(0);
        }
    });
});

describe('scenario capability validation', () => {
    it('all scenarios with analytical overlays declare analytical overlay grouping on every sensitivity dimension', () => {
        for (const scenario of listScenarios()) {
            if (scenario.capabilities.analyticalMethod === 'none' || scenario.capabilities.analyticalMethod === 'digitized-reference') continue;
            for (const dimension of scenario.sensitivities) {
                expect(
                    dimension.analyticalOverlayMode,
                    `${scenario.key} / ${dimension.key} should declare analyticalOverlayMode explicitly`,
                ).toBeDefined();
            }
        }
    });

    it('sweep sensitivity dimensions declare explicit analytical overlay policies', () => {
        expect(getScenario('sweep_areal')?.sensitivities.map((dim) => [dim.key, dim.analyticalOverlayMode])).toEqual([
            ['mobility', 'per-result'],
            ['areal_heterogeneity', 'shared'],
            ['sor', 'per-result'],
            ['grid_resolution', 'shared'],
        ]);
        expect(getScenario('wf_bl1d')?.sensitivities.map((dim) => [dim.key, dim.analyticalOverlayMode])).toEqual([
            ['mobility', 'per-result'],
            ['corey_no', 'per-result'],
            ['sor', 'per-result'],
            ['grid', 'shared'],
            ['solver_formulation', 'shared'],
        ]);
        expect(getScenario('dep_pss')?.sensitivities.map((dim) => [dim.key, dim.analyticalOverlayMode])).toEqual([
            ['drainage_shape', 'per-result'],
            ['well_position', 'per-result'],
            ['skin', 'per-result'],
        ]);
        expect(getScenario('dep_decline')?.sensitivities.map((dim) => [dim.key, dim.analyticalOverlayMode])).toEqual([
            ['permeability', 'per-result'],
            ['skin', 'per-result'],
            ['timestep', 'shared'],
            ['grid_refinement', 'shared'],
        ]);
        expect(getScenario('dep_arps')?.sensitivities.map((dim) => [dim.key, dim.analyticalOverlayMode])).toEqual([
            ['layer_contrast', 'per-result'],
            ['vertical_communication', 'shared'],
        ]);
        expect(getScenario('gas_injection')?.sensitivities.map((dim) => [dim.key, dim.analyticalOverlayMode])).toEqual([
            ['mobility', 'per-result'],
            ['s_gc', 'per-result'],
            ['perm', 'shared'],
            ['grid', 'shared'],
        ]);
        expect(getScenario('sweep_vertical')?.sensitivities.map((dim) => [dim.key, dim.analyticalOverlayMode])).toEqual([
            ['heterogeneity', 'per-result'],
            ['mobility', 'per-result'],
            ['endpoints_vs_geology', 'per-result'],
        ]);
        expect(getScenario('sweep_combined')?.sensitivities.map((dim) => [dim.key, dim.analyticalOverlayMode])).toEqual([
            ['interaction_core', 'per-result'],
            ['sweep_ladder', 'shared'],
        ]);
    });

    it('every scenario passes capability validation against its analytical output contract', () => {
        for (const scenario of listScenarios()) {
            const errors = validateScenarioCapabilities(scenario.capabilities);
            expect(errors, `${scenario.key}: ${errors.join('; ')}`).toEqual([]);
        }
    });

    it('declares spatial-profile path semantics in scenarios that expose a diagonal path', () => {
        for (const key of ['dep_arps', 'dep_pss', 'dep_welltest']) {
            expect(getScenario(key)?.capabilities.spatialProfile).toEqual({
                defaultAxis: 'i',
                wellPathLabel: 'Diagonal',
            });
        }
        expect(getScenario('spe1_gas_injection')?.capabilities.spatialProfile).toEqual({
            defaultAxis: 'i',
            wellPathLabel: 'Injector → producer',
        });
        for (const key of ['sweep_areal', 'sweep_combined']) {
            expect(getScenario(key)?.capabilities.spatialProfile).toEqual({
                defaultAxis: 'well-path',
                wellPathLabel: 'Injector → producer',
            });
        }
    });

    it('rejects a scenario that defaults to an unnamed spatial path', () => {
        const errors = validateScenarioCapabilities({
            analyticalMethod: 'none',
            hasInjector: false,
            default3DScalar: 'pressure',
            spatialProfile: { defaultAxis: 'well-path' },
            requiresThreePhaseMode: false,
        });
        expect(errors).toContain("spatialProfile.defaultAxis 'well-path' requires a wellPathLabel.");
    });

    it('every scenario chart layout only asks for reference curves its method emits', () => {
        for (const scenario of listScenarios()) {
            const emitted = new Set(
                getAnalyticalMethodDescriptor(scenario.capabilities.analyticalMethod)
                    .slots.map((slot) => slot.curveKey),
            );
            const errors = validateScenarioChartLayout(scenario, emitted);
            expect(errors, `${scenario.key}: ${errors.join('; ')}`).toEqual([]);
        }
    });

    it('prioritizes gas-drive GOR and recovery while collapsing oil rate by default', () => {
        const layout = getScenarioChartLayout(getScenario('gas_drive')!).rateChart!;
        expect(layout.panelOrder?.slice(0, 3)).toEqual(['gor', 'recovery', 'rates']);
        expect(layout.panels?.gor?.expanded).toBe(true);
        expect(layout.panels?.recovery?.expanded).toBe(true);
        expect(layout.panels?.rates?.expanded).toBe(false);
    });

    it('validateScenarioChartLayout flags a layout asking for a curve the method cannot produce', () => {
        const sweepScenario = listScenarios().find((s) => s.capabilities.analyticalMethod === 'sweep')!;
        // 'sweep' emits no primary reference curves at all, so any -reference key
        // in its layout is a dead reference rather than something to suppress.
        const errors = validateScenarioChartLayout(
            {
                ...sweepScenario,
                chartLayoutPatch: {
                    rateChart: { panels: { rates: { curveKeys: ['water-cut-sim', 'water-cut-reference'] } } },
                },
            },
            new Set<string>(),
        );
        expect(errors.some((e) => e.includes('water-cut-reference'))).toBe(true);
    });

    it('validateScenarioChartLayout rejects panels that mix physical properties', () => {
        const scenario = listScenarios()[0];
        const errors = validateScenarioChartLayout({
            ...scenario,
            chartLayoutPatch: {
                rateChart: { panels: { diagnostics: { curveKeys: ['avg-pressure-sim', 'mbe-ooip-ratio'] } } },
            },
        }, new Set<string>());
        expect(errors.some((error) => error.includes('mixes properties'))).toBe(true);
    });

    it('any prerun-artifacts scenario declares bundled artifact keys and disables the 3D view (E7)', () => {
        const prerun = listScenarios().filter((s) => s.capabilities.runMode === 'prerun-artifacts');
        for (const scenario of prerun) {
            expect(
                listDeclaredOpmFlowArtifactKeys(scenario.referenceSources).length,
                `${scenario.key} must declare an 'opm-flow' referenceSources entry`,
            ).toBeGreaterThan(0);
            expect(
                scenario.capabilities.default3DScalar,
                `${scenario.key} must set default3DScalar null (3D off)`,
            ).toBeNull();
        }
    });

    it('every live scenario declares the spatial property that best exposes its physics', () => {
        const expectedDefaults = {
            wf_bl1d: 'saturation_water',
            wf_capillary: 'saturation_water',
            // The gravity tongue is a water-saturation structure in the k direction.
            wf_gravity: 'saturation_water',
            wf_gravity_stability: 'saturation_water',
            sweep_areal: 'saturation_water',
            sweep_vertical: 'saturation_water',
            sweep_combined: 'saturation_water',
            dep_pss: 'pressure',
            dep_decline: 'pressure',
            dep_arps: 'pressure',
            // Skin changes flowing BHP, not the rate-controlled pressure field.
            dep_welltest: null,
            // Black-oil depletion is a gas-liberation exhibit, unlike the oil-only depletion cases.
            dep_pvt: 'saturation_gas',
            gas_injection: 'saturation_gas',
            gas_drive: 'saturation_gas',
            spe1_gas_injection: 'saturation_gas',
            // This formulation comparison is a two-phase waterflood.
        } as const;

        expect(Object.fromEntries(
            listScenarios().map((scenario) => [scenario.key, scenario.capabilities.default3DScalar]),
        )).toEqual(expectedDefaults);
    });

    it('validateScenarioCapabilities rejects a prerun-artifacts scenario that leaves the 3D view on', () => {
        const errors = validateScenarioCapabilities({
            analyticalMethod: 'none',
           
            hasInjector: true,
            default3DScalar: 'saturation_water',
            requiresThreePhaseMode: false,
            runMode: 'prerun-artifacts',
        });
        expect(errors.some((e) => e.includes('default3DScalar to null'))).toBe(true);
    });

    it('resolveCapabilities defaults runMode to live-worker and honors prerun-artifacts', () => {
        const live = resolveCapabilities({ analyticalMethod: 'buckley-leverett', hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false });
        expect(live.runMode).toBe('live-worker');
        const prerun = resolveCapabilities({ analyticalMethod: 'none', hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false, runMode: 'prerun-artifacts' });
        expect(prerun.runMode).toBe('prerun-artifacts');
    });

    it('resolveCapabilities produces correct defaults for each analytical method', () => {
        const bl = resolveCapabilities({ analyticalMethod: 'buckley-leverett', hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false });
        expect(bl.primaryRateCurve).toBe('water-cut');
        expect(bl.analyticalNativeXAxis).toBe('pvi');
        expect(bl.hasTauDimensionlessTime).toBe(false);
        expect(bl.sweepGeometry).toBeNull();

        const dep = resolveCapabilities({ analyticalMethod: 'depletion', hasInjector: false, default3DScalar: null, requiresThreePhaseMode: false });
        expect(dep.primaryRateCurve).toBe('oil-rate');
        expect(dep.analyticalNativeXAxis).toBe('time');
        expect(dep.hasTauDimensionlessTime).toBe(true);
        expect(dep.sweepGeometry).toBeNull();

        const gasOil = resolveCapabilities({ analyticalMethod: 'gas-oil-bl', hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false });
        expect(gasOil.primaryRateCurve).toBe('gas-cut');
        expect(gasOil.analyticalNativeXAxis).toBe('pvi');
        expect(gasOil.sweepGeometry).toBeNull();
    });

    it('resolveCapabilities respects explicit overrides', () => {
        const resolved = resolveCapabilities({
            analyticalMethod: 'sweep',
            primaryRateCurve: 'water-cut',
            analyticalNativeXAxis: 'time', // explicit override
            sweepGeometry: 'vertical',
            hasInjector: true,
            default3DScalar: null,
            requiresThreePhaseMode: false,
        });
        expect(resolved.primaryRateCurve).toBe('water-cut');
        expect(resolved.analyticalNativeXAxis).toBe('time');
        expect(resolved.sweepGeometry).toBe('vertical');
    });

    // ── Compile-time capability rules ──────────────────────────────────────
    // These three used to be runtime checks in validateScenarioCapabilities.
    // ScenarioCapabilities is now a discriminated union over analyticalMethod,
    // so they are compile errors instead. `@ts-expect-error` *is* the assertion:
    // `pnpm run typecheck` fails if any of these ever starts compiling.
    it('rejects a primaryRateCurve the analytical method cannot produce', () => {
        // @ts-expect-error depletion supports primaryRateCurve 'oil-rate' only
        const caps: ScenarioCapabilities = {
            analyticalMethod: 'depletion',
            primaryRateCurve: 'water-cut',
            hasInjector: false,
            default3DScalar: null,
            requiresThreePhaseMode: false,
        };
        expect(caps.analyticalMethod).toBe('depletion');
    });

    it('requires sweepGeometry on the sweep method', () => {
        // @ts-expect-error sweepGeometry is mandatory when analyticalMethod is 'sweep'
        const caps: ScenarioCapabilities = {
            analyticalMethod: 'sweep',
            hasInjector: true,
            default3DScalar: null,
            requiresThreePhaseMode: false,
        };
        expect(caps.analyticalMethod).toBe('sweep');
    });

    it('rejects sweepGeometry on a non-sweep method', () => {
        const caps: ScenarioCapabilities = {
            analyticalMethod: 'buckley-leverett',
            // @ts-expect-error sweepGeometry is typed never off the sweep method
            sweepGeometry: 'areal',
            hasInjector: true,
            default3DScalar: null,
            requiresThreePhaseMode: false,
        };
        expect(caps.analyticalMethod).toBe('buckley-leverett');
    });

    it('resolveCapabilities turns the sweep panels on for the sweep method only', () => {
        const sweep = resolveCapabilities({
            analyticalMethod: 'sweep', sweepGeometry: 'vertical',
            hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false,
        });
        expect(sweep.showSweepPanel).toBe(true);
        expect(sweep.sweepGeometry).toBe('vertical');

        const bl = resolveCapabilities({
            analyticalMethod: 'buckley-leverett',
            hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false,
        });
        expect(bl.showSweepPanel).toBe(false);
        expect(bl.sweepGeometry).toBeNull();
    });

    it('resolved capabilities include defaultPanelExpansion from the output contract', () => {
        const bl = resolveCapabilities({ analyticalMethod: 'buckley-leverett', hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false });
        expect(bl.defaultPanelExpansion.diagnostics).toBe(false);

        const dep = resolveCapabilities({ analyticalMethod: 'depletion', hasInjector: false, default3DScalar: null, requiresThreePhaseMode: false });
        expect(dep.defaultPanelExpansion.diagnostics).toBe(true);
    });

    it('ANALYTICAL_OUTPUT_CONTRACTS covers all AnalyticalMethod values', () => {
        // Previously hardcoded 4 of the 7 methods and silently stopped covering
        // 'well-test' and 'digitized-reference' when they were added. Driven off
        // the union's own keys now, so a new method cannot slip past it.
        const methods = Object.keys(ANALYTICAL_OUTPUT_CONTRACTS) as AnalyticalMethod[];
        expect(methods).toContain('sweep');
        expect(methods).toContain('well-test');
        for (const method of methods) {
            const contract = ANALYTICAL_OUTPUT_CONTRACTS[method];
            expect(contract, method).toBeDefined();
            expect(contract.supportedRateCurves.length, method).toBeGreaterThan(0);
            // ScenarioCapabilities narrows primaryRateCurve to supportedRateCurves,
            // so a default outside that set would be unrepresentable in a scenario
            // yet still returned by resolveCapabilities().
            expect(contract.supportedRateCurves as readonly string[], method)
                .toContain(contract.defaultPrimaryRateCurve);
        }
    });

    it('chart presets expose scenario-controlled x-axis range policies', () => {
        expect(CHART_LAYOUTS.waterflood.rateChart?.xAxisRangePolicy).toEqual({
            mode: 'rate-tail-threshold',
            relativeThreshold: 1e-7,
        });
        expect(CHART_LAYOUTS.sweep.rateChart?.xAxisRangePolicy).toEqual({
            mode: 'pvi-window',
            minPvi: 0,
            maxPvi: 2.5,
        });
        expect(CHART_LAYOUTS.oil_depletion.rateChart?.xAxisRangePolicy).toEqual({
            mode: 'data-extent',
        });
        expect(CHART_LAYOUTS.well_test.rateChart).toMatchObject({
            panelOrder: ['producer_bhp', 'diagnostics', 'oil_rate', 'control_limits'],
            panels: {
                producer_bhp: { visible: true, expanded: true },
                oil_rate: { expanded: false },
                diagnostics: { expanded: false },
                control_limits: { visible: true, expanded: false },
            },
        });
    });

    it('merges scenario chart layout patches on top of shared sweep layouts', () => {
        const arealLayout = getScenarioChartLayout(getScenario('sweep_areal')!);
        const combinedLayout = getScenarioChartLayout(getScenario('sweep_combined')!);

        expect(arealLayout.rateChart?.panels?.sweep_vertical?.visible).toBe(false);
        expect(arealLayout.rateChart?.panels?.sweep_areal?.visible).toBe(true);
        expect(arealLayout.rateChart?.panels?.rates?.curveKeys).toEqual(['water-cut-sim']);
        expect(arealLayout.rateChart?.panels?.recovery?.curveKeys).toEqual(['recovery-factor-primary']);

        expect(combinedLayout.rateChart?.panels?.rates?.curveKeys).toEqual(['water-cut-sim']);
        expect(combinedLayout.rateChart?.panels?.recovery?.curveKeys).toEqual(['recovery-factor-primary']);
        expect(combinedLayout.rateChart?.panels?.sweep_combined?.title).toBe('Total Sweep Efficiency (E_vol)');
        expect(combinedLayout.rateChart?.panels?.sweep_combined_mobile_oil?.visible).toBe(true);
    });
});

describe('SPE1 scenario fidelity guards', () => {
    it('ships the exact OPM tabular SCAL inputs in the base case', () => {
        const scenario = getScenario('spe1_gas_injection');

        expect(scenario?.params.k_rw_max).toBe(0.00001);
        expect(scenario?.params.gasRedissolutionEnabled).toBe(false);
        expect((scenario?.params.scalTables as { swof: unknown[]; sgof: unknown[] } | undefined)?.swof).toHaveLength(15);
        expect((scenario?.params.scalTables as { swof: unknown[]; sgof: unknown[] } | undefined)?.sgof).toHaveLength(15);
    });

    it('uses an undersaturated c_o consistent with the SPE1 PVTO 9014.7 psia continuation for Rs = 1.27', () => {
        const scenario = getScenario('spe1_gas_injection');
        const pBubble = 4014.7 / 14.5038;
        const boBubble = 1.695;
        const pUndersat = 9014.7 / 14.5038;
        const boUndersat = 1.579;

        const expectedCo = -Math.log(boUndersat / boBubble) / (pUndersat - pBubble);

        expect(scenario?.params.c_o).toBeCloseTo(expectedCo, 6);
    });

    it('applies tighter numerics to the fine-grid SPE1 sensitivity', () => {
        const params = getScenarioWithVariantParams('spe1_gas_injection', 'grid', 'grid_20');

        expect(params).toMatchObject({
            nx: 20,
            ny: 20,
            delta_t_days: 2.5,
            steps: 1600,
            max_sat_change_per_step: 0.03,
            max_pressure_change_per_step: 30,
            max_well_rate_change_fraction: 0.35,
        });
    });
});

describe('depletion scenario fidelity guards', () => {
    it('holds drainage area fixed across every Dietz PSS geometry', () => {
        // The shape factor is only isolated if area, rate and completion are
        // constant, so a geometry variant that changed the pore volume would
        // silently confound the exhibit with a material-balance difference.
        const area = (params: Record<string, any>) =>
            Number(params.nx) * Number(params.cellDx) * Number(params.ny) * Number(params.cellDy);
        const base = area(getScenarioWithVariantParams('dep_pss', 'drainage_shape', 'geom_square'));
        expect(base).toBeCloseTo(176_400, 0);

        for (const [dimension, keys] of [
            ['drainage_shape', ['geom_2to1', 'geom_4to1', 'geom_5to1', 'geom_4to1_offset']],
            ['well_position', ['pos_quarter', 'pos_quadrant']],
        ] as const) {
            for (const key of keys) {
                const params = getScenarioWithVariantParams('dep_pss', dimension, key);
                expect(Math.abs(area(params) / base - 1), key).toBeLessThan(0.002);
                expect(params.targetProducerRate, key).toBe(40);
                expect(params.well_skin, key).toBe(0);
            }
        }
    });

    it('keeps Fetkovich decline sensitivities in a numerically resolved range', () => {
        const highPerm = getScenarioWithVariantParams('dep_decline', 'permeability', 'perm_good');
        const coarseGrid = getScenarioWithVariantParams('dep_decline', 'grid_refinement', 'grid_coarse');
        const coarseTimestep = getScenarioWithVariantParams('dep_decline', 'timestep', 'timestep_large');

        expect(highPerm).toMatchObject({
            uniformPermX: 40,
            uniformPermY: 40,
            uniformPermZ: 4,
        });
        expect(coarseGrid).toMatchObject({
            nx: 24,
            cellDx: 20,
            producerI: 23,
            well_skin: -0.4581453659370775,
            delta_t_days: 0.2,
            steps: 120,
        });
        expect(coarseTimestep).toMatchObject({
            delta_t_days: 0.1,
            steps: 240,
        });
    });

    it('isolates layered-depletion contrast at fixed total PI', () => {
        const variants = ['contrast_low', 'contrast_base', 'contrast_high'].map((variantKey) =>
            getScenarioWithVariantParams('dep_arps', 'layer_contrast', variantKey),
        );

        for (const params of variants) {
            const permsX = params.layerPermsX as number[];
            expect(permsX.reduce((sum, permeability) => sum + permeability, 0)).toBeCloseTo(100, 4);
            expect(params.layerPermsY).toEqual(permsX);
            expect(params.layerPermsZ).toEqual([1e-9, 1e-9, 1e-9, 1e-9, 1e-9]);
            expect(params.analyticalLayeredComposite).toBe(true);
            expect(params).toMatchObject({ nx: 9, ny: 9, nz: 5, producerI: 4, producerJ: 4 });
            expect(Number(params.nx) * Number(params.cellDx)).toBeCloseTo(1000, 8);
            expect(Number(params.ny) * Number(params.cellDy)).toBeCloseTo(1000, 8);
        }

        const contrast = (params: Record<string, unknown>) => {
            const permeabilities = params.layerPermsX as number[];
            return Math.max(...permeabilities) / Math.min(...permeabilities);
        };
        expect(contrast(variants[0])).toBeCloseTo(3, 1);
        expect(contrast(variants[1])).toBeCloseTo(20, 1);
        expect(contrast(variants[2])).toBeCloseTo(100, 0);
    });
});
