import { describe, expect, it } from 'vitest';

import type { GridState } from '../simulator-types';
import {
    compositionComponentId,
    compositionIndex,
    compositionProperty,
    resolveSpatialProperty,
    spatialPropertyOptions,
    spatialPropertyValues,
} from './spatialProperty';

const blackOil: GridState = {
    pressure: Float64Array.from([200, 190]),
    sat_water: Float64Array.from([0.2, 0.3]),
    sat_oil: Float64Array.from([0.8, 0.7]),
    sat_gas: Float64Array.from([0, 0]),
};

const compositional: GridState = {
    pressure: Float64Array.from([140, 60]),
    sat_water: new Float64Array(2),
    sat_oil: Float64Array.from([0.4, 1]),
    sat_gas: Float64Array.from([0.6, 0]),
    composition: {
        componentIds: ['CO2', 'C1', 'C10'],
        values: [Float64Array.from([0.9, 0.1]), Float64Array.from([0.05, 0.3]), Float64Array.from([0.05, 0.6])],
    },
};

describe('spatial properties', () => {
    it('offers the black-oil set on a grid without composition', () => {
        expect(spatialPropertyOptions(blackOil).map((option) => option.value)).toEqual([
            'pressure', 'saturation_water', 'saturation_oil', 'saturation_gas', 'saturation_ternary',
        ]);
    });

    it('offers one property per component and no water on a compositional grid', () => {
        expect(spatialPropertyOptions(compositional)).toEqual([
            { value: 'pressure', label: 'Pressure' },
            { value: 'saturation_oil', label: 'Oil Sat' },
            { value: 'saturation_gas', label: 'Gas Sat' },
            { value: 'composition_0', label: 'z CO2' },
            { value: 'composition_1', label: 'z C1' },
            { value: 'composition_2', label: 'z C10' },
        ]);
    });

    it('round-trips a component index and names its component', () => {
        expect(compositionIndex(compositionProperty(2))).toBe(2);
        expect(compositionIndex('saturation_gas')).toBeNull();
        expect(compositionComponentId(compositional, 'composition_1')).toBe('C1');
        expect(compositionComponentId(blackOil, 'composition_1')).toBeNull();
    });

    it('reads each property from its own per-cell array', () => {
        expect(Array.from(spatialPropertyValues(compositional, 'composition_2') ?? [])).toEqual([0.05, 0.6]);
        expect(Array.from(spatialPropertyValues(compositional, 'saturation_gas') ?? [])).toEqual([0.6, 0]);
        expect(spatialPropertyValues(blackOil, 'composition_0')).toBeUndefined();
        expect(spatialPropertyValues(blackOil, 'saturation_ternary')).toBeUndefined();
    });

    it('falls back to pressure when the grid cannot show the requested property', () => {
        expect(resolveSpatialProperty('composition_1', blackOil)).toBe('pressure');
        expect(resolveSpatialProperty('composition_5', compositional)).toBe('pressure');
        expect(resolveSpatialProperty('saturation_water', compositional)).toBe('pressure');
        expect(resolveSpatialProperty('composition_1', compositional)).toBe('composition_1');
        expect(resolveSpatialProperty('saturation_gas', blackOil)).toBe('saturation_gas');
    });

    it('keeps the request when there is no grid to check it against', () => {
        expect(resolveSpatialProperty('composition_1', null)).toBe('composition_1');
    });
});
