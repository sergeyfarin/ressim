import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const appCssSource = fs.readFileSync(path.join(__dirname, 'app.css'), 'utf8');
const appSource = fs.readFileSync(path.join(__dirname, 'App.svelte'), 'utf8');
const scenarioPickerSource = fs.readFileSync(path.join(__dirname, 'lib', 'ui', 'modes', 'ScenarioPicker.svelte'), 'utf8');
const referenceExecutionSource = fs.readFileSync(path.join(__dirname, 'lib', 'ui', 'cards', 'ReferenceExecutionCard.svelte'), 'utf8');
const referenceResultsSource = fs.readFileSync(path.join(__dirname, 'lib', 'ui', 'cards', 'ReferenceResultsCard.svelte'), 'utf8');
const warningPanelSource = fs.readFileSync(path.join(__dirname, 'lib', 'ui', 'feedback', 'WarningPolicyPanel.svelte'), 'utf8');
const comparisonChartSource = fs.readFileSync(path.join(__dirname, 'lib', 'charts', 'ReferenceComparisonChart.svelte'), 'utf8');
const threeDViewSource = fs.readFileSync(path.join(__dirname, 'lib', 'visualization', '3dview.svelte'), 'utf8');

describe('app theme typography', () => {
  it('uses a shared sans/mono font system and inherits it through form controls', () => {
    expect(appCssSource).toMatch(/IBM\+Plex\+Sans/);
    expect(appCssSource).toMatch(/IBM\+Plex\+Mono/);
    expect(appCssSource).toMatch(/--font-sans: 'IBM Plex Sans'/);
    expect(appCssSource).toMatch(/--font-mono: 'IBM Plex Mono'/);
    expect(appCssSource).toMatch(/font-family: var\(--font-sans\);/);
    expect(appCssSource).toMatch(/button,[\s\S]*input,[\s\S]*select,[\s\S]*textarea[\s\S]*font: inherit;/);
  });

  it('defines semantic typography utilities for headings, chips, and microcopy', () => {
    expect(appCssSource).toMatch(/\.ui-section-kicker/);
    expect(appCssSource).toMatch(/\.ui-panel-kicker/);
    expect(appCssSource).toMatch(/\.ui-subsection-kicker/);
    expect(appCssSource).toMatch(/\.ui-card-title/);
    expect(appCssSource).toMatch(/\.ui-support-copy/);
    expect(appCssSource).toMatch(/\.ui-microcopy/);
    expect(appCssSource).toMatch(/\.ui-chip/);
    expect(appCssSource).toMatch(/\.ui-chip-caps/);
  });

  it('uses the shared typography utilities across the main shell and output surfaces', () => {
    expect(appSource).toMatch(/ui-section-kicker/);
    expect(scenarioPickerSource).toMatch(/ui-panel-kicker/);
    expect(scenarioPickerSource).toMatch(/ui-chip/);
    expect(referenceExecutionSource).toMatch(/ui-support-copy/);
    expect(referenceResultsSource).toMatch(/ui-subsection-kicker/);
    expect(warningPanelSource).toMatch(/ui-chip-caps/);
    expect(comparisonChartSource).toMatch(/ui-section-kicker/);
    expect(threeDViewSource).toMatch(/ui-chip/);
  });
});