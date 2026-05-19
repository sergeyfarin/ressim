import type { PublishedReferenceSeries } from './scenarios';
import wfBl1dArtifact from './opm-flow-results/wf_bl1d.json';
import spe1Artifact from './opm-flow-results/spe1_gas_injection.json';

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
};

const ARTIFACTS = [
    wfBl1dArtifact as OpmFlowArtifact,
    spe1Artifact as OpmFlowArtifact,
];

export function listOpmFlowArtifacts(): OpmFlowArtifact[] {
    return ARTIFACTS;
}

export function getOpmFlowArtifactsForScenario(scenarioKey: string): OpmFlowArtifact[] {
    return ARTIFACTS.filter((artifact) => artifact.scenarioKey === scenarioKey);
}

export function getOpmFlowPublishedReferenceSeries(scenarioKey: string): PublishedReferenceSeries[] {
    return getOpmFlowArtifactsForScenario(scenarioKey).flatMap((artifact) => {
        if (artifact.status !== 'parsed') return [];
        return artifact.series.map((series) => ({
            ...series,
            sourceType: 'opm-flow-precomputed' as const,
            sourceArtifactKey: artifact.caseKey,
        }));
    });
}
