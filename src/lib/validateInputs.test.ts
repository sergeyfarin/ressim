import { describe, it, expect } from 'vitest';
import { validateInputs, type SimulationInputs } from './validateInputs';

/** Returns a valid base set of inputs (no errors expected). */
function makeValidInputs(overrides: Partial<SimulationInputs> = {}): SimulationInputs {
    return {
        nx: 10, ny: 10, nz: 1,
        cellDx: 10, cellDy: 10, cellDz: 5,
        steps: 30,
        initialSaturation: 0.3,
        delta_t_days: 0.5,
        well_radius: 0.1,
        mu_w: 0.5, mu_o: 1.0,
        c_o: 1e-5, c_w: 3e-6,
        rock_compressibility: 1e-6,
        volume_expansion_o: 1.0, volume_expansion_w: 1.0,
        max_sat_change_per_step: 0.1,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        injectorI: 0, injectorJ: 0,
        producerI: 9, producerJ: 9,
        s_wc: 0.1, s_or: 0.1,
        uniformPermX: 100,
        reservoirPorosity: 0.2,
        minPerm: 50, maxPerm: 500,
        injectorEnabled: true,
        injectorControlMode: 'pressure',
        producerControlMode: 'pressure',
        injectorBhp: 500, producerBhp: 100,
        targetInjectorRate: 350,
        targetProducerRate: 350,
        ...overrides,
    };
}

describe('validateInputs', () => {
    it('returns no errors for valid inputs', () => {
        const result = validateInputs(makeValidInputs());
        expect(Object.keys(result.errors)).toHaveLength(0);
        expect(result.warnings).toHaveLength(0);
    });

    // ── Grid dimension validation ──

    describe('grid dimensions', () => {
        it('errors on nx = 0', () => {
            const result = validateInputs(makeValidInputs({ nx: 0 }));
            expect(result.errors.nx).toBeDefined();
        });

        it('errors on fractional nx', () => {
            const result = validateInputs(makeValidInputs({ nx: 5.5 }));
            expect(result.errors.nx).toBeDefined();
        });

        it('errors on negative cellDx', () => {
            const result = validateInputs(makeValidInputs({ cellDx: -1 }));
            expect(result.errors.cellDx).toBeDefined();
        });

        it('errors on zero cellDz', () => {
            const result = validateInputs(makeValidInputs({ cellDz: 0 }));
            expect(result.errors.cellDz).toBeDefined();
        });

        it('errors on NaN cellDy', () => {
            const result = validateInputs(makeValidInputs({ cellDy: NaN }));
            expect(result.errors.cellDy).toBeDefined();
        });
    });

    // ── Timestep validation ──

    describe('timestep', () => {
        it('errors on zero delta_t_days', () => {
            const result = validateInputs(makeValidInputs({ delta_t_days: 0 }));
            expect(result.errors.deltaT).toBeDefined();
        });

        it('errors on negative delta_t_days', () => {
            const result = validateInputs(makeValidInputs({ delta_t_days: -0.5 }));
            expect(result.errors.deltaT).toBeDefined();
        });

        it('errors on Infinity delta_t_days', () => {
            const result = validateInputs(makeValidInputs({ delta_t_days: Infinity }));
            expect(result.errors.deltaT).toBeDefined();
        });

        it('errors on steps = 0', () => {
            const result = validateInputs(makeValidInputs({ steps: 0 }));
            expect(result.errors.steps).toBeDefined();
        });
    });

    // ── Saturation validation ──

    describe('saturation', () => {
        it('errors on initial saturation > 1', () => {
            const result = validateInputs(makeValidInputs({ initialSaturation: 1.5 }));
            expect(result.errors.initialSaturation).toBeDefined();
        });

        it('errors on initial saturation < 0', () => {
            const result = validateInputs(makeValidInputs({ initialSaturation: -0.1 }));
            expect(result.errors.initialSaturation).toBeDefined();
        });

        it('accepts boundary values 0 and 1', () => {
            expect(Object.keys(validateInputs(makeValidInputs({ initialSaturation: 0 })).errors)).not.toContain('initialSaturation');
            expect(Object.keys(validateInputs(makeValidInputs({ initialSaturation: 1 })).errors)).not.toContain('initialSaturation');
        });

        it('errors when s_wc + s_or >= 1', () => {
            const result = validateInputs(makeValidInputs({ s_wc: 0.5, s_or: 0.5 }));
            expect(result.errors.saturationEndpoints).toBeDefined();
        });

        it('errors on max_sat_change_per_step > 1', () => {
            const result = validateInputs(makeValidInputs({ max_sat_change_per_step: 1.5 }));
            expect(result.errors.max_sat_change_per_step).toBeDefined();
        });

        it('errors on max_sat_change_per_step = 0', () => {
            const result = validateInputs(makeValidInputs({ max_sat_change_per_step: 0 }));
            expect(result.errors.max_sat_change_per_step).toBeDefined();
        });
    });

    // ── Fluid / rock properties ──

    describe('fluid and rock properties', () => {
        it('errors on zero viscosity', () => {
            expect(validateInputs(makeValidInputs({ mu_w: 0 })).errors.mu_w).toBeDefined();
            expect(validateInputs(makeValidInputs({ mu_o: 0 })).errors.mu_o).toBeDefined();
        });

        it('errors on negative compressibility', () => {
            expect(validateInputs(makeValidInputs({ c_o: -1 })).errors.c_o).toBeDefined();
            expect(validateInputs(makeValidInputs({ c_w: -1 })).errors.c_w).toBeDefined();
            expect(validateInputs(makeValidInputs({ rock_compressibility: -1 })).errors.rock_compressibility).toBeDefined();
        });

        it('allows zero compressibility', () => {
            const result = validateInputs(makeValidInputs({ c_o: 0, c_w: 0, rock_compressibility: 0 }));
            expect(result.errors.c_o).toBeUndefined();
            expect(result.errors.c_w).toBeUndefined();
            expect(result.errors.rock_compressibility).toBeUndefined();
        });

        it('errors on zero well radius', () => {
            expect(validateInputs(makeValidInputs({ well_radius: 0 })).errors.wellRadius).toBeDefined();
        });

        it('errors on zero volume expansion factor', () => {
            expect(validateInputs(makeValidInputs({ volume_expansion_o: 0 })).errors.volume_expansion_o).toBeDefined();
        });

        it('errors when minPerm > maxPerm', () => {
            const result = validateInputs(makeValidInputs({ minPerm: 500, maxPerm: 50 }));
            expect(result.errors.permBounds).toBeDefined();
        });
    });

    // ── Well index validation ──

    describe('well indices', () => {
        it('errors on non-integer well indices', () => {
            const result = validateInputs(makeValidInputs({ injectorI: 1.5 }));
            expect(result.errors.wellIndexType).toBeDefined();
        });

        it('errors on out-of-bounds well indices', () => {
            const result = validateInputs(makeValidInputs({ producerI: 10 })); // >= nx
            expect(result.errors.wellIndexRange).toBeDefined();
        });

        it('errors on negative well indices', () => {
            const result = validateInputs(makeValidInputs({ injectorI: -1 }));
            expect(result.errors.wellIndexRange).toBeDefined();
        });

        it('errors on overlapping wells when injector enabled', () => {
            const result = validateInputs(makeValidInputs({ injectorI: 5, injectorJ: 5, producerI: 5, producerJ: 5 }));
            expect(result.errors.wellOverlap).toBeDefined();
        });

        it('no overlap error when injector disabled', () => {
            const result = validateInputs(makeValidInputs({ injectorEnabled: false, injectorI: 5, injectorJ: 5, producerI: 5, producerJ: 5 }));
            expect(result.errors.wellOverlap).toBeUndefined();
        });
    });

    // ── Well control validations ──

    describe('well controls', () => {
        it('errors when injector BHP <= producer BHP (both pressure-controlled)', () => {
            const result = validateInputs(makeValidInputs({ injectorBhp: 100, producerBhp: 200 }));
            expect(result.errors.wellPressureOrder).toBeDefined();
        });

        it('no error when control modes differ', () => {
            const result = validateInputs(makeValidInputs({ injectorControlMode: 'rate', injectorBhp: 50, producerBhp: 200, targetInjectorRate: 100 }));
            expect(result.errors.wellPressureOrder).toBeUndefined();
        });

        it('errors on zero injector rate when rate-controlled and enabled', () => {
            const result = validateInputs(makeValidInputs({ injectorControlMode: 'rate', targetInjectorRate: 0 }));
            expect(result.errors.injectorRate).toBeDefined();
        });

        it('errors on zero producer rate when rate-controlled', () => {
            const result = validateInputs(makeValidInputs({ producerControlMode: 'rate', targetProducerRate: 0 }));
            expect(result.errors.producerRate).toBeDefined();
        });
    });

    // ── Warnings ──

    describe('warnings', () => {
        it('warns when run exceeds 10 years', () => {
            const result = validateInputs(makeValidInputs({ delta_t_days: 100, steps: 40 }));
            expect(result.warnings.length).toBeGreaterThan(0);
            expect(result.warnings[0]?.code).toBe('long-run-duration');
            expect(result.warnings[0]?.surface).toBe('advisory');
            expect(result.warnings[0]?.message).toMatch(/10 years/);
        });

        it('warns on large max pressure change per step', () => {
            const result = validateInputs(makeValidInputs({ max_pressure_change_per_step: 300 }));
            expect(result.warnings.some((warning) => warning.code === 'pressure-step-large')).toBe(true);
            expect(result.warnings.some((warning) => warning.surface === 'non-physical')).toBe(true);
        });

        it('no warnings for moderate settings', () => {
            const result = validateInputs(makeValidInputs());
            expect(result.warnings).toHaveLength(0);
        });
    });
});
