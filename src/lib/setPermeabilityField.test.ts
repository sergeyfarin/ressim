import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from './ressim/pkg/simulator.js';

// E1: exercises the additive `setPermeabilityField` wasm setter directly (the
// same real-engine pattern as wf_tornado.test.ts / dep_nct.test.ts). A full
// `cargo test` is not a valid gate here (SPE1/FIM tests hang), so this drives
// the compiled binding instead.

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    if (!wasmReady) {
        wasmReady = readFile(new URL('./ressim/pkg/simulator_bg.wasm', import.meta.url)).then(
            (wasmBytes) => initWasm({ module_or_path: wasmBytes }),
        );
    }
    await wasmReady;
}

// 2 x 1 x 2 grid → 4 cells.
const NX = 2;
const NY = 1;
const NZ = 2;
const TOTAL = NX * NY * NZ;

function newSim(): ReservoirSimulator {
    return new ReservoirSimulator(NX, NY, NZ, 0.2);
}

describe('setPermeabilityField (E1 per-cell permeability)', () => {
    it('accepts full-length, finite, positive per-cell field vectors', async () => {
        await ensureWasmReady();
        const sim = newSim();
        const x = new Float64Array([100, 200, 300, 400]);
        const y = new Float64Array([100, 200, 300, 400]);
        const z = new Float64Array([10, 20, 30, 40]);
        expect(() => sim.setPermeabilityField(x, y, z)).not.toThrow();
    });

    it('rejects vectors whose length is not nx*ny*nz', async () => {
        await ensureWasmReady();
        const sim = newSim();
        const short = new Float64Array(TOTAL - 1).fill(100);
        const ok = new Float64Array(TOTAL).fill(100);
        expect(() => sim.setPermeabilityField(short, ok, ok)).toThrow();
        expect(() => sim.setPermeabilityField(ok, ok, new Float64Array(TOTAL + 1).fill(100))).toThrow();
    });

    it('rejects non-positive or non-finite permeabilities', async () => {
        await ensureWasmReady();
        const sim = newSim();
        const ok = new Float64Array(TOTAL).fill(100);
        expect(() => sim.setPermeabilityField(new Float64Array([100, 0, 100, 100]), ok, ok)).toThrow();
        expect(() => sim.setPermeabilityField(new Float64Array([100, -5, 100, 100]), ok, ok)).toThrow();
        expect(() => sim.setPermeabilityField(new Float64Array([100, Number.NaN, 100, 100]), ok, ok)).toThrow();
    });
});
