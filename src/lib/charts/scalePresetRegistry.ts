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

// ─── Live (UniversalChart) scale configs ─────────────────────────────────────
// These four differ from the shared presets above in axis placement or dynamic
// titling, so they stay separate objects; they are consumed only through
// buildGetScalePresetConfig().

const RECOVERY_SCALES = {
    y: {
        type: 'linear', display: true, position: 'left', min: 0, max: 1,
        alignToPixels: true, title: { display: true, text: 'Recovery Factor' },
        ticks: { count: 6 }, _fraction: true,
    },
};

const BREAKTHROUGH_SCALES = {
    y1: {
        type: 'linear', display: true, position: 'right', min: 0, max: 1,
        alignToPixels: true, title: { display: true, text: 'Water Cut / Saturation' },
        grid: { drawOnChartArea: false }, ticks: { count: 6 }, _fraction: true,
    },
};

const DIAGNOSTICS_SCALES = {
    y: {
        type: 'linear', display: true, position: 'left', alignToPixels: true,
        title: { display: true, text: 'Pressure (bar)' }, ticks: { count: 6 }, _auto: true,
    },
    y1: {
        type: 'linear', display: true, position: 'right', min: 0, alignToPixels: true,
        title: { display: true, text: 'Fraction' }, grid: { drawOnChartArea: false },
        ticks: { count: 6 },
        _dynamicTitle: (labels: string[]) => {
            const parts: string[] = [];
            if (labels.some((l) => l.includes('VRR'))) parts.push('VRR');
            if (labels.some((l) => l.includes('WOR'))) parts.push('WOR');
            if (labels.some((l) => l.includes('Sat'))) parts.push('Saturation');
            if (labels.some((l) => l.includes('Cut'))) parts.push('Water Cut');
            return parts.length > 0 ? parts.join(' / ') : 'Fraction';
        },
    },
    y2: {
        type: 'linear', display: true, position: 'right', min: 0, alignToPixels: true,
        title: { display: true, text: 'MB Error (m³)' },
        grid: { drawOnChartArea: false }, ticks: { count: 6 },
    },
};

const SWEEP_RF_SCALES = {
    y: {
        type: 'linear', display: true, position: 'left', min: 0, max: 1,
        alignToPixels: true, title: { display: true, text: 'Recovery Factor' },
        ticks: {
            count: 6,
            _tickFormatter: (v: string | number) =>
                typeof v === 'number' ? (v * 100).toFixed(0) + '%' : v,
        },
    },
};

/**
 * Build a getScalePresetConfig function for the live chart. Returned as a
 * closure so UniversalChart can call resolveChartPanelDefinition without
 * importing the scale objects directly — the rates axis title depends on
 * whether rates are normalized.
 */
export function buildGetScalePresetConfig(normalizeRates: boolean): (preset: string) => Record<string, any> {
    const ratesScales = {
        y: {
            type: 'linear', display: true, position: 'left', min: 0, alignToPixels: true,
            title: { display: true, text: normalizeRates ? 'Normalized Rate (q/q₀)' : 'Rate (m³/day)' },
            ticks: { count: 6 },
        },
    };
    return (scalePreset: string) => {
        if (scalePreset === 'sweep') return SCALE_SWEEP;
        if (scalePreset === 'sweep_rf') return SWEEP_RF_SCALES;
        if (scalePreset === 'breakthrough') return BREAKTHROUGH_SCALES;
        if (scalePreset === 'pressure') return SCALE_PRESSURE;
        if (scalePreset === 'productivity') return SCALE_PRODUCTIVITY;
        if (scalePreset === 'shape_factor') return SCALE_SHAPE_FACTOR;
        if (scalePreset === 'ratio') return SCALE_RATIO;
        if (scalePreset === 'gor') return SCALE_GOR;
        if (scalePreset === 'cumulative') return SCALE_CUMULATIVE;
        if (scalePreset === 'cumulative_volumes') return SCALE_CUMULATIVE_VOLUMES;
        if (scalePreset === 'recovery') return RECOVERY_SCALES;
        if (scalePreset === 'diagnostics') return DIAGNOSTICS_SCALES;
        if (scalePreset === 'fraction') return SCALE_FRACTION;
        return ratesScales;
    };
}
