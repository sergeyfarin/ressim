import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const comparisonChartSource = fs.readFileSync(path.join(__dirname, 'ReferenceComparisonChart.svelte'), 'utf8');
const spatialProfileSource = fs.readFileSync(path.join(__dirname, '..', 'visualization', 'SpatialProfileChart.svelte'), 'utf8');
const spatialProfileModelSource = fs.readFileSync(path.join(__dirname, '..', 'visualization', 'spatialProfileModel.ts'), 'utf8');
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
    // The mismatch-summary copy this used to assert lived in UniversalChart,
    // deleted with the unreferenced live-chart path on 2026-08-02.
    // The Sw profile moved to the 3D group as SpatialProfileChart (it shows one
    // snapshot, not a whole run) and now follows the shared property selector,
    // so its copy is property-neutral apart from the method-specific reference
    // overlay and the explicit Craig-to-diagonal disclosure.
    expect(spatialProfileSource).toMatch(/frontOverlay\.label/);
    expect(spatialProfileSource).toMatch(/Craig E_A is mapped to the diagonal/);
    expect(spatialProfileModelSource).toMatch(/Buckley–Leverett reference/);
    expect(spatialProfileModelSource).toMatch(/Craig \+ BL reference/);
    expect(spatialProfileModelSource).toMatch(/Craig \+ Stiles \+ BL reference/);
    expect(spatialProfileSource).toMatch(/Profile/);
    expect(spatialProfileSource).not.toMatch(/Analytical Front Profile/);
  });
});
