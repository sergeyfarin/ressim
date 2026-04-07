/**
 * universalChartTypes.ts — shared contract for the scenario-agnostic chart system.
 *
 * The four curve types drive styling automatically:
 *   simulation         → solid, bold       (our IMPES solver)
 *   analytical         → dashed [7,4]      (mathematical reference solution)
 *   reference          → dotted [4,4]      (published / digitized external data)
 *   reference-simulation → solid, thin     (another simulator's output)
 *
 * Scenarios declare UniversalPanelDef[] listing exactly which curves to show.
 * buildUniversalChartData() iterates these defs, calls getData/getDataXY per curve,
 * applies curveType styling, and returns UniversalChartDataSet[] ready for
 * ChartSubPanel — with no knowledge of oil rates, water cuts, or scenarios.
 */

import type { CurveConfig } from './chartTypes';
import type { RateChartPanelId, RateChartXAxisMode } from './rateChartLayoutConfig';
import type { AnalyticalProductionPoint, RateHistoryPoint } from '../simulator-types';
import type { SweepRFResult } from '../analytical/sweepEfficiency';

// Re-export so callers import from one place
export type { XYPoint } from './axisAdapters';

// ─── Four-tier curve classification ───────────────────────────────────────────

/**
 * Primary styling axis for every curve.
 *   simulation           our IMPES solver output       — solid, 2.5px
 *   analytical           computed reference solution   — dashed [7,4], 2.0px
 *   reference            published / digitized data    — dotted [4,4], 1.5px
 *   reference-simulation another simulator's output    — solid, 1.5px
 */
export type CurveType =
    | 'simulation'
    | 'analytical'
    | 'reference'
    | 'reference-simulation';

// ─── Pre-computed live series ─────────────────────────────────────────────────

/**
 * Series derived from rateHistory + analyticalProductionData, aligned index-for-index
 * to rateHistory. Built once by buildLiveDerivedSeries() and reused by all getData callbacks.
 */
export type LiveDerivedSeries = {
    time: number[];

    // Simulation rates
    oilRate: number[];
    waterRate: number[];
    liquidRate: number[];
    injectionRate: number[];

    // Simulation cumulatives
    cumOil: number[];
    cumWater: number[];
    cumLiquid: number[];
    cumInjection: number[];
    pvi: number[];
    pvp: number[];

    // Recovery
    recoveryFactor: Array<number | null>;

    // Analytical cumulatives (integrated from analyticalProductionData + time grid)
    analyticalCumOil: Array<number | null>;
    analyticalRecoveryFactor: Array<number | null>;

    // Diagnostic series — simulation
    avgPressure: Array<number | null>;
    avgWaterSat: Array<number | null>;
    vrr: Array<number | null>;
    worSim: Array<number | null>;
    waterCutSim: number[];
    mbError: Array<number | null>;

    // Diagnostic series — analytical counterparts
    analyticalAvgPressure: Array<number | null>;
    worAnalytical: Array<number | null>;
    waterCutAnalytical: Array<number | null>;
};

// ─── Sweep context (only present when showSweepPanel is true) ─────────────────

/** Pre-computed sweep analytical results handed to sweep curve getData callbacks. */
export type LiveSweepContext = {
    arealSweepCurve: Array<{ pvi: number; efficiency: number }>;
    verticalSweepCurve: Array<{ pvi: number; efficiency: number }>;
    combinedSweepCurve: Array<{ pvi: number; efficiency: number }>;
    rfResult: SweepRFResult;
    showAreal: boolean;
    showVertical: boolean;
};

// ─── Curve data context ───────────────────────────────────────────────────────

/** Everything a UniversalCurveDef.getData / .getDataXY callback can access. */
export type LiveCurveContext = {
    /** Pre-computed simulation + analytical series (length = rateHistory.length). */
    sim: LiveDerivedSeries;
    /** Raw analytical production points (parallel to rateHistory). */
    analytical: AnalyticalProductionPoint[];
    /** Computed x-axis values for the current xAxisMode (may contain nulls for logTime at t=0). */
    xValues: Array<number | null>;
    xAxisMode: RateChartXAxisMode;
    /** Raw PVI values — used by sweep getDataXY to map analytical curves onto active x-axis. */
    pviArr: number[];
    /** Rate normalization factor: 1/q₀ when normalizeRates is active, else 1.0. */
    scaleFactor: number;
    /** Theme-dependent neutral color: '#f8fafc' (dark) or '#0f172a' (light).
     *  Use the sentinel 'neutral' in curve color fields to request this automatically. */
    neutralColor: string;
    ooipM3: number;
    /** Raw rate history — needed by sweep callbacks for time→xValue mapping. */
    rateHistory: RateHistoryPoint[];
    /** Sweep pre-computation — null when showSweepPanel is false or inputs are missing. */
    sweep: LiveSweepContext | null;
    /** Raw sweep simulation history — parallel to sweepEfficiencySimSeries prop. */
    sweepSimSeries: Array<{
        time: number;
        eA: number | null;
        eV: number | null;
        eVol: number;
        mobileOilRecovered: number | null;
    }> | null;
};

// ─── Curve definition (scenario-owned) ───────────────────────────────────────

/**
 * A single curve declared by a scenario panel definition.
 *
 * color: hex string, or the sentinel 'neutral' which resolves to
 *        ctx.neutralColor at build time.
 *
 * Provide either getData (y-values; builder zips with xValues) or
 * getDataXY (pre-mapped XYPoints; used for sweep analytical curves and
 * published reference series where x must be mapped independently).
 */
export type UniversalCurveDef = {
    key: string;
    label: string;
    curveType: CurveType;
    yAxisID: string;
    color: string;   // hex or 'neutral'
    defaultVisible?: boolean;
    toggleGroupKey?: string;
    getData?: (ctx: LiveCurveContext) => Array<number | null>;
    getDataXY?: (ctx: LiveCurveContext) => Array<{ x: number; y: number | null }>;
};

// ─── Panel definition (scenario-owned) ───────────────────────────────────────

/**
 * A panel declared by a scenario. Defines exactly which curves to show —
 * no hidden fallbacks.
 */
export type UniversalPanelDef = {
    panelKey: RateChartPanelId;
    curves: UniversalCurveDef[];
};

// ─── Builder output ───────────────────────────────────────────────────────────

/** One panel's worth of built data — ready for ChartSubPanel. */
export type UniversalChartDataSet = {
    panelKey: RateChartPanelId;
    curves: CurveConfig[];
    series: Array<{ x: number; y: number | null }[]>;
};

/** Mismatch between simulation and analytical oil rate (normalized). */
export type MismatchSummary = {
    pointsCompared: number;
    mae: number;
    rmse: number;
    mape: number;
};

/** Full result from buildUniversalChartData. */
export type UniversalChartResult = {
    datasets: UniversalChartDataSet[];
    mismatchSummary: MismatchSummary;
    pviAvailable: boolean;
    pvpAvailable: boolean;
    xValues: Array<number | null>;
    pviArr: number[];
};
