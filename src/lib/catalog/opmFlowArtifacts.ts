import type { ReferenceXAxisMap } from '../charts/axisAdapters';
import type { PublishedReferenceSeries, ScenarioReferenceSourceDef } from './scenarios';
import wfBl1dArtifact from './opm-flow-results/wf_bl1d.json';
import spe1Artifact from './opm-flow-results/spe1_gas_injection.json';
import gasDriveArtifact from './opm-flow-results/gas_drive.json';
import wfGravityArtifact from './opm-flow-results/wf_gravity.json';
import wfNumericsArtifact from './opm-flow-results/wf_numerics.json';
import wfNumericsFineArtifact from './opm-flow-results/wf_numerics_fine.json';
import depGasPzArtifact from './opm-flow-results/dep_gas_pz.json';
import depGasPzGeopressuredArtifact from './opm-flow-results/dep_gas_pz_geopressured.json';

export type ReferenceSourceType =
    | 'analytical'
    | 'published-reference'
    | 'opm-flow-precomputed'
    | 'simulation';

export type OpmFlowArtifactSeries = {
    panelKey: string;
    label: string;
    curveKey: string;
    data: { x: number; y: number }[];
    yAxisID?: string;
};

/**
 * The run's own time -> PVI / cumulative-injection mapping, emitted by
 * `tools/opm_flow` when the case declares a cumulative reservoir-volume
 * injection vector and a pore volume. Optional: cases with no injector (and
 * artifacts generated before the mapping existed) have none, and their series
 * are simply unavailable on injection-based axes.
 */
export type OpmFlowArtifactXAxis = ReferenceXAxisMap & {
    /** Present when the case declares one; a depletion deck need not. */
    poreVolumeM3?: number;
    /** Summary mnemonic the injection mapping was built from, e.g. 'FVIT'. */
    cumulativeInjectionCurve?: string;
    /** Summary mnemonic the gas-production mapping was built from, e.g. 'FGPT'. */
    cumulativeGasCurve?: string;
};

export type OpmFlowArtifact = {
    schemaVersion: 1;
    sourceType: 'opm-flow-precomputed';
    caseKey: string;
    scenarioKey: string;
    label: string;
    flowVersion: string | null;
    deckHash: string;
    generatedAt: string;
    units: Record<string, string>;
    supportedCurves: string[];
    series: OpmFlowArtifactSeries[];
    status: 'deck-ready' | 'flow-run' | 'parsed' | 'error';
    notes?: string;
    xAxis?: OpmFlowArtifactXAxis;
};

const ARTIFACTS = [
    wfBl1dArtifact as OpmFlowArtifact,
    spe1Artifact as OpmFlowArtifact,
    gasDriveArtifact as OpmFlowArtifact,
    wfGravityArtifact as OpmFlowArtifact,
    wfNumericsArtifact as OpmFlowArtifact,
    wfNumericsFineArtifact as OpmFlowArtifact,
    depGasPzArtifact as OpmFlowArtifact,
    depGasPzGeopressuredArtifact as OpmFlowArtifact,
];

export function listOpmFlowArtifacts(): OpmFlowArtifact[] {
    return ARTIFACTS;
}

export function getOpmFlowArtifactsForScenario(scenarioKey: string): OpmFlowArtifact[] {
    return ARTIFACTS.filter((artifact) => artifact.scenarioKey === scenarioKey);
}

/**
 * Resolve bundled OPM Flow artifact series by case key. Silently skips keys with
 * no artifact or an unparsed one, so a deck that has not been run yet degrades to
 * "no reference curves" rather than breaking the chart.
 */
function getOpmFlowArtifactSeriesByKeys(
    caseKeys: readonly string[],
    options: { primary?: boolean; defaultVisible?: boolean } = {},
): PublishedReferenceSeries[] {
    return caseKeys.flatMap((caseKey) => {
        const artifact = ARTIFACTS.find((candidate) => candidate.caseKey === caseKey);
        if (!artifact || artifact.status !== 'parsed') return [];
        return artifact.series.map((series) => ({
            ...series,
            sourceType: 'opm-flow-precomputed' as const,
            sourceArtifactKey: artifact.caseKey,
            // Carried per series so the chart layer never has to look an
            // artifact back up: a series knows how to place itself on any axis
            // it can honestly be placed on.
            ...(artifact.xAxis ? { xAxisMap: artifact.xAxis } : {}),
            // Only stamped for primary content; an overlay leaves it absent so
            // the chart's `primary === true` test reads false.
            ...(options.primary ? { primary: true } : {}),
            ...(options.defaultVisible === false ? { defaultVisible: false } : {}),
        }));
    });
}

/**
 * Resolve a scenario's declared reference sources into the flat series list the
 * chart layer consumes, in declaration order.
 *
 * This is the only path from a scenario to its non-simulation curves. There is
 * deliberately no scenarioKey-based lookup: an artifact appears on a chart only
 * because the scenario named it.
 */
export function resolveScenarioReferenceSeries(
    sources: readonly ScenarioReferenceSourceDef[] | undefined,
): PublishedReferenceSeries[] {
    if (!sources?.length) return [];
    return sources.flatMap((source) => (
        source.kind === 'published'
            ? [...source.series]
            : getOpmFlowArtifactSeriesByKeys(source.artifactKeys, {
                primary: source.role === 'primary',
                defaultVisible: source.defaultVisible,
            })
    ));
}

/** Case keys of every OPM artifact a scenario declares, in declaration order. */
export function listDeclaredOpmFlowArtifactKeys(
    sources: readonly ScenarioReferenceSourceDef[] | undefined,
): string[] {
    return (sources ?? []).flatMap((source) => (
        source.kind === 'opm-flow' ? [...source.artifactKeys] : []
    ));
}
