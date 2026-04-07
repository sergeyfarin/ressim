/**
 * depletionLivePanels.ts — live chart panel definitions for depletion scenarios.
 *
 * Used by: dep_pss, dep_decline, dep_arps.
 *
 * Depletion has no injector so injection rate, VRR, and water-cut panels are omitted.
 * Analytical reference is the Dietz PSS / Arps decline model.
 */

import type { UniversalPanelDef } from '../../charts/universalChartTypes';

export const depletionLivePanels: UniversalPanelDef[] = [
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
                key: 'mb-error', label: 'MB Error', curveType: 'simulation',
                yAxisID: 'y2', color: '#ef4444', defaultVisible: false,
                getData: (ctx) => ctx.sim.mbError,
            },
        ],
    },
];
