/**
 * panelDefs.ts — canonical default metadata for every chart panel.
 *
 * PANEL_DEFS is the single source of truth for panel titles, scale presets,
 * and default visibility/expansion state. Components spread from it and
 * override only the fields that depend on context (family, analyticalMethod,
 * dynamic curve lists, etc.).
 *
 * Kept in sync with DEFAULT_RATE_CHART_PANEL_ORDER in rateChartLayoutConfig.ts.
 */

import type { ChartPanelFallback } from './chartPanelSelection';
import type { RateChartPanelId } from './rateChartLayoutConfig';

export const PANEL_DEFS: Record<RateChartPanelId, ChartPanelFallback> = {
    // ── Primary panels ───────────────────────────────────────────────────────
    rates: {
        title: 'Rates',
        scalePreset: 'rates',
        allowLogToggle: true,
        visible: true,
        expanded: true,
    },
    recovery: {
        title: 'Recovery Factor',
        curveKeys: ['recovery-factor'],
        scalePreset: 'recovery',
        visible: true,
        expanded: true,
    },
    cumulative: {
        title: 'Cum Oil',
        curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
        scalePreset: 'cumulative_volumes',
        visible: true,
        expanded: false,
    },
    diagnostics: {
        title: 'Diagnostics',
        scalePreset: 'diagnostics',
        visible: true,
        expanded: false,
    },
    gor: {
        title: 'GOR',
        curveKeys: ['gor-sim', 'published-gor'],
        scalePreset: 'gor',
        visible: false,
        expanded: false,
    },
    volumes: {
        title: 'Cum Injection',
        curveKeys: ['cum-injection'],
        scalePreset: 'cumulative_volumes',
        visible: true,
        expanded: false,
    },
    oil_rate: {
        title: 'Oil Rate',
        curveKeys: ['oil-rate-sim'],
        scalePreset: 'rates',
        visible: true,
        expanded: false,
    },
    injection_rate: {
        title: 'Injection Rate',
        curveKeys: ['injection-rate-sim', 'published-injection-rate'],
        scalePreset: 'rates',
        visible: true,
        expanded: false,
    },
    producer_bhp: {
        title: 'Producer WBHP',
        curveKeys: ['producer-bhp-sim', 'published-producer-bhp'],
        scalePreset: 'pressure',
        visible: false,
        expanded: false,
    },
    injector_bhp: {
        title: 'Injector WBHP',
        curveKeys: ['injector-bhp-sim', 'published-injector-bhp'],
        scalePreset: 'pressure',
        visible: false,
        expanded: false,
    },
    control_limits: {
        title: 'Control-Limit Fraction',
        curveKeys: ['producer-bhp-limited-sim', 'injector-bhp-limited-sim'],
        scalePreset: 'fraction',
        visible: false,
        expanded: false,
    },
    // ── Sweep panels ─────────────────────────────────────────────────────────
    sweep_rf: {
        title: 'Sweep Recovery Factor',
        scalePreset: 'sweep_rf',
        visible: true,
        expanded: false,
    },
    sweep_areal: {
        title: 'Areal Sweep Efficiency (E_A)',
        scalePreset: 'sweep',
        visible: true,
        expanded: true,
    },
    sweep_vertical: {
        title: 'Vertical Sweep Efficiency (E_V)',
        scalePreset: 'sweep',
        visible: true,
        expanded: true,
    },
    sweep_combined: {
        title: 'Combined Sweep Efficiency (E_vol)',
        scalePreset: 'sweep',
        visible: true,
        expanded: true,
    },
    sweep_combined_mobile_oil: {
        title: 'Analytical Total E_vol vs Simulated Mobile Oil Recovered',
        scalePreset: 'sweep',
        visible: false,
        expanded: false,
    },
};
