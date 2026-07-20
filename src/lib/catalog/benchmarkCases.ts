/**
 * Stub — the legacy "benchmark family" runtime data/logic that used to live
 * here was archived 2026-07 (frontend execution plan, Wave 3 W3.3): see
 * `.archive/README.md` for the full rationale and how to resurrect it.
 *
 * Confirmed unreachable from the live UI before archiving (three
 * independent checks — see `.archive/README.md`), so this stub preserves
 * the exact same exported names/types with empty data rather than
 * requiring every consumer (`runtimeStore.svelte.ts`,
 * `navigationStore.svelte.ts`, `caseLibrary.ts`, `ReferenceExecutionCard.svelte`)
 * to be edited: they already null-check defensively, and nothing in the
 * live UI ever populated these paths with a real value regardless, so this
 * is a zero-observable-behavior-change swap.
 *
 * Types are unaffected — they still live in `../scenario/referenceTypes.ts`
 * (moved there in W3.1) and are re-exported here unchanged, since the new
 * scenario-first path (`runModel.ts`, `benchmarkRunModel.ts`,
 * `scenarioChartModel.ts`, `referenceChartConfig.ts`) still uses them.
 */
export type {
    BenchmarkEntry,
    BenchmarkSensitivityAxisKey,
    BenchmarkReferenceKind,
    BenchmarkXAxisKey,
    BenchmarkPanelKey,
    BenchmarkRunPolicy,
    BenchmarkReferenceDefinition,
    BenchmarkBreakthroughCriterion,
    BenchmarkComparisonMetric,
    BenchmarkDisplayDefaults,
    BenchmarkStylePolicy,
    BenchmarkVariantAnalyticalValidity,
    BenchmarkFamily,
    BenchmarkVariant,
    BenchmarkDimensionOption,
} from '../scenario/referenceTypes';

import type {
    BenchmarkEntry,
    BenchmarkSensitivityAxisKey,
    BenchmarkFamily,
    BenchmarkVariant,
    BenchmarkDimensionOption,
} from '../scenario/referenceTypes';

export const sourceBenchmarkCases: BenchmarkEntry[] = [];
export const benchmarkFamilies: BenchmarkFamily[] = [];
export const benchmarkVariants: BenchmarkVariant[] = [];
export const benchmarkCases: BenchmarkEntry[] = [];

export function getBenchmarkEntry(_key: string | null | undefined): BenchmarkEntry | null {
    return null;
}

export function getBenchmarkFamily(_key: string | null | undefined): BenchmarkFamily | null {
    return null;
}

export function getBenchmarkVariantsForFamily(_familyKey: string | null | undefined): BenchmarkVariant[] {
    return [];
}

export function getBenchmarkVariant(_key: string | null | undefined): BenchmarkVariant | null {
    return null;
}

export function getBenchmarkSensitivityAxisLabel(axis: BenchmarkSensitivityAxisKey): string {
    return axis;
}

export function getBenchmarkDimensionOptions(): BenchmarkDimensionOption[] {
    return [];
}
