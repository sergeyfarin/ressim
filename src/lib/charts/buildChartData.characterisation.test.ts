/**
 * Characterisation of `buildReferenceComparisonModel`'s output shape.
 *
 * Not a specification — a *record* of what every catalog scenario currently
 * plots, panel by panel and curve by curve, so that a refactor of the chart
 * builder is provably behaviour-preserving. `buildChartData.ts` has no direct
 * unit test; it is exercised indirectly through `referenceComparisonModel.test.ts`,
 * which asserts specific behaviours and would not notice a curve that simply
 * stopped being emitted.
 *
 * Written 2026-08-02 ahead of the chart-layer refactor
 * (`docs/CHART_ARCHITECTURE_REVIEW_2026-08-02.md`). When a diff appears here,
 * the question to answer is "did I mean to change what this scenario plots?" —
 * if yes, update the expectation in the same commit and say so in the message.
 *
 * The run is synthetic and deterministic: no WASM, no solver. Absolute values
 * are irrelevant and deliberately not asserted; only the emitted structure is.
 */

import { describe, expect, it } from 'vitest';
import type { BenchmarkFamily } from '../catalog/benchmarkCases';
import { listScenarios, getScenario } from '../catalog/scenarios';
import { resolveScenarioReferenceSeries } from '../catalog/opmFlowArtifacts';
import { buildBenchmarkRunResult } from '../benchmarkRunModel';
import type { BenchmarkRunSpec } from '../benchmarkRunModel';
import { buildReferenceComparisonModel } from './buildChartData';
import { getPoreVolume } from '../reservoirVolumes';

/** A deterministic, physics-free rate history with every field the builder reads. */
function syntheticRateHistory(params: Record<string, any>) {
    const poreVolume = getPoreVolume(params);
    const initialPressure = Number(params.initialPressure ?? 300);
    const producerBhp = Number(params.producerBhp ?? 50);
    const hasInjector = Boolean(params.injectorEnabled);

    return Array.from({ length: 12 }, (_, index) => {
        const step = index + 1;
        const time = step * 10;
        const decay = Math.exp(-step / 6);
        const oilRate = 50 * decay;
        const waterRate = hasInjector ? 20 * (1 - decay) : 0;
        return {
            time,
            total_production_oil: oilRate,
            total_production_liquid: oilRate + waterRate,
            total_production_gas: 500 * (1 - decay),
            total_injection: hasInjector ? poreVolume * 0.002 : 0,
            avg_reservoir_pressure: producerBhp + (initialPressure - producerBhp) * decay,
            avg_water_saturation: Number(params.initialSaturation ?? 0.2) + 0.2 * (1 - decay),
            producing_gor: 200 + 300 * (1 - decay),
        };
    });
}

function runSpecFor(scenarioKey: string): BenchmarkRunSpec {
    const scenario = getScenario(scenarioKey)!;
    return {
        key: scenario.key,
        caseKey: scenario.key,
        familyKey: scenario.key,
        analyticalMethod: scenario.capabilities.analyticalMethod,
        variantKey: null,
        variantLabel: null,
        label: scenario.label,
        description: scenario.description,
        params: { ...scenario.params },
        steps: Number(scenario.params.steps),
        deltaTDays: Number(scenario.params.delta_t_days),
        historyInterval: 1,
        reference: { kind: 'analytical', source: `${scenario.key}:analytical` },
        comparisonMetric: null,
        breakthroughCriterion: null,
        comparisonMeaning: 'Characterisation fixture.',
    } as BenchmarkRunSpec;
}

function familyFor(scenarioKey: string): BenchmarkFamily {
    const scenario = getScenario(scenarioKey)!;
    return {
        key: scenario.key,
        label: scenario.label,
        description: scenario.description,
        analyticalMethod: scenario.capabilities.analyticalMethod,
        chartLayoutKey: scenario.chartLayoutKey,
        chartLayoutPatch: scenario.chartLayoutPatch,
        showSweepPanel: scenario.capabilities.analyticalMethod === 'sweep',
        sweepGeometry: (scenario.capabilities as { sweepGeometry?: string | null }).sweepGeometry ?? null,
        publishedReferenceSeries: resolveScenarioReferenceSeries(scenario.referenceSources),
    } as unknown as BenchmarkFamily;
}

/** `panel:curveKey,curveKey` for every panel that emitted anything, sorted. */
function digest(scenarioKey: string): string {
    const spec = runSpecFor(scenarioKey);
    const result = buildBenchmarkRunResult({
        spec,
        rateHistory: syntheticRateHistory(spec.params),
    });
    const model = buildReferenceComparisonModel({
        family: familyFor(scenarioKey),
        results: [result],
        xAxisMode: 'time',
    });

    return Object.entries(model.panels)
        .filter(([, panel]) => (panel?.curves?.length ?? 0) > 0)
        .map(([panelKey, panel]) => {
            const keys = [...new Set((panel?.curves ?? []).map((curve) => curve.curveKey ?? '?'))].sort();
            return `${panelKey}: ${keys.join(',')}`;
        })
        .sort()
        .join('\n');
}

describe('buildReferenceComparisonModel — emitted panel/curve structure', () => {
    for (const scenario of listScenarios()) {
        it(`${scenario.key} emits a stable set of panels and curves`, () => {
            const actual = digest(scenario.key);
            // Every scenario must plot *something*; an empty model is the
            // failure mode this file exists to catch.
            expect(actual.length, `${scenario.key} produced no curves at all`).toBeGreaterThan(0);
        });
    }

    it('classifies every curve it emits, including runtime-minted reference keys', () => {
        // The guarantee behind the single-property-per-panel rule: an
        // unclassified curve would silently pass that check. `appendSeries`
        // stamps `property` on every built curve, so a gap here means a new
        // curve key reached the chart without being described anywhere.
        const unclassified: string[] = [];
        for (const scenario of listScenarios()) {
            const spec = runSpecFor(scenario.key);
            const model = buildReferenceComparisonModel({
                family: familyFor(scenario.key),
                results: [buildBenchmarkRunResult({ spec, rateHistory: syntheticRateHistory(spec.params) })],
                xAxisMode: 'time',
            });
            for (const panel of Object.values(model.panels)) {
                for (const curve of panel?.curves ?? []) {
                    if (!curve.property) unclassified.push(`${scenario.key}: ${curve.curveKey ?? curve.label}`);
                }
            }
        }
        expect(unclassified).toEqual([]);
    });

    /**
     * Panels that mix properties today. Two are defects the guard found, one is
     * deliberate. Recorded explicitly so the rule stays enforced everywhere
     * else; see TODO.md, "Chart-layer audit findings".
     */
    const KNOWN_MIXED_PROPERTY_PANELS = new Set([
        // Deliberate: the panel's own title is "Analytical Total E_vol vs
        // Simulated Mobile Oil Recovered" — comparing the two *is* the exhibit,
        // and it ships hidden by default.
        'sweep_combined / sweep_combined_mobile_oil',
        // Defect: a panel titled "Gas Rate" whose ResSim curve is `oil-rate-sim`,
        // which for a dry-gas reservoir is ~0. ResSim has no simulated gas-rate
        // curve at all in the comparison chart.
        'dep_gas_pz / rates',
        // Defect: the OPM artifact contributes `opm-cum-gas` into the cumulative
        // *oil* panel. The layout validator cannot see it, because reference
        // curves are appended by the builder rather than declared by the layout.
        'spe1_gas_injection / cumulative',
    ]);

    it('keeps every panel to a single property', () => {
        // Enforced against what is actually *built*, not only against the curve
        // keys a layout declares — the layout validator cannot see the OPM and
        // published reference curves, which are appended by the builder.
        const violations: string[] = [];
        for (const scenario of listScenarios()) {
            const spec = runSpecFor(scenario.key);
            const model = buildReferenceComparisonModel({
                family: familyFor(scenario.key),
                results: [buildBenchmarkRunResult({ spec, rateHistory: syntheticRateHistory(spec.params) })],
                xAxisMode: 'time',
            });
            for (const [panelKey, panel] of Object.entries(model.panels)) {
                const properties = [...new Set((panel?.curves ?? []).map((curve) => curve.property))];
                if (properties.length > 1 && !KNOWN_MIXED_PROPERTY_PANELS.has(`${scenario.key} / ${panelKey}`)) {
                    violations.push(`${scenario.key} / ${panelKey}: ${properties.join(', ')}`);
                }
            }
        }
        expect(violations).toEqual([]);
    });

    it('records the catalog-wide structure so a refactor can be compared against it', () => {
        const all = listScenarios()
            .map((scenario) => `── ${scenario.key}\n${digest(scenario.key)}`)
            .join('\n');
        expect(all).toMatchSnapshot();
    });
});
