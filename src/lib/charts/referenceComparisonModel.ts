import { calculateDepletionAnalyticalProduction } from '../analytical/depletionAnalytical';
import { calculateMaterialBalance } from '../analytical/materialBalance';
import { calculateAnalyticalProduction, calculateGasOilAnalyticalProduction } from '../analytical/fractionalFlow';
import type { RockProps, FluidProps, GasOilRockProps, GasOilFluidProps } from '../analytical/fractionalFlow';
import { computeCombinedSweep, computeSweepRecoveryFactor, getSweepComponentVisibility, type SweepAnalyticalMethod, type SweepGeometry } from '../analytical/sweepEfficiency';
import type { BenchmarkFamily } from '../catalog/benchmarkCases';
import type { AnalyticalOverlayMode } from '../catalog/scenarios';
import type { BenchmarkRunResult } from '../benchmarkRunModel';
import type { CurveConfig } from './chartTypes';
import type { RateChartPanelId, RateChartAuxiliaryPanelKey, RateChartPanelKey, RateChartXAxisMode } from './rateChartLayoutConfig';
import {
    ANALYTICAL_DASH, PUBLISHED_DASH, AUXILIARY_DASH,
    SWEEP_DASH_AREAL, SWEEP_DASH_VERTICAL, SWEEP_DASH_COMBINED,
    LEGEND_SECTIONS,
    ANALYTICAL_BORDER,
    simBorderWidth,
} from './curveStylePolicy';
import {
    type DerivedRunSeries,
    type XYPoint,
    toXYSeries,
    buildXAxisValues,
    getSweepZeroXAxisValue,
    mapPviSeriesToXAxis,
    interpolateXAxisAtTimes,
    requiresRunMappedAnalyticalXAxis,
    buildAnalyticalAxisWarning,
} from './axisAdapters';
import {
    toFiniteNumber,
    getTotalThickness,
    getAverageLayerThickness,
    getPoreVolume,
    getOoip,
    extractRockProps,
    extractFluidProps,
    extractGasOilRockProps,
    extractGasOilFluidProps,
    getBuckleyLeverettOverlaySignature,
    hasDistinctBuckleyLeverettOverlays,
    getGasOilBLOverlaySignature,
    hasDistinctGasOilBLOverlays,
    resolveOverlayMode,
    computeBLAnalyticalFromParams,
    computeGasOilBLAnalyticalFromParams,
    computeDepletionAnalyticalFromParams,
    computeDepletionTau,
    computeMbeDiagnostics,
    type MbeDiagnostics,
    MIN_GOR_OIL_RATE_SM3_DAY,
    buildDerivedRunSeries,
    getLayerPermeabilities,
} from './analyticalParamAdapters';

export type { XYPoint };

export type ReferenceComparisonPanel = {
    curves: CurveConfig[];
    series: XYPoint[][];
};

type ReferenceComparisonSweepPanels = {
    rf: ReferenceComparisonPanel | null;
    areal: ReferenceComparisonPanel | null;
    vertical: ReferenceComparisonPanel | null;
    combined: ReferenceComparisonPanel | null;
    combinedMobileOil: ReferenceComparisonPanel | null;
};

type ReferenceComparisonPrimaryPanelMap = Record<RateChartPanelKey, ReferenceComparisonPanel>;
type ReferenceComparisonAuxiliaryPanelMap = Record<RateChartAuxiliaryPanelKey, ReferenceComparisonPanel | null>;

export type ReferenceComparisonPanelMap = ReferenceComparisonPrimaryPanelMap & ReferenceComparisonAuxiliaryPanelMap;

export type ReferenceComparisonModel = {
    orderedResults: BenchmarkRunResult[];
    /**
     * Preview/pending variant entries for the cases selector UI.
     * Populated when:
     *  - Pure preview (no results): multi-variant analytical preview cases.
     *  - Mid-sweep (some results done): remaining queued/running variants.
     * Empty when all variants have completed results (orderedResults covers everything).
     */
    previewCases: ReferenceComparisonPreviewCase[];
    panels: ReferenceComparisonPanelMap;
    axisMappingWarning: string | null;
};

export type ReferenceComparisonTheme = 'dark' | 'light';

/**
 * A preview or pending variant entry shown in the cases selector UI.
 * Used both in pure-preview mode (no results yet) and during mid-sweep
 * (some results done, others still queued/running).
 */
export type ReferenceComparisonPreviewCase = {
    /** Variant key — matches caseKey on the chart series for visibility toggling. */
    key: string;
    /** Display label for the cases selector button. */
    label: string;
    /** Color palette index used for this variant's curves. */
    colorIndex: number;
};

/**
 * One entry in the multi-variant analytical preview shown before any simulation
 * results exist. Each entry produces its own colored analytical curve so the user
 * can see how the 4 mobility (or other sensitivity) variants differ analytically
 * without having to run anything first.
 */
export type AnalyticalPreviewVariant = {
    /** Display label used in curve legends (e.g. "Favorable", "Base"). */
    label: string;
    /** Variant key used as caseKey on chart series for future toggle support. */
    variantKey: string;
    /** Full merged params (base scenario params + variant paramPatch). */
    params: Record<string, any>;
};


const MIN_GOR_OIL_RATE_SM3_DAY = 10.0;

type AnalyticalOverlay = {
    rates: { label: string; values: Array<number | null> } | null;
    cumulative: {
        recoveryLabel: string;
        recoveryValues: Array<number | null>;
        cumulativeLabel: string;
        cumulativeValues: Array<number | null>;
    } | null;
    diagnostics: { label: string; values: Array<number | null> } | null;
    xValues: Array<number | null>;
};

/** Tableau 20 — 20 perceptually distinct colors for categorical data. */
const CASE_COLORS = [
    '#4e79a7',
    '#f28e2b',
    '#e15759',
    '#76b7b2',
    '#59a14f',
    '#edc948',
    '#b07aa1',
    '#ff9da7',
    '#9c755f',
    '#bab0ac',
    '#af7aa1',
    '#d37295',
    '#fabfd2',
    '#b6992d',
    '#499894',
    '#86bcb6',
    '#8cd17d',
    '#f1ce63',
    '#a0cbe8',
    '#ffbe7d',
];

export function getReferenceComparisonCaseColor(index: number): string {
    return CASE_COLORS[index % CASE_COLORS.length];
}

function getReferenceColor(theme: ReferenceComparisonTheme): string {
    return theme === 'dark' ? '#f8fafc' : '#0f172a';
}

/** Neutral grey used as the toggle-group line indicator for simulation and
 *  analytical legend items in comparison charts. The actual line colors come
 *  from the case-color palette (shown in the Cases selector above the charts). */
function getLegendGrey(theme: ReferenceComparisonTheme): string {
    return theme === 'dark' ? '#94a3b8' : '#64748b';
}

/**
 * Strips the scenario-name prefix from a case label so sub-panel legend buttons
 * stay compact. E.g. "Rate Decline — s=0 (clean)" → "s=0 (clean)".
 * Falls back to the full label when no separator is found.
 */
function compactCaseLabel(label: string): string {
    const emDash = label.indexOf(' — ');
    if (emDash !== -1) return label.slice(emDash + 3).trim();
    const hyphen = label.indexOf(' - ');
    if (hyphen !== -1) return label.slice(hyphen + 3).trim();
    return label;
}

function getBaseResult(results: BenchmarkRunResult[]): BenchmarkRunResult | null {
    return results.find((result) => result.variantKey === null) ?? results[0] ?? null;
}

function orderResults(
    results: BenchmarkRunResult[],
    variantOrder?: AnalyticalPreviewVariant[],
): BenchmarkRunResult[] {
    if (!variantOrder?.length) return [...results];
    const orderIndex = new Map(variantOrder.map((v, i) => [v.variantKey, i]));
    return [...results].sort((a, b) => {
        const ai = a.variantKey != null ? orderIndex.get(a.variantKey) ?? Infinity : -1;
        const bi = b.variantKey != null ? orderIndex.get(b.variantKey) ?? Infinity : -1;
        return ai - bi;
    });
}

function buildBuckleyLeverettReference(
    baseResult: BenchmarkRunResult,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
): AnalyticalOverlay {
    if (xAxisMode === 'pvi') {
        const analytical = computeBLAnalyticalFromParams(baseResult.params);
        if (!analytical) {
            return {
                rates: null,
                cumulative: null,
                diagnostics: null,
                xValues: [],
            };
        }

        return {
            rates: { label: 'Reference Solution Water Cut', values: analytical.waterCut },
            cumulative: {
                recoveryLabel: 'Reference Solution Recovery',
                recoveryValues: analytical.recovery,
                cumulativeLabel: 'Reference Solution Cum Oil',
                cumulativeValues: analytical.cumulativeOil,
            },
            diagnostics: null,
            xValues: analytical.xValues,
        };
    }

    const analytical = computeBLAnalyticalFromParams(baseResult.params, {
        xValues: buildXAxisValues(derived, xAxisMode),
        timeHistory: derived.time,
        injectionRateSeries: baseResult.rateHistory.map((point) => Math.max(0, toFiniteNumber(point.total_injection, 0))),
        poreVolume: getPoreVolume(baseResult.params),
        recoveryDenominator: getOoip(baseResult.params),
    });
    if (!analytical) {
        return {
            rates: null,
            cumulative: null,
            diagnostics: null,
            xValues: [],
        };
    }

    return {
        rates: { label: 'Reference Solution Water Cut', values: analytical.waterCut },
        cumulative: {
            recoveryLabel: 'Reference Solution Recovery',
            recoveryValues: analytical.recovery,
            cumulativeLabel: 'Reference Solution Cum Oil',
            cumulativeValues: analytical.cumulativeOil,
        },
        diagnostics: null,
        xValues: analytical.xValues,
    };
}

function buildDepletionReference(
    baseResult: BenchmarkRunResult,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
): AnalyticalOverlay {
    const analyticalResult = calculateDepletionAnalyticalProduction({
        reservoir: {
            length: toFiniteNumber(baseResult.params.nx, 1) * toFiniteNumber(baseResult.params.cellDx, 10),
            area: toFiniteNumber(baseResult.params.ny, 1)
                * toFiniteNumber(baseResult.params.cellDy, 10)
                * getTotalThickness(baseResult.params),
            porosity: toFiniteNumber(baseResult.params.reservoirPorosity ?? baseResult.params.porosity, 0.2),
        },
        timeHistory: derived.time,
        minTimeDays: toFiniteNumber(baseResult.params.analyticalDepletionStartDays, 0),
        initialSaturation: toFiniteNumber(baseResult.params.initialSaturation, 0.3),
        nz: toFiniteNumber(baseResult.params.nz, 1),
        permMode: String(baseResult.params.permMode ?? 'uniform'),
        uniformPermX: toFiniteNumber(baseResult.params.uniformPermX, 100),
        uniformPermY: toFiniteNumber(baseResult.params.uniformPermY ?? baseResult.params.uniformPermX, 100),
        layerPermsX: Array.isArray(baseResult.params.layerPermsX) ? baseResult.params.layerPermsX.map(Number) : [],
        layerPermsY: Array.isArray(baseResult.params.layerPermsY) ? baseResult.params.layerPermsY.map(Number) : [],
        cellDx: toFiniteNumber(baseResult.params.cellDx, 10),
        cellDy: toFiniteNumber(baseResult.params.cellDy, 10),
        cellDz: getAverageLayerThickness(baseResult.params),
        wellRadius: toFiniteNumber(baseResult.params.well_radius, 0.1),
        wellSkin: toFiniteNumber(baseResult.params.well_skin, 0),
        muO: toFiniteNumber(baseResult.params.mu_o, 1),
        sWc: toFiniteNumber(baseResult.params.s_wc, 0.1),
        sOr: toFiniteNumber(baseResult.params.s_or, 0.1),
        nO: toFiniteNumber(baseResult.params.n_o, 2),
        c_o: toFiniteNumber(baseResult.params.c_o, 1e-5),
        c_w: toFiniteNumber(baseResult.params.c_w, 3e-6),
        cRock: toFiniteNumber(baseResult.params.rock_compressibility, 1e-6),
        initialPressure: toFiniteNumber(baseResult.params.initialPressure, 300),
        producerBhp: toFiniteNumber(baseResult.params.producerBhp, 100),
        depletionRateScale: toFiniteNumber(baseResult.params.analyticalDepletionRateScale, 1),
        arpsB: toFiniteNumber(baseResult.params.analyticalArpsB, 0),
        nx: toFiniteNumber(baseResult.params.nx, 1),
        ny: toFiniteNumber(baseResult.params.ny, 1),
        producerI: baseResult.params.producerI != null ? toFiniteNumber(baseResult.params.producerI, 0) : undefined,
        producerJ: baseResult.params.producerJ != null ? toFiniteNumber(baseResult.params.producerJ, 0) : undefined,
    });
    const ooip = getOoip(baseResult.params);
    const tau = analyticalResult.meta.tau ?? null;

    return {
        rates: {
            label: 'Reference Solution Oil Rate',
            values: analyticalResult.production.map((point) => point.oilRate),
        },
        cumulative: {
            recoveryLabel: 'Reference Solution Recovery',
            recoveryValues: analyticalResult.production.map((point) => (
                ooip > 1e-12 ? Math.max(0, Math.min(1, point.cumulativeOil / ooip)) : null
            )),
            cumulativeLabel: 'Reference Solution Cum Oil',
            cumulativeValues: analyticalResult.production.map((point) => point.cumulativeOil),
        },
        diagnostics: {
            label: 'Reference Solution Avg Pressure',
            values: analyticalResult.production.map((point) => point.avgPressure),
        },
        xValues: buildXAxisValues(
            {
                ...derived,
                time: analyticalResult.production.map((point) => point.time),
            },
            xAxisMode,
            tau,
        ),
    };
}

function appendSeries(
    panel: ReferenceComparisonPanel,
    curve: CurveConfig,
    xValues: Array<number | null>,
    yValues: Array<number | null>,
) {
    panel.curves.push(curve);
    panel.series.push(toXYSeries(xValues, yValues));
}

function appendBhpLimitDiagnostics(
    panel: ReferenceComparisonPanel,
    input: {
        label: string;
        caseKey: string;
        toggleLabel: string;
        borderWidth: number;
        defaultVisible: boolean;
        xValues: Array<number | null>;
        producerValues: Array<number | null>;
        injectorValues: Array<number | null>;
    },
) {
    appendSeries(panel, {
        label: `${input.label} Producer BHP-limited`,
        curveKey: 'producer-bhp-limited-sim',
        caseKey: input.caseKey,
        toggleGroupKey: input.caseKey,
        toggleLabel: input.toggleLabel,
        legendSection: 'sim',
        legendSectionLabel: LEGEND_SECTIONS.sim,
        color: '#c2410c',
        borderWidth: input.borderWidth,
        yAxisID: 'y',
        defaultVisible: input.defaultVisible,
    }, input.xValues, input.producerValues);

    appendSeries(panel, {
        label: `${input.label} Injector BHP-limited`,
        curveKey: 'injector-bhp-limited-sim',
        caseKey: input.caseKey,
        toggleGroupKey: input.caseKey,
        toggleLabel: input.toggleLabel,
        legendSection: 'sim',
        legendSectionLabel: LEGEND_SECTIONS.sim,
        color: '#0369a1',
        borderWidth: input.borderWidth,
        yAxisID: 'y',
        defaultVisible: input.defaultVisible,
    }, input.xValues, input.injectorValues);
}

function appendPublishedReferenceSeries(
    panels: Record<RateChartPanelKey, ReferenceComparisonPanel>,
    family: BenchmarkFamily | null,
) {
    if (!family?.publishedReferenceSeries?.length) return;

    const publishedColor = '#e74c3c';
    for (const series of family.publishedReferenceSeries) {
        const targetPanel = panels[series.panelKey as RateChartPanelKey];
        if (!targetPanel) continue;
        appendSeries(targetPanel, {
            label: series.label,
            curveKey: series.curveKey,
            toggleGroupKey: 'published-reference',
            toggleLabel: 'Published reference',
            legendSection: 'published',
            legendSectionLabel: LEGEND_SECTIONS.published,
            color: publishedColor,
            borderWidth: 1.5,
            borderDash: PUBLISHED_DASH,
            yAxisID: series.yAxisID ?? 'y',
            pointRadius: 0,
        }, series.data.map((pt) => pt.x), series.data.map((pt) => pt.y));
    }
}

function createReferenceComparisonPanel(): ReferenceComparisonPanel {
    return { curves: [], series: [] };
}

function createSweepPanels(): Record<keyof ReferenceComparisonSweepPanels, ReferenceComparisonPanel> {
    return {
        rf: createReferenceComparisonPanel(),
        areal: createReferenceComparisonPanel(),
        vertical: createReferenceComparisonPanel(),
        combined: createReferenceComparisonPanel(),
        combinedMobileOil: createReferenceComparisonPanel(),
    };
}

function stripReferenceCurveKeys(
    panel: ReferenceComparisonPanel,
    excludedCurveKeys: Set<string>,
): ReferenceComparisonPanel {
    const keptEntries = panel.curves
        .map((curve, index) => ({ curve, series: panel.series[index] ?? [] }))
        .filter((entry) => !excludedCurveKeys.has(entry.curve.curveKey ?? entry.curve.label));

    return {
        curves: keptEntries.map((entry) => entry.curve),
        series: keptEntries.map((entry) => entry.series),
    };
}

function suppressPrimaryAnalyticalPanels(
    panels: Record<RateChartPanelKey, ReferenceComparisonPanel>,
): Record<RateChartPanelKey, ReferenceComparisonPanel> {
    const excludedCurveKeys = new Set([
        'oil-rate-reference',
        'water-cut-reference',
        'gas-cut-reference',
        'recovery-factor-reference',
        'cum-oil-reference',
        'avg-pressure-reference',
        'p_z_reference',
    ]);

    return {
        rates: stripReferenceCurveKeys(panels.rates, excludedCurveKeys),
        recovery: stripReferenceCurveKeys(panels.recovery, excludedCurveKeys),
        cumulative: stripReferenceCurveKeys(panels.cumulative, excludedCurveKeys),
        diagnostics: stripReferenceCurveKeys(panels.diagnostics, excludedCurveKeys),
        gor: panels.gor,
        volumes: panels.volumes,
        oil_rate: stripReferenceCurveKeys(panels.oil_rate, excludedCurveKeys),
        injection_rate: panels.injection_rate,
        producer_bhp: panels.producer_bhp,
        injector_bhp: panels.injector_bhp,
        control_limits: panels.control_limits,
    };
}

function emptyPanelMap(): ReferenceComparisonPanelMap {
    return {
        rates: createReferenceComparisonPanel(),
        recovery: createReferenceComparisonPanel(),
        cumulative: createReferenceComparisonPanel(),
        diagnostics: createReferenceComparisonPanel(),
        gor: createReferenceComparisonPanel(),
        volumes: createReferenceComparisonPanel(),
        oil_rate: createReferenceComparisonPanel(),
        injection_rate: createReferenceComparisonPanel(),
        producer_bhp: createReferenceComparisonPanel(),
        injector_bhp: createReferenceComparisonPanel(),
        control_limits: createReferenceComparisonPanel(),
        sweep_rf: null,
        sweep_areal: null,
        sweep_vertical: null,
        sweep_combined: null,
        sweep_combined_mobile_oil: null,
    };
}

function combinePanelMaps(input: {
    primary: ReferenceComparisonPrimaryPanelMap;
    sweep?: ReferenceComparisonSweepPanels;
}): ReferenceComparisonPanelMap {
    return {
        ...emptyPanelMap(),
        rates: input.primary.rates,
        recovery: input.primary.recovery,
        cumulative: input.primary.cumulative,
        diagnostics: input.primary.diagnostics,
        gor: input.primary.gor,
        volumes: input.primary.volumes,
        oil_rate: input.primary.oil_rate,
        injection_rate: input.primary.injection_rate,
        producer_bhp: input.primary.producer_bhp,
        injector_bhp: input.primary.injector_bhp,
        control_limits: input.primary.control_limits,
        sweep_rf: input.sweep?.rf ?? null,
        sweep_areal: input.sweep?.areal ?? null,
        sweep_vertical: input.sweep?.vertical ?? null,
        sweep_combined: input.sweep?.combined ?? null,
        sweep_combined_mobile_oil: input.sweep?.combinedMobileOil ?? null,
    };
}

function emptySweepPanels(): ReferenceComparisonSweepPanels {
    return {
        rf: null,
        areal: null,
        vertical: null,
        combined: null,
        combinedMobileOil: null,
    };
}

function finalizeSweepPanels(
    panels: Record<keyof ReferenceComparisonSweepPanels, ReferenceComparisonPanel>,
): ReferenceComparisonSweepPanels {
    return {
        rf: panels.rf.curves.length > 0 ? panels.rf : null,
        areal: panels.areal.curves.length > 0 ? panels.areal : null,
        vertical: panels.vertical.curves.length > 0 ? panels.vertical : null,
        combined: panels.combined.curves.length > 0 ? panels.combined : null,
        combinedMobileOil: panels.combinedMobileOil.curves.length > 0 ? panels.combinedMobileOil : null,
    };
}

function buildGasOilBLReference(
    baseResult: BenchmarkRunResult,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
): AnalyticalOverlay {
    if (xAxisMode === 'pvi') {
        const analytical = computeGasOilBLAnalyticalFromParams(baseResult.params);
        if (analytical) {
            const ooip = getOoip(baseResult.params);
            return {
                rates: { label: 'Reference Solution Gas Cut', values: analytical.gasCut },
                cumulative: {
                    recoveryLabel: 'Reference Solution Recovery',
                    recoveryValues: analytical.recovery,
                    cumulativeLabel: 'Reference Solution Cum Oil',
                    cumulativeValues: analytical.cumulativeOil.map((value) => (
                        Number.isFinite(value) && ooip > 1e-12 ? Number(value) * ooip : null
                    )),
                },
                diagnostics: null,
                xValues: analytical.pviValues,
            };
        }
    }

    const poreVolume = getPoreVolume(baseResult.params);
    const ooip = getOoip(baseResult.params);
    const analyticalProduction = calculateGasOilAnalyticalProduction(
        extractGasOilRockProps(baseResult.params),
        extractGasOilFluidProps(baseResult.params),
        toFiniteNumber(baseResult.params.initialGasSaturation, 0),
        derived.time,
        baseResult.rateHistory.map((point) => Math.max(0, toFiniteNumber(point.total_injection, 0))),
        poreVolume,
    );

    const gasCut = analyticalProduction.map((point) => {
        const total = Math.max(0, point.oilRate + point.gasRate);
        return total > 1e-12 ? point.gasRate / total : 0;
    });
    const recovery = analyticalProduction.map((point) => (
        ooip > 1e-12 ? Math.max(0, Math.min(1, point.cumulativeOil / ooip)) : null
    ));

    return {
        rates: { label: 'Reference Solution Gas Cut', values: gasCut },
        cumulative: {
            recoveryLabel: 'Reference Solution Recovery',
            recoveryValues: recovery,
            cumulativeLabel: 'Reference Solution Cum Oil',
            cumulativeValues: analyticalProduction.map((point) => point.cumulativeOil),
        },
        diagnostics: null,
        xValues: buildXAxisValues(derived, xAxisMode),
    };
}

// ─── Chart Visual Convention ──────────────────────────────────────────────────
//
//  Solid line   = simulation result (primary output from IMPES solver)
//  Dashed line  = analytical reference (Craig, Dykstra-Parsons, Buckley-Leverett, etc.)
//  Color        = sensitivity variant / case index (CASE_COLORS[index])
//
//  Sweep panel — all curves are analytical (no simulation sweep efficiency output
//  exists yet). Within a single variant color, dash PATTERN distinguishes curve type:
//    E_A  (areal):    [7, 4]  medium dash,  weight 2.0
//    E_V  (vertical): [3, 4]  short dash,   weight 1.6  (hidden by default)
//    E_vol (combined):[12, 4] long dash,    weight 2.4  ← boldest = key result
//
//  Future: add simulation sweep efficiency (E_A_sim computed from saturation
//  snapshots) as solid lines alongside these analytical dashed curves — see TODO.md.
// ─────────────────────────────────────────────────────────────────────────────


function mapSweepTimeToPvi(result: BenchmarkRunResult, time: number): number | null {
    const idx = result.rateHistory.findIndex((point) => Number(point.time) >= time - 1e-9);
    if (idx >= 0) {
        return result.pviSeries[idx] ?? null;
    }
    return result.pviSeries.at(-1) ?? null;
}

function dedupeSweepSeries(points: XYPoint[]): XYPoint[] {
    const deduped: XYPoint[] = [];
    for (const point of points) {
        const previous = deduped.at(-1);
        if (previous && Math.abs(previous.x - point.x) <= 1e-9) {
            if (deduped.length === 1 && Math.abs(previous.x) <= 1e-9) {
                continue;
            }
            previous.y = point.y;
            continue;
        }
        deduped.push({ ...point });
    }
    return deduped;
}

function buildSimulationSweepSeries(
    result: BenchmarkRunResult,
    xAxisMode: RateChartXAxisMode,
    tau: number | null,
    derived: DerivedRunSeries,
    geometry: SweepGeometry,
): {
    rf: XYPoint[];
    areal: XYPoint[];
    vertical: XYPoint[];
    combined: XYPoint[];
    combinedMobileOil: XYPoint[];
    showAreal: boolean;
    showVertical: boolean;
} {
    const visibility = getSweepComponentVisibility(geometry);

    const areal: XYPoint[] = geometry === 'both' ? [] : [{ x: 0, y: 0 }];
    const vertical: XYPoint[] = geometry === 'both' ? [] : [{ x: 0, y: 0 }];
    const combined: XYPoint[] = [{ x: 0, y: 0 }];
    const combinedMobileOil: XYPoint[] = geometry === 'both' ? [{ x: 0, y: 0 }] : [];

    for (let i = 0; i < result.rateHistory.length; i++) {
        const sweep = result.rateHistory[i].sweep;
        if (!sweep) continue;
        const pvi = result.pviSeries[i] ?? null;
        if (pvi == null || !Number.isFinite(pvi)) continue;
        const selectedXAxis = mapPviSeriesToXAxis([pvi], derived, xAxisMode, tau)[0];
        if (!Number.isFinite(selectedXAxis)) continue;
        // Component metrics (e_a, e_v) are only meaningful for non-'both' geometry;
        // for 'both' the Rust simulator leaves them null.
        if (geometry !== 'both' && sweep.e_a != null) {
            areal.push({ x: Number(selectedXAxis), y: sweep.e_a });
        }
        if (geometry !== 'both' && sweep.e_v != null) {
            vertical.push({ x: Number(selectedXAxis), y: sweep.e_v });
        }
        combined.push({ x: Number(selectedXAxis), y: sweep.e_vol });
        if (geometry === 'both' && sweep.mobile_oil_recovered != null) {
            combinedMobileOil.push({ x: Number(selectedXAxis), y: sweep.mobile_oil_recovered });
        }
    }

    const sweepRfXValues = mapPviSeriesToXAxis(result.pviSeries, derived, xAxisMode, tau);

    return {
        rf: dedupeSweepSeries(toXYSeries(sweepRfXValues, result.recoverySeries)),
        areal: visibility.showAreal ? dedupeSweepSeries(areal) : [],
        vertical: visibility.showVertical ? dedupeSweepSeries(vertical) : [],
        combined: dedupeSweepSeries(combined),
        combinedMobileOil: geometry === 'both' ? dedupeSweepSeries(combinedMobileOil) : [],
        showAreal: visibility.showAreal,
        showVertical: visibility.showVertical,
    };
}

function buildAnalyticalSweepSeries(
    params: Record<string, any>,
    derived: DerivedRunSeries,
    xAxisMode: RateChartXAxisMode,
    tau: number | null,
    geometry: SweepGeometry,
    method: SweepAnalyticalMethod,
): {
    xValues: Array<number | null>;
    rf: Array<number | null>;
    areal: Array<number | null>;
    vertical: Array<number | null>;
    combined: Array<number | null>;
    showAreal: boolean;
    showVertical: boolean;
} {
    const rock = extractRockProps(params);
    const fluid = extractFluidProps(params);
    const permeabilities = getLayerPermeabilities(params);
    const thickness = getAverageLayerThickness(params);
    const visibility = getSweepComponentVisibility(geometry);
    const sweep = computeCombinedSweep(rock, fluid, permeabilities, thickness, 3.0, 200, geometry, method);
    const recovery = computeSweepRecoveryFactor(rock, fluid, permeabilities, thickness, 3.0, 200, geometry, method);
    const xValues = mapPviSeriesToXAxis(sweep.arealSweep.curve.map((point) => point.pvi), derived, xAxisMode, tau);
    return {
        xValues,
        rf: recovery.curve.map((point) => point.rfSweep),
        areal: sweep.arealSweep.curve.map((point) => point.efficiency),
        vertical: sweep.verticalSweep.curve.map((point) => point.efficiency),
        combined: sweep.combined.map((point) => point.efficiency),
        showAreal: visibility.showAreal,
        showVertical: visibility.showVertical,
    };
}

function appendAnalyticalSweepCurves(
    panels: Record<keyof ReferenceComparisonSweepPanels, ReferenceComparisonPanel>,
    input: {
        label: string;
        caseKey?: string;
        toggleLabel: string;
        color: string;
        params: Record<string, any>;
        derived: DerivedRunSeries;
        xAxisMode: RateChartXAxisMode;
        tau: number | null;
        geometry: SweepGeometry;
        theme: ReferenceComparisonTheme;
        method: SweepAnalyticalMethod;
    },
) {
    const analytical = buildAnalyticalSweepSeries(input.params, input.derived, input.xAxisMode, input.tau, input.geometry, input.method);
    const toggleGroupKey = input.caseKey ? `${input.caseKey}__ref` : 'analytical';
    const legendColor = input.caseKey ? undefined : getLegendGrey(input.theme);
    const borderWidth = input.caseKey ? 1.5 : 2;
    const analyticalRfLabel = input.method === 'stiles'
        ? `${input.label} — Analytical Total RF (Stiles Layered BL)`
        : `${input.label} — Analytical Total RF (Dykstra-Parsons)`;
    const analyticalEvolLabel = input.method === 'stiles'
        ? `${input.label} — Analytical Total E_vol (Stiles Layered BL)`
        : `${input.label} — Analytical Total E_vol (Dykstra-Parsons)`;

    appendSeries(panels.rf, {
        label: analyticalRfLabel,
        curveKey: 'sweep-rf-reference',
        ...(input.caseKey ? { caseKey: input.caseKey } : {}),
        toggleGroupKey,
        toggleLabel: input.toggleLabel,
        legendSection: 'analytical',
        legendSectionLabel: LEGEND_SECTIONS.analytical,
        color: input.color,
        ...(legendColor ? { legendColor } : {}),
        borderWidth,
        borderDash: ANALYTICAL_DASH,
        yAxisID: 'y',
    }, analytical.xValues, analytical.rf);

    if (analytical.showAreal) {
        appendSeries(panels.areal, {
            label: `${input.label} — Analytical E_A (diagnostic decomposition)`,
            curveKey: 'sweep-areal-reference',
            ...(input.caseKey ? { caseKey: input.caseKey } : {}),
            toggleGroupKey,
            toggleLabel: input.toggleLabel,
            legendSection: 'analytical',
            legendSectionLabel: LEGEND_SECTIONS.analytical,
            color: input.color,
            ...(legendColor ? { legendColor } : {}),
            borderWidth,
            borderDash: SWEEP_DASH_AREAL,
            yAxisID: 'y',
        }, analytical.xValues, analytical.areal);
    }

    if (analytical.showVertical) {
        appendSeries(panels.vertical, {
            label: `${input.label} — Analytical E_V (diagnostic decomposition)`,
            curveKey: 'sweep-vertical-reference',
            ...(input.caseKey ? { caseKey: input.caseKey } : {}),
            toggleGroupKey,
            toggleLabel: input.toggleLabel,
            legendSection: 'analytical',
            legendSectionLabel: LEGEND_SECTIONS.analytical,
            color: input.color,
            ...(legendColor ? { legendColor } : {}),
            borderWidth,
            borderDash: SWEEP_DASH_VERTICAL,
            yAxisID: 'y',
        }, analytical.xValues, analytical.vertical);
    }

    appendSeries(panels.combined, {
        label: analyticalEvolLabel,
        curveKey: 'sweep-combined-reference',
        ...(input.caseKey ? { caseKey: input.caseKey } : {}),
        toggleGroupKey,
        toggleLabel: input.toggleLabel,
        legendSection: 'analytical',
        legendSectionLabel: LEGEND_SECTIONS.analytical,
        color: input.color,
        ...(legendColor ? { legendColor } : {}),
        borderWidth,
        borderDash: SWEEP_DASH_COMBINED,
        yAxisID: 'y',
    }, analytical.xValues, analytical.combined);

    if (input.geometry === 'both') {
        appendSeries(panels.combinedMobileOil, {
            label: analyticalEvolLabel,
            curveKey: 'sweep-combined-reference',
            ...(input.caseKey ? { caseKey: input.caseKey } : {}),
            toggleGroupKey,
            toggleLabel: input.toggleLabel,
            legendSection: 'analytical',
            legendSectionLabel: LEGEND_SECTIONS.analytical,
            color: input.color,
            ...(legendColor ? { legendColor } : {}),
            borderWidth,
            borderDash: SWEEP_DASH_COMBINED,
            yAxisID: 'y',
        }, analytical.xValues, analytical.combined);
    }
}

function appendSimulationSweepCurves(
    panels: Record<keyof ReferenceComparisonSweepPanels, ReferenceComparisonPanel>,
    result: BenchmarkRunResult,
    color: string,
    xAxisMode: RateChartXAxisMode,
    tau: number | null,
    derived: DerivedRunSeries,
    geometry: SweepGeometry,
) {
    const simulation = buildSimulationSweepSeries(result, xAxisMode, tau, derived, geometry);
    const caseLabel = compactCaseLabel(result.label);

    if (simulation.rf.length > 0) {
        panels.rf.curves.push({
            label: `${result.label} RF`,
            curveKey: 'sweep-rf-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.rf.series.push(simulation.rf);
    }

    if (simulation.areal.length > 0) {
        panels.areal.curves.push({
            label: `${result.label} E_A`,
            curveKey: 'sweep-areal-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.areal.series.push(simulation.areal);
    }

    if (simulation.vertical.length > 0) {
        panels.vertical.curves.push({
            label: `${result.label} E_V`,
            curveKey: 'sweep-vertical-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.vertical.series.push(simulation.vertical);
    }

    if (simulation.combined.length > 0) {
        panels.combined.curves.push({
            label: `${result.label} E_vol`,
            curveKey: 'sweep-combined-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: simBorderWidth(result.variantKey),
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.combined.series.push(simulation.combined);
    }

    if (simulation.combinedMobileOil.length > 0) {
        panels.combinedMobileOil.curves.push({
            label: `${result.label} Mobile Oil Recovered`,
            curveKey: 'sweep-combined-mobile-oil-sim',
            caseKey: result.key,
            toggleGroupKey: result.key,
            toggleLabel: caseLabel,
            legendSection: 'sim',
            legendSectionLabel: LEGEND_SECTIONS.sim,
            color,
            borderWidth: 1.8,
            borderDash: [3, 3],
            yAxisID: 'y',
            defaultVisible: true,
        });
        panels.combinedMobileOil.series.push(simulation.combinedMobileOil);
    }
}

/**
 * Build analytical-only preview panels before any simulation results exist.
 *
 * Accepts an array of variant entries so multiple colored curves can be rendered
 * when the selected sensitivity dimension affects the analytical solution (e.g.
 * mobility ratio). Each entry is computed independently — a numerical failure in
 * one variant is silently skipped rather than aborting the whole preview.
 *
 * Single-entry arrays use the neutral reference color (current base-preview
 * behavior). Multi-entry arrays use per-variant case colors.
 */

function buildAnalyticalPreviewPanels(
    variants: AnalyticalPreviewVariant[],
    xAxisMode: RateChartXAxisMode,
    analyticalMethod: string,
    theme: ReferenceComparisonTheme,
): Record<RateChartPanelKey, ReferenceComparisonPanel> {
    const panels: Record<RateChartPanelKey, ReferenceComparisonPanel> = {
        rates: { curves: [], series: [] },
        recovery: { curves: [], series: [] },
        cumulative: { curves: [], series: [] },
        diagnostics: { curves: [], series: [] },
        gor: { curves: [], series: [] },
        volumes: { curves: [], series: [] },
        oil_rate: { curves: [], series: [] },
        injection_rate: { curves: [], series: [] },
        producer_bhp: { curves: [], series: [] },
        injector_bhp: { curves: [], series: [] },
        control_limits: { curves: [], series: [] },
    };

    if (variants.length === 0) return panels;

    const multiVariant = variants.length > 1;
    const neutralColor = getReferenceColor(theme);
    const legendGrey = getLegendGrey(theme);

    const getColor = (index: number) =>
        multiVariant ? getReferenceComparisonCaseColor(index) : neutralColor;

    const labelPrefix = (variant: AnalyticalPreviewVariant) =>
        multiVariant ? `${variant.label} — ` : '';

    const analyticalLabel = variants.length === 1
        ? 'Analytical solution'
        : `Analytical solution (${variants.length})`;

    if (analyticalMethod === 'buckley-leverett' || analyticalMethod === 'waterflood') {
        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;

            const curves = computeBLAnalyticalFromParams(variant.params);
            if (!curves) return; // numerical failure — skip, don't abort other variants

            appendSeries(panels.rates, {
                label: `${prefix}Analytical Water Cut`,
                curveKey: 'water-cut-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.waterCut);
            appendSeries(panels.recovery, {
                label: `${prefix}Analytical Recovery Factor`,
                curveKey: 'recovery-factor-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.recovery);
        });

        return panels;
    }

    if (analyticalMethod === 'depletion') {
        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;

            const curves = computeDepletionAnalyticalFromParams(variant.params, xAxisMode);
            if (!curves) return; // bad params or numerical failure — skip, don't abort others

            appendSeries(panels.rates, {
                label: `${prefix}Analytical Oil Rate`,
                curveKey: 'oil-rate-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.oilRates);
            appendSeries(panels.recovery, {
                label: `${prefix}Analytical Recovery Factor`,
                curveKey: 'recovery-factor-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.recoveryValues);
            appendSeries(panels.cumulative, {
                label: `${prefix}Analytical Cum Oil`,
                curveKey: 'cum-oil-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.cumulativeOilValues);
            appendSeries(panels.diagnostics, {
                label: `${prefix}Analytical Avg Pressure`,
                curveKey: 'avg-pressure-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.xValues, curves.avgPressureValues);
        });

        return panels;
    }

    if (analyticalMethod === 'gas-oil-bl') {
        variants.forEach((variant, index) => {
            const color = getColor(index);
            const prefix = labelPrefix(variant);
            const caseKey = multiVariant ? variant.variantKey : undefined;

            const curves = computeGasOilBLAnalyticalFromParams(variant.params);
            if (!curves) return;

            appendSeries(panels.rates, {
                label: `${prefix}Analytical Gas Cut`,
                curveKey: 'gas-cut-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.pviValues, curves.gasCut);
            appendSeries(panels.recovery, {
                label: `${prefix}Analytical Recovery Factor`,
                curveKey: 'recovery-factor-reference',
                ...(caseKey ? { caseKey } : {}),
                toggleGroupKey: 'analytical',
                toggleLabel: analyticalLabel,
                color,
                legendColor: legendGrey,
                borderWidth: ANALYTICAL_BORDER,
                borderDash: ANALYTICAL_DASH,
                yAxisID: 'y',
            }, curves.pviValues, curves.recovery);
        });

        return panels;
    }

    return panels;
}

function buildPreviewSweepPanels(input: {
    variants: AnalyticalPreviewVariant[];
    theme: ReferenceComparisonTheme;
    geometry: SweepGeometry;
    method: SweepAnalyticalMethod;
}): ReferenceComparisonSweepPanels {
    const panels = createSweepPanels();
    const multiVariant = input.variants.length > 1;
    const referenceColor = getReferenceColor(input.theme);
    const previewDerived: DerivedRunSeries = {
        time: [],
        historyTime: [],
        oilRate: [],
        injectionRate: [],
        waterCut: [],
        gasCut: [],
        avgWaterSat: [],
        pressure: [],
        producerBhp: [],
        injectorBhp: [],
        recovery: [],
        cumulativeOil: [],
        cumulativeInjection: [],
        cumulativeLiquid: [],
        cumulativeGas: [],
        p_z: [],
        pvi: [],
        pvp: [],
        gor: [],
        producerBhpLimitedFraction: [],
        injectorBhpLimitedFraction: [],
    };

    input.variants.forEach((variant, index) => {
        const color = multiVariant ? getReferenceComparisonCaseColor(index) : referenceColor;
        const label = variant.label || 'Analytical';
        appendAnalyticalSweepCurves(panels, {
            label,
            ...(multiVariant ? { caseKey: variant.variantKey } : {}),
            toggleLabel: multiVariant ? compactCaseLabel(label) : 'Analytical solution',
            color,
            params: variant.params,
            theme: input.theme,
            derived: previewDerived,
            xAxisMode: 'pvi',
            tau: null,
            geometry: input.geometry,
            method: input.method,
        });
    });

    return finalizeSweepPanels(panels);
}

function buildSweepPanels(input: {
    orderedResults: BenchmarkRunResult[];
    pendingPreviewVariants?: AnalyticalPreviewVariant[];
    previewVariantParams?: AnalyticalPreviewVariant[];
    theme: ReferenceComparisonTheme;
    xAxisMode: RateChartXAxisMode;
    derivedByKey: Map<string, DerivedRunSeries>;
    geometry: SweepGeometry;
    method: SweepAnalyticalMethod;
}): ReferenceComparisonSweepPanels {
    const panels = createSweepPanels();

    input.orderedResults.forEach((result, index) => {
        const color = getReferenceComparisonCaseColor(index);
        const derived = input.derivedByKey.get(result.key);
        if (!derived) return;
        const tau = computeDepletionTau(result.params);
        appendSimulationSweepCurves(panels, result, color, input.xAxisMode, tau, derived, input.geometry);
        appendAnalyticalSweepCurves(panels, {
            label: result.label,
            caseKey: result.key,
            toggleLabel: compactCaseLabel(result.label),
            color,
            params: result.params,
            theme: input.theme,
            derived,
            xAxisMode: input.xAxisMode,
            tau,
            geometry: input.geometry,
            method: input.method,
        });
    });

    if (input.pendingPreviewVariants?.length && input.xAxisMode === 'pvi') {
        const declarationOrder = new Map((input.previewVariantParams ?? []).map((variant, index) => [variant.variantKey, index]));
        const previewDerived = input.orderedResults[0] ? buildDerivedRunSeries(input.orderedResults[0]) : {
            time: [],
            historyTime: [],
            oilRate: [],
            injectionRate: [],
            waterCut: [],
            gasCut: [],
            avgWaterSat: [],
            pressure: [],
            producerBhp: [],
            injectorBhp: [],
            recovery: [],
            cumulativeOil: [],
            cumulativeInjection: [],
            cumulativeLiquid: [],
            cumulativeGas: [],
            p_z: [],
            pvi: [],
            pvp: [],
            gor: [],
            producerBhpLimitedFraction: [],
            injectorBhpLimitedFraction: [],
        };
        input.pendingPreviewVariants.forEach((variant, fallbackIndex) => {
            const colorIndex = declarationOrder.get(variant.variantKey) ?? (input.orderedResults.length + fallbackIndex);
            appendAnalyticalSweepCurves(panels, {
                label: variant.label,
                caseKey: variant.variantKey,
                toggleLabel: compactCaseLabel(variant.label),
                color: getReferenceComparisonCaseColor(colorIndex),
                params: variant.params,
                theme: input.theme,
                derived: previewDerived,
                xAxisMode: input.xAxisMode,
                tau: null,
                geometry: input.geometry,
                method: input.method,
            });
        });
    }

    return finalizeSweepPanels(panels);
}

export function buildReferenceComparisonModel(input: {
    family: BenchmarkFamily | null | undefined;
    results: BenchmarkRunResult[];
    xAxisMode: RateChartXAxisMode;
    theme?: ReferenceComparisonTheme;
    /** True when the active sensitivity variants change parameters that feed the
     *  analytical solution (e.g. viscosity → fractional flow). Each result then
     *  gets its own analytical curve. False (default) → one shared reference. */
    analyticalPerVariant?: boolean;
    /**
     * When provided and no results exist yet, render one analytical curve per
     * variant so the user can see the spread before running any simulations.
     * Takes priority over previewBaseParams when non-empty.
     */
    previewVariantParams?: AnalyticalPreviewVariant[];
    /**
     * Variants whose simulations are still queued/running (results not yet in
     * `results`). Rendered as analytical-only dashed overlays alongside the
     * completed results so the chart never collapses from N preview curves to
     * fewer as the sweep progresses. Colors continue the case-color sequence
     * from where orderedResults leaves off so each variant keeps its color
     * throughout preview → in-progress → completed.
     */
    pendingPreviewVariants?: AnalyticalPreviewVariant[];
    /** Fallback single-curve preview (used when analyticalPerVariant is false). */
    previewBaseParams?: Record<string, any>;
    previewAnalyticalMethod?: string;
}): ReferenceComparisonModel {
    const family = input.family ?? null;
    const suppressPrimaryAnalyticalOverlays = family?.suppressPrimaryAnalyticalOverlays
        ?? (family?.showSweepPanel === true);
    const orderedResults = orderResults(input.results, input.previewVariantParams);
    const referenceColor = getReferenceColor(input.theme ?? 'dark');
    const legendGrey = getLegendGrey(input.theme ?? 'dark');
    const analyticalMethod = family?.analyticalMethod ?? input.previewAnalyticalMethod ?? null;
    const requestedOverlayMode = family?.analyticalOverlayMode ?? 'auto';
    const usesRunMappedAnalyticalXAxis = requiresRunMappedAnalyticalXAxis(
        analyticalMethod,
        input.xAxisMode,
    );
    const distinctBuckleyLeverettOverlays = hasDistinctBuckleyLeverettOverlays([
        ...orderedResults.map((result) => result.params),
        ...(input.pendingPreviewVariants ?? []).map((variant) => variant.params),
    ]);
    const distinctGasOilBLOutlays = hasDistinctGasOilBLOutlays([
        ...orderedResults.map((result) => result.params),
        ...(input.pendingPreviewVariants ?? []).map((variant) => variant.params),
    ]);
    const buckleyLeverettOverlayMode = resolveOverlayMode({
        requested: requestedOverlayMode,
        distinctByPhysics: distinctBuckleyLeverettOverlays,
    });
    const gasOilOverlayMode = resolveOverlayMode({
        requested: requestedOverlayMode,
        distinctByPhysics: distinctGasOilBLOutlays,
    });
    const depletionOverlayMode = resolveOverlayMode({
        requested: requestedOverlayMode,
        distinctByPhysics: false,
        analyticalPerVariant: input.analyticalPerVariant,
    });
    let hidesPendingAnalyticalWithoutMapping = false;

    const panels: Record<RateChartPanelKey, ReferenceComparisonPanel> = {
        rates: { curves: [], series: [] },
        recovery: { curves: [], series: [] },
        cumulative: { curves: [], series: [] },
        diagnostics: { curves: [], series: [] },
        gor: { curves: [], series: [] },
        volumes: { curves: [], series: [] },
        oil_rate: { curves: [], series: [] },
        injection_rate: { curves: [], series: [] },
        producer_bhp: { curves: [], series: [] },
        injector_bhp: { curves: [], series: [] },
        control_limits: { curves: [], series: [] },
    };

    if (!family || orderedResults.length === 0) {
        if (orderedResults.length === 0 && input.previewAnalyticalMethod) {
            if (requiresRunMappedAnalyticalXAxis(input.previewAnalyticalMethod, input.xAxisMode)) {
                hidesPendingAnalyticalWithoutMapping = Boolean(
                    input.previewBaseParams || (input.previewVariantParams?.length ?? 0) > 0,
                );
                return {
                    orderedResults,
                    previewCases: [],
                    panels: (() => {
                        appendPublishedReferenceSeries(panels, family);
                        return combinePanelMaps({ primary: panels });
                    })(),
                    axisMappingWarning: buildAnalyticalAxisWarning({
                        usesRunMappedAnalyticalXAxis: false,
                        hidesPendingAnalyticalWithoutMapping,
                    }),
                };
            }
            // Prefer per-variant preview when available; fall back to single base preview.
            const variants: AnalyticalPreviewVariant[] =
                input.previewVariantParams?.length
                    ? input.previewVariantParams
                    : input.previewBaseParams
                        ? [{ label: '', variantKey: 'base', params: input.previewBaseParams }]
                        : [];
            if (variants.length > 0) {
                const analyticalPreviewVariants = input.previewAnalyticalMethod === 'buckley-leverett'
                    && !usesRunMappedAnalyticalXAxis
                    && buckleyLeverettOverlayMode === 'shared'
                    ? [variants[0]]
                    : input.previewAnalyticalMethod === 'gas-oil-bl'
                    && !usesRunMappedAnalyticalXAxis
                    && gasOilOverlayMode === 'shared'
                    ? [variants[0]]
                    : variants;
                const previewPanels = suppressPrimaryAnalyticalOverlays
                    ? panels
                    : buildAnalyticalPreviewPanels(
                        analyticalPreviewVariants,
                        input.xAxisMode,
                        input.previewAnalyticalMethod,
                        input.theme ?? 'dark',
                    );
                // Expose multi-variant preview entries so the cases selector can
                // render toggle buttons even before any simulations have completed.
                const previewCases: ReferenceComparisonPreviewCase[] = variants.length > 1
                    ? variants.map((v, i) => ({ key: v.variantKey, label: v.label, colorIndex: i }))
                    : [];
                return {
                    orderedResults,
                    previewCases,
                    panels: (() => {
                        const primaryPanels = suppressPrimaryAnalyticalOverlays
                            ? suppressPrimaryAnalyticalPanels(previewPanels)
                            : previewPanels;
                        appendPublishedReferenceSeries(primaryPanels, family);
                        return combinePanelMaps({
                            primary: primaryPanels,
                            sweep: family?.showSweepPanel === true
                                ? buildPreviewSweepPanels({
                                    variants,
                                    theme: input.theme ?? 'dark',
                                    geometry: family?.sweepGeometry ?? 'both',
                                    method: family?.sweepAnalyticalMethod ?? 'dykstra-parsons',
                                })
                                : emptySweepPanels(),
                        });
                    })(),
                    axisMappingWarning: null,
                };
            }
        }
        appendPublishedReferenceSeries(panels, family);
        return {
            orderedResults,
            previewCases: [],
            panels: combinePanelMaps({ primary: panels }),
            axisMappingWarning: null,
        };
    }

    const derivedByKey = new Map<string, DerivedRunSeries>(
        orderedResults.map((result) => [result.key, buildDerivedRunSeries(result)]),
    );
    const baseResult = getBaseResult(orderedResults);

    orderedResults.forEach((result, index) => {
        const derived = derivedByKey.get(result.key);
        if (!derived) return;
        const color = getReferenceComparisonCaseColor(index);
        const tau = analyticalMethod === 'depletion' ? computeDepletionTau(result.params) : null;
        const xValues = buildXAxisValues(derived, input.xAxisMode, tau);
        const defaultVisible = true;

        const caseLabel = compactCaseLabel(result.label);

        if (family.analyticalMethod === 'buckley-leverett') {
            appendSeries(
                panels.rates,
                {
                    label: `${result.label} Water Cut`,
                    curveKey: 'water-cut-sim',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: simBorderWidth(result.variantKey),
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.waterCut,
            );
            appendSeries(
                panels.rates,
                {
                    label: `${result.label} Avg Water Sat`,
                    curveKey: 'avg-water-sat',
                    caseKey: result.key,
                    // No toggleGroupKey override — falls back to curveKey so all cases
                    // share one "Avg Sw" toggle, keeping it out of the per-case section.
                    toggleLabel: 'Avg Sw',
                    color,
                    borderWidth: 1.6,
                    borderDash: AUXILIARY_DASH,
                    yAxisID: 'y',
                    defaultVisible: false,
                },
                xValues,
                derived.avgWaterSat,
            );
            appendSeries(
                panels.recovery,
                {
                    label: `${result.label} Recovery`,
                    curveKey: 'recovery-factor-primary',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: simBorderWidth(result.variantKey),
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.recovery,
            );
            appendSeries(
                panels.cumulative,
                {
                    label: `${result.label} Cum Oil`,
                    curveKey: 'cum-oil-sim',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: simBorderWidth(result.variantKey),
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.cumulativeOil,
            );
            appendSeries(
                panels.oil_rate,
                {
                    label: `${result.label} Oil Rate`,
                    curveKey: 'oil-rate-sim',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: simBorderWidth(result.variantKey),
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.oilRate,
            );
            appendSeries(
                panels.injection_rate,
                {
                    label: `${result.label} Injection Rate`,
                    curveKey: 'injection-rate-sim',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: simBorderWidth(result.variantKey),
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.injectionRate,
            );
            appendSeries(
                panels.volumes,
                {
                    label: `${result.label} Cum Injection`,
                    curveKey: 'cum-injection',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: simBorderWidth(result.variantKey),
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.cumulativeInjection,
            );
            appendSeries(
                panels.diagnostics,
                {
                    label: `${result.label} Avg Pressure`,
                    curveKey: 'avg-pressure-sim',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: simBorderWidth(result.variantKey),
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.pressure,
            );
            appendSeries(
                panels.diagnostics,
                {
                    label: `${result.label} P/z`,
                    curveKey: 'p_z_sim',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: simBorderWidth(result.variantKey),
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.p_z,
            );
            appendSeries(
                panels.gor,
                {
                    label: `${result.label} GOR`,
                    curveKey: 'gor-sim',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: simBorderWidth(result.variantKey),
                    yAxisID: 'y',
                    defaultVisible,
                },
                xValues,
                derived.gor,
            );
            return;
        }

        if (family.analyticalMethod === 'gas-oil-bl') {
            const historyXAxis = interpolateXAxisAtTimes(derived.time, xValues, derived.historyTime);
            appendSeries(panels.rates, {
                label: `${result.label} Gas Cut`,
                curveKey: 'gas-cut-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.gasCut);
            appendSeries(panels.recovery, {
                label: `${result.label} Recovery`,
                curveKey: 'recovery-factor-primary',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.recovery);
            appendSeries(panels.cumulative, {
                label: `${result.label} Cum Oil`,
                curveKey: 'cum-oil-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.cumulativeOil);
            appendSeries(panels.oil_rate, {
                label: `${result.label} Oil Rate`,
                curveKey: 'oil-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.oilRate);
            appendSeries(panels.injection_rate, {
                label: `${result.label} Injection Rate`,
                curveKey: 'injection-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.injectionRate);
            appendSeries(panels.volumes, {
                label: `${result.label} Cum Injection`,
                curveKey: 'cum-injection',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.cumulativeInjection);
            appendSeries(panels.diagnostics, {
                label: `${result.label} Avg Pressure`,
                curveKey: 'avg-pressure-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.pressure);
            appendSeries(panels.diagnostics, {
                label: `${result.label} P/z`,
                curveKey: 'p_z_sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.p_z);
            appendSeries(panels.gor, {
                label: `${result.label} GOR`,
                curveKey: 'gor-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, xValues, derived.gor);
            appendSeries(panels.producer_bhp, {
                label: `${result.label} Producer WBHP`,
                curveKey: 'producer-bhp-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, historyXAxis, derived.producerBhp);
            appendSeries(panels.injector_bhp, {
                label: `${result.label} Injector WBHP`,
                curveKey: 'injector-bhp-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            }, historyXAxis, derived.injectorBhp);
            appendBhpLimitDiagnostics(panels.control_limits, {
                label: result.label,
                caseKey: result.key,
                toggleLabel: caseLabel,
                borderWidth: simBorderWidth(result.variantKey),
                defaultVisible,
                xValues,
                producerValues: derived.producerBhpLimitedFraction,
                injectorValues: derived.injectorBhpLimitedFraction,
            });
            return;
        }

        appendSeries(
            panels.rates,
            {
                label: `${result.label} Oil Rate`,
                curveKey: 'oil-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.oilRate,
        );
        appendSeries(
            panels.recovery,
            {
                label: `${result.label} Recovery`,
                curveKey: 'recovery-factor-primary',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.recovery,
        );
        appendSeries(
            panels.cumulative,
            {
                label: `${result.label} Cum Oil`,
                curveKey: 'cum-oil-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
            },
            xValues,
            derived.cumulativeOil,
        );
        appendSeries(
            panels.oil_rate,
            {
                label: `${result.label} Oil Rate`,
                curveKey: 'oil-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.oilRate,
        );
        appendSeries(
            panels.injection_rate,
            {
                label: `${result.label} Injection Rate`,
                curveKey: 'injection-rate-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.injectionRate,
        );
        appendSeries(
            panels.diagnostics,
            {
                label: `${result.label} Avg Pressure`,
                curveKey: 'avg-pressure-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.pressure,
        );
        appendSeries(
            panels.diagnostics,
            {
                label: `${result.label} P/z`,
                curveKey: 'p_z_sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.p_z,
        );
        appendSeries(
            panels.gor,
            {
                label: `${result.label} GOR`,
                curveKey: 'gor-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            },
            xValues,
            derived.gor,
        );
        const historyXAxis = interpolateXAxisAtTimes(derived.time, xValues, derived.historyTime);
        appendSeries(
            panels.producer_bhp,
            {
                label: `${result.label} Producer WBHP`,
                curveKey: 'producer-bhp-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            },
            historyXAxis,
            derived.producerBhp,
        );
        appendSeries(
            panels.injector_bhp,
            {
                label: `${result.label} Injector WBHP`,
                curveKey: 'injector-bhp-sim',
                caseKey: result.key,
                toggleGroupKey: result.key,
                toggleLabel: caseLabel,
                legendSection: 'sim',
                legendSectionLabel: LEGEND_SECTIONS.sim,
                color,
                borderWidth: simBorderWidth(result.variantKey),
                yAxisID: 'y',
                defaultVisible,
            },
            historyXAxis,
            derived.injectorBhp,
        );
        appendBhpLimitDiagnostics(panels.control_limits, {
            label: result.label,
            caseKey: result.key,
            toggleLabel: caseLabel,
            borderWidth: simBorderWidth(result.variantKey),
            defaultVisible,
            xValues,
            producerValues: derived.producerBhpLimitedFraction,
            injectorValues: derived.injectorBhpLimitedFraction,
        });

        // ── MBE diagnostics (Havlena-Odeh) ─────────────────────────
        if (analyticalMethod === 'depletion') {
            const mbe = computeMbeDiagnostics(result, derived);
            appendSeries(
                panels.diagnostics,
                {
                    label: `${result.label} MBE OOIP Ratio`,
                    curveKey: 'mbe-ooip-ratio',
                    caseKey: result.key,
                    toggleGroupKey: result.key,
                    toggleLabel: caseLabel,
                    legendSection: 'sim',
                    legendSectionLabel: LEGEND_SECTIONS.sim,
                    color,
                    borderWidth: 1.6,
                    borderDash: [2, 3],
                    yAxisID: 'y1',
                    defaultVisible: false,
                },
                xValues,
                mbe.ooipRatio,
            );

            // ── Drive mechanism indices ─────────────────────────────
            appendSeries(
                panels.diagnostics,
                {
                    label: `${result.label} Drive: Compaction`,
                    curveKey: 'drive-compaction',
                    caseKey: result.key,
                    toggleGroupKey: `${result.key}-drive`,
                    toggleLabel: caseLabel,
                    legendSection: 'drive',
                    legendSectionLabel: LEGEND_SECTIONS.driveIndices,
                    color: '#e67e22',
                    borderWidth: 1.4,
                    yAxisID: 'y1',
                    defaultVisible: false,
                },
                xValues,
                mbe.driveCompaction,
            );
            appendSeries(
                panels.diagnostics,
                {
                    label: `${result.label} Drive: Oil Expansion`,
                    curveKey: 'drive-oil-expansion',
                    caseKey: result.key,
                    toggleGroupKey: `${result.key}-drive`,
                    toggleLabel: caseLabel,
                    legendSection: 'drive',
                    legendSectionLabel: LEGEND_SECTIONS.driveIndices,
                    color: '#27ae60',
                    borderWidth: 1.4,
                    yAxisID: 'y1',
                    defaultVisible: false,
                },
                xValues,
                mbe.driveOilExpansion,
            );
            appendSeries(
                panels.diagnostics,
                {
                    label: `${result.label} Drive: Gas Cap`,
                    curveKey: 'drive-gas-cap',
                    caseKey: result.key,
                    toggleGroupKey: `${result.key}-drive`,
                    toggleLabel: caseLabel,
                    legendSection: 'drive',
                    legendSectionLabel: LEGEND_SECTIONS.driveIndices,
                    color: '#2980b9',
                    borderWidth: 1.4,
                    yAxisID: 'y1',
                    defaultVisible: false,
                },
                xValues,
                mbe.driveGasCap,
            );
        }
    });

    if (!baseResult) {
        return {
            orderedResults,
            previewCases: [],
            panels: combinePanelMaps({ primary: panels }),
            axisMappingWarning: buildAnalyticalAxisWarning({
                usesRunMappedAnalyticalXAxis,
                hidesPendingAnalyticalWithoutMapping,
            }),
        };
    }

    const baseDerived = derivedByKey.get(baseResult.key);
    if (!baseDerived) {
        return {
            orderedResults,
            previewCases: [],
            panels: combinePanelMaps({ primary: panels }),
            axisMappingWarning: buildAnalyticalAxisWarning({
                usesRunMappedAnalyticalXAxis,
                hidesPendingAnalyticalWithoutMapping,
            }),
        };
    }

    if (family.analyticalMethod === 'buckley-leverett' && !suppressPrimaryAnalyticalOverlays) {
        const allSameAnalytical = buckleyLeverettOverlayMode === 'shared';

        if (allSameAnalytical && !usesRunMappedAnalyticalXAxis) {
            // Shared reference — one curve for all (analytical is grid/timestep-independent).
            const refOverlay = buildBuckleyLeverettReference(baseResult, baseDerived, input.xAxisMode);
            if (refOverlay.rates) {
                appendSeries(panels.rates, {
                    label: 'Reference Solution Water Cut',
                    curveKey: 'water-cut-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.rates.values);
            }
            if (refOverlay.cumulative) {
                appendSeries(panels.recovery, {
                    label: 'Reference Solution Recovery',
                    curveKey: 'recovery-factor-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
            }
        } else {
            // Per-result analytical — either the analytical physics differs by case,
            // or the selected x-axis requires remapping the same PVI solution onto
            // each completed run's own time/injection history.
            orderedResults.forEach((result, index) => {
                const derived = derivedByKey.get(result.key);
                if (!derived) return;
                const color = getReferenceComparisonCaseColor(index);
                const caseLabel = compactCaseLabel(result.label);
                const refOverlay = buildBuckleyLeverettReference(result, derived, input.xAxisMode);
                if (refOverlay.rates) {
                    appendSeries(panels.rates, {
                        label: `${result.label} — Reference`,
                        curveKey: 'water-cut-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.rates.values);
                }
                if (refOverlay.cumulative) {
                    appendSeries(panels.recovery, {
                        label: `${result.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                }
            });

            // Analytical-only overlay for variants still queued/running.
            // Color indices continue from orderedResults.length so each variant
            // keeps the same color from initial preview → in-progress → completed.
            // Uses the same unit-PVI grid as the all-analytical preview for a
            // consistent x-axis (BL scenarios default to PVI).
            if (input.pendingPreviewVariants?.length) {
                if (usesRunMappedAnalyticalXAxis) {
                    hidesPendingAnalyticalWithoutMapping = true;
                }
                if (!usesRunMappedAnalyticalXAxis) {
                input.pendingPreviewVariants.forEach((variant, i) => {
                    const color = getReferenceComparisonCaseColor(orderedResults.length + i);
                    const curves = computeBLAnalyticalFromParams(variant.params);
                    if (!curves) return; // bad params — skip this variant
                    const vLabel = compactCaseLabel(variant.label);
                    appendSeries(panels.rates, {
                        label: `${variant.label} — Reference`,
                        curveKey: 'water-cut-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, curves.xValues, curves.waterCut);
                    appendSeries(panels.recovery, {
                        label: `${variant.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, curves.xValues, curves.recovery);
                });
                }
            }
        }
    } else if (family.analyticalMethod === 'gas-oil-bl') {
        const allSameAnalytical = gasOilOverlayMode === 'shared';

        if (allSameAnalytical && !usesRunMappedAnalyticalXAxis) {
            const refOverlay = buildGasOilBLReference(baseResult, baseDerived, input.xAxisMode);
            if (refOverlay.rates) {
                appendSeries(panels.rates, {
                    label: 'Reference Solution Gas Cut',
                    curveKey: 'gas-cut-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.rates.values);
            }
            if (refOverlay.cumulative) {
                appendSeries(panels.recovery, {
                    label: 'Reference Solution Recovery',
                    curveKey: 'recovery-factor-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                appendSeries(panels.cumulative, {
                    label: 'Reference Solution Cum Oil',
                    curveKey: 'cum-oil-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.cumulativeValues);
            }
        } else {
            orderedResults.forEach((result, index) => {
                const derived = derivedByKey.get(result.key);
                if (!derived) return;
                const color = getReferenceComparisonCaseColor(index);
                const caseLabel = compactCaseLabel(result.label);
                const refOverlay = buildGasOilBLReference(result, derived, input.xAxisMode);
                if (refOverlay.rates) {
                    appendSeries(panels.rates, {
                        label: `${result.label} — Reference`,
                        curveKey: 'gas-cut-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.rates.values);
                }
                if (refOverlay.cumulative) {
                    appendSeries(panels.recovery, {
                        label: `${result.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                }
            });

            if (input.pendingPreviewVariants?.length) {
                if (usesRunMappedAnalyticalXAxis) {
                    hidesPendingAnalyticalWithoutMapping = true;
                }
                if (!usesRunMappedAnalyticalXAxis) {
                    input.pendingPreviewVariants.forEach((variant, i) => {
                        const color = getReferenceComparisonCaseColor(orderedResults.length + i);
                        const curves = computeGasOilBLAnalyticalFromParams(variant.params);
                        if (!curves) return;
                        const vLabel = compactCaseLabel(variant.label);
                        appendSeries(panels.rates, {
                            label: `${variant.label} — Reference`,
                            curveKey: 'gas-cut-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.pviValues, curves.gasCut);
                        appendSeries(panels.recovery, {
                            label: `${variant.label} — Reference Recovery`,
                            curveKey: 'recovery-factor-reference',
                            caseKey: variant.variantKey,
                            toggleGroupKey: variant.variantKey + '__ref',
                            toggleLabel: vLabel,
                            legendSection: 'analytical',
                            legendSectionLabel: LEGEND_SECTIONS.analytical,
                            color,
                            borderWidth: 1.5,
                            borderDash: ANALYTICAL_DASH,
                            yAxisID: 'y',
                        }, curves.pviValues, curves.recovery);
                    });
                }
            }
        }
    } else {
        // Depletion path.
        if (depletionOverlayMode === 'per-result' || usesRunMappedAnalyticalXAxis) {
            // Per-result analytical — each variant gets its own dashed reference curve
            // in its case color so the user can directly compare sim vs. analytical.
            // This path also handles shared analytical physics on axes whose x-values
            // are derived from each completed run's simulation history.
            orderedResults.forEach((result, index) => {
                const derived = derivedByKey.get(result.key);
                if (!derived) return;
                const color = getReferenceComparisonCaseColor(index);
                const caseLabel = compactCaseLabel(result.label);
                let refOverlay: ReturnType<typeof buildDepletionReference>;
                try {
                    refOverlay = buildDepletionReference(result, derived, input.xAxisMode);
                } catch {
                    return; // bad params — skip this result's reference curve
                }
                if (refOverlay.rates) {
                    appendSeries(panels.rates, {
                        label: `${result.label} — Reference`,
                        curveKey: 'oil-rate-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.rates.values);
                }
                if (refOverlay.cumulative) {
                    appendSeries(panels.recovery, {
                        label: `${result.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                    appendSeries(panels.cumulative, {
                        label: `${result.label} — Reference Cum Oil`,
                        curveKey: 'cum-oil-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.cumulative.cumulativeValues);
                }
                if (refOverlay.diagnostics) {
                    appendSeries(panels.diagnostics, {
                        label: `${result.label} — Reference Pressure`,
                        curveKey: 'avg-pressure-reference',
                        caseKey: result.key,
                        toggleGroupKey: result.key + '__ref',
                        toggleLabel: caseLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, refOverlay.xValues, refOverlay.diagnostics.values);
                }
            });

            // Analytical-only overlay for variants still queued/running (pending).
            // Color indices continue from orderedResults.length so each variant
            // keeps its color throughout preview → in-progress → completed.
            if (input.pendingPreviewVariants?.length) {
                if (usesRunMappedAnalyticalXAxis) {
                    hidesPendingAnalyticalWithoutMapping = true;
                }
                if (!usesRunMappedAnalyticalXAxis) {
                input.pendingPreviewVariants.forEach((variant, i) => {
                    const color = getReferenceComparisonCaseColor(orderedResults.length + i);
                    const curves = computeDepletionAnalyticalFromParams(variant.params, input.xAxisMode);
                    if (!curves) return;
                    const vLabel = compactCaseLabel(variant.label);
                    appendSeries(panels.rates, {
                        label: `${variant.label} — Reference`,
                        curveKey: 'oil-rate-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, curves.xValues, curves.oilRates);
                    appendSeries(panels.recovery, {
                        label: `${variant.label} — Reference Recovery`,
                        curveKey: 'recovery-factor-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, curves.xValues, curves.recoveryValues);
                    appendSeries(panels.cumulative, {
                        label: `${variant.label} — Reference Cum Oil`,
                        curveKey: 'cum-oil-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, curves.xValues, curves.cumulativeOilValues);
                    appendSeries(panels.diagnostics, {
                        label: `${variant.label} — Reference Pressure`,
                        curveKey: 'avg-pressure-reference',
                        caseKey: variant.variantKey,
                        toggleGroupKey: variant.variantKey + '__ref',
                        toggleLabel: vLabel,
                        legendSection: 'analytical',
                        legendSectionLabel: LEGEND_SECTIONS.analytical,
                        color,
                        borderWidth: 1.5,
                        borderDash: ANALYTICAL_DASH,
                        yAxisID: 'y',
                    }, curves.xValues, curves.avgPressureValues);
                });
                }
            }
        } else {
            // Shared reference from base result — one curve for all cases.
            const refOverlay = buildDepletionReference(baseResult, baseDerived, input.xAxisMode);
            if (refOverlay.rates) {
                appendSeries(panels.rates, {
                    label: refOverlay.rates.label,
                    curveKey: 'oil-rate-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.rates.values);
            }
            if (refOverlay.cumulative) {
                appendSeries(panels.recovery, {
                    label: refOverlay.cumulative.recoveryLabel,
                    curveKey: 'recovery-factor-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.recoveryValues);
                appendSeries(panels.cumulative, {
                    label: refOverlay.cumulative.cumulativeLabel,
                    curveKey: 'cum-oil-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.cumulative.cumulativeValues);
            }
            if (refOverlay.diagnostics) {
                appendSeries(panels.diagnostics, {
                    label: refOverlay.diagnostics.label,
                    curveKey: 'avg-pressure-reference',
                    toggleGroupKey: 'analytical-shared',
                    toggleLabel: 'Analytical solution',
                    legendSection: 'analytical',
                    legendSectionLabel: LEGEND_SECTIONS.analytical,
                    color: referenceColor,
                    legendColor: legendGrey,
                    borderWidth: ANALYTICAL_BORDER,
                    borderDash: ANALYTICAL_DASH,
                    yAxisID: 'y',
                }, refOverlay.xValues, refOverlay.diagnostics.values);
            }
        }
    }

    // Pending preview cases for mid-sweep: variants whose results haven't landed yet.
    // These appear in the cases selector alongside completed orderedResults entries.
    // Color indices use declaration order from previewVariantParams so each variant
    // keeps the same color throughout preview → in-progress → completed.
    const pendingPreviewCases: ReferenceComparisonPreviewCase[] =
        (input.pendingPreviewVariants?.length
            && ((input.analyticalPerVariant && !usesRunMappedAnalyticalXAxis)
                || family.showSweepPanel === true))
            ? (() => {
                const declOrder = new Map(
                    (input.previewVariantParams ?? []).map((v, i) => [v.variantKey, i]),
                );
                return input.pendingPreviewVariants!.map((v) => ({
                    key: v.variantKey,
                    label: v.label,
                    colorIndex: declOrder.get(v.variantKey) ?? orderedResults.length,
                }));
            })()
            : [];

    const sweepPanels = (family.showSweepPanel === true)
        ? buildSweepPanels({
            orderedResults,
            theme: input.theme ?? 'dark',
            pendingPreviewVariants: input.pendingPreviewVariants,
            previewVariantParams: input.previewVariantParams,
            xAxisMode: input.xAxisMode,
            derivedByKey,
            geometry: family.sweepGeometry ?? 'both',
            method: family.sweepAnalyticalMethod ?? 'dykstra-parsons',
        })
        : emptySweepPanels();

    const visiblePanels = suppressPrimaryAnalyticalOverlays
        ? suppressPrimaryAnalyticalPanels(panels)
        : panels;

    // ── Published reference overlays (static benchmark data) ────────────
    appendPublishedReferenceSeries(visiblePanels, family);

    return {
        orderedResults,
        previewCases: pendingPreviewCases,
        panels: combinePanelMaps({ primary: visiblePanels, sweep: sweepPanels }),
        axisMappingWarning: buildAnalyticalAxisWarning({
            usesRunMappedAnalyticalXAxis,
            hidesPendingAnalyticalWithoutMapping,
        }),
    };
}