import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../scenarios';
import { calculateMaterialBalance } from '../../analytical/materialBalance';
import { integrateRunSeries } from '../../runSeries';
import { getInitialSaturations, getPoreVolume } from '../../reservoirVolumes';

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

const BUBBLE_POINT_BAR = 150;

/**
 * 120 days at the scenario's own 0.75 d step. Long enough to carry the slower
 * PVT variant across the bubble point (88 d) and past the peak of the
 * average-pressure gap (34 d), which is everything the assertions below need;
 * the shipped run continues to 225 d.
 */
const STEPS = 160;

type RunPoint = { time: number; avgPressure: number };

function buildAndRun(params: Params, steps: number): { points: RunPoint[]; history: any[] } {
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

    // The producer is rate-controlled here, so it is the rate targets that
    // matter and the BHP is only a floor. `sim.worker.ts` opens the BHP window
    // to zero for a rate-controlled producer; mirror that, or the well is
    // silently BHP-limited and the constant-withdrawal exhibit disappears.
    sim.setWellControlModes(String(params.injectorControlMode), String(params.producerControlMode));
    const rateControlled = String(params.producerControlMode) === 'rate';
    sim.setTargetWellRates(0, Number(params.targetProducerRate ?? 0));
    (sim as unknown as { setTargetWellSurfaceRates: (i: number, p: number) => void })
        .setTargetWellSurfaceRates(0, Number(params.targetProducerSurfaceRate ?? 0));
    const producerBhp = Number(params.producerBhp);
    sim.setWellBhpLimits(rateControlled ? 0 : producerBhp, Number(params.initialPressure));
    sim.add_well(
        Number(params.producerI), Number(params.producerJ), 0,
        producerBhp, Number(params.well_radius), Number(params.well_skin), false,
    );

    const dt = Number(params.delta_t_days);
    for (let i = 0; i < steps; i++) sim.step(dt);

    const history = sim.getRateHistorySince(0) as any[];
    const points = history.map((h) => ({
        time: Number(h.time),
        avgPressure: Number(h.avg_reservoir_pressure),
    }));
    return { points, history };
}

/** First report time at which the volumetric average has fallen to the bubble point. */
function timeToBubblePoint(points: RunPoint[]): number | null {
    const crossing = points.find((p) => p.avgPressure <= BUBBLE_POINT_BAR);
    return crossing ? crossing.time : null;
}

/**
 * N_mbe / N_volumetric, the same Havlena-Odeh ratio the chart's
 * `mbe_ooip` panel draws — see `computeMbeDiagnostics`.
 */
function materialBalanceRatios(params: Params, history: any[]): number[] {
    const cumulative = integrateRunSeries(history);
    const saturations = getInitialSaturations(params);
    const result = calculateMaterialBalance({
        initialPressure: Number(params.initialPressure),
        initialWaterSaturation: saturations.water,
        initialGasSaturation: saturations.gas,
        porosity: Number(params.reservoirPorosity),
        poreVolume: getPoreVolume(params),
        c_w: Number(params.c_w),
        c_rock: Number(params.rock_compressibility),
        pvtMode: 'black-oil',
        Bo_constant: Number(params.volume_expansion_o),
        Bw_constant: Number(params.volume_expansion_w),
        c_o: Number(params.c_o),
        pvtTable: params.pvtTable as any,
        apiGravity: 35,
        gasSpecificGravity: 0.75,
        reservoirTemperature: 80,
        bubblePoint: BUBBLE_POINT_BAR,
        pressureHistory: history.map((h) => Number(h.avg_reservoir_pressure)),
        cumulativeOilSC: cumulative.oil,
        cumulativeGasSC: cumulative.gas,
        cumulativeWaterSC: cumulative.water,
        timeHistory: history.map((h) => Number(h.time)),
    });
    return result.points
        .map((pt) => (pt.N_mbe === null ? null : pt.N_mbe / result.volumetricOoip))
        .filter((r): r is number => r !== null);
}

describe('dep_pvt — PVT-table representation risk', () => {
    it('the two PVT tables share an identical calibration point at and below the bubble point', () => {
        const correlationParams = getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_correlation');
        const labParams = getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_lab_report');

        const correlationTable = correlationParams.pvtTable as Array<{ p_bar: number; rs_m3m3: number; bo_m3m3: number }>;
        const labTable = labParams.pvtTable as Array<{ p_bar: number; rs_m3m3: number; bo_m3m3: number }>;

        for (let i = 0; i < correlationTable.length; i++) {
            if (correlationTable[i].p_bar > BUBBLE_POINT_BAR) continue;
            expect(labTable[i].p_bar).toBeCloseTo(correlationTable[i].p_bar, 9);
            expect(labTable[i].rs_m3m3).toBeCloseTo(correlationTable[i].rs_m3m3, 9);
            expect(labTable[i].bo_m3m3).toBeCloseTo(correlationTable[i].bo_m3m3, 9);
        }

        // Above the bubble point, Bo must diverge (that's the whole point).
        const aboveBp = correlationTable.findIndex((row) => row.p_bar > BUBBLE_POINT_BAR + 20);
        expect(aboveBp).toBeGreaterThan(-1);
        expect(labTable[aboveBp].bo_m3m3).not.toBeCloseTo(correlationTable[aboveBp].bo_m3m3, 4);
        expect(labTable[aboveBp].bo_m3m3).toBeLessThan(correlationTable[aboveBp].bo_m3m3);
    });

    /**
     * The case's central quantitative claim. Under constant-rate withdrawal an
     * undersaturated reservoir depletes at dP/dt = -q_res/(V_p·c_t), so the
     * time to reach the bubble point scales with c_t = c_o·S_o + c_w·S_w +
     * c_rock. Measured 2026-08-02: c_t is 9.13e-5 and 2.263e-4 /bar (ratio
     * 2.48) and the crossings land at 36 d and 88 d (ratio 2.4).
     *
     * The tolerance is deliberately loose. The claim under test is that the
     * *storage* argument governs, not that the simulator reproduces a
     * hand-integrated constant; S_o and hence c_t drift as oil is withdrawn.
     */
    it('unmeasured undersaturated compressibility rescales the depletion clock', async () => {
        await ensureWasmReady();

        const correlationParams = getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_correlation');
        const labParams = getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_lab_report');

        const correlation = buildAndRun(correlationParams, STEPS);
        const lab = buildAndRun(labParams, STEPS);

        const tCorrelation = timeToBubblePoint(correlation.points);
        const tLab = timeToBubblePoint(lab.points);

        // Both variants must actually reach the saturated branch inside the
        // run — the previous design promised reconvergence and never delivered
        // it, ending 55 bar above the bubble point.
        expect(tCorrelation).not.toBeNull();
        expect(tLab).not.toBeNull();
        expect(tCorrelation!).toBeGreaterThan(25);
        expect(tCorrelation!).toBeLessThan(50);
        expect(tLab!).toBeGreaterThan(70);
        expect(tLab!).toBeLessThan(105);

        const c_t = (c_o: number) => c_o * 0.9 + 3e-6 * 0.1 + 1e-6;
        const expectedRatio = c_t(Number(labParams.c_o)) / c_t(Number(correlationParams.c_o));
        expect(tLab! / tCorrelation!).toBeGreaterThan(0.8 * expectedRatio);
        expect(tLab! / tCorrelation!).toBeLessThan(1.2 * expectedRatio);
    }, 300000);

    /**
     * Guards the redesign's premise. This case's headline chart is Avg
     * Pressure and its on-chart self-check is the Havlena-Odeh ratio, and both
     * are only meaningful if one pressure describes the reservoir. The
     * BHP-controlled 0.5 mD predecessor read 2.5-7.8 here instead of 1: the
     * near-well cells liberated gas while the average was still
     * undersaturated, and the tank under-counted the reservoir's energy
     * eight-fold. Measured 2026-08-02 on the shipped design: 0.999-1.017.
     */
    it('closes a tank material balance, so its average pressure is representative', async () => {
        await ensureWasmReady();

        for (const variant of ['pvt_correlation', 'pvt_lab_report']) {
            const params = getScenarioWithVariantParams('dep_pvt', 'pvt_model', variant);
            const { history } = buildAndRun(params, STEPS);
            const ratios = materialBalanceRatios(params, history);

            expect(ratios.length).toBeGreaterThan(0);
            for (const ratio of ratios) {
                expect(ratio).toBeGreaterThan(0.95);
                expect(ratio).toBeLessThan(1.05);
            }
        }
    }, 300000);

    /**
     * The reconvergence the description promises. Below the bubble point both
     * variants are the same fluid — identical Rs(P), Bo(P) — so the
     * average-pressure gap peaks as the faster variant crosses and then
     * closes. Measured 2026-08-02 over the full 225 d run: 77.5 bar at
     * t = 35 d, down to 14 bar at t = 169 d and 16.8 bar at the end.
     */
    it('the average-pressure gap peaks near the bubble point and then closes', async () => {
        await ensureWasmReady();

        const correlation = buildAndRun(getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_correlation'), STEPS);
        const lab = buildAndRun(getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_lab_report'), STEPS);

        const gap = correlation.points.map((p, i) => Math.abs(lab.points[i].avgPressure - p.avgPressure));
        const peakIndex = gap.indexOf(Math.max(...gap));
        const peak = gap[peakIndex];
        const final = gap[gap.length - 1];

        // The peak is a real separation, not numerical noise…
        expect(peak).toBeGreaterThan(50);
        // …it happens while the faster variant is crossing the bubble point…
        expect(correlation.points[peakIndex].time).toBeGreaterThan(20);
        expect(correlation.points[peakIndex].time).toBeLessThan(60);
        // …and it has closed substantially by the end of the window.
        expect(final).toBeLessThan(0.5 * peak);
    }, 300000);
});
