import { Chart, type ChartDataset } from 'chart.js';

/**
 * Return the typed line dataset for a given chart index or undefined.
 * Use this everywhere instead of indexing `chart.data.datasets[...]` directly.
 */
export function getLineDataset<T = Array<number | null>>(chart: Chart | null | undefined, datasetIndex: number): ChartDataset<'line', T> | undefined {
  return chart?.data?.datasets?.[datasetIndex] as ChartDataset<'line', T> | undefined;
}

/**
 * Safely assign `data` to a dataset if present — returns true on success.
 */
export function safeSetDatasetData<D = unknown>(chart: Chart | null | undefined, datasetIndex: number, data: D): boolean {
  const ds = getLineDataset<D>(chart, datasetIndex);
  if (!ds) return false;
  ds.data = data as unknown as D;
  return true;
}

/**
 * Helper to read a dataset label or `undefined` when absent.
 */
export function getDatasetLabel(chart: Chart | null | undefined, datasetIndex: number): string | undefined {
  return getLineDataset(chart, datasetIndex)?.label as string | undefined;
}

/**
 * Apply theme-specific grid colors to common chart scales.
 * Kept small and side-effecting so it can be called from components.
 */
export function applyThemeToChart(chart: Chart | null | undefined, theme: 'dark' | 'light') {
  if (!chart) return;
  const gridColor = theme === 'dark' ? 'rgba(203, 213, 225, 0.07)' : 'rgba(15, 23, 42, 0.10)';
  const textColor = theme === 'dark' ? '#9ca3af' : '#4b5563'; // Tailwind slate-400 vs gray-600

  Chart.defaults.color = textColor;

  // chart.options.scales is a loose object in Chart.js typing — guard access
  const scales = (chart.options && (chart.options as any).scales) ?? {};
  for (const axisId of Object.keys(scales)) {
    const scale = scales[axisId];
    if (!scale) continue;
    if (scale.grid) scale.grid.color = gridColor;
    if (scale.ticks) scale.ticks.color = textColor;
  }
  try {
    chart.update();
  } catch (e) {
    // chart.update might throw in tests if chart is a partial mock; ignore safely
  }
}

