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
import { getPoreVolume } from '@ressim/quantities/reservoirVolumes';
import { buildRunResult, type RunSpec } from '../scenario/runModel';
import { buildCompositionalRunRecord } from '../compositional/runRecord';
import type { CompositionalCaseConfig, CompositionalSnapshot } from '../compositional/types';

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
            total_injection: hasInjector ? poreVolume * 0.002 : 0, total_injection_resv: hasInjector ? poreVolume * 0.002 : 0,
            avg_reservoir_pressure: producerBhp + (initialPressure - producerBhp) * decay,
            avg_water_saturation: Number(params.initialSaturation ?? 0.2) + 0.2 * (1 - decay),
            producing_gor: 200 + 300 * (1 - decay),
        };
    });
}

/** The component ids each pinned fluid reports (`fluid/pinned.rs`). */
const PINNED_COMPONENT_IDS: Record<string, string[]> = {
    'pinned-ternary': ['CO2', 'C1', 'C10'],
    'pinned-binary': ['C1', 'C10'],
};

/** A deterministic, physics-free compositional run with every field the series reads. */
function syntheticCompositionalSnapshots(config: CompositionalCaseConfig): CompositionalSnapshot[] {
    const componentIds = PINNED_COMPONENT_IDS[config.fluid];
    const cells = config.grid.cells;
    return Array.from({ length: 12 }, (_, step) => ({
        time_days: step,
        component_ids: componentIds,
        pressure: Array.from({ length: cells }, () => config.initial_pressure_bar - step),
        composition: config.initial_composition.map((z) => Array.from({ length: cells }, () => z)),
        phase_state: Array.from({ length: cells }, () => 'two-phase'),
        vapour_saturation: Array.from({ length: cells }, () => step / 12),
        inventory: componentIds.map(() => 100 + step),
        cumulative_well_moles: [componentIds.map(() => step)],
    }));
}

/** The run each scenario's engine produces: a rate history, or a compositional record. */
function syntheticResult(spec: BenchmarkRunSpec) {
    if (spec.params.fluidModel === 'compositional') {
        const config = spec.params.compositional as CompositionalCaseConfig;
        return buildRunResult({
            spec: spec as RunSpec,
            rateHistory: [],
            compositional: buildCompositionalRunRecord({
                config,
                snapshots: syntheticCompositionalSnapshots(config),
                stop: { reason: 'completed', message: '' },
            }),
        });
    }
    return buildBenchmarkRunResult({ spec, rateHistory: syntheticRateHistory(spec.params) });
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
    const result = syntheticResult(runSpecFor(scenarioKey));
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
                results: [syntheticResult(spec)],
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
     * Panels that mix properties on purpose. The two defects this guard found
     * on 2026-08-02 — `dep_gas_pz`'s gas-rate panel drawing the oil rate, and
     * SPE1's cumulative-oil panel receiving OPM's cumulative gas — are fixed,
     * not excused; gas rate and cumulative gas now have their own quantities
     * and panels.
     */
    const KNOWN_MIXED_PROPERTY_PANELS = new Set([
        // Deliberate: the panel's own title is "Analytical Total E_vol vs
        // Simulated Mobile Oil Recovered" — comparing the two *is* the exhibit,
        // and it ships hidden by default.
        'sweep_combined / sweep_combined_mobile_oil',
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
                results: [syntheticResult(spec)],
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
