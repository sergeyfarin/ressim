import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const runControlsSource = fs.readFileSync(path.join(__dirname, 'cards', 'RunControls.svelte'), 'utf8');
const referenceExecutionSource = fs.readFileSync(path.join(__dirname, 'cards', 'ReferenceExecutionCard.svelte'), 'utf8');
const analyticalSectionSource = fs.readFileSync(path.join(__dirname, 'sections', 'AnalyticalSection.svelte'), 'utf8');
const warningPanelSource = fs.readFileSync(path.join(__dirname, 'feedback', 'WarningPolicyPanel.svelte'), 'utf8');
const modePanelSource = fs.readFileSync(path.join(__dirname, 'modes', 'ModePanel.svelte'), 'utf8');
const modePanelSectionsSource = fs.readFileSync(path.join(__dirname, 'modePanelSections.ts'), 'utf8');
const caseLibrarySource = fs.readFileSync(path.join(__dirname, '..', 'catalog', 'caseLibrary.ts'), 'utf8');

describe('ui terminology copy', () => {
  it('uses run-oriented labels in the run controls', () => {
    expect(runControlsSource).toMatch(/Run \{steps\} Step/);
    expect(runControlsSource).toMatch(/Advance 1 Step/);
    expect(runControlsSource).toMatch(/Stop Run/);
    expect(runControlsSource).toMatch(/Reset Model/);
    expect(runControlsSource).toMatch(/Review Inputs/);
    expect(runControlsSource).not.toMatch(/Reinit/);
  });

  it('uses run-set language in the reference execution card', () => {
    expect(referenceExecutionSource).toMatch(/Reference Runs/);
    expect(referenceExecutionSource).toMatch(/Run Set/);
    expect(referenceExecutionSource).toMatch(/Run Base Case/);
    expect(referenceExecutionSource).toMatch(/Stop Reference Runs/);
    expect(referenceExecutionSource).toMatch(/Selected run set:/);
    expect(referenceExecutionSource).not.toMatch(/Reference Execution/);
    expect(referenceExecutionSource).not.toMatch(/Execution Set/);
  });

  it('uses reference-solution language in the analytical section and warning sources', () => {
    expect(analyticalSectionSource).toMatch(/Reference Inputs/);
    expect(analyticalSectionSource).toMatch(/Reference Solution/);
    expect(analyticalSectionSource).not.toMatch(/Analytical Inputs/);
    expect(analyticalSectionSource).not.toMatch(/Analytical Model/);
    expect(warningPanelSource).toMatch(/return "Inputs"/);
    expect(warningPanelSource).toMatch(/return "Run"/);
    expect(warningPanelSource).toMatch(/return "Reference"/);
  });

  it('uses reference-guidance wording in the inputs disclosure and section labels', () => {
    expect(modePanelSource).toMatch(/Reference source:/);
    expect(modePanelSource).toMatch(/Primary review metric:/);
    expect(modePanelSource).toMatch(/Reference guidance now depends on whichever curated case you restore or activate next/);
    expect(modePanelSource).toMatch(/Single locked reference run/);
    expect(modePanelSource).toMatch(/Library sensitivity run set/);
    expect(modePanelSource).toMatch(/Reference review run/);
    expect(modePanelSectionsSource).toMatch(/label: "Reference Solution"/);
    expect(caseLibrarySource).toMatch(/No library sensitivity run set is exposed for this reference case/);
    expect(caseLibrarySource).toMatch(/No locked library sensitivity run set is defined for this starter case/);
    expect(modePanelSource).not.toMatch(/Reference basis:/);
    expect(modePanelSource).not.toMatch(/Primary comparison metric:/);
  });
});