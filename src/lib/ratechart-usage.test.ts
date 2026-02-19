import { describe, it, expect } from 'vitest';
import fs from 'fs';
import path from 'path';

const rateChartPath = path.join(__dirname, '..', 'lib', 'RateChart.svelte');
const src = fs.readFileSync(rateChartPath, 'utf8');

describe('RateChart implementation checks', () => {
  it('does not declare a local getLineDataset function', () => {
    expect(/function\s+getLineDataset\s*\(/.test(src)).toBe(false);
  });

  it('uses shared chart helpers for dataset operations', () => {
    expect(/safeSetDatasetData\(|getDatasetLabel\(chart/.test(src)).toBe(true);
  });
});
