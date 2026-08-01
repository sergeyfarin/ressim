import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { computeVerticalSweep } from '../../analytical/sweepEfficiency';
import { getScenario, getScenarioWithVariantParams } from '../scenarios';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

type VariantRun = {
    breakthroughPvi: number;
    recoveryAtOnePvi: number;
    maxPvi: number;
    warning: string;
};

/** Runs one variant of the layered section against the wasm core. */
function runVariant(dimensionKey: string, variantKey: string): VariantRun {
    const params = getScenarioWithVariantParams('sweep_crossflow', dimensionKey, variantKey);
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const nz = Number(params.nz);
    const porosity = Number(params.reservoirPorosity);
    const sim = new ReservoirSimulator(nx, ny, nz, porosity);

    sim.setFimEnabled(Boolean(params.fimEnabled));
    sim.setCellDimensions(Number(params.cellDx), Number(params.cellDy), Number(params.cellDz));
    sim.setRelPermProps(
        Number(params.s_wc), Number(params.s_or), Number(params.n_w), Number(params.n_o),
        Number(params.k_rw_max), Number(params.k_ro_max),
    );
    sim.setInitialPressure(Number(params.initialPressure));
    sim.setInitialSaturation(Number(params.initialSaturation));
    sim.setFluidProperties(Number(params.mu_o), Number(params.mu_w));
    sim.setFluidCompressibilities(Number(params.c_o), Number(params.c_w));
    sim.setRockProperties(
        Number(params.rock_compressibility), Number(params.depth_reference),
        Number(params.volume_expansion_o), Number(params.volume_expansion_w),
    );
    sim.setFluidDensities(Number(params.rho_o), Number(params.rho_w));
    sim.setCapillaryParams(
        params.capillaryEnabled === true ? Number(params.capillaryPEntry) : 0,
        Number(params.capillaryLambda),
    );
    sim.setGravityEnabled(Boolean(params.gravityEnabled));
    sim.setPermeabilityPerLayer(
        new Float64Array(params.layerPermsX as number[]),
        new Float64Array(params.layerPermsY as number[]),
        new Float64Array(params.layerPermsZ as number[]),
    );
    sim.setStabilityParams(
        Number(params.max_sat_change_per_step),
        Number(params.max_pressure_change_per_step),
        Number(params.max_well_rate_change_fraction),
    );
    sim.setWellControlModes(String(params.injectorControlMode), String(params.producerControlMode));
    sim.setTargetWellRates(Number(params.targetInjectorRate), Number(params.targetProducerRate));
    sim.setWellBhpLimits(Number(params.producerBhp), Number(params.injectorBhp));
    for (let k = 0; k < nz; k += 1) {
        sim.add_well(0, 0, k, Number(params.injectorBhp), Number(params.well_radius), Number(params.well_skin), true);
    }
    for (let k = 0; k < nz; k += 1) {
        sim.add_well(
            Number(params.producerI), 0, k,
            Number(params.producerBhp), Number(params.well_radius), Number(params.well_skin), false,
        );
    }

    const poreVolume = nx * Number(params.cellDx) * ny * Number(params.cellDy)
        * nz * Number(params.cellDz) * porosity;
    const oilInPlace = poreVolume * (1 - Number(params.s_wc));
    const dt = Number(params.delta_t_days);

    let cumulativeOil = 0;
    let cumulativeInjection = 0;
    let breakthroughPvi = Number.NaN;
    let recoveryAtOnePvi = 0;
    let pvi = 0;
    for (let step = 0; step < Number(params.steps); step += 1) {
        sim.step(dt);
        const point = sim.getRateHistory().at(-1) as Record<string, number> | undefined;
        const oilRate = Math.abs(Number(point?.total_production_oil ?? 0));
        const liquidRate = Math.abs(Number(point?.total_production_liquid ?? 0));
        const waterRate = Math.max(0, liquidRate - oilRate);
        cumulativeOil += oilRate * dt;
        cumulativeInjection += Math.abs(Number(point?.total_injection_reservoir ?? 0)) * dt;
        pvi = cumulativeInjection / poreVolume;
        if (!Number.isFinite(breakthroughPvi) && liquidRate > 0 && waterRate / liquidRate > 0.01) {
            breakthroughPvi = pvi;
        }
        if (recoveryAtOnePvi === 0 && pvi >= 1) {
            recoveryAtOnePvi = cumulativeOil / oilInPlace;
        }
    }
    const warning = sim.getLastSolverWarning();
    sim.free();
    return { breakthroughPvi, recoveryAtOnePvi, maxPvi: pvi, warning };
}

describe('sweep_crossflow scenario definition', () => {
    it('varies only vertical permeability along the communication ladder', () => {
        const scenario = getScenario('sweep_crossflow');
        expect(scenario).not.toBeNull();
        const dimension = scenario!.sensitivities.find((d) => d.key === 'vertical_communication')!;
        expect(dimension.analyticalOverlayMode).toBe('shared');

        for (const variant of dimension.variants) {
            expect(Object.keys(variant.paramPatch).filter((k) => k !== 'layerPermsZ'), variant.key).toEqual([]);
            expect(variant.affectsAnalytical, variant.key).toBe(false);
        }
    });

    it('leaves the analytical sweep model blind to vertical permeability', () => {
        // The claim the whole case rests on: kv is not an input to the
        // correlation, so every rung of the ladder has the same reference.
        const scenario = getScenario('sweep_crossflow');
        const mobilityRatio = (0.25 / 0.5) / (1 / 1);
        const curves = scenario!.sensitivities
            .find((d) => d.key === 'vertical_communication')!.variants
            .map((variant) => {
                const params = getScenarioWithVariantParams('sweep_crossflow', 'vertical_communication', variant.key);
                return computeVerticalSweep(params.layerPermsX as number[], Number(params.cellDz), mobilityRatio);
            });
        for (const curve of curves.slice(1)) {
            expect(curve.vdp).toBeCloseTo(curves[0].vdp, 12);
            expect(curve.curve.map((p) => p.efficiency)).toEqual(curves[0].curve.map((p) => p.efficiency));
        }
    });

    it('keeps every mobility variant on the same pore volumes injected per report step', () => {
        const scenario = getScenario('sweep_crossflow');
        for (const variant of scenario!.sensitivities.find((d) => d.key === 'mobility_crossover')!.variants) {
            const params = getScenarioWithVariantParams('sweep_crossflow', 'mobility_crossover', variant.key);
            // Report step scales with oil viscosity, since the rate under a
            // fixed drawdown scales inversely with it.
            expect(Number(params.delta_t_days) / Number(params.mu_o), variant.key).toBeGreaterThan(0.15);
            expect(Number(params.delta_t_days) / Number(params.mu_o), variant.key).toBeLessThan(1.1);
        }
    });
});

describe('sweep_crossflow measured behaviour', () => {
    it('walks away from a fixed analytical reference as the layers open up', async () => {
        await ensureWasmReady();
        const ladder = ['kv_sealed', 'kv_001', 'kv_01', 'kv_base', 'kv_isotropic']
            .map((key) => runVariant('vertical_communication', key));

        for (const run of ladder) {
            expect(run.warning).toBe('');
            expect(run.maxPvi).toBeGreaterThan(1.2);
        }

        // Breakthrough moves out monotonically over the whole ladder.
        for (let i = 1; i < ladder.length; i += 1) {
            expect(ladder[i].breakthroughPvi, `rung ${i}`)
                .toBeGreaterThan(ladder[i - 1].breakthroughPvi);
        }
        expect(ladder[0].breakthroughPvi).toBeCloseTo(0.427, 2);
        expect(ladder.at(-1)!.breakthroughPvi).toBeCloseTo(0.599, 2);
        // A 40 % shift from a parameter the correlation cannot see.
        expect(ladder.at(-1)!.breakthroughPvi / ladder[0].breakthroughPvi).toBeGreaterThan(1.35);

        // Recovery rises and then saturates rather than continuing to climb.
        expect(ladder[0].recoveryAtOnePvi).toBeCloseTo(0.735, 2);
        expect(ladder[3].recoveryAtOnePvi).toBeCloseTo(0.783, 2);
        expect(ladder.at(-1)!.recoveryAtOnePvi).toBeLessThan(ladder[3].recoveryAtOnePvi);
    }, 300_000);

    it('reverses the sign of the crossflow benefit between favourable and unfavourable mobility', async () => {
        await ensureWasmReady();
        const pairs = [
            ['mob_fav_sealed', 'mob_fav_open'],
            ['mob_unit_sealed', 'mob_unit_open'],
            ['mob_unfav_sealed', 'mob_unfav_open'],
        ] as const;
        const deltas = pairs.map(([sealed, open]) => {
            const a = runVariant('mobility_crossover', sealed);
            const b = runVariant('mobility_crossover', open);
            for (const run of [a, b]) {
                expect(run.warning).toBe('');
                expect(run.maxPvi).toBeGreaterThan(1.2);
            }
            return b.recoveryAtOnePvi - a.recoveryAtOnePvi;
        });

        expect(deltas[0]).toBeGreaterThan(0.03);
        expect(Math.abs(deltas[1])).toBeLessThan(0.005);
        expect(deltas[2]).toBeLessThan(-0.01);
        // Monotone in mobility ratio, and it crosses zero inside the range.
        expect(deltas[0]).toBeGreaterThan(deltas[1]);
        expect(deltas[1]).toBeGreaterThan(deltas[2]);
    }, 300_000);

    it('shows capillary crossflow as super-additive with a path and inert without one', async () => {
        await ensureWasmReady();
        const sealedDry = runVariant('capillary_crossflow', 'cap_sealed_dry');
        const sealedPc = runVariant('capillary_crossflow', 'cap_sealed_pc');
        const openDry = runVariant('capillary_crossflow', 'cap_open_dry');
        const openPc = runVariant('capillary_crossflow', 'cap_open_pc');
        for (const run of [sealedDry, sealedPc, openDry, openPc]) {
            expect(run.warning).toBe('');
            expect(run.maxPvi).toBeGreaterThan(1.2);
        }

        // Capillarity alone, with the layers sealed, does nothing measurable.
        const capillaryAlone = sealedPc.recoveryAtOnePvi - sealedDry.recoveryAtOnePvi;
        expect(Math.abs(capillaryAlone)).toBeLessThan(0.002);

        // With a path, it is worth another point of recovery on top of the
        // viscous exchange — more than the two effects add to separately.
        const viscousAlone = openDry.recoveryAtOnePvi - sealedDry.recoveryAtOnePvi;
        const both = openPc.recoveryAtOnePvi - sealedDry.recoveryAtOnePvi;
        expect(viscousAlone).toBeGreaterThan(0.03);
        expect(both).toBeGreaterThan(viscousAlone + capillaryAlone + 0.005);
        expect(openPc.breakthroughPvi).toBeGreaterThan(openDry.breakthroughPvi);
    }, 300_000);
});
