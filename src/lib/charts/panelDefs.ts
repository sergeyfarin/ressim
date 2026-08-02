/**
 * panelDefs.ts — canonical default metadata for every chart panel.
 *
 * PANEL_DEFS is the single source of truth for panel titles, scale presets,
 * and default visibility/expansion state. Components spread from it and
 * override only the fields that depend on context (family, analyticalMethod,
 * dynamic curve lists, etc.).
 *
 * Kept in sync with DEFAULT_CHART_PANEL_ORDER in chartLayoutConfig.ts.
 */

import type { ChartPanelFallback } from './chartPanelSelection';
import type { KnownPrimaryPanelId, KnownSweepPanelId, ChartPanelId } from './chartLayoutConfig';

export const PANEL_DEFS: Record<KnownPrimaryPanelId | KnownSweepPanelId, ChartPanelFallback> = {
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
    gas_rate: {
        title: 'Gas Rate',
        curveKeys: ['gas-rate-sim'],
        scalePreset: 'rates',
        allowLogToggle: true,
        visible: true,
        expanded: false,
    },
    cumulative_gas: {
        title: 'Cum Gas',
        curveKeys: ['cum-gas-sim'],
        scalePreset: 'cumulative_volumes',
        visible: true,
        expanded: false,
    },
    diagnostics: {
        title: 'Average Pressure',
        scalePreset: 'pressure',
        visible: true,
        expanded: false,
    },
    avg_water_sat: {
        title: 'Average Water Saturation',
        curveKeys: ['avg-water-sat'],
        scalePreset: 'fraction',
        visible: false,
        expanded: false,
    },
    mbe_ooip: {
        title: 'MBE OOIP Ratio',
        scalePreset: 'ratio',
        visible: false,
        expanded: false,
    },
    drive_indices: {
        title: 'Drive Indices',
        scalePreset: 'fraction',
        visible: false,
        expanded: false,
    },
    pz: {
        title: 'p/z',
        scalePreset: 'pressure',
        visible: false,
        expanded: false,
    },
    pss_drawdown: {
        title: 'Drawdown p̄ − p_wf',
        scalePreset: 'pressure',
        visible: false,
        expanded: false,
    },
    pss_productivity: {
        title: 'PSS Productivity Index',
        scalePreset: 'productivity',
        visible: false,
        expanded: false,
    },
    pss_shape_factor: {
        title: 'Dietz Shape Factor',
        scalePreset: 'shape_factor',
        visible: false,
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

/**
 * Defaults for a panel this module has never heard of.
 *
 * Panel ids are an open type as of 2026-08-02, so a layout may name a panel that
 * predates no entry here. It gets a readable title and a neutral scale rather
 * than `undefined` reaching the renderer.
 */
export const GENERIC_PANEL_FALLBACK: ChartPanelFallback = {
    title: 'Panel',
    scalePreset: 'rates',
    allowLogToggle: true,
    visible: true,
    expanded: false,
};

/** Panel defaults by id, with a generic fallback for ids not declared above. */
export function getPanelFallback(panelId: ChartPanelId): ChartPanelFallback {
    const known = (PANEL_DEFS as Record<string, ChartPanelFallback | undefined>)[panelId];
    if (known) return known;
    return {
        ...GENERIC_PANEL_FALLBACK,
        // `mbe_ooip` → "Mbe Ooip" is poor, but it is a placeholder for a panel
        // whose owner has not given it a title, not a naming scheme.
        title: panelId.replaceAll('_', ' ').replace(/^./, (char) => char.toUpperCase()),
    };
}
