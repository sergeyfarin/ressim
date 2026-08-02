/**
 * Semantic property classification for catalog chart curves.
 *
 * A panel may compare cases, numerical/reference solutions, or several
 * components of one property. It must not combine different properties merely
 * because a second y-axis makes that technically possible.
 *
 * **What a curve *is* should be data on the curve, not a fact recovered from its
 * name.** Until 2026-08-02 this module was a ladder of `curveKey.startsWith(…)`
 * tests: renaming a key silently returned `null`, and every new curve had to be
 * taught here as a string rule. Classification now comes from three sources, in
 * order:
 *
 *   1. `curve.property` — declared where the curve is built. `appendSeries`
 *      stamps it, so no built curve can escape unclassified.
 *   2. `CURVE_PROPERTY_BY_KEY` — an explicit table, for the layout validator,
 *      which sees only the curve *keys* a scenario declares and has no built
 *      curve to read a property from.
 *   3. The key-shape rules below — a deliberate, temporary fallback for the
 *      externally-sourced curves whose keys are minted from an artifact key at
 *      runtime (`opm-fine-water-cut`, `published-gor`, …).
 *
 * Step 2 of `docs/CHART_ARCHITECTURE_REVIEW_2026-08-02.md`. Source 3 goes away
 * when panels declare their curves as descriptors; 1 and 2 are the end state.
 */

import type { CurveConfig } from './chartTypes';

/**
 * Every curve key the builders mint, and the property it carries.
 *
 * This is the list a reviewer reads to answer "what does this chart plot?".
 * Keys absent from it fall through to the key-shape rules below.
 */
export const CURVE_PROPERTY_BY_KEY: Record<string, string> = {
    'water-cut-sim': 'water-cut',
    'water-cut-reference': 'water-cut',
    'gas-cut-sim': 'gas-cut',
    'gas-cut-reference': 'gas-cut',
    'avg-water-sat': 'average-water-saturation',
    'recovery-factor-primary': 'recovery-factor',
    'recovery-factor-reference': 'recovery-factor',
    'recovery-factor-gas': 'recovery-factor',
    'recovery-factor-sim': 'recovery-factor',
    'recovery-factor-analytical': 'recovery-factor',
    'cum-oil-sim': 'cumulative-oil',
    'cum-oil-reference': 'cumulative-oil',
    'cum-injection': 'cumulative-injection',
    'oil-rate-sim': 'oil-rate',
    'oil-rate-reference': 'oil-rate',
    'injection-rate-sim': 'injection-rate',
    'avg-pressure-sim': 'average-pressure',
    'avg-pressure-reference': 'average-pressure',
    'producer-bhp-sim': 'producer-bhp',
    'producer-bhp-reference': 'producer-bhp',
    'injector-bhp-sim': 'injector-bhp',
    'injector-bhp-reference': 'injector-bhp',
    'producer-bhp-limited-sim': 'control-limit-fraction',
    'injector-bhp-limited-sim': 'control-limit-fraction',
    'gor-sim': 'gor',
    'gor-reference': 'gor',
    'mbe-ooip-ratio': 'mbe-ooip-ratio',
    'drive-compaction': 'drive-index',
    'drive-oil-expansion': 'drive-index',
    'drive-gas-cap': 'drive-index',
    'p-over-z-sim': 'p-over-z',
    'p-over-z-reference': 'p-over-z',
    'p-over-z-compaction-reference': 'p-over-z',
    'pss-drawdown-sim': 'pss-drawdown',
    'pss-drawdown-reference': 'pss-drawdown',
    'pss-productivity-sim': 'pss-productivity',
    'pss-productivity-reference': 'pss-productivity',
    'pss-shape-factor-sim': 'dietz-shape-factor',
    'pss-shape-factor-reference': 'dietz-shape-factor',
    'sweep-rf-sim': 'sweep-recovery-factor',
    'sweep-rf-reference': 'sweep-recovery-factor',
    'sweep-areal-sim': 'areal-sweep-efficiency',
    'sweep-areal-reference': 'areal-sweep-efficiency',
    'sweep-vertical-sim': 'vertical-sweep-efficiency',
    'sweep-vertical-reference': 'vertical-sweep-efficiency',
    'sweep-combined-sim': 'volumetric-sweep-efficiency',
    'sweep-combined-reference': 'volumetric-sweep-efficiency',
    'sweep-combined-mobile-oil-sim': 'mobile-oil-recovered',
};

/** Property suffixes shared by the runtime-minted external-reference keys. */
const KEY_SHAPE_RULES: Array<[RegExp, string]> = [
    [/water-cut/, 'water-cut'],
    [/gas-cut/, 'gas-cut'],
    [/gas-injection-rate/, 'injection-rate'],
    [/injection-rate/, 'injection-rate'],
    [/oil-rate/, 'oil-rate'],
    [/gas-rate/, 'gas-rate'],
    [/-bhp-limited/, 'control-limit-fraction'],
    [/producer-bhp/, 'producer-bhp'],
    [/injector-bhp/, 'injector-bhp'],
    [/avg-pressure|(^|-)pressure$/, 'average-pressure'],
    [/cum-oil/, 'cumulative-oil'],
    [/cum-gas/, 'cumulative-gas'],
    [/cum-injection/, 'cumulative-injection'],
    [/p-over-z/, 'p-over-z'],
    [/recovery-factor/, 'recovery-factor'],
    [/(^|-)gor$|gor-/, 'gor'],
    [/^drive-/, 'drive-index'],
];

/**
 * Classification for a key minted at runtime from an artifact key
 * (`opm-…`, `opm-fine-…`, `published-…`), where the property is the tail.
 */
function propertyFromKeyShape(curveKey: string): string | null {
    for (const [pattern, property] of KEY_SHAPE_RULES) {
        if (pattern.test(curveKey)) return property;
    }
    return null;
}

/** The property a curve key carries, or null when nothing classifies it. */
export function chartPropertyForCurve(curveKey: string): string | null {
    return CURVE_PROPERTY_BY_KEY[curveKey] ?? propertyFromKeyShape(curveKey);
}

/** The property of a *built* curve: its own declaration first, then its key. */
export function chartPropertyForCurveConfig(curve: CurveConfig): string | null {
    return curve.property ?? (curve.curveKey ? chartPropertyForCurve(curve.curveKey) : null);
}

export function validateSinglePropertyPanel(panelKey: string, curveKeys: readonly string[]): string[] {
    if (curveKeys.length === 0) return [];
    const classified = curveKeys.map((curveKey) => ({ curveKey, property: chartPropertyForCurve(curveKey) }));
    const unknown = classified.filter(({ property }) => property === null).map(({ curveKey }) => curveKey);
    if (unknown.length > 0) {
        return [`panel '${panelKey}' has unclassified curve(s): ${unknown.join(', ')}.`];
    }
    const properties = [...new Set(classified.map(({ property }) => property!))];
    return properties.length <= 1
        ? []
        : [`panel '${panelKey}' mixes properties ${properties.join(', ')} (${curveKeys.join(', ')}).`];
}
