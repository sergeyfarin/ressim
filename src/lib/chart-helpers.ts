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

/**
 * Custom Plotly-style HTML Tooltip handler.
 * Renders tooltip body labels equipped with explicit SVG-style dashed/solid CSS borders instead of basic square colored boxes.
 */
export function externalTooltipHandler(context: any) {
  const { chart, tooltip } = context;

  // Find or create the DOM element for the tooltip
  let tooltipEl = chart.canvas.parentNode.querySelector('div.chartjs-tooltip');
  if (!tooltipEl) {
    tooltipEl = document.createElement('div');
    tooltipEl.className = 'chartjs-tooltip';
    tooltipEl.style.background = 'rgba(30, 30, 30, 0.85)';
    tooltipEl.style.color = '#fff';
    tooltipEl.style.borderRadius = '6px';
    tooltipEl.style.pointerEvents = 'none';
    tooltipEl.style.position = 'absolute';
    tooltipEl.style.transform = 'translate(-50%, 0)';
    tooltipEl.style.transition = 'all .1s ease';
    tooltipEl.style.fontFamily = "'JetBrains Mono', monospace";
    tooltipEl.style.fontSize = '11px';
    tooltipEl.style.padding = '8px';
    tooltipEl.style.boxShadow = '0 2px 8px rgba(0,0,0,0.2)';
    tooltipEl.style.zIndex = '50'; // Make sure it sits above the chart elements

    chart.canvas.parentNode.appendChild(tooltipEl);
  }

  // Hide if no tooltip
  if (tooltip.opacity === 0) {
    tooltipEl.style.opacity = '0';
    return;
  }

  // Set Text inside the custom HTML tooltip
  if (tooltip.body) {
    const titleLines = tooltip.title || [];
    const bodyLines = tooltip.body.map((b: any) => b.lines);

    let innerHtml = '<div style="margin-bottom: 4px; font-weight: bold; font-size: 11px; opacity: 0.9;">';
    titleLines.forEach((title: string) => {
      innerHtml += '<div>' + title + '</div>';
    });
    innerHtml += '</div>';
    innerHtml += '<div style="display: flex; flex-direction: column; gap: 4px;">';

    bodyLines.forEach((body: string, i: number) => {
      const colors = tooltip.labelColors[i];
      const dataPoint = tooltip.dataPoints[i];
      const datasetIndex = dataPoint.datasetIndex;
      const dataset = chart.data.datasets[datasetIndex];

      const isDashed = dataset.borderDash && dataset.borderDash.length > 0;
      const borderStyle = isDashed ? 'dashed' : 'solid';
      const borderColor = colors.borderColor || dataset.borderColor || '#ccc';
      const borderWidth = dataset.borderWidth || 2;

      // Plotly-style line prefix
      const lineMarker = `<span style="display:inline-block; vertical-align:middle; width:20px; border-top:${borderWidth}px ${borderStyle} ${borderColor}; margin-right:6px;"></span>`;

      innerHtml += `<div style="display: flex; align-items: center;">${lineMarker}<span style="font-size: 11px;">${body}</span></div>`;
    });
    innerHtml += '</div>';

    tooltipEl.innerHTML = innerHtml;
  }

  // Use offset properties of canvas to accurately position the tooltip within the relative parent DIV
  const { offsetLeft: positionX, offsetTop: positionY } = chart.canvas;

  tooltipEl.style.opacity = '1';
  // Small vertical adjustment to make it float neatly above the pointer
  tooltipEl.style.left = positionX + tooltip.caretX + 'px';
  tooltipEl.style.top = positionY + tooltip.caretY - 8 + 'px';
}

