/**
 * Case catalog — defines all scenario presets grouped into categories.
 *
 * Each category has a label, description, and an array of cases.
 * Each case has a key (matching old preset keys), a human label,
 * a short description, and a params object that the simulator understands.
 *
 * This module is consumed by both the frontend (TopBar/CaseSelector)
 * and the export-cases script.
 */

export type CaseParams = Record<string, any>;

export type CaseEntry = {
    key: string;
    label: string;
    description: string;
    params: CaseParams;
};

export type CaseCategory = {
    label: string;
    description: string;
    cases: CaseEntry[];
};

// Default fluid / rock properties shared across all presets
const DEFAULTS: CaseParams = {
    mu_w: 0.5,
    mu_o: 1.0,
    c_o: 1e-5,
    c_w: 3e-6,
    rock_compressibility: 1e-6,
    depth_reference: 0.0,
    volume_expansion_o: 1.0,
    volume_expansion_w: 1.0,
    rho_w: 1000.0,
    rho_o: 800.0,
    well_radius: 0.1,
    well_skin: 0.0,
    max_pressure_change_per_step: 75.0,
    max_well_rate_change_fraction: 0.75,
    injectorEnabled: true,
    injectorControlMode: 'pressure',
    producerControlMode: 'pressure',
    injectorBhp: 400.0,
    producerBhp: 100.0,
    targetInjectorRate: 350.0,
    targetProducerRate: 350.0,
    reservoirPorosity: 0.2,
};

/**
 * Merge a case's sparse params with defaults, and calculate
 * derived well positions if not specified.
 */
export function resolveParams(sparse: CaseParams): CaseParams {
    const merged: CaseParams = { ...DEFAULTS, ...sparse };
    // Default well positions if not specified
    if (merged.injectorI === undefined) merged.injectorI = 0;
    if (merged.injectorJ === undefined) merged.injectorJ = 0;
    if (merged.producerI === undefined) merged.producerI = Number(merged.nx ?? 1) - 1;
    if (merged.producerJ === undefined) merged.producerJ = 0;
    return merged;
}

export const caseCatalog: Record<string, CaseCategory> = {
    depletion: {
        label: 'Depletion vs. Analytical',
        description: 'Primary depletion with analytical comparison (Dietz shape factor)',
        cases: [
            {
                key: 'depletion_corner_producer',
                label: 'Corner Producer (1D)',
                description: '1D depletion with producer at corner — linear geometry',
                params: {
                    nx: 48, ny: 1, nz: 1,
                    cellDx: 10, cellDy: 10, cellDz: 5,
                    delta_t_days: 0.5, steps: 36,
                    max_sat_change_per_step: 0.1,
                    initialPressure: 300, initialSaturation: 0.1,
                    injectorEnabled: false,
                    producerControlMode: 'pressure', producerBhp: 80,
                    injectorI: 0, injectorJ: 0,
                    producerI: 0, producerJ: 0,
                    permMode: 'uniform',
                    uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                },
            },
            {
                key: 'depletion_center_producer',
                label: 'Center Producer (2D)',
                description: '2D depletion with producer at centre — radial geometry',
                params: {
                    nx: 49, ny: 49, nz: 1,
                    cellDx: 10, cellDy: 10, cellDz: 5,
                    delta_t_days: 2.0, steps: 50,
                    max_sat_change_per_step: 0.1,
                    initialPressure: 300, initialSaturation: 0.1,
                    injectorEnabled: false,
                    producerControlMode: 'pressure', producerBhp: 80,
                    injectorI: 0, injectorJ: 0,
                    producerI: 24, producerJ: 24,
                    permMode: 'uniform',
                    uniformPermX: 100, uniformPermY: 100, uniformPermZ: 10,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                },
            },
            {
                key: 'depletion_1d_clean',
                label: 'Clean 1D Depletion',
                description: 'Sw=Swc, no capillary — ideal for analytical match',
                params: {
                    nx: 20, ny: 1, nz: 1,
                    cellDx: 10, cellDy: 10, cellDz: 10,
                    delta_t_days: 1.0, steps: 50,
                    max_sat_change_per_step: 0.1,
                    initialPressure: 300, initialSaturation: 0.1,
                    injectorEnabled: false,
                    producerControlMode: 'pressure', producerBhp: 100,
                    injectorI: 0, injectorJ: 0,
                    producerI: 0, producerJ: 0,
                    permMode: 'uniform',
                    uniformPermX: 200, uniformPermY: 200, uniformPermZ: 20,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                },
            },
            {
                key: 'depletion_2d_radial_clean',
                label: 'Clean 2D Radial Depletion',
                description: 'Center producer, Sw=Swc, no capillary — radial analytical match',
                params: {
                    nx: 21, ny: 21, nz: 1,
                    cellDx: 10, cellDy: 10, cellDz: 10,
                    delta_t_days: 2.0, steps: 50,
                    max_sat_change_per_step: 0.1,
                    initialPressure: 300, initialSaturation: 0.1,
                    injectorEnabled: false,
                    producerControlMode: 'pressure', producerBhp: 100,
                    injectorI: 0, injectorJ: 0,
                    producerI: 10, producerJ: 10,
                    permMode: 'uniform',
                    uniformPermX: 200, uniformPermY: 200, uniformPermZ: 20,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                },
            },
        ],
    },

    waterflood: {
        label: 'Waterflood vs. Analytical (BL)',
        description: 'Waterflood scenarios with Buckley-Leverett analytical comparison',
        cases: [
            {
                key: 'bl_case_a_refined',
                label: 'BL Case A — Refined',
                description: 'Benchmark-grade 96-cell case A with fine timestep',
                params: {
                    nx: 96, ny: 1, nz: 1,
                    cellDx: 10, cellDy: 10, cellDz: 1,
                    delta_t_days: 0.5, steps: 60,
                    max_sat_change_per_step: 0.05,
                    initialPressure: 300, initialSaturation: 0.1,
                    mu_w: 0.5, mu_o: 1.0,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                    permMode: 'uniform',
                    uniformPermX: 2000, uniformPermY: 2000, uniformPermZ: 2000,
                    injectorBhp: 500, producerBhp: 100,
                    rateControlledWells: true,
                    injectorControlMode: 'rate', producerControlMode: 'rate',
                    targetInjectorRate: 350, targetProducerRate: 350,
                    injectorI: 0, injectorJ: 0,
                    producerI: 95, producerJ: 0,
                },
            },
            {
                key: 'bl_case_b_refined',
                label: 'BL Case B — Refined',
                description: 'Benchmark-grade 96-cell case B with different fluid properties',
                params: {
                    nx: 96, ny: 1, nz: 1,
                    cellDx: 10, cellDy: 10, cellDz: 1,
                    delta_t_days: 0.5, steps: 60,
                    max_sat_change_per_step: 0.05,
                    initialPressure: 300, initialSaturation: 0.15,
                    mu_w: 0.6, mu_o: 1.4,
                    s_wc: 0.15, s_or: 0.15, n_w: 2.2, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                    permMode: 'uniform',
                    uniformPermX: 2000, uniformPermY: 2000, uniformPermZ: 2000,
                    injectorBhp: 500, producerBhp: 100,
                    rateControlledWells: true,
                    injectorControlMode: 'rate', producerControlMode: 'rate',
                    targetInjectorRate: 350, targetProducerRate: 350,
                    injectorI: 0, injectorJ: 0,
                    producerI: 95, producerJ: 0,
                },
            },
            {
                key: 'bl_aligned_homogeneous',
                label: 'BL Aligned — Homogeneous',
                description: '48-cell homogeneous 1D-like waterflood',
                params: {
                    nx: 48, ny: 1, nz: 1,
                    cellDx: 5, cellDy: 10, cellDz: 10,
                    delta_t_days: 0.5, steps: 50,
                    max_sat_change_per_step: 0.05,
                    initialPressure: 300, initialSaturation: 0.2,
                    mu_w: 0.5, mu_o: 1.0,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                    permMode: 'uniform',
                    uniformPermX: 150, uniformPermY: 150, uniformPermZ: 150,
                    rateControlledWells: true,
                    injectorControlMode: 'rate', producerControlMode: 'rate',
                    targetInjectorRate: 250, targetProducerRate: 250,
                    injectorI: 0, injectorJ: 0,
                    producerI: 47, producerJ: 0,
                },
            },
            {
                key: 'bl_aligned_mild_capillary',
                label: 'BL Aligned — Mild Capillary',
                description: '48-cell with mild capillary pressure',
                params: {
                    nx: 48, ny: 1, nz: 1,
                    cellDx: 5, cellDy: 10, cellDz: 10,
                    delta_t_days: 0.5, steps: 50,
                    max_sat_change_per_step: 0.05,
                    initialPressure: 300, initialSaturation: 0.2,
                    mu_w: 0.5, mu_o: 1.0,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: true, capillaryPEntry: 0.75, capillaryLambda: 3.2,
                    permMode: 'uniform',
                    uniformPermX: 150, uniformPermY: 150, uniformPermZ: 150,
                    rateControlledWells: true,
                    injectorControlMode: 'rate', producerControlMode: 'rate',
                    targetInjectorRate: 250, targetProducerRate: 250,
                    injectorI: 0, injectorJ: 0,
                    producerI: 47, producerJ: 0,
                },
            },
            {
                key: 'bl_aligned_mobility_balanced',
                label: 'BL Aligned — Mobility Balanced',
                description: '48-cell with balanced mobility ratio',
                params: {
                    nx: 48, ny: 1, nz: 1,
                    cellDx: 5, cellDy: 10, cellDz: 10,
                    delta_t_days: 0.5, steps: 50,
                    max_sat_change_per_step: 0.05,
                    initialPressure: 300, initialSaturation: 0.2,
                    mu_w: 0.8, mu_o: 1.0,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.1, n_o: 1.9,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                    permMode: 'uniform',
                    uniformPermX: 150, uniformPermY: 150, uniformPermZ: 150,
                    rateControlledWells: true,
                    injectorControlMode: 'rate', producerControlMode: 'rate',
                    targetInjectorRate: 250, targetProducerRate: 250,
                    injectorI: 0, injectorJ: 0,
                    producerI: 47, producerJ: 0,
                },
            },
            {
                key: 'waterflood_bl_clean',
                label: 'BL Clean — Ideal Match',
                description: 'Sw=Swc, no capillary, rate-controlled — cleanest BL comparison',
                params: {
                    nx: 48, ny: 1, nz: 1,
                    cellDx: 5, cellDy: 10, cellDz: 10,
                    delta_t_days: 0.5, steps: 50,
                    max_sat_change_per_step: 0.05,
                    initialPressure: 300, initialSaturation: 0.1,
                    mu_w: 0.5, mu_o: 1.0,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                    permMode: 'uniform',
                    uniformPermX: 500, uniformPermY: 500, uniformPermZ: 50,
                    rateControlledWells: true,
                    injectorControlMode: 'rate', producerControlMode: 'rate',
                    targetInjectorRate: 200, targetProducerRate: 200,
                    injectorI: 0, injectorJ: 0,
                    producerI: 47, producerJ: 0,
                },
            },
            {
                key: 'waterflood_unfavorable_mobility',
                label: 'BL Unfavorable Mobility',
                description: 'High oil viscosity (M≈17) — early breakthrough',
                params: {
                    nx: 48, ny: 1, nz: 1,
                    cellDx: 5, cellDy: 10, cellDz: 10,
                    delta_t_days: 0.5, steps: 50,
                    max_sat_change_per_step: 0.05,
                    initialPressure: 300, initialSaturation: 0.1,
                    mu_w: 0.3, mu_o: 5.0,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    gravityEnabled: false,
                    capillaryEnabled: false, capillaryPEntry: 0.0, capillaryLambda: 2.0,
                    permMode: 'uniform',
                    uniformPermX: 500, uniformPermY: 500, uniformPermZ: 50,
                    rateControlledWells: true,
                    injectorControlMode: 'rate', producerControlMode: 'rate',
                    targetInjectorRate: 200, targetProducerRate: 200,
                    injectorI: 0, injectorJ: 0,
                    producerI: 47, producerJ: 0,
                },
            },
        ],
    },

    exploration: {
        label: 'Exploration Scenarios',
        description: 'Multi-dimensional scenarios with varied heterogeneity and physics',
        cases: [
            {
                key: 'baseline_waterflood',
                label: 'Baseline Waterflood',
                description: 'Random heterogeneity with capillary pressure',
                params: {
                    nx: 15, ny: 10, nz: 10,
                    cellDx: 10, cellDy: 10, cellDz: 1,
                    delta_t_days: 0.25, steps: 20,
                    max_sat_change_per_step: 0.1,
                    initialPressure: 300, initialSaturation: 0.3,
                    s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0,
                    capillaryEnabled: true, capillaryPEntry: 5.0, capillaryLambda: 2.0,
                    permMode: 'random',
                    minPerm: 50, maxPerm: 200,
                    useRandomSeed: true, randomSeed: 12345,
                    gravityEnabled: false,
                },
            },
            {
                key: 'high_contrast_layers',
                label: 'High Contrast Layers',
                description: 'Layered permeability with 10x contrast and strong capillary',
                params: {
                    nx: 15, ny: 10, nz: 10,
                    cellDx: 10, cellDy: 10, cellDz: 1,
                    delta_t_days: 0.25, steps: 20,
                    max_sat_change_per_step: 0.1,
                    initialPressure: 320, initialSaturation: 0.25,
                    s_wc: 0.12, s_or: 0.12, n_w: 2.2, n_o: 2.2,
                    capillaryEnabled: true, capillaryPEntry: 8.0, capillaryLambda: 2.5,
                    permMode: 'perLayer',
                    layerPermsX: [30, 40, 60, 90, 150, 400, 150, 90, 60, 40],
                    layerPermsY: [30, 40, 60, 90, 150, 400, 150, 90, 60, 40],
                    layerPermsZ: [3, 4, 6, 9, 15, 40, 15, 9, 6, 4],
                    gravityEnabled: false,
                },
            },
            {
                key: 'viscous_fingering_risk',
                label: 'Viscous Fingering Risk',
                description: 'Unfavourable mobility ratio with strong heterogeneity',
                params: {
                    nx: 15, ny: 10, nz: 10,
                    cellDx: 10, cellDy: 10, cellDz: 1,
                    delta_t_days: 0.25, steps: 20,
                    max_sat_change_per_step: 0.1,
                    initialPressure: 280, initialSaturation: 0.2,
                    s_wc: 0.08, s_or: 0.15, n_w: 1.6, n_o: 2.4,
                    capillaryEnabled: true, capillaryPEntry: 3.0, capillaryLambda: 1.6,
                    permMode: 'random',
                    minPerm: 20, maxPerm: 500,
                    useRandomSeed: true, randomSeed: 987654,
                    gravityEnabled: false,
                },
            },
        ],
    },
};

/** Flat list of all category keys */
export const categoryKeys = Object.keys(caseCatalog);

/** Flat lookup: key → { categoryKey, case } */
export function findCaseByKey(key: string): { categoryKey: string; case: CaseEntry } | null {
    for (const [catKey, cat] of Object.entries(caseCatalog)) {
        const found = cat.cases.find((c) => c.key === key);
        if (found) return { categoryKey: catKey, case: found };
    }
    return null;
}
