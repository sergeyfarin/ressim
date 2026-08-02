import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import {
    computeGasMaterialBalance,
    fitStraightLineGiip,
    gasFormationVolumeFactor,
    pressureOverZ,
    type GasPvtRow,
} from '../../analytical/gasMaterialBalance';
import { getScenario, getScenarioWithVariantParams } from '../scenarios';
import { listOpmFlowArtifacts } from '../opmFlowArtifacts';

let wasmReady: Promise<unknown> | null = null;

async function ensureWasmReady() {
    wasmReady ??= readFile(new URL('../../ressim/pkg/simulator_bg.wasm', import.meta.url))
        .then((wasmBytes) => initWasm({ module_or_path: wasmBytes }));
    await wasmReady;
}

type VariantRun = {
    /** Gas initially in place from grid volumes and the run's own gas law [Sm³]. */
    volumetricGiip: number;
    /** Gas in place implied by extrapolating the stabilised p/z tail [Sm³]. */
    straightLineGiip: number;
    /** Relative error the straight-line reserves estimate carries [-]. */
    giipError: number;
    /** The same estimate made from the first 40 % of the history instead. */
    earlyGiipError: number;
    /** Recovery factor against the volumetric gas in place at the end of the run. */
    recovery: number;
    finalPressure: number;
    pressureAt500Days: number;
    pressureAt1000Days: number;
    pressureAt2000Days: number;
    /** Produced + remaining, against volumetric — the bookkeeping closure check. */
    inventoryClosure: number;
    warning: string;
};

function runVariant(dimensionKey: string, variantKey: string): VariantRun {
    const params = getScenarioWithVariantParams('dep_gas_pz', dimensionKey, variantKey);
    const nx = Number(params.nx);
    const ny = Number(params.ny);
    const nz = Number(params.nz);
    const porosity = Number(params.reservoirPorosity);
    const table = params.pvtTable as GasPvtRow[];
    const temperature = Number(params.reservoirTemperature);

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
    sim.setPvtTable(table as never);
    sim.setInitialRs(Number(params.initialRs));
    sim.setRockProperties(
        Number(params.rock_compressibility), Number(params.depth_reference),
        Number(params.volume_expansion_o), Number(params.volume_expansion_w),
    );
    sim.setFluidDensities(Number(params.rho_o), Number(params.rho_w));
    sim.setCapillaryParams(0, Number(params.capillaryLambda));
    sim.setGravityEnabled(Boolean(params.gravityEnabled));
    sim.setThreePhaseModeEnabled(true);
    sim.setThreePhaseRelPermProps(
        Number(params.s_wc), Number(params.s_or),
        Number(params.s_gc), Number(params.s_gr), Number(params.s_org),
        Number(params.n_w), Number(params.n_o), Number(params.n_g),
        Number(params.k_rw_max), Number(params.k_ro_max), Number(params.k_rg_max),
    );
    sim.setGasFluidProperties(Number(params.mu_g), Number(params.c_g), Number(params.rho_g));
    sim.setGasRedissolutionEnabled(false);
    sim.setInitialGasSaturation(Number(params.initialGasSaturation));
    // Mirrors the worker: `permMode` decides whether the per-layer arrays or
    // the uniform values are used.
    const perLayer = String(params.permMode) === 'perLayer';
    const fill = (value: number) => new Float64Array(Array.from({ length: nz }, () => value));
    sim.setPermeabilityPerLayer(
        perLayer ? new Float64Array(params.layerPermsX as number[]) : fill(Number(params.uniformPermX)),
        perLayer ? new Float64Array(params.layerPermsY as number[]) : fill(Number(params.uniformPermY)),
        perLayer ? new Float64Array(params.layerPermsZ as number[]) : fill(Number(params.uniformPermZ)),
    );
    sim.setStabilityParams(
        Number(params.max_sat_change_per_step),
        Number(params.max_pressure_change_per_step),
        Number(params.max_well_rate_change_fraction),
    );
    sim.setWellControlModes(String(params.injectorControlMode), String(params.producerControlMode));
    sim.setTargetWellRates(0, 0);
    sim.setWellBhpLimits(Number(params.producerBhp), Number(params.injectorBhp));
    const perforations = (params.producerKLayers as number[] | undefined)?.length
        ? (params.producerKLayers as number[])
        : Array.from({ length: nz }, (_, k) => k);
    for (const k of perforations) {
        sim.add_well(
            Number(params.producerI), Number(params.producerJ), k,
            Number(params.producerBhp), Number(params.well_radius), Number(params.well_skin), false,
        );
    }

    const poreVolume = nx * Number(params.cellDx) * ny * Number(params.cellDy)
        * nz * Number(params.cellDz) * porosity;
    const volumetricGiip = (poreVolume * Number(params.initialGasSaturation))
        / gasFormationVolumeFactor(table, Number(params.initialPressure));
    const dt = Number(params.delta_t_days);

    let cumulativeGas = 0;
    const cumulative: number[] = [];
    const simulatedPOverZ: Array<number | null> = [];
    let finalPressure = Number(params.initialPressure);
    let pressureAt500Days = finalPressure;
    let pressureAt1000Days = finalPressure;
    let pressureAt2000Days = finalPressure;
    for (let step = 0; step < Number(params.steps); step += 1) {
        sim.step(dt);
        const point = sim.getRateHistory().at(-1) as Record<string, number> | undefined;
        cumulativeGas += Math.abs(Number(point?.total_production_gas ?? 0)) * dt;
        finalPressure = Number(point?.avg_reservoir_pressure ?? finalPressure);
        if ((step + 1) * dt <= 500) pressureAt500Days = finalPressure;
        if ((step + 1) * dt <= 1000) pressureAt1000Days = finalPressure;
        if ((step + 1) * dt <= 2000) pressureAt2000Days = finalPressure;
        cumulative.push(cumulativeGas);
        simulatedPOverZ.push(pressureOverZ(table, finalPressure, temperature));
    }

    // Gas still in the ground at the end, cell by cell, at surface conditions.
    const gasSaturations = Array.from(sim.getSatGas());
    const pressures = Array.from(sim.getPressures());
    const cellPoreVolume = poreVolume / (nx * ny * nz);
    let remaining = 0;
    for (let cell = 0; cell < gasSaturations.length; cell += 1) {
        const bg = gasFormationVolumeFactor(table, pressures[cell]);
        if (bg > 0) remaining += (cellPoreVolume * gasSaturations[cell]) / bg;
    }
    const warning = sim.getLastSolverWarning();
    sim.free();

    const fit = fitStraightLineGiip(cumulative, simulatedPOverZ, 1 / 3);
    // The same extrapolation made early in the life of the field, from the
    // first 40 % of the record — which is when a reserves number is actually
    // wanted, and where the compaction bend has not yet flattened out.
    const earlyCount = Math.max(2, Math.floor(cumulative.length * 0.4));
    const earlyFit = fitStraightLineGiip(
        cumulative.slice(0, earlyCount), simulatedPOverZ.slice(0, earlyCount), 1,
    );
    return {
        volumetricGiip,
        straightLineGiip: fit?.giip ?? Number.NaN,
        giipError: (fit?.giip ?? Number.NaN) / volumetricGiip - 1,
        earlyGiipError: (earlyFit?.giip ?? Number.NaN) / volumetricGiip - 1,
        recovery: cumulativeGas / volumetricGiip,
        finalPressure,
        pressureAt500Days,
        pressureAt1000Days,
        pressureAt2000Days,
        inventoryClosure: (cumulativeGas + remaining) / volumetricGiip - 1,
        warning,
    };
}

describe('dep_gas_pz scenario definition', () => {
    it('describes a reservoir with no oil in it', () => {
        const params = getScenario('dep_gas_pz')!.params as Record<string, number>;
        expect(params.initialSaturation + params.initialGasSaturation).toBeCloseTo(1, 12);
        expect(params.initialRs).toBe(0);
        expect(params.s_or).toBe(0);
    });

    it('carries the surface gas density that matches its own PVT table', () => {
        // The trap this case is most exposed to: `rho_g` is a *surface* density
        // and must belong to the gravity the table was generated with, or
        // produced volumes and gas in place stop reconciling. 0.65 gravity air
        // at 1.2232 kg/m3 gives 0.795 kg/m3.
        const params = getScenario('dep_gas_pz')!.params as Record<string, number>;
        expect(params.rho_g).toBeCloseTo(0.65 * 1.2232, 3);
    });

    it('supplies every input the analytical reference needs', () => {
        const params = getScenario('dep_gas_pz')!.params as Record<string, unknown>;
        expect(Array.isArray(params.pvtTable)).toBe(true);
        expect((params.pvtTable as unknown[]).length).toBeGreaterThan(10);
        expect(Number(params.reservoirTemperature)).toBeGreaterThan(0);
        // Without both of these the p/z curve has no z and must not be drawn.
        const balance = computeGasMaterialBalance({
            pvtTable: params.pvtTable as GasPvtRow[],
            reservoirTemperature: Number(params.reservoirTemperature),
            initialPressure: Number(params.initialPressure),
            giip: 1e8,
            cumulativeGas: [0, 5e7],
            initialWaterSaturation: Number(params.initialSaturation),
            c_w: Number(params.c_w),
            c_f: Number(params.rock_compressibility),
        });
        // At 400 bar and 90 C this gas is above its z minimum, so z > 1 and
        // p/z sits *below* p — the opposite of the moderate-pressure case, and
        // a reason the plot is drawn in p/z rather than p.
        expect(balance.initialPOverZ).toBeGreaterThan(0);
        expect(balance.initialPOverZ).toBeLessThan(Number(params.initialPressure));
        expect(balance.points[1].pOverZ).toBeCloseTo(balance.initialPOverZ * 0.5, 6);
    });
});

describe('dep_gas_pz measured behaviour', () => {
    it('closes its own gas inventory, which is what the p/z line is measuring', async () => {
        await ensureWasmReady();
        const base = runVariant('pore_compressibility', 'cf_normal');
        const opm = listOpmFlowArtifacts().find((artifact) => artifact.caseKey === 'dep_gas_pz')!;
        const opmPressure = opm.series.find((series) => series.curveKey === 'opm-avg-pressure')!;
        const nearest = (day: number) => opmPressure.data.reduce((best, point) => (
            Math.abs(point.x - day) < Math.abs(best.x - day) ? point : best
        ));
        for (const [day, ressimPressure] of [
            [500, base.pressureAt500Days],
            [1000, base.pressureAt1000Days],
            [2000, base.pressureAt2000Days],
        ] as const) {
            const opmPoint = nearest(day);
            expect(
                Math.abs(ressimPressure - opmPoint.y) / opmPoint.y,
                `average pressure at ${day} d`,
            ).toBeLessThan(0.1);
        }
        expect(base.warning).toBe('');
        // Produced plus remaining, against what the volumetrics said was there.
        // Not expected to be exactly zero: compaction and connate-water
        // expansion legitimately deliver gas the initial-volume estimate never
        // counted, which is this case's whole subject. At the base
        // compressibility that is a couple of percent; a surface gas density
        // that did not match the PVT table's gravity would put it in double
        // figures, which is the failure this guards.
        expect(Math.abs(base.inventoryClosure)).toBeLessThan(0.04);

        // And the residual is that physics, not noise: it scales with c_f, by
        // about what compaction can physically deliver.
        const geopressured = runVariant('pore_compressibility', 'cf_geopressured');
        expect(geopressured.inventoryClosure).toBeGreaterThan(base.inventoryClosure);
        expect(geopressured.inventoryClosure).toBeLessThan(base.inventoryClosure + 0.02);
    }, 600_000);

    it('bends the plot in mid-life and lets it straighten again by abandonment', async () => {
        await ensureWasmReady();
        const ladder = ['cf_normal', 'cf_moderate', 'cf_high', 'cf_geopressured']
            .map((key) => runVariant('pore_compressibility', key));
        for (const run of ladder) expect(run.warning).toBe('');

        // Compaction delivers extra gas, monotonically in c_f.
        for (let index = 1; index < ladder.length; index += 1) {
            expect(ladder[index].recovery, `rung ${index}`)
                .toBeGreaterThan(ladder[index - 1].recovery);
        }

        // Read early, the reserves error grows with compressibility.
        for (let index = 1; index < ladder.length; index += 1) {
            expect(ladder[index].earlyGiipError, `early rung ${index}`)
                .toBeGreaterThan(ladder[index - 1].earlyGiipError);
        }
        expect(ladder[0].earlyGiipError).toBeLessThan(0.01);
        expect(ladder.at(-1)!.earlyGiipError).toBeGreaterThan(0.02);

        // Read to abandonment, it does not — the bend has straightened out.
        for (const run of ladder) expect(run.giipError).toBeLessThan(0.01);
    }, 600_000);

    it('over-reads reserves when the well is draining one compartment', async () => {
        await ensureWasmReady();
        const connected = runVariant('connectivity', 'conn_one_tank');
        const sealed = runVariant('connectivity', 'conn_sealed');
        for (const run of [connected, sealed]) expect(run.warning).toBe('');
        console.log('connected', JSON.stringify(connected));
        console.log('sealed', JSON.stringify(sealed));
        expect(sealed.recovery).toBeLessThan(connected.recovery);
    }, 600_000);

    it('changes the answer with how much of the history you have', async () => {
        await ensureWasmReady();
        const full = runVariant('abandonment', 'aband_30');
        const early = runVariant('abandonment', 'aband_200');
        for (const run of [full, early]) expect(run.warning).toBe('');
        console.log('aband30', JSON.stringify(full));
        console.log('aband200', JSON.stringify(early));
        expect(early.recovery).toBeLessThan(full.recovery);
    }, 600_000);
});

/** Cumulative gas produced by a bundled OPM Flow run of this case [Sm³]. */
function opmCumulativeGas(caseKey: string): number {
    const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === caseKey)!;
    expect(artifact.status, caseKey).toBe('parsed');
    return artifact.xAxis!.cumulativeGasSm3!.at(-1)!;
}

describe('dep_gas_pz against OPM Flow', () => {
    it('agrees on the base case, where the pore volume barely moves', () => {
        // ResSim 131.6e6 (measured above), OPM 129.9e6, hand balance 131.2e6.
        const opm = opmCumulativeGas('dep_gas_pz');
        expect(opm).toBeGreaterThan(1.25e8);
        expect(opm).toBeLessThan(1.35e8);
    });

    /**
     * The compaction term, checked against an independent simulator.
     *
     * This is the assertion that found the defect fixed on 2026-08-01: ResSim
     * used to report a 10.7 % compaction increment where OPM Flow and a hand
     * dry-gas material balance both said about 1.9 %, because the pore volume
     * was referenced to the previous timestep's pressure and so never
     * accumulated. It now agrees.
     */
    it('agrees with OPM Flow on how much gas compaction releases', async () => {
        await ensureWasmReady();
        const opmIncrement = opmCumulativeGas('dep_gas_pz_geopressured') / opmCumulativeGas('dep_gas_pz') - 1;
        expect(opmIncrement).toBeGreaterThan(0.01);
        expect(opmIncrement).toBeLessThan(0.03);

        const base = runVariant('pore_compressibility', 'cf_normal');
        const geopressured = runVariant('pore_compressibility', 'cf_geopressured');
        const resSimIncrement = geopressured.recovery / base.recovery - 1;

        // Same order, and within half a percentage point of an independent
        // simulator on the identical deck.
        expect(resSimIncrement).toBeGreaterThan(0.005);
        expect(Math.abs(resSimIncrement - opmIncrement)).toBeLessThan(0.006);
    }, 600_000);
});
