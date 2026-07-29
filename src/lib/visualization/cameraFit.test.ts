import { describe, expect, it } from 'vitest';
import { fitPerspectiveCameraToBox } from './cameraFit';

const CAMERA_DIRECTION = [1.2, -1.8, 0.8] as const;
const VERTICAL_FOV = 7 * Math.PI / 180;

function fit(halfExtents: readonly [number, number, number], aspect = 16 / 9): number {
    return fitPerspectiveCameraToBox({
        halfExtents,
        cameraDirection: CAMERA_DIRECTION,
        verticalFovRadians: VERTICAL_FOV,
        aspect,
    });
}

describe('fitPerspectiveCameraToBox', () => {
    it('keeps a vertical XZ model farther away than a thin 1D model', () => {
        const oneDimensional = fit([500, 5, 5]);
        const verticalXz = fit([500, 5, 250]);

        expect(verticalXz).toBeGreaterThan(oneDimensional);
    });

    it('fits landscape canvases by height and portrait canvases by width', () => {
        const box = [500, 200, 100] as const;

        expect(fit(box, 16 / 9)).toBeLessThan(fit(box, 9 / 16));
    });

    it('applies extra space without scaling away the box depth', () => {
        const snug = fitPerspectiveCameraToBox({
            halfExtents: [100, 20, 10],
            cameraDirection: CAMERA_DIRECTION,
            verticalFovRadians: VERTICAL_FOV,
            aspect: 1.5,
            padding: 1,
        });
        const padded = fitPerspectiveCameraToBox({
            halfExtents: [100, 20, 10],
            cameraDirection: CAMERA_DIRECTION,
            verticalFovRadians: VERTICAL_FOV,
            aspect: 1.5,
            padding: 1.15,
        });

        expect(padded).toBeGreaterThan(snug);
        expect(padded).toBeLessThan(snug * 1.15);
    });
});
