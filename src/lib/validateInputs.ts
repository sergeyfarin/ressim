/**
 * Input validation logic extracted from simulationStore for testability.
 * Returns structured validation state with blocking errors and typed warnings.
 */

export type SimulationInputs = {
    nx: number;
    ny: number;
    nz: number;
    cellDx: number;
    cellDy: number;
    cellDz: number;
    steps: number;
    initialSaturation: number;
    delta_t_days: number;
    well_radius: number;
    mu_w: number;
    mu_o: number;
    c_o: number;
    c_w: number;
    rock_compressibility: number;
    volume_expansion_o: number;
    volume_expansion_w: number;
    max_sat_change_per_step: number;
    max_pressure_change_per_step: number;
    max_well_rate_change_fraction: number;
    injectorI: number;
    injectorJ: number;
    producerI: number;
    producerJ: number;
    s_wc: number;
    s_or: number;
    // Three-phase (optional)
    s_gc?: number;
    s_gr?: number;
    s_org?: number;
    n_g?: number;
    mu_g?: number;
    c_g?: number;
    threePhaseModeEnabled?: boolean;
    uniformPermX: number;
    reservoirPorosity: number;
    minPerm: number;
    maxPerm: number;
    injectorEnabled: boolean;
    injectorControlMode: string;
    producerControlMode: string;
    injectorBhp: number;
    producerBhp: number;
    targetInjectorRate: number;
    targetProducerRate: number;
};

export type ValidationState = {
    errors: Record<string, string>;
    warnings: ValidationWarning[];
};

export type ValidationWarningSurface = 'non-physical' | 'advisory';

export type ValidationWarning = {
    code: 'long-run-duration' | 'pressure-step-large' | 'low-permeability' | 'high-viscosity-ratio' | 'large-grid' | 'small-timestep' | 'high-mobility-ratio';
    message: string;
    surface: ValidationWarningSurface;
    fieldKey?: string;
};

export function validateInputs(input: SimulationInputs): ValidationState {
    const errors: Record<string, string> = {};
    const warnings: ValidationWarning[] = [];
    const numeric = (value: unknown) => Number(value);
    const isFiniteNumber = (value: unknown) => Number.isFinite(numeric(value));

    if (!Number.isInteger(numeric(input.nx)) || numeric(input.nx) < 1) errors.nx = 'Nx must be an integer ≥ 1.';
    if (!Number.isInteger(numeric(input.ny)) || numeric(input.ny) < 1) errors.ny = 'Ny must be an integer ≥ 1.';
    if (!Number.isInteger(numeric(input.nz)) || numeric(input.nz) < 1) errors.nz = 'Nz must be an integer ≥ 1.';
    if (!isFiniteNumber(input.cellDx) || numeric(input.cellDx) <= 0) errors.cellDx = 'Cell Δx must be positive.';
    if (!isFiniteNumber(input.cellDy) || numeric(input.cellDy) <= 0) errors.cellDy = 'Cell Δy must be positive.';
    if (!isFiniteNumber(input.cellDz) || numeric(input.cellDz) <= 0) errors.cellDz = 'Cell Δz must be positive.';
    if (!Number.isInteger(numeric(input.steps)) || numeric(input.steps) < 1) errors.steps = 'Steps must be an integer ≥ 1.';
    if (input.initialSaturation < 0 || input.initialSaturation > 1) errors.initialSaturation = 'Initial water saturation must be in [0, 1].';
    if (!isFiniteNumber(input.delta_t_days) || numeric(input.delta_t_days) <= 0) errors.deltaT = 'Timestep must be positive.';
    if (!isFiniteNumber(input.well_radius) || numeric(input.well_radius) <= 0) errors.wellRadius = 'Well radius must be positive.';
    if (!isFiniteNumber(input.mu_w) || numeric(input.mu_w) <= 0) errors.mu_w = 'Water viscosity must be positive.';
    if (!isFiniteNumber(input.mu_o) || numeric(input.mu_o) <= 0) errors.mu_o = 'Oil viscosity must be positive.';
    if (!isFiniteNumber(input.c_o) || numeric(input.c_o) < 0) errors.c_o = 'Oil compressibility must be ≥ 0.';
    if (!isFiniteNumber(input.c_w) || numeric(input.c_w) < 0) errors.c_w = 'Water compressibility must be ≥ 0.';
    if (!isFiniteNumber(input.rock_compressibility) || numeric(input.rock_compressibility) < 0) errors.rock_compressibility = 'Rock compressibility must be ≥ 0.';
    if (!isFiniteNumber(input.volume_expansion_o) || numeric(input.volume_expansion_o) <= 0) errors.volume_expansion_o = 'Oil formation volume factor must be positive.';
    if (!isFiniteNumber(input.volume_expansion_w) || numeric(input.volume_expansion_w) <= 0) errors.volume_expansion_w = 'Water formation volume factor must be positive.';
    if (!isFiniteNumber(input.max_sat_change_per_step) || numeric(input.max_sat_change_per_step) <= 0 || numeric(input.max_sat_change_per_step) > 1) errors.max_sat_change_per_step = 'Max ΔSw per step must be in (0, 1].';
    if (!isFiniteNumber(input.max_pressure_change_per_step) || numeric(input.max_pressure_change_per_step) <= 0) errors.max_pressure_change_per_step = 'Max ΔP per step must be positive.';
    if (!isFiniteNumber(input.max_well_rate_change_fraction) || numeric(input.max_well_rate_change_fraction) <= 0) errors.max_well_rate_change_fraction = 'Max well-rate change fraction must be positive.';

    if (
        !Number.isInteger(numeric(input.injectorI)) ||
        !Number.isInteger(numeric(input.injectorJ)) ||
        !Number.isInteger(numeric(input.producerI)) ||
        !Number.isInteger(numeric(input.producerJ))
    ) {
        errors.wellIndexType = 'Well indices must be integers.';
    } else if (
        numeric(input.injectorI) < 0 || numeric(input.injectorI) >= numeric(input.nx) ||
        numeric(input.injectorJ) < 0 || numeric(input.injectorJ) >= numeric(input.ny) ||
        numeric(input.producerI) < 0 || numeric(input.producerI) >= numeric(input.nx) ||
        numeric(input.producerJ) < 0 || numeric(input.producerJ) >= numeric(input.ny)
    ) {
        errors.wellIndexRange = 'Well indices must lie within the grid bounds.';
    }

    if (input.threePhaseModeEnabled) {
        const s_gc = input.s_gc ?? 0;
        const s_gr = input.s_gr ?? 0;
        if (input.s_wc + input.s_or + s_gc + s_gr >= 1) {
            errors.saturationEndpoints = 'S_wc + S_or + S_gc + S_gr must be < 1.';
        }
        if (typeof input.n_g === 'number' && input.n_g <= 0) {
            errors.n_g = 'Gas Corey exponent must be positive.';
        }
        if (typeof input.mu_g === 'number' && input.mu_g <= 0) {
            errors.mu_g = 'Gas viscosity must be positive.';
        }
        if (typeof input.c_g === 'number' && input.c_g < 0) {
            errors.c_g = 'Gas compressibility must be non-negative.';
        }
    } else {
        if (input.s_wc + input.s_or >= 1) errors.saturationEndpoints = 'S_wc + S_or must be < 1.';
    }
    if (input.minPerm > input.maxPerm) errors.permBounds = 'Min perm must not exceed max perm.';
    if (input.injectorEnabled && input.injectorI === input.producerI && input.injectorJ === input.producerJ) {
        errors.wellOverlap = 'Injector and producer cannot share the same i/j location.';
    }
    if (input.injectorControlMode === 'pressure' && input.producerControlMode === 'pressure' && input.injectorBhp <= input.producerBhp) {
        errors.wellPressureOrder = 'Injector BHP should be greater than producer BHP.';
    }
    if (input.injectorControlMode === 'rate' && input.targetInjectorRate <= 0 && input.injectorEnabled) {
        errors.injectorRate = 'Injector rate must be positive when enabled and rate-controlled.';
    }
    if (input.producerControlMode === 'rate' && input.targetProducerRate <= 0) {
        errors.producerRate = 'Producer rate must be positive when rate-controlled.';
    }
    if (input.delta_t_days * input.steps > 3650) {
        warnings.push({
            code: 'long-run-duration',
            message: 'Requested run covers more than 10 years; results may require tighter timestep limits.',
            surface: 'advisory',
            fieldKey: 'steps',
        });
    }
    if (input.max_pressure_change_per_step > 250) {
        warnings.push({
            code: 'pressure-step-large',
            message: 'Large max ΔP per step may reduce numerical robustness.',
            surface: 'non-physical',
            fieldKey: 'max_pressure_change_per_step',
        });
    }
    if (input.uniformPermX < 0.1) {
        warnings.push({
            code: 'low-permeability',
            message: 'Permeability < 0.1 mD: convergence may be slow; consider smaller timestep.',
            surface: 'advisory',
        });
    }
    const mobilityRatio = (input.mu_o / input.mu_w) > 0 ? input.mu_o / input.mu_w : 1;
    if (mobilityRatio > 50) {
        warnings.push({
            code: 'high-mobility-ratio',
            message: `High mobility ratio (μ_o/μ_w ≈ ${mobilityRatio.toFixed(0)}): expect early breakthrough and poor sweep.`,
            surface: 'advisory',
        });
    }
    const totalCells = input.nx * input.ny * input.nz;
    if (totalCells > 50000) {
        warnings.push({
            code: 'large-grid',
            message: `${totalCells.toLocaleString()} cells: simulation may be slow; each step > 1 s.`,
            surface: 'advisory',
        });
    }
    if (input.delta_t_days < 0.01) {
        warnings.push({
            code: 'small-timestep',
            message: 'Very small timestep: simulation will need many steps to cover meaningful time.',
            surface: 'advisory',
        });
    }
    return { errors, warnings };
}
