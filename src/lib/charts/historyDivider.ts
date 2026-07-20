/**
 * historyDivider.ts — history/forecast chart divider (E5).
 *
 * Draws a shaded "history" region up to a scenario-declared boundary plus a
 * dashed divider line and "History"/"Forecast" labels, so match-then-forecast
 * cases can visually separate observed history from the extrapolated forecast.
 *
 * Implemented as an inline Chart.js v4 plugin (no `chartjs-plugin-annotation`
 * dependency — CLAUDE.md forbids new deps unless essential). The pure
 * resolve/color helpers here are unit-tested; the canvas drawing itself has no
 * headless check.
 */

import type { Chart, Plugin } from 'chart.js';
import type { HistoryWindow } from '../catalog/scenarios';

export type ResolvedHistoryDivider = {
    boundary: number;
    historyLabel: string;
    forecastLabel: string;
};

/**
 * Resolve a scenario's `historyWindow` against the chart's active x-axis.
 * Returns null (divider suppressed) when the window is absent, the boundary is
 * non-finite, or the window is declared for a different axis than the one shown.
 */
export function resolveHistoryDivider(
    window: HistoryWindow | null | undefined,
    xAxisMode: string,
): ResolvedHistoryDivider | null {
    if (!window || !Number.isFinite(window.boundary)) return null;
    const axis = window.axis ?? 'time';
    if (axis !== xAxisMode) return null;
    return {
        boundary: window.boundary,
        historyLabel: window.historyLabel ?? 'History',
        forecastLabel: window.forecastLabel ?? 'Forecast',
    };
}

export type HistoryDividerColors = {
    shade: string;
    line: string;
    text: string;
};

/** Theme-aware colors for the shaded region, divider line, and labels. */
export function historyDividerColors(theme: 'dark' | 'light'): HistoryDividerColors {
    return theme === 'dark'
        ? { shade: 'rgba(148,163,184,0.10)', line: 'rgba(148,163,184,0.55)', text: 'rgba(203,213,225,0.9)' }
        : { shade: 'rgba(100,116,139,0.08)', line: 'rgba(71,85,105,0.5)', text: 'rgba(51,65,85,0.9)' };
}

export type HistoryDividerPluginOptions = {
    boundary?: number;
    historyLabel?: string;
    forecastLabel?: string;
    colors?: HistoryDividerColors;
};

export const HISTORY_DIVIDER_PLUGIN_ID = 'historyDivider';

/**
 * Inline Chart.js plugin. Reads its options from
 * `chart.options.plugins.historyDivider`. A no-op when the boundary is
 * unset/non-finite or falls outside the current plotted x-range.
 */
export function createHistoryDividerPlugin(): Plugin<'line'> {
    return {
        id: HISTORY_DIVIDER_PLUGIN_ID,
        beforeDatasetsDraw(chart: Chart, _args, opts: HistoryDividerPluginOptions) {
            const boundary = opts?.boundary;
            if (boundary == null || !Number.isFinite(boundary)) return;
            const xScale = chart.scales.x;
            const area = chart.chartArea;
            if (!xScale || !area) return;

            const px = xScale.getPixelForValue(boundary);
            const { left, right, top, bottom } = area;
            // Skip when the boundary is off-chart (e.g. wrong axis leftover, or
            // the run hasn't reached the boundary yet).
            if (!Number.isFinite(px) || px <= left || px >= right) return;

            const colors = opts.colors ?? historyDividerColors('dark');
            const ctx = chart.ctx;
            ctx.save();

            // Shaded history region [left, boundary].
            ctx.fillStyle = colors.shade;
            ctx.fillRect(left, top, px - left, bottom - top);

            // Dashed divider line at the boundary.
            ctx.strokeStyle = colors.line;
            ctx.lineWidth = 1;
            ctx.setLineDash([4, 4]);
            ctx.beginPath();
            ctx.moveTo(px, top);
            ctx.lineTo(px, bottom);
            ctx.stroke();

            // Labels flanking the divider at the top of the plot area.
            ctx.setLineDash([]);
            ctx.fillStyle = colors.text;
            ctx.font = "10px 'JetBrains Mono', monospace";
            ctx.textBaseline = 'top';
            ctx.textAlign = 'right';
            ctx.fillText(opts.historyLabel ?? 'History', px - 5, top + 4);
            ctx.textAlign = 'left';
            ctx.fillText(opts.forecastLabel ?? 'Forecast', px + 5, top + 4);

            ctx.restore();
        },
    };
}
