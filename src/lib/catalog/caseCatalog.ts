import catalogDataRaw from './catalog.json';

// --- Type Definitions ---
export type CaseMode = 'dep' | 'wf' | 'sim' | 'benchmark';

export type DimensionOption = {
    value: string;
    label: string;
    params?: Record<string, any>;
    default?: boolean;
};

export type Dimension = {
    key: string;
    label: string;
    options: DimensionOption[];
};

export type DisabilityRule = {
    when: Record<string, string | string[]>;
    disable: Record<string, string[]>;
    note?: string;
    reason?: string;
};

export type BenchmarkEntry = {
    key: string;
    label: string;
    description: string;
    params: Record<string, any>;
};

export type ModeCatalog = {
    baseParams: Record<string, any>;
    dimensions: Dimension[];
    disabilityRules: DisabilityRule[];
};

export type CatalogSchema = {
    version: number;
    defaults: Record<string, any>;
    modes: Record<CaseMode, ModeCatalog>;
    benchmarks: BenchmarkEntry[];
};

export const catalog: CatalogSchema = catalogDataRaw as unknown as CatalogSchema;

export type ToggleState = Record<string, string>;

function normalizeMode(mode: string | undefined): CaseMode {
    if (mode === 'wf' || mode === 'sim' || mode === 'benchmark') return mode;
    return 'dep';
}

export function getModeCatalog(mode: string | undefined): ModeCatalog {
    return catalog.modes[normalizeMode(mode)] ?? catalog.modes.dep;
}

export function getModeDimensions(mode: string | undefined): Dimension[] {
    return getModeCatalog(mode).dimensions;
}

// --- Helper Functions ---

/**
 * Derives well grid indices based on Geometry and Well Position selections.
 */
export function computeWellPositions(params: Record<string, any>, geo: string, well: string): Record<string, number> {
    const nx = Number(params.nx || 1);
    const ny = Number(params.ny || 1);

    let injI = 0, injJ = 0;
    let prodI = nx - 1, prodJ = ny - 1;

    if (geo === '1d') {
        injI = 0; injJ = 0;
        prodI = nx - 1; prodJ = 0;
    } else if (geo === '2dxz') {
        if (well === 'e2e') {
            injI = 0; injJ = 0;
            prodI = nx - 1; prodJ = 0;
        }
    } else {
        // 2D/3D Areal
        if (well === 'e2e' || well === 'corner') {
            injI = 0; injJ = 0;
            prodI = nx - 1; prodJ = ny - 1;
        } else if (well === 'center') {
            injI = 0; injJ = 0; // if injector present
            prodI = Math.floor(nx / 2); prodJ = Math.floor(ny / 2);
        } else if (well === 'offctr') {
            injI = 0; injJ = 0;
            prodI = Math.floor(nx / 4); prodJ = Math.floor(ny / 2);
        }
    }

    return { injectorI: injI, injectorJ: injJ, producerI: prodI, producerJ: prodJ };
}

/**
 * Composes the final physical parameters by overlaying defaults, all active dimension params, and derived well positions.
 */
export function composeCaseParams(toggles: ToggleState): Record<string, any> {
    const mode = normalizeMode(toggles.mode);

    if (mode === 'benchmark') {
        const bench = catalog.benchmarks.find(b => b.key === toggles.benchmarkId);
        if (bench) return { ...catalog.defaults, ...bench.params };
        return catalog.defaults;
    }

    const modeCatalog = getModeCatalog(mode);
    let params = { ...catalog.defaults, ...modeCatalog.baseParams };

    // Overlay order strictly follows JSON dimension order
    for (const dim of modeCatalog.dimensions) {
        const val = toggles[dim.key];
        if (val) {
            const opt = dim.options.find(o => o.value === val);
            if (opt && opt.params) {
                params = { ...params, ...opt.params };
            }
        }
    }

    const wellPositions = computeWellPositions(params, toggles.geo, toggles.well);
    return { ...params, ...wellPositions };
}

/**
 * Builds a deterministic key for identifying the selected scenario.
 */
export function buildCaseKey(toggles: ToggleState): string {
    const mode = normalizeMode(toggles.mode);

    if (mode === 'benchmark') {
        return `bench_${toggles.benchmarkId.replace(/_/g, '-')}`;
    }

    const modeCatalog = getModeCatalog(mode);

    // Keep mode prefix stable while using mode-local dimension order.
    return [`mode-${mode}`]
        .concat(modeCatalog.dimensions
        .map(d => `${d.key}-${toggles[d.key] || d.options[0].value}`)
        )
        .join('_');
}

/**
 * Returns a map of disabled options and their tooltip reasons based on the current toggle state.
 * Structure: { [dimensionKey]: { [optionValue]: "Reason string" } }
 */
export function getDisabledOptions(toggles: ToggleState): Record<string, Record<string, string>> {
    const mode = normalizeMode(toggles.mode);
    if (mode === 'benchmark') return {};

    const modeCatalog = getModeCatalog(mode);
    const disabled: Record<string, Record<string, string>> = {};
    const dimensionDefaults = Object.fromEntries(
        modeCatalog.dimensions.map((dim) => [dim.key, dim.options[0]?.value]),
    );

    for (const rule of modeCatalog.disabilityRules) {
        // Check if `when` condition is met
        let conditionMet = true;
        for (const [key, expectedValue] of Object.entries(rule.when)) {
            const actualValue = toggles[key] || dimensionDefaults[key];
            if (Array.isArray(expectedValue)) {
                if (!expectedValue.includes(actualValue as string)) {
                    conditionMet = false;
                    break;
                }
            } else {
                if (actualValue !== expectedValue) {
                    conditionMet = false;
                    break;
                }
            }
        }

        if (conditionMet) {
            const reason = rule.reason || rule.note || 'Invalid combination';
            for (const [dimKey, optionsToDisable] of Object.entries(rule.disable)) {
                if (!disabled[dimKey]) disabled[dimKey] = {};

                if (optionsToDisable.length === 0) {
                    // Empty array means ALL options are disabled except the current selection (or just disable the whole dimension if we wanted to hide it)
                    // For now, let's treat it as a placeholder if we ever need to disable 'everything else'
                } else {
                    for (const optVal of optionsToDisable) {
                        disabled[dimKey][optVal] = reason;
                    }
                }
            }
        }
    }

    return disabled;
}

/**
 * Iteratively repairs a toggle state until no selected option is disabled.
 * This avoids one-pass repair bugs when rules cascade across dimensions.
 */
export function stabilizeToggleState(input: ToggleState): ToggleState {
    const toggles: ToggleState = { ...input };
    const modeCatalog = getModeCatalog(toggles.mode);
    const maxPasses = Math.max(1, modeCatalog.dimensions.length * 3);

    for (let pass = 0; pass < maxPasses; pass++) {
        const disabled = getDisabledOptions(toggles);
        let changed = false;

        for (const dim of modeCatalog.dimensions) {
            if (!dim.options.length) continue;

            const selected = toggles[dim.key] ?? dim.options[0].value;
            const reasonMap = disabled[dim.key] ?? {};
            const selectedIsKnown = dim.options.some((o) => o.value === selected);

            if (selectedIsKnown && !reasonMap[selected]) continue;

            const validOpt = dim.options.find((o) => !reasonMap[o.value]) ?? dim.options[0];
            if (validOpt && toggles[dim.key] !== validOpt.value) {
                toggles[dim.key] = validOpt.value;
                changed = true;
            }
        }

        if (!changed) break;
    }

    return toggles;
}

/**
 * Generate a default valid toggle state.
 */
export function getDefaultToggles(mode: CaseMode = 'dep'): ToggleState {
    const resolvedMode = normalizeMode(mode);
    const toggles: ToggleState = { mode: resolvedMode };
    for (const dim of getModeDimensions(resolvedMode)) {
        const defaultOpt = dim.options.find(o => o.default);
        toggles[dim.key] = (defaultOpt ?? dim.options[0]).value;
    }

    return stabilizeToggleState(toggles);
}
