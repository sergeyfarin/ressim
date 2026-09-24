/**
 * The active output: which run every result consumer is showing.
 *
 * There are two sources. One is the live runtime, the model the worker is (or was last)
 * stepping. The other is a stored run result the reader picked from a sensitivity or comparison
 * set. `resolveOutputSource` makes that choice once. The 3D view's payload, the spatial-profile
 * payload and the default 3D property are then built from the chosen source alone (#14).
 *
 * They used to decide separately. Each payload re-derived "stored result or live?" field by
 * field (`result.params.x ?? live.x`), so a field a result lacked was silently taken from
 * whichever model was live. Two analytical helpers ignored the selection altogether, and one of
 * them mixed the selected run's rock properties with the live run's permeabilities. Those helpers
 * had no consumer left and were deleted.
 *
 * **Extension point.** A new result consumer takes an `OutputSource`, or a payload built here
 * from one, never the runtime and parameter stores directly. A new source kind is a new member of
 * the union: every builder then fails to type-check until it handles it.
 */
import type { GridState, RateHistoryPoint, SimulatorSnapshot, WellState } from '../simulator-types';
import type { RockProps, FluidProps } from '@ressim/analytical/fractionalFlow';
import type { Scenario } from '../catalog/scenarios';
import { resolvePressureDisplayRange, type PressureDisplayRange } from '../visualization/spatialViewModel';
import type { SpatialProfileReference } from '../visualization/spatialProfileModel';
import { getLayerPermeabilities } from '@ressim/charts/analyticalParamAdapters';

export type OutputScenarioMode = 'waterflood' | 'depletion' | 'none';

type OutputSourceBase = {
    /** Everything the source was run with. */
    params: Record<string, unknown>;
    history: SimulatorSnapshot[];
    rateHistory: RateHistoryPoint[];
    /** State at the end of the run. The live source's is the worker's latest state. */
    finalGrid: GridState | null;
    finalWells: WellState | null;
    finalTime: number | null;
    scenarioMode: OutputScenarioMode;
    label: string;
};

export type LiveOutputSource = OutputSourceBase & { kind: 'live' };
export type ResultOutputSource = OutputSourceBase & { kind: 'result'; resultKey: string };
export type OutputSource = LiveOutputSource | ResultOutputSource;

/** The fields of a stored run result an output source needs. */
export type StoredRunOutput = {
    key: string;
    label: string;
    params: Record<string, unknown>;
    history: SimulatorSnapshot[];
    rateHistory: RateHistoryPoint[];
    finalSnapshot: SimulatorSnapshot | null;
};

export type LiveRunOutput = {
    params: Record<string, unknown>;
    history: SimulatorSnapshot[];
    rateHistory: RateHistoryPoint[];
    gridState: GridState | null;
    wellState: WellState | null;
    simTime: number;
    scenarioMode: OutputScenarioMode;
};

/** The selected stored result if there is one, the live runtime otherwise. */
export function resolveOutputSource(input: {
    selected: StoredRunOutput | null;
    selectedScenarioMode: OutputScenarioMode;
    live: LiveRunOutput;
}): OutputSource {
    const { selected, live } = input;
    if (selected) {
        return {
            kind: 'result',
            resultKey: selected.key,
            label: selected.label,
            params: selected.params,
            history: selected.history,
            rateHistory: selected.rateHistory,
            finalGrid: selected.finalSnapshot?.grid ?? null,
            finalWells: selected.finalSnapshot?.wells ?? null,
            finalTime: selected.finalSnapshot?.time ?? selected.rateHistory.at(-1)?.time ?? null,
            scenarioMode: input.selectedScenarioMode,
        };
    }
    return {
        kind: 'live',
        label: 'Live runtime',
        params: live.params,
        history: live.history,
        rateHistory: live.rateHistory,
        finalGrid: live.gridState,
        finalWells: live.wellState,
        finalTime: live.simTime,
        scenarioMode: live.scenarioMode,
    };
}

// ---------- Payloads built from a source ----------

export type OutputSelectionProfile = {
    gridState: GridState | null;
    nx: number; ny: number; nz: number;
    cellDx: number; cellDy: number; cellDz: number;
    simTime: number; porosity: number; rateHistory: RateHistoryPoint[];
    scenarioMode: OutputScenarioMode;
    spatialReference: SpatialProfileReference | null;
    spatialProfileDefaultAxis: 'i' | 'j' | 'k' | 'well-path' | null;
    spatialProfileWellPathLabel: string;
    sourceLabel: string;
    injectorI: number; injectorJ: number; producerI: number; producerJ: number;
    /** Perforated layers; empty means every layer. */
    injectorKLayers: number[];
    producerKLayers: number[];
    initialSaturation: number;
    rockProps: RockProps; fluidProps: FluidProps;
};

export type Output3DSelection = {
    history: SimulatorSnapshot[];
    nx: number; ny: number; nz: number;
    cellDx: number; cellDy: number; cellDz: number; cellDzPerLayer: number[];
    gridState: GridState | null;
    wellState: WellState | null;
    pressureDisplayRange: PressureDisplayRange;
    replayTime: number | null;
    currentIndex: number;
    sourceLabel: string;
};

export type Output3DProperty =
    'pressure' | 'saturation_water' | 'saturation_oil' | 'saturation_gas' | 'saturation_ternary';

const numberArray = (value: unknown): number[] =>
    Array.isArray(value) ? value.map((entry) => Number(entry)) : [];

/** The spatial-profile payload: the source's final state plus what the profile needs to label it. */
export function buildOutputProfile(
    source: OutputSource,
    capabilities: Scenario['capabilities'] | undefined,
): OutputSelectionProfile {
    const p = source.params;
    const sweepGeometry = capabilities?.analyticalMethod === 'sweep' ? capabilities.sweepGeometry : null;
    return {
        gridState: source.finalGrid,
        nx: Number(p.nx), ny: Number(p.ny), nz: Number(p.nz),
        cellDx: Number(p.cellDx), cellDy: Number(p.cellDy), cellDz: Number(p.cellDz),
        simTime: Number(source.finalTime ?? 0),
        porosity: Number(p.reservoirPorosity),
        rateHistory: source.rateHistory,
        scenarioMode: source.scenarioMode,
        spatialReference: sweepGeometry === 'areal' || sweepGeometry === 'both'
            ? { kind: 'sweep', geometry: sweepGeometry, layerPermeabilities: getLayerPermeabilities(p) }
            : source.scenarioMode === 'waterflood' ? { kind: 'buckley-leverett' } : null,
        spatialProfileDefaultAxis: capabilities?.spatialProfile?.defaultAxis ?? null,
        spatialProfileWellPathLabel: capabilities?.spatialProfile?.wellPathLabel
            ?? (capabilities?.hasInjector || p.injectorEnabled === true
                ? 'Injector → producer'
                : 'Diagonal'),
        sourceLabel: source.label,
        injectorI: Number(p.injectorI), injectorJ: Number(p.injectorJ),
        producerI: Number(p.producerI), producerJ: Number(p.producerJ),
        // Which layer the injector is perforated in, so a profile taken down the K axis knows
        // which end of the column the flood starts at.
        injectorKLayers: numberArray(p.injectorKLayers),
        producerKLayers: numberArray(p.producerKLayers),
        initialSaturation: Number(p.initialSaturation),
        rockProps: {
            s_wc: Number(p.s_wc), s_or: Number(p.s_or),
            n_w: Number(p.n_w), n_o: Number(p.n_o),
            k_rw_max: Number(p.k_rw_max), k_ro_max: Number(p.k_ro_max),
        },
        fluidProps: { mu_w: Number(p.mu_w), mu_o: Number(p.mu_o) },
    };
}

/**
 * The 3D view's payload at replay position `requestedIndex`, clamped to the source's history.
 * Without history it shows the source's final state.
 */
export function buildOutput3D(
    source: OutputSource,
    requestedIndex: number,
    liveReplayTime: number | null,
): Output3DSelection {
    const p = source.params;
    const history = source.history;
    const currentIndex = history.length === 0
        ? -1
        : Math.max(0, Math.min(requestedIndex, history.length - 1));
    const snapshot = currentIndex >= 0 ? history[currentIndex] : null;
    return {
        history,
        nx: Number(p.nx), ny: Number(p.ny), nz: Number(p.nz),
        cellDx: Number(p.cellDx), cellDy: Number(p.cellDy), cellDz: Number(p.cellDz),
        cellDzPerLayer: numberArray(p.cellDzPerLayer),
        gridState: snapshot?.grid ?? source.finalGrid,
        wellState: snapshot?.wells ?? source.finalWells,
        pressureDisplayRange: resolvePressureDisplayRange(p),
        replayTime: snapshot?.time
            ?? (source.kind === 'live' ? liveReplayTime : source.finalTime),
        currentIndex,
        sourceLabel: source.label,
    };
}

/** The 3D property to open on: gas saturation for a gas flood, else the scenario's declared one. */
export function defaultOutput3DProperty(
    source: OutputSource,
    capabilities: Scenario['capabilities'] | undefined,
): Output3DProperty | null {
    const p = source.params;
    const gasFlood = source.kind === 'result'
        ? p.injectedFluid === 'gas'
        : p.injectedFluid === 'gas' && p.threePhaseModeEnabled === true;
    if (gasFlood) return 'saturation_gas';
    return (capabilities?.default3DScalar as Output3DProperty | undefined) ?? null;
}
