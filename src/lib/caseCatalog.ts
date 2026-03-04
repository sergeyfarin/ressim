/**
 * Case catalog — Defines all 92 scenario presets for the faceted UI.
 * Follows a flat array structure with typed facets for filtering.
 */

// ── Facet Types ──
export type CaseMode = 'depletion' | 'waterflood' | 'simulation';
export type CaseGeometry = '1D' | '2D' | '3D';
export type CasePerm = 'uniform' | 'layered' | 'random';

export type CaseFacets = {
    mode: CaseMode;
    geometry: CaseGeometry;
    permeability: CasePerm;
    gravity: boolean;
    capillary: boolean;
    fluidVariation: string[];
    studyType: string[];
    // Future extension slots
    wellModel?: string;
    boundary?: string;
    phases?: string;
};

// ── Rendering Configuration ──
export type CurveLayoutConfig = {
    visible?: boolean;
    disabled?: boolean;
};

export type RateChartLayoutConfig = {
    logScale?: boolean;
    xAxisMode?: 'time' | 'logTime' | 'pvi' | 'cumLiquid' | 'cumInjection';
    ratesExpanded?: boolean;
    cumulativeExpanded?: boolean;
    diagnosticsExpanded?: boolean;
    curves?: Record<string, CurveLayoutConfig>;
};

export type CaseLayoutConfig = {
    rateChart?: RateChartLayoutConfig;
    threeDView?: Record<string, any>;
    swProfile?: Record<string, any>;
};

export type CaseParams = Record<string, any> & {
    layout?: CaseLayoutConfig;
};

export type CaseEntry = {
    key: string;
    label: string;
    description: string;
    runTimeEstimate: 'fast' | 'medium' | 'slow';
    facets: CaseFacets;
    params: CaseParams;
    comparisonGroup?: string;
};

// Facet value lists for UI toggle rendering
export const FACET_OPTIONS = {
    mode: ['depletion', 'waterflood', 'simulation'] as CaseMode[],
    geometry: ['1D', '2D', '3D'] as CaseGeometry[],
    permeability: ['uniform', 'layered', 'random'] as CasePerm[],
    fluidVariation: [
        'heavy-oil', 'light-oil', 'high-comp', 'low-comp',
        'altered-rp', 'endpoint', 'density-contrast', 'low-k'
    ],
    studyType: ['published', 'grid-convergence', 'dt-convergence'],
} as const;

// ── Base Parameter Sets ──

const GLOBAL_DEFAULTS: CaseParams = {
    depth_reference: 0.0,
    volume_expansion_o: 1.0,
    volume_expansion_w: 1.0,
    rho_w: 1000.0,
    rho_o: 800.0,
    well_radius: 0.1,
    well_skin: 0.0,
    max_pressure_change_per_step: 75.0,
    max_well_rate_change_fraction: 0.75,
};

const DEP_BASE: CaseParams = {
    ...GLOBAL_DEFAULTS,
    nx: 48, ny: 1, nz: 1,
    cellDx: 10, cellDy: 10, cellDz: 10,
    delta_t_days: 0.5, steps: 36,
    max_sat_change_per_step: 0.1,
    initialPressure: 300, initialSaturation: 0.1,
    injectorEnabled: false,
    producerControlMode: 'pressure', producerBhp: 100,
    producerI: 0, producerJ: 0,
    permMode: 'uniform',
    uniformPermX: 200, uniformPermY: 200, uniformPermZ: 20,
    reservoirPorosity: 0.2,
    mu_w: 0.5, mu_o: 1.0,
    c_o: 1e-5, c_w: 3e-6, rock_compressibility: 1e-6,
    s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2,
    k_rw_max: 1.0, k_ro_max: 1.0,
    gravityEnabled: false,
    capillaryEnabled: false, capillaryPEntry: 0, capillaryLambda: 2,
    layout: { rateChart: { logScale: true } },
    analyticalSolutionMode: 'depletion',
    analyticalDepletionRateScale: 1.0
};

const WF_BASE: CaseParams = {
    ...GLOBAL_DEFAULTS,
    nx: 48, ny: 1, nz: 1,
    cellDx: 5, cellDy: 10, cellDz: 10,
    delta_t_days: 0.5, steps: 50,
    max_sat_change_per_step: 0.05,
    initialPressure: 300, initialSaturation: 0.1,
    injectorEnabled: true,
    injectorControlMode: 'rate', producerControlMode: 'rate',
    targetInjectorRate: 200, targetProducerRate: 200,
    injectorI: 0, injectorJ: 0,
    producerI: 47, producerJ: 0,
    permMode: 'uniform',
    uniformPermX: 500, uniformPermY: 500, uniformPermZ: 50,
    reservoirPorosity: 0.2,
    mu_w: 0.5, mu_o: 1.0,
    c_o: 1e-5, c_w: 3e-6, rock_compressibility: 1e-6,
    s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2,
    k_rw_max: 1.0, k_ro_max: 1.0,
    gravityEnabled: false,
    capillaryEnabled: false, capillaryPEntry: 0, capillaryLambda: 2,
    analyticalSolutionMode: 'waterflood',
    analyticalDepletionRateScale: 1.0
};

const SIM_BASE: CaseParams = {
    ...WF_BASE,
    nx: 21, ny: 21, nz: 1,
    cellDx: 20, cellDy: 20, cellDz: 10,
    delta_t_days: 0.5, steps: 30,
    max_sat_change_per_step: 0.1,
    injectorControlMode: 'pressure', producerControlMode: 'pressure',
    injectorBhp: 500, producerBhp: 100,
    injectorI: 0, injectorJ: 0,
    producerI: 20, producerJ: 20,
    analyticalSolutionMode: 'none',
};

/**
 * Merge a case's sparse params with the correct base config.
 */
export function resolveParams(sparse: CaseParams, baseKey: CaseMode = 'simulation'): CaseParams {
    const base = baseKey === 'depletion' ? DEP_BASE : baseKey === 'waterflood' ? WF_BASE : SIM_BASE;
    const merged: CaseParams = { ...base, ...sparse };
    // Default well positions if not specified in sparse and base is somehow deficient
    if (merged.injectorI === undefined) merged.injectorI = 0;
    if (merged.injectorJ === undefined) merged.injectorJ = 0;
    if (merged.producerI === undefined) merged.producerI = Number(merged.nx ?? 1) - 1;
    if (merged.producerJ === undefined) merged.producerJ = Number(merged.ny ?? 1) - 1;
    return merged;
}

// ── Case Catalog ──

const rawDepletion = [
    // Geometry & Shape Factor
    ['dep_1d_slab', '1D Slab Baseline', 'Core Dietz CA case for 1D. Strict exponential decline expected.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, {}, 'dep-geo'],
    ['dep_1d_long', 'Long 1D Slab', 'Doubled length. Slower PSS transition, lower initial PI, longer decline constant τ.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 96 }, 'dep-geo'],
    ['dep_1d_short', 'Short 1D Slab', 'Shorter length. Rapid depletion, reaches boundary-dominated flow quickly.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 12, cellDx: 40 }, 'dep-geo'],
    ['dep_2d_center', '2D Center Producer', 'Radial Dietz shape factor (CA≈30.88). Excellent analytical match expected for ideal PSS.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 21, ny: 21, cellDx: 20, cellDy: 20, producerI: 10, producerJ: 10, delta_t_days: 2.0, steps: 50 }, 'dep-geo'],
    ['dep_2d_corner', '2D Corner Producer', 'Corner Dietz (CA≈0.56). Longest linear flow path, slowest pressure drop.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 21, ny: 21, cellDx: 20, cellDy: 20, producerI: 0, producerJ: 0, delta_t_days: 2.0, steps: 50 }, 'dep-geo'],
    ['dep_2d_offcenter', '2D Off-Center', 'Asymmetric drainage area. Intermediate shape factor characteristics.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 21, ny: 21, cellDx: 20, cellDy: 20, producerI: 5, producerJ: 10, delta_t_days: 2.0, steps: 50 }, 'dep-geo'],
    ['dep_2d_rect2', '2D Rectangle 2:1', 'Elongated rectangular shape factor test.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 42, ny: 21, producerI: 21, producerJ: 10, delta_t_days: 2.0, steps: 50 }, 'dep-geo'],
    ['dep_2d_rect4', '2D Rectangle 4:1', 'Highly elongated geometry. Approaches 1D linear flow behavior.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 42, ny: 11, producerI: 21, producerJ: 5, delta_t_days: 2.0, steps: 50, cellDy: 40 }, 'dep-geo'],
    ['dep_3d_center', '3D Center Producer', '3D shape factor effects. Vertical communication adds total drainage volume.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 11, ny: 11, nz: 5, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-geo'],
    ['dep_3d_tall', '3D Tall Reservoir', 'Thick 3D reservoir with more vertical cross-flow dynamics.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 11, ny: 11, nz: 10, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-geo'],

    // Grid Convergence
    ['dep_conv_nx12', '12 Cells (Coarse)', 'Coarse spatial resolution. Noticeable discretization error vs analytical.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['grid-convergence'] }, { nx: 12, cellDx: 40 }, 'dep-grid'],
    ['dep_conv_nx24', '24 Cells (Medium)', 'Medium spatial refinement.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['grid-convergence'] }, { nx: 24, cellDx: 20 }, 'dep-grid'],
    ['dep_conv_nx48', '48 Cells (Fine)', 'Fine resolution. Standard baseline for 1D.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['grid-convergence'] }, {}, 'dep-grid'],
    ['dep_conv_nx96', '96 Cells (V. Fine)', 'Very fine spatial mesh. Asymptotic numerical convergence target.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['grid-convergence'] }, { nx: 96, cellDx: 5 }, 'dep-grid'],

    // Timestep Convergence
    ['dep_dt_5d', 'dt = 5.0 days', 'Large timestep. Sub-stepping engages heavily, potential numerical smearing.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['dt-convergence'] }, { delta_t_days: 5.0, steps: 10 }, 'dep-dt'],
    ['dep_dt_1d', 'dt = 1.0 day', 'Medium timestep size.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['dt-convergence'] }, { delta_t_days: 1.0 }, 'dep-dt'],
    ['dep_dt_025d', 'dt = 0.25 days', 'Fine timestep. Excellent temporal resolution of the decline curve.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['dt-convergence'] }, { delta_t_days: 0.25, steps: 72 }, 'dep-dt'],

    // Heterogeneity
    ['dep_het_mild', 'Mild Random Perm', 'Slight stochastic variation in 2D. Mild deviation from ideal analytical.', 'medium', { geometry: '2D', permeability: 'random', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 21, ny: 21, cellDx: 20, cellDy: 20, permMode: 'random', minPerm: 100, maxPerm: 300, useRandomSeed: true, randomSeed: 42, delta_t_days: 2.0, steps: 50 }, 'dep-het'],
    ['dep_het_strong', 'Strong Random Perm', 'Significant random heterogeneity. Shows limitations of homogeneous analytical models.', 'medium', { geometry: '2D', permeability: 'random', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 21, ny: 21, cellDx: 20, cellDy: 20, permMode: 'random', minPerm: 20, maxPerm: 500, useRandomSeed: true, randomSeed: 42, delta_t_days: 2.0, steps: 50 }, 'dep-het'],
    ['dep_het_layered', '5-Layer System', 'Vertical variations in permeability. Differential layer depletion rates.', 'slow', { geometry: '3D', permeability: 'layered', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 11, ny: 11, nz: 5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5], delta_t_days: 2.0, steps: 50 }, 'dep-het'],
    ['dep_het_contrast', '10:1 Layer Contrast', 'Strong vertical contrast. Significant cross-flow during depletion.', 'slow', { geometry: '3D', permeability: 'layered', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 11, ny: 11, nz: 5, permMode: 'perLayer', layerPermsX: [20, 20, 200, 200, 20], layerPermsY: [20, 20, 200, 200, 20], layerPermsZ: [2, 2, 20, 20, 2], delta_t_days: 2.0, steps: 50 }, 'dep-het'],

    // Fluid Properties
    ['dep_heavy_oil', 'Heavy Oil', 'High viscosity (10 cP). Low productivity index, much slower decline tau.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['heavy-oil'], studyType: [] }, { mu_o: 10.0, steps: 50 }, 'dep-fluid'],
    ['dep_light_oil', 'Light Oil', 'Low viscosity (0.3 cP). High PI, very rapid pressure drop initially.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['light-oil'], studyType: [] }, { mu_o: 0.3 }, 'dep-fluid'],
    ['dep_high_comp', 'High Compressibility', 'Increased oil compressibility. More expansion support, slower decline.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['high-comp'], studyType: [] }, { c_o: 5e-5, steps: 50 }, 'dep-fluid'],
    ['dep_low_comp', 'Low Compressibility', 'Stiff fluid. Very small time constant, fast exponential decay.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['low-comp'], studyType: [] }, { c_o: 1e-6 }, 'dep-fluid'],

    // Rel-Perm & Saturation
    ['dep_rp_mobile_w', 'Two-Phase Mobile Water', 'Initial saturation > Swc. Co-production of water and oil.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { initialSaturation: 0.3 }, 'dep-rp'],
    ['dep_rp_low_kro', 'Low k_ro Endpoint', 'Reduced max oil mobility. Lowers effective PI without changing viscosity.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['endpoint'], studyType: [] }, { k_ro_max: 0.6, steps: 50 }, 'dep-rp'],
    ['dep_rp_narrow', 'Narrow Saturation Window', 'High Swc and Sor. Shrinks mobile saturation range.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { s_wc: 0.25, s_or: 0.25 }, 'dep-rp'],

    // Gravity
    ['dep_grav_3d', '3D Gravity Drainage', 'Vertical segregation occurs simultaneously with pressure depletion.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: false, fluidVariation: [], studyType: [] }, { nx: 11, ny: 11, nz: 5, gravityEnabled: true, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-grav'],
    ['dep_grav_density', 'Strong Density Contrast', 'Enhanced gravity segregation during primary depletion.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: false, fluidVariation: ['density-contrast'], studyType: [] }, { nx: 11, ny: 11, nz: 5, gravityEnabled: true, rho_o: 600, rho_w: 1050, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-grav'],

    // Capillary
    ['dep_cap_mild', 'Mild Capillary', 'Slight capillary distribution effects on pressure transient.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: true, fluidVariation: [], studyType: [] }, { capillaryEnabled: true, capillaryPEntry: 0.5, capillaryLambda: 3, initialSaturation: 0.3 }, 'dep-cap'],
    ['dep_cap_strong', 'Strong Capillary', 'Significant capillary retention alters phase mobilities.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: true, fluidVariation: [], studyType: [] }, { capillaryEnabled: true, capillaryPEntry: 3.0, capillaryLambda: 2, initialSaturation: 0.3 }, 'dep-cap'],

    // Multi-Factor
    ['dep_grav_cap_3d', 'Gravity + Capillary 3D', 'Combined Pc-G equilibrium dynamics during depletion.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: true, fluidVariation: [], studyType: [] }, { nx: 11, ny: 11, nz: 5, producerI: 5, producerJ: 5, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5, initialSaturation: 0.3, delta_t_days: 2.0, steps: 50 }, 'dep-multi'],
    ['dep_het_grav_3d', 'Layered + Gravity 3D', 'Layered heterogeneity interacting with gravity segregation.', 'slow', { geometry: '3D', permeability: 'layered', gravity: true, capillary: false, fluidVariation: [], studyType: [] }, { nx: 11, ny: 11, nz: 5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5], gravityEnabled: true, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-multi'],
    ['dep_het_cap_2d', 'Random + Capillary 2D', 'Capillary smearing across heterogeneous permeability fields.', 'medium', { geometry: '2D', permeability: 'random', gravity: false, capillary: true, fluidVariation: [], studyType: [] }, { nx: 21, ny: 21, permMode: 'random', minPerm: 50, maxPerm: 300, useRandomSeed: true, randomSeed: 42, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5, initialSaturation: 0.3, delta_t_days: 2.0, steps: 50 }, 'dep-multi']
] as const;

const rawWaterflood = [
    // Mobility Ratio
    ['wf_mob_piston', 'Piston-Like (M≈0.5)', 'Favorable mobility ratio. Very late breakthrough, near 100% initial sweep.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { mu_w: 1.0, mu_o: 0.5 }, 'wf-mob'],
    ['wf_mob_unit', 'Unit Mobility', 'Equal phase viscosities. Standard benchmark response.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { mu_w: 0.5, mu_o: 0.5 }, 'wf-mob'],
    ['wf_mob_base', 'Base Case (M≈2)', 'Mildly unfavorable mobility ratio. Expected standard BL shock front.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, {}, 'wf-mob'],
    ['wf_mob_adverse', 'Adverse (M≈5)', 'Unfavorable ratio. Early breakthrough, extended two-phase production tail.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { mu_w: 0.3, mu_o: 3.0 }, 'wf-mob'],
    ['wf_mob_viscous', 'Viscous (M≈17)', 'Highly unfavorable. Theoretical viscous fingering tendencies.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { mu_w: 0.3, mu_o: 5.0 }, 'wf-mob'],
    ['wf_mob_heavy', 'Heavy Oil (M≈50)', 'Extreme mobility ratio. Very early water breakthrough, poor sweep efficiency.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['heavy-oil'], studyType: [] }, { mu_w: 0.3, mu_o: 15.0 }, 'wf-mob'],

    // Corey Exponents
    ['wf_rp_linear', 'Linear Rel-Perm (n=1)', 'Straight-line relative permeability curves (Brooks-Corey n=1).', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { n_w: 1, n_o: 1 }, 'wf-corey'],
    ['wf_rp_mild', 'Mild Curvature (n=1.5)', 'Low curvature endpoints. Less acute shock front.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { n_w: 1.5, n_o: 1.5 }, 'wf-corey'],
    ['wf_rp_base', 'Standard Corey (n=2)', 'Typical sandstone Corey exponent.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { n_w: 2, n_o: 2 }, 'wf-corey'],
    ['wf_rp_high', 'High Curvature (n=3)', 'More pronounced S-shape in fractional flow.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { n_w: 3, n_o: 3 }, 'wf-corey'],
    ['wf_rp_extreme', 'Extreme (n=4)', 'Very non-linear flow. Produces a very sharp BL shock front.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { n_w: 4, n_o: 4 }, 'wf-corey'],
    ['wf_rp_ww', 'Water-Wet Asymmetric', 'Water-wet signature (n_water < n_oil typically).', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { n_w: 1.5, n_o: 3.0 }, 'wf-corey'],
    ['wf_rp_ow', 'Oil-Wet Asymmetric', 'Oil-wet signature (n_oil < n_water typically).', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { n_w: 3.0, n_o: 1.5 }, 'wf-corey'],

    // Saturation Endpoints
    ['wf_sat_low_swc', 'Low Swc (0.05)', 'Low connate water, wide mobile oil range. High ultimate recovery potential.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { s_wc: 0.05, initialSaturation: 0.05 }, 'wf-sat'],
    ['wf_sat_high_swc', 'High Swc (0.25)', 'High connate water. Faster water propagation through remaining pore volume.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { s_wc: 0.25, initialSaturation: 0.25 }, 'wf-sat'],
    ['wf_sat_low_sor', 'Low Sor (0.05)', 'Efficient microscopic sweep. Longer productive life.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { s_or: 0.05 }, 'wf-sat'],
    ['wf_sat_high_sor', 'High Sor (0.3)', 'Large residual oil saturation. Poor ultimate expected recovery.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { s_or: 0.3 }, 'wf-sat'],
    ['wf_sat_narrow', 'Narrow Mobile Range', 'Combination of high Swc and high Sor. Very brief mobile oil production window.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['altered-rp'], studyType: [] }, { s_wc: 0.25, s_or: 0.25, initialSaturation: 0.25 }, 'wf-sat'],

    // Endpoint Scaling
    ['wf_ep_low_krw', 'Low krw_max (0.3)', 'Reduced maximum water permeability. Restricts injectivity post-breakthrough.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['endpoint'], studyType: [] }, { k_rw_max: 0.3 }, 'wf-ep'],
    ['wf_ep_low_kro', 'Low kro_max (0.6)', 'Reduced maximum oil relative permeability. Lowers initial productivity.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['endpoint'], studyType: [] }, { k_ro_max: 0.6 }, 'wf-ep'],
    ['wf_ep_both', 'Both Endpoints Reduced', 'Combined impact of low krw_max and kro_max.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['endpoint'], studyType: [] }, { k_rw_max: 0.3, k_ro_max: 0.6 }, 'wf-ep'],

    // Grid Convergence
    ['wf_conv_nx12', '12 Cells (Coarse)', 'Heavy numerical diffusion smears the shock front. Analytical match will be poor.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['grid-convergence'] }, { nx: 12, cellDx: 20, producerI: 11 }, 'wf-grid'],
    ['wf_conv_nx24', '24 Cells (Medium)', 'Medium numerical diffusion.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['grid-convergence'] }, { nx: 24, cellDx: 10, producerI: 23 }, 'wf-grid'],
    ['wf_conv_nx48', '48 Cells (Fine)', 'Fine grid baseline. Acceptable trade-off of resolution and speed.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['grid-convergence'] }, {}, 'wf-grid'],
    ['wf_conv_nx96', '96 Cells (V. Fine)', 'Very fine grid target for resolving the Buckley-Leverett shock front sharply.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['grid-convergence'] }, { nx: 96, cellDx: 2.5, producerI: 95 }, 'wf-grid'],

    // Published Benchmarks
    ['wf_pub_a', 'SPE BL Case A', 'Standardized verification dataset parameterization for Buckley-Leverett. (k=2000mD, L=960m, q=350)', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['published'] }, { nx: 96, cellDx: 10, cellDz: 1, uniformPermX: 2000, uniformPermY: 2000, uniformPermZ: 2000, targetInjectorRate: 350, targetProducerRate: 350, producerI: 95 }, 'wf-pub'],
    ['wf_pub_b', 'SPE BL Case B', 'Second verification dataset with altered viscosities and corey parameters.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: ['published'] }, { nx: 96, cellDx: 10, cellDz: 1, initialSaturation: 0.15, mu_w: 0.6, mu_o: 1.4, s_wc: 0.15, s_or: 0.15, n_w: 2.2, uniformPermX: 2000, uniformPermY: 2000, uniformPermZ: 2000, targetInjectorRate: 350, targetProducerRate: 350, producerI: 95 }, 'wf-pub'],

    // Capillary Effects
    ['wf_cap_mild', 'Mild Capillary', 'Slight capillary distribution smears the sharp BL shock front slightly.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: true, fluidVariation: [], studyType: [] }, { capillaryEnabled: true, capillaryPEntry: 0.5, capillaryLambda: 3 }, 'wf-cap'],
    ['wf_cap_mod', 'Moderate Capillary', 'Noticeable deviation from idealized BL theory due to capillary forces.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: true, fluidVariation: [], studyType: [] }, { capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2.5 }, 'wf-cap'],
    ['wf_cap_strong', 'Strong Capillary', 'Major front dispersion. The analytical Buckley-Leverett theory is no longer applicable.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: true, fluidVariation: [], studyType: [] }, { capillaryEnabled: true, capillaryPEntry: 3.0, capillaryLambda: 2 }, 'wf-cap'],
] as const;

const rawSimulation = [
    // 2D Waterfloods
    ['sim_2d_uniform', '2D Uniform Sweep', 'Standard corner-to-corner areal sweep baseline.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, {}, 'sim-2d'],
    ['sim_2d_random', '2D Random Perm', 'Areal sweep distorted by stochastic permeability variations.', 'medium', { geometry: '2D', permeability: 'random', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { permMode: 'random', minPerm: 50, maxPerm: 300, useRandomSeed: true, randomSeed: 42 }, 'sim-2d'],
    ['sim_2d_channel', 'High-Perm Channel', 'Thief zone causing premature breakthrough.', 'medium', { geometry: '2D', permeability: 'layered', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { permMode: 'perLayer', nz: 3, layerPermsX: [30, 300, 30], layerPermsY: [30, 300, 30], layerPermsZ: [3, 30, 3], cellDz: 3 }, 'sim-2d'],
    ['sim_2d_barrier', 'Low-Perm Barrier', 'Flow diversion around a central low-permeability obstacle.', 'medium', { geometry: '2D', permeability: 'layered', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { permMode: 'perLayer', nz: 3, layerPermsX: [200, 5, 200], layerPermsY: [200, 5, 200], layerPermsZ: [20, 0.5, 20], cellDz: 3 }, 'sim-2d'],
    ['sim_2d_cap_random', 'Random + Capillary', 'Capillary forces interacting with a heterogeneous permeability field.', 'medium', { geometry: '2D', permeability: 'random', gravity: false, capillary: true, fluidVariation: [], studyType: [] }, { permMode: 'random', minPerm: 50, maxPerm: 300, useRandomSeed: true, randomSeed: 42, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5 }, 'sim-2d'],

    // 3D Waterfloods
    ['sim_3d_uniform', '3D Uniform', 'Baseline 3D volumetric displacement. Idealized layer-by-layer sweep.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 15, ny: 10, nz: 5, producerI: 14, producerJ: 9, cellDz: 2 }, 'sim-3d'],
    ['sim_3d_layered', '3D Layered', 'Stratified flow with differential layer breakthrough times.', 'slow', { geometry: '3D', permeability: 'layered', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 15, ny: 10, nz: 5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5], producerI: 14, producerJ: 9, cellDz: 2 }, 'sim-3d'],
    ['sim_3d_random', '3D Random', 'Complex 3D stochastic flow network and sweep pattern.', 'slow', { geometry: '3D', permeability: 'random', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 15, ny: 10, nz: 5, permMode: 'random', minPerm: 50, maxPerm: 200, useRandomSeed: true, randomSeed: 12345, producerI: 14, producerJ: 9, cellDz: 2 }, 'sim-3d'],
    ['sim_3d_contrast', '3D 10:1 Layers', 'High contrast multiple layers causing severe thief zones in 3D.', 'slow', { geometry: '3D', permeability: 'layered', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 15, ny: 10, nz: 10, permMode: 'perLayer', layerPermsX: [30, 40, 60, 90, 150, 400, 150, 90, 60, 40], layerPermsY: [30, 40, 60, 90, 150, 400, 150, 90, 60, 40], layerPermsZ: [3, 4, 6, 9, 15, 40, 15, 9, 6, 4], producerI: 14, producerJ: 9, cellDz: 1 }, 'sim-3d'],

    // Gravity
    ['sim_grav_column', 'Gravity Column', 'Pure vertical gravity segregation with no injection.', 'fast', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: false, fluidVariation: [], studyType: [] }, { nx: 1, ny: 1, nz: 20, cellDz: 2, injectorEnabled: false, gravityEnabled: true, initialSaturation: 0.5, steps: 50 }, 'sim-grav'],
    ['sim_grav_override', 'Gravity Override 3D', 'Water underrun in thick reservoir due to density differences.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: false, fluidVariation: [], studyType: [] }, { nx: 11, ny: 11, nz: 5, cellDz: 3, gravityEnabled: true, producerI: 10, producerJ: 10, steps: 40 }, 'sim-grav'],
    ['sim_grav_het', 'Gravity + Layers', 'Competition between gravity forces and stratigraphic high-perm layers.', 'slow', { geometry: '3D', permeability: 'layered', gravity: true, capillary: false, fluidVariation: [], studyType: [] }, { nx: 11, ny: 11, nz: 5, cellDz: 3, gravityEnabled: true, permMode: 'perLayer', layerPermsX: [50, 200, 50, 200, 50], layerPermsY: [50, 200, 50, 200, 50], layerPermsZ: [5, 20, 5, 20, 5], producerI: 10, producerJ: 10, steps: 40 }, 'sim-grav'],
    ['sim_grav_density', 'Strong Density Contrast', 'Exaggerated phase densities driving enhanced vertical sweep anomalies.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: false, fluidVariation: ['density-contrast'], studyType: [] }, { nx: 11, ny: 11, nz: 5, cellDz: 3, gravityEnabled: true, rho_o: 600, rho_w: 1050, producerI: 10, producerJ: 10, steps: 40 }, 'sim-grav'],

    // Capillary-Gravity Equilibrium
    ['sim_pcg_column', 'Pc-G Column', 'Formation of a capillary transition zone balancing gravity.', 'fast', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: true, fluidVariation: [], studyType: [] }, { nx: 1, ny: 1, nz: 10, cellDz: 3, injectorEnabled: false, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2.5, initialSaturation: 0.5, steps: 50 }, 'sim-pcg'],
    ['sim_pcg_transition', 'Transition Zone', 'Dynamic flow interaction with a naturally formed transition zone.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: true, fluidVariation: [], studyType: [] }, { nx: 5, ny: 5, nz: 10, cellDz: 3, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2.5, initialSaturation: 0.4, producerI: 4, producerJ: 4, steps: 40 }, 'sim-pcg'],
    ['sim_pcg_het', 'Pc-G + Layers', 'Complex layered capillary-gravity equilibrium trapping.', 'slow', { geometry: '3D', permeability: 'layered', gravity: true, capillary: true, fluidVariation: [], studyType: [] }, { nx: 5, ny: 5, nz: 10, cellDz: 3, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2.5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50, 50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50, 50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5, 5, 10, 20, 10, 5], initialSaturation: 0.4, producerI: 4, producerJ: 4, steps: 40 }, 'sim-pcg'],

    // Well Control
    ['sim_well_bhp', 'BHP-Driven', 'Wells operating under constant bottom hole pressure.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, {}, 'sim-well'],
    ['sim_well_rate', 'Rate-Driven', 'Wells constrained by volumetric rate targets.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { injectorControlMode: 'rate', producerControlMode: 'rate', targetInjectorRate: 200, targetProducerRate: 200 }, 'sim-well'],

    // Realistic Scenarios
    ['sim_real_sand', 'North Sea Sandstone', 'Analogous rock & fluid params for a typical North Sea clastic reservoir.', 'slow', { geometry: '3D', permeability: 'layered', gravity: true, capillary: true, fluidVariation: [], studyType: [] }, { nx: 15, ny: 10, nz: 5, permMode: 'perLayer', layerPermsX: [80, 120, 200, 150, 60], layerPermsY: [80, 120, 200, 150, 60], layerPermsZ: [8, 12, 20, 15, 6], gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 0.8, capillaryLambda: 2.5, cellDz: 3, producerI: 14, producerJ: 9, steps: 40 }, 'sim-real'],
    ['sim_real_carb', 'Carbonate Primary Dep', 'Tight carbonate with high capillary pressure and natural depletion.', 'slow', { geometry: '3D', permeability: 'layered', gravity: true, capillary: true, fluidVariation: ['low-k'], studyType: [] }, { nx: 11, ny: 11, nz: 5, injectorEnabled: false, permMode: 'perLayer', layerPermsX: [10, 30, 50, 30, 10], layerPermsY: [10, 30, 50, 30, 10], layerPermsZ: [1, 3, 5, 3, 1], gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 3.0, capillaryLambda: 1.5, mu_o: 5, cellDz: 3, producerI: 5, producerJ: 5, producerControlMode: 'pressure', producerBhp: 80, steps: 50 }, 'sim-real'],
    ['sim_real_heavy', 'Heavy Oil Cold Flow', 'Viscous-dominated secondary recovery in a dipping structural play setup.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: false, fluidVariation: ['heavy-oil'], studyType: [] }, { mu_o: 50, gravityEnabled: true, nz: 5, cellDz: 3, producerI: 10, producerJ: 10, steps: 50 }, 'sim-real'],

    // Stress Tests
    ['sim_stress_highq', 'High Injection Rate', 'CFL sub-stepping heavily taxed due to rapid flux.', 'fast', { geometry: '1D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 48, ny: 1, nz: 1, cellDx: 5, injectorControlMode: 'rate', producerControlMode: 'rate', targetInjectorRate: 2000, targetProducerRate: 2000, injectorI: 0, producerI: 47, steps: 30 }, 'sim-stress'],
    ['sim_stress_lowk', 'Tight Rock (k=1 mD)', 'Verify solver stability at very low permeabilities/fluxes.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: false, capillary: false, fluidVariation: ['low-k'], studyType: [] }, { uniformPermX: 1, uniformPermY: 1, uniformPermZ: 0.1, steps: 50 }, 'sim-stress'],
    ['sim_stress_large', 'Large 3D Grid', '4000 active cells to test WebAssembly worker performance limits.', 'slow', { geometry: '3D', permeability: 'random', gravity: false, capillary: false, fluidVariation: [], studyType: [] }, { nx: 20, ny: 20, nz: 10, cellDz: 2, permMode: 'random', minPerm: 50, maxPerm: 200, useRandomSeed: true, randomSeed: 77, producerI: 19, producerJ: 19, steps: 20 }, 'sim-stress'],

    // Multi-Factor
    ['sim_3d_all', '3D Full Physics', 'Gravity + Capillary + Heterogeneity all actively engaged.', 'slow', { geometry: '3D', permeability: 'layered', gravity: true, capillary: true, fluidVariation: [], studyType: [] }, { nx: 15, ny: 10, nz: 5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5], gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5, producerI: 14, producerJ: 9, cellDz: 2, steps: 30 }, 'sim-multi'],
    ['sim_heavy_grav', 'Heavy Oil + Gravity 3D', 'Sluggish heavy oil gravity segregation dynamics.', 'slow', { geometry: '3D', permeability: 'uniform', gravity: true, capillary: false, fluidVariation: ['heavy-oil'], studyType: [] }, { nx: 11, ny: 11, nz: 5, mu_o: 10, gravityEnabled: true, cellDz: 3, producerI: 10, producerJ: 10, steps: 40 }, 'sim-multi'],
    ['sim_2d_cap_grav', '2D Capillary + Gravity', 'Cross-sectional study of combined downward gravity and capillary smearing.', 'medium', { geometry: '2D', permeability: 'uniform', gravity: true, capillary: true, fluidVariation: [], studyType: [] }, { ny: 1, nz: 21, cellDz: 3, cellDy: 20, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5, producerI: 20, producerJ: 0, injectorI: 0, injectorJ: 0, steps: 30 }, 'sim-multi'],
] as const;

export const caseCatalog: CaseEntry[] = [
    ...rawDepletion.map(c => ({
        key: c[0], label: c[1], description: c[2], runTimeEstimate: c[3] as any,
        facets: { mode: 'depletion', ...c[4] } as unknown as CaseFacets,
        params: resolveParams(c[5], 'depletion'),
        comparisonGroup: c[6],
    })),
    ...rawWaterflood.map(c => ({
        key: c[0], label: c[1], description: c[2], runTimeEstimate: c[3] as any,
        facets: { mode: 'waterflood', ...c[4] } as unknown as CaseFacets,
        params: resolveParams(c[5], 'waterflood'),
        comparisonGroup: c[6],
    })),
    ...rawSimulation.map(c => ({
        key: c[0], label: c[1], description: c[2], runTimeEstimate: c[3] as any,
        facets: { mode: 'simulation', ...c[4] } as unknown as CaseFacets,
        params: resolveParams(c[5], 'simulation'),
        comparisonGroup: c[6],
    }))
];

export function findCaseByKey(key: string): CaseEntry | null {
    return caseCatalog.find(c => c.key === key) ?? null;
}
