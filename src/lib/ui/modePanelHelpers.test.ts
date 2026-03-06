import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';
import { MODE_PANEL_SECTIONS, getModePanelSections } from './modePanelSections';

const geometryEditorPath = path.join(__dirname, 'GeometryGridQuickEditor.svelte');
const geometryEditorSource = fs.readFileSync(geometryEditorPath, 'utf8');

describe('mode panel helpers', () => {
  it('defines geometry section metadata for the mode panels', () => {
    const geometry = MODE_PANEL_SECTIONS.find((section) => section.key === 'geometry');

    expect(geometry).toBeDefined();
    expect(geometry?.dims).toEqual(['geo', 'grid']);
  });

  it('keeps geometry quick-pick definitions local to the geometry component', () => {
    expect(geometryEditorSource).toMatch(/const GEOMETRY_GRID_CONTROLS/);
    expect(geometryEditorSource).toMatch(/function getQuickPickMatch/);
    expect(geometryEditorSource).toMatch(/function getControlErrorMessage/);
  });

  it('defines nx quick picks with inline custom entry behavior', () => {
    expect(geometryEditorSource).toContain('{ key: "12", label: "12", patch: { nx: 12 } }');
    expect(geometryEditorSource).toContain('{ key: "24", label: "24", patch: { nx: 24 } }');
    expect(geometryEditorSource).toContain('{ key: "48", label: "48", patch: { nx: 48 } }');
    expect(geometryEditorSource).toContain('{ key: "96", label: "96", patch: { nx: 96 } }');
    expect(geometryEditorSource).toContain('customLabel: "Custom X cells"');
    expect(geometryEditorSource).toContain('integer: true');
  });

  it('uses toggle-plus-custom controls for the rest of the geometry editor', () => {
    expect(geometryEditorSource).toContain('{ key: "1", label: "1", patch: { ny: 1 } }');
    expect(geometryEditorSource).toContain('{ key: "11", label: "11", patch: { ny: 11 } }');
    expect(geometryEditorSource).toContain('{ key: "21", label: "21", patch: { ny: 21 } }');
    expect(geometryEditorSource).toContain('{ key: "41", label: "41", patch: { ny: 41 } }');
    expect(geometryEditorSource).toContain('{ key: "1", label: "1", patch: { nz: 1 } }');
    expect(geometryEditorSource).toContain('{ key: "3", label: "3", patch: { nz: 3 } }');
    expect(geometryEditorSource).toContain('{ key: "5", label: "5", patch: { nz: 5 } }');
    expect(geometryEditorSource).toContain('{ key: "10", label: "10", patch: { nz: 10 } }');
    expect(geometryEditorSource).toContain('changeBehavior: "sync-layer-arrays"');
    expect(geometryEditorSource).toContain('{ key: "5", label: "5 m", patch: { cellDx: 5 } }');
    expect(geometryEditorSource).toContain('{ key: "10", label: "10 m", patch: { cellDx: 10 } }');
    expect(geometryEditorSource).toContain('{ key: "20", label: "20 m", patch: { cellDx: 20 } }');
    expect(geometryEditorSource).toContain('{ key: "40", label: "40 m", patch: { cellDx: 40 } }');
    expect(geometryEditorSource).toContain('{ key: "5", label: "5 m", patch: { cellDy: 5 } }');
    expect(geometryEditorSource).toContain('{ key: "10", label: "10 m", patch: { cellDy: 10 } }');
    expect(geometryEditorSource).toContain('{ key: "20", label: "20 m", patch: { cellDy: 20 } }');
    expect(geometryEditorSource).toContain('{ key: "40", label: "40 m", patch: { cellDy: 40 } }');
    expect(geometryEditorSource).toContain('{ key: "1", label: "1 m", patch: { cellDz: 1 } }');
    expect(geometryEditorSource).toContain('{ key: "5", label: "5 m", patch: { cellDz: 5 } }');
    expect(geometryEditorSource).toContain('{ key: "10", label: "10 m", patch: { cellDz: 10 } }');
    expect(geometryEditorSource).toContain('{ key: "20", label: "20 m", patch: { cellDz: 20 } }');
  });

  it('returns shared mode panel sections for current modes', () => {
    expect(getModePanelSections('dep')).toEqual(MODE_PANEL_SECTIONS);
    expect(getModePanelSections('sim')).toEqual(MODE_PANEL_SECTIONS);
  });
});