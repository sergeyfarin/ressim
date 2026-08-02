import { describe, expect, it } from 'vitest';
import { RECOVERY_SCALES } from './scalePresetRegistry';

describe('scalePresetRegistry', () => {
    it('lets recovery charts scale to their visible data', () => {
        expect(RECOVERY_SCALES.y.min).toBe(0);
        expect(RECOVERY_SCALES.y).not.toHaveProperty('max');
        expect(RECOVERY_SCALES.y).not.toHaveProperty('_maxCap');
    });
});
