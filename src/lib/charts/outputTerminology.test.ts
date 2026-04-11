import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const comparisonChartSource = fs.readFileSync(path.join(__dirname, 'ReferenceComparisonChart.svelte'), 'utf8');
const rateChartSource = fs.readFileSync(path.join(__dirname, 'RateChart.svelte'), 'utf8');
const universalChartSource = fs.readFileSync(path.join(__dirname, 'UniversalChart.svelte'), 'utf8');
const swProfileSource = fs.readFileSync(path.join(__dirname, 'SwProfileChart.svelte'), 'utf8');
const appSource = fs.readFileSync(path.join(__dirname, '..', '..', 'App.svelte'), 'utf8');
const threeDViewCardSource = fs.readFileSync(path.join(__dirname, '..', 'ui', 'cards', 'ThreeDViewCard.svelte'), 'utf8');

describe('output terminology copy', () => {
  it('does not render the deprecated reference results summary table copy', () => {
    expect(appSource).not.toMatch(/Reference Run Results/);
    expect(appSource).not.toMatch(/ready for output review/);
    expect(appSource).not.toMatch(/Delta vs reference/);
  });

  it('uses output-comparison wording in the comparison chart shell and app empty states', () => {
    expect(comparisonChartSource).toMatch(/Comparison Plots/);
    expect(comparisonChartSource).toMatch(/Cases/);
    expect(comparisonChartSource).toMatch(/Analytical preview —/);
    expect(comparisonChartSource).toMatch(/analytical preview/);
    expect(comparisonChartSource).not.toMatch(/Stored Run Comparison/);
    expect(appSource).toMatch(/Results/);
    expect(appSource).not.toMatch(/Outputs/);
    expect(appSource).toMatch(/Loading output chart…/);
    // 3D loading strings live in ThreeDViewCard (extracted in Phase 7)
    expect(threeDViewCardSource).toMatch(/Loading 3D output\.\.\./);
    expect(threeDViewCardSource).toMatch(/Open 3D View/);
  });

  it('uses reference-solution wording in output-side solution cards and profile copy', () => {
    expect(universalChartSource).toMatch(/Reference Solution: \{mismatchSummary\.pointsCompared\} pts/);
    expect(universalChartSource).not.toMatch(/Analytical: \{mismatchSummary\.pointsCompared\} pts/);
    expect(swProfileSource).toMatch(/Reference Front Profile/);
    expect(swProfileSource).toMatch(/reference flood-front profile/);
    expect(swProfileSource).toMatch(/Reference front is near cell/);
    expect(swProfileSource).not.toMatch(/Analytical Front Profile/);
  });
});