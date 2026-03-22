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
import type { SweepGeometry } from '../analytical/sweepEfficiency';

// ─── Per-scenario imports ────────────────────────────────────────────────────

import { wf_bl1d } from './scenarios/wf_bl1d';
import { sweep_areal } from './scenarios/sweep_areal';
import { sweep_vertical } from './scenarios/sweep_vertical';
import { sweep_combined } from './scenarios/sweep_combined';
import { dep_pss } from './scenarios/dep_pss';
import { dep_arps } from './scenarios/dep_arps';
import { dep_decline } from './scenarios/dep_decline';
import { gas_injection } from './scenarios/gas_injection';
import { gas_drive } from './scenarios/gas_drive';

// ─── Types ────────────────────────────────────────────────────────────────────

/** Physical classification — retained as metadata label; behavioral routing uses capabilities. */
export type ScenarioClass = 'waterflood' | 'depletion' | '3phase' | 'gas-oil-bl';

/**
 * UI domain — controls which tab a scenario appears under in ScenarioPicker.
 * Purely presentational — behavioral routing uses capabilities, not domain.
 */
export type ScenarioDomain = 'waterflood' | 'sweep' | 'depletion' | 'gas';

/** Which analytical reference model to compute for overlay curves. */
export type AnalyticalMethod =
    | 'buckley-leverett'
    | 'gas-oil-bl'
    | 'depletion'
    | 'none';

/** Which primary rate curve to show in the "rates" chart panel. */
export type PrimaryRateCurve = 'water-cut' | 'gas-cut' | 'oil-rate';

/** How analytical overlays should be grouped for a selected sensitivity dimension. */
export type AnalyticalOverlayMode = 'auto' | 'shared' | 'per-result';

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
    'none': {
        produces: [],
        supportedRateCurves: ['water-cut', 'gas-cut', 'oil-rate'],
        nativeXAxis: 'time',
        defaultPrimaryRateCurve: 'oil-rate',
        hasTau: false,
        defaultPanelExpansion: { rates: true, recovery: true, cumulative: false, diagnostics: false },
    },
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
     * Override the scenario's chartPreset when this dimension is active.
     * Useful when a dimension (e.g. grid convergence) benefits from a different
     * default view than the scenario's primary chart.
     */
    chartPresetOverride?: string;
};

export type Scenario = {
    key: string;
    label: string;
    description: string;
    analyticalMethodSummary: string;
    analyticalMethodReference: string;
    /** Physical classification — metadata label, not used for behavioral routing. */
    scenarioClass: ScenarioClass;
    /** UI domain — controls which tab this scenario appears under (presentational only). */
    domain: ScenarioDomain;
    /** Complete, self-contained simulator parameter set. No shared base objects. */
    params: Record<string, unknown>;
    /** Key into CHART_PRESETS — controls default x-axis, panels, and curve selection. */
    chartPreset: string;
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
};

/** Default capabilities for custom mode (no predefined scenario). */
export const CUSTOM_MODE_CAPABILITIES: ScenarioCapabilities = {
    analyticalMethod: 'none',
    showSweepPanel: false,
    sweepGeometry: undefined,
    hasInjector: true,
    default3DScalar: null,
    requiresThreePhaseMode: false,
};

// ─── Chart presets ────────────────────────────────────────────────────────────

export const CHART_PRESETS: Record<string, RateChartLayoutConfig> = {
    /**
     * 1D Buckley-Leverett waterflood: PVI x-axis, one variable per panel.
     * Rates → water cut  |  Recovery → RF  |  Cum Oil (collapsed)  |  Pressure (collapsed)
     */
    waterflood: {
        rateChart: {
            xAxisMode: 'pvi',
            xAxisOptions: ['pvi', 'time', 'cumInjection'],
            xAxisRangePolicy: { mode: 'rate-tail-threshold', relativeThreshold: 1e-7 },
            allowLogScale: false,
            logScale: false,
            ratesExpanded: true,
            recoveryExpanded: true,
            cumulativeExpanded: false,
            diagnosticsExpanded: false,
            panels: {
                rates: {
                    title: 'Watercut',
                    curveKeys: ['water-cut-sim', 'water-cut-reference'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                },
                volumes: {
                    title: 'Cum Injection',
                    curveKeys: ['cum-injection'],
                    scalePreset: 'cumulative_volumes',
                },
                oil_rate: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    scalePreset: 'rates',
                },
            },
        },
    },

    /**
     * Sweep efficiency scenarios: all standard panels collapsed — the sweep-specific panels
     * (RF sweep, E_A, E_V, E_vol) are the primary display when showSweepPanel is active.
     */
    sweep: {
        rateChart: {
            xAxisMode: 'pvi',
            xAxisOptions: ['pvi', 'time'],
            xAxisRangePolicy: { mode: 'pvi-window', minPvi: 0, maxPvi: 2.5 },
            allowLogScale: false,
            logScale: false,
            ratesExpanded: false,
            recoveryExpanded: false,
            cumulativeExpanded: false,
            diagnosticsExpanded: false,
            panels: {
                rates: {
                    title: 'Watercut',
                    curveKeys: ['water-cut-sim', 'water-cut-reference'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                },
            },
        },
    },

    /**
     * Oil depletion (PSS/transient): time x-axis, oil rate + RF + pressure panels.
     * Analytical reference solution shown alongside simulation.
     */
    oil_depletion: {
        rateChart: {
            xAxisMode: 'time',
            xAxisOptions: ['time', 'tD', 'logTime'],
            xAxisRangePolicy: { mode: 'data-extent' },
            allowLogScale: true,
            logScale: false,
            ratesExpanded: true,
            recoveryExpanded: true,
            cumulativeExpanded: false,
            diagnosticsExpanded: true,
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure & MBE',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference', 'mbe-ooip-ratio', 'drive-compaction', 'drive-oil-expansion', 'drive-gas-cap'],
                    scalePreset: 'pressure',
                },
            },
        },
    },

    /**
     * Fetkovich exponential decline: log-time x-axis, log-scale rates.
     * Same panel structure as oil_depletion.
     */
    fetkovich: {
        rateChart: {
            xAxisMode: 'logTime',
            xAxisOptions: ['logTime', 'time', 'tD'],
            xAxisRangePolicy: { mode: 'data-extent' },
            allowLogScale: true,
            logScale: true,
            ratesExpanded: true,
            recoveryExpanded: true,
            cumulativeExpanded: false,
            diagnosticsExpanded: true,
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim', 'oil-rate-reference'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure & MBE',
                    curveKeys: ['avg-pressure-sim', 'avg-pressure-reference', 'mbe-ooip-ratio', 'drive-compaction', 'drive-oil-expansion', 'drive-gas-cap'],
                    scalePreset: 'pressure',
                },
            },
        },
    },

    /**
     * Gas-oil Buckley-Leverett: PVI x-axis, gas cut + recovery + cum oil panels
     * with analytical reference overlays (gas-oil fractional flow).
     */
    gas_oil_bl: {
        rateChart: {
            xAxisMode: 'pvi',
            xAxisOptions: ['pvi', 'time', 'cumInjection'],
            xAxisRangePolicy: { mode: 'rate-tail-threshold', relativeThreshold: 1e-7 },
            allowLogScale: false,
            logScale: false,
            ratesExpanded: true,
            recoveryExpanded: true,
            cumulativeExpanded: false,
            diagnosticsExpanded: false,
            panels: {
                rates: {
                    title: 'Gas Breakthrough',
                    curveKeys: ['gas-cut-sim', 'gas-cut-reference'],
                    scalePreset: 'breakthrough',
                    allowLogToggle: false,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary', 'recovery-factor-reference'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim', 'cum-oil-reference'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                },
                volumes: {
                    title: 'Cum Injection',
                    curveKeys: ['cum-injection'],
                    scalePreset: 'cumulative_volumes',
                },
                oil_rate: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    scalePreset: 'rates',
                },
            },
        },
    },

    /**
     * Gas-domain scenarios: no validated analytical reference yet.
     * Same panel structure as oil_depletion but without analytical reference curves.
     */
    gas: {
        rateChart: {
            xAxisMode: 'time',
            xAxisOptions: ['time', 'logTime'],
            xAxisRangePolicy: { mode: 'data-extent' },
            allowLogScale: true,
            logScale: false,
            ratesExpanded: true,
            recoveryExpanded: true,
            cumulativeExpanded: false,
            diagnosticsExpanded: true,
            panels: {
                rates: {
                    title: 'Oil Rate',
                    curveKeys: ['oil-rate-sim'],
                    scalePreset: 'rates',
                    allowLogToggle: true,
                },
                recovery: {
                    title: 'Recovery Factor',
                    curveKeys: ['recovery-factor-primary'],
                    scalePreset: 'recovery',
                },
                cumulative: {
                    title: 'Cum Oil',
                    curveKeys: ['cum-oil-sim'],
                    scalePreset: 'cumulative_volumes',
                },
                diagnostics: {
                    title: 'Pressure',
                    curveKeys: ['avg-pressure-sim'],
                    scalePreset: 'pressure',
                },
            },
        },
    },
};

// ─── Scenarios ────────────────────────────────────────────────────────────────

export const SCENARIOS: Scenario[] = [
    wf_bl1d,
    sweep_areal,
    sweep_vertical,
    sweep_combined,
    dep_pss,
    dep_decline,
    dep_arps,
    gas_injection,
    gas_drive,
];

// Freeze all scenario params objects to catch accidental in-place mutation early.
// A mutation to one scenario's params cannot silently corrupt another.
for (const scenario of SCENARIOS) {
    Object.freeze(scenario.params);
}

// ─── Lookup helpers ───────────────────────────────────────────────────────────

const scenarioMap = new Map(SCENARIOS.map((s) => [s.key, s]));

export function getScenario(key: string | null | undefined): Scenario | null {
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

/** Returns the chart layout config for a preset key, or {} if not found. */
export function getChartPreset(presetKey: string | null | undefined): RateChartLayoutConfig {
    return CHART_PRESETS[presetKey ?? ''] ?? {};
}

export function listScenarios(): Scenario[] {
    return SCENARIOS;
}
