import catalogDataRaw from '../../public/cases/catalog.json';

// --- Type Definitions ---
export type CaseMode = 'dep' | 'wf' | 'sim' | 'benchmark';

export type DimensionOption = {
    value: string;
    label: string;
    params?: Record<string, any>;
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

export type CatalogSchema = {
    version: number;
    defaults: Record<string, any>;
    dimensions: Dimension[];
    disabilityRules: DisabilityRule[];
    benchmarks: BenchmarkEntry[];
};

export const catalog: CatalogSchema = catalogDataRaw as unknown as CatalogSchema;

export type ToggleState = Record<string, string>;

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
    if (toggles.mode === 'benchmark') {
        const bench = catalog.benchmarks.find(b => b.key === toggles.benchmarkId);
        if (bench) return { ...catalog.defaults, ...bench.params };
        return catalog.defaults;
    }

    let params = { ...catalog.defaults };

    // Overlay order strictly follows JSON dimension order
    for (const dim of catalog.dimensions) {
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
 * Builds the deterministic key used to find the pre-run file.
 */
export function buildCaseKey(toggles: ToggleState): string {
    if (toggles.mode === 'benchmark') {
        return `bench_${toggles.benchmarkId.replace(/_/g, '-')}`;
    }

    // Always use the dimensions in the order defined in catalog
    const parts = catalog.dimensions.map(d => `${d.key}-${toggles[d.key] || d.options[0].value}`);
    return parts.join('_');
}

/**
 * Returns a map of disabled options and their tooltip reasons based on the current toggle state.
 * Structure: { [dimensionKey]: { [optionValue]: "Reason string" } }
 */
export function getDisabledOptions(toggles: ToggleState): Record<string, Record<string, string>> {
    const disabled: Record<string, Record<string, string>> = {};

    for (const rule of catalog.disabilityRules) {
        // Check if `when` condition is met
        let conditionMet = true;
        for (const [key, expectedValue] of Object.entries(rule.when)) {
            const actualValue = toggles[key] || catalog.dimensions.find(d => d.key === key)?.options[0].value;
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
 * Generate a default valid toggle state.
 */
export function getDefaultToggles(mode: CaseMode = 'dep'): ToggleState {
    const toggles: ToggleState = {};
    for (const dim of catalog.dimensions) {
        if (dim.key === 'mode') {
            toggles[dim.key] = mode;
        } else {
            toggles[dim.key] = dim.options[0].value;
        }
    }

    // Attempt to clear disabled violations on initial fallback
    const disabledList = getDisabledOptions(toggles);
    for (const [dimKey, reasonMap] of Object.entries(disabledList)) {
        if (reasonMap[toggles[dimKey]]) {
            const dim = catalog.dimensions.find(d => d.key === dimKey);
            if (dim) {
                const validOpt = dim.options.find(o => !reasonMap[o.value]);
                if (validOpt) toggles[dimKey] = validOpt.value;
            }
        }
    }

    return toggles;
}
