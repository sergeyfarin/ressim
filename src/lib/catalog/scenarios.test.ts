import { describe, expect, it } from 'vitest';
import { calculateAnalyticalProduction } from '../analytical/fractionalFlow';
import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { computeCombinedSweep } from '../analytical/sweepEfficiency';
import type { RockProps, FluidProps } from '../analytical/fractionalFlow';
import {
    getScenario,
    getScenarioWithVariantParams,
    listScenarios,
    resolveCapabilities,
    validateScenarioCapabilities,
    ANALYTICAL_OUTPUT_CONTRACTS,
} from './scenarios';

describe('sweep scenario sensitivities', () => {
    it('provides analytical method metadata for every canonical scenario', () => {
        const scenarioKeys = [
            'wf_bl1d',
            'sweep_areal',
            'sweep_vertical',
            'sweep_combined',
            'dep_pss',
            'dep_decline',
            'gas_injection',
            'gas_drive',
        ];

        for (const key of scenarioKeys) {
            const scenario = getScenario(key);
            expect(scenario?.analyticalMethodSummary?.length).toBeGreaterThan(10);
            expect(scenario?.analyticalMethodReference?.length).toBeGreaterThan(5);
        }
    });

    it('adds a seeded areal heterogeneity axis for the areal sweep scenario', () => {
        const scenario = getScenario('sweep_areal');

        expect(scenario?.defaultSensitivityDimensionKey).toBe('mobility');
        expect(scenario?.sensitivities.map((dimension) => dimension.key)).toEqual([
            'mobility',
            'areal_heterogeneity',
            'sor',
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
            minPerm: 120,
            maxPerm: 280,
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
        nx: params.nx != null ? toNum(params.nx, 1) : undefined,
        ny: params.ny != null ? toNum(params.ny, 1) : undefined,
        producerI: params.producerI != null ? toNum(params.producerI, 0) : undefined,
        producerJ: params.producerJ != null ? toNum(params.producerJ, 0) : undefined,
    });
    return [result.meta.q0 ?? 0, result.meta.tau ?? 0, ...result.production.map((pt) => pt.oilRate)];
}

function analyticalFingerprint(scenarioClass: string, params: Record<string, unknown>): number[] {
    if (scenarioClass === 'depletion') return depletionFingerprint(params);
    // waterflood class covers both BL and sweep scenarios
    return [...blFingerprint(params), ...sweepFingerprint(params)];
}

function arraysEqual(a: number[], b: number[], tol = 1e-12): boolean {
    if (a.length !== b.length) return false;
    return a.every((v, i) => Math.abs(v - b[i]) < tol);
}

describe('affectsAnalytical contract', () => {
    const analyticalScenarios = ['wf_bl1d', 'sweep_areal', 'sweep_vertical', 'sweep_combined', 'dep_pss', 'dep_decline'];

    for (const scenarioKey of analyticalScenarios) {
        const scenario = getScenario(scenarioKey)!;

        for (const dim of scenario.sensitivities) {
            const baseFingerprint = analyticalFingerprint(
                scenario.scenarioClass,
                scenario.params as Record<string, unknown>,
            );

            for (const variant of dim.variants) {
                const isBaseCase = Object.keys(variant.paramPatch).length === 0;
                const variantParams = getScenarioWithVariantParams(scenarioKey, dim.key, variant.key);
                const variantFp = analyticalFingerprint(scenario.scenarioClass, variantParams);

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

describe('scenario capability validation', () => {
    it('every scenario passes capability validation against its analytical output contract', () => {
        for (const scenario of listScenarios()) {
            const errors = validateScenarioCapabilities(scenario.capabilities);
            expect(errors, `${scenario.key}: ${errors.join('; ')}`).toEqual([]);
        }
    });

    it('resolveCapabilities produces correct defaults for each analytical method', () => {
        const bl = resolveCapabilities({ analyticalMethod: 'buckley-leverett', showSweepPanel: false, hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false });
        expect(bl.primaryRateCurve).toBe('water-cut');
        expect(bl.analyticalNativeXAxis).toBe('pvi');
        expect(bl.hasTauDimensionlessTime).toBe(false);

        const dep = resolveCapabilities({ analyticalMethod: 'depletion', showSweepPanel: false, hasInjector: false, default3DScalar: null, requiresThreePhaseMode: false });
        expect(dep.primaryRateCurve).toBe('oil-rate');
        expect(dep.analyticalNativeXAxis).toBe('time');
        expect(dep.hasTauDimensionlessTime).toBe(true);

        const gasOil = resolveCapabilities({ analyticalMethod: 'gas-oil-bl', showSweepPanel: false, hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false });
        expect(gasOil.primaryRateCurve).toBe('gas-cut');
        expect(gasOil.analyticalNativeXAxis).toBe('pvi');
    });

    it('resolveCapabilities respects explicit overrides', () => {
        const resolved = resolveCapabilities({
            analyticalMethod: 'buckley-leverett',
            primaryRateCurve: 'oil-rate', // explicit override
            analyticalNativeXAxis: 'time', // explicit override
            showSweepPanel: false,
            hasInjector: true,
            default3DScalar: null,
            requiresThreePhaseMode: false,
        });
        expect(resolved.primaryRateCurve).toBe('oil-rate');
        expect(resolved.analyticalNativeXAxis).toBe('time');
    });

    it('validateScenarioCapabilities catches invalid primaryRateCurve for analytical method', () => {
        const errors = validateScenarioCapabilities({
            analyticalMethod: 'depletion',
            primaryRateCurve: 'water-cut', // depletion cannot produce water-cut
            showSweepPanel: false,
            hasInjector: false,
            default3DScalar: null,
            requiresThreePhaseMode: false,
        });
        expect(errors.length).toBeGreaterThan(0);
        expect(errors[0]).toContain('water-cut');
    });

    it('resolved capabilities include defaultPanelExpansion from the output contract', () => {
        const bl = resolveCapabilities({ analyticalMethod: 'buckley-leverett', showSweepPanel: false, hasInjector: true, default3DScalar: null, requiresThreePhaseMode: false });
        expect(bl.defaultPanelExpansion.diagnostics).toBe(false);

        const dep = resolveCapabilities({ analyticalMethod: 'depletion', showSweepPanel: false, hasInjector: false, default3DScalar: null, requiresThreePhaseMode: false });
        expect(dep.defaultPanelExpansion.diagnostics).toBe(true);
    });

    it('ANALYTICAL_OUTPUT_CONTRACTS covers all AnalyticalMethod values', () => {
        const methods = ['buckley-leverett', 'gas-oil-bl', 'depletion', 'none'] as const;
        for (const method of methods) {
            expect(ANALYTICAL_OUTPUT_CONTRACTS[method]).toBeDefined();
            expect(ANALYTICAL_OUTPUT_CONTRACTS[method].supportedRateCurves.length).toBeGreaterThan(0);
        }
    });
});