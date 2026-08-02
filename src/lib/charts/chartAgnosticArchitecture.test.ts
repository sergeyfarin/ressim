/**
 * The chart layer's architectural rules, enforced.
 *
 * `catalog/scenarioAgnosticArchitecture.test.ts` already forbids branching on a
 * scenario *key*, and that rule held throughout the audit — it simply was not
 * where the coupling lived. Every finding in
 * `docs/CHART_ARCHITECTURE_REVIEW_2026-08-02.md` was a different shape of
 * leak: branching on the analytical *method*, naming panels in the builder, and
 * reservoir vocabulary in components. Step 6 of that review closes the loop by
 * making each one a test.
 *
 * A failure here is not necessarily a bug — it is the layer drifting back
 * towards knowing what a reservoir is. Fix the leak, or, if the exception is
 * genuine, add it to the list below *with its reason*.
 */

import { describe, expect, it } from 'vitest';
import fs from 'fs';
import path from 'path';

const chartsDir = path.join(__dirname);

function chartSources(filter: (name: string) => boolean): Array<{ name: string; source: string }> {
    return fs.readdirSync(chartsDir)
        .filter((name) => /\.(ts|svelte)$/.test(name) && !/\.test\.ts$/.test(name))
        .filter(filter)
        .map((name) => ({ name, source: fs.readFileSync(path.join(chartsDir, name), 'utf8') }));
}

/** Strips block and line comments so prose about a rule is not read as a breach. */
function withoutComments(source: string): string {
    return source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');
}

describe('the chart layer does not branch on the analytical method', () => {
    // `analyticalMethodRegistry` is where the branch is *supposed* to live: one
    // descriptor per method, consumers ask the registry. It exists because this
    // ladder was once duplicated across four contexts.
    const OWNERS = new Set(['analyticalMethodRegistry.ts']);

    it('leaves method-specific behaviour to the method descriptor', () => {
        const offenders = chartSources((name) => !OWNERS.has(name))
            .filter(({ source }) => /analyticalMethod\s*[!=]==/.test(withoutComments(source)))
            .map(({ name }) => name);

        expect(
            offenders,
            'Add a field to AnalyticalMethodDescriptor and read it, instead of testing the method here: '
            + offenders.join(', '),
        ).toEqual([]);
    });
});

describe('the builder does not name panels', () => {
    /**
     * Panels the builder still addresses directly, and why.
     *
     * All are diagnostic families whose curves come from a *computed object*
     * (`computeMbeDiagnostics`, `computeDietzPssSimulationDiagnostics`) rather
     * than from the run's derived series, so they cannot be expressed as
     * `simulationCurves.ts` rows without a second descriptor shape — a quantity
     * whose source is a diagnostic. That belongs with the accessor work in §5
     * of the review, not before it.
     *
     * The list may shrink. It must not grow: a new simulation curve is a table
     * row in `simulationCurves.ts`.
     */
    const PANELS_THE_BUILDER_MAY_STILL_NAME = new Set([
        'control_limits',
        'mbe_ooip',
        'drive_indices',
        'pss_drawdown',
        'pss_productivity',
        'pss_shape_factor',
    ]);

    it('places simulation curves from the descriptor table, not by naming panels', () => {
        const source = withoutComments(
            fs.readFileSync(path.join(chartsDir, 'buildChartData.ts'), 'utf8'),
        );
        const named = [...source.matchAll(/panels\.(\w+)/g)].map((match) => match[1]);
        const unexpected = [...new Set(named)].filter((panel) => !PANELS_THE_BUILDER_MAY_STILL_NAME.has(panel));

        expect(
            unexpected,
            'Add a row to SIMULATION_CURVES instead of naming a panel in the builder: ' + unexpected.join(', '),
        ).toEqual([]);
    });
});

describe('chart components do not know what a reservoir is', () => {
    /**
     * Rendering is generic. `ChartSubPanel.svelte` draws curves; it must not
     * learn that some of them are oil. The words below are the ones that made
     * the panel system worth auditing.
     */
    const RESERVOIR_VOCABULARY = /\b(oil|water|gas|saturation|watercut|reservoir|recovery|permeability|porosity|wellbore)\b/i;

    it('keeps reservoir vocabulary out of the rendering components', () => {
        const offenders: string[] = [];
        for (const { name, source } of chartSources((file) => file.endsWith('.svelte'))) {
            const body = withoutComments(source);
            const hits = [...new Set(
                (body.match(new RegExp(RESERVOIR_VOCABULARY, 'gi')) ?? []).map((hit) => hit.toLowerCase()),
            )];
            if (hits.length > 0) offenders.push(`${name}: ${hits.join(', ')}`);
        }

        expect(
            offenders,
            'A chart component named a reservoir quantity. Move it to a descriptor '
            + '(runQuantities.ts / simulationCurves.ts / a scale preset): ' + offenders.join(' | '),
        ).toEqual([]);
    });
});
