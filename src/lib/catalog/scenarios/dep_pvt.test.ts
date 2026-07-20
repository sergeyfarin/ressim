import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../scenarios';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    if (!wasmReady) {
        wasmReady = readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url)).then(
            (wasmBytes) => initWasm({ module_or_path: wasmBytes }),
        );
    }
    await wasmReady;
}

type Params = Record<string, unknown>;

function buildAndRun(params: Params, steps: number): { avgPressure: number; time: number } {
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const nz = Number(params.nz);

    const sim = new ReservoirSimulator(nx, ny, nz, Number(params.reservoirPorosity));
    sim.setFimEnabled(Boolean(params.fimEnabled));
    sim.setCellDimensions(Number(params.cellDx), Number(params.cellDy), Number(params.cellDz));
    sim.setRelPermProps(
        Number(params.s_wc), Number(params.s_or),
        Number(params.n_w), Number(params.n_o),
        Number(params.k_rw_max), Number(params.k_ro_max),
    );
    sim.setFluidProperties(Number(params.mu_o), Number(params.mu_w));
    sim.setFluidCompressibilities(Number(params.c_o), Number(params.c_w));
    (sim as unknown as { setPvtTable: (t: unknown) => void }).setPvtTable(params.pvtTable);
    sim.setRockProperties(
        Number(params.rock_compressibility), Number(params.depth_reference),
        Number(params.volume_expansion_o), Number(params.volume_expansion_w),
    );
    sim.setFluidDensities(Number(params.rho_o), Number(params.rho_w));
    sim.setInitialPressure(Number(params.initialPressure));
    sim.setInitialSaturation(Number(params.initialSaturation));
    sim.setCapillaryParams(
        Boolean(params.capillaryEnabled) ? Number(params.capillaryPEntry) : 0,
        Number(params.capillaryLambda),
    );
    sim.setGravityEnabled(Boolean(params.gravityEnabled));
    (sim as unknown as { setThreePhaseModeEnabled: (b: boolean) => void }).setThreePhaseModeEnabled(true);
    (sim as unknown as { setThreePhaseRelPermProps: (...a: number[]) => void }).setThreePhaseRelPermProps(
        Number(params.s_wc), Number(params.s_or),
        Number(params.s_gc), Number(params.s_gr), Number(params.s_org),
        Number(params.n_w), Number(params.n_o), Number(params.n_g),
        Number(params.k_rw_max), Number(params.k_ro_max), Number(params.k_rg_max),
    );
    (sim as unknown as { setGasFluidProperties: (...a: number[]) => void }).setGasFluidProperties(
        Number(params.mu_g), Number(params.c_g), Number(params.rho_g),
    );
    sim.setPermeabilityPerLayer(
        new Float64Array(Array.from({ length: nz }, () => Number(params.uniformPermX))),
        new Float64Array(Array.from({ length: nz }, () => Number(params.uniformPermY))),
        new Float64Array(Array.from({ length: nz }, () => Number(params.uniformPermZ))),
    );
    sim.setStabilityParams(
        Number(params.max_sat_change_per_step),
        Number(params.max_pressure_change_per_step),
        Number(params.max_well_rate_change_fraction),
    );
    sim.setWellControlModes(String(params.injectorControlMode), String(params.producerControlMode));
    sim.setTargetWellRates(0, 0);
    const producerBhp = Number(params.producerBhp);
    sim.setWellBhpLimits(producerBhp, Number(params.initialPressure));
    sim.add_well(
        Number(params.producerI), Number(params.producerJ), 0,
        producerBhp, Number(params.well_radius), Number(params.well_skin), false,
    );

    const dt = Number(params.delta_t_days);
    let avgPressure = Number(params.initialPressure);
    let time = 0;
    for (let i = 0; i < steps; i++) {
        sim.step(dt);
        const hist = sim.getRateHistorySince(0) as Array<{ avg_reservoir_pressure?: number; time?: number }>;
        if (hist.length > 0) {
            const last = hist[hist.length - 1];
            if (typeof last.avg_reservoir_pressure === 'number') avgPressure = last.avg_reservoir_pressure;
            if (typeof last.time === 'number') time = last.time;
        }
    }
    return { avgPressure, time };
}

describe('dep_pvt — PVT-table representation risk', () => {
    it('the two PVT tables share an identical calibration point at and below the bubble point', () => {
        const correlationParams = getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_correlation');
        const labParams = getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_lab_report');

        const correlationTable = correlationParams.pvtTable as Array<{ p_bar: number; rs_m3m3: number; bo_m3m3: number }>;
        const labTable = labParams.pvtTable as Array<{ p_bar: number; rs_m3m3: number; bo_m3m3: number }>;

        const bubblePointBar = 150;
        for (let i = 0; i < correlationTable.length; i++) {
            if (correlationTable[i].p_bar > bubblePointBar) continue;
            expect(labTable[i].p_bar).toBeCloseTo(correlationTable[i].p_bar, 9);
            expect(labTable[i].rs_m3m3).toBeCloseTo(correlationTable[i].rs_m3m3, 9);
            expect(labTable[i].bo_m3m3).toBeCloseTo(correlationTable[i].bo_m3m3, 9);
        }

        // Above the bubble point, Bo must diverge (that's the whole point).
        const aboveBp = correlationTable.findIndex((row) => row.p_bar > bubblePointBar + 20);
        expect(aboveBp).toBeGreaterThan(-1);
        expect(labTable[aboveBp].bo_m3m3).not.toBeCloseTo(correlationTable[aboveBp].bo_m3m3, 4);
        expect(labTable[aboveBp].bo_m3m3).toBeLessThan(correlationTable[aboveBp].bo_m3m3);
    });

    it('the two PVT-table variants produce different pressure depletion while undersaturated', async () => {
        await ensureWasmReady();

        const correlationParams = getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_correlation');
        const labParams = getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_lab_report');

        // A short run, still comfortably above the 150 bar bubble point
        // (initial pressure 280 bar), is enough to see the two undersaturated
        // Bo trends produce measurably different pressure trajectories.
        const shortSteps = 20;
        const correlationResult = buildAndRun(correlationParams, shortSteps);
        const labResult = buildAndRun(labParams, shortSteps);

        expect(correlationResult.avgPressure).toBeGreaterThan(150);
        expect(labResult.avgPressure).toBeGreaterThan(150);
        expect(correlationResult.avgPressure).not.toBeCloseTo(labResult.avgPressure, 2);
    }, 30000);
});
