import type { ChartScalePreset } from './chartLayoutConfig';
/**
 * scalePresetRegistry.ts — shared Chart.js scale configuration objects.
 *
 * Exports the scale configs shared by every panel. The presets that depend on
 * context (rates, breakthrough, recovery, diagnostics) stay with their consumer,
 * because they differ in axis placement, dynamic titles, or derived values.
 *
 * Import individual scale objects for use inside a consumer's own
 * getScalePresetConfig function.
 */

export const SCALE_CUMULATIVE_VOLUMES = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        min: 0,
        alignToPixels: true,
        title: { display: true, text: 'Cumulative (m³)' },
        ticks: { count: 6 },
    },
};

export const SCALE_CUMULATIVE = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        min: 0,
        alignToPixels: true,
        title: { display: true, text: 'Cumulative (m³)' },
        ticks: { count: 6 },
    },
    y1: {
        type: 'linear',
        display: true,
        position: 'right',
        min: 0,
        max: 1,
        alignToPixels: true,
        title: { display: true, text: 'Recovery Factor' },
        grid: { drawOnChartArea: false },
        ticks: { count: 6 },
        _fraction: true,
    },
};

export const SCALE_PRESSURE = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        alignToPixels: true,
        title: { display: true, text: 'Pressure (bar)' },
        ticks: { count: 6 },
        _auto: true,
    },
};

export const SCALE_PRODUCTIVITY = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        min: 0,
        alignToPixels: true,
        title: { display: true, text: 'Productivity Index (m³/day/bar)' },
        ticks: { count: 6 },
        _auto: true,
    },
};

export const SCALE_SHAPE_FACTOR = {
    y: {
        type: 'linear', display: true, position: 'left', min: 0,
        alignToPixels: true,
        title: { display: true, text: 'Dietz Shape Factor C_A' },
        ticks: { count: 6 }, _auto: true,
    },
};

export const SCALE_RATIO = {
    y: {
        type: 'linear', display: true, position: 'left', min: 0,
        alignToPixels: true,
        title: { display: true, text: 'OOIP Estimate / Input OOIP' },
        ticks: { count: 6 }, _auto: true,
    },
};

export const SCALE_GOR = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        min: 0,
        alignToPixels: true,
        title: { display: true, text: 'GOR (Sm³/Sm³)' },
        ticks: { count: 6 },
        _auto: true,
    },
};

export const SCALE_FRACTION = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        min: 0,
        max: 1,
        alignToPixels: true,
        title: { display: true, text: 'Fraction' },
        ticks: { count: 6 },
        _fraction: true,
    },
};

/** Sweep efficiency panels (E_A, E_V, E_vol) — 0–1 fraction axis. */
export const SCALE_SWEEP = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        min: 0,
        max: 1,
        alignToPixels: true,
        title: { display: true, text: 'Sweep Efficiency' },
        ticks: { count: 6 },
    },
};

// ─── Context-dependent presets and the preset lookup ──────────────────────────
//
// These four used to live inside `ReferenceComparisonChart.svelte`, along with
// the if-ladder that selected between them — which is how axis titles like
// "Water Cut / Saturation" ended up inside a rendering component. A scale is a
// presentation policy keyed by preset name; the component now asks for one.

export const BREAKTHROUGH_SCALES = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        min: 0,
        max: 1,
        alignToPixels: true,
        title: { display: true, text: 'Water Cut / Saturation' },
        ticks: { count: 6 },
        _fraction: true,
    },
};
export const RATE_SCALES = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        min: 0,
        alignToPixels: true,
        title: { display: true, text: 'Rate (m³/day)' },
        ticks: { count: 6 },
    },
};
export const RECOVERY_SCALES = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        min: 0,
        alignToPixels: true,
        title: { display: true, text: 'Recovery Factor' },
        ticks: { count: 6 },
        _fraction: true,
        _maxCap: 1,
    },
};
export const DIAGNOSTICS_SCALES = {
    y: {
        type: 'linear',
        display: true,
        position: 'left',
        alignToPixels: true,
        title: { display: true, text: 'Pressure (bar)' },
        ticks: { count: 6 },
        _auto: true,
    },
    y1: {
        type: 'linear',
        display: true,
        position: 'right',
        min: 0,
        max: 1,
        alignToPixels: true,
        title: { display: true, text: 'BHP-limited fraction' },
        grid: { drawOnChartArea: false },
        ticks: { count: 6 },
        _fraction: true,
    },
};

export function getScalePresetConfig(scalePreset: ChartScalePreset): Record<string, any> {
    if (scalePreset === 'sweep') return SCALE_SWEEP;
    if (scalePreset === 'sweep_rf') return SCALE_SWEEP;
    if (scalePreset === 'breakthrough') return BREAKTHROUGH_SCALES;
    if (scalePreset === 'pressure') return SCALE_PRESSURE;
    if (scalePreset === 'productivity') return SCALE_PRODUCTIVITY;
    if (scalePreset === 'shape_factor') return SCALE_SHAPE_FACTOR;
    if (scalePreset === 'ratio') return SCALE_RATIO;
    if (scalePreset === 'gor') return SCALE_GOR;
    if (scalePreset === 'diagnostics') return DIAGNOSTICS_SCALES;
    if (scalePreset === 'fraction') return SCALE_FRACTION;
    if (scalePreset === 'recovery') return RECOVERY_SCALES;
    if (scalePreset === 'cumulative_volumes') return SCALE_CUMULATIVE_VOLUMES;
    if (scalePreset === 'cumulative') return SCALE_CUMULATIVE;
    return RATE_SCALES;
}
