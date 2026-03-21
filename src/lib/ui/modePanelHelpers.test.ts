import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';
import { MODE_PANEL_SECTIONS, getModePanelSections } from './modePanelSections';

const geometryEditorPath = path.join(__dirname, 'sections', 'GeometrySection.svelte');
const geometryEditorSource = fs.readFileSync(geometryEditorPath, 'utf8');

describe('mode panel helpers', () => {
  it('defines geometry section metadata for the mode panels', () => {
    const geometry = MODE_PANEL_SECTIONS.find((section) => section.key === 'geometry');

    expect(geometry).toBeDefined();
    expect(geometry?.dims).toEqual(['geo', 'grid']);
  });

  it('keeps geometry editing helpers local to the geometry component', () => {
    expect(geometryEditorSource).toMatch(/const totalCells = \$derived\(bindings\.nx \* bindings\.ny \* bindings\.nz\)/);
    expect(geometryEditorSource).toMatch(/function fmtLen\(v: number\)/);
    expect(geometryEditorSource).toMatch(/function setInt\(param: "nx" \| "ny" \| "nz", raw: string\)/);
    expect(geometryEditorSource).toMatch(/function setFloat\(param: "cellDx" \| "cellDy" \| "cellDz", raw: string\)/);
  });

  it('renders direct X-axis grid editing inside the compact table layout', () => {
    expect(geometryEditorSource).toContain('<Collapsible title="Grid"');
    expect(geometryEditorSource).toContain('<th class="px-1 py-0.5 font-medium">Axis</th>');
    expect(geometryEditorSource).toContain('<th class="px-1 py-0.5 font-medium">Cells</th>');
    expect(geometryEditorSource).toContain('<th class="px-1 py-0.5 font-medium">Size (m)</th>');
    expect(geometryEditorSource).toContain("value={bindings.nx} oninput={(e) => setInt('nx', (e.currentTarget as HTMLInputElement).value)}");
    expect(geometryEditorSource).toContain("value={bindings.cellDx} oninput={(e) => setFloat('cellDx', (e.currentTarget as HTMLInputElement).value)}");
  });

  it('keeps Y and Z editing wired directly to numeric inputs and derived totals', () => {
    expect(geometryEditorSource).toContain("value={bindings.ny} oninput={(e) => setInt('ny', (e.currentTarget as HTMLInputElement).value)}");
    expect(geometryEditorSource).toContain("value={bindings.nz} oninput={(e) => setInt('nz', (e.currentTarget as HTMLInputElement).value)}");
    expect(geometryEditorSource).toContain("value={bindings.cellDy} oninput={(e) => setFloat('cellDy', (e.currentTarget as HTMLInputElement).value)}");
    expect(geometryEditorSource).toContain("value={bindings.cellDz} oninput={(e) => setFloat('cellDz', (e.currentTarget as HTMLInputElement).value)}");
    expect(geometryEditorSource).toContain('bindings.handleNzOrPermModeChange()');
    expect(geometryEditorSource).toContain('{fmtLen(totalY)}');
    expect(geometryEditorSource).toContain('{fmtLen(totalZ)}');
  });

  it('returns shared mode panel sections for current modes', () => {
    expect(getModePanelSections()).toEqual(MODE_PANEL_SECTIONS);
    expect(getModePanelSections()).toEqual(MODE_PANEL_SECTIONS);
  });
});