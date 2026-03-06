import { describe, expect, it } from 'vitest';
import {
  GEOMETRY_GRID_QUICK_EDITOR,
  getGeometryGridControlErrorMessage,
  getGeometryGridQuickPickMatch,
  type GeometryGridQuickPickControl,
  isGeometryGridQuickPickControl,
} from './geometryGridQuickEditor';
import { MODE_PANEL_SECTIONS, getModePanelSections } from './modePanelSections';

function getQuickPick(param: GeometryGridQuickPickControl['param']): GeometryGridQuickPickControl {
  const control = GEOMETRY_GRID_QUICK_EDITOR.controls.find(
    (entry) => isGeometryGridQuickPickControl(entry) && entry.param === param,
  );

  if (!control || !isGeometryGridQuickPickControl(control)) {
    throw new Error(`Expected ${param} quick-pick control to exist`);
  }

  return control;
}

describe('mode panel helpers', () => {
  it('defines geometry section metadata for the mode panels', () => {
    const geometry = MODE_PANEL_SECTIONS.find((section) => section.key === 'geometry');

    expect(geometry).toBeDefined();
    expect(geometry?.dims).toEqual(['geo', 'grid']);
  });

  it('defines nx quick picks with inline custom entry behavior', () => {
    const quickPick = getQuickPick('nx');

    expect(quickPick.options.map((option) => option.patch.nx)).toEqual([12, 24, 48, 96]);
    expect(quickPick.custom.type).toBe('inline-number');
    expect(quickPick.custom.integer).toBe(true);
  });

  it('uses toggle-plus-custom controls for the rest of the geometry editor', () => {
    const nyQuickPick = getQuickPick('ny');
    const nzQuickPick = getQuickPick('nz');
    const dxQuickPick = getQuickPick('cellDx');
    const dyQuickPick = getQuickPick('cellDy');
    const dzQuickPick = getQuickPick('cellDz');

    expect(nyQuickPick.options.map((option) => option.patch.ny)).toEqual([1, 11, 21, 41]);
    expect(nzQuickPick.options.map((option) => option.patch.nz)).toEqual([1, 3, 5, 10]);
    expect(nzQuickPick.custom.changeBehavior).toBe('sync-layer-arrays');
    expect(dxQuickPick.options.map((option) => option.patch.cellDx)).toEqual([5, 10, 20, 40]);
    expect(dyQuickPick.options.map((option) => option.patch.cellDy)).toEqual([5, 10, 20, 40]);
    expect(dzQuickPick.options.map((option) => option.patch.cellDz)).toEqual([1, 5, 10, 20]);
  });

  it('matches quick-pick options only when the current value matches a preset', () => {
    const quickPick = getQuickPick('nx');

    expect(getGeometryGridQuickPickMatch(quickPick, 24)?.key).toBe('24');
    expect(getGeometryGridQuickPickMatch(quickPick, 30)).toBeNull();
  });

  it('returns the first matching error message for a control', () => {
    expect(
      getGeometryGridControlErrorMessage(
        {
          ny: 'Y must be at least 1',
          nz: 'Z must be at least 1',
        },
        ['ny', 'nz'],
      ),
    ).toBe('Y must be at least 1');
  });

  it('returns shared mode panel sections for current modes', () => {
    expect(getModePanelSections('dep')).toEqual(MODE_PANEL_SECTIONS);
    expect(getModePanelSections('sim')).toEqual(MODE_PANEL_SECTIONS);
  });
});
