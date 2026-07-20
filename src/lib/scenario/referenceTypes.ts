/**
 * referenceTypes.ts — shared reference/comparison-run type definitions,
 * owned by the scenario-first architecture (ROADMAP Priority 3.1).
 *
 * These types used to live in `src/lib/catalog/benchmarkCases.ts` alongside
 * that file's legacy benchmark-family runtime data/logic. They are pure
 * type definitions with no runtime behavior, so they were relocated here
 * first (frontend execution plan, Wave 3 W3.1) — deliberately dependency-
 * free (no imports from `benchmarkRunModel.ts` or `scenario/runModel.ts`)
 * to avoid a circular import, since both of those files import some of
 * these types as values flow the other direction.
 *
 * `benchmarkCases.ts` re-exports these for its existing consumers; new code
 * should import directly from here.
 */

export type BenchmarkEntry = {
    key: string;
    label: string;
    description: string;
    params: Record<string, any>;
};

export type BenchmarkSensitivityAxisKey =
    | 'grid-refinement'
    | '2d-grid-refinement'
    | 'timestep-refinement'
    | 'heterogeneity';

export type BenchmarkReferenceKind = 'analytical' | 'numerical-refined';

export type BenchmarkXAxisKey = 'time' | 'pvi' | 'tD';

export type BenchmarkPanelKey =
    | 'watercut-breakthrough'
    | 'recovery'
    | 'pressure'
    | 'rates'
    | 'oil-rate'
    | 'cumulative-oil'
    | 'decline-diagnostics';

export type BenchmarkRunPolicy = 'single' | 'sweep' | 'compare-to-reference';

export type BenchmarkReferenceDefinition = {
    kind: BenchmarkReferenceKind;
    source: string;
};

export type BenchmarkBreakthroughCriterion = {
    kind: 'watercut-threshold';
    value: number;
};

export type BenchmarkComparisonMetric = {
    kind: 'breakthrough-pv-relative-error';
    target: 'analytical-reference' | 'numerical-reference';
    tolerance: number;
};

export type BenchmarkDisplayDefaults = {
    xAxis: BenchmarkXAxisKey;
    panels: BenchmarkPanelKey[];
};

export type BenchmarkStylePolicy = {
    colorBy: 'case';
    lineStyleBy: 'quantity-or-reference';
    separatePressurePanel: boolean;
};

export type BenchmarkVariantAnalyticalValidity =
    | 'same-reference'
    | 'numerical-reference-required';

type BenchmarkFamilyDefinition = {
    key: string;
    baseCaseKey: string;
    /** Which analytical reference model to use — unified vocabulary with ScenarioCapabilities. */
    analyticalMethod: import('../catalog/scenarios').AnalyticalMethod;
    sensitivityAxes: BenchmarkSensitivityAxisKey[];
    reference: BenchmarkReferenceDefinition;
    breakthroughCriterion?: BenchmarkBreakthroughCriterion;
    comparisonMetric?: BenchmarkComparisonMetric;
    displayDefaults: BenchmarkDisplayDefaults;
    stylePolicy: BenchmarkStylePolicy;
    runPolicy: BenchmarkRunPolicy;
};

export type BenchmarkFamily = BenchmarkFamilyDefinition & {
    label: string;
    description: string;
    baseCase: BenchmarkEntry;
    suppressPrimaryAnalyticalOverlays?: boolean;
    /** True only for sweep-domain scenarios where E_A/E_V/E_vol panels are physically meaningful. */
    showSweepPanel?: boolean;
    sweepGeometry?: import('../analytical/sweepEfficiency').SweepGeometry | null;
    sweepAnalyticalMethod?: import('../analytical/sweepEfficiency').SweepAnalyticalMethod;
    analyticalOverlayMode?: import('../catalog/scenarios').AnalyticalOverlayMode;
    /** Static published-benchmark reference series to overlay on charts. */
    publishedReferenceSeries?: import('../catalog/scenarios').PublishedReferenceSeries[];
};

export type BenchmarkVariant = {
    key: string;
    familyKey: string;
    variantKey: string;
    axis: BenchmarkSensitivityAxisKey;
    label: string;
    description: string;
    params: Record<string, any>;
    paramsDelta: Record<string, any>;
    baseCaseKey: string;
    reference: BenchmarkReferenceDefinition;
    comparisonMetric: BenchmarkComparisonMetric | null;
    comparisonMeaning: string;
    analyticalValidity: BenchmarkVariantAnalyticalValidity;
};

export type BenchmarkDimensionOption = {
    value: string;
    label: string;
    default?: boolean;
};
