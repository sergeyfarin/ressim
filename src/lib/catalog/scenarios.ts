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
import { DEFAULT_SWEEP_METHOD, describeSweepMethod } from '../analytical/sweepMethods';
import type { RateHistoryPoint } from '../simulator-types';
import { getChartLayout, mergeChartLayoutConfig } from './chartLayouts';

export { CHART_LAYOUTS, getChartLayout, mergeChartLayoutConfig } from './chartLayouts';

// ─── Per-scenario imports ────────────────────────────────────────────────────

import { wf_bl1d } from './scenarios/wf_bl1d';
import { wf_capillary } from './scenarios/wf_capillary';
import { sweep_areal } from './scenarios/sweep_areal';
import { sweep_vertical } from './scenarios/sweep_vertical';
import { sweep_combined } from './scenarios/sweep_combined';
import { dep_pss } from './scenarios/dep_pss';
import { dep_arps } from './scenarios/dep_arps';
import { dep_decline } from './scenarios/dep_decline';
import { dep_nct } from './scenarios/dep_nct';
import { dep_welltest } from './scenarios/dep_welltest';
import { dep_pvt } from './scenarios/dep_pvt';
import { gas_injection } from './scenarios/gas_injection';
import { gas_drive } from './scenarios/gas_drive';
import { spe1_gas_injection } from './scenarios/spe1_gas_injection';

// ─── Types ────────────────────────────────────────────────────────────────────

/** Which analytical reference model to compute for overlay curves. */
export type AnalyticalMethod =
    | 'buckley-leverett'
    | 'gas-oil-bl'
    | 'sweep'
    | 'depletion'
    | 'well-test'
    | 'digitized-reference'
    | 'none';

/** Coarse analytical family used by the custom/editor UI. */
export type AnalyticalMode = 'waterflood' | 'depletion' | 'none';

/** Stable, explicit catalog taxonomy used by ScenarioPicker. */
export type ScenarioGroup =
    | 'buckley-leverett-displacement'
    | 'sweep-efficiency'
    | 'depletion-decline'
    | 'pressure-transient'
    | 'gas-black-oil'
    | 'validation-benchmarks'
    | 'other';

/** What kind of product content the scenario represents. */
export type ScenarioRole = 'simulation' | 'interpretation' | 'benchmark';

/** Coarse application mode selected with the scenario. */
export type ScenarioCaseMode = 'wf' | 'dep' | '3p';

/**
 * Catalog placement is declared by the scenario instead of inferred from its
 * solver or analytical method. Those technical traits do not form a sound
 * information architecture: a gas benchmark is still a benchmark, and a
 * well test is not depletion merely because both have one producer.
 */
export type ScenarioCatalog = {
    group: ScenarioGroup;
    role: ScenarioRole;
    caseMode: ScenarioCaseMode;
    /** Scenario-owned concise picker copy; never reconstructed from guessed fields. */
    parameterSummary: string;
};

export const SCENARIO_GROUPS: readonly {
    key: ScenarioGroup;
    label: string;
    description: string;
}[] = [
    {
        key: 'buckley-leverett-displacement',
        label: 'Buckley–Leverett Displacement',
        description: 'One-dimensional displacement fundamentals and departures from the Buckley–Leverett assumptions.',
    },
    {
        key: 'sweep-efficiency',
        label: 'Sweep Efficiency',
        description: 'Areal, vertical, and combined reservoir contact during waterflooding.',
    },
    {
        key: 'depletion-decline',
        label: 'Depletion & Decline',
        description: 'Pressure depletion, boundary-dominated decline, forecasting fits, and depletion non-uniqueness.',
    },
    {
        key: 'pressure-transient',
        label: 'Pressure-Transient Analysis',
        description: 'Well-test interpretation workflows such as drawdown, buildup, Horner, and interference tests.',
    },
    {
        key: 'gas-black-oil',
        label: 'Gas-Dominated Recovery',
        description: 'Gas injection and solution-gas drive recovery mechanisms.',
    },
    {
        key: 'validation-benchmarks',
        label: 'Validation Benchmarks',
        description: 'Published comparative-solution and external-reference cases used to validate the simulator.',
    },
    {
        key: 'other',
        label: 'Other',
        description: 'Cross-cutting interpretation cases awaiting a permanent catalog family.',
    },
] as const;

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

/**
 * One selectable analytical variant for a scenario, as offered by the picker.
 *
 * Derived, never declared: `getScenarioAnalyticalOptions()` generates these from
 * the scenario's capabilities. `sweep_combined` used to carry a hand-written
 * array of these with the label/summary/citation prose inline, which is why the
 * Stiles / Dykstra-Parsons choice existed on exactly one scenario.
 */
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
// `satisfies` rather than a type annotation: it enforces the contract shape while
// preserving each entry's literal types, so `supportedRateCurves` stays a narrow
// tuple (e.g. `readonly ['water-cut']`) instead of widening to
// `readonly PrimaryRateCurve[]`. `ScenarioCapabilities` reads those tuples to
// narrow `primaryRateCurve` per method, which makes this table the single source
// of truth for the rule at both compile time and run time.
export const ANALYTICAL_OUTPUT_CONTRACTS = {
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
    'sweep': {
        // Sweep correlations (Craig areal, Dykstra-Parsons / Stiles vertical)
        // answer "what fraction of the pattern is contacted", not "what is the
        // producing water cut at time t". They therefore produce no primary
        // rate/recovery reference curve at all — their output is the dedicated
        // E_A / E_V / E_vol sweep panels. The displacement inside the contacted
        // region is Buckley-Leverett, but that is a fact about the correlation's
        // internals, not a reference curve to draw against the simulation.
        produces: ['sweep-areal', 'sweep-vertical', 'sweep-combined', 'sweep-rf'],
        supportedRateCurves: ['water-cut'],
        nativeXAxis: 'pvi',
        defaultPrimaryRateCurve: 'water-cut',
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
    'well-test': {
        // A constant-rate pressure transient: the reference curve is flowing
        // bottomhole pressure against time, and the rate is the controlled
        // input rather than a result. No recovery/cumulative curve is meaningful
        // over the hours-to-days span of a test.
        produces: ['pressure', 'oil-rate'],
        supportedRateCurves: ['oil-rate'],
        nativeXAxis: 'time',
        defaultPrimaryRateCurve: 'oil-rate',
        hasTau: false,
        defaultPanelExpansion: { rates: false, recovery: false, cumulative: false, diagnostics: true },
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
} as const satisfies Record<AnalyticalMethod, AnalyticalOutputContract>;

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

/** Capability fields that mean the same thing for every analytical method. */
type ScenarioCapabilitiesBase = {
    /** Override the native x-axis for this analytical method. */
    analyticalNativeXAxis?: 'pvi' | 'time';
    /** Override whether tau is meaningful for this analytical method. */
    hasTauDimensionlessTime?: boolean;
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

/**
 * The method-dependent half of a scenario's capabilities.
 *
 * `primaryRateCurve` is narrowed to whatever that method's entry in
 * `ANALYTICAL_OUTPUT_CONTRACTS` lists as supported, so a depletion scenario
 * cannot ask for a water-cut curve — that is a compile error, not a test
 * failure. `sweepGeometry` is required on 'sweep' and typed `never` elsewhere,
 * making "sweep panels without a geometry" and "geometry on a non-sweep method"
 * both unrepresentable.
 */
type CapabilitiesForMethod<M extends AnalyticalMethod> = {
    /** Which analytical reference model to use — the primary routing key. */
    analyticalMethod: M;
    /** Override the default primary rate curve; limited to this method's supported set. */
    primaryRateCurve?: (typeof ANALYTICAL_OUTPUT_CONTRACTS)[M]['supportedRateCurves'][number];
} & (M extends 'sweep'
    ? {
        /**
         * Sweep decomposition geometry — which of the E_A / E_V / E_vol panels
         * are physically meaningful. There is no separate `showSweepPanel`
         * flag: sweep is a method, not a decoration on another method.
         */
        sweepGeometry: SweepGeometry;
        /**
         * Selectable sweep correlations, most-preferred first — the first entry
         * is the default. Omit (or give one entry) when the scenario has no
         * meaningful choice; a toggle is only offered for two or more.
         *
         * Opt-in rather than automatic because the choice is not always live:
         * at `sweepGeometry: 'areal'` the Stiles and Dykstra-Parsons paths are
         * numerically identical, so offering the toggle there would be a control
         * that does nothing. Pinned by `sweepMethods.test.ts`.
         */
        sweepMethods?: readonly SweepAnalyticalMethod[];
    }
    : { sweepGeometry?: never; sweepMethods?: never });

/**
 * Scenario capability declarations — the single source of truth for all
 * behavioral routing. Fields derivable from `analyticalMethod` are optional
 * overrides; omitted fields inherit from ANALYTICAL_OUTPUT_CONTRACTS.
 *
 * A discriminated union over `analyticalMethod`, so the method's own contract
 * constrains what the rest of the object may say.
 *
 * Consumer code reads resolved capabilities via `resolveCapabilities()`.
 */
export type ScenarioCapabilities = ScenarioCapabilitiesBase & {
    [M in AnalyticalMethod]: CapabilitiesForMethod<M>;
}[AnalyticalMethod];

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
        showSweepPanel: caps.analyticalMethod === 'sweep',
        // No `?? 'both'` fallback: the union makes sweepGeometry mandatory on
        // 'sweep' and absent elsewhere, so this discriminant check is exhaustive.
        sweepGeometry: caps.analyticalMethod === 'sweep' ? caps.sweepGeometry : null,
        hasInjector: caps.hasInjector,
        default3DScalar: caps.default3DScalar,
        requiresThreePhaseMode: caps.requiresThreePhaseMode,
        runMode: caps.runMode ?? 'live-worker',
        defaultPanelExpansion: contract.defaultPanelExpansion,
    };
}

/**
 * Runtime capability checks that the type system cannot express.
 *
 * The three method-derived rules this used to carry — unsupported
 * `primaryRateCurve`, missing `sweepGeometry` on sweep, and `sweepGeometry` on a
 * non-sweep method — are now unrepresentable in `ScenarioCapabilities` itself,
 * so they are compile errors rather than test failures.
 *
 * What remains is the `runMode` / `default3DScalar` rule. It is deliberately not
 * folded into the union: `runMode` is a second, independent discriminant, and
 * crossing it with `analyticalMethod` would multiply the arms for one rule.
 */
export function validateScenarioCapabilities(caps: ScenarioCapabilities): string[] {
    const errors: string[] = [];
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

/**
 * One declared source of non-simulation curves for a scenario's charts.
 *
 * This replaced three separate mechanisms that between them made "which
 * references does this chart show?" unanswerable from the scenario file:
 * a `publishedReferenceSeries` array, an `opmFlowReferenceArtifactKeys` array
 * that only had an effect on `prerun-artifacts` scenarios, and — the invisible
 * one — an implicit match of every bundled OPM artifact whose `scenarioKey`
 * equalled the scenario's key, which is how live scenarios actually got their
 * OPM overlays without saying so anywhere.
 *
 * Order is preserved: sources are resolved and concatenated as declared.
 */
export type ScenarioReferenceSourceDef =
    | {
        kind: 'opm-flow';
        /** Bundled artifact case keys (`opm-flow-results/<caseKey>.json`). */
        artifactKeys: readonly string[];
        /**
         * 'overlay' (default) — dashed reference beside the live simulation curves.
         * 'primary' — solid content; the artifact *is* the exhibit, for
         * `runMode: 'prerun-artifacts'` scenarios with no live run to compare to.
         */
        role?: 'overlay' | 'primary';
    }
    | {
        kind: 'published';
        /** Static digitized series from a paper or published benchmark. */
        series: readonly PublishedReferenceSeries[];
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
    /** Explicit catalog placement, content role, app mode, and picker copy. */
    catalog: ScenarioCatalog;
    description: string;
    analyticalMethodSummary: string;
    analyticalMethodReference: string;
    /** Complete, self-contained simulator parameter set. No shared base objects. */
    params: Record<string, unknown>;
    /** Key into CHART_LAYOUTS — selects the shared chart layout template for this scenario. */
    chartLayoutKey: string;
    /** Scenario-local tweaks applied on top of the shared chart layout. */
    chartLayoutPatch?: RateChartLayoutConfig;
    /** Behavioral capability declarations — single source of truth for all routing logic. */
    capabilities: ScenarioCapabilities;
    /** Scenario-owned numerical formulation choice and user-facing rationale. */
    solverPolicy: ScenarioSolverPolicy;
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
     * Every non-simulation curve source this scenario's charts draw, in order.
     * Nothing is matched implicitly: an OPM artifact appears only if listed here.
     */
    referenceSources?: ScenarioReferenceSourceDef[];
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

/** Scenario after generic catalog enhancements (currently sensitivity injection). */
export type CatalogScenario = Scenario;

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
    sweepGeometry: undefined,
    hasInjector: true,
    default3DScalar: null,
    requiresThreePhaseMode: false,
};

// ─── Shared chart layouts live in ./chartLayouts.ts ─────────────────────────

// ─── Scenarios ────────────────────────────────────────────────────────────────

const SOURCE_SCENARIOS: Scenario[] = [
    wf_bl1d,
    wf_capillary,
    sweep_areal,
    sweep_vertical,
    sweep_combined,
    dep_pss,
    dep_decline,
    dep_arps,
    dep_nct,
    dep_welltest,
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

function applyCatalogEnhancements(source: Scenario): CatalogScenario {
    return {
        ...source,
        params: {
            ...source.params,
            fimEnabled: source.solverPolicy.defaultSolver === 'fim',
        },
        sensitivities: source.solverPolicy.comparisonSensitivityAvailable
            ? [...source.sensitivities, solverComparisonSensitivity(source.solverPolicy.defaultSolver)]
            : [...source.sensitivities],
    };
}

export const SCENARIOS: CatalogScenario[] = SOURCE_SCENARIOS.map(applyCatalogEnhancements);

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

/**
 * Validates that a scenario's chart layout only asks for analytical reference
 * curves its analytical method can actually produce.
 *
 * This used to be the inverse: a scenario opted *out* of unwanted overlays by
 * omitting `-reference` curve keys from its layout, and the check was a negative
 * one against `showSweepPanel`. Since sweep became a first-class analytical
 * method with no primary curve slots, unwanted overlays are never built, so the
 * layout can be checked positively — a layout naming a curve the method does not
 * emit is a dead reference, not a suppression mechanism.
 */
export function validateScenarioChartLayout(
    scenario: Pick<Scenario, 'key' | 'capabilities' | 'chartLayoutKey' | 'chartLayoutPatch' | 'sensitivities'>,
    emittedReferenceCurveKeys: ReadonlySet<string>,
): string[] {
    const errors: string[] = [];
    const dimensionKeys = [null, ...scenario.sensitivities.map((dimension) => dimension.key)];

    for (const dimensionKey of dimensionKeys) {
        const layout = getScenarioChartLayout(scenario, dimensionKey);
        for (const panelKey of PRIMARY_ANALYTICAL_PANEL_KEYS) {
            const curveKeys = layout.rateChart?.panels?.[panelKey]?.curveKeys ?? [];
            for (const curveKey of curveKeys) {
                if (!curveKey.endsWith('-reference')) continue;
                // Published/OPM series are appended by source, not by analytical
                // method, so they are not the method's business.
                if (curveKey.startsWith('published-')) continue;
                if (emittedReferenceCurveKeys.has(curveKey)) continue;
                errors.push(
                    `scenario '${scenario.key}'${dimensionKey ? ` / ${dimensionKey}` : ''} panel '${panelKey}' asks for '${curveKey}', `
                    + `which analyticalMethod '${scenario.capabilities.analyticalMethod}' does not produce.`,
                );
            }
        }
    }

    return errors;
}

/**
 * The analytical variants a scenario offers, derived from its capabilities.
 *
 * Only the sweep method has a user-selectable variant today. Returns `[]` when
 * there is no genuine choice — including a sweep scenario that declares one
 * method, or none — so the picker shows a toggle exactly when one is warranted.
 */
export function getScenarioAnalyticalOptions(
    scenario: Pick<Scenario, 'capabilities'> | null | undefined,
): ScenarioAnalyticalOption[] {
    const caps = scenario?.capabilities;
    if (caps?.analyticalMethod !== 'sweep') return [];
    const methods = caps.sweepMethods ?? [];
    if (methods.length < 2) return [];
    return methods.map((method, index) => ({
        ...describeSweepMethod(method, caps.sweepGeometry),
        sweepMethod: method,
        // First declared entry is the default — no separate `default: true` flag
        // to keep in sync with the ordering.
        default: index === 0,
    }));
}

/**
 * The sweep correlation a scenario uses when the user has not chosen one.
 * First declared entry wins; falls back to the engine-wide default.
 */
export function getDefaultSweepMethod(
    scenario: Pick<Scenario, 'capabilities'> | null | undefined,
): SweepAnalyticalMethod {
    const caps = scenario?.capabilities;
    if (caps?.analyticalMethod !== 'sweep') return DEFAULT_SWEEP_METHOD;
    return caps.sweepMethods?.[0] ?? DEFAULT_SWEEP_METHOD;
}

export function getAnalyticalModeForMethod(method: AnalyticalMethod): AnalyticalMode {
    // Well test shares the coarse 'depletion' family: single producer, no
    // injector, pressure-and-rate outputs on a time axis. Sweep shares the
    // coarse 'waterflood' family. The fine-grained routing is on
    // `analyticalMethod`, not this coarse mode.
    if (method === 'depletion' || method === 'well-test') return 'depletion';
    if (method === 'none') return 'none';
    if (method === 'digitized-reference') return 'none';
    return 'waterflood';
}

export function getDefaultScenarioAnalyticalMode(caps: ScenarioCapabilities): AnalyticalMode {
    return getAnalyticalModeForMethod(caps.analyticalMethod);
}

export function getScenarioGroup(scenario: Pick<Scenario, 'catalog'>): ScenarioGroup {
    return scenario.catalog.group;
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
