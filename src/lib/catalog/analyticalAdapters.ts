/**
 * analyticalAdapters.ts — pre-built ScenarioAnalyticalDef objects for each
 * analytical family (BL waterflood, depletion, gas-oil BL).
 *
 * Imported by scenario files to avoid repeating the same inputsFromParams
 * logic across multiple scenarios that share the same physics.
 */

import {
    calculateAnalyticalProduction,
    calculateGasOilAnalyticalProduction,
    type RockProps,
    type FluidProps,
    type GasOilRockProps,
    type GasOilFluidProps,
} from '../analytical/fractionalFlow';
import {
    calculateDepletionAnalyticalProduction,
    type DepletionAnalyticalParams,
} from '../analytical/depletionAnalytical';
import type { RateHistoryPoint } from '../simulator-types';
import type { ScenarioAnalyticalDef, ScenarioAnalyticalOutput } from './scenarios';

// ─── Waterflood Buckley-Leverett ─────────────────────────────────────────────

type BLInputs = {
    rock: RockProps;
    fluid: FluidProps;
    initialSaturation: number;
    timeHistory: number[];
    injectionRates: number[];
    poreVolume: number;
};

export const waterfloodBLDef: ScenarioAnalyticalDef = {
    fn: (inputs): ScenarioAnalyticalOutput => {
        const { rock, fluid, initialSaturation, timeHistory, injectionRates, poreVolume } = inputs as BLInputs;
        const points = calculateAnalyticalProduction(rock, fluid, initialSaturation, timeHistory, injectionRates, poreVolume);
        return {
            production: points,
            meta: { mode: 'waterflood', shapeFactor: null, shapeLabel: '' },
        };
    },
    inputsFromParams: (params: Record<string, unknown>, rh: RateHistoryPoint[]): BLInputs => ({
        rock: {
            s_wc: params.s_wc as number,
            s_or: params.s_or as number,
            n_w: params.n_w as number,
            n_o: params.n_o as number,
            k_rw_max: params.k_rw_max as number,
            k_ro_max: params.k_ro_max as number,
        },
        fluid: {
            mu_w: params.mu_w as number,
            mu_o: params.mu_o as number,
        },
        initialSaturation: params.initialSaturation as number,
        timeHistory: rh.map(p => p.time),
        injectionRates: rh.map(p => Number(p.total_injection ?? 0)),
        poreVolume: (params.nx as number) * (params.cellDx as number)
            * (params.ny as number) * (params.cellDy as number)
            * (params.nz as number) * (params.cellDz as number)
            * (params.reservoirPorosity as number),
    }),
};

// ─── Pressure Depletion (Dietz / Arps) ───────────────────────────────────────

export const depletionDef: ScenarioAnalyticalDef = {
    fn: (inputs): ScenarioAnalyticalOutput => {
        const result = calculateDepletionAnalyticalProduction(inputs as DepletionAnalyticalParams);
        return {
            production: result.production,
            meta: result.meta,
        };
    },
    inputsFromParams: (params: Record<string, unknown>, rh: RateHistoryPoint[]): DepletionAnalyticalParams => ({
        reservoir: {
            length: (params.nx as number) * (params.cellDx as number),
            area: (params.ny as number) * (params.cellDy as number) * (params.nz as number) * (params.cellDz as number),
            porosity: params.reservoirPorosity as number,
        },
        timeHistory: rh.map(p => p.time),
        minTimeDays: params.analyticalDepletionStartDays as number | undefined,
        initialSaturation: params.initialSaturation as number,
        nz: params.nz as number,
        permMode: params.permMode as string,
        uniformPermX: params.uniformPermX as number,
        uniformPermY: params.uniformPermY as number,
        layerPermsX: params.layerPermsX as number[],
        layerPermsY: params.layerPermsY as number[],
        cellDx: params.cellDx as number,
        cellDy: params.cellDy as number,
        cellDz: params.cellDz as number,
        wellRadius: params.well_radius as number,
        wellSkin: params.well_skin as number,
        muO: params.mu_o as number,
        sWc: params.s_wc as number,
        sOr: params.s_or as number,
        nO: params.n_o as number,
        c_o: params.c_o as number,
        c_w: params.c_w as number,
        cRock: params.rock_compressibility as number,
        initialPressure: params.initialPressure as number,
        producerBhp: params.producerBhp as number,
        depletionRateScale: params.analyticalDepletionRateScale as number,
        arpsB: params.analyticalArpsB as number | undefined,
        nx: params.nx as number,
        ny: params.ny as number,
        producerI: params.producerI as number,
        producerJ: params.producerJ as number,
    }),
};

// ─── Gas-Oil Buckley-Leverett ─────────────────────────────────────────────────

type GasOilBLInputs = {
    rock: GasOilRockProps;
    fluid: GasOilFluidProps;
    initialGasSaturation: number;
    timeHistory: number[];
    injectionRates: number[];
    poreVolume: number;
};

export const gasOilBLDef: ScenarioAnalyticalDef = {
    fn: (inputs): ScenarioAnalyticalOutput => {
        const { rock, fluid, initialGasSaturation, timeHistory, injectionRates, poreVolume } = inputs as GasOilBLInputs;
        const points = calculateGasOilAnalyticalProduction(rock, fluid, initialGasSaturation, timeHistory, injectionRates, poreVolume);
        return {
            production: points,
            meta: { mode: 'gas-oil-bl', shapeFactor: null, shapeLabel: '' },
        };
    },
    inputsFromParams: (params: Record<string, unknown>, rh: RateHistoryPoint[]): GasOilBLInputs => ({
        rock: {
            s_wc: params.s_wc as number,
            s_gc: params.s_gc as number,
            s_gr: params.s_gr as number,
            s_org: params.s_org as number,
            n_o: params.n_o as number,
            n_g: params.n_g as number,
            k_ro_max: params.k_ro_max as number,
            k_rg_max: params.k_rg_max as number,
        },
        fluid: {
            mu_o: params.mu_o as number,
            mu_g: params.mu_g as number,
        },
        initialGasSaturation: params.initialGasSaturation as number,
        timeHistory: rh.map(p => p.time),
        injectionRates: rh.map(p => Number(p.total_injection ?? 0)),
        poreVolume: (params.nx as number) * (params.cellDx as number)
            * (params.ny as number) * (params.cellDy as number)
            * (params.nz as number) * (params.cellDz as number)
            * (params.reservoirPorosity as number),
    }),
};
