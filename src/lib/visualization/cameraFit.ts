export type Vector3Tuple = readonly [number, number, number];

export type PerspectiveBoxFitInput = {
    halfExtents: Vector3Tuple;
    cameraDirection: Vector3Tuple;
    verticalFovRadians: number;
    aspect: number;
    padding?: number;
};

function dot(a: Vector3Tuple, b: Vector3Tuple): number {
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

function normalize(vector: Vector3Tuple): Vector3Tuple {
    const length = Math.hypot(...vector);
    if (!(length > 0)) return [1, 0, 0];
    return [vector[0] / length, vector[1] / length, vector[2] / length];
}

function cross(a: Vector3Tuple, b: Vector3Tuple): Vector3Tuple {
    return [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
}

/**
 * Returns the camera distance needed to contain an axis-aligned box.
 *
 * Unlike a bounding-sphere fit, this evaluates the eight projected corners,
 * so a long thin model is not pushed away by empty space around the box.
 */
export function fitPerspectiveCameraToBox({
    halfExtents,
    cameraDirection,
    verticalFovRadians,
    aspect,
    padding = 1.15,
}: PerspectiveBoxFitInput): number {
    const outward = normalize(cameraDirection);
    const forward: Vector3Tuple = [-outward[0], -outward[1], -outward[2]];
    const worldUp: Vector3Tuple = [0, 0, 1];
    let right = normalize(cross(forward, worldUp));
    if (Math.abs(dot(forward, worldUp)) > 0.999) {
        right = normalize(cross(forward, [0, 1, 0]));
    }
    const cameraUp = normalize(cross(right, forward));
    const tanHalfVertical = Math.max(Math.tan(verticalFovRadians * 0.5), 1e-6);
    const tanHalfHorizontal = Math.max(tanHalfVertical * Math.max(aspect, 1e-6), 1e-6);
    const safePadding = Math.max(1, padding);

    let distance = 0;
    for (const sx of [-1, 1]) {
        for (const sy of [-1, 1]) {
            for (const sz of [-1, 1]) {
                const corner: Vector3Tuple = [
                    sx * halfExtents[0],
                    sy * halfExtents[1],
                    sz * halfExtents[2],
                ];
                const towardCamera = dot(outward, corner);
                const horizontalDistance = Math.abs(dot(right, corner)) / tanHalfHorizontal;
                const verticalDistance = Math.abs(dot(cameraUp, corner)) / tanHalfVertical;
                distance = Math.max(
                    distance,
                    towardCamera + horizontalDistance * safePadding,
                    towardCamera + verticalDistance * safePadding,
                );
            }
        }
    }

    return Math.max(distance, 0.001);
}
