/**
 * The per-cell properties the 3D view and the spatial profile can show — one selector for both.
 *
 * The fixed set is the black-oil one: pressure and the three phase saturations. A grid that carries
 * a composition (a compositional run) adds one property per component, `composition_<index>`, in
 * the order the grid lists its components; it has no water, so water saturation and the ternary
 * blend are not offered for it.
 *
 * Pure leaf module: both views import it, so it imports neither.
 */

import type { GridState } from '../simulator-types';

export type PhaseSpatialProperty =
    | 'pressure'
    | 'saturation_water'
    | 'saturation_oil'
    | 'saturation_gas'
    | 'saturation_ternary';

/** Overall mole fraction of the grid's component at this index. */
export type CompositionSpatialProperty = `composition_${number}`;

export type SpatialProperty = PhaseSpatialProperty | CompositionSpatialProperty;

export type SpatialPropertyOption = { value: SpatialProperty; label: string };

const COMPOSITION_PREFIX = 'composition_';

const PHASE_OPTIONS: SpatialPropertyOption[] = [
    { value: 'pressure', label: 'Pressure' },
    { value: 'saturation_water', label: 'Water Sat' },
    { value: 'saturation_oil', label: 'Oil Sat' },
    { value: 'saturation_gas', label: 'Gas Sat' },
    { value: 'saturation_ternary', label: 'Ternary Sat' },
];

/** Phase properties that mean nothing on a grid without water. */
const WATER_PROPERTIES: ReadonlySet<SpatialProperty> = new Set(['saturation_water', 'saturation_ternary']);

export function compositionProperty(componentIndex: number): CompositionSpatialProperty {
    return `${COMPOSITION_PREFIX}${componentIndex}`;
}

/** The component index a property names, or null when it is not a composition property. */
export function compositionIndex(property: SpatialProperty): number | null {
    if (!property.startsWith(COMPOSITION_PREFIX)) return null;
    const index = Number(property.slice(COMPOSITION_PREFIX.length));
    return Number.isInteger(index) && index >= 0 ? index : null;
}

/** The component's name for a composition property, or null. */
export function compositionComponentId(
    grid: GridState | null | undefined,
    property: SpatialProperty,
): string | null {
    const index = compositionIndex(property);
    return index === null ? null : grid?.composition?.componentIds[index] ?? null;
}

/** The properties a grid can show, in selector order. */
export function spatialPropertyOptions(grid: GridState | null | undefined): SpatialPropertyOption[] {
    const composition = grid?.composition;
    if (!composition) return PHASE_OPTIONS;
    return [
        ...PHASE_OPTIONS.filter((option) => !WATER_PROPERTIES.has(option.value)),
        ...composition.componentIds.map((id, index) => ({ value: compositionProperty(index), label: `z ${id}` })),
    ];
}

/**
 * The property to show on `grid`: the requested one when the grid offers it, otherwise the first
 * it does. Without a grid there is nothing to check against, and the request stands.
 */
export function resolveSpatialProperty(
    property: SpatialProperty,
    grid: GridState | null | undefined,
): SpatialProperty {
    if (!grid) return property;
    const options = spatialPropertyOptions(grid);
    return options.some((option) => option.value === property) ? property : options[0].value;
}

/**
 * The per-cell array a scalar property reads from, or undefined when the grid lacks it. The
 * ternary blend has no single array.
 */
export function spatialPropertyValues(
    grid: GridState | null | undefined,
    property: SpatialProperty,
): ArrayLike<number> | undefined {
    if (!grid) return undefined;
    switch (property) {
        case 'pressure': return grid.pressure;
        case 'saturation_water': return grid.sat_water;
        case 'saturation_oil': return grid.sat_oil;
        case 'saturation_gas': return grid.sat_gas;
        case 'saturation_ternary': return undefined;
    }
    const index = compositionIndex(property);
    return index === null ? undefined : grid.composition?.values[index];
}
