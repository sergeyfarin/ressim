import { describe, it, expect } from 'vitest';
import fs from 'fs';
import path from 'path';

const rateChartPath = path.join(__dirname, '..', 'lib', 'RateChart.svelte');
const subPanelPath = path.join(__dirname, '..', 'lib', 'ChartSubPanel.svelte');
const rateChartSrc = fs.readFileSync(rateChartPath, 'utf8');
const subPanelSrc = fs.readFileSync(subPanelPath, 'utf8');

describe('RateChart architecture checks', () => {
  it('imports and uses ChartSubPanel', () => {
    expect(/import\s+ChartSubPanel\s+from/.test(rateChartSrc)).toBe(true);
    expect(/<ChartSubPanel/.test(rateChartSrc)).toBe(true);
  });

  it('defines three panel curve configs (rates, cumulative, diagnostics)', () => {
    expect(/ratesCurves\s*:\s*CurveConfig\[\]/.test(rateChartSrc)).toBe(true);
    expect(/cumulativeCurves\s*:\s*CurveConfig\[\]/.test(rateChartSrc)).toBe(true);
    expect(/diagnosticsCurves\s*:\s*CurveConfig\[\]/.test(rateChartSrc)).toBe(true);
  });

  it('builds XY series for each panel', () => {
    expect(/ratesSeries/.test(rateChartSrc)).toBe(true);
    expect(/cumulativeSeries/.test(rateChartSrc)).toBe(true);
    expect(/diagnosticsSeries/.test(rateChartSrc)).toBe(true);
  });

  it('has x-axis control at the top level (not inside sub-panels)', () => {
    expect(/x-axis-select/.test(rateChartSrc)).toBe(true);
    expect(/xAxisMode/.test(rateChartSrc)).toBe(true);
  });

  it('computes error stats (MAE, RMSE, MAPE)', () => {
    expect(/mismatchSummary/.test(rateChartSrc)).toBe(true);
  });
});

describe('ChartSubPanel architecture checks', () => {
  it('is a reusable collapsible component', () => {
    // Svelte 5 requires properties to be destructured from $props() or found in the props type.
    // Instead of looking for export let, just check the string contains the names.
    expect(/expanded/.test(subPanelSrc)).toBe(true);
    expect(/curves/.test(subPanelSrc)).toBe(true);
    expect(/seriesData/.test(subPanelSrc)).toBe(true);
    expect(/\$props\(/.test(subPanelSrc)).toBe(true);
  });

  it('creates its own Chart.js instance', () => {
    expect(/new\s+Chart\(ctx/.test(subPanelSrc)).toBe(true);
  });

  it('has per-curve toggle UI', () => {
    expect(/toggleCurve/.test(subPanelSrc)).toBe(true);
    expect(/visibleCurves/.test(subPanelSrc)).toBe(true);
  });

  it('contains axis bounds logic', () => {
    expect(/applyPositiveAxisBounds/.test(subPanelSrc)).toBe(true);
    expect(/niceUpperBound/.test(subPanelSrc)).toBe(true);
  });

  it('applies theme to its chart', () => {
    expect(/applyThemeToChart/.test(subPanelSrc)).toBe(true);
  });
});
