import type { Scenario } from '../scenarios';
import { buildCompositionalCase, peacemanWellIndex } from '../../compositional/createPayload';
import type {
    CompositionalCaseConfig,
    CompositionalRelPermConfig,
} from '../../compositional/types';

/**
 * 1D compositional CO2 flood — three components, two hydrocarbon phases, no water.
 *
 * CO2 is injected into a CO2/methane/decane mixture at 150 °C. Unlike a black-oil gas flood, the
 * injected CO2 and the resident oil exchange components: CO2 dissolves into the liquid and
 * intermediate components vaporize into the gas, so the phase compositions change everywhere
 * behind the front and the front itself is not a saturation shock of fixed composition. That is the
 * behaviour a black-oil formulation cannot represent at all, and it is the reason this scenario
 * exists.
 *
 * **Provenance.** The case is `OPM/opm-tests` `compositional/1D_COMP.DATA` — five cells over 300 m,
 * 100 mD, 10% porosity — and the fluid, the grid and the relative permeability table are that
 * deck's. © 2024 SINTEF Digital / TNO, Open Database License; the deck and its notice are in
 * `opm/compositional/1d_comp/`.
 *
 * **Validation.** This is the case C12 validated ResSim against, using OPM's `flowexp_comp` as an
 * independent simulator. At matched temporal resolution the two agree to 0.0074 bar over the
 * trajectory and 0.0014% on cumulative injection; the flash agrees with OPM's own to 1.31e-7 in
 * saturation across 140 states of the displacement. Full record and its caveats:
 * `docs/COMPOSITIONAL_VALIDATION.md` §8.
 *
 * **No analytical overlay, and that is not a gap to fill later.** There is no closed-form solution
 * for multicomponent two-phase displacement with interphase mass transfer. A Buckley–Leverett curve
 * would be actively misleading here: it assumes immiscible phases of fixed composition, which is
 * the one assumption this case is built to violate. `gas_drive` is the precedent for a
 * simulation-only scenario.
 */

/**
 * The deck's `SGOF` table, transcribed.
 *
 * `SGOF` is keyed on **gas** saturation and the engine's table is keyed on **liquid** saturation,
 * so the rows are reversed and `S_L = 1 − S_g`. It is Corey-squared — `krg = Sg²`,
 * `kro = (1−Sg)²` — but it is carried as a table rather than recognized as a Corey curve, because
 * what the source supplies is a table and that is what C12 measured against.
 *
 * This is a **sourced** curve, which is why this scenario may ship: the engine flags its `linear`
 * model as verification-only and `docs/COMPOSITIONAL_VALIDATION.md` §6 says not to ship on it.
 */
const DECK_SGOF: CompositionalRelPermConfig = (() => {
    const liquid_saturation: number[] = [];
    const kr_liquid: number[] = [];
    const kr_vapour: number[] = [];
    for (let step = 20; step >= 0; step -= 1) {
        const sg = step * 0.05;
        liquid_saturation.push(1 - sg);
        kr_liquid.push((1 - sg) * (1 - sg));
        kr_vapour.push(sg * sg);
    }
    return { model: 'tabulated', liquid_saturation, kr_liquid, kr_vapour };
})();

const CELLS = 5;
const CELL_DX_M = 60;
const CELL_DY_M = 6;
const CELL_DZ_M = 6;
const PERMEABILITY_MD = 100;
const POROSITY = 0.1;
/** `COMPDAT ... 0.0151` is a **diameter**; reading it as a radius overstates the well index. */
const WELL_DIAMETER_M = 0.0151;

const WELL_INDEX = peacemanWellIndex({
    permeabilityMd: PERMEABILITY_MD,
    dxM: CELL_DX_M,
    dyM: CELL_DY_M,
    dzM: CELL_DZ_M,
    wellRadiusM: WELL_DIAMETER_M / 2,
});

/** `ZMF` — 10% CO2, 30% methane, 60% decane. */
const INITIAL_COMPOSITION = [0.1, 0.3, 0.6];
/** `WELLSTRE ISTR 1.0 0.0 0.0` — the injected stream is pure CO2. */
const INJECTION_STREAM = [1, 0, 0];

/** The reservoir is 300 m long however it is discretized. Refinement splits it; it does not change it. */
const RESERVOIR_LENGTH_M = CELLS * CELL_DX_M;

/**
 * Build the case at a given cell count.
 *
 * The reservoir is fixed: same length, same rock, same fluid, same well pressures. Only the
 * discretization moves — which is what `published-benchmark-decks` permits and what C12's own
 * refinement study varied. The well index moves with it, because Peaceman's equivalent radius
 * depends on the cell the well sits in; that is a consequence of refinement, not a different well.
 */
function buildCase(cells: number = CELLS): CompositionalCaseConfig {
    const dxM = RESERVOIR_LENGTH_M / cells;
    const wellIndex = peacemanWellIndex({
        permeabilityMd: PERMEABILITY_MD,
        dxM,
        dyM: CELL_DY_M,
        dzM: CELL_DZ_M,
        wellRadiusM: WELL_DIAMETER_M / 2,
    });
    return buildCompositionalCase({
        fluid: 'pinned-ternary',
        cells,
        cellDxM: dxM,
        cellDyM: CELL_DY_M,
        cellDzM: CELL_DZ_M,
        porosity: POROSITY,
        permeabilityMd: PERMEABILITY_MD,
        initialPressureBar: 75,
        initialComposition: INITIAL_COMPOSITION,
        relperm: DECK_SGOF,
        // `ROCK 68.9476 0` — the deck's rock is incompressible.
        rockReferencePressureBar: 68.9476,
        rockCompressibilityPerBar: 0,
        wells: [
            {
                id: 'INJ',
                completions: [{ cell: 0, well_index: wellIndex }],
                control: 'bhp',
                target_bar: 150,
                injection_composition: INJECTION_STREAM,
            },
            {
                id: 'PROD',
                completions: [{ cell: cells - 1, well_index: wellIndex }],
                control: 'bhp',
                target_bar: 50,
            },
        ],
    });
}

export const comp_co2_1d: Scenario = {
    key: 'comp_co2_1d',
    label: '1D Compositional CO₂ Flood',
    catalog: {
        group: 'published-benchmark-decks',
        role: 'benchmark',
        caseMode: '3p',
        parameterSummary: 'Three-component CO₂ flood · compositional · OPM flowexp_comp reference',
    },
    description:
        'CO₂ injected into a CO₂/methane/decane mixture at 150 °C. Components transfer between liquid and vapour as the front advances, so phase compositions change behind it — behaviour black-oil cannot represent. From OPM’s compositional deck.',
    analyticalMethodSummary:
        'Simulation-only — no analytical overlay, and none is possible. There is no closed-form solution for multicomponent two-phase displacement with interphase mass transfer; a Buckley–Leverett curve assumes immiscible phases of fixed composition, which is the assumption this case is built to violate. The reference is an independent simulator instead: OPM’s flowexp_comp, agreeing to 0.0074 bar at matched temporal resolution.',
    analyticalMethodReference:
        'Case: OPM/opm-tests compositional/1D_COMP.DATA, © 2024 SINTEF Digital / TNO, Open Database License. Validation: docs/COMPOSITIONAL_VALIDATION.md §8 (C12), against OPM flowexp_comp release/2026.04/final.',
    chartLayoutKey: 'gas',
    capabilities: {
        analyticalMethod: 'none',
        hasInjector: true,
        default3DScalar: 'saturation_gas',
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'fim',
        rationale:
            'The compositional model is fully implicit throughout. Phase appearance and disappearance, and the equilibrium the flash solves at every cell, are not representable in an IMPES split.',
    },
    params: {
        // The compositional case. `fluidModel` is what routes this to the compositional engine;
        // absent, it would be read as black-oil, which is the rule that protects every scenario
        // serialized before this one existed.
        fluidModel: 'compositional',
        compositional: buildCase(),

        // The grid, mirrored in the black-oil vocabulary so the 3D view and the generic parameter
        // readers see a consistent geometry. The compositional engine reads none of these.
        nx: CELLS, ny: 1, nz: 1,
        cellDx: CELL_DX_M, cellDy: CELL_DY_M, cellDz: CELL_DZ_M,
        reservoirPorosity: POROSITY,
        uniformPermX: PERMEABILITY_MD, uniformPermY: PERMEABILITY_MD, uniformPermZ: PERMEABILITY_MD,
        permMode: 'uniform',
        initialPressure: 75,
        injectorEnabled: true,
        injectorI: 0, injectorJ: 0,
        producerI: CELLS - 1, producerJ: 0,
        injectorControlMode: 'pressure',
        producerControlMode: 'pressure',
        injectorBhp: 150, producerBhp: 50,
        delta_t_days: 0.05,
        steps: 400,
        fimEnabled: true,
    },
    defaultSensitivityDimensionKey: 'grid_refinement',
    sensitivities: [
        {
            key: 'grid_refinement',
            label: 'Grid Refinement',
            description:
                'The same 300 m reservoir on 5, 10, 20 and 40 cells — the ladder C12 ran against OPM’s simulator re-solved on each grid, agreeing to 0.03 bar at every one. The front sharpens as numerical diffusion falls; the reservoir does not.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'grid_5',
                    label: '5 cells',
                    description: 'The deck’s own grid. Coarse enough that the CO₂ front is spread over a cell or two.',
                    paramPatch: { nx: CELLS, cellDx: RESERVOIR_LENGTH_M / CELLS, producerI: CELLS - 1, compositional: buildCase(CELLS) },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_10',
                    label: '10 cells',
                    description: 'Halving the cell size. The front sharpens; where it is at a given time barely moves.',
                    paramPatch: { nx: 10, cellDx: RESERVOIR_LENGTH_M / 10, producerI: 9, compositional: buildCase(10) },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_20',
                    label: '20 cells',
                    description: 'Halving again. The displacement is converging, not changing.',
                    paramPatch: { nx: 20, cellDx: RESERVOIR_LENGTH_M / 20, producerI: 19, compositional: buildCase(20) },
                    affectsAnalytical: false,
                },
                {
                    key: 'grid_40',
                    label: '40 cells',
                    description: 'The finest grid C12 measured against an independently re-solved reference.',
                    paramPatch: { nx: 40, cellDx: RESERVOIR_LENGTH_M / 40, producerI: 39, compositional: buildCase(40) },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'timestep',
            label: 'Timestep',
            description:
                'How finely time is resolved. The model is fully implicit, so a coarse step is stable rather than divergent — it smears the front and understates the transient. C12 measured this ladder converging at first order, as backward Euler gives.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'dt_base',
                    label: '0.05 d',
                    description: 'The step C12’s comparisons used, on the converged part of the curve.',
                    paramPatch: { delta_t_days: 0.05, steps: 400 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_coarse',
                    label: '0.25 d',
                    description: 'Five times coarser. Still stable, and visibly less sharp through the transient.',
                    paramPatch: { delta_t_days: 0.25, steps: 80 },
                    affectsAnalytical: false,
                },
                {
                    key: 'dt_fine',
                    label: '0.0125 d',
                    description: 'Four times finer, and close enough to the coarser step to show the answer has settled.',
                    paramPatch: { delta_t_days: 0.0125, steps: 1600 },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
