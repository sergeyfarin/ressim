/**
 * waterfloodLivePanels.ts — live chart panel definitions for waterflood scenarios.
 *
 * Used by: wf_bl1d (and as the base for sweep scenarios via sweepLivePanels.ts).
 *
 * Three curve tiers per metric:
 *   simulation  — solid 2.5px  (our IMPES output)
 *   analytical  — dashed       (Buckley-Leverett / Welge reference solution)
 *   (reference/reference-simulation tiers added per scenario as needed)
 */

import type { UniversalPanelDef } from '../../charts/universalChartTypes';

export const waterfloodLivePanels: UniversalPanelDef[] = [
    // ── Rates ─────────────────────────────────────────────────────────────────
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
                key: 'water-rate-sim', label: 'Water Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#1e3a8a',
                getData: (ctx) => ctx.sim.waterRate.map((v) => v * ctx.scaleFactor),
            },
            {
                key: 'water-rate-analytical', label: 'Water Rate (Analytical)', curveType: 'analytical',
                yAxisID: 'y', color: 'neutral',
                getData: (ctx) => ctx.analytical.map((p) =>
                    p.waterRate != null ? p.waterRate * ctx.scaleFactor : null),
            },
            {
                key: 'injection-rate', label: 'Injection Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#06b6d4',
                getData: (ctx) => ctx.sim.injectionRate.map((v) => v * ctx.scaleFactor),
            },
            {
                key: 'liquid-rate', label: 'Liquid Rate', curveType: 'simulation',
                yAxisID: 'y', color: '#2563eb', defaultVisible: false,
                getData: (ctx) => ctx.sim.liquidRate.map((v) => v * ctx.scaleFactor),
            },
        ],
    },

    // ── Recovery Factor ────────────────────────────────────────────────────────
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

    // ── Cumulative ─────────────────────────────────────────────────────────────
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
                yAxisID: 'y', color: '#06b6d4',
                getData: (ctx) => ctx.sim.cumInjection,
            },
            {
                key: 'cum-water', label: 'Cum Water', curveType: 'simulation',
                yAxisID: 'y', color: '#1e3a8a',
                getData: (ctx) => ctx.sim.cumWater,
            },
        ],
    },

    // ── Diagnostics ────────────────────────────────────────────────────────────
    {
        panelKey: 'diagnostics',
        curves: [
            {
                key: 'avg-pressure-sim', label: 'Avg Pressure', curveType: 'simulation',
                yAxisID: 'y', color: '#dc2626',
                getData: (ctx) => ctx.sim.avgPressure,
            },
            {
                key: 'avg-pressure-analytical', label: 'Avg Pressure (Analytical)', curveType: 'analytical',
                yAxisID: 'y', color: 'neutral',
                getData: (ctx) => ctx.sim.analyticalAvgPressure,
            },
            {
                key: 'vrr', label: 'VRR', curveType: 'simulation',
                yAxisID: 'y1', color: '#7c3aed', defaultVisible: false,
                getData: (ctx) => ctx.sim.vrr,
            },
            {
                key: 'wor-sim', label: 'WOR', curveType: 'simulation',
                yAxisID: 'y1', color: '#d97706', defaultVisible: false,
                getData: (ctx) => ctx.sim.worSim,
            },
            {
                key: 'wor-analytical', label: 'WOR (Analytical)', curveType: 'analytical',
                yAxisID: 'y1', color: 'neutral', defaultVisible: false,
                getData: (ctx) => ctx.sim.worAnalytical,
            },
            {
                key: 'avg-water-sat', label: 'Avg Water Sat', curveType: 'simulation',
                yAxisID: 'y1', color: '#1d4ed8', defaultVisible: false,
                getData: (ctx) => ctx.sim.avgWaterSat,
            },
            {
                key: 'water-cut-sim', label: 'Water Cut', curveType: 'simulation',
                yAxisID: 'y1', color: '#2563eb', defaultVisible: false,
                getData: (ctx) => ctx.sim.waterCutSim,
            },
            {
                key: 'water-cut-analytical', label: 'Water Cut (Analytical)', curveType: 'analytical',
                yAxisID: 'y1', color: 'neutral', defaultVisible: false,
                getData: (ctx) => ctx.sim.waterCutAnalytical,
            },
            {
                key: 'mb-error', label: 'MB Error', curveType: 'simulation',
                yAxisID: 'y2', color: '#ef4444', defaultVisible: false,
                getData: (ctx) => ctx.sim.mbError,
            },
        ],
    },
];
