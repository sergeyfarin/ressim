/**
 * Scenario -> BenchmarkFamily adapter.
 *
 * **Why `scenario/` and not `charts/`.** Its input is a catalog `Scenario` and its output is the
 * `BenchmarkFamily` that `scenario/referenceTypes` defines; neither is a chart concept, and it
 * renders nothing. Sitting in `charts/` made `charts` import the catalog, which was the last
 * thing stopping `charts` from being a package that knows nothing about this application's
 * scenarios. It imports nothing from `charts/` — keep it that way.
 */

import type { BenchmarkFamily } from './referenceTypes';
import {
    resolveCapabilities,
    type Scenario,
    type ScenarioAnalyticalOption,
} from '../catalog/scenarios';
import { resolveScenarioReferenceSeries } from '../catalog/opmFlowArtifacts';

export function buildScenarioComparisonFamily(input: {
    scenario: Scenario | null | undefined;
    activeDimensionKey?: string | null;
    analyticalOption?: ScenarioAnalyticalOption | null;
}): BenchmarkFamily | null {
    const scenario = input.scenario ?? null;
    if (!scenario) return null;

    const resolved = resolveCapabilities(scenario.capabilities);
    const activeDimension = scenario.sensitivities.find((dimension) => dimension.key === input.activeDimensionKey) ?? null;
    const xAxis = resolved.analyticalNativeXAxis as BenchmarkFamily['displayDefaults']['xAxis'];
    const panels = (resolved.primaryRateCurve === 'oil-rate'
        ? ['oil-rate', 'cumulative-oil', 'decline-diagnostics']
        : ['watercut-breakthrough', 'recovery', 'pressure']) as BenchmarkFamily['displayDefaults']['panels'][number][];

    return {
        key: scenario.key,
        baseCaseKey: scenario.key,
        analyticalMethod: resolved.analyticalMethod,
        sensitivityAxes: [],
        reference: {
            kind: 'analytical' as const,
            source: `${scenario.key}:${resolved.analyticalMethod}`,
        },
        displayDefaults: { xAxis, panels },
        stylePolicy: {
            colorBy: 'case' as const,
            lineStyleBy: 'quantity-or-reference' as const,
            separatePressurePanel: true,
        },
        runPolicy: 'compare-to-reference' as const,
        label: scenario.label,
        description: scenario.description,
        baseCase: {
            key: scenario.key,
            label: scenario.label,
            description: scenario.description,
            params: scenario.params,
        },
        showSweepPanel: resolved.showSweepPanel,
        sweepGeometry: resolved.sweepGeometry,
        sweepAnalyticalMethod: input.analyticalOption?.sweepMethod,
        analyticalOverlayMode: activeDimension?.analyticalOverlayMode ?? 'auto',
        publishedReferenceSeries: resolveScenarioReferenceSeries(
            scenario.referenceSources,
            scenario.params,
            input.activeDimensionKey ?? null,
        ),
    } as BenchmarkFamily;
}
