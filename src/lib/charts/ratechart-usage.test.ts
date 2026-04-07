import { describe, it, expect } from 'vitest';
import fs from 'fs';
import path from 'path';

const rateChartPath = path.join(__dirname, 'RateChart.svelte');
const universalChartPath = path.join(__dirname, 'UniversalChart.svelte');
const buildRateChartDataPath = path.join(__dirname, 'buildRateChartData.ts');
const buildLiveDerivedSeriesPath = path.join(__dirname, 'buildLiveDerivedSeries.ts');
const buildUniversalChartDataPath = path.join(__dirname, 'buildUniversalChartData.ts');
const subPanelPath = path.join(__dirname, 'ChartSubPanel.svelte');
const referenceComparisonChartPath = path.join(__dirname, 'ReferenceComparisonChart.svelte');
const chartPanelSelectionPath = path.join(__dirname, 'chartPanelSelection.ts');
const rateChartSrc = fs.readFileSync(rateChartPath, 'utf8');
const universalChartSrc = fs.readFileSync(universalChartPath, 'utf8');
const buildRateChartDataSrc = fs.readFileSync(buildRateChartDataPath, 'utf8');
const buildLiveDerivedSeriesSrc = fs.readFileSync(buildLiveDerivedSeriesPath, 'utf8');
const buildUniversalChartDataSrc = fs.readFileSync(buildUniversalChartDataPath, 'utf8');
const subPanelSrc = fs.readFileSync(subPanelPath, 'utf8');
const referenceComparisonChartSrc = fs.readFileSync(referenceComparisonChartPath, 'utf8');
const chartPanelSelectionSrc = fs.readFileSync(chartPanelSelectionPath, 'utf8');

describe('RateChart architecture checks', () => {
  it('RateChart.svelte is a thin adapter that delegates to UniversalChart', () => {
    expect(/import\s+UniversalChart\s+from/.test(rateChartSrc)).toBe(true);
    expect(/<UniversalChart/.test(rateChartSrc)).toBe(true);
    // Thin adapter: should not contain any data computation or x-axis state
    expect(/buildRateChartData/.test(rateChartSrc)).toBe(false);
    expect(/let xAxisMode/.test(rateChartSrc)).toBe(false);
  });

  it('UniversalChart imports and uses ChartSubPanel', () => {
    expect(/import\s+ChartSubPanel\s+from/.test(universalChartSrc)).toBe(true);
    expect(/<ChartSubPanel/.test(universalChartSrc)).toBe(true);
  });

  it('RateChart (adapter) delegates series computation to buildLiveDerivedSeries', () => {
    expect(/from\s+['"]\.\/buildLiveDerivedSeries['"]/.test(rateChartSrc)).toBe(true);
    expect(/buildLiveDerivedSeries/.test(rateChartSrc)).toBe(true);
  });

  it('UniversalChart delegates panel building to buildUniversalChartData', () => {
    expect(/from\s+['"]\.\/buildUniversalChartData['"]/.test(universalChartSrc)).toBe(true);
    expect(/buildUniversalChartData/.test(universalChartSrc)).toBe(true);
    expect(/liveData/.test(universalChartSrc)).toBe(true);
  });

  it('defines three panel curve configs (rates, cumulative, diagnostics) in buildRateChartData', () => {
    expect(/ratesCurves/.test(buildRateChartDataSrc)).toBe(true);
    expect(/cumulativeCurves/.test(buildRateChartDataSrc)).toBe(true);
    expect(/diagnosticsCurves/.test(buildRateChartDataSrc)).toBe(true);
  });

  it('builds XY series for each panel in buildRateChartData', () => {
    expect(/ratesSeries/.test(buildRateChartDataSrc)).toBe(true);
    expect(/cumulativeSeries/.test(buildRateChartDataSrc)).toBe(true);
    expect(/diagnosticsSeries/.test(buildRateChartDataSrc)).toBe(true);
  });

  it('has x-axis control at the top level (not inside sub-panels)', () => {
    expect(/ToggleGroup/.test(universalChartSrc)).toBe(true);
    expect(/xAxisMode/.test(universalChartSrc)).toBe(true);
  });

  it('uses shared panel-selection helpers instead of duplicating shell logic', () => {
    expect(/chartPanelSelection/.test(universalChartSrc)).toBe(true);
    expect(/getConfiguredXAxisOptions/.test(universalChartSrc)).toBe(true);
    expect(/resolveChartPanelDefinition/.test(universalChartSrc)).toBe(true);
    expect(/chartPanelSelection/.test(referenceComparisonChartSrc)).toBe(true);
    expect(/getConfiguredXAxisOptions/.test(referenceComparisonChartSrc)).toBe(true);
    expect(/resolveChartPanelDefinition/.test(referenceComparisonChartSrc)).toBe(true);
    expect(/coerceChartAxisState/.test(chartPanelSelectionSrc)).toBe(true);
  });

  it('computes error stats (MAE, RMSE, MAPE)', () => {
    expect(/mismatchSummary/.test(universalChartSrc)).toBe(true);
  });

  it('buildLiveDerivedSeries computes rates, cumulatives, and diagnostics', () => {
    expect(/oilRate/.test(buildLiveDerivedSeriesSrc)).toBe(true);
    expect(/cumOil/.test(buildLiveDerivedSeriesSrc)).toBe(true);
    expect(/recoveryFactor/.test(buildLiveDerivedSeriesSrc)).toBe(true);
    expect(/buildMismatchSummary/.test(buildLiveDerivedSeriesSrc)).toBe(true);
  });

  it('buildUniversalChartData is domain-agnostic (no oil/water references)', () => {
    expect(/oilRate/.test(buildUniversalChartDataSrc)).toBe(false);
    expect(/waterRate/.test(buildUniversalChartDataSrc)).toBe(false);
    expect(/getData/.test(buildUniversalChartDataSrc)).toBe(true);
    expect(/getDataXY/.test(buildUniversalChartDataSrc)).toBe(true);
    expect(/applyCurveTypeStyle/.test(buildUniversalChartDataSrc)).toBe(true);
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
    expect(/curves\.length !== seriesData\.length/.test(subPanelSrc)).toBe(true);
  });

  it('warns when more than twenty comparison runs are visible', () => {
    expect(/MAX_RECOMMENDED_VISIBLE_CASES\s*=\s*20/.test(referenceComparisonChartSrc)).toBe(true);
    expect(/Showing \$\{visibleResults\.length\} runs\./.test(referenceComparisonChartSrc)).toBe(true);
  });

  it('contains axis bounds logic', () => {
    expect(/applyPositiveAxisBounds/.test(subPanelSrc)).toBe(true);
  });

  it('applies theme to its chart', () => {
    expect(/applyThemeToChart/.test(subPanelSrc)).toBe(true);
  });
});
