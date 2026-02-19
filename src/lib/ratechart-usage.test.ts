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

  it('handles custom analytical sub-case keys in scenario selection', () => {
    expect(/depletion_custom_subcase\s*:\s*\[/.test(src)).toBe(true);
    expect(/waterflood_custom_subcase\s*:\s*\[/.test(src)).toBe(true);
    expect(/normalizedCase\s*===\s*'waterflood_custom_subcase'/.test(src)).toBe(true);

    expect(
      /depletion_custom_subcase\s*:\s*\[\s*DATASET_INDEX\.OIL_RATE\s*,\s*DATASET_INDEX\.ANALYTICAL_OIL_RATE\s*,\s*DATASET_INDEX\.CUM_OIL\s*,\s*DATASET_INDEX\.ANALYTICAL_CUM_OIL\s*,\s*DATASET_INDEX\.OIL_RATE_ABS_ERROR\s*,\s*DATASET_INDEX\.AVG_PRESSURE\s*,?\s*\]/s.test(src)
    ).toBe(true);

    expect(
      /waterflood_custom_subcase\s*:\s*\[\s*DATASET_INDEX\.RF_VS_PVI\s*,\s*DATASET_INDEX\.WATERCUT_SIM_VS_PVI\s*,\s*DATASET_INDEX\.WATERCUT_ANALYTICAL_VS_PVI\s*,?\s*\]/s.test(src)
    ).toBe(true);
  });

  it('keeps pressure on a dedicated axis and applies dynamic fraction-axis bounds', () => {
    expect(/label:\s*'Average Reservoir Pressure'[\s\S]*?yAxisID:\s*'y3'/.test(src)).toBe(true);
    expect(/scales\.y3\)\s*scales\.y3\.display\s*=\s*activeAxisIds\.has\('y3'\)/.test(src)).toBe(true);
    expect(/function\s+applyFractionAxisBounds\s*\(/.test(src)).toBe(true);
    expect(/axis\.max\s*=\s*Math\.min\(1,\s*niceUpperBound\(targetMax\)\)/.test(src)).toBe(true);
  });

  it('reapplies selection inside updateChart', () => {
    expect(/const\s+effectiveSelection\s*=\s*selectedDatasetIndexes\.length\s*>\s*0[\s\S]*?applySelection\(effectiveSelection\)/.test(src)).toBe(true);
  });
});
