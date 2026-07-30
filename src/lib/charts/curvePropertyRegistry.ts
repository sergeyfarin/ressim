/**
 * Semantic property classification for catalog chart curves.
 *
 * A panel may compare cases, numerical/reference solutions, or several
 * components of one property. It must not combine different properties merely
 * because a second y-axis makes that technically possible.
 */
export function chartPropertyForCurve(curveKey: string): string | null {
    if (curveKey.startsWith('water-cut-')) return 'water-cut';
    if (curveKey.startsWith('gas-cut-')) return 'gas-cut';
    if (curveKey.startsWith('recovery-factor')) return 'recovery-factor';
    if (curveKey.startsWith('cum-oil-')) return 'cumulative-oil';
    if (curveKey === 'cum-injection') return 'cumulative-injection';
    if (curveKey.startsWith('oil-rate-') || curveKey === 'published-oil-rate') return 'oil-rate';
    if (curveKey.startsWith('injection-rate-') || curveKey === 'published-injection-rate') return 'injection-rate';
    if (curveKey.startsWith('avg-pressure-') || curveKey === 'published-pressure') return 'average-pressure';
    if (curveKey.endsWith('-bhp-limited-sim')) return 'control-limit-fraction';
    if (curveKey.startsWith('producer-bhp-') || curveKey === 'published-producer-bhp') return 'producer-bhp';
    if (curveKey.startsWith('injector-bhp-') || curveKey === 'published-injector-bhp') return 'injector-bhp';
    if (curveKey.startsWith('gor-') || curveKey === 'published-gor') return 'gor';
    if (curveKey === 'mbe-ooip-ratio') return 'mbe-ooip-ratio';
    if (curveKey.startsWith('drive-')) return 'drive-index';
    if (curveKey.startsWith('pss-productivity-')) return 'pss-productivity';
    if (curveKey.startsWith('pss-shape-factor-')) return 'dietz-shape-factor';
    return null;
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
