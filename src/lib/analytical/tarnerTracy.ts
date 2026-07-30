import type { PvtRow } from '../simulator-types';
import { k_rg, k_ro_gas, type GasOilRockProps } from './fractionalFlow';

export type TarnerTracyInputs = {
    pressureBar: Array<number | null>;
    pvtTable: PvtRow[];
    initialPressureBar: number;
    poreVolumeM3: number;
    initialWaterSaturation: number;
    initialGasSaturation: number;
    rock: GasOilRockProps;
};

export type TarnerTracyPoint = {
    pressureBar: number;
    cumulativeOilM3: number;
    recoveryFactor: number;
    producingGorM3M3: number;
    gasSaturation: number;
};

type PvtState = Pick<PvtRow, 'rs_m3m3' | 'bo_m3m3' | 'mu_o_cp' | 'bg_m3m3' | 'mu_g_cp'>;

function interpolatePvt(table: PvtRow[], pressureBar: number): PvtState | null {
    const rows = table
        .filter((row) => Number.isFinite(row.p_bar))
        .sort((left, right) => left.p_bar - right.p_bar);
    if (!rows.length) return null;
    const upperIndex = rows.findIndex((row) => row.p_bar >= pressureBar);
    const upper = upperIndex < 0 ? rows[rows.length - 1] : rows[upperIndex];
    const lower = upperIndex <= 0 ? rows[0] : rows[upperIndex - 1];
    const span = upper.p_bar - lower.p_bar;
    const fraction = span > 0 ? Math.max(0, Math.min(1, (pressureBar - lower.p_bar) / span)) : 0;
    const lerp = (key: keyof PvtState) => Number(lower[key]) + fraction * (Number(upper[key]) - Number(lower[key]));
    return {
        rs_m3m3: lerp('rs_m3m3'),
        bo_m3m3: lerp('bo_m3m3'),
        mu_o_cp: lerp('mu_o_cp'),
        bg_m3m3: lerp('bg_m3m3'),
        mu_g_cp: lerp('mu_g_cp'),
    };
}

function producingGor(state: PvtState, gasSaturation: number, rock: GasOilRockProps): number {
    const kro = k_ro_gas(gasSaturation, rock);
    const krg = k_rg(gasSaturation, rock);
    const freeGasGor = kro > 1e-12
        ? (krg / kro) * (state.mu_o_cp / Math.max(1e-12, state.mu_g_cp))
            * (state.bo_m3m3 / Math.max(1e-12, state.bg_m3m3))
        : 0;
    return Math.max(0, state.rs_m3m3 + freeGasGor);
}

/**
 * Tarner–Tracy prediction for a volumetric solution-gas-drive tank.
 *
 * Each pressure step solves material balance together with the trapezoidal
 * instantaneous-GOR relation. Water/rock compressibility, gravity segregation,
 * spatial pressure gradients and well deliverability are deliberately absent.
 * The result is therefore a mechanism reference versus average pressure, not an
 * independent prediction of rate versus time.
 */
export function calculateTarnerTracy(inputs: TarnerTracyInputs): TarnerTracyPoint[] {
    const pressureValues = inputs.pressureBar.filter((value): value is number => Number.isFinite(value));
    if (!pressureValues.length || inputs.pvtTable.length < 2 || !(inputs.poreVolumeM3 > 0)) return [];

    const initialPressure = inputs.initialPressureBar;
    const initialPvt = interpolatePvt(inputs.pvtTable, initialPressure);
    if (!initialPvt) return [];

    const sw = Math.max(0, Math.min(1, inputs.initialWaterSaturation));
    const sgInitial = Math.max(0, Math.min(1 - sw, inputs.initialGasSaturation));
    const soInitial = Math.max(1e-12, 1 - sw - sgInitial);
    const hydrocarbonPoreVolume = inputs.poreVolumeM3 * (1 - sw);
    const originalOilM3 = inputs.poreVolumeM3 * soInitial / Math.max(1e-12, initialPvt.bo_m3m3);
    const initialFreeGasReservoirM3 = inputs.poreVolumeM3 * sgInitial;
    const originalGasM3 = originalOilM3 * initialPvt.rs_m3m3
        + initialFreeGasReservoirM3 / Math.max(1e-12, initialPvt.bg_m3m3);

    let previousPressure = initialPressure;
    let previousCumulativeOil = 0;
    let previousCumulativeGas = 0;
    let previousGor = producingGor(initialPvt, sgInitial, inputs.rock);
    const points: TarnerTracyPoint[] = [];

    for (const requestedPressure of pressureValues) {
        const pressure = Math.min(previousPressure, Math.max(1e-6, requestedPressure));
        const pvt = interpolatePvt(inputs.pvtTable, pressure);
        if (!pvt) continue;

        const evaluate = (cumulativeOil: number) => {
            const oilRemaining = Math.max(0, originalOilM3 - cumulativeOil);
            const oilReservoirVolume = oilRemaining * pvt.bo_m3m3;
            const freeGasReservoirVolume = Math.max(0, hydrocarbonPoreVolume - oilReservoirVolume);
            const gasSaturation = freeGasReservoirVolume / inputs.poreVolumeM3;
            const gasRemaining = oilRemaining * pvt.rs_m3m3
                + freeGasReservoirVolume / Math.max(1e-12, pvt.bg_m3m3);
            const cumulativeGas = Math.max(0, originalGasM3 - gasRemaining);
            const gor = producingGor(pvt, gasSaturation, inputs.rock);
            return { cumulativeGas, gasSaturation, gor };
        };

        let lo = previousCumulativeOil;
        let hi = originalOilM3 * (1 - 1e-10);
        const residual = (cumulativeOil: number) => {
            const state = evaluate(cumulativeOil);
            const producedOil = cumulativeOil - previousCumulativeOil;
            const producedGas = state.cumulativeGas - previousCumulativeGas;
            return producedGas - producedOil * 0.5 * (previousGor + state.gor);
        };

        if (residual(lo) > 0 && residual(hi) < 0) {
            for (let iteration = 0; iteration < 80; iteration += 1) {
                const mid = 0.5 * (lo + hi);
                if (residual(mid) > 0) lo = mid;
                else hi = mid;
            }
            previousCumulativeOil = 0.5 * (lo + hi);
        }

        const state = evaluate(previousCumulativeOil);
        previousCumulativeGas = state.cumulativeGas;
        previousGor = state.gor;
        previousPressure = pressure;
        points.push({
            pressureBar: pressure,
            cumulativeOilM3: previousCumulativeOil,
            recoveryFactor: previousCumulativeOil / originalOilM3,
            producingGorM3M3: state.gor,
            gasSaturation: state.gasSaturation,
        });
    }

    return points;
}
