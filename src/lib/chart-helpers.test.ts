import { describe, it, expect } from 'vitest';
import type { Chart } from 'chart.js';
import { getLineDataset, safeSetDatasetData, getDatasetLabel, applyThemeToChart } from './chart-helpers';

describe('chart-helpers', () => {
  const fakeChart = {
    data: {
      datasets: [
        { label: 'A', data: [1, 2, 3] },
        { label: 'B', data: [] },
      ],
    },
    options: { scales: { x: { grid: {} }, y: { grid: {} } } },
    update: () => {},
  } as unknown as Chart<'line', any[], any>;

  it('getLineDataset returns dataset when present', () => {
    const ds0 = getLineDataset(fakeChart, 0);
    expect(ds0).toBeDefined();
    expect(ds0?.label).toBe('A');

    const ds1 = getLineDataset(fakeChart, 1);
    expect(ds1).toBeDefined();
    expect(ds1?.label).toBe('B');

    const missing = getLineDataset(fakeChart, 9);
    expect(missing).toBeUndefined();
  });

  it('safeSetDatasetData assigns data and returns true/false appropriately', () => {
    const ok = safeSetDatasetData(fakeChart, 1, [{ x: 0, y: 0 }]);
    expect(ok).toBe(true);
    expect((fakeChart.data.datasets[1] as any).data).toEqual([{ x: 0, y: 0 }]);

    const nok = safeSetDatasetData(fakeChart, 9, [1, 2, 3]);
    expect(nok).toBe(false);
  });

  it('getDatasetLabel returns label or undefined', () => {
    expect(getDatasetLabel(fakeChart, 0)).toBe('A');
    expect(getDatasetLabel(fakeChart, 9)).toBeUndefined();
  });

  it('applyThemeToChart updates scale grid color safely', () => {
    // dark theme
    applyThemeToChart(fakeChart, 'dark');
    const scales = (fakeChart.options as any).scales;
    expect(scales.x.grid.color).toBe('rgba(203, 213, 225, 0.07)');

    // light theme
    applyThemeToChart(fakeChart, 'light');
    expect(scales.x.grid.color).toBe('rgba(15, 23, 42, 0.10)');

    // null chart safe no-op
    expect(() => applyThemeToChart(null, 'dark')).not.toThrow();
  });
});
