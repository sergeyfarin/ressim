import type { RateChartCurveOverride, RateChartLayoutConfig } from '../charts/rateChartLayoutConfig';
import { depletion_corner_producer } from './presetCases/depletion_corner_producer';
import { depletion_center_producer } from './presetCases/depletion_center_producer';
import { depletion_1d_clean } from './presetCases/depletion_1d_clean';
import { depletion_2d_radial_clean } from './presetCases/depletion_2d_radial_clean';
import { bl_aligned_homogeneous } from './presetCases/bl_aligned_homogeneous';
import { bl_aligned_mild_capillary } from './presetCases/bl_aligned_mild_capillary';
import { bl_aligned_mobility_balanced } from './presetCases/bl_aligned_mobility_balanced';
import { waterflood_bl_clean } from './presetCases/waterflood_bl_clean';
import { waterflood_unfavorable_mobility } from './presetCases/waterflood_unfavorable_mobility';
import { baseline_waterflood } from './presetCases/baseline_waterflood';
import { high_contrast_layers } from './presetCases/high_contrast_layers';
import { viscous_fingering_risk } from './presetCases/viscous_fingering_risk';

export type PresetCategory = 'depletion' | 'waterflood' | 'exploration';

export type PresetMode = 'dep' | 'wf' | 'sim';

export type PresetLayoutCurveOverride = RateChartCurveOverride;

export type PresetLayoutConfig = RateChartLayoutConfig;

type SourcePresetDefinition = {
    key: string;
    category: PresetCategory;
    mode: PresetMode;
    label: string;
    description: string;
    params: Record<string, any>;
    layoutConfig?: PresetLayoutConfig;
};

export type PresetEntry = SourcePresetDefinition;

export type PresetDimensionOption = {
    value: string;
    label: string;
    description: string;
};

const sourcePresetDefinitions: SourcePresetDefinition[] = [
    depletion_corner_producer,
    depletion_center_producer,
    depletion_1d_clean,
    depletion_2d_radial_clean,
    bl_aligned_homogeneous,
    bl_aligned_mild_capillary,
    bl_aligned_mobility_balanced,
    waterflood_bl_clean,
    waterflood_unfavorable_mobility,
    baseline_waterflood,
    high_contrast_layers,
    viscous_fingering_risk,
];

export const presetCases: PresetEntry[] = sourcePresetDefinitions.map((entry) => ({ ...entry }));

const presetEntryMap = new Map(presetCases.map((entry) => [entry.key, entry]));

const presetCasesByMode = new Map<PresetMode, PresetEntry[]>([
    ['dep', presetCases.filter((entry) => entry.mode === 'dep')],
    ['wf', presetCases.filter((entry) => entry.mode === 'wf')],
    ['sim', presetCases.filter((entry) => entry.mode === 'sim')],
]);

const presetCasesByCategory = new Map<PresetCategory, PresetEntry[]>([
    ['depletion', presetCases.filter((entry) => entry.category === 'depletion')],
    ['waterflood', presetCases.filter((entry) => entry.category === 'waterflood')],
    ['exploration', presetCases.filter((entry) => entry.category === 'exploration')],
]);

export function getPresetEntry(key: string | null | undefined): PresetEntry | null {
    if (!key) return null;
    return presetEntryMap.get(key) ?? null;
}

export function getPresetEntriesForMode(mode: PresetMode | null | undefined): PresetEntry[] {
    if (!mode) return [];
    return presetCasesByMode.get(mode) ?? [];
}

export function getPresetEntriesForCategory(category: PresetCategory | null | undefined): PresetEntry[] {
    if (!category) return [];
    return presetCasesByCategory.get(category) ?? [];
}

export function getPresetDimensionOptions(mode: PresetMode | null | undefined): PresetDimensionOption[] {
    return getPresetEntriesForMode(mode).map((entry) => ({
        value: entry.key,
        label: entry.label,
        description: entry.description,
    }));
}
