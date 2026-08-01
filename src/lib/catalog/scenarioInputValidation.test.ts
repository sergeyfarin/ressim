/**
 * Every catalog scenario, and every sensitivity variant of it, must pass the
 * same input validation the UI runs before it will start a simulation.
 *
 * This exists because `wf_gravity_stability` shipped with a blocking
 * "Injector and producer cannot share the same i/j location" error: its 1 x 1 x 60
 * column puts both wells in the one column the grid has, which the rule had not
 * anticipated. Scenario-level physics tests drive the wasm core directly and so
 * never saw it. This test goes through the product's own gate instead.
 */
import { describe, expect, it } from 'vitest';
import { listScenarios, getScenarioWithVariantParams, getDefaultScenarioAnalyticalMode } from './scenarios';
import { evaluateAnalyticalStatus } from '../warningPolicy';
import { getDefaultToggles } from './caseCatalog';
import {
    buildFloodFrontOverlay,
    defaultSpatialProfileAxis,
    resolveDisplacementAxis,
} from '../visualization/spatialProfileModel';
import { validateInputs, type SimulationInputs } from '../validateInputs';
import { buildCreatePayloadFromState } from '../buildCreatePayload';

/** Maps a scenario param record onto the validation input contract. */
function toValidationInput(params: Record<string, unknown>): SimulationInputs {
    const number = (key: string, fallback = 0) => {
        const value = Number(params[key]);
        return Number.isFinite(value) ? value : fallback;
    };
    const layers = (key: string) => (Array.isArray(params[key]) ? (params[key] as number[]) : undefined);
    return {
        nx: number('nx', 1), ny: number('ny', 1), nz: number('nz', 1),
        cellDx: number('cellDx', 10), cellDy: number('cellDy', 10), cellDz: number('cellDz', 1),
        steps: number('steps', 1),
        initialSaturation: number('initialSaturation'),
        delta_t_days: number('delta_t_days', 1),
        well_radius: number('well_radius', 0.1),
        mu_w: number('mu_w', 0.5), mu_o: number('mu_o', 1),
        c_o: number('c_o', 1e-5), c_w: number('c_w', 3e-6),
        rock_compressibility: number('rock_compressibility', 1e-6),
        volume_expansion_o: number('volume_expansion_o', 1),
        volume_expansion_w: number('volume_expansion_w', 1),
        max_sat_change_per_step: number('max_sat_change_per_step', 0.1),
        max_pressure_change_per_step: number('max_pressure_change_per_step', 75),
        max_well_rate_change_fraction: number('max_well_rate_change_fraction', 0.75),
        injectorI: number('injectorI'), injectorJ: number('injectorJ'),
        producerI: number('producerI'), producerJ: number('producerJ'),
        injectorKLayers: layers('injectorKLayers'),
        producerKLayers: layers('producerKLayers'),
        s_wc: number('s_wc'), s_or: number('s_or'),
        s_gc: number('s_gc'), s_gr: number('s_gr'), s_org: number('s_org'),
        n_g: number('n_g', 1.5), mu_g: number('mu_g', 0.02), c_g: number('c_g', 1e-4),
        threePhaseModeEnabled: params.threePhaseModeEnabled === true,
        uniformPermX: number('uniformPermX', 100),
        reservoirPorosity: number('reservoirPorosity', 0.2),
        minPerm: number('minPerm', 1), maxPerm: number('maxPerm', 1000),
        injectorEnabled: params.injectorEnabled !== false,
        injectorControlMode: String(params.injectorControlMode ?? 'pressure'),
        producerControlMode: String(params.producerControlMode ?? 'pressure'),
        injectorBhp: number('injectorBhp', 500), producerBhp: number('producerBhp', 100),
        targetInjectorRate: number('targetInjectorRate'),
        targetProducerRate: number('targetProducerRate'),
        targetInjectorSurfaceRate: params.targetInjectorSurfaceRate as number | undefined,
        targetProducerSurfaceRate: params.targetProducerSurfaceRate as number | undefined,
    };
}

describe('catalog scenarios pass the product input gate', () => {
    it('has no blocking validation error in any scenario or variant', () => {
        const failures: string[] = [];

        for (const scenario of listScenarios()) {
            const cases: { label: string; params: Record<string, unknown> }[] = [
                { label: 'base', params: scenario.params },
                ...scenario.sensitivities.flatMap((dimension) => dimension.variants.map((variant) => ({
                    label: `${dimension.key}/${variant.key}`,
                    params: getScenarioWithVariantParams(scenario.key, dimension.key, variant.key),
                }))),
            ];

            for (const { label, params } of cases) {
                const { errors } = validateInputs(toValidationInput(params));
                for (const [key, message] of Object.entries(errors)) {
                    failures.push(`${scenario.key} [${label}] ${key}: ${message}`);
                }
            }
        }

        expect(failures).toEqual([]);
    });
});

describe('catalog scenarios produce a sane well payload', () => {
    it('perforates the layers each scenario declares, and never two wells in one cell', () => {
        const failures: string[] = [];

        for (const scenario of listScenarios()) {
            const cases: { label: string; params: Record<string, unknown> }[] = [
                { label: 'base', params: scenario.params },
                ...scenario.sensitivities.flatMap((dimension) => dimension.variants.map((variant) => ({
                    label: `${dimension.key}/${variant.key}`,
                    params: getScenarioWithVariantParams(scenario.key, dimension.key, variant.key),
                }))),
            ];

            for (const { label, params } of cases) {
                const where = `${scenario.key} [${label}]`;
                const payload = buildCreatePayloadFromState(params as Parameters<typeof buildCreatePayloadFromState>[0]);
                const wells = payload.wells ?? [];
                const expectedWells = params.injectorEnabled === false ? 1 : 2;
                if (wells.length !== expectedWells) {
                    failures.push(`${where}: expected ${expectedWells} wells, got ${wells.length}`);
                }

                for (const well of wells) {
                    const declared = well.injector ? params.injectorKLayers : params.producerKLayers;
                    const expectedLayers = Array.isArray(declared) && declared.length > 0
                        ? (declared as number[])
                        : Array.from({ length: Number(params.nz) }, (_, k) => k);
                    const actual = well.completions.map((completion) => completion.k);
                    if (JSON.stringify(actual) !== JSON.stringify(expectedLayers)) {
                        failures.push(`${where}: ${well.id} perforates [${actual}], expected [${expectedLayers}]`);
                    }
                    for (const completion of well.completions) {
                        if (completion.k >= Number(params.nz) || completion.i >= Number(params.nx) || completion.j >= Number(params.ny)) {
                            failures.push(`${where}: ${well.id} completion outside grid at ${completion.i},${completion.j},${completion.k}`);
                        }
                    }
                }

                const cells = wells.flatMap((well) => well.completions.map((c) => `${c.i},${c.j},${c.k}`));
                if (new Set(cells).size !== cells.length) {
                    failures.push(`${where}: two wells share a cell (${cells.join(' ')})`);
                }
            }
        }

        expect(failures).toEqual([]);
    });
});

describe('analytical caveats describe the model actually being run', () => {
    /**
     * Locked because these strings are the product's honesty layer. They used to
     * be read from the Scenario Builder's toggle state, whose dimension catalog
     * ships empty, so every waterflood scenario — `wf_bl1d` included — was told
     * "Geometry is not 1D" and "Wells are not end-to-end" regardless of its grid.
     */
    const EXPECTED_CAVEATS: Record<string, string[]> = {
        wf_bl1d: [],
        wf_capillary: ['capillary-enabled'],
        wf_gravity: ['wf-geometry-not-1d', 'gravity-enabled'],
        wf_gravity_stability: ['gravity-enabled'],
        sweep_areal: ['wf-geometry-not-1d'],
        sweep_vertical: ['wf-geometry-not-1d'],
        sweep_combined: ['wf-geometry-not-1d'],
        dep_welltest: [],
        dep_pss: [],
        dep_decline: [],
        dep_arps: ['perm-layered-depletion'],
        dep_pvt: ['analytical-disabled'],
        gas_injection: [],
        gas_drive: ['analytical-disabled'],
        spe1_gas_injection: ['analytical-disabled'],
    };

    it('raises exactly the caveats each scenario earns', () => {
        const actual: Record<string, string[]> = {};

        for (const scenario of listScenarios()) {
            const params = scenario.params as Record<string, unknown>;
            const status = evaluateAnalyticalStatus({
                activeMode: scenario.catalog.caseMode,
                analyticalMode: getDefaultScenarioAnalyticalMode(scenario.capabilities),
                injectorEnabled: params.injectorEnabled !== false,
                gravityEnabled: params.gravityEnabled === true,
                capillaryEnabled: params.capillaryEnabled === true,
                permMode: (params.permMode ?? 'uniform') as 'uniform' | 'random' | 'perLayer' | 'field',
                toggles: getDefaultToggles(scenario.catalog.caseMode),
                geometry: {
                    nx: Number(params.nx), ny: Number(params.ny), nz: Number(params.nz),
                    injectorI: Number(params.injectorI ?? 0), injectorJ: Number(params.injectorJ ?? 0),
                    producerI: Number(params.producerI ?? 0), producerJ: Number(params.producerJ ?? 0),
                    injectorKLayers: params.injectorKLayers as number[] | undefined,
                    producerKLayers: params.producerKLayers as number[] | undefined,
                },
            });
            actual[scenario.key] = status.reasonDetails.map((reason) => reason.code);
        }

        expect(actual).toEqual(EXPECTED_CAVEATS);
    });
});

describe('the spatial profile carries its analytical reference', () => {
    /**
     * Per scenario: which axis the flood runs down, which axis the profile opens
     * on, and whether the Buckley-Leverett saturation overlay is therefore drawn
     * there. `wf_gravity_stability` is the reason this exists — its flood runs
     * down K, the overlay only ever handled I, and the profile plot showed a
     * simulated curve with no reference beside it.
     */
    const EXPECTED = {
        wf_bl1d: { displacement: 'i', defaultAxis: 'i', overlay: true },
        wf_capillary: { displacement: 'i', defaultAxis: 'i', overlay: true },
        // Opens on K to show the tongue's vertical structure; the flood runs
        // along I, and BL has nothing to say about a saturation profile across
        // the flood direction, so no reference is drawn there. Switching the
        // profile to I brings it back.
        wf_gravity: { displacement: 'i', defaultAxis: 'k', overlay: false },
        wf_gravity_stability: { displacement: 'k', defaultAxis: 'k', overlay: true },
        sweep_vertical: { displacement: 'i', defaultAxis: 'i', overlay: true },
    } as const;

    it('draws the flood-front overlay wherever the profile axis is the flood axis', () => {
        const actual: Record<string, { displacement: string; defaultAxis: string; overlay: boolean }> = {};

        for (const scenario of listScenarios()) {
            if (!(scenario.key in EXPECTED)) continue;
            const params = scenario.params as Record<string, number[] | number | undefined>;
            const grid = {
                nx: Number(params.nx), ny: Number(params.ny), nz: Number(params.nz),
                cellDx: Number(params.cellDx), cellDy: Number(params.cellDy), cellDz: Number(params.cellDz),
            };
            const layer = (list: unknown, fallback: number) => (
                Array.isArray(list) && list.length > 0 ? Number(list[0]) : fallback
            );
            const wells = {
                injector: {
                    i: Number(params.injectorI ?? 0), j: Number(params.injectorJ ?? 0),
                    k: layer(params.injectorKLayers, 0),
                },
                producer: {
                    i: Number(params.producerI ?? 0), j: Number(params.producerJ ?? 0),
                    k: layer(params.producerKLayers, grid.nz - 1),
                },
            };
            const axis = defaultSpatialProfileAxis(
                grid,
                scenario.capabilities.spatialProfile?.defaultAxis ?? null,
            );
            const overlay = buildFloodFrontOverlay({
                grid, axis, property: 'saturation_water',
                rock: {
                    s_wc: Number(params.s_wc), s_or: Number(params.s_or),
                    n_w: Number(params.n_w), n_o: Number(params.n_o),
                    k_rw_max: Number(params.k_rw_max), k_ro_max: Number(params.k_ro_max),
                },
                fluid: { mu_w: Number(params.mu_w), mu_o: Number(params.mu_o) },
                initialSaturation: Number(params.initialSaturation),
                porosity: Number(params.reservoirPorosity),
                // Half a pore volume in, so the front is inside the model.
                injectedVolume: 0.5 * grid.nx * grid.cellDx * grid.ny * grid.cellDy
                    * grid.nz * grid.cellDz * Number(params.reservoirPorosity),
                wells,
            });

            actual[scenario.key] = {
                displacement: resolveDisplacementAxis(wells.injector, wells.producer),
                defaultAxis: axis,
                overlay: overlay !== null,
            };
        }

        expect(actual).toEqual(EXPECTED);
    });
});
