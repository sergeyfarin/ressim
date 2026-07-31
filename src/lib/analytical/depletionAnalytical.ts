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
    layerTimeConstants?: number[];
};

export type DepletionAnalyticalParams = {
    reservoir: ReservoirGeometry | null | undefined;
    timeHistory: number[];
    minTimeDays?: number;
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
    /** Superpose independent, boundary-dominated layer responses. */
    layeredComposite?: boolean;
    /** Physical reference contract. Defaults to the lumped PSS tank. */
    model?: "tank" | "finite-slab";
    nx?: number;
    ny?: number;
    producerI?: number;
    producerJ?: number;
};

type FiniteSlabMode = { lambda: number; coefficient: number };

/**
 * Eigenmodes for a finite linear reservoir with a no-flow outer boundary and
 * a finite-productivity (Robin) producing boundary.  In dimensionless form
 * the eigenvalues satisfy lambda.tan(lambda) = beta, where beta is the ratio
 * of reservoir conductance to well resistance.  This retains the early
 * distributed transient that a one-time-constant tank necessarily discards.
 */
export function finiteSlabModes(beta: number, count = 80): FiniteSlabMode[] {
    const betaSafe = Math.max(1e-12, beta);
    const modes: FiniteSlabMode[] = [];
    for (let n = 0; n < count; n++) {
        let lo = n * Math.PI + 1e-10;
        let hi = n * Math.PI + Math.PI / 2 - 1e-10;
        for (let iteration = 0; iteration < 80; iteration++) {
            const mid = (lo + hi) / 2;
            if (mid * Math.tan(mid) > betaSafe) hi = mid;
            else lo = mid;
        }
        const lambda = (lo + hi) / 2;
        const norm = 0.5 + Math.sin(2 * lambda) / (4 * lambda);
        const coefficient = (Math.sin(lambda) / lambda) / norm;
        modes.push({ lambda, coefficient });
    }
    return modes;
}

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
 * Dietz shape factor C_A — tabulated entries only.
 *
 * A shape factor is a global boundary-value result for one (geometry, well
 * position) pair. It cannot be interpolated from well coordinates, so this
 * module recognises the tabulated cases and returns null for everything else
 * rather than inventing a value.
 *
 * Entries below are Dietz (1965) as reproduced in Earlougher (1977) Table C.1
 * and Dake (1978) Table 7.1, restricted to the closed rectangular geometries
 * this simulator can represent on a Cartesian grid:
 *
 *   1:1, well centred                    30.8828
 *   1:1, well at the centre of a quadrant  4.5132
 *   2:1, well centred                    21.8369
 *   4:1, well centred                     5.379
 *
 * Removed 2026-07-31: a `CA_SQUARE_CORNER = 0.5598` entry for a well in the
 * corner cell. It is not reproducible and was never exercised by a scenario.
 * Two independent reasons:
 *   - Reflecting a corner well in its two adjacent no-flow walls maps it onto
 *     itself, so the physical problem is a well with only a quarter of its
 *     sandface open. The resulting drawdown is ~4x a centred well's, which
 *     corresponds to a C_A many orders of magnitude below 0.5598.
 *   - The engine's Peaceman well index assumes an interior block with full
 *     radial inflow, so it cannot produce corner-well behaviour anyway. A
 *     measured run gives an effective C_A of ~1.6e-3 and a PI 63% below what
 *     0.5598 predicts.
 * Every tabulated position here is therefore an interior one.
 */
const CA_SQUARE_CENTER = 30.8828;
const CA_SQUARE_QUADRANT = 4.5132;
const CA_2TO1_CENTER = 21.8369;
const CA_4TO1_CENTER = 5.379;

/** Aspect ratios are matched within this relative tolerance. */
const ASPECT_TOLERANCE = 0.05;

/**
 * Well positions are matched within this tolerance, as a fraction of the
 * corresponding side length. Half a cell on the coarsest grid the catalog
 * ships (21 cells) is 0.024, so this accepts an exactly-placed well without
 * accepting a visibly off-position one.
 */
const POSITION_TOLERANCE = 0.03;

export function dietzProductivityIndex(input: {
    permeabilityMd: number;
    thicknessM: number;
    mobilityPerCp: number;
    drainageAreaM2: number;
    shapeFactor: number;
    wellRadiusM: number;
    skin: number;
}): number {
    // Dietz's gamma is exp(Euler-Mascheroni) = 1.781, not exp(2*gamma).
    // See Dake's semi-steady inflow equation: 4A / (gamma C_A r_w^2).
    const eulerGamma = 0.5772156649;
    const denominator = 0.5 * Math.log(
        (4 * Math.max(1e-12, input.drainageAreaM2)) /
        (Math.max(1e-12, input.shapeFactor) * Math.exp(eulerGamma) *
            Math.max(1e-12, input.wellRadiusM * input.wellRadiusM)),
    ) + input.skin;
    return DARCY_METRIC_FACTOR * 2 * Math.PI * Math.max(0, input.permeabilityMd) *
        Math.max(0, input.thicknessM) * Math.max(0, input.mobilityPerCp) /
        Math.max(1e-9, denominator);
}

/**
 * Invert the Dietz PSS productivity equation to recover the effective shape
 * factor represented by a measured productivity index. This is the useful
 * numerical comparison quantity: a finite-volume run should approach the
 * tabulated C_A as its late-time pressure field reaches PSS and the grid is
 * refined.
 */
export function dietzShapeFactorFromProductivityIndex(input: {
    productivityIndex: number;
    permeabilityMd: number;
    thicknessM: number;
    mobilityPerCp: number;
    drainageAreaM2: number;
    wellRadiusM: number;
    skin: number;
}): number | null {
    const {
        productivityIndex, permeabilityMd, thicknessM, mobilityPerCp,
        drainageAreaM2, wellRadiusM, skin,
    } = input;
    if (
        !(productivityIndex > 0) || !(permeabilityMd > 0) || !(thicknessM > 0) ||
        !(mobilityPerCp > 0) || !(drainageAreaM2 > 0) || !(wellRadiusM > 0)
    ) return null;

    const numerator = DARCY_METRIC_FACTOR * 2 * Math.PI * permeabilityMd *
        thicknessM * mobilityPerCp;
    const dietzDenominator = numerator / productivityIndex;
    const exponent = 2 * (dietzDenominator - skin);
    if (!Number.isFinite(exponent) || exponent > 700 || exponent < -700) return null;

    const eulerGamma = 0.5772156649;
    const shapeFactor = (4 * drainageAreaM2) /
        (Math.exp(eulerGamma) * wellRadiusM * wellRadiusM * Math.exp(exponent));
    return Number.isFinite(shapeFactor) && shapeFactor > 0 ? shapeFactor : null;
}

function near(value: number, target: number, tolerance: number): boolean {
    return Math.abs(value - target) <= tolerance;
}

/**
 * Dietz shape factor for a closed rectangular drainage area.
 *
 * `aspectRatio` is lengthX/lengthY; the well position is given as grid indices
 * and converted to cell-centre fractions of each side. The long side is
 * normalised to x so a 2:1 and a 1:2 rectangle resolve to the same entry.
 *
 * Returns `shapeFactor: null` with an explanatory label for any geometry or
 * position outside the table — callers must handle that rather than assume a
 * square.
 */
export function computeShapeFactor(input: {
    nxCells: number;
    nyCells: number;
    aspectRatio: number;
    nx?: number;
    ny?: number;
    producerI?: number;
    producerJ?: number;
}): { shapeFactor: number | null; shapeLabel: string } {
    const { nxCells, nyCells, aspectRatio, nx, ny, producerI, producerJ } = input;

    if (nyCells <= 1) {
        return { shapeFactor: 0, shapeLabel: "1D Slab (end well)" };
    }
    if (!(aspectRatio > 0)) {
        return { shapeFactor: null, shapeLabel: "Unsupported drainage geometry" };
    }

    const gridNx = nx ?? nxCells;
    const gridNy = ny ?? nyCells;
    const hasPosition =
        producerI !== undefined && producerI !== null &&
        producerJ !== undefined && producerJ !== null;

    // Cell-centre position as a fraction of each side. Without an explicit
    // well position the caller is describing a centred well.
    const rawFx = hasPosition && gridNx > 0 ? ((producerI as number) + 0.5) / gridNx : 0.5;
    const rawFy = hasPosition && gridNy > 0 ? ((producerJ as number) + 0.5) / gridNy : 0.5;

    // Normalise so x is the long side.
    const elongated = aspectRatio >= 1;
    const ratio = elongated ? aspectRatio : 1 / aspectRatio;
    const fLong = elongated ? rawFx : rawFy;
    const fShort = elongated ? rawFy : rawFx;

    const centred =
        near(fLong, 0.5, POSITION_TOLERANCE) && near(fShort, 0.5, POSITION_TOLERANCE);

    if (near(ratio, 1, ASPECT_TOLERANCE)) {
        if (centred) {
            return { shapeFactor: CA_SQUARE_CENTER, shapeLabel: "Square (centred well)" };
        }
        // Centre of any quadrant — the four positions are symmetry-equivalent.
        const quarterLong = near(fLong, 0.25, POSITION_TOLERANCE) || near(fLong, 0.75, POSITION_TOLERANCE);
        const quarterShort = near(fShort, 0.25, POSITION_TOLERANCE) || near(fShort, 0.75, POSITION_TOLERANCE);
        if (quarterLong && quarterShort) {
            return { shapeFactor: CA_SQUARE_QUADRANT, shapeLabel: "Square (well at quadrant centre)" };
        }
        return { shapeFactor: null, shapeLabel: "Unsupported off-centre square" };
    }

    if (centred && near(ratio, 2, ASPECT_TOLERANCE * 2)) {
        return { shapeFactor: CA_2TO1_CENTER, shapeLabel: "2:1 rectangle (centred well)" };
    }
    if (centred && near(ratio, 4, ASPECT_TOLERANCE * 4)) {
        return { shapeFactor: CA_4TO1_CENTER, shapeLabel: "4:1 rectangle (centred well)" };
    }

    return {
        shapeFactor: null,
        shapeLabel: centred ? "Unsupported aspect ratio" : "Unsupported off-centre rectangle",
    };
}

export function calculateDepletionAnalyticalProduction(
    params: DepletionAnalyticalParams,
): DepletionAnalyticalResult {
    const {
        reservoir,
        timeHistory,
        minTimeDays,
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
    if (shapeFactor === null) {
        return {
            meta: { ...emptyDepletionAnalyticalResult().meta, shapeLabel },
            production: [],
        };
    }

    const layerOilPis: number[] = [];
    const layerWellPis: number[] = [];

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
            layerWellPis.push(peacemanPi);

            const wellResistance = Math.max(1e-12, 1 / peacemanPi);
            if (nxCells <= 1) {
                // A one-cell layer is a lumped tank: there is no additional
                // within-layer pressure-gradient resistance beyond its well
                // connection. This is the clean contract used by the layered
                // lumped-depletion contract.
                oilPiForLayer = peacemanPi;
            } else {
                const linearResistance =
                    lengthX /
                    (3 * averagePerm * crossSectionArea * DARCY_METRIC_FACTOR * (kroAtInitialSw / muO));
                oilPiForLayer = 1 / (linearResistance + wellResistance);
            }
        } else {
            oilPiForLayer = dietzProductivityIndex({
                permeabilityMd: averagePerm,
                thicknessM: cellDz,
                mobilityPerCp: kroAtInitialSw / muO,
                drainageAreaM2: drainageArea,
                shapeFactor,
                wellRadiusM: wellRadiusSafe,
                skin: wellSkin,
            });
            layerWellPis.push(oilPiForLayer);
        }

        layerOilPis.push(Math.max(0, oilPiForLayer));
    }

    const totalOilPi = layerOilPis.reduce((sum, pi) => sum + pi, 0);
    const oilPi = Math.max(1e-12, totalOilPi);
    const oilSaturation = 1 - sw;
    const totalCompressibility = Math.max(1e-12, oilSaturation * c_o + sw * c_w + cRock);
    const tau = Math.max(1e-6, (poreVolume * totalCompressibility) / oilPi);
    const pressureDrop = Math.max(0, initialPressure - producerBhp);
    const q0 = oilPi * pressureDrop * Math.max(0, depletionRateScale);
    const Di = 1 / tau; // Initial decline rate [1/day]
    const layeredComposite = params.layeredComposite === true && layerOilPis.length > 1;
    const layerPoreVolume = poreVolume / Math.max(1, layerOilPis.length);
    const layerStorage = Math.max(1e-12, layerPoreVolume * totalCompressibility);
    const layerTimeConstants = layeredComposite
        ? layerOilPis.map((pi) => Math.max(1e-6, layerStorage / Math.max(1e-12, pi)))
        : undefined;
    const finiteSlab = params.model === "finite-slab" && nz === 1 && shapeFactor === 0;
    const finiteSlabPerm = Math.max(1e-6, uniformPermX);
    const finiteSlabConductivity =
        DARCY_METRIC_FACTOR * finiteSlabPerm * lengthY * cellDz * (kroAtInitialSw / muO);
    const finiteSlabWellPi = Math.max(1e-12, layerWellPis[0] ?? oilPi);
    const finiteSlabBeta = finiteSlabWellPi * lengthX / Math.max(1e-12, finiteSlabConductivity);
    const finiteSlabDiffusivity =
        DARCY_METRIC_FACTOR * finiteSlabPerm * (kroAtInitialSw / muO) /
        Math.max(1e-12, reservoir.porosity * totalCompressibility);
    const slabModes = finiteSlab ? finiteSlabModes(finiteSlabBeta) : [];

    // Arps decline exponent: b=0 exponential, 0<b<1 hyperbolic, b=1 harmonic.
    // Fetkovich (1980) shows b=0 for single-phase slightly-compressible bounded
    // reservoirs at constant BHP.  Values of b>0 arise from layered/commingled
    // production, multiphase flow, or heterogeneous reservoirs — Arps (1945).
    const b = Math.max(0, Math.min(1, params.arpsB ?? 0));

    const minAnalyticalTime = Math.max(0, Number(minTimeDays) || 0);
    const production = timeHistory.flatMap((timeValue) => {
        const time = Math.max(0, Number(timeValue) || 0);
        if (time < minAnalyticalTime) {
            return [];
        }
        let oilRate: number;
        let cumulativeOil: number;

        if (finiteSlab) {
            let boundaryPressureFraction = 0;
            let averagePressureFraction = 0;
            for (const mode of slabModes) {
                const decay = Math.exp(-Math.min(
                    700,
                    finiteSlabDiffusivity * mode.lambda * mode.lambda * time /
                    (lengthX * lengthX),
                ));
                boundaryPressureFraction += mode.coefficient * Math.cos(mode.lambda) * decay;
                averagePressureFraction +=
                    mode.coefficient * (Math.sin(mode.lambda) / mode.lambda) * decay;
            }
            oilRate = finiteSlabWellPi * pressureDrop * boundaryPressureFraction;
            const storage = poreVolume * totalCompressibility;
            cumulativeOil = storage * pressureDrop * (1 - averagePressureFraction);
            return [{
                time,
                oilRate: Math.max(0, oilRate),
                waterRate: 0,
                cumulativeOil: Math.max(0, cumulativeOil),
                avgPressure: producerBhp + pressureDrop * averagePressureFraction,
            }];
        } else if (layeredComposite && layerTimeConstants) {
            oilRate = 0;
            cumulativeOil = 0;
            let pressureFraction = 0;

            for (let layerIndex = 0; layerIndex < layerOilPis.length; layerIndex++) {
                const layerPi = layerOilPis[layerIndex];
                const layerTau = layerTimeConstants[layerIndex];
                const expTerm = Math.exp(-Math.min(700, time / layerTau));
                const layerQ0 = layerPi * pressureDrop * Math.max(0, depletionRateScale);
                oilRate += layerQ0 * expTerm;
                cumulativeOil += layerQ0 * layerTau * (1 - expTerm);
                pressureFraction += expTerm / layerOilPis.length;
            }

            return [{
                time,
                oilRate,
                waterRate: 0,
                cumulativeOil,
                // Equal-thickness layers have equal storage, so their mean
                // pressures are volume-averaged rather than PI-weighted.
                avgPressure: producerBhp + pressureDrop * pressureFraction,
            }];
        } else if (b < 1e-8) {
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

        return [{
            time,
            oilRate,
            waterRate: 0,
            cumulativeOil,
            avgPressure,
        }];
    });

    return {
        meta: {
            mode: "depletion",
            shapeFactor,
            shapeLabel,
            q0,
            tau,
            arpsB: layeredComposite || finiteSlab ? undefined : b,
            layerTimeConstants,
        },
        production,
    };
}
