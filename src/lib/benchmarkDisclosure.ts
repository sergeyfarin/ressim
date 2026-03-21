import type {
    BenchmarkComparisonMetric,
    BenchmarkDisplayDefaults,
    BenchmarkReferenceKind,
    BenchmarkRunPolicy,
} from './catalog/benchmarkCases';
import type { AnalyticalMethod } from './catalog/scenarios';

export type BenchmarkCaseSnapshot = {
    grid: string;
    horizon: string;
    controls: string;
    reservoir: string;
};

export type BenchmarkReferenceGuidance = {
    reference: string;
    metric: string;
    outputs: string;
    runApproach: string;
};

export type BenchmarkPrimaryVariation = {
    label: string;
    value: string;
};

const X_AXIS_LABELS = {
    time: 'time',
    pvi: 'pore volume injected',
    tD: 'dimensionless time',
} as const;

const PANEL_LABELS = {
    'watercut-breakthrough': 'breakthrough',
    recovery: 'recovery',
    pressure: 'pressure',
    rates: 'rates',
    'oil-rate': 'oil rate',
    'cumulative-oil': 'cumulative oil',
    'decline-diagnostics': 'decline diagnostics',
} as const;

const RUN_POLICY_LABELS: Record<BenchmarkRunPolicy, string> = {
    single: 'Single locked reference run',
    sweep: 'Library sensitivity run set',
    'compare-to-reference': 'Reference review run',
};

function toFiniteNumber(value: unknown): number | null {
    const numeric = Number(value);
    return Number.isFinite(numeric) ? numeric : null;
}

function formatNumber(value: number | null, digits = 2): string {
    if (!Number.isFinite(value)) return 'n/a';
    const numeric = Number(value);
    if (Math.abs(numeric - Math.round(numeric)) <= 1e-9) return String(Math.round(numeric));
    return numeric.toFixed(digits).replace(/\.0+$/, '').replace(/(\.\d*?)0+$/, '$1');
}

function prettifyKeyLabel(key: string): string {
    return key
        .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
        .replace(/_/g, ' ')
        .replace(/^./, (value) => value.toUpperCase());
}

function buildWellControlSummary(
    label: 'Injector' | 'Producer',
    enabled: boolean,
    mode: string | null,
    bhp: number | null,
    rate: number | null,
): string {
    if (!enabled) return `${label} off`;
    if (mode === 'rate' && Number.isFinite(rate)) {
        return `${label} rate ${formatNumber(rate, 0)} m3/d`;
    }
    if (Number.isFinite(bhp)) {
        return `${label} BHP ${formatNumber(bhp, 0)} bar`;
    }
    return `${label} ${mode ?? 'control set'}`;
}

function buildPermeabilitySummary(params: Record<string, any>): string {
    const mode = String(params.permMode ?? 'uniform');

    if (mode === 'random') {
        const minPerm = toFiniteNumber(params.minPerm);
        const maxPerm = toFiniteNumber(params.maxPerm);
        if (Number.isFinite(minPerm) && Number.isFinite(maxPerm)) {
            return `random ${formatNumber(minPerm, 0)}-${formatNumber(maxPerm, 0)} mD`;
        }
        return 'random permeability';
    }

    const permX = toFiniteNumber(params.uniformPermX);
    const permY = toFiniteNumber(params.uniformPermY ?? params.uniformPermX);
    const permZ = toFiniteNumber(params.uniformPermZ ?? params.uniformPermX);
    if (Number.isFinite(permX) && Number.isFinite(permY) && Number.isFinite(permZ)) {
        if (Math.abs((permX as number) - (permY as number)) <= 1e-9 && Math.abs((permX as number) - (permZ as number)) <= 1e-9) {
            return `uniform ${formatNumber(permX, 0)} mD`;
        }
        return `uniform ${formatNumber(permX, 0)}/${formatNumber(permY, 0)}/${formatNumber(permZ, 0)} mD`;
    }

    return `${mode} permeability`;
}

function buildReferenceLabel(
    analyticalMethod: AnalyticalMethod,
    referenceKind: BenchmarkReferenceKind,
): string {
    if (analyticalMethod === 'buckley-leverett' && referenceKind === 'numerical-refined') {
        return 'Refined numerical reference';
    }
    if (analyticalMethod === 'buckley-leverett') {
        return 'Buckley-Leverett reference solution';
    }
    if (analyticalMethod === 'gas-oil-bl') {
        return 'Gas-oil Buckley-Leverett reference solution';
    }
    return 'Depletion reference solution';
}

export function buildBenchmarkCaseSnapshot(params: Record<string, any>): BenchmarkCaseSnapshot {
    const nx = toFiniteNumber(params.nx) ?? 1;
    const ny = toFiniteNumber(params.ny) ?? 1;
    const nz = toFiniteNumber(params.nz) ?? 1;
    const cellDx = toFiniteNumber(params.cellDx) ?? 10;
    const cellDy = toFiniteNumber(params.cellDy) ?? 10;
    const cellDz = toFiniteNumber(params.cellDz) ?? 1;
    const steps = toFiniteNumber(params.steps) ?? 1;
    const deltaTDays = toFiniteNumber(params.delta_t_days) ?? 1;
    const totalDays = (steps as number) * (deltaTDays as number);
    const porosity = toFiniteNumber(params.reservoirPorosity ?? params.porosity) ?? 0.2;
    const muW = toFiniteNumber(params.mu_w) ?? 0.5;
    const muO = toFiniteNumber(params.mu_o) ?? 1;

    return {
        grid: `${formatNumber(nx, 0)} x ${formatNumber(ny, 0)} x ${formatNumber(nz, 0)} cells · ${formatNumber(cellDx)} x ${formatNumber(cellDy)} x ${formatNumber(cellDz)} m`,
        horizon: `${formatNumber(steps, 0)} steps × ${formatNumber(deltaTDays)} d = ${formatNumber(totalDays)} d`,
        controls: [
            buildWellControlSummary(
                'Injector',
                Boolean(params.injectorEnabled ?? true),
                params.injectorControlMode ? String(params.injectorControlMode) : null,
                toFiniteNumber(params.injectorBhp),
                toFiniteNumber(params.targetInjectorRate),
            ),
            buildWellControlSummary(
                'Producer',
                true,
                params.producerControlMode ? String(params.producerControlMode) : null,
                toFiniteNumber(params.producerBhp),
                toFiniteNumber(params.targetProducerRate),
            ),
        ].join(' · '),
        reservoir: `φ ${formatNumber(porosity)} · k ${buildPermeabilitySummary(params)} · μw/μo ${formatNumber(muW)}/${formatNumber(muO)}`,
    };
}

export function buildBenchmarkReferenceGuidance(input: {
    analyticalMethod: AnalyticalMethod;
    referenceKind: BenchmarkReferenceKind;
    comparisonMetric: BenchmarkComparisonMetric | null;
    displayDefaults: BenchmarkDisplayDefaults | null;
    runPolicy: BenchmarkRunPolicy | null;
}): BenchmarkReferenceGuidance {
    const reference = `Reference solution: ${buildReferenceLabel(input.analyticalMethod, input.referenceKind)}.`;
    const metric = input.comparisonMetric
        ? `Review metric: arrival-PVI relative error against ${input.comparisonMetric.target === 'analytical-reference' ? 'the reference solution' : 'the refined numerical reference'} (${formatNumber(input.comparisonMetric.tolerance * 100, 1)}% tolerance).`
        : 'Review metric: trend comparison against the reference solution.';
    const outputs = input.displayDefaults
        ? `Default outputs: ${X_AXIS_LABELS[input.displayDefaults.xAxis]} x-axis · ${input.displayDefaults.panels.map((panel) => PANEL_LABELS[panel]).join(', ')}.`
        : 'Default outputs: follow the active family chart layout.';
    const runApproach = `Run approach: ${input.runPolicy ? RUN_POLICY_LABELS[input.runPolicy] : 'Reference review run'}.`;

    return {
        reference,
        metric,
        outputs,
        runApproach,
    };
}

export function buildBenchmarkPrimaryVariation(paramsDelta: Record<string, any>): BenchmarkPrimaryVariation {
    const keys = Object.keys(paramsDelta);
    if (keys.length === 0) {
        return {
            label: 'Base case',
            value: 'Locked reference case',
        };
    }

    const nx = toFiniteNumber(paramsDelta.nx);
    const ny = toFiniteNumber(paramsDelta.ny);
    const nz = toFiniteNumber(paramsDelta.nz);
    const deltaTDays = toFiniteNumber(paramsDelta.delta_t_days);
    const minPerm = toFiniteNumber(paramsDelta.minPerm);
    const maxPerm = toFiniteNumber(paramsDelta.maxPerm);
    const randomSeed = toFiniteNumber(paramsDelta.randomSeed);
    const uniformPermX = toFiniteNumber(paramsDelta.uniformPermX);
    const uniformPermY = toFiniteNumber(paramsDelta.uniformPermY ?? paramsDelta.uniformPermX);
    const uniformPermZ = toFiniteNumber(paramsDelta.uniformPermZ ?? paramsDelta.uniformPermX);
    const producerI = toFiniteNumber(paramsDelta.producerI);
    const producerJ = toFiniteNumber(paramsDelta.producerJ);
    const injectorI = toFiniteNumber(paramsDelta.injectorI);
    const injectorJ = toFiniteNumber(paramsDelta.injectorJ);

    if (Number.isFinite(deltaTDays)) {
        return {
            label: 'dt',
            value: `${formatNumber(deltaTDays)} d`,
        };
    }

    if (Number.isFinite(nx) || Number.isFinite(ny) || Number.isFinite(nz)) {
        const parts = [
            Number.isFinite(nx) ? `nx ${formatNumber(nx, 0)}` : null,
            Number.isFinite(ny) ? `ny ${formatNumber(ny, 0)}` : null,
            Number.isFinite(nz) ? `nz ${formatNumber(nz, 0)}` : null,
        ].filter(Boolean);

        return {
            label: 'Grid',
            value: parts.join(' · '),
        };
    }

    if (String(paramsDelta.permMode ?? '') === 'random' || Number.isFinite(minPerm) || Number.isFinite(maxPerm)) {
        return {
            label: 'k range',
            value: Number.isFinite(minPerm) && Number.isFinite(maxPerm)
                ? `${formatNumber(minPerm, 0)}-${formatNumber(maxPerm, 0)} mD`
                : Number.isFinite(randomSeed)
                    ? `seed ${formatNumber(randomSeed, 0)}`
                    : 'Random permeability',
        };
    }

    if (Number.isFinite(uniformPermX) || Number.isFinite(uniformPermY) || Number.isFinite(uniformPermZ)) {
        const permParts = [uniformPermX, uniformPermY, uniformPermZ]
            .filter((value) => Number.isFinite(value))
            .map((value) => formatNumber(value, 0));

        return {
            label: 'Permeability',
            value: `${permParts.join('/')} mD`,
        };
    }

    if (Number.isFinite(producerI) || Number.isFinite(producerJ) || Number.isFinite(injectorI) || Number.isFinite(injectorJ)) {
        const producer = [
            Number.isFinite(producerI) ? `i=${formatNumber(producerI, 0)}` : null,
            Number.isFinite(producerJ) ? `j=${formatNumber(producerJ, 0)}` : null,
        ].filter(Boolean).join(', ');
        const injector = [
            Number.isFinite(injectorI) ? `i=${formatNumber(injectorI, 0)}` : null,
            Number.isFinite(injectorJ) ? `j=${formatNumber(injectorJ, 0)}` : null,
        ].filter(Boolean).join(', ');

        return {
            label: 'Well index',
            value: [
                producer ? `P ${producer}` : null,
                injector ? `I ${injector}` : null,
            ].filter(Boolean).join(' · '),
        };
    }

    const firstKey = keys[0];
    const firstValue = paramsDelta[firstKey];
    return {
        label: prettifyKeyLabel(firstKey),
        value: Array.isArray(firstValue)
            ? `${firstValue.length} values`
            : typeof firstValue === 'boolean'
                ? (firstValue ? 'On' : 'Off')
                : String(firstValue),
    };
}

export function deriveBenchmarkParamsDelta(
    baseParams: Record<string, any>,
    nextParams: Record<string, any>,
): Record<string, any> {
    const delta: Record<string, any> = {};
    const keys = new Set([...Object.keys(baseParams), ...Object.keys(nextParams)]);
    for (const key of keys) {
        if (JSON.stringify(baseParams[key]) !== JSON.stringify(nextParams[key])) {
            delta[key] = nextParams[key];
        }
    }
    return delta;
}

export function buildBenchmarkVariantDeltaSummary(paramsDelta: Record<string, any>): string {
    const parts: string[] = [];
    const nx = toFiniteNumber(paramsDelta.nx);
    const ny = toFiniteNumber(paramsDelta.ny);
    const nz = toFiniteNumber(paramsDelta.nz);
    const deltaTDays = toFiniteNumber(paramsDelta.delta_t_days);
    const steps = toFiniteNumber(paramsDelta.steps);
    const minPerm = toFiniteNumber(paramsDelta.minPerm);
    const maxPerm = toFiniteNumber(paramsDelta.maxPerm);
    const randomSeed = toFiniteNumber(paramsDelta.randomSeed);
    const producerI = toFiniteNumber(paramsDelta.producerI);
    const producerJ = toFiniteNumber(paramsDelta.producerJ);
    const injectorI = toFiniteNumber(paramsDelta.injectorI);
    const injectorJ = toFiniteNumber(paramsDelta.injectorJ);

    if (Number.isFinite(nx) || Number.isFinite(ny) || Number.isFinite(nz)) {
        const gridParts = [
            Number.isFinite(nx) ? `nx=${formatNumber(nx, 0)}` : null,
            Number.isFinite(ny) ? `ny=${formatNumber(ny, 0)}` : null,
            Number.isFinite(nz) ? `nz=${formatNumber(nz, 0)}` : null,
        ].filter(Boolean);
        if (gridParts.length > 0) parts.push(`Grid ${gridParts.join(', ')}`);
    }

    if (Number.isFinite(deltaTDays) || Number.isFinite(steps)) {
        const timestepParts = [
            Number.isFinite(deltaTDays) ? `${formatNumber(deltaTDays)} d` : null,
            Number.isFinite(steps) ? `${formatNumber(steps, 0)} steps` : null,
        ].filter(Boolean);
        if (timestepParts.length > 0) parts.push(`Timestep ${timestepParts.join(' · ')}`);
    }

    if (String(paramsDelta.permMode ?? '') === 'random' || Number.isFinite(minPerm) || Number.isFinite(maxPerm)) {
        const range = Number.isFinite(minPerm) && Number.isFinite(maxPerm)
            ? `${formatNumber(minPerm, 0)}-${formatNumber(maxPerm, 0)} mD`
            : 'random permeability';
        const seed = Number.isFinite(randomSeed) ? `seed ${formatNumber(randomSeed, 0)}` : null;
        parts.push(`Permeability ${range}${seed ? ` · ${seed}` : ''}`);
    }

    const wellParts = [
        Number.isFinite(producerI) || Number.isFinite(producerJ)
            ? `Producer ${[
                Number.isFinite(producerI) ? `i=${formatNumber(producerI, 0)}` : null,
                Number.isFinite(producerJ) ? `j=${formatNumber(producerJ, 0)}` : null,
            ].filter(Boolean).join(', ')}`
            : null,
        Number.isFinite(injectorI) || Number.isFinite(injectorJ)
            ? `Injector ${[
                Number.isFinite(injectorI) ? `i=${formatNumber(injectorI, 0)}` : null,
                Number.isFinite(injectorJ) ? `j=${formatNumber(injectorJ, 0)}` : null,
            ].filter(Boolean).join(', ')}`
            : null,
    ].filter(Boolean);
    if (wellParts.length > 0) parts.push(wellParts.join(' · '));

    if (parts.length === 0) {
        const keys = Object.keys(paramsDelta);
        if (keys.length === 0) return 'No change from base case.';
        return `Overrides ${keys.slice(0, 3).join(', ')}`;
    }

    return parts.slice(0, 3).join(' · ');
}