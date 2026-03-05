import { describe, expect, it } from 'vitest';
import {
    buildBasePresetProfile,
    buildParameterOverrides,
    evaluateAnalyticalStatus,
    groupParameterOverrides,
} from './phase2PresetContract';

describe('phase2PresetContract', () => {
    it('marks modified presets as custom source', () => {
        const profile = buildBasePresetProfile({
            key: 'mode-dep_geo-1d',
            mode: 'dep',
            toggles: { mode: 'dep', geo: '1d', well: 'e2e' },
            isModified: true,
        });

        expect(profile.source).toBe('custom');
        expect(profile.mode).toBe('dep');
    });

    it('detects scalar and array overrides', () => {
        const overrides = buildParameterOverrides({
            currentParams: {
                nx: 20,
                gravityEnabled: true,
                layerPermsX: [50, 100],
            },
            baseParams: {
                nx: 10,
                gravityEnabled: false,
                layerPermsX: [50, 200],
            },
            trackedKeys: ['nx', 'gravityEnabled', 'layerPermsX'],
        });

        expect(Object.keys(overrides)).toEqual(['nx', 'gravityEnabled', 'layerPermsX']);
        expect(overrides.nx.base).toBe(10);
        expect(overrides.nx.current).toBe(20);
    });

    it('groups overrides by configured group keys', () => {
        const grouped = groupParameterOverrides({
            nx: { base: 10, current: 20 },
            gravityEnabled: { base: false, current: true },
            injectorBhp: { base: 400, current: 450 },
        });

        expect(grouped.grid).toEqual(['nx']);
        expect(grouped.physics).toEqual(['gravityEnabled']);
        expect(grouped.wells).toEqual(['injectorBhp']);
    });

    it('returns reference analytical status for ideal waterflood assumptions', () => {
        const status = evaluateAnalyticalStatus({
            activeMode: 'wf',
            analyticalMode: 'waterflood',
            injectorEnabled: true,
            gravityEnabled: false,
            capillaryEnabled: false,
            permMode: 'uniform',
            toggles: { mode: 'wf', geo: '1d', well: 'e2e' },
        });

        expect(status.level).toBe('reference');
        expect(status.reasons).toEqual([]);
    });

    it('returns approximate analytical status with reasons for non-ideal setup', () => {
        const status = evaluateAnalyticalStatus({
            activeMode: 'sim',
            analyticalMode: 'waterflood',
            injectorEnabled: true,
            gravityEnabled: true,
            capillaryEnabled: true,
            permMode: 'random',
            toggles: { mode: 'sim', geo: '2dxy', well: 'corner' },
        });

        expect(status.level).toBe('approximate');
        expect(status.reasons.length).toBeGreaterThan(0);
    });
});
