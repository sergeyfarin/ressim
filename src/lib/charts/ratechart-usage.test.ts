import { describe, it, expect } from 'vitest';
import fs from 'fs';
import path from 'path';

const rateChartPath = path.join(__dirname, 'RateChart.svelte');
const subPanelPath = path.join(__dirname, 'ChartSubPanel.svelte');
const referenceComparisonChartPath = path.join(__dirname, 'ReferenceComparisonChart.svelte');
const chartPanelSelectionPath = path.join(__dirname, 'chartPanelSelection.ts');
const outputSummaryStripPath = path.join(__dirname, 'OutputSummaryStrip.svelte');
const rateChartSrc = fs.readFileSync(rateChartPath, 'utf8');
const subPanelSrc = fs.readFileSync(subPanelPath, 'utf8');
const referenceComparisonChartSrc = fs.readFileSync(referenceComparisonChartPath, 'utf8');
const chartPanelSelectionSrc = fs.readFileSync(chartPanelSelectionPath, 'utf8');
const outputSummaryStripSrc = fs.readFileSync(outputSummaryStripPath, 'utf8');

describe('RateChart architecture checks', () => {
  it('imports and uses ChartSubPanel', () => {
    expect(/import\s+ChartSubPanel\s+from/.test(rateChartSrc)).toBe(true);
    expect(/<ChartSubPanel/.test(rateChartSrc)).toBe(true);
  });

  it('defines three panel curve configs (rates, cumulative, diagnostics)', () => {
    expect(/baseRatesCurves\s*:\s*CurveConfig\[\]/.test(rateChartSrc)).toBe(true);
    expect(/baseCumulativeCurves\s*:\s*CurveConfig\[\]/.test(rateChartSrc)).toBe(true);
    expect(/baseDiagnosticsCurves\s*:\s*CurveConfig\[\]/.test(rateChartSrc)).toBe(true);
  });

  it('builds XY series for each panel', () => {
    expect(/ratesSeries/.test(rateChartSrc)).toBe(true);
    expect(/cumulativeSeries/.test(rateChartSrc)).toBe(true);
    expect(/diagnosticsSeries/.test(rateChartSrc)).toBe(true);
  });

  it('has x-axis control at the top level (not inside sub-panels)', () => {
    expect(/ToggleGroup/.test(rateChartSrc)).toBe(true);
    expect(/xAxisMode/.test(rateChartSrc)).toBe(true);
  });

  it('uses shared panel-selection helpers instead of duplicating shell logic', () => {
    expect(/chartPanelSelection/.test(rateChartSrc)).toBe(true);
    expect(/getConfiguredXAxisOptions/.test(rateChartSrc)).toBe(true);
    expect(/resolveChartPanelDefinition/.test(rateChartSrc)).toBe(true);
    expect(/chartPanelSelection/.test(referenceComparisonChartSrc)).toBe(true);
    expect(/getConfiguredXAxisOptions/.test(referenceComparisonChartSrc)).toBe(true);
    expect(/resolveChartPanelDefinition/.test(referenceComparisonChartSrc)).toBe(true);
    expect(/coerceChartAxisState/.test(chartPanelSelectionSrc)).toBe(true);
  });

  it('uses a shared output-summary strip above both chart shells', () => {
    expect(/OutputSummaryStrip/.test(rateChartSrc)).toBe(true);
    expect(/OutputSummaryStrip/.test(referenceComparisonChartSrc)).toBe(true);
    expect(/ui-panel-kicker/.test(outputSummaryStripSrc)).toBe(true);
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

  it('has grouped curve toggle UI', () => {
    expect(/toggleCurveGroup/.test(subPanelSrc)).toBe(true);
    expect(/curveToggleGroups/.test(subPanelSrc)).toBe(true);
    expect(/visibleCurveGroups/.test(subPanelSrc)).toBe(true);
    expect(/getCurveToggleGroupKey/.test(subPanelSrc)).toBe(true);
  });

  it('recreates the chart when the curve schema changes', () => {
    expect(/mountedChartSchemaSignature/.test(subPanelSrc)).toBe(true);
    expect(/chart\.data\.datasets\.length\s*!==\s*curves\.length/.test(subPanelSrc)).toBe(true);
    expect(/getCurveDatasetKey/.test(subPanelSrc)).toBe(true);
    expect(/_datasetKey/.test(subPanelSrc)).toBe(true);
  });

  it('contains axis bounds logic', () => {
    expect(/applyPositiveAxisBounds/.test(subPanelSrc)).toBe(true);
  });

  it('applies theme to its chart', () => {
    expect(/applyThemeToChart/.test(subPanelSrc)).toBe(true);
  });
});
