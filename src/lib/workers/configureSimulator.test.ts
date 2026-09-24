import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../ressim/pkg/simulator.js';
import { buildCreatePayloadForRun, buildScenarioRunSpecs } from '../scenario/runModel';
import type { SimulatorCreatePayload } from '../simulator-types';
import { configureReservoirSimulator } from './configureSimulator';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

/** The payload a scenario's base run sends to the worker, built the way the app builds it. */
function scenarioBasePayload(scenarioKey: string): SimulatorCreatePayload {
    const [spec] = buildScenarioRunSpecs({ scenarioKey, dimensionKey: null, variantKeys: [] });
    expect(spec, `${scenarioKey} should have a base run`).toBeDefined();
    return buildCreatePayloadForRun(spec);
}

/** A simulator configured by the worker's own setup function. */
function configure(payload: SimulatorCreatePayload): ReservoirSimulator {
    const simulator = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));
    configureReservoirSimulator(simulator, payload);
    return simulator;
}

type WellEntry = {
    i: number;
    j: number;
    k: number;
    injector: boolean;
    physical_well_id?: string;
    schedule: {
        control_mode?: string;
        target_surface_rate_m3_day?: number;
        bhp_limit?: number;
        enabled: boolean;
    };
};

/**
 * #12: SPE1's layered geometry and its two single-layer wells must reach the engine as the deck
 * defines them. These go through the worker's setup (`configureReservoirSimulator`), so a wiring
 * mistake in the worker fails here rather than surfacing as a quietly different SPE1 run.
 */
describe('SPE1 worker wiring', () => {
    it('keeps the 20 / 30 / 50 ft layer thicknesses', async () => {
        await ensureWasmReady();
        const payload = scenarioBasePayload('spe1_gas_injection');

        const simulator = configure(payload);
        const thicknesses = Array.from(simulator.getLayerThicknesses());
        simulator.free();
        expect(thicknesses).toHaveLength(3);
        [6.096, 9.144, 15.24].forEach((expected, k) => expect(thicknesses[k]).toBeCloseTo(expected, 10));

        // The check can tell: without the per-layer array the grid falls back to the uniform
        // `cellDz`, which is what a dropped field would silently produce.
        const uniform = configure({ ...payload, cellDzPerLayer: undefined });
        const fallback = Array.from(uniform.getLayerThicknesses());
        uniform.free();
        expect(fallback).toEqual([payload.cellDz, payload.cellDz, payload.cellDz]);
    });

    it('completes each well in its deck layer under the deck controls', async () => {
        await ensureWasmReady();
        const simulator = configure(scenarioBasePayload('spe1_gas_injection'));
        const wells = simulator.getWellState() as WellEntry[];
        simulator.free();

        const summary = wells.map((well) => ({
            id: well.physical_well_id,
            injector: well.injector,
            cell: [well.i, well.j, well.k],
            control: well.schedule.control_mode,
            surfaceTarget: well.schedule.target_surface_rate_m3_day,
            bhpLimit: well.schedule.bhp_limit,
            enabled: well.schedule.enabled,
        }));
        expect(summary).toEqual([
            // Producer at (10, 10) in the bottom layer: ORAT 20,000 STB/d, 1000 psia floor.
            {
                id: 'producer-main', injector: false, cell: [9, 9, 2],
                control: 'rate', surfaceTarget: 3179.74, bhpLimit: 69, enabled: true,
            },
            // Injector at (1, 1) in the top layer: 100 MMscf/d gas, 9014 psia ceiling.
            {
                id: 'injector-main', injector: true, cell: [0, 0, 0],
                control: 'rate', surfaceTarget: 2_831_680, bhpLimit: 621, enabled: true,
            },
        ]);
    });
});
