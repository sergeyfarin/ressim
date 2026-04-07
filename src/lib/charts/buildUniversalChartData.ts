/**
 * buildUniversalChartData.ts — scenario-agnostic panel builder.
 *
 * Takes UniversalPanelDef[] (declared by the scenario) and a LiveCurveContext
 * (built by UniversalChart.svelte) and returns UniversalChartResult ready
 * for ChartSubPanel rendering.
 *
 * This function has ZERO knowledge of oil rates, water cuts, sweep geometry,
 * or any other domain concept. It only:
 *   1. Resolves the curve color ('neutral' → ctx.neutralColor)
 *   2. Calls getData or getDataXY per curve
 *   3. Applies curveType → borderWidth/borderDash via applyCurveTypeStyle
 *   4. Zips y-values with ctx.xValues into XYPoints
 *   5. Returns UniversalChartResult
 */

import type { CurveConfig } from './chartTypes';
import { applyCurveTypeStyle, CURVE_TYPE_LEGEND_SECTION } from './curveStylePolicy';
import { buildMismatchSummary } from './buildLiveDerivedSeries';
import type {
    LiveCurveContext,
    UniversalPanelDef,
    UniversalChartDataSet,
    UniversalChartResult,
} from './universalChartTypes';

// ─── XY helper ────────────────────────────────────────────────────────────────

function toXYSeries(
    xValues: Array<number | null>,
    yValues: Array<number | null>,
): Array<{ x: number; y: number | null }> {
    const out: Array<{ x: number; y: number | null }> = [];
    for (let i = 0; i < yValues.length; i++) {
        const rawX = xValues[i];
        if (!Number.isFinite(rawX)) continue;
        const rawY = yValues[i];
        out.push({ x: Number(rawX), y: Number.isFinite(rawY) ? Number(rawY) : null });
    }
    return out;
}

// ─── Main builder ─────────────────────────────────────────────────────────────

export function buildUniversalChartData(
    panelDefs: UniversalPanelDef[],
    ctx: LiveCurveContext,
): UniversalChartResult {
    const datasets: UniversalChartDataSet[] = [];

    for (const panelDef of panelDefs) {
        const curves: CurveConfig[] = [];
        const series: Array<{ x: number; y: number | null }[]> = [];

        for (const curveDef of panelDef.curves) {
            // Resolve color sentinel
            const color = curveDef.color === 'neutral' ? ctx.neutralColor : curveDef.color;

            // Apply automatic styling from curveType
            const typeStyle = applyCurveTypeStyle(curveDef.curveType);

            const curve: CurveConfig = {
                label:          curveDef.label,
                curveKey:       curveDef.key,
                color,
                yAxisID:        curveDef.yAxisID,
                defaultVisible: curveDef.defaultVisible,
                toggleGroupKey: curveDef.toggleGroupKey,
                legendSection:  CURVE_TYPE_LEGEND_SECTION[curveDef.curveType],
                ...typeStyle,
            };
            curves.push(curve);

            // Build series data
            let pts: Array<{ x: number; y: number | null }>;
            if (curveDef.getDataXY) {
                pts = curveDef.getDataXY(ctx);
            } else if (curveDef.getData) {
                pts = toXYSeries(ctx.xValues, curveDef.getData(ctx));
            } else {
                pts = [];
            }
            series.push(pts);
        }

        datasets.push({ panelKey: panelDef.panelKey, curves, series });
    }

    // ── Derived metadata ───────────────────────────────────────────────────────
    const pviAvailable = (ctx.pviArr.at(-1) ?? 0) > 1e-12;
    const pvpAvailable = (ctx.sim.pvp.at(-1) ?? 0) > 1e-12;
    const mismatchSummary = buildMismatchSummary(ctx.sim, ctx.analytical, ctx.scaleFactor);

    return {
        datasets,
        mismatchSummary,
        pviAvailable,
        pvpAvailable,
        xValues: ctx.xValues,
        pviArr:  ctx.pviArr,
    };
}
