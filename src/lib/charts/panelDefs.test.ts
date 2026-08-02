import { describe, expect, it } from 'vitest';
import { PANEL_DEFS, getPanelFallback, GENERIC_PANEL_FALLBACK } from './panelDefs';
import {
    KNOWN_PRIMARY_PANEL_IDS,
    KNOWN_SWEEP_PANEL_IDS,
    DEFAULT_RATE_CHART_PANEL_ORDER,
} from './rateChartLayoutConfig';

describe('panel identity is an open set', () => {
    it('gives every known panel a declared default', () => {
        for (const id of [...KNOWN_PRIMARY_PANEL_IDS, ...KNOWN_SWEEP_PANEL_IDS]) {
            expect(PANEL_DEFS[id], id).toBeDefined();
            expect(PANEL_DEFS[id].title.length, id).toBeGreaterThan(0);
        }
    });

    it('accepts a panel id nobody declared, instead of returning undefined', () => {
        // The point of step 3: a panel can be introduced beside the code that
        // fills it, without first editing a union and PANEL_DEFS. Before this,
        // an unknown id reached the renderer as `undefined` and threw.
        const fallback = getPanelFallback('gas_rate');
        expect(fallback.title).toBe('Gas rate');
        expect(fallback.scalePreset).toBe(GENERIC_PANEL_FALLBACK.scalePreset);
    });

    it('prefers a declared default over the generic one', () => {
        expect(getPanelFallback('recovery')).toBe(PANEL_DEFS.recovery);
    });

    it('keeps the default render order in step with the known ids', () => {
        // The order is the list; a known panel missing from it would never render
        // under the default layout.
        const ordered = new Set(DEFAULT_RATE_CHART_PANEL_ORDER);
        for (const id of [...KNOWN_PRIMARY_PANEL_IDS, ...KNOWN_SWEEP_PANEL_IDS]) {
            expect(ordered.has(id), `${id} missing from DEFAULT_RATE_CHART_PANEL_ORDER`).toBe(true);
        }
    });
});
