import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const referenceResultsSource = fs.readFileSync(path.join(__dirname, '..', 'ui', 'cards', 'ReferenceResultsCard.svelte'), 'utf8');
const comparisonChartSource = fs.readFileSync(path.join(__dirname, 'ReferenceComparisonChart.svelte'), 'utf8');
const rateChartSource = fs.readFileSync(path.join(__dirname, 'RateChart.svelte'), 'utf8');
const swProfileSource = fs.readFileSync(path.join(__dirname, 'SwProfileChart.svelte'), 'utf8');
const appSource = fs.readFileSync(path.join(__dirname, '..', '..', 'App.svelte'), 'utf8');

describe('output terminology copy', () => {
  it('uses output-review wording in the reference results summary card', () => {
    expect(referenceResultsSource).toMatch(/Reference Run Results/);
    expect(referenceResultsSource).toMatch(/ready for output review/);
    expect(referenceResultsSource).toMatch(/BT PVI/);
    expect(referenceResultsSource).toMatch(/BT Time \(d\)/);
    expect(referenceResultsSource).toMatch(/Delta vs reference/);
    expect(referenceResultsSource).toMatch(/Status/);
    expect(referenceResultsSource).toMatch(/Base case/);
    expect(referenceResultsSource).toMatch(/Variant/);
    expect(referenceResultsSource).not.toMatch(/Stored Reference Results/);
    expect(referenceResultsSource).not.toMatch(/Base Reference/);
  });

  it('uses output-comparison wording in the comparison chart shell and app empty states', () => {
    expect(comparisonChartSource).toMatch(/Comparison Plots/);
    expect(comparisonChartSource).toMatch(/Cases/);
    expect(comparisonChartSource).toMatch(/Analytical preview —/);
    expect(comparisonChartSource).toMatch(/comparison-sweep-combined/);
    expect(comparisonChartSource).not.toMatch(/Stored Run Comparison/);
    expect(appSource).toMatch(/Results/);
    expect(appSource).not.toMatch(/Outputs/);
    expect(appSource).toMatch(/Loading output chart…/);
    expect(appSource).toMatch(/Loading 3D output\.\.\./);
    expect(appSource).toMatch(/Open 3D View/);
  });

  it('uses reference-solution wording in output-side solution cards and profile copy', () => {
    expect(appSource).toMatch(/Depletion Reference Solution/);
    expect(appSource).toMatch(/Waterflood Reference Solution/);
    expect(appSource).toMatch(/Reference solution: Buckley-Leverett fractional flow/);
    expect(rateChartSource).toMatch(/Reference Solution: \{mismatchSummary\.pointsCompared\} pts/);
    expect(rateChartSource).not.toMatch(/Analytical: \{mismatchSummary\.pointsCompared\} pts/);
    expect(swProfileSource).toMatch(/Reference Front Profile/);
    expect(swProfileSource).toMatch(/reference flood-front profile/);
    expect(swProfileSource).toMatch(/Reference front is near cell/);
    expect(swProfileSource).not.toMatch(/Analytical Front Profile/);
  });
});