import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../scenarios';
import { integrateRunSeries } from '../../runSeries';
import { getStockTankOilInPlace } from '../../reservoirVolumes';

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

/**
 * 300 days at the scenario's own 10 d step — enough for every rung of the
 * critical-gas-saturation ladder to fall to the 130 bar comparison pressure
 * (the slowest, s_gc = 0.15, reaches it at 250 d). The shipped run continues
 * to 600 d.
 */
const STEPS = 30;

/** Average reservoir pressure the rungs are compared at. */
const COMPARISON_PRESSURE_BAR = 130;

function buildAndRun(params: Params, steps: number): any[] {
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
    (sim as unknown as { setInitialGasSaturation: (s: number) => void })
        .setInitialGasSaturation(Number(params.initialGasSaturation));
    sim.setCapillaryParams(
        Boolean(params.capillaryEnabled) ? Number(params.capillaryPEntry) : 0,
        Number(params.capillaryLambda),
    );
    sim.setGravityEnabled(Boolean(params.gravityEnabled));
    (sim as unknown as { setThreePhaseModeEnabled: (b: boolean) => void }).setThreePhaseModeEnabled(true);
    (sim as unknown as { setGasRedissolutionEnabled: (b: boolean) => void })
        .setGasRedissolutionEnabled(Boolean(params.gasRedissolutionEnabled));
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
    for (let i = 0; i < steps; i++) sim.step(dt);
    return sim.getRateHistorySince(0) as any[];
}

type AtPressure = { time: number; gor: number; gasSaturation: number; oilRecovery: number };

/**
 * The state of a run when the *reservoir* first reaches a given average
 * pressure. Sampling at matched pressure rather than matched time is the whole
 * point: the discarded permeability ladder looked like three different curves
 * on a time axis and turned out to be one curve on three clocks.
 */
function sampleAtPressure(params: Params, history: any[], pressureBar: number): AtPressure | null {
    const index = history.findIndex((h) => Number(h.avg_reservoir_pressure) <= pressureBar);
    if (index < 0) return null;
    const cumulative = integrateRunSeries(history);
    const stockTankOilInPlace = getStockTankOilInPlace(params);
    expect(stockTankOilInPlace).not.toBeNull();
    return {
        time: Number(history[index].time),
        gor: Number(history[index].producing_gor),
        gasSaturation: Number(history[index].avg_gas_saturation),
        oilRecovery: cumulative.oil[index] / stockTankOilInPlace!,
    };
}

describe('gas_drive — critical gas saturation changes the drive, not the clock', () => {
    /**
     * Measured 2026-08-02 at a matched 130 bar average pressure:
     *
     *   s_gc   GOR [m3/m3]   Sg      oil RF
     *   0.02   685           0.114   2.2 %
     *   0.05   474           0.120   3.0 %
     *   0.15   103           0.160   8.9 %
     *
     * Contrast the ladder this replaced, on the same axis: GOR 459/463/460 and
     * Sg 0.106/0.109/0.110 across a 100x permeability range.
     */
    it('the rungs separate at matched average pressure', async () => {
        await ensureWasmReady();

        const samples = ['sgc_low', 'sgc_base', 'sgc_high'].map((variantKey) => {
            const params = getScenarioWithVariantParams('gas_drive', 's_gc', variantKey);
            const sample = sampleAtPressure(params, buildAndRun(params, STEPS), COMPARISON_PRESSURE_BAR);
            expect(sample, `${variantKey} never reached ${COMPARISON_PRESSURE_BAR} bar`).not.toBeNull();
            return { variantKey, s_gc: Number(params.s_gc), ...sample! };
        });

        const [low, base, high] = samples;
        expect([low.s_gc, base.s_gc, high.s_gc]).toEqual([0.02, 0.05, 0.15]);

        // Trapping liberated gas keeps it in the reservoir instead of the
        // tubing: GOR falls monotonically as s_gc rises…
        expect(low.gor).toBeGreaterThan(base.gor);
        expect(base.gor).toBeGreaterThan(high.gor);
        // …and the separation is large, not a rounding difference. The
        // permeability ladder this replaced spanned 1.01x here.
        expect(low.gor / high.gor).toBeGreaterThan(3);

        // Retained gas is gas still in place doing work, so Sg is higher…
        expect(high.gasSaturation).toBeGreaterThan(base.gasSaturation);
        expect(base.gasSaturation).toBeGreaterThan(low.gasSaturation);

        // …and it produces more oil for the same reservoir pressure drop,
        // which is the reservoir-engineering point of the dimension.
        expect(high.oilRecovery).toBeGreaterThan(base.oilRecovery);
        expect(base.oilRecovery).toBeGreaterThan(low.oilRecovery);
        expect(high.oilRecovery / low.oilRecovery).toBeGreaterThan(2);
    }, 300000);
});
