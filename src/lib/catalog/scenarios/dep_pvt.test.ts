import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import initWasm, { ReservoirSimulator } from '../../ressim/pkg/simulator.js';
import { getScenarioWithVariantParams } from '../scenarios';
import { calculateMaterialBalance } from '@ressim/analytical/materialBalance';
import { integrateRunSeries } from '@ressim/quantities/runSeries';
import { getInitialSaturations, getPoreVolume, getStockTankOilInPlace } from '@ressim/quantities/reservoirVolumes';
import { listOpmFlowArtifacts } from '../opmFlowArtifacts';
import depPvtDeckTables from '../../../../opm/reference-decks/small-direct/dep-pvt-tables.json';

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

type RunPoint = { time: number; avgPressure: number; gor: number; cumOil: number };

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
    const cumulative = integrateRunSeries(history);
    const points = history.map((h, i) => ({
        time: Number(h.time),
        avgPressure: Number(h.avg_reservoir_pressure),
        gor: Number(h.producing_gor),
        cumOil: cumulative.oil[i],
    }));
    return { points, history };
}

type Run = { params: Params; points: RunPoint[]; history: any[] };
const runs = new Map<string, Run>();

/**
 * One rung, run over the scenario's own 480 x 0.75 d, cached: several tests read the same run.
 * `null` is the base case, which is a rung of both ladders.
 */
function run(dimension: string | null, variant: string | null, patch: Params = {}): Run {
    const key = JSON.stringify([dimension, variant, patch]);
    if (!runs.has(key)) {
        const params = { ...getScenarioWithVariantParams('dep_pvt', dimension, variant), ...patch };
        runs.set(key, { params, ...buildAndRun(params, Number(params.steps)) });
    }
    return runs.get(key)!;
}

/** The first report at which the volumetric average has fallen to `pressure`. */
function atPressure(r: Run, pressure: number): { time: number; gor: number; recoveryPct: number } {
    const point = r.points.find((p) => p.avgPressure <= pressure);
    if (!point) throw new Error(`run never reaches ${pressure} bar`);
    return {
        time: point.time,
        gor: point.gor,
        recoveryPct: (100 * point.cumOil) / getStockTankOilInPlace(r.params)!,
    };
}

/** Average pressure at `time`, interpolated between reports: two runs' reports do not line up. */
function pressureAt(points: readonly { time: number; avgPressure: number }[], time: number): number {
    const next = points.findIndex((point) => point.time >= time);
    if (next <= 0) return points[Math.max(next, 0)].avgPressure;
    const [a, b] = [points[next - 1], points[next]];
    return a.avgPressure + ((time - a.time) / (b.time - a.time)) * (b.avgPressure - a.avgPressure);
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

const PVT_MODEL = ['pvt_correlation', 'pvt_lab_report'] as const;
const SATURATED_PVT = ['sat_petrosky_farshad', 'sat_standing', 'sat_al_marhoun'] as const;
/** Matched average pressures below the bubble point at which both ladders are read. */
const BELOW_PB_BAR = [130, 110, 90];

type TableRow = { p_bar: number; rs_m3m3: number; bo_m3m3: number };

describe('dep_pvt — PVT-table representation risk', () => {
    it('every table shares the one calibration point, and each ladder varies one side of it', () => {
        const table = (dimension: string, variant: string) =>
            getScenarioWithVariantParams('dep_pvt', dimension, variant).pvtTable as TableRow[];
        const correlation = table('pvt_model', 'pvt_correlation');
        const lab = table('pvt_model', 'pvt_lab_report');
        const [petrosky, standing, alMarhoun] = SATURATED_PVT.map((variant) => table('saturated_pvt', variant));
        expect(standing).toEqual(correlation);

        correlation.forEach((row, i) => {
            if (row.p_bar <= BUBBLE_POINT_BAR) expect(lab[i]).toEqual(row);
            if (row.p_bar >= BUBBLE_POINT_BAR) {
                expect(petrosky[i]).toEqual(row);
                expect(alMarhoun[i]).toEqual(row);
            }
        });

        // Above the bubble point the lab report's Bo must diverge (that's the whole point)…
        const aboveBp = correlation.findIndex((row) => row.p_bar > BUBBLE_POINT_BAR + 20);
        expect(aboveBp).toBeGreaterThan(-1);
        expect(lab[aboveBp].bo_m3m3).toBeLessThan(correlation[aboveBp].bo_m3m3 - 1e-3);
        // …and below it the saturated correlations must hold their gas in a fixed order.
        const belowBp = correlation.findIndex((row) => row.p_bar > 100);
        expect(petrosky[belowBp].rs_m3m3).toBeGreaterThan(standing[belowBp].rs_m3m3 + 1);
        expect(standing[belowBp].rs_m3m3).toBeGreaterThan(alMarhoun[belowBp].rs_m3m3 + 1);
    });

    /**
     * The case's central quantitative claim. Under constant-rate withdrawal an
     * undersaturated reservoir depletes at dP/dt = -q_res/(V_p·c_t), so the
     * time to reach the bubble point scales with c_t = c_o·S_o + c_w·S_w +
     * c_rock. With c_rock = 5e-5 /bar (#26), c_t is 1.40e-4 and 2.75e-4 /bar
     * (ratio 1.96) and the crossings land at 46.5 d and 91.5 d (ratio 1.97).
     * History: at c_rock = 1e-6 they were 36 / 88 d on the lean pre-#60 fluid
     * and 30.4 / 75.75 d (2.49x) on the corrected one.
     *
     * The tolerance is deliberately loose. The claim under test is that the
     * *storage* argument governs, not that the simulator reproduces a
     * hand-integrated constant; S_o and hence c_t drift as oil is withdrawn.
     */
    it('unmeasured undersaturated compressibility rescales the depletion clock', async () => {
        await ensureWasmReady();
        const [correlation, lab] = PVT_MODEL.map((variant) => run('pvt_model', variant));
        const tCorrelation = timeToBubblePoint(correlation.points);
        const tLab = timeToBubblePoint(lab.points);

        expect(tCorrelation).not.toBeNull();
        expect(tLab).not.toBeNull();
        expect(tCorrelation!).toBeGreaterThan(40);
        expect(tCorrelation!).toBeLessThan(55);
        expect(tLab!).toBeGreaterThan(80);
        expect(tLab!).toBeLessThan(105);

        const c_t = (params: Params) =>
            Number(params.c_o) * 0.9 + Number(params.c_w) * 0.1 + Number(params.rock_compressibility);
        const expectedRatio = c_t(lab.params) / c_t(correlation.params);
        expect(expectedRatio).toBeGreaterThan(1.9);
        expect(expectedRatio).toBeLessThan(2.0);
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
     * eight-fold. Measured on the #26 design (c_rock 5e-5, 360 d): 0.9985-1.0134
     * across all four rungs.
     */
    it('closes a tank material balance on every rung, so its average pressure is representative', async () => {
        await ensureWasmReady();
        const rungs = [
            ...PVT_MODEL.map((variant) => run('pvt_model', variant)),
            ...SATURATED_PVT.map((variant) => run('saturated_pvt', variant)),
        ];
        for (const r of rungs) {
            const ratios = materialBalanceRatios(r.params, r.history);
            expect(ratios.length).toBeGreaterThan(0);
            for (const ratio of ratios) {
                expect(ratio).toBeGreaterThan(0.95);
                expect(ratio).toBeLessThan(1.05);
            }
        }
    }, 300000);

    /**
     * The reconvergence the `pvt_model` description promises, read where it
     * holds: at matched pressure. Below the bubble point both rungs are the
     * same fluid, so the producing GOR agrees (measured within 1% at 130, 110
     * and 90 bar) and recovery differs only by the oil the lab-report fluid
     * expanded out above the bubble point, a constant 1.70-1.77 points.
     *
     * In time the lab-report run is ~45 d behind. The average-pressure gap
     * peaks at 64.1 bar at t = 45 d, narrows to 6.7 bar at t = 134 d, and then
     * widens again to 25 bar at 360 d, because the same lag costs more pressure
     * as depletion accelerates. (On the 225 d, c_rock = 1e-6 design before #26
     * it peaked at 77.6 bar and closed to 6.8.)
     */
    it('below the bubble point the two compressibility rungs are one fluid', async () => {
        await ensureWasmReady();
        const [correlation, lab] = PVT_MODEL.map((variant) => run('pvt_model', variant));

        const offsets = BELOW_PB_BAR.map((pressure) => {
            const [c, l] = [atPressure(correlation, pressure), atPressure(lab, pressure)];
            expect(Math.abs(l.gor / c.gor - 1), `GOR at ${pressure} bar`).toBeLessThan(0.03);
            return l.recoveryPct - c.recoveryPct;
        });
        for (const offset of offsets) {
            expect(offset).toBeGreaterThan(1.2);
            expect(offset).toBeLessThan(2.2);
        }
        expect(Math.max(...offsets) - Math.min(...offsets)).toBeLessThan(0.3);

        const gap = correlation.points.map((point) => ({
            time: point.time,
            gap: Math.abs(pressureAt(lab.points, point.time) - point.avgPressure),
        }));
        const peak = gap.reduce((a, b) => (b.gap > a.gap ? b : a));
        const narrowest = gap.filter((g) => g.time > peak.time).reduce((a, b) => (b.gap < a.gap ? b : a));
        expect(peak.gap).toBeGreaterThan(50);
        expect(peak.time).toBeGreaterThan(30);
        expect(peak.time).toBeLessThan(60);
        expect(narrowest.gap).toBeLessThan(0.2 * peak.gap);
    }, 300000);

    /**
     * Why the degree of undersaturation is not a ladder of its own (#26): the
     * undersaturated leg identifies only c_t·(p_i - P_b). The lab-report table
     * started at the initial pressure that banks the same expansion as the
     * base case reproduces it: P_b at 47.25 d against 46.5 d, recovery within
     * 0.02 points and GOR within 0.5% at every matched pressure.
     */
    it('a lower initial pressure on the stiffer table reproduces the base run', async () => {
        await ensureWasmReady();
        const base = run(null, null);
        const lab = getScenarioWithVariantParams('dep_pvt', 'pvt_model', 'pvt_lab_report');
        const c_t = (params: Params) =>
            Number(params.c_o) * 0.9 + Number(params.c_w) * 0.1 + Number(params.rock_compressibility);
        const banked = c_t(base.params) * (Number(base.params.initialPressure) - BUBBLE_POINT_BAR);
        const twin = run('pvt_model', 'pvt_lab_report', { initialPressure: BUBBLE_POINT_BAR + banked / c_t(lab) });

        expect(Math.abs(timeToBubblePoint(twin.points)! - timeToBubblePoint(base.points)!)).toBeLessThan(2);
        for (const pressure of BELOW_PB_BAR) {
            const [b, t] = [atPressure(base, pressure), atPressure(twin, pressure)];
            expect(Math.abs(t.recoveryPct - b.recoveryPct), `recovery at ${pressure} bar`).toBeLessThan(0.1);
            expect(Math.abs(t.gor / b.gor - 1), `GOR at ${pressure} bar`).toBeLessThan(0.02);
        }
    }, 300000);

    /**
     * The second dimension (#26), the mirror image of the first: identical until
     * the bubble point, fanning out below it. Measured at matched average pressure,
     * Petrosky-Farshad / Standing / Al-Marhoun:
     *
     *     130 bar: GOR 125 / 150 / 179 m3/m3, recovery 6.91 / 7.45 / 7.94 %
     *     110 bar: GOR 318 / 400 / 469 m3/m3, recovery 10.70 / 11.20 / 11.50 %
     *      90 bar: GOR 530 / 688 / 801 m3/m3, recovery 13.13 / 13.49 / 13.63 %
     *
     * and cumulative gas at 360 d of 291 / 344 / 379 thousand Sm3. The recovery
     * spread narrows with depletion (0.1 points by 60 bar); the GOR spread does not.
     */
    it('the saturated-branch rungs are identical until the bubble point and fan out below it', async () => {
        await ensureWasmReady();
        const rungs = SATURATED_PVT.map((variant) => run('saturated_pvt', variant));
        const [petrosky, standing, alMarhoun] = rungs;

        const crossings = rungs.map((r) => timeToBubblePoint(r.points)!);
        expect(Math.max(...crossings) - Math.min(...crossings)).toBeLessThanOrEqual(1);
        for (const r of [petrosky, alMarhoun]) {
            for (const point of standing.points.filter((p) => p.time <= 40)) {
                expect(Math.abs(pressureAt(r.points, point.time) - point.avgPressure)).toBeLessThan(0.05);
            }
        }

        for (const pressure of BELOW_PB_BAR) {
            const [pf, st, am] = rungs.map((r) => atPressure(r, pressure));
            expect(st.gor / pf.gor, `GOR at ${pressure} bar`).toBeGreaterThan(1.1);
            expect(am.gor / st.gor, `GOR at ${pressure} bar`).toBeGreaterThan(1.1);
        }
        for (const [pressure, spread] of [[130, 0.8], [110, 0.5]]) {
            const [pf, st, am] = rungs.map((r) => atPressure(r, pressure));
            expect(pf.recoveryPct).toBeLessThan(st.recoveryPct);
            expect(st.recoveryPct).toBeLessThan(am.recoveryPct);
            expect(am.recoveryPct - pf.recoveryPct, `recovery spread at ${pressure} bar`).toBeGreaterThan(spread);
        }

        const cumGas = rungs.map((r) => integrateRunSeries(r.history).gas.at(-1)!);
        expect(cumGas[1] / cumGas[0]).toBeGreaterThan(1.1);
        expect(cumGas[2] / cumGas[1]).toBeGreaterThan(1.05);
    }, 300000);
});

describe('dep_pvt against OPM Flow (#20)', () => {
    const flowPressure = (caseKey: string) => {
        const artifact = listOpmFlowArtifacts().find((candidate) => candidate.caseKey === caseKey)!;
        return artifact.series.find((series) => series.mnemonic === 'FPR')!.data
            .map((point) => ({ time: point.x, avgPressure: point.y }));
    };

    it('ships the PVT tables its Flow decks were generated from', () => {
        // opm_small_direct.rs builds the decks from this fixture. If the correlation behind
        // generateBlackOilTable changes, the scenario drifts from its decks here, before any
        // chart shows a reference for a fluid the scenario no longer has.
        const table = (dimension: string | null, variant: string | null) =>
            getScenarioWithVariantParams('dep_pvt', dimension, variant).pvtTable;
        expect(table(null, null)).toEqual(depPvtDeckTables.correlation);
        expect(table('pvt_model', 'pvt_lab_report')).toEqual(depPvtDeckTables.lab_report);
        expect(table('saturated_pvt', 'sat_petrosky_farshad')).toEqual(depPvtDeckTables.petrosky_farshad);
        expect(table('saturated_pvt', 'sat_al_marhoun')).toEqual(depPvtDeckTables.al_marhoun);
    });

    it('Flow reaches the bubble point 2x later on the lab-report table, as ResSim does', () => {
        const crossing = (caseKey: string) => flowPressure(caseKey).find((point) => point.avgPressure < BUBBLE_POINT_BAR)!.time;
        // Measured (flow 2026.04, #26 design): 46.5 d and 91.5 d on the 0.75 d report grid,
        // the same report as the scenario's own runs. All three saturated rungs cross with the base.
        expect(crossing('dep_pvt_correlation')).toBeCloseTo(46.5, 1);
        expect(crossing('dep_pvt_lab_report')).toBeCloseTo(91.5, 1);
        expect(crossing('dep_pvt_petrosky_farshad')).toBeCloseTo(46.5, 1);
        expect(crossing('dep_pvt_al_marhoun')).toBeCloseTo(46.5, 1);
    });

    /**
     * Each deck is the scenario's own rung, so the scenario's run and Flow's must agree on the
     * headline chart at every report, not just at the crossing. Measured (flow 2026.04, #26
     * design), worst over 480 reports: 0.030 / 0.059 / 0.027 / 0.066 bar for the base,
     * lab-report, Petrosky-Farshad and Al-Marhoun rungs. The 0.25 bar band is ~4x the worst.
     */
    it("every rung's average pressure follows its Flow run", async () => {
        await ensureWasmReady();
        const rungs: Array<[string, Run]> = [
            ['dep_pvt_correlation', run(null, null)],
            ['dep_pvt_lab_report', run('pvt_model', 'pvt_lab_report')],
            ['dep_pvt_petrosky_farshad', run('saturated_pvt', 'sat_petrosky_farshad')],
            ['dep_pvt_al_marhoun', run('saturated_pvt', 'sat_al_marhoun')],
        ];
        for (const [caseKey, r] of rungs) {
            const worst = Math.max(...flowPressure(caseKey).map((point) => (
                Math.abs(pressureAt(r.points, point.time) - point.avgPressure)
            )));
            expect(worst, caseKey).toBeLessThan(0.25);
        }
    }, 300000);
});
