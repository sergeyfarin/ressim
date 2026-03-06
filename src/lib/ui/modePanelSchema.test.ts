import { describe, expect, it } from 'vitest';
import {
  GEOMETRY_GRID_SECTION_SCHEMA,
  MODE_PANEL_SECTIONS,
  getControlErrorMessage,
  getModePanelSections,
  getQuickPickMatch,
} from './modePanelSchema';

describe('modePanelSchema', () => {
  it('defines geometry section metadata in the mode panel schema', () => {
    const geometry = MODE_PANEL_SECTIONS.find((section) => section.key === 'geometry');

    expect(geometry).toBeDefined();
    expect(geometry?.schemaKey).toBe('geometry-grid');
    expect(geometry?.dims).toEqual(['geo', 'grid']);
  });

  it('defines nx quick picks with inline custom entry behavior', () => {
    const quickPick = GEOMETRY_GRID_SECTION_SCHEMA.controls.find(
      (control) => control.type === 'quick-picks' && control.param === 'nx',
    );

    expect(quickPick).toBeDefined();
    expect(quickPick?.options.map((option) => option.patch.nx)).toEqual([12, 24, 48, 96]);
    expect(quickPick?.custom.type).toBe('inline-number');
    expect(quickPick?.custom.integer).toBe(true);
  });

  it('matches quick-pick options only when the current value matches a preset', () => {
    const quickPick = GEOMETRY_GRID_SECTION_SCHEMA.controls.find(
      (control) => control.type === 'quick-picks' && control.param === 'nx',
    );

    if (!quickPick || quickPick.type !== 'quick-picks') {
      throw new Error('Expected nx quick-pick control to exist');
    }

    expect(getQuickPickMatch(quickPick, 24)?.key).toBe('24');
    expect(getQuickPickMatch(quickPick, 30)).toBeNull();
  });

  it('returns the first matching error message for a control', () => {
    expect(
      getControlErrorMessage(
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
