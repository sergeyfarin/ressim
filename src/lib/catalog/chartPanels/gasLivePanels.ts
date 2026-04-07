/**
 * gasLivePanels.ts — live chart panel definitions for gas scenarios.
 *
 * gasInjectionLivePanels: gas_injection (gas-oil BL analytical, has injector)
 * gasDriveLivePanels:     gas_drive (no analytical, three-phase depletion)
 * spe1LivePanels:         spe1_gas_injection (digitized reference only, no computed analytical)
 *
 * Gas scenarios show GOR instead of (or alongside) water cut.
 * Analytical curves only appear where the scenario has a computed analytical def.
 */

import type { UniversalPanelDef } from '../../charts/universalChartTypes';

// ─── Gas Injection (gas-oil BL, has analytical) ───────────────────────────────

export const gasInjectionLivePanels: UniversalPanelDef[] = [
    {
        panelKey: 'rates',
        curves: [
            {
                key: 'oil-rate-sim', label: 'Oil Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#16a34a',
                getData: (ctx) => ctx.sim.oilRate.map((v) => v * ctx.scaleFactor),
            },
            {
                key: 'oil-rate-analytical', label: 'Oil Rate (Analytical)', curveType: 'analytical',
                yAxisID: 'y', color: 'neutral',
                getData: (ctx) => ctx.analytical.map((p) =>
                    p.oilRate != null ? p.oilRate * ctx.scaleFactor : null),
            },
            {
                key: 'injection-rate', label: 'Injection Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#f59e0b',
                getData: (ctx) => ctx.sim.injectionRate.map((v) => v * ctx.scaleFactor),
            },
        ],
    },
    {
        panelKey: 'recovery',
        curves: [
            {
                key: 'recovery-factor-sim', label: 'Recovery Factor', curveType: 'simulation',
                yAxisID: 'y', color: '#22c55e',
                getData: (ctx) => ctx.sim.recoveryFactor,
            },
            {
                key: 'recovery-factor-analytical', label: 'Recovery Factor (Analytical)', curveType: 'analytical',
                yAxisID: 'y', color: 'neutral',
                getData: (ctx) => ctx.sim.analyticalRecoveryFactor,
            },
        ],
    },
    {
        panelKey: 'cumulative',
        curves: [
            {
                key: 'cum-oil-sim', label: 'Cum Oil', curveType: 'simulation',
                yAxisID: 'y', color: '#0f5132',
                getData: (ctx) => ctx.sim.cumOil,
            },
            {
                key: 'cum-oil-analytical', label: 'Cum Oil (Analytical)', curveType: 'analytical',
                yAxisID: 'y', color: 'neutral',
                getData: (ctx) => ctx.sim.analyticalCumOil,
            },
            {
                key: 'cum-injection', label: 'Cum Injection', curveType: 'simulation',
                yAxisID: 'y', color: '#f59e0b',
                getData: (ctx) => ctx.sim.cumInjection,
            },
        ],
    },
    {
        panelKey: 'diagnostics',
        curves: [
            {
                key: 'avg-pressure-sim', label: 'Avg Pressure', curveType: 'simulation',
                yAxisID: 'y', color: '#dc2626',
                getData: (ctx) => ctx.sim.avgPressure,
            },
            {
                key: 'vrr', label: 'VRR', curveType: 'simulation',
                yAxisID: 'y1', color: '#7c3aed', defaultVisible: false,
                getData: (ctx) => ctx.sim.vrr,
            },
            {
                key: 'mb-error', label: 'MB Error', curveType: 'simulation',
                yAxisID: 'y2', color: '#ef4444', defaultVisible: false,
                getData: (ctx) => ctx.sim.mbError,
            },
        ],
    },
    {
        panelKey: 'gor',
        curves: [
            {
                key: 'gor-sim', label: 'GOR', curveType: 'simulation',
                yAxisID: 'y', color: '#f59e0b',
                getData: (ctx) => ctx.rateHistory.map((p) => {
                    const v = p.producing_gor;
                    return Number.isFinite(v) ? v as number : null;
                }),
            },
        ],
    },
];

// ─── Gas Drive (no analytical, three-phase) ───────────────────────────────────

export const gasDriveLivePanels: UniversalPanelDef[] = [
    {
        panelKey: 'rates',
        curves: [
            {
                key: 'oil-rate-sim', label: 'Oil Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#16a34a',
                getData: (ctx) => ctx.sim.oilRate.map((v) => v * ctx.scaleFactor),
            },
        ],
    },
    {
        panelKey: 'recovery',
        curves: [
            {
                key: 'recovery-factor-sim', label: 'Recovery Factor', curveType: 'simulation',
                yAxisID: 'y', color: '#22c55e',
                getData: (ctx) => ctx.sim.recoveryFactor,
            },
        ],
    },
    {
        panelKey: 'cumulative',
        curves: [
            {
                key: 'cum-oil-sim', label: 'Cum Oil', curveType: 'simulation',
                yAxisID: 'y', color: '#0f5132',
                getData: (ctx) => ctx.sim.cumOil,
            },
        ],
    },
    {
        panelKey: 'diagnostics',
        curves: [
            {
                key: 'avg-pressure-sim', label: 'Avg Pressure', curveType: 'simulation',
                yAxisID: 'y', color: '#dc2626',
                getData: (ctx) => ctx.sim.avgPressure,
            },
            {
                key: 'mb-error', label: 'MB Error', curveType: 'simulation',
                yAxisID: 'y2', color: '#ef4444', defaultVisible: false,
                getData: (ctx) => ctx.sim.mbError,
            },
        ],
    },
    {
        panelKey: 'gor',
        curves: [
            {
                key: 'gor-sim', label: 'GOR', curveType: 'simulation',
                yAxisID: 'y', color: '#f59e0b',
                getData: (ctx) => ctx.rateHistory.map((p) => {
                    const v = p.producing_gor;
                    return Number.isFinite(v) ? v as number : null;
                }),
            },
        ],
    },
];

// ─── SPE1 Gas Injection (digitized reference only — no computed analytical) ───

export const spe1LivePanels: UniversalPanelDef[] = [
    {
        panelKey: 'oil_rate',
        curves: [
            {
                key: 'oil-rate-sim', label: 'Oil Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#16a34a',
                getData: (ctx) => ctx.sim.oilRate.map((v) => v * ctx.scaleFactor),
            },
        ],
    },
    {
        panelKey: 'recovery',
        curves: [
            {
                key: 'recovery-factor-sim', label: 'Recovery Factor', curveType: 'simulation',
                yAxisID: 'y', color: '#22c55e',
                getData: (ctx) => ctx.sim.recoveryFactor,
            },
        ],
    },
    {
        panelKey: 'cumulative',
        curves: [
            {
                key: 'cum-oil-sim', label: 'Cum Oil', curveType: 'simulation',
                yAxisID: 'y', color: '#0f5132',
                getData: (ctx) => ctx.sim.cumOil,
            },
            {
                key: 'cum-injection', label: 'Cum Injection', curveType: 'simulation',
                yAxisID: 'y', color: '#f59e0b',
                getData: (ctx) => ctx.sim.cumInjection,
            },
        ],
    },
    {
        panelKey: 'diagnostics',
        curves: [
            {
                key: 'avg-pressure-sim', label: 'Avg Pressure', curveType: 'simulation',
                yAxisID: 'y', color: '#dc2626',
                getData: (ctx) => ctx.sim.avgPressure,
            },
            {
                key: 'mb-error', label: 'MB Error', curveType: 'simulation',
                yAxisID: 'y2', color: '#ef4444', defaultVisible: false,
                getData: (ctx) => ctx.sim.mbError,
            },
        ],
    },
    {
        panelKey: 'gor',
        curves: [
            {
                key: 'gor-sim', label: 'GOR', curveType: 'simulation',
                yAxisID: 'y', color: '#f59e0b',
                getData: (ctx) => ctx.rateHistory.map((p) => {
                    const v = p.producing_gor;
                    return Number.isFinite(v) ? v as number : null;
                }),
            },
        ],
    },
    {
        panelKey: 'producer_bhp',
        curves: [
            {
                key: 'producer-bhp-sim', label: 'Producer BHP', curveType: 'simulation',
                yAxisID: 'y', color: '#dc2626',
                getData: (ctx) => ctx.rateHistory.map((p) => {
                    // aggregate BHP from well state is not in rateHistory directly;
                    // use avg_reservoir_pressure as proxy until proper BHP series exists
                    return null;
                }),
            },
        ],
    },
];
