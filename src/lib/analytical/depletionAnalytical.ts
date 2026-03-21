export type ReservoirGeometry = {
    length: number;
    area: number;
    porosity: number;
};

export type DepletionAnalyticalPoint = {
    time: number;
    oilRate: number;
    waterRate: number;
    cumulativeOil: number;
    avgPressure: number;
};

export type DepletionAnalyticalMeta = {
    mode: "depletion";
    shapeFactor: number | null;
    shapeLabel: string;
    q0?: number;
    tau?: number;
    arpsB?: number;
};

export type DepletionAnalyticalParams = {
    reservoir: ReservoirGeometry | null | undefined;
    timeHistory: number[];
    initialSaturation: number;
    nz: number;
    permMode: string;
    uniformPermX: number;
    uniformPermY: number;
    layerPermsX: number[];
    layerPermsY: number[];
    cellDx: number;
    cellDy: number;
    cellDz: number;
    wellRadius: number;
    wellSkin: number;
    muO: number;
    sWc: number;
    sOr: number;
    nO: number;
    c_o: number;
    c_w: number;
    cRock: number;
    initialPressure: number;
    producerBhp: number;
    depletionRateScale: number;
    arpsB?: number;
    nx?: number;
    ny?: number;
    producerI?: number;
    producerJ?: number;
};

export type DepletionAnalyticalResult = {
    meta: DepletionAnalyticalMeta;
    production: DepletionAnalyticalPoint[];
};

export const DARCY_METRIC_FACTOR = 8.5269888e-3;

export function emptyDepletionAnalyticalResult(): DepletionAnalyticalResult {
    return {
        meta: {
            mode: "depletion",
            shapeFactor: null,
            shapeLabel: "",
            q0: undefined,
            tau: undefined,
        },
        production: [],
    };
}

/**
 * Dietz shape factor C_A for known drainage geometries and well positions.
 *
 * For square drainage areas the well position determines C_A via log-linear
 * interpolation between the two tabulated endpoints:
 *   - Center well: C_A = 30.8828  (Dietz 1965)
 *   - Corner well: C_A = 0.5598   (quarter-drainage symmetry)
 *
 * The interpolation variable is the Chebyshev (L∞) normalised distance of
 * the well from the grid centre — 0 at centre, 1 at corner.
 */
const CA_SQUARE_CENTER = 30.8828;
const CA_SQUARE_CORNER = 0.5598;

export function computeShapeFactor(input: {
    nxCells: number;
    nyCells: number;
    aspectRatio: number;
    nx?: number;
    ny?: number;
    producerI?: number;
    producerJ?: number;
}): { shapeFactor: number; shapeLabel: string } {
    const { nxCells, nyCells, aspectRatio, nx, ny, producerI, producerJ } = input;

    if (nyCells <= 1) {
        return { shapeFactor: 0, shapeLabel: "1D Slab (end well)" };
    }

    // Square drainage area — use position-aware shape factor
    if (aspectRatio > 0.5 && aspectRatio < 2.0) {
        const gridNx = nx ?? nxCells;
        const gridNy = ny ?? nyCells;
        const hasPosition =
            producerI !== undefined && producerI !== null &&
            producerJ !== undefined && producerJ !== null;

        if (!hasPosition || (gridNx <= 1 && gridNy <= 1)) {
            return { shapeFactor: CA_SQUARE_CENTER, shapeLabel: "Square (center)" };
        }

        // Normalised Chebyshev distance from grid centre: 0 = center, 1 = corner
        const cx = (gridNx - 1) / 2;
        const cy = (gridNy - 1) / 2;
        const dx = cx > 0 ? Math.abs((producerI as number) - cx) / cx : 0;
        const dy = cy > 0 ? Math.abs((producerJ as number) - cy) / cy : 0;
        const d = Math.min(1, Math.max(0, Math.max(dx, dy)));

        // Log-linear interpolation between tabulated endpoints
        const logCA =
            Math.log(CA_SQUARE_CENTER) * (1 - d) +
            Math.log(CA_SQUARE_CORNER) * d;
        const shapeFactor = Math.exp(logCA);

        if (d < 0.15) {
            return { shapeFactor, shapeLabel: "Square (center)" };
        } else if (d > 0.85) {
            return { shapeFactor, shapeLabel: `Square (corner, C_A ≈ ${shapeFactor.toFixed(2)})` };
        }
        return { shapeFactor, shapeLabel: `Square (off-center, C_A ≈ ${shapeFactor.toFixed(2)})` };
    }

    // Non-square rectangles — aspect-ratio based (no position adjustment yet)
    if (aspectRatio >= 2.0 && aspectRatio < 5.0) {
        return { shapeFactor: 10.84, shapeLabel: `Rectangle ${aspectRatio.toFixed(1)}:1` };
    }
    if (aspectRatio >= 5.0) {
        return { shapeFactor: 2.36, shapeLabel: `Elongated ${aspectRatio.toFixed(0)}:1` };
    }
    return { shapeFactor: 10.84, shapeLabel: `Rectangle 1:${(1 / aspectRatio).toFixed(1)}` };
}

export function calculateDepletionAnalyticalProduction(
    params: DepletionAnalyticalParams,
): DepletionAnalyticalResult {
    const {
        reservoir,
        timeHistory,
        initialSaturation,
        nz,
        permMode,
        uniformPermX,
        uniformPermY,
        layerPermsX,
        layerPermsY,
        cellDx,
        cellDy,
        cellDz,
        wellRadius,
        wellSkin,
        muO,
        sWc,
        sOr,
        nO,
        c_o,
        c_w,
        cRock,
        initialPressure,
        producerBhp,
        depletionRateScale,
    } = params;

    if (!reservoir || timeHistory.length === 0) {
        return emptyDepletionAnalyticalResult();
    }

    const poreVolume = Math.max(1e-9, reservoir.length * reservoir.area * reservoir.porosity);
    const wellRadiusSafe = Math.max(1e-6, wellRadius);

    const sw = Math.min(1, Math.max(0, initialSaturation));
    const mobileRange = Math.max(1e-9, 1 - sWc - sOr);
    const effectiveSaturation = Math.min(1, Math.max(0, (sw - sWc) / mobileRange));
    const kroAtInitialSw = Math.max(0, (1 - effectiveSaturation) ** nO);

    const wellboreHeight = nz * cellDz;
    const lengthX = reservoir.length;
    const lengthY = Math.max(cellDy, reservoir.area / Math.max(1e-6, wellboreHeight));
    const drainageArea = lengthX * lengthY;

    const nxCells = Math.max(1, Math.round(lengthX / Math.max(1e-6, cellDx)));
    const nyCells = Math.max(1, Math.round(lengthY / Math.max(1e-6, cellDy)));

    const { shapeFactor, shapeLabel } = computeShapeFactor({
        nxCells,
        nyCells,
        aspectRatio: lengthX / Math.max(1e-6, lengthY),
        nx: params.nx,
        ny: params.ny,
        producerI: params.producerI,
        producerJ: params.producerJ,
    });

    let totalOilPi = 0;

    for (let layerIndex = 0; layerIndex < nz; layerIndex++) {
        let permX = uniformPermX;
        let permY = uniformPermY;

        if (permMode === "perLayer") {
            permX = layerPermsX[layerIndex] ?? uniformPermX;
            permY = layerPermsY[layerIndex] ?? uniformPermY;
        }

        permX = Math.max(1e-6, permX);
        permY = Math.max(1e-6, permY);
        const averagePerm = Math.sqrt(permX * permY);

        let oilPiForLayer = 0;

        if (shapeFactor === 0) {
            const crossSectionArea = lengthY * cellDz;
            const anisotropyRatio = permX / permY;
            const equivalentRadius =
                (0.28 *
                    Math.sqrt(
                        Math.sqrt(anisotropyRatio) * cellDx * cellDx +
                            Math.sqrt(1 / anisotropyRatio) * cellDy * cellDy,
                    )) /
                (anisotropyRatio ** 0.25 + (1 / anisotropyRatio) ** 0.25);
            const piDenominator = Math.max(
                1e-9,
                Math.log(Math.max(1 + 1e-9, equivalentRadius / wellRadiusSafe)) + wellSkin,
            );
            const peacemanPi =
                (DARCY_METRIC_FACTOR *
                    2 *
                    Math.PI *
                    averagePerm *
                    cellDz *
                    (kroAtInitialSw / muO)) /
                piDenominator;

            const linearResistance =
                lengthX /
                (3 * averagePerm * crossSectionArea * DARCY_METRIC_FACTOR * (kroAtInitialSw / muO));
            const wellResistance = Math.max(1e-12, 1 / peacemanPi);
            oilPiForLayer = 1 / (linearResistance + wellResistance);
        } else {
            const eulerGamma = 0.5772156649;
            const shapeDenominator =
                0.5 *
                Math.log((4 * drainageArea) / (shapeFactor * Math.exp(2 * eulerGamma) * wellRadiusSafe * wellRadiusSafe));
            oilPiForLayer =
                (DARCY_METRIC_FACTOR *
                    2 *
                    Math.PI *
                    averagePerm *
                    cellDz *
                    (kroAtInitialSw / muO)) /
                Math.max(1e-9, shapeDenominator + wellSkin);
        }

        totalOilPi += Math.max(0, oilPiForLayer);
    }

    const oilPi = Math.max(1e-12, totalOilPi);
    const oilSaturation = 1 - sw;
    const totalCompressibility = Math.max(1e-12, oilSaturation * c_o + sw * c_w + cRock);
    const tau = Math.max(1e-6, (poreVolume * totalCompressibility) / oilPi);
    const pressureDrop = Math.max(0, initialPressure - producerBhp);
    const q0 = oilPi * pressureDrop * Math.max(0, depletionRateScale);
    const Di = 1 / tau; // Initial decline rate [1/day]

    // Arps decline exponent: b=0 exponential, 0<b<1 hyperbolic, b=1 harmonic.
    // Fetkovich (1971) shows b=0 for single-phase slightly-compressible bounded
    // reservoirs at constant BHP.  Values of b>0 arise from layered/commingled
    // production, multiphase flow, or heterogeneous reservoirs — Arps (1945).
    const b = Math.max(0, Math.min(1, params.arpsB ?? 0));

    const production = timeHistory.map((timeValue) => {
        const time = Math.max(0, Number(timeValue) || 0);
        let oilRate: number;
        let cumulativeOil: number;

        if (b < 1e-8) {
            // ── Exponential decline (b ≈ 0) ──────────────────────────────
            // q(t) = q_i · exp(−D_i·t)
            // N_p(t) = q_i/D_i · [1 − exp(−D_i·t)]
            const exponent = Math.min(700, Di * time);
            const expTerm = Math.exp(-exponent);
            oilRate = q0 * expTerm;
            cumulativeOil = (q0 / Di) * (1 - expTerm);
        } else if (b > 1 - 1e-8) {
            // ── Harmonic decline (b ≈ 1) ─────────────────────────────────
            // q(t) = q_i / (1 + D_i·t)
            // N_p(t) = q_i/D_i · ln(1 + D_i·t)
            const denominator = 1 + Di * time;
            oilRate = q0 / denominator;
            cumulativeOil = (q0 / Di) * Math.log(denominator);
        } else {
            // ── Hyperbolic decline (0 < b < 1) ──────────────────────────
            // q(t) = q_i / (1 + b·D_i·t)^(1/b)
            // N_p(t) = q_i/((1−b)·D_i) · [1 − (1 + b·D_i·t)^((b−1)/b)]
            const base = 1 + b * Di * time;
            oilRate = q0 * Math.pow(base, -1 / b);
            cumulativeOil = (q0 / ((1 - b) * Di)) * (1 - Math.pow(base, (b - 1) / b));
        }

        // Pressure tracks rate through the PI relationship:
        // P_avg = P_bhp + q(t)/PI = P_bhp + ΔP · q(t)/q_i
        const avgPressure = producerBhp + pressureDrop * (q0 > 0 ? oilRate / q0 : 0);

        return {
            time,
            oilRate,
            waterRate: 0,
            cumulativeOil,
            avgPressure,
        };
    });

    return {
        meta: {
            mode: "depletion",
            shapeFactor,
            shapeLabel,
            q0,
            tau,
            arpsB: b,
        },
        production,
    };
}