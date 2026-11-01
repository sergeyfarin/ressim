import type { Scenario } from '../scenarios';
import { buildCompositionalCase, peacemanWellIndex } from '../../compositional/createPayload';
import type {
    CompositionalCaseConfig,
    CompositionalRelPermConfig,
    CompositionalSgofRow,
    CompositionalSwofRow,
    CompositionalWellEvent,
} from '../../compositional/types';

/**
 * SPE3 Case 1 — **modified: fixed recycle composition**.
 *
 * Kenyon & Behie (1987) cycle separator gas through a 9×9×4, four-layer reservoir for ten years,
 * then blow it down for five. The published problem recycles the combined separator vapour, whose
 * composition changes as the reservoir produces. This payload holds the injected stream at the
 * initial separator-gas composition because the engine has no produced-gas composition feedback
 * (#97). The 4700 Mscf/d rate is the published rate; the frozen composition is an explicit
 * admission gap, not a property established by the black-oil deck's missing `WELLSTRE`.
 *
 * **The separator train is fixed at its initial high-pressure primary (815 psia).** #51 records
 * the published rule that the primary switches to 315 psia below 2500 psia reservoir pressure.
 * The compositional engine carries one separator train per case, so a run that crosses that
 * pressure would need a runtime switch it does not have. The scenario states the pressure at
 * which that matters rather than hiding it; the cycling period is above it in the reference run,
 * and the blowdown tail is labelled in `docs/SPE3_VALIDATION.md`.
 *
 * **The published project solicited compositional simulations.** The OPM deck available locally
 * is a black-oil approximation with `VAPOIL`, not the benchmark itself; it is a grid, rock, well
 * and schedule source. This scenario runs the nine-component Juell–Whitson–Hoda characterization
 * #51 admitted, so the appropriate published comparison is the compositional participants'
 * curves, and the independent reference is a JutulDarcy compositional run of this payload.
 *
 * **Deliberate differences from the OPM deck implementation, recorded rather than hidden.**
 * - Capillary pressure is zero; the deck's water–oil column is negligible and its gas–oil column
 *   is zero.
 * - Surface separation is the three-stage train in the pinned fluid, not the deck's PVT table.
 * - The recycle composition is frozen; see #97 and `docs/SPE3_VALIDATION.md`.
 */

/** 293.3 ft in metres. */
const CELL_DX_M = 293.3 * 0.3048;
const CELL_DY_M = 293.3 * 0.3048;
/** 30 / 30 / 50 / 50 ft. */
const LAYER_DZ_M = [30 * 0.3048, 30 * 0.3048, 50 * 0.3048, 50 * 0.3048];
const NX = 9;
const NY = 9;
const NZ = 4;
const CELLS = NX * NY * NZ;
const POROSITY = 0.13;
const LAYER_KX_MD = [130, 40, 20, 150];
const LAYER_KY_MD = [130, 40, 20, 150];
const LAYER_KZ_MD = [13, 4, 2, 15];
/** 7315 ft, the top of the reservoir in the deck's `TOPS`. */
const TOP_DEPTH_M = 7315 * 0.3048;
/** The pinned EOS's mass density at 3550 psia / 200 °F gives the hydrostatic gradient. */
const INITIAL_PRESSURE_GRADIENT_BAR_PER_M = 0.030348685;
/** 3550 psia at the 7500 ft datum, 185 ft below the top. */
const INITIAL_PRESSURE_BAR =
    3550 * 0.0689476 - 185 * 0.3048 * INITIAL_PRESSURE_GRADIENT_BAR_PER_M;

/** The nine-component Juell–Whitson–Hoda original, in the pinned component order. */
const INITIAL_COMPOSITION = [0.0121, 0.0194, 0.6599, 0.0869, 0.0591, 0.0967, 0.0472, 0.0153, 0.0034];
const CONNATE_WATER = 0.16;

/** 6200 Mscf/d of separator gas, in m³/d at standard conditions. */
const PRODUCER_GAS_RATE_M3_PER_DAY = 6200 * 1000 * 0.028316846592;
/** 4700 Mscf/d of recycle gas. */
const INJECTOR_GAS_RATE_M3_PER_DAY = 4700 * 1000 * 0.028316846592;
const PRODUCER_BHP_LIMIT_BAR = 500 * 0.0689476;
const INJECTOR_BHP_LIMIT_BAR = 4000 * 0.0689476;

const WELL_RADIUS_M = 1.0 * 0.3048;

function layerWellIndex(permeabilityMd: number, dzM: number): number {
    return peacemanWellIndex({
        permeabilityMd,
        dxM: CELL_DX_M,
        dyM: CELL_DY_M,
        dzM,
        wellRadiusM: WELL_RADIUS_M,
    });
}

/** Row-major `(k*ny + j)*nx + i`, the engine's own order. */
function cell(i: number, j: number, k: number): number {
    return (k * NY + j) * NX + i;
}

const PRODUCER_CELLS = [cell(6, 6, 2), cell(6, 6, 3)];
const INJECTOR_CELLS = [cell(0, 0, 0), cell(0, 0, 1)];
const PRODUCER_WI = [layerWellIndex(LAYER_KX_MD[2], LAYER_DZ_M[2]), layerWellIndex(LAYER_KX_MD[3], LAYER_DZ_M[3])];
const INJECTOR_WI = [layerWellIndex(LAYER_KX_MD[0], LAYER_DZ_M[0]), layerWellIndex(LAYER_KX_MD[1], LAYER_DZ_M[1])];
/** Per-well datum: the top-most perforation, as `WELSPECS` names it. */
const PRODUCER_HEAD_OFFSETS = [25 * 0.3048 * INITIAL_PRESSURE_GRADIENT_BAR_PER_M, 75 * 0.3048 * INITIAL_PRESSURE_GRADIENT_BAR_PER_M];
const INJECTOR_HEAD_OFFSETS = [15 * 0.3048 * INITIAL_PRESSURE_GRADIENT_BAR_PER_M, 45 * 0.3048 * INITIAL_PRESSURE_GRADIENT_BAR_PER_M];

/** Kenyon & Behie Table 2, `SGOF`; the deck's capillary column is zero. */
const SGOF: Array<[number, number, number]> = [
    [0.0, 0.0, 0.8], [0.04, 0.005, 0.65], [0.08, 0.013, 0.513], [0.12, 0.026, 0.4],
    [0.16, 0.04, 0.315], [0.2, 0.058, 0.25], [0.24, 0.078, 0.196], [0.28, 0.1, 0.15],
    [0.32, 0.126, 0.112], [0.36, 0.156, 0.082], [0.4, 0.187, 0.06], [0.44, 0.222, 0.04],
    [0.48, 0.26, 0.024], [0.52, 0.3, 0.012], [0.56, 0.348, 0.005], [0.6, 0.4, 0],
    [0.64, 0.45, 0], [0.68, 0.505, 0], [0.72, 0.562, 0], [0.76, 0.62, 0],
    [0.8, 0.68, 0], [0.84, 0.74, 0],
];
/** Kenyon & Behie Table 2, `SWOF`; the deck's capillary column is nonzero and is not carried. */
const SWOF: Array<[number, number, number]> = [
    [0.16, 0.0, 0.8], [0.2, 0.002, 0.65], [0.24, 0.01, 0.513], [0.28, 0.02, 0.4],
    [0.32, 0.033, 0.315], [0.36, 0.049, 0.25], [0.4, 0.066, 0.196], [0.44, 0.09, 0.15],
    [0.48, 0.119, 0.112], [0.52, 0.15, 0.082], [0.56, 0.186, 0.06], [0.6, 0.227, 0.04],
    [0.64, 0.277, 0.024], [0.68, 0.33, 0.012], [0.72, 0.39, 0.005], [0.76, 0.462, 0],
    [0.8, 0.54, 0], [0.84, 0.62, 0], [0.88, 0.71, 0], [0.92, 0.8, 0], [0.96, 0.9, 0],
    [1.0, 1.0, 0],
];

function builtRelperm(): CompositionalRelPermConfig {
    const swof: CompositionalSwofRow[] = SWOF.map(([sw, krw, krow]) => ({ sw, krw, krow, pcow: 0 }));
    const sgof: CompositionalSgofRow[] = SGOF.map(([sg, krg, krog]) => ({ sg, krg, krog, pcog: 0 }));
    return { model: 'stone-ii', swof, sgof };
}

/** Injection is held at 4700 Mscf/d for ten years, then stopped; production continues to year 15. */
function schedule(): CompositionalWellEvent[] {
    return [
        {
            time_days: 10 * 365.25,
            well: 'INJ',
            control: { control: 'shut-in' },
        },
    ];
}

function buildCase(): CompositionalCaseConfig {
    return buildCompositionalCase({
        fluid: 'pinned-spe3',
        cells: CELLS,
        cellDxM: CELL_DX_M,
        cellDyM: CELL_DY_M,
        cellDzM: LAYER_DZ_M[0],
        porosity: POROSITY,
        permeabilityMd: LAYER_KX_MD[0],
        // ROCK 3550 4E-6 /psi.
        rockReferencePressureBar: 3550 * 0.0689476,
        rockCompressibilityPerBar: 4e-6 / 0.0689476,
        initialPressureBar: INITIAL_PRESSURE_BAR,
        initialComposition: INITIAL_COMPOSITION,
        relperm: builtRelperm(),
        boxGrid: {
            nx: NX,
            ny: NY,
            nz: NZ,
            layer_dz_m: LAYER_DZ_M,
            layer_kx_md: LAYER_KX_MD,
            layer_ky_md: LAYER_KY_MD,
            layer_kz_md: LAYER_KZ_MD,
            top_depth_m: TOP_DEPTH_M,
        },
        gravityEnabled: true,
        initialPressureGradientBarPerM: INITIAL_PRESSURE_GRADIENT_BAR_PER_M,
        water: {
            // PVTW 3427.6 1.02629 0.30698E-05 0.31107 0.59410E-05 /.
            reference_pressure_bar: 3427.6 * 0.0689476,
            formation_volume_factor: 1.02629,
            compressibility_per_bar: 0.30698e-5 / 0.0689476,
            viscosity_cp: 0.31107,
            viscosibility_per_bar: 0.5941e-5 / 0.0689476,
            surface_density_kg_per_m3: 62.37 * 16.018463,
            initial_saturation: CONNATE_WATER,
        },
        wells: [
            {
                id: 'PROD',
                completions: PRODUCER_CELLS.map((index, layer) => ({
                    cell: index,
                    well_index: PRODUCER_WI[layer],
                    head_offset_bar: PRODUCER_HEAD_OFFSETS[layer],
                })),
                control: 'surface-rate',
                target_m3_per_day: -PRODUCER_GAS_RATE_M3_PER_DAY,
                phase: 'vapour',
                bhp_limit_bar: PRODUCER_BHP_LIMIT_BAR,
            },
            {
                id: 'INJ',
                completions: INJECTOR_CELLS.map((index, layer) => ({
                    cell: index,
                    well_index: INJECTOR_WI[layer],
                    head_offset_bar: INJECTOR_HEAD_OFFSETS[layer],
                })),
                control: 'surface-rate',
                target_m3_per_day: INJECTOR_GAS_RATE_M3_PER_DAY,
                phase: 'vapour',
                bhp_limit_bar: INJECTOR_BHP_LIMIT_BAR,
                // The recycle stream is frozen at the initial separator-gas composition. The
                // published case recycles the changing combined separator vapour; that feedback
                // is not implemented, so this payload is the explicit modified case tracked by
                // #97. The composition is the stock-tank gas of the pinned fluid at the
                // 815 psia / 80 °F primary, normalised, not a value inferred from the deck.
                injection_composition: [
                    0.013228073195, 0.02122247367, 0.721862750614, 0.09465804254,
                    0.06114881315, 0.08615072872, 0.001683906626, 0.000045210653, 8.32e-10,
                ],
            },
        ],
        schedule: schedule(),
    });
}

export const comp_spe3_gas_cycling: Scenario = {
    key: 'comp_spe3_gas_cycling',
    label: 'SPE3 Gas Cycling Benchmark (modified fixed recycle)',
    catalog: {
        group: 'published-benchmark-decks',
        role: 'benchmark',
        caseMode: '3p',
        parameterSummary: 'SPE3 Case 1 (modified) · fixed recycle composition · 9×9×4 nine components · withheld',
    },
    description:
        'Modified SPE3 Case 1: ten years of gas cycling in a four-layer retrograde-condensate reservoir, then five years of blowdown. The recycle composition is frozen; the published case recycles changing separator vapour (#97).',
    analyticalMethodSummary:
        'Simulation-only — there is no closed-form solution for multicomponent retrograde condensation with cycling. The independent compositional JutulDarcy reference of this modified case — same grid, characterization and separator train, fixed recycle stream — is not committed yet; docs/SPE3_VALIDATION.md records the admission gap and the remaining validation work.',
    analyticalMethodReference:
        'Kenyon, D.E. & Behie, G.A. (1987) “Third SPE Comparative Solution Project: Gas Cycling of Retrograde Condensate Reservoirs”, JPT 39(8), 981–997. Characterization: Juell, Whitson & Hoda (2009), SPE 121252, Tables 3–5. Deck: OPM/opm-tests spe3/SPE3CASE1.DATA, Open Database License. This is a modified case: the recycle composition is fixed (#97). Validation: docs/SPE3_VALIDATION.md.',
    chartLayoutKey: 'compositional',
    capabilities: {
        analyticalMethod: 'none',
        hasInjector: true,
        default3DScalar: 'saturation_gas',
        requiresThreePhaseMode: true,
    },
    solverPolicy: {
        defaultSolver: 'fim',
        rationale:
            'The compositional model is fully implicit. Retrograde condensation, gas cycling and the blowdown tail all change phase state and composition, which an IMPES split cannot represent.',
    },
    params: {
        fluidModel: 'compositional',
        compositional: buildCase(),

        nx: NX, ny: NY, nz: NZ,
        cellDx: CELL_DX_M, cellDy: CELL_DY_M, cellDz: LAYER_DZ_M[0],
        cellDzPerLayer: [...LAYER_DZ_M],
        reservoirPorosity: POROSITY,
        permMode: 'perLayer',
        uniformPermX: LAYER_KX_MD[0], uniformPermY: LAYER_KY_MD[0], uniformPermZ: LAYER_KZ_MD[0],
        layerPermsX: [...LAYER_KX_MD], layerPermsY: [...LAYER_KY_MD], layerPermsZ: [...LAYER_KZ_MD],
        initialPressure: INITIAL_PRESSURE_BAR,
        initialPressureGradient: INITIAL_PRESSURE_GRADIENT_BAR_PER_M,
        depth_reference: TOP_DEPTH_M,
        initialSaturation: CONNATE_WATER,
        injectorEnabled: true,
        injectorI: 0, injectorJ: 0, injectorKLayers: [0, 1],
        producerI: 6, producerJ: 6, producerKLayers: [2, 3],
        injectorControlMode: 'rate',
        producerControlMode: 'rate',
        targetProducerSurfaceRate: PRODUCER_GAS_RATE_M3_PER_DAY,
        targetInjectorSurfaceRate: INJECTOR_GAS_RATE_M3_PER_DAY,
        producerBhp: PRODUCER_BHP_LIMIT_BAR,
        injectorBhp: INJECTOR_BHP_LIMIT_BAR,
        bhpMin: PRODUCER_BHP_LIMIT_BAR,
        bhpMax: INJECTOR_BHP_LIMIT_BAR,
        gravityEnabled: true,
        capillaryEnabled: false,
        fimEnabled: true,
        // 15 years at the deck's calendar average month.
        delta_t_days: 30.4375,
        steps: 180,
    },
    sensitivities: [],
};
