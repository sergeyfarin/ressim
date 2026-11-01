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
 * SPE5 Case 1 — the fifth comparative solution project's six-component miscible WAG flood.
 *
 * Killough & Kossack (1987) compare simulators on a 7×7×3 layered reservoir produced for two
 * years and then water-alternating-gas injected for eighteen. The fluid is OPM's
 * `Opm::Spe5FluidSystem` (#45), the water phase and Stone II relative permeability are #46, the
 * 3-D box and gravity are #47, the surface-rate controls are #48, and the WAG schedule is #50.
 *
 * **The published project compared four-component and fully compositional models as well as
 * black-oil + solvent representations.** ResSim runs a six-component Peng–Robinson model, which is
 * one of the published model classes. The OPM deck available locally implements the case as
 * black-oil + solvent; that deck is a grid/rock/relperm/schedule source, not the benchmark itself.
 * The appropriate published comparison is the compositional participants' curves, and the
 * independent reference is a JutulDarcy compositional run of this payload — see
 * `docs/SPE5_VALIDATION.md`.
 *
 * **Deliberate differences from the OPM deck implementation, recorded rather than hidden.**
 * - Capillary pressure is zero. The compositional engine has no hydrocarbon capillary pressure;
 *   the deck's water–oil and gas–oil columns are small but not zero.
 * - Relative permeability is the deck's table, interpreted by Stone II. The deck's `SOF3` table is
 *   keyed on oil saturation; the engine's `SWOF`/`SGOF` rows are the union of the two tables'
 *   transformed knots, so no original breakpoint is dropped.
 * - Surface separation is the compositional model's single equilibrium stage at 1 atm / 288.71 K.
 *   The deck's stock-tank rates are converted through that stage, not through the black-oil PVT
 *   table. The rate targets are the deck's.
 *
 * **Recorded deviation.** The paper's GOR 10 Mscf/STB and WOR 5 STB/STB shut-in criteria are not
 * enforced: the compositional run loop has no surface-rate history (#95) or termination evaluation
 * (#98). The payload always requests the full 240 reports.
 */

/** 500 ft in metres. */
const CELL_DX_M = 152.4;
const CELL_DY_M = 152.4;
/** 20 / 30 / 50 ft. */
const LAYER_DZ_M = [6.096, 9.144, 15.24];
const NX = 7;
const NY = 7;
const NZ = 3;
const CELLS = NX * NY * NZ;
const POROSITY = 0.3;
const LAYER_KX_MD = [500, 50, 200];
const LAYER_KY_MD = [500, 50, 200];
const LAYER_KZ_MD = [50, 50, 25];
/** 8325 ft, the top of the reservoir in the deck's `TOPS`. */
const TOP_DEPTH_M = 8325 * 0.3048;
/** The pinned EOS's mass density at 4000 psia / 160 °F gives the hydrostatic gradient. */
const INITIAL_PRESSURE_GRADIENT_BAR_PER_M = 0.054611196;
/** 4000 psia at the 8400 ft datum; the top is 75 ft above it. */
const INITIAL_PRESSURE_BAR =
    4000 * 0.0689476 - 75 * 0.3048 * INITIAL_PRESSURE_GRADIENT_BAR_PER_M;

const INITIAL_COMPOSITION = [0.5, 0.03, 0.07, 0.2, 0.15, 0.05];
const INJECTION_GAS = [0.77, 0.2, 0.03, 0, 0, 0];
const CONNATE_WATER = 0.2;

/** 12000 STB/d in m³/d. */
const OIL_RATE_M3_PER_DAY = 12000 * 0.158987294928;
/** 12000 Mscf/d in m³/d at standard conditions (1 scf = 0.028316846592 m³). */
const GAS_RATE_M3_PER_DAY = 12000 * 1000 * 0.028316846592;
const PRODUCER_BHP_LIMIT_BAR = 1000 * 0.0689476;
const INJECTOR_BHP_LIMIT_BAR = 10000 * 0.0689476;

const WELL_RADIUS_M = 0.25 * 0.3048;
/** `COMPDAT` item 9 is a diameter; these are the deck's 0.5 ft. */

function layerWellIndex(permeabilityMd: number, dzM: number): number {
    return peacemanWellIndex({
        permeabilityMd,
        dxM: CELL_DX_M,
        dyM: CELL_DY_M,
        dzM,
        wellRadiusM: WELL_RADIUS_M,
    });
}

const PRODUCER_CELL = (NZ - 1) * NY * NX + (NY - 1) * NX + (NX - 1);
const INJECTOR_CELL = 0;
/** `COMPDAT` item 10 sets Kh = 10000 mD·ft on every completion. Layers 1 and 3 already have that
 * (500×20 and 200×50); the middle layer's own kh is 50×30 = 1500 and is overridden. These wells sit
 * in layers 1 and 3, so the override does not change their index. */
const PRODUCER_WI = layerWellIndex(LAYER_KX_MD[NZ - 1], LAYER_DZ_M[NZ - 1]);
const INJECTOR_WI = layerWellIndex(LAYER_KX_MD[0], LAYER_DZ_M[0]);

/**
 * The deck's `SWFN` and `SOF3` tables, sampled together.
 *
 * `SOF3` gives `krow` against oil saturation (with connate water present); `SWOF` wants it against
 * water saturation. Linear interpolation reproduces the deck's piecewise-linear table, and every
 * row is kept at a deck node rather than at an invented one.
 */
const SWFN: Array<[number, number]> = [
    [0.2, 0], [0.2899, 0.0022], [0.3778, 0.018], [0.4667, 0.0607], [0.5556, 0.1438],
    [0.6444, 0.2809], [0.7, 0.4089], [0.7333, 0.4855], [0.8222, 0.7709], [0.9111, 1], [1, 1],
];
const SOF3: Array<[number, number | null, number | null]> = [
    [0, 0, 0], [0.0889, 0, 0], [0.15, null, 0], [0.1778, 0, 0.011], [0.2667, 0, 0.037],
    [0.3, 0, null], [0.3556, 0.0123, 0.0878], [0.4444, 0.0835, 0.1715], [0.5333, 0.2178, 0.2963],
    [0.6222, 0.4153, 0.4705], [0.7111, 0.6769, 0.7023], [0.75, null, 0.88], [0.8, 1, 1],
];
const SGFN: Array<[number, number]> = [
    [0, 0], [0.05, 0], [0.0889, 0.001], [0.1778, 0.01], [0.2667, 0.03], [0.3556, 0.05],
    [0.4443, 0.1], [0.5333, 0.2], [0.6222, 0.35], [0.65, 0.39], [0.7111, 0.56], [0.8, 1],
];

/** Linear interpolation on a piecewise-linear table, for a query inside its stated range. */
function interpolate(points: Array<[number, number]>, x: number): number {
    for (let i = 1; i < points.length; i += 1) {
        const [x1, y1] = points[i - 1];
        const [x2, y2] = points[i];
        if (x <= x2) {
            return y1 + ((x - x1) / (x2 - x1)) * (y2 - y1);
        }
    }
    return points[points.length - 1][1];
}

/** The union of two knot sets, sorted. A missing `1*` entry is dropped before interpolation. */
function unionKnots(first: number[], second: number[]): number[] {
    const out: number[] = [];
    for (const value of [...first, ...second]) {
        // A transformed knot can differ from the first table's own value by one ulp. Keep the
        // first occurrence so a raw breakpoint is never replaced by its arithmetic image.
        if (!out.some((existing) => Math.abs(existing - value) <= 1e-12)) {
            out.push(value);
        }
    }
    return out.sort((a, b) => a - b);
}

function statedSo(column: 1 | 2): Array<[number, number]> {
    return SOF3.filter((row) => row[column] !== null).map((row) => [
        row[0],
        row[column] as number,
    ]);
}

/**
 * Build Stone II's `SWOF`/`SGOF` tables from the deck's three tables.
 *
 * The sample nodes are the **union** of each curve's own knots and the transformed `SOF3` knots,
 * so every breakpoint the deck states is preserved exactly. A missing `krow`/`krog` is the deck's
 * `1*`, not a zero: interpolation uses the nearest stated neighbours.
 */
function builtRelperm(): CompositionalRelPermConfig {
    const krow = statedSo(1);
    const krog = statedSo(2);
    const swNodes = unionKnots(
        SWFN.map(([sw]) => sw),
        SOF3.map(([so]) => 1 - so),
    );
    const sgNodes = unionKnots(
        SGFN.map(([sg]) => sg),
        SOF3.map(([so]) => 1 - CONNATE_WATER - so),
    );
    const swof: CompositionalSwofRow[] = swNodes.map((sw) => ({
        sw,
        krw: interpolate(SWFN, sw),
        krow: Math.max(0, interpolate(krow, 1 - sw)),
        pcow: 0,
    }));
    const sgof: CompositionalSgofRow[] = sgNodes.map((sg) => ({
        sg,
        krg: interpolate(SGFN, sg),
        krog: Math.max(0, interpolate(krog, 1 - CONNATE_WATER - sg)),
        pcog: 0,
    }));
    return { model: 'stone-ii', swof, sgof };
}

/**
 * The deck's WAG schedule in absolute days.
 *
 * Two years of production, then one-year water and one-year gas slugs, starting with water in year
 * three and ending with gas in year twenty. `365.25` is the deck's calendar average and keeps every
 * switch on a monthly report boundary.
 */
function wagSchedule(): CompositionalWellEvent[] {
    const events: CompositionalWellEvent[] = [];
    for (let cycle = 0; cycle < 9; cycle += 1) {
        const waterStart = (2 + 2 * cycle) * 365.25;
        const gasStart = waterStart + 365.25;
        events.push({
            time_days: waterStart,
            well: 'INJW',
            control: {
                control: 'surface-rate',
                target_m3_per_day: OIL_RATE_M3_PER_DAY,
                phase: 'water',
                bhp_limit_bar: INJECTOR_BHP_LIMIT_BAR,
            },
        });
        events.push({
            time_days: waterStart,
            well: 'INJG',
            control: { control: 'shut-in' },
        });
        events.push({
            time_days: gasStart,
            well: 'INJW',
            control: { control: 'shut-in' },
        });
        events.push({
            time_days: gasStart,
            well: 'INJG',
            control: {
                control: 'surface-rate',
                target_m3_per_day: GAS_RATE_M3_PER_DAY,
                phase: 'vapour',
                bhp_limit_bar: INJECTOR_BHP_LIMIT_BAR,
            },
        });
    }
    return events;
}

function buildCase(): CompositionalCaseConfig {
    return buildCompositionalCase({
        fluid: 'pinned-spe5',
        cells: CELLS,
        cellDxM: CELL_DX_M,
        cellDyM: CELL_DY_M,
        cellDzM: LAYER_DZ_M[0],
        porosity: POROSITY,
        permeabilityMd: LAYER_KX_MD[0],
        rockReferencePressureBar: 14.7 * 0.0689476,
        // ROCK 14.7 5E-6 /psi.
        rockCompressibilityPerBar: 5e-6 / 0.0689476,
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
            // PVTW 14.7 1.0 3.3E-6 0.70 0.0 / — pressure in psia, compressibility in 1/psi.
            reference_pressure_bar: 14.7 * 0.0689476,
            formation_volume_factor: 1.0,
            compressibility_per_bar: 3.3e-6 / 0.0689476,
            viscosity_cp: 0.7,
            viscosibility_per_bar: 0,
            surface_density_kg_per_m3: 62.4 * 16.018463,
            initial_saturation: CONNATE_WATER,
        },
        wells: [
            {
                id: 'PROD',
                completions: [{ cell: PRODUCER_CELL, well_index: PRODUCER_WI, head_offset_bar: 0 }],
                control: 'surface-rate',
                target_m3_per_day: -OIL_RATE_M3_PER_DAY,
                phase: 'liquid',
                bhp_limit_bar: PRODUCER_BHP_LIMIT_BAR,
            },
            {
                id: 'INJW',
                completions: [{ cell: INJECTOR_CELL, well_index: INJECTOR_WI, head_offset_bar: 0 }],
                control: 'shut-in',
                inject_water: true,
            },
            {
                id: 'INJG',
                completions: [{ cell: INJECTOR_CELL, well_index: INJECTOR_WI, head_offset_bar: 0 }],
                control: 'shut-in',
                injection_composition: [...INJECTION_GAS],
            },
        ],
        schedule: wagSchedule(),
    });
}

export const comp_spe5_wag: Scenario = {
    key: 'comp_spe5_wag',
    label: 'SPE5 Compositional WAG Benchmark',
    catalog: {
        group: 'published-benchmark-decks',
        role: 'benchmark',
        caseMode: '3p',
        parameterSummary: 'SPE5 Case 1 · 7×7×3 six-component WAG · JutulDarcy reference',
    },
    description:
        'SPE5 Case 1: two years of production, then eighteen years of one-year water/gas slugs in a layered 7×7×3 reservoir. Six hydrocarbon components exchange between oil and gas, so the miscible front is not a black-oil saturation shock.',
    analyticalMethodSummary:
        'Simulation-only — no closed-form solution. The independent compositional JutulDarcy reference for the same grid, rock, fluid, relative permeability and WAG schedule is not committed yet; docs/SPE5_VALIDATION.md records the case and the remaining validation work.',
    analyticalMethodReference:
        'Killough, J.E. & Kossack, C.A. (1987) “Fifth Comparative Solution Project: Evaluation of Miscible Flood Simulators”, SPE 16000, Ninth SPE Symposium on Reservoir Simulation. Case deck: OPM/opm-tests spe5/SPE5CASE1.DATA, Open Database License. Fluid: OPM Spe5FluidSystem.hpp. Validation: docs/SPE5_VALIDATION.md.',
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
            'The compositional model is fully implicit. WAG alternates water and hydrocarbon injection, and the miscible front needs component conservation and phase appearance/disappearance solved simultaneously.',
    },
    params: {
        fluidModel: 'compositional',
        compositional: buildCase(),

        // The same geometry in the black-oil vocabulary the 3-D view and generic parameter readers
        // use. The compositional engine reads only `compositional`.
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
        injectorI: 0, injectorJ: 0,
        producerI: NX - 1, producerJ: NY - 1,
        injectorControlMode: 'rate',
        producerControlMode: 'rate',
        targetProducerSurfaceRate: OIL_RATE_M3_PER_DAY,
        targetInjectorSurfaceRate: GAS_RATE_M3_PER_DAY,
        producerBhp: PRODUCER_BHP_LIMIT_BAR,
        injectorBhp: INJECTOR_BHP_LIMIT_BAR,
        bhpMin: PRODUCER_BHP_LIMIT_BAR,
        bhpMax: INJECTOR_BHP_LIMIT_BAR,
        gravityEnabled: true,
        capillaryEnabled: false,
        fimEnabled: true,
        // 20 years at the deck's calendar average month; every WAG switch lands on a report time.
        delta_t_days: 30.4375,
        steps: 240,
    },
    sensitivities: [],
};
