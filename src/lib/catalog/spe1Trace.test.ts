import { describe, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import init, { ReservoirSimulator } from '../ressim/pkg/simulator.js';
import { buildBenchmarkCreatePayload } from '../benchmarkRunModel';
import { getScenarioWithVariantParams } from './scenarios';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    if (!wasmReady) {
        wasmReady = readFile(new URL('../ressim/pkg/simulator_bg.wasm', import.meta.url)).then(
            (wasmBytes) => init({ module_or_path: wasmBytes }),
        );
    }
    await wasmReady;
}

function configureSimulator(payload: ReturnType<typeof buildBenchmarkCreatePayload>) {
    const simulator = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));

    if (payload.cellDzPerLayer?.length) {
        simulator.setCellDimensionsPerLayer(Number(payload.cellDx), Number(payload.cellDy), new Float64Array(payload.cellDzPerLayer));
    } else {
        simulator.setCellDimensions(Number(payload.cellDx), Number(payload.cellDy), Number(payload.cellDz));
    }

    simulator.setFluidProperties(Number(payload.mu_o), Number(payload.mu_w));
    simulator.setFluidCompressibilities(Number(payload.c_o), Number(payload.c_w));
    if (payload.pvtMode === 'black-oil' && payload.pvtTable) {
        simulator.setPvtTable(payload.pvtTable);
    }
    simulator.setRockProperties(
        Number(payload.rock_compressibility),
        Number(payload.depth_reference),
        Number(payload.volume_expansion_o),
        Number(payload.volume_expansion_w),
    );
    simulator.setFluidDensities(Number(payload.rho_o), Number(payload.rho_w));
    simulator.setInitialPressure(payload.initialPressure);
    simulator.setInitialSaturation(payload.initialSaturation);
    simulator.setCapillaryParams(payload.capillaryEnabled ? Number(payload.capillaryPEntry) : 0, Number(payload.capillaryLambda));
    simulator.setGravityEnabled(Boolean(payload.gravityEnabled));
    simulator.setRelPermProps(payload.s_wc, payload.s_or, payload.n_w, payload.n_o, payload.k_rw_max ?? 1, payload.k_ro_max ?? 1);

    if (payload.threePhaseModeEnabled) {
        simulator.setThreePhaseModeEnabled(true);
        simulator.setThreePhaseRelPermProps(
            payload.s_wc,
            payload.s_or,
            payload.s_gc ?? 0.05,
            payload.s_gr ?? 0.05,
            payload.s_org ?? 0.15,
            payload.n_w,
            payload.n_o,
            payload.n_g ?? 1.5,
            payload.k_rw_max ?? 1,
            payload.k_ro_max ?? 1,
            payload.k_rg_max ?? 1,
        );
        if (payload.scalTables) {
            simulator.setThreePhaseScalTables(payload.scalTables);
        }
        simulator.setGasFluidProperties(payload.mu_g ?? 0.02, payload.c_g ?? 1e-4, payload.rho_g ?? 10);
        simulator.setGasRedissolutionEnabled(payload.gasRedissolutionEnabled !== false);
        simulator.setInjectedFluid(payload.injectedFluid ?? 'gas');
    }

    simulator.setStabilityParams(
        payload.max_sat_change_per_step,
        payload.max_pressure_change_per_step,
        payload.max_well_rate_change_fraction,
    );
    simulator.setWellControlModes(String(payload.injectorControlMode ?? 'pressure'), String(payload.producerControlMode ?? 'pressure'));
    simulator.setTargetWellRates(Number(payload.targetInjectorRate ?? 0), Number(payload.targetProducerRate ?? 0));
    simulator.setTargetWellSurfaceRates(Number(payload.targetInjectorSurfaceRate ?? 0), Number(payload.targetProducerSurfaceRate ?? 0));
    simulator.setWellBhpLimits(Number(payload.bhpMin ?? 0), Number(payload.bhpMax ?? Math.max(payload.injectorBhp, payload.producerBhp)));
    simulator.setPermeabilityPerLayer(new Float64Array(payload.permsX), new Float64Array(payload.permsY), new Float64Array(payload.permsZ));

    const producerLayers = Array.isArray(payload.producerKLayers)
        ? payload.producerKLayers
        : Array.from({ length: payload.nz }, (_, index) => index);
    const injectorLayers = Array.isArray(payload.injectorKLayers)
        ? payload.injectorKLayers
        : Array.from({ length: payload.nz }, (_, index) => index);

    for (const k of producerLayers) {
        simulator.add_well(Number(payload.producerI ?? payload.nx - 1), Number(payload.producerJ ?? 0), k, Number(payload.producerBhp), payload.well_radius, payload.well_skin, false);
    }
    for (const k of injectorLayers) {
        simulator.add_well(Number(payload.injectorI ?? 0), Number(payload.injectorJ ?? 0), k, Number(payload.injectorBhp), payload.well_radius, payload.well_skin, true);
    }

    return simulator;
}

function turningPoints(history: ReturnType<ReservoirSimulator['getRateHistory']>) {
    const points: Array<Record<string, number>> = [];
    for (let i = 1; i < history.length - 1; i += 1) {
        const prev = Number(history[i - 1].producing_gor ?? 0);
        const curr = Number(history[i].producing_gor ?? 0);
        const next = Number(history[i + 1].producing_gor ?? 0);
        if ((curr > prev && curr > next) || (curr < prev && curr < next)) {
            points.push({
                t: Math.round(Number(history[i].time ?? 0)),
                gor: Number(curr.toFixed(1)),
                p: Number(Number(history[i].avg_reservoir_pressure ?? 0).toFixed(1)),
                prodClamp: Number(Number(history[i].producer_bhp_limited_fraction ?? 0).toFixed(2)),
                injClamp: Number(Number(history[i].injector_bhp_limited_fraction ?? 0).toFixed(2)),
            });
        }
    }
    return points;
}

function sample(history: ReturnType<ReservoirSimulator['getRateHistory']>) {
    return history
        .filter((_, index) => index % 10 === 0 || index === history.length - 1)
        .map((point) => ({
            t: Math.round(Number(point.time ?? 0)),
            gor: Number(Number(point.producing_gor ?? 0).toFixed(1)),
            p: Number(Number(point.avg_reservoir_pressure ?? 0).toFixed(1)),
            prodClamp: Number(Number(point.producer_bhp_limited_fraction ?? 0).toFixed(2)),
            injClamp: Number(Number(point.injector_bhp_limited_fraction ?? 0).toFixed(2)),
            oil: Number(Number(point.total_production_oil ?? 0).toFixed(1)),
            gas: Number(Number(point.total_production_gas ?? 0).toFixed(1)),
        }));
}

function producerCellId(payload: ReturnType<typeof buildBenchmarkCreatePayload>) {
    const i = Number(payload.producerI ?? payload.nx - 1);
    const j = Number(payload.producerJ ?? 0);
    const k = Array.isArray(payload.producerKLayers) && payload.producerKLayers.length > 0
        ? Number(payload.producerKLayers[0])
        : 0;
    return i + payload.nx * (j + payload.ny * k);
}

describe('temporary SPE1 trace', () => {
    it('prints coarse and base GOR/pressure behavior', async () => {
        await ensureWasmReady();
        for (const variantKey of ['grid_5', 'grid_10']) {
            const params = getScenarioWithVariantParams('spe1_gas_injection', 'grid', variantKey);
            const payload = buildBenchmarkCreatePayload(params);
            const simulator = configureSimulator(payload);
            const traceWindow = variantKey === 'grid_5'
                ? { start: 1040, end: 1100 }
                : { start: 1440, end: 1520 };
            const cellId = producerCellId(payload);
            const localWindow: Array<Record<string, number>> = [];
            for (let step = 0; step < Number(params.steps); step += 1) {
                simulator.step(Number(params.delta_t_days));
                const point = simulator.getRateHistory().at(-1)!;
                const time = Number(point.time ?? 0);
                if (time >= traceWindow.start && time <= traceWindow.end) {
                    const pressures = simulator.getPressures();
                    const sw = simulator.getSatWater();
                    const so = simulator.getSatOil();
                    const sg = simulator.getSatGas();
                    const rs = simulator.getRs();
                    localWindow.push({
                        t: Math.round(time),
                        gor: Number(Number(point.producing_gor ?? 0).toFixed(1)),
                        pAvg: Number(Number(point.avg_reservoir_pressure ?? 0).toFixed(1)),
                        pProd: Number(Number(pressures[cellId] ?? 0).toFixed(1)),
                        sw: Number(Number(sw[cellId] ?? 0).toFixed(4)),
                        so: Number(Number(so[cellId] ?? 0).toFixed(4)),
                        sg: Number(Number(sg[cellId] ?? 0).toFixed(4)),
                        rs: Number(Number(rs[cellId] ?? 0).toFixed(1)),
                        prodClamp: Number(Number(point.producer_bhp_limited_fraction ?? 0).toFixed(2)),
                    });
                }
            }
            const history = simulator.getRateHistory();
            console.log(`TRACE ${variantKey}`);
            console.log(JSON.stringify({
                first: sample(history).slice(0, 16),
                last: sample(history).slice(-16),
                extrema: turningPoints(history).slice(0, 24),
                localWindow,
            }, null, 2));
            simulator.free();
        }
    });
});