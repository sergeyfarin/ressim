/**
 * scalePresetRegistry.ts — shared Chart.js scale configuration objects.
 *
 * Exports the scale configs that are identical across RateChart and
 * ReferenceComparisonChart. Component-specific configs (rates, breakthrough,
 * recovery, diagnostics) stay local because they differ in axis placement,
 * dynamic titles, or derived values.
 *
 * Import individual scale objects for use inside each component's own
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
