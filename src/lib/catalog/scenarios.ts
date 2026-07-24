/**
 * scenarios.ts — single source of truth for all predefined simulation scenarios.
 *
 * This barrel file re-exports shared types, chart presets, and lookup helpers.
 * Individual scenario definitions live in ./scenarios/<key>.ts for easy
 * side-by-side comparison and independent editing.
 *
 * Key naming convention:
 *   {domain}_{physics_descriptor}
 *   e.g. wf_bl1d, sweep_areal, dep_pss, dep_decline
 *
 * Sensitivity dimension keys: lower_snake of parameter name (mobility, corey_no, sor, …)
 * Sensitivity variant keys:   {dim_abbrev}_{value_tag} (e.g. mob_favorable, sor_low)
 */

import type { RateChartLayoutConfig } from '../charts/rateChartLayoutConfig';
import type { SweepAnalyticalMethod, SweepGeometry } from '../analytical/sweepEfficiency';
import type { RateHistoryPoint } from '../simulator-types';
import { getChartLayout, mergeChartLayoutConfig } from './chartLayouts';

export { CHART_LAYOUTS, getChartLayout, mergeChartLayoutConfig } from './chartLayouts';

// ─── Per-scenario imports ────────────────────────────────────────────────────

import { wf_bl1d } from './scenarios/wf_bl1d';
import { wf_bl1d_opm } from './scenarios/wf_bl1d_opm';
import { wf_tornado } from './scenarios/wf_tornado';
import { wf_capillary } from './scenarios/wf_capillary';
import { sweep_areal } from './scenarios/sweep_areal';
import { sweep_vertical } from './scenarios/sweep_vertical';
import { sweep_combined } from './scenarios/sweep_combined';
import { dep_pss } from './scenarios/dep_pss';
import { dep_arps } from './scenarios/dep_arps';
import { dep_decline } from './scenarios/dep_decline';
import { dep_nct } from './scenarios/dep_nct';
import { dep_pvt } from './scenarios/dep_pvt';
import { gas_injection } from './scenarios/gas_injection';
import { gas_drive } from './scenarios/gas_drive';
import { spe1_gas_injection } from './scenarios/spe1_gas_injection';

// ─── Types ────────────────────────────────────────────────────────────────────

/** Which analytical reference model to compute for overlay curves. */
export type AnalyticalMethod =
    | 'buckley-leverett'
    | 'gas-oil-bl'
    | 'depletion'
    | 'digitized-reference'
    | 'none';

/** Coarse analytical family used by the custom/editor UI. */
export type AnalyticalMode = 'waterflood' | 'depletion' | 'none';

/** Presentational scenario grouping used by ScenarioPicker. */
export type ScenarioGroup = 'waterflood' | 'sweep' | 'depletion' | 'gas';

/** Which primary rate curve to show in the "rates" chart panel. */
export type PrimaryRateCurve = 'water-cut' | 'gas-cut' | 'oil-rate';

/** How analytical overlays should be grouped for a selected sensitivity dimension. */
export type AnalyticalOverlayMode = 'auto' | 'shared' | 'per-result';

/** Numerical formulation used for a live simulator run. */
export type SimulationSolver = 'impes' | 'fim';

/** Catalog-owned solver choice and the reason exposed to the frontend. */
export type ScenarioSolverPolicy = {
    defaultSolver: SimulationSolver;
    rationale: string;
    comparisonSensitivityAvailable: boolean;
};

export type ScenarioAnalyticalOption = {
    key: string;
    label: string;
    summary: string;
    reference: string;
    sweepMethod?: SweepAnalyticalMethod;
    default?: boolean;
};

/** Default 3D scalar property to display when a scenario loads. */
export type Default3DScalar = 'saturation_water' | 'saturation_gas' | null;

// ─── Analytical output contract ──────────────────────────────────────────────

/** What a given analytical method produces and its display defaults. */
export type AnalyticalOutputContract = {
    /** Curve outputs this method can produce. */
    produces: readonly string[];
    /** Which primary rate curves are valid for this method. */
    supportedRateCurves: readonly PrimaryRateCurve[];
    /** Native x-axis of the analytical solution. */
    nativeXAxis: 'pvi' | 'time';
    /** Default primary rate curve. */
    defaultPrimaryRateCurve: PrimaryRateCurve;
    /** Whether depletion tau (tD axis) is meaningful. */
    hasTau: boolean;
    /** Default panel expansion state for the RateChart. */
    defaultPanelExpansion: {
        rates: boolean;
        recovery: boolean;
        cumulative: boolean;
        diagnostics: boolean;
    };
};

/**
 * Each analytical method declares its output contract — what it produces and
 * how results should be displayed by default. Scenarios inherit these defaults
 * and only override what they need.
 */
export const ANALYTICAL_OUTPUT_CONTRACTS: Record<AnalyticalMethod, AnalyticalOutputContract> = {
    'buckley-leverett': {
        produces: ['water-cut', 'recovery', 'cum-oil'],
        supportedRateCurves: ['water-cut'],
        nativeXAxis: 'pvi',
        defaultPrimaryRateCurve: 'water-cut',
        hasTau: false,
        defaultPanelExpansion: { rates: true, recovery: true, cumulative: false, diagnostics: false },
    },
    'gas-oil-bl': {
        produces: ['gas-cut', 'recovery', 'cum-oil'],
        supportedRateCurves: ['gas-cut'],
        nativeXAxis: 'pvi',
        defaultPrimaryRateCurve: 'gas-cut',
        hasTau: false,
        defaultPanelExpansion: { rates: true, recovery: true, cumulative: false, diagnostics: false },
    },
    'depletion': {
        produces: ['oil-rate', 'recovery', 'cum-oil', 'pressure'],
        supportedRateCurves: ['oil-rate'],
        nativeXAxis: 'time',
        defaultPrimaryRateCurve: 'oil-rate',
        hasTau: true,
        defaultPanelExpansion: { rates: true, recovery: true, cumulative: false, diagnostics: true },
    },
    'digitized-reference': {
        produces: [],
        supportedRateCurves: ['water-cut', 'gas-cut', 'oil-rate'],
        nativeXAxis: 'time',
        defaultPrimaryRateCurve: 'oil-rate',
        hasTau: false,
        defaultPanelExpansion: { rates: true, recovery: true, cumulative: false, diagnostics: false },
    },
    'none': {
        produces: [],
        supportedRateCurves: ['water-cut', 'gas-cut', 'oil-rate'],
        nativeXAxis: 'time',
        defaultPrimaryRateCurve: 'oil-rate',
        hasTau: false,
        defaultPanelExpansion: { rates: true, recovery: true, cumulative: false, diagnostics: false },
    },
};

// ─── Analytical def — scenario-owned computation ─────────────────────────────

/** Unified point type covering waterflood, depletion, and gas-oil outputs. */
export type ScenarioAnalyticalPoint = {
    time: number;
    oilRate: number;
    waterRate?: number;
    gasRate?: number;
    cumulativeOil: number;
    avgPressure?: number;
};

export type ScenarioAnalyticalMeta = {
    mode: 'waterflood' | 'depletion' | 'gas-oil-bl' | 'none';
    shapeFactor: number | null;
    shapeLabel: string;
    q0?: number;
    tau?: number;
    arpsB?: number;
};

export type ScenarioAnalyticalOutput = {
    production: ScenarioAnalyticalPoint[];
    meta: ScenarioAnalyticalMeta;
};

/**
 * Encapsulates how a scenario computes its analytical overlay.
 * The fn/inputsFromParams split keeps pure calculation separate from
 * param-extraction, enabling both live (App.svelte) and benchmark
 * (referenceComparisonModel / future buildChartData) call sites.
 */
export type ScenarioAnalyticalDef = {
    /** Pure analytical calculation — call with the output of inputsFromParams. */
    fn: (inputs: unknown) => ScenarioAnalyticalOutput;
    /** Assemble analytical inputs from scenario params and rate history. */
    inputsFromParams: (params: Record<string, unknown>, rateHistory: RateHistoryPoint[]) => unknown;
};

// ─── Scenario capabilities ───────────────────────────────────────────────────

/**
 * Scenario capability declarations — the single source of truth for all
 * behavioral routing. Fields derivable from `analyticalMethod` are optional
 * overrides; omitted fields inherit from ANALYTICAL_OUTPUT_CONTRACTS.
 *
 * Consumer code reads resolved capabilities via `resolveCapabilities()`.
 */
export type ScenarioCapabilities = {
    /** Which analytical reference model to use — the primary routing key. */
    analyticalMethod: AnalyticalMethod;
    /** Override the default primary rate curve for this analytical method. */
    primaryRateCurve?: PrimaryRateCurve;
    /** Override the native x-axis for this analytical method. */
    analyticalNativeXAxis?: 'pvi' | 'time';
    /** Override whether tau is meaningful for this analytical method. */
    hasTauDimensionlessTime?: boolean;
    /** Whether the sweep efficiency panel (E_A, E_V, E_vol) should be shown. */
    showSweepPanel: boolean;
    /** Scenario-defined sweep decomposition geometry. Drives panel visibility and semantics. */
    sweepGeometry?: SweepGeometry;
    /** Whether the scenario includes an active injector. */
    hasInjector: boolean;
    /** Default 3D scalar to show on load. */
    default3DScalar: Default3DScalar;
    /** Whether the gas domain tab gate applies (scenario only visible in 3-phase mode). */
    requiresThreePhaseMode: boolean;
    /**
     * How this scenario produces results. 'live-worker' (default) runs the WASM
     * simulator; 'prerun-artifacts' ships entirely precomputed — no worker run,
     * variants map to bundled artifact keys, 3D off. Foundation for Tier-6 exhibits.
     */
    runMode?: 'live-worker' | 'prerun-artifacts';
};

/** Fully resolved capabilities — all fields guaranteed present. */
export type ResolvedCapabilities = {
    analyticalMethod: AnalyticalMethod;
    primaryRateCurve: PrimaryRateCurve;
    analyticalNativeXAxis: 'pvi' | 'time';
    hasTauDimensionlessTime: boolean;
    showSweepPanel: boolean;
    sweepGeometry: SweepGeometry | null;
    hasInjector: boolean;
    default3DScalar: Default3DScalar;
    requiresThreePhaseMode: boolean;
    runMode: 'live-worker' | 'prerun-artifacts';
    /** Panel expansion defaults from the analytical output contract. */
    defaultPanelExpansion: AnalyticalOutputContract['defaultPanelExpansion'];
};

/** Merge analytical method defaults with scenario overrides. */
export function resolveCapabilities(caps: ScenarioCapabilities): ResolvedCapabilities {
    const contract = ANALYTICAL_OUTPUT_CONTRACTS[caps.analyticalMethod];
    return {
        analyticalMethod: caps.analyticalMethod,
        primaryRateCurve: caps.primaryRateCurve ?? contract.defaultPrimaryRateCurve,
        analyticalNativeXAxis: caps.analyticalNativeXAxis ?? contract.nativeXAxis,
        hasTauDimensionlessTime: caps.hasTauDimensionlessTime ?? contract.hasTau,
        showSweepPanel: caps.showSweepPanel,
        sweepGeometry: caps.showSweepPanel ? (caps.sweepGeometry ?? 'both') : null,
        hasInjector: caps.hasInjector,
        default3DScalar: caps.default3DScalar,
        requiresThreePhaseMode: caps.requiresThreePhaseMode,
        runMode: caps.runMode ?? 'live-worker',
        defaultPanelExpansion: contract.defaultPanelExpansion,
    };
}

/**
 * Validate that a scenario's capabilities are consistent with the analytical
 * method's output contract. Returns an array of error strings (empty = valid).
 */
export function validateScenarioCapabilities(caps: ScenarioCapabilities): string[] {
    const contract = ANALYTICAL_OUTPUT_CONTRACTS[caps.analyticalMethod];
    const errors: string[] = [];
    const effectiveRateCurve = caps.primaryRateCurve ?? contract.defaultPrimaryRateCurve;
    if (!contract.supportedRateCurves.includes(effectiveRateCurve)) {
        errors.push(
            `analyticalMethod '${caps.analyticalMethod}' does not support primaryRateCurve '${effectiveRateCurve}' `
            + `(supported: ${contract.supportedRateCurves.join(', ')})`,
        );
    }
    if (caps.showSweepPanel && !caps.sweepGeometry) {
        errors.push('showSweepPanel scenarios must declare sweepGeometry.');
    }
    if (!caps.showSweepPanel && caps.sweepGeometry) {
        errors.push('sweepGeometry can only be set when showSweepPanel is true.');
    }
    if (caps.runMode === 'prerun-artifacts' && caps.default3DScalar !== null) {
        errors.push("prerun-artifacts scenarios must set default3DScalar to null (3D view is off).");
    }
    return errors;
}

export type SensitivityVariant = {
    key: string;
    label: string;
    description: string;
    /** Parameters merged on top of the scenario base params for this variant. */
    paramPatch: Record<string, unknown>;
    /**
     * True  → this variant changes a parameter that feeds the analytical solution
     *         (e.g. mu_o changes fractional flow → both sim and analytical update).
     * False → the analytical solution is independent of this parameter; only the
     *         simulation result changes (e.g. grid refinement, layer heterogeneity).
     */
    affectsAnalytical: boolean;
    /**
     * Whether this variant is pre-selected when the dimension loads.
     * Omit or set true for the normal case; set false to make a variant available
     * but not run by default (useful for extreme or slow cases in large dimensions).
     */
    enabledByDefault?: boolean;
};

export type SensitivityDimension = {
    key: string;
    label: string;
    description: string;
    variants: SensitivityVariant[];
    /**
     * How comparison-chart analytical overlays should be grouped when this
     * dimension is active. `auto` falls back to physics-signature inference;
     * explicit modes are preferred for scenario-defined sensitivity studies.
     */
    analyticalOverlayMode?: AnalyticalOverlayMode;
    /**
     * Override the scenario's chartLayoutKey when this dimension is active.
     * Useful when a dimension (e.g. grid convergence) benefits from a different
     * default view than the scenario's primary chart.
     */
    chartLayoutKeyOverride?: string;
    /** Patch applied on top of the resolved shared layout for this dimension. */
    chartLayoutPatchOverride?: RateChartLayoutConfig;
};

/**
 * A single static reference data series from a published benchmark.
 * Used to overlay published simulator results (e.g. Eclipse SPE1) on charts.
 */
export type PublishedReferenceSeries = {
    /** Explicit source category for reference/comparison curves. */
    sourceType?: import('./opmFlowArtifacts').ReferenceSourceType;
    /** Optional artifact key when this series comes from bundled precomputed data. */
    sourceArtifactKey?: string;
    /** Which chart panel this series appears in (e.g. 'diagnostics', 'rates', 'oil_rate'). */
    panelKey: string;
    /** Display label in the legend (e.g. 'Eclipse — Avg Pressure'). */
    label: string;
    /** Curve key for toggle grouping (e.g. 'published-pressure'). */
    curveKey: string;
    /** Static data points — x is time in days, y is the metric value. */
    data: { x: number; y: number }[];
    /** Chart.js y-axis ID (e.g. 'y' for primary, 'y1' for secondary). */
    yAxisID?: string;
    /**
     * When true, render as solid primary content instead of a dashed reference
     * overlay — used by prerun-artifacts scenarios whose entire content IS the
     * bundled artifact (there is no live simulation curve to compare against).
     */
    primary?: boolean;
};

export type ScenarioTerminationCondition =
    | {
        kind: 'watercut-threshold';
        /** Water cut threshold in fractional units, e.g. 0.01 for 1%. */
        value: number;
        /** Which producer scope should be evaluated. */
        scope?: 'producer' | 'any-producer';
    }
    | {
        kind: 'phase-rate-threshold';
        /** Phase rate to monitor. */
        phase: 'oil' | 'water' | 'gas';
        /** Compare using <= or >=. Use <= 0 for "drops to zero" conditions. */
        relation: 'lte' | 'gte';
        /** Threshold in surface-rate units. */
        value: number;
        /** Which well scope should be evaluated. */
        scope?: 'producer' | 'injector' | 'any';
    }
    | {
        kind: 'gor-threshold';
        /** Compare using <= or >=. Use gte for "GOR exceeds" conditions. */
        relation: 'lte' | 'gte';
        /** Threshold in Sm^3/Sm^3. */
        value: number;
        /** Which producer scope should be evaluated. */
        scope?: 'producer' | 'any';
    };

export type ScenarioTerminationPolicy = {
    /** Whether any one condition or all conditions must be met to stop the run. */
    mode: 'any' | 'all';
    conditions: ScenarioTerminationCondition[];
};

/**
 * Optional "history / forecast" split marker for the comparison chart. Renders
 * a shaded history region up to `boundary` plus a divider line, so match-then-
 * forecast cases (e.g. dep_nct now, Tavassoli/PUNQ-S3 later) can visually
 * separate the observed-history window from the extrapolated forecast.
 */
export type HistoryWindow = {
    /** X-axis value (in `axis` units) where matched history ends and forecast begins. */
    boundary: number;
    /**
     * Which x-axis this boundary is expressed in. The divider only renders when
     * the chart's active x-axis matches. Defaults to 'time'.
     */
    axis?: 'time' | 'pvi';
    /** Label drawn in the history (shaded) region. Defaults to 'History'. */
    historyLabel?: string;
    /** Label drawn in the forecast region. Defaults to 'Forecast'. */
    forecastLabel?: string;
};

export type Scenario = {
    key: string;
    label: string;
    description: string;
    analyticalMethodSummary: string;
    analyticalMethodReference: string;
    analyticalOptions?: ScenarioAnalyticalOption[];
    /** Complete, self-contained simulator parameter set. No shared base objects. */
    params: Record<string, unknown>;
    /** Key into CHART_LAYOUTS — selects the shared chart layout template for this scenario. */
    chartLayoutKey: string;
    /** Scenario-local tweaks applied on top of the shared chart layout. */
    chartLayoutPatch?: RateChartLayoutConfig;
    /** Behavioral capability declarations — single source of truth for all routing logic. */
    capabilities: ScenarioCapabilities;
    /**
     * Sensitivity dimensions available for this scenario.
     * Empty array = no sensitivity study defined.
     * First element is the default dimension shown on load unless
     * defaultSensitivityDimensionKey is set.
     */
    sensitivities: SensitivityDimension[];
    /**
     * Key of the dimension to activate when the scenario is first selected.
     * Defaults to sensitivities[0].key if omitted.
     */
    defaultSensitivityDimensionKey?: string;
    /**
     * Static reference data from published benchmarks (e.g. Eclipse SPE1 results).
     * Overlaid on charts as dashed reference curves alongside simulation output.
     */
    publishedReferenceSeries?: PublishedReferenceSeries[];
    /** Keys for offline OPM Flow artifacts that can provide precomputed reference curves. */
    opmFlowReferenceArtifactKeys?: string[];
    /** Optional stop policy for terminating a run when a production condition is met. */
    terminationPolicy?: ScenarioTerminationPolicy;
    /** Optional history/forecast divider marker for the comparison chart. */
    historyWindow?: HistoryWindow;
    /**
     * Scenario-owned analytical computation. When present, App.svelte and
     * chart builders call this instead of string-routing on analyticalMode/Method.
     * Absent for 'none' and 'digitized-reference' analytical methods.
     */
    analyticalDef?: ScenarioAnalyticalDef;
    /**
     * Live-chart panel definitions — exactly which panels and curves to show
     * in the single-run rate chart. Declares curveType (simulation / analytical /
     * reference / reference-simulation), color, and getData callback per curve.
     * When absent, UniversalChart falls back to a generic default panel set.
     */
    liveChartPanels?: import('../charts/universalChartTypes').UniversalPanelDef[];
};

/** Scenario after catalog-level solver policy has been applied. */
export type CatalogScenario = Scenario & {
    solverPolicy: ScenarioSolverPolicy;
};

// Scenario-first product vocabulary. The older Scenario/Sensitivity names
// remain exported while migration continues, but new frontend code should
// prefer these aliases.
export type ScenarioDefinition = Scenario;
export type ScenarioCaseParams = Record<string, unknown>;
export type ScenarioSensitivityDimension = SensitivityDimension;
export type ScenarioVariant = SensitivityVariant;
export type ScenarioChartDefinition = Pick<Scenario, 'chartLayoutKey' | 'chartLayoutPatch' | 'liveChartPanels'>;
export type { ScenarioReferenceSource, ScenarioRunPolicy } from '../scenario/runModel';

/** Default capabilities for custom mode (no predefined scenario). */
export const CUSTOM_MODE_CAPABILITIES: ScenarioCapabilities = {
    analyticalMethod: 'none',
    showSweepPanel: false,
    sweepGeometry: undefined,
    hasInjector: true,
    default3DScalar: null,
    requiresThreePhaseMode: false,
};

// ─── Shared chart layouts live in ./chartLayouts.ts ─────────────────────────

// ─── Scenarios ────────────────────────────────────────────────────────────────

const SOURCE_SCENARIOS: Scenario[] = [
    wf_bl1d,
    wf_bl1d_opm,
    wf_tornado,
    wf_capillary,
    sweep_areal,
    sweep_vertical,
    sweep_combined,
    dep_pss,
    dep_decline,
    dep_arps,
    dep_nct,
    dep_pvt,
    gas_injection,
    gas_drive,
    spe1_gas_injection,
];

function solverComparisonSensitivity(defaultSolver: SimulationSolver): SensitivityDimension {
    const orderedSolvers: SimulationSolver[] = defaultSolver === 'impes'
        ? ['impes', 'fim']
        : ['fim', 'impes'];
    return {
        key: 'solver_comparison',
        label: 'FIM vs. IMPES',
        description: 'Run the same oil/water case with both numerical formulations. Physics inputs, grid, wells, timestep, and analytical reference stay fixed.',
        analyticalOverlayMode: 'shared',
        variants: orderedSolvers.map((solver) => ({
            key: `solver_${solver}`,
            label: solver.toUpperCase(),
            description: solver === 'fim'
                ? 'Fully implicit coupled pressure/saturation Newton solve.'
                : 'Implicit pressure with explicit saturation transport.',
            paramPatch: { fimEnabled: solver === 'fim' },
            affectsAnalytical: false,
        })),
    };
}

function applySolverPolicy(source: Scenario): CatalogScenario {
    const involvesGas = source.capabilities.requiresThreePhaseMode;
    const defaultSolver: SimulationSolver = involvesGas ? 'fim' : 'impes';
    const solverPolicy: ScenarioSolverPolicy = {
        defaultSolver,
        rationale: involvesGas
            ? 'FIM is required for coupled free/dissolved-gas, PVT, phase-appearance, and well-control updates; IMPES gas transport remains an explicit approximation.'
            : 'IMPES is the measured faster default for the catalog oil/water workload; use the FIM vs. IMPES sensitivity to compare formulations on this scenario.',
        comparisonSensitivityAvailable: !involvesGas,
    };
    return {
        ...source,
        params: {
            ...source.params,
            fimEnabled: defaultSolver === 'fim',
        },
        sensitivities: involvesGas
            ? [...source.sensitivities]
            : [...source.sensitivities, solverComparisonSensitivity(defaultSolver)],
        solverPolicy,
    };
}

export const SCENARIOS: CatalogScenario[] = SOURCE_SCENARIOS.map(applySolverPolicy);

// Freeze all scenario params objects to catch accidental in-place mutation early.
// A mutation to one scenario's params cannot silently corrupt another.
for (const scenario of SCENARIOS) {
    Object.freeze(scenario.params);
}

// ─── Lookup helpers ───────────────────────────────────────────────────────────

const scenarioMap = new Map(SCENARIOS.map((s) => [s.key, s]));

export function getScenario(key: string | null | undefined): CatalogScenario | null {
    if (!key) return null;
    const found = scenarioMap.get(key);
    if (!found && import.meta.env.DEV) {
        console.warn(`[scenarios] getScenario: unknown key "${key}"`);
    }
    return found ?? null;
}

/** Returns the full base params for a scenario, or {} if not found. */
export function getScenarioParams(key: string | null | undefined): Record<string, unknown> {
    return getScenario(key)?.params ?? {};
}

/**
 * Returns the full params for a scenario + sensitivity dimension + variant combination.
 * Merges the variant's paramPatch on top of the scenario base params.
 * If dimensionKey or variantKey are null/undefined, returns the base scenario params.
 */
export function getScenarioWithVariantParams(
    scenarioKey: string,
    dimensionKey: string | null | undefined,
    variantKey: string | null | undefined,
): Record<string, unknown> {
    const scenario = getScenario(scenarioKey);
    if (!scenario) return {};
    if (!dimensionKey || !variantKey) return scenario.params;

    const dimension = scenario.sensitivities.find((d) => d.key === dimensionKey);
    if (!dimension) {
        if (import.meta.env.DEV) {
            console.warn(`[scenarios] getScenarioWithVariantParams: unknown dimensionKey "${dimensionKey}" for scenario "${scenarioKey}"`);
        }
        return scenario.params;
    }

    const variant = dimension.variants.find((v) => v.key === variantKey);
    if (!variant) {
        if (import.meta.env.DEV) {
            console.warn(`[scenarios] getScenarioWithVariantParams: unknown variantKey "${variantKey}" in dimension "${dimensionKey}" of scenario "${scenarioKey}"`);
        }
        return scenario.params;
    }

    return { ...scenario.params, ...variant.paramPatch };
}

/**
 * Returns the variant keys that should be pre-selected when a dimension loads.
 * All variants are enabled by default unless explicitly set enabledByDefault: false.
 */
export function getDefaultVariantKeys(dimension: SensitivityDimension): string[] {
    return dimension.variants
        .filter((v) => v.enabledByDefault !== false)
        .map((v) => v.key);
}

/** Resolve the shared chart layout for a scenario plus any scenario/dimension patches. */
export function getScenarioChartLayout(
    scenario: Pick<Scenario, 'chartLayoutKey' | 'chartLayoutPatch' | 'sensitivities'>,
    dimensionKey?: string | null,
): RateChartLayoutConfig {
    const activeDimension = dimensionKey
        ? scenario.sensitivities.find((dimension) => dimension.key === dimensionKey)
        : undefined;
    const layoutKey = activeDimension?.chartLayoutKeyOverride ?? scenario.chartLayoutKey;
    return mergeChartLayoutConfig(
        mergeChartLayoutConfig(getChartLayout(layoutKey), scenario.chartLayoutPatch),
        activeDimension?.chartLayoutPatchOverride,
    );
}

const PRIMARY_ANALYTICAL_PANEL_KEYS = ['rates', 'recovery', 'cumulative', 'diagnostics', 'oil_rate', 'producer_bhp', 'injector_bhp', 'control_limits'] as const;

export function hasPrimaryAnalyticalReferenceCurves(layoutConfig: RateChartLayoutConfig): boolean {
    return PRIMARY_ANALYTICAL_PANEL_KEYS.some((panelKey) => (
        layoutConfig.rateChart?.panels?.[panelKey]?.curveKeys?.some((curveKey) => curveKey.includes('-reference'))
    ));
}

export function suppressesPrimaryAnalyticalOverlays(layoutConfig: RateChartLayoutConfig): boolean {
    return !hasPrimaryAnalyticalReferenceCurves(layoutConfig);
}

export function validateScenarioChartLayout(scenario: Pick<Scenario, 'key' | 'capabilities' | 'chartLayoutKey' | 'chartLayoutPatch' | 'sensitivities'>): string[] {
    const errors: string[] = [];
    const dimensionKeys = [null, ...scenario.sensitivities.map((dimension) => dimension.key)];

    for (const dimensionKey of dimensionKeys) {
        const layout = getScenarioChartLayout(scenario, dimensionKey);
        if (scenario.capabilities.showSweepPanel && hasPrimaryAnalyticalReferenceCurves(layout)) {
            errors.push(
                `scenario '${scenario.key}'${dimensionKey ? ` / ${dimensionKey}` : ''} must not include primary analytical reference curves when showSweepPanel is true.`,
            );
        }
    }

    return errors;
}

export function getAnalyticalModeForMethod(method: AnalyticalMethod): AnalyticalMode {
    if (method === 'depletion') return 'depletion';
    if (method === 'none') return 'none';
    if (method === 'digitized-reference') return 'none';
    return 'waterflood';
}

export function getDefaultScenarioAnalyticalMode(caps: ScenarioCapabilities): AnalyticalMode {
    return getAnalyticalModeForMethod(caps.analyticalMethod);
}

export function getScenarioGroup(scenario: Pick<Scenario, 'capabilities'>): ScenarioGroup {
    const { capabilities } = scenario;
    if (capabilities.requiresThreePhaseMode || capabilities.analyticalMethod === 'gas-oil-bl') {
        return 'gas';
    }
    if (capabilities.showSweepPanel) return 'sweep';
    if (capabilities.analyticalMethod === 'depletion') return 'depletion';
    return 'waterflood';
}

export function listScenarios(): CatalogScenario[] {
    return SCENARIOS;
}

export function solverLabel(solver: SimulationSolver): string {
    return solver === 'fim' ? 'FIM' : 'IMPES';
}

export function solverFromParams(params: Record<string, unknown>): SimulationSolver {
    return params.fimEnabled === true ? 'fim' : 'impes';
}
