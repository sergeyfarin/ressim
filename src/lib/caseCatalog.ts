/**
 * Case catalog — 92+ scenario presets with guided toggle selection.
 * Each case is uniquely addressed by its facets OR by facets + variant.
 */

// ── Facet Types ──
export type CaseMode = 'depletion' | 'waterflood' | 'simulation' | 'benchmark';
export type CaseGeometry = '1D' | '2D' | '3D';
export type CasePerm = 'uniform' | 'layered' | 'random';
export type WellPosition = 'corner' | 'center' | 'off-center' | 'end-to-end';
export type FluidPreset = 'standard' | 'heavy-oil' | 'light-oil' | 'high-comp' | 'low-comp';

export type CaseFacets = {
    mode: CaseMode;
    geometry: CaseGeometry;
    wellPosition: WellPosition;
    permeability: CasePerm;
    gravity: boolean;
    capillary: boolean;
    fluids: FluidPreset;
    focus: string;   // mode-dependent value
};

export type CaseEntry = {
    key: string;
    label: string;
    description: string;
    runTimeEstimate: 'fast' | 'medium' | 'slow';
    facets: CaseFacets;
    params: CaseParams;
    comparisonGroup?: string;
    variantGroup?: string;   // groups cases sharing the same toggle combo
    variantLabel?: string;   // label shown in variant pill
};

// ── Focus Options per Mode ──
export const FOCUS_OPTIONS: Record<string, { value: string; label: string }[]> = {
    depletion: [
        { value: 'shape-factor', label: 'Shape Factor' },
        { value: 'rel-perm', label: 'Rel-Perm' },
        { value: 'grid-refinement', label: 'Grid Refinement' },
        { value: 'time-refinement', label: 'Time Refinement' },
    ],
    waterflood: [
        { value: 'displacement', label: 'Displacement' },
        { value: 'mobility-ratio', label: 'Mobility Ratio' },
        { value: 'corey-curves', label: 'Corey Curves' },
        { value: 'saturation-limits', label: 'Saturation Limits' },
        { value: 'kr-endpoints', label: 'kr Endpoints' },
        { value: 'grid-refinement', label: 'Grid Refinement' },
    ],
    simulation: [
        { value: 'sweep', label: 'Sweep' },
        { value: 'thief-zone', label: 'Thief Zone' },
        { value: 'gravity-effects', label: 'Gravity Effects' },
        { value: 'pcg-balance', label: 'Pc-G Balance' },
        { value: 'well-control', label: 'Well Control' },
        { value: 'stress-test', label: 'Stress Test' },
        { value: 'realistic', label: 'Realistic' },
    ],
    benchmark: [
        { value: 'spe-bl', label: 'SPE Buckley-Leverett' },
    ],
};

// ── Rendering Configuration ──
export type CurveLayoutConfig = { visible?: boolean; disabled?: boolean };
export type RateChartLayoutConfig = {
    logScale?: boolean;
    xAxisMode?: 'time' | 'logTime' | 'pvi' | 'cumLiquid' | 'cumInjection';
    ratesExpanded?: boolean; cumulativeExpanded?: boolean; diagnosticsExpanded?: boolean;
    curves?: Record<string, CurveLayoutConfig>;
};
export type CaseLayoutConfig = {
    rateChart?: RateChartLayoutConfig;
    threeDView?: Record<string, any>;
    swProfile?: Record<string, any>;
};
export type CaseParams = Record<string, any> & { layout?: CaseLayoutConfig };

// ── Facet Options for UI ──
export const FACET_OPTIONS = {
    mode: ['depletion', 'waterflood', 'simulation', 'benchmark'] as CaseMode[],
    geometry: ['1D', '2D', '3D'] as CaseGeometry[],
    wellPosition: ['end-to-end', 'corner', 'center', 'off-center'] as WellPosition[],
    permeability: ['uniform', 'layered', 'random'] as CasePerm[],
    fluids: ['standard', 'heavy-oil', 'light-oil', 'high-comp', 'low-comp'] as FluidPreset[],
} as const;

// ── Base Parameter Sets ──
const GLOBAL_DEFAULTS: CaseParams = {
    depth_reference: 0.0, volume_expansion_o: 1.0, volume_expansion_w: 1.0,
    rho_w: 1000.0, rho_o: 800.0, well_radius: 0.1, well_skin: 0,
    max_pressure_change_per_step: 75, max_well_rate_change_fraction: 0.75,
};

const DEP_BASE: CaseParams = {
    ...GLOBAL_DEFAULTS,
    nx: 48, ny: 1, nz: 1,
    cellDx: 10, cellDy: 10, cellDz: 5,
    delta_t_days: 0.5, steps: 36,
    max_sat_change_per_step: 0.1,
    initialPressure: 300, initialSaturation: 0.1,
    injectorEnabled: false,
    producerControlMode: 'pressure', producerBhp: 100,
    producerI: 0, producerJ: 0,
    permMode: 'uniform',
    uniformPermX: 200, uniformPermY: 200, uniformPermZ: 20,
    reservoirPorosity: 0.2,
    mu_w: 0.5, mu_o: 1.0, c_o: 1e-5, c_w: 3e-6, rock_compressibility: 1e-6,
    s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2, k_rw_max: 1.0, k_ro_max: 1.0,
    gravityEnabled: false,
    capillaryEnabled: false, capillaryPEntry: 0, capillaryLambda: 2,
    analyticalSolutionMode: 'depletion', analyticalDepletionRateScale: 1.0,
    layout: { rateChart: { logScale: true } },
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
    injectorI: 0, injectorJ: 0, producerI: 47, producerJ: 0,
    permMode: 'uniform',
    uniformPermX: 500, uniformPermY: 500, uniformPermZ: 50,
    reservoirPorosity: 0.2,
    mu_w: 0.5, mu_o: 1.0,
    s_wc: 0.1, s_or: 0.1, n_w: 2, n_o: 2, k_rw_max: 1.0, k_ro_max: 1.0,
    c_o: 1e-5, c_w: 3e-6, rock_compressibility: 1e-6,
    gravityEnabled: false,
    capillaryEnabled: false, capillaryPEntry: 0, capillaryLambda: 2,
    analyticalSolutionMode: 'waterflood', analyticalDepletionRateScale: 1.0,
};

const SIM_BASE: CaseParams = {
    ...WF_BASE,
    nx: 21, ny: 21, nz: 1,
    cellDx: 20, cellDy: 20, cellDz: 10,
    delta_t_days: 0.5, steps: 30,
    max_sat_change_per_step: 0.1,
    injectorControlMode: 'pressure', producerControlMode: 'pressure',
    injectorBhp: 500, producerBhp: 100,
    injectorI: 0, injectorJ: 0, producerI: 20, producerJ: 20,
    analyticalSolutionMode: 'none',
};

export function resolveParams(sparse: CaseParams, baseKey: CaseMode = 'simulation'): CaseParams {
    const base = baseKey === 'depletion' ? DEP_BASE
        : baseKey === 'waterflood' || baseKey === 'benchmark' ? WF_BASE
            : SIM_BASE;
    const merged: CaseParams = { ...base, ...sparse };
    if (merged.injectorI === undefined) merged.injectorI = 0;
    if (merged.injectorJ === undefined) merged.injectorJ = 0;
    if (merged.producerI === undefined) merged.producerI = Number(merged.nx ?? 1) - 1;
    if (merged.producerJ === undefined) merged.producerJ = Number(merged.ny ?? 1) - 1;
    return merged;
}

// ── Helper: build a CaseEntry from compact tuple ──
type RawCase = [
    key: string, label: string, description: string,
    runtime: 'fast' | 'medium' | 'slow',
    facets: Omit<CaseFacets, 'mode'>,
    params: CaseParams,
    compGroup: string,
    variantGroup?: string,
    variantLabel?: string,
];

function buildCases(mode: CaseMode, raw: readonly RawCase[]): CaseEntry[] {
    return raw.map(c => ({
        key: c[0], label: c[1], description: c[2], runTimeEstimate: c[3],
        facets: { mode, ...c[4] } as CaseFacets,
        params: resolveParams(c[5], mode),
        comparisonGroup: c[6],
        variantGroup: c[7],
        variantLabel: c[8],
    }));
}

// ── Shorthand for facets ──
const F = (
    geo: CaseGeometry, well: WellPosition, perm: CasePerm,
    grav: boolean, cap: boolean, fluids: FluidPreset, focus: string
): Omit<CaseFacets, 'mode'> => ({
    geometry: geo, wellPosition: well, permeability: perm,
    gravity: grav, capillary: cap, fluids, focus,
});

// ═══════════════════════════════════════════════════════════════
//  DEPLETION CASES (35)
// ═══════════════════════════════════════════════════════════════
const depletionCases: RawCase[] = [
    // ── Shape Factor: 1D ──
    ['dep_1d_slab', '1D Slab Baseline', 'Core Dietz CA for 1D slab.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'shape-factor'),
        {}, 'dep-geo', 'dep-1d-shape', 'Baseline'],
    ['dep_1d_long', 'Long 1D Slab', '2× length → lower rate, longer τ.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'shape-factor'),
        { nx: 96 }, 'dep-geo', 'dep-1d-shape', 'Long (2×)'],
    ['dep_1d_short', 'Short 1D Slab', 'Rapid depletion, fast boundary-dominated flow.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'shape-factor'),
        { nx: 12, cellDx: 40 }, 'dep-geo', 'dep-1d-shape', 'Short (¼)'],

    // ── Shape Factor: 2D well positions ──
    ['dep_2d_center', '2D Center Producer', 'Radial Dietz CA≈30.88. Best analytical match.', 'medium',
        F('2D', 'center', 'uniform', false, false, 'standard', 'shape-factor'),
        { nx: 21, ny: 21, cellDx: 20, cellDy: 20, producerI: 10, producerJ: 10, delta_t_days: 2.0, steps: 50 }, 'dep-geo'],
    ['dep_2d_corner', '2D Corner Producer', 'Corner Dietz (CA≈0.56). Slowest decline.', 'medium',
        F('2D', 'corner', 'uniform', false, false, 'standard', 'shape-factor'),
        { nx: 21, ny: 21, cellDx: 20, cellDy: 20, producerI: 0, producerJ: 0, delta_t_days: 2.0, steps: 50 }, 'dep-geo'],
    ['dep_2d_offcenter', '2D Off-Center', 'Asymmetric drainage, intermediate CA.', 'medium',
        F('2D', 'off-center', 'uniform', false, false, 'standard', 'shape-factor'),
        { nx: 21, ny: 21, cellDx: 20, cellDy: 20, producerI: 5, producerJ: 10, delta_t_days: 2.0, steps: 50 }, 'dep-geo'],

    // ── Shape Factor: 2D rectangles (off-center well in elongated shapes) ──
    ['dep_2d_rect2', '2D Rectangle 2:1', 'Elongated rectangle shape factor test.', 'medium',
        F('2D', 'center', 'uniform', false, false, 'standard', 'shape-factor'),
        { nx: 42, ny: 21, producerI: 21, producerJ: 10, delta_t_days: 2.0, steps: 50 }, 'dep-geo', 'dep-2d-rect', '2:1 Rect'],
    ['dep_2d_rect4', '2D Rectangle 4:1', 'Highly elongated → approaches 1D behavior.', 'medium',
        F('2D', 'center', 'uniform', false, false, 'standard', 'shape-factor'),
        { nx: 42, ny: 11, producerI: 21, producerJ: 5, delta_t_days: 2.0, steps: 50, cellDy: 40 }, 'dep-geo', 'dep-2d-rect', '4:1 Rect'],

    // ── Shape Factor: 3D ──
    ['dep_3d_center', '3D Center Producer', '3D shape factor with vertical communication.', 'slow',
        F('3D', 'center', 'uniform', false, false, 'standard', 'shape-factor'),
        { nx: 11, ny: 11, nz: 5, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-geo', 'dep-3d-shape', 'Standard'],
    ['dep_3d_tall', '3D Tall Reservoir', 'Thick reservoir → more drainage volume.', 'slow',
        F('3D', 'center', 'uniform', false, false, 'standard', 'shape-factor'),
        { nx: 11, ny: 11, nz: 10, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-geo', 'dep-3d-shape', 'Tall (nz=10)'],

    // ── Grid Refinement ──
    ['dep_conv_nx12', '12 Cells (Coarse)', 'Noticeable discretization error vs analytical.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'grid-refinement'),
        { nx: 12, cellDx: 40 }, 'dep-grid', 'dep-grid', '12 cells'],
    ['dep_conv_nx24', '24 Cells (Medium)', 'Medium spatial refinement.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'grid-refinement'),
        { nx: 24, cellDx: 20 }, 'dep-grid', 'dep-grid', '24 cells'],
    ['dep_conv_nx48', '48 Cells (Fine)', 'Standard baseline for 1D.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'grid-refinement'),
        {}, 'dep-grid', 'dep-grid', '48 cells'],
    ['dep_conv_nx96', '96 Cells (V. Fine)', 'Asymptotic numerical convergence target.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'grid-refinement'),
        { nx: 96, cellDx: 5 }, 'dep-grid', 'dep-grid', '96 cells'],

    // ── Time Refinement ──
    ['dep_dt_5d', 'dt = 5.0 days', 'Large timestep. Sub-stepping engages heavily.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'time-refinement'),
        { delta_t_days: 5.0, steps: 10 }, 'dep-dt', 'dep-dt', 'dt=5d'],
    ['dep_dt_1d', 'dt = 1.0 day', 'Medium timestep size.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'time-refinement'),
        { delta_t_days: 1.0 }, 'dep-dt', 'dep-dt', 'dt=1d'],
    ['dep_dt_025d', 'dt = 0.25 days', 'Fine timestep for excellent decline resolution.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'time-refinement'),
        { delta_t_days: 0.25, steps: 72 }, 'dep-dt', 'dep-dt', 'dt=0.25d'],

    // ── Heterogeneity ──
    ['dep_het_mild', 'Mild Random Perm', 'Slight stochastic variation in 2D.', 'medium',
        F('2D', 'center', 'random', false, false, 'standard', 'shape-factor'),
        { nx: 21, ny: 21, cellDx: 20, cellDy: 20, permMode: 'random', minPerm: 100, maxPerm: 300, useRandomSeed: true, randomSeed: 42, delta_t_days: 2.0, steps: 50, producerI: 10, producerJ: 10 }, 'dep-het', 'dep-2d-random', 'Mild (3:1)'],
    ['dep_het_strong', 'Strong Random Perm', 'Significant random heterogeneity in 2D.', 'medium',
        F('2D', 'center', 'random', false, false, 'standard', 'shape-factor'),
        { nx: 21, ny: 21, cellDx: 20, cellDy: 20, permMode: 'random', minPerm: 20, maxPerm: 500, useRandomSeed: true, randomSeed: 42, delta_t_days: 2.0, steps: 50, producerI: 10, producerJ: 10 }, 'dep-het', 'dep-2d-random', 'Strong (25:1)'],
    ['dep_het_layered', '5-Layer System', 'Vertical perm variations. Differential layer rates.', 'slow',
        F('3D', 'center', 'layered', false, false, 'standard', 'shape-factor'),
        { nx: 11, ny: 11, nz: 5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5], delta_t_days: 2.0, steps: 50, producerI: 5, producerJ: 5 }, 'dep-het', 'dep-3d-layered', 'Symmetric'],
    ['dep_het_contrast', '10:1 Layer Contrast', 'Strong vertical contrast. Significant cross-flow.', 'slow',
        F('3D', 'center', 'layered', false, false, 'standard', 'shape-factor'),
        { nx: 11, ny: 11, nz: 5, permMode: 'perLayer', layerPermsX: [20, 20, 200, 200, 20], layerPermsY: [20, 20, 200, 200, 20], layerPermsZ: [2, 2, 20, 20, 2], delta_t_days: 2.0, steps: 50, producerI: 5, producerJ: 5 }, 'dep-het', 'dep-3d-layered', '10:1 Contrast'],

    // ── Fluids (1D shape-factor with fluid variation) ──
    ['dep_heavy_oil', 'Heavy Oil (μₒ=10)', 'Low PI, much slower decline.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'heavy-oil', 'shape-factor'),
        { mu_o: 10.0, steps: 50 }, 'dep-fluid'],
    ['dep_light_oil', 'Light Oil (μₒ=0.3)', 'High PI, very rapid initial drop.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'light-oil', 'shape-factor'),
        { mu_o: 0.3 }, 'dep-fluid'],
    ['dep_high_comp', 'High Compressibility', 'More expansion support, slower decline.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'high-comp', 'shape-factor'),
        { c_o: 5e-5, steps: 50 }, 'dep-fluid'],
    ['dep_low_comp', 'Low Compressibility', 'Stiff fluid. Very small τ, rapid decay.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'low-comp', 'shape-factor'),
        { c_o: 1e-6 }, 'dep-fluid'],

    // ── Rel-Perm ──
    ['dep_rp_mobile_w', 'Mobile Water (Sw₀=0.3)', 'Co-production of water and oil.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'rel-perm'),
        { initialSaturation: 0.3 }, 'dep-rp', 'dep-rp', 'Sw₀=0.3'],
    ['dep_rp_low_kro', 'Low kro_max (0.6)', 'Reduced max oil mobility.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'rel-perm'),
        { k_ro_max: 0.6, steps: 50 }, 'dep-rp', 'dep-rp', 'kro=0.6'],
    ['dep_rp_narrow', 'Narrow Sat Window', 'High Swc and Sor. Shrinks mobile range.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'rel-perm'),
        { s_wc: 0.25, s_or: 0.25 }, 'dep-rp', 'dep-rp', 'Narrow'],

    // ── Gravity ──
    ['dep_grav_3d', '3D Gravity Drainage', 'Vertical segregation + pressure depletion.', 'slow',
        F('3D', 'center', 'uniform', true, false, 'standard', 'shape-factor'),
        { nx: 11, ny: 11, nz: 5, gravityEnabled: true, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-grav'],
    ['dep_grav_density', 'Density Contrast Drainage', 'Enhanced gravity segregation.', 'slow',
        F('3D', 'center', 'uniform', true, false, 'standard', 'shape-factor'),
        { nx: 11, ny: 11, nz: 5, gravityEnabled: true, rho_o: 600, rho_w: 1050, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-grav', 'dep-3d-grav', 'ρ contrast'],

    // ── Capillary ──
    ['dep_cap_mild', 'Mild Capillary', 'Slight Pc distribution effects.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, true, 'standard', 'shape-factor'),
        { capillaryEnabled: true, capillaryPEntry: 0.5, capillaryLambda: 3, initialSaturation: 0.3 }, 'dep-cap', 'dep-cap', 'Mild'],
    ['dep_cap_strong', 'Strong Capillary', 'Significant Pc retention alters mobilities.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, true, 'standard', 'shape-factor'),
        { capillaryEnabled: true, capillaryPEntry: 3.0, capillaryLambda: 2, initialSaturation: 0.3 }, 'dep-cap', 'dep-cap', 'Strong'],

    // ── Multi-Factor ──
    ['dep_grav_cap_3d', 'Gravity + Capillary 3D', 'Combined Pc-G equilibrium during depletion.', 'slow',
        F('3D', 'center', 'uniform', true, true, 'standard', 'shape-factor'),
        { nx: 11, ny: 11, nz: 5, producerI: 5, producerJ: 5, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5, initialSaturation: 0.3, delta_t_days: 2.0, steps: 50 }, 'dep-multi'],
    ['dep_het_grav_3d', 'Layered + Gravity 3D', 'Layered heterogeneity + gravity segregation.', 'slow',
        F('3D', 'center', 'layered', true, false, 'standard', 'shape-factor'),
        { nx: 11, ny: 11, nz: 5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5], gravityEnabled: true, producerI: 5, producerJ: 5, delta_t_days: 2.0, steps: 50 }, 'dep-multi'],
    ['dep_het_cap_2d', 'Random + Capillary 2D', 'Pc smearing across heterogeneous field.', 'medium',
        F('2D', 'center', 'random', false, true, 'standard', 'shape-factor'),
        { nx: 21, ny: 21, permMode: 'random', minPerm: 50, maxPerm: 300, useRandomSeed: true, randomSeed: 42, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5, initialSaturation: 0.3, delta_t_days: 2.0, steps: 50, producerI: 10, producerJ: 10 }, 'dep-multi'],
];

// ═══════════════════════════════════════════════════════════════
//  WATERFLOOD CASES (30)
// ═══════════════════════════════════════════════════════════════
const waterfloodCases: RawCase[] = [
    // ── Displacement baseline ──
    ['wf_mob_base', 'Base Case (M≈2)', 'Standard BL shock front. Mildly unfavorable.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'displacement'),
        {}, 'wf-mob'],

    // ── Mobility Ratio sweep ──
    ['wf_mob_piston', 'Piston-Like (M≈0.5)', 'Favorable mobility. Near 100% sweep.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'mobility-ratio'),
        { mu_w: 1.0, mu_o: 0.5 }, 'wf-mob', 'wf-mob', 'M≈0.5'],
    ['wf_mob_unit', 'Unit Mobility', 'Equal viscosities.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'mobility-ratio'),
        { mu_w: 0.5, mu_o: 0.5 }, 'wf-mob', 'wf-mob', 'M≈1'],
    ['wf_mob_m2', 'Mild Unfavorable (M≈2)', 'Standard Buckley-Leverett.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'mobility-ratio'),
        {}, 'wf-mob', 'wf-mob', 'M≈2'],
    ['wf_mob_adverse', 'Adverse (M≈5)', 'Unfavorable. Early breakthrough.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'mobility-ratio'),
        { mu_w: 0.3, mu_o: 3.0 }, 'wf-mob', 'wf-mob', 'M≈5'],
    ['wf_mob_viscous', 'Viscous (M≈17)', 'Highly unfavorable. Viscous fingering regime.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'mobility-ratio'),
        { mu_w: 0.3, mu_o: 5.0 }, 'wf-mob', 'wf-mob', 'M≈17'],
    ['wf_mob_heavy', 'Heavy Oil (M≈50)', 'Extreme mobility ratio. Very early breakthrough.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'heavy-oil', 'mobility-ratio'),
        { mu_w: 0.3, mu_o: 15.0 }, 'wf-mob', 'wf-mob-heavy', 'M≈50'],

    // ── Corey Curves sweep ──
    ['wf_rp_linear', 'Linear kr (n=1)', 'Straight-line relative permeability.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'corey-curves'),
        { n_w: 1, n_o: 1 }, 'wf-corey', 'wf-corey', 'n=1'],
    ['wf_rp_mild', 'Mild Corey (n=1.5)', 'Low curvature endpoints.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'corey-curves'),
        { n_w: 1.5, n_o: 1.5 }, 'wf-corey', 'wf-corey', 'n=1.5'],
    ['wf_rp_base', 'Standard Corey (n=2)', 'Typical sandstone exponent.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'corey-curves'),
        { n_w: 2, n_o: 2 }, 'wf-corey', 'wf-corey', 'n=2'],
    ['wf_rp_high', 'High Corey (n=3)', 'Pronounced S-shape in fw.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'corey-curves'),
        { n_w: 3, n_o: 3 }, 'wf-corey', 'wf-corey', 'n=3'],
    ['wf_rp_extreme', 'Extreme (n=4)', 'Very sharp BL shock front.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'corey-curves'),
        { n_w: 4, n_o: 4 }, 'wf-corey', 'wf-corey', 'n=4'],
    ['wf_rp_ww', 'Water-Wet', 'n_w < n_o asymmetric.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'corey-curves'),
        { n_w: 1.5, n_o: 3.0 }, 'wf-corey', 'wf-corey', 'WW'],
    ['wf_rp_ow', 'Oil-Wet', 'n_o < n_w asymmetric.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'corey-curves'),
        { n_w: 3.0, n_o: 1.5 }, 'wf-corey', 'wf-corey', 'OW'],

    // ── Saturation Limits ──
    ['wf_sat_low_swc', 'Low Swc (0.05)', 'Wide mobile oil range.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'saturation-limits'),
        { s_wc: 0.05, initialSaturation: 0.05 }, 'wf-sat', 'wf-sat', 'Swc=0.05'],
    ['wf_sat_high_swc', 'High Swc (0.25)', 'High connate water.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'saturation-limits'),
        { s_wc: 0.25, initialSaturation: 0.25 }, 'wf-sat', 'wf-sat', 'Swc=0.25'],
    ['wf_sat_low_sor', 'Low Sor (0.05)', 'Efficient microscopic sweep.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'saturation-limits'),
        { s_or: 0.05 }, 'wf-sat', 'wf-sat', 'Sor=0.05'],
    ['wf_sat_high_sor', 'High Sor (0.3)', 'Poor ultimate recovery.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'saturation-limits'),
        { s_or: 0.3 }, 'wf-sat', 'wf-sat', 'Sor=0.3'],
    ['wf_sat_narrow', 'Narrow Window', 'High Swc + Sor. Brief mobile window.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'saturation-limits'),
        { s_wc: 0.25, s_or: 0.25, initialSaturation: 0.25 }, 'wf-sat', 'wf-sat', 'Narrow'],

    // ── kr Endpoints ──
    ['wf_ep_low_krw', 'Low krw_max (0.3)', 'Restricted injectivity post-BT.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'kr-endpoints'),
        { k_rw_max: 0.3 }, 'wf-ep', 'wf-ep', 'krw=0.3'],
    ['wf_ep_low_kro', 'Low kro_max (0.6)', 'Reduced initial oil PI.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'kr-endpoints'),
        { k_ro_max: 0.6 }, 'wf-ep', 'wf-ep', 'kro=0.6'],
    ['wf_ep_both', 'Both Reduced', 'Combined impact.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'kr-endpoints'),
        { k_rw_max: 0.3, k_ro_max: 0.6 }, 'wf-ep', 'wf-ep', 'Both'],

    // ── Grid Refinement ──
    ['wf_conv_nx12', '12 Cells', 'Heavy numerical diffusion.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'grid-refinement'),
        { nx: 12, cellDx: 20, producerI: 11 }, 'wf-grid', 'wf-grid', '12 cells'],
    ['wf_conv_nx24', '24 Cells', 'Medium numerical diffusion.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'grid-refinement'),
        { nx: 24, cellDx: 10, producerI: 23 }, 'wf-grid', 'wf-grid', '24 cells'],
    ['wf_conv_nx48', '48 Cells', 'Fine baseline.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'grid-refinement'),
        {}, 'wf-grid', 'wf-grid', '48 cells'],
    ['wf_conv_nx96', '96 Cells', 'Very fine. Sharp BL shock.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'grid-refinement'),
        { nx: 96, cellDx: 2.5, producerI: 95 }, 'wf-grid', 'wf-grid', '96 cells'],

    // ── Capillary ──
    ['wf_cap_mild', 'Mild Capillary', 'Slight front smearing.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, true, 'standard', 'displacement'),
        { capillaryEnabled: true, capillaryPEntry: 0.5, capillaryLambda: 3 }, 'wf-cap', 'wf-cap', 'Mild'],
    ['wf_cap_mod', 'Moderate Capillary', 'Noticeable deviation from ideal BL.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, true, 'standard', 'displacement'),
        { capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2.5 }, 'wf-cap', 'wf-cap', 'Moderate'],
    ['wf_cap_strong', 'Strong Capillary', 'Major front dispersion. BL no longer applicable.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, true, 'standard', 'displacement'),
        { capillaryEnabled: true, capillaryPEntry: 3.0, capillaryLambda: 2 }, 'wf-cap', 'wf-cap', 'Strong'],
];

// ═══════════════════════════════════════════════════════════════
//  SIMULATION CASES (27)
// ═══════════════════════════════════════════════════════════════
const simulationCases: RawCase[] = [
    // ── Sweep ──
    ['sim_2d_uniform', '2D Uniform Sweep', 'Baseline corner-to-corner areal sweep.', 'medium',
        F('2D', 'corner', 'uniform', false, false, 'standard', 'sweep'),
        {}, 'sim-2d'],
    ['sim_2d_random', '2D Random Perm', 'Sweep distorted by stochastic perm.', 'medium',
        F('2D', 'corner', 'random', false, false, 'standard', 'sweep'),
        { permMode: 'random', minPerm: 50, maxPerm: 300, useRandomSeed: true, randomSeed: 42 }, 'sim-2d'],
    ['sim_2d_cap_random', 'Random + Capillary', 'Capillary interaction with heterogeneous field.', 'medium',
        F('2D', 'corner', 'random', false, true, 'standard', 'sweep'),
        { permMode: 'random', minPerm: 50, maxPerm: 300, useRandomSeed: true, randomSeed: 42, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5 }, 'sim-2d'],

    // ── Thief Zone ──
    ['sim_2d_channel', 'High-Perm Channel', 'Thief zone causing premature breakthrough.', 'medium',
        F('2D', 'corner', 'layered', false, false, 'standard', 'thief-zone'),
        { permMode: 'perLayer', nz: 3, layerPermsX: [30, 300, 30], layerPermsY: [30, 300, 30], layerPermsZ: [3, 30, 3], cellDz: 3 }, 'sim-2d', 'sim-thief', 'Channel'],
    ['sim_2d_barrier', 'Low-Perm Barrier', 'Flow diversion around a barrier.', 'medium',
        F('2D', 'corner', 'layered', false, false, 'standard', 'thief-zone'),
        { permMode: 'perLayer', nz: 3, layerPermsX: [200, 5, 200], layerPermsY: [200, 5, 200], layerPermsZ: [20, 0.5, 20], cellDz: 3 }, 'sim-2d', 'sim-thief', 'Barrier'],

    // ── 3D Sweep ──
    ['sim_3d_uniform', '3D Uniform', 'Baseline 3D volumetric displacement.', 'slow',
        F('3D', 'corner', 'uniform', false, false, 'standard', 'sweep'),
        { nx: 15, ny: 10, nz: 5, producerI: 14, producerJ: 9, cellDz: 2 }, 'sim-3d'],
    ['sim_3d_layered', '3D Layered', 'Stratified flow with differential BT.', 'slow',
        F('3D', 'corner', 'layered', false, false, 'standard', 'sweep'),
        { nx: 15, ny: 10, nz: 5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5], producerI: 14, producerJ: 9, cellDz: 2 }, 'sim-3d'],
    ['sim_3d_random', '3D Random', 'Complex 3D stochastic sweep.', 'slow',
        F('3D', 'corner', 'random', false, false, 'standard', 'sweep'),
        { nx: 15, ny: 10, nz: 5, permMode: 'random', minPerm: 50, maxPerm: 200, useRandomSeed: true, randomSeed: 12345, producerI: 14, producerJ: 9, cellDz: 2 }, 'sim-3d'],
    ['sim_3d_contrast', '3D 10:1 Layers', 'High contrast multi-layer thief zones.', 'slow',
        F('3D', 'corner', 'layered', false, false, 'standard', 'thief-zone'),
        { nx: 15, ny: 10, nz: 10, permMode: 'perLayer', layerPermsX: [30, 40, 60, 90, 150, 400, 150, 90, 60, 40], layerPermsY: [30, 40, 60, 90, 150, 400, 150, 90, 60, 40], layerPermsZ: [3, 4, 6, 9, 15, 40, 15, 9, 6, 4], producerI: 14, producerJ: 9, cellDz: 1 }, 'sim-3d'],

    // ── Gravity Effects ──
    ['sim_grav_column', 'Gravity Column', 'Pure vertical gravity segregation.', 'fast',
        F('3D', 'center', 'uniform', true, false, 'standard', 'gravity-effects'),
        { nx: 1, ny: 1, nz: 20, cellDz: 2, injectorEnabled: false, gravityEnabled: true, initialSaturation: 0.5, steps: 50 }, 'sim-grav', 'sim-grav', 'Column'],
    ['sim_grav_override', 'Gravity Override 3D', 'Water underrun in thick reservoir.', 'slow',
        F('3D', 'corner', 'uniform', true, false, 'standard', 'gravity-effects'),
        { nx: 11, ny: 11, nz: 5, cellDz: 3, gravityEnabled: true, producerI: 10, producerJ: 10, steps: 40 }, 'sim-grav', 'sim-grav', 'Override'],
    ['sim_grav_het', 'Gravity + Layers', 'Gravity vs stratigraphic high-perm layers.', 'slow',
        F('3D', 'corner', 'layered', true, false, 'standard', 'gravity-effects'),
        { nx: 11, ny: 11, nz: 5, cellDz: 3, gravityEnabled: true, permMode: 'perLayer', layerPermsX: [50, 200, 50, 200, 50], layerPermsY: [50, 200, 50, 200, 50], layerPermsZ: [5, 20, 5, 20, 5], producerI: 10, producerJ: 10, steps: 40 }, 'sim-grav', 'sim-grav', 'Layered'],
    ['sim_grav_density', 'Strong Density Contrast', 'Exaggerated densities driving segregation.', 'slow',
        F('3D', 'corner', 'uniform', true, false, 'standard', 'gravity-effects'),
        { nx: 11, ny: 11, nz: 5, cellDz: 3, gravityEnabled: true, rho_o: 600, rho_w: 1050, producerI: 10, producerJ: 10, steps: 40 }, 'sim-grav', 'sim-grav', 'Density'],

    // ── Pc-G Balance ──
    ['sim_pcg_column', 'Pc-G Column', 'Capillary transition zone balancing gravity.', 'fast',
        F('3D', 'center', 'uniform', true, true, 'standard', 'pcg-balance'),
        { nx: 1, ny: 1, nz: 10, cellDz: 3, injectorEnabled: false, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2.5, initialSaturation: 0.5, steps: 50 }, 'sim-pcg', 'sim-pcg', 'Column'],
    ['sim_pcg_transition', 'Transition Zone', 'Dynamic flow with natural transition zone.', 'slow',
        F('3D', 'center', 'uniform', true, true, 'standard', 'pcg-balance'),
        { nx: 5, ny: 5, nz: 10, cellDz: 3, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2.5, initialSaturation: 0.4, producerI: 4, producerJ: 4, steps: 40 }, 'sim-pcg', 'sim-pcg', 'Transition'],
    ['sim_pcg_het', 'Pc-G + Layers', 'Layered capillary-gravity trapping.', 'slow',
        F('3D', 'center', 'layered', true, true, 'standard', 'pcg-balance'),
        { nx: 5, ny: 5, nz: 10, cellDz: 3, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.5, capillaryLambda: 2.5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50, 50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50, 50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5, 5, 10, 20, 10, 5], initialSaturation: 0.4, producerI: 2, producerJ: 2, steps: 40 }, 'sim-pcg', 'sim-pcg', 'Layered'],

    // ── Well Control ──
    ['sim_well_bhp', 'BHP-Driven', 'Constant bottom hole pressure.', 'medium',
        F('2D', 'corner', 'uniform', false, false, 'standard', 'well-control'),
        {}, 'sim-well', 'sim-well', 'BHP'],
    ['sim_well_rate', 'Rate-Driven', 'Volumetric rate targets.', 'medium',
        F('2D', 'corner', 'uniform', false, false, 'standard', 'well-control'),
        { injectorControlMode: 'rate', producerControlMode: 'rate', targetInjectorRate: 200, targetProducerRate: 200 }, 'sim-well', 'sim-well', 'Rate'],

    // ── Realistic ──
    ['sim_real_sand', 'North Sea Sandstone', 'Typical North Sea clastic reservoir analogue.', 'slow',
        F('3D', 'corner', 'layered', true, true, 'standard', 'realistic'),
        { nx: 15, ny: 10, nz: 5, permMode: 'perLayer', layerPermsX: [80, 120, 200, 150, 60], layerPermsY: [80, 120, 200, 150, 60], layerPermsZ: [8, 12, 20, 15, 6], gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 0.8, capillaryLambda: 2.5, cellDz: 3, producerI: 14, producerJ: 9, steps: 40 }, 'sim-real', 'sim-real', 'Sandstone'],
    ['sim_real_carb', 'Carbonate Primary', 'Tight carbonate with high Pc and depletion.', 'slow',
        F('3D', 'center', 'layered', true, true, 'standard', 'realistic'),
        { nx: 11, ny: 11, nz: 5, injectorEnabled: false, permMode: 'perLayer', layerPermsX: [10, 30, 50, 30, 10], layerPermsY: [10, 30, 50, 30, 10], layerPermsZ: [1, 3, 5, 3, 1], gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 3.0, capillaryLambda: 1.5, mu_o: 5, cellDz: 3, producerI: 5, producerJ: 5, producerControlMode: 'pressure', producerBhp: 80, steps: 50 }, 'sim-real', 'sim-real', 'Carbonate'],
    ['sim_real_heavy', 'Heavy Oil Cold Flow', 'Viscous-dominated secondary recovery.', 'slow',
        F('3D', 'corner', 'uniform', true, false, 'heavy-oil', 'realistic'),
        { mu_o: 50, gravityEnabled: true, nz: 5, cellDz: 3, producerI: 10, producerJ: 10, steps: 50 }, 'sim-real', 'sim-real', 'Heavy Oil'],

    // ── Stress Test ──
    ['sim_stress_highq', 'High Injection Rate', 'CFL sub-stepping heavily taxed.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'stress-test'),
        { nx: 48, ny: 1, nz: 1, cellDx: 5, injectorControlMode: 'rate', producerControlMode: 'rate', targetInjectorRate: 2000, targetProducerRate: 2000, injectorI: 0, producerI: 47, steps: 30 }, 'sim-stress', 'sim-stress', 'High Rate'],
    ['sim_stress_lowk', 'Tight Rock (k=1 mD)', 'Solver stability at very low k.', 'medium',
        F('2D', 'corner', 'uniform', false, false, 'standard', 'stress-test'),
        { uniformPermX: 1, uniformPermY: 1, uniformPermZ: 0.1, steps: 50 }, 'sim-stress', 'sim-stress', 'Tight'],
    ['sim_stress_large', 'Large 3D Grid', '4000 cells. WebAssembly perf test.', 'slow',
        F('3D', 'corner', 'random', false, false, 'standard', 'stress-test'),
        { nx: 20, ny: 20, nz: 10, cellDz: 2, permMode: 'random', minPerm: 50, maxPerm: 200, useRandomSeed: true, randomSeed: 77, producerI: 19, producerJ: 19, steps: 20 }, 'sim-stress', 'sim-stress', 'Large Grid'],

    // ── Multi-Factor ──
    ['sim_3d_all', '3D Full Physics', 'Gravity + Capillary + Heterogeneity.', 'slow',
        F('3D', 'corner', 'layered', true, true, 'standard', 'sweep'),
        { nx: 15, ny: 10, nz: 5, permMode: 'perLayer', layerPermsX: [50, 100, 200, 100, 50], layerPermsY: [50, 100, 200, 100, 50], layerPermsZ: [5, 10, 20, 10, 5], gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5, producerI: 14, producerJ: 9, cellDz: 2, steps: 30 }, 'sim-multi'],
    ['sim_heavy_grav', 'Heavy Oil + Gravity 3D', 'Sluggish heavy oil gravity segregation.', 'slow',
        F('3D', 'corner', 'uniform', true, false, 'heavy-oil', 'gravity-effects'),
        { nx: 11, ny: 11, nz: 5, mu_o: 10, gravityEnabled: true, cellDz: 3, producerI: 10, producerJ: 10, steps: 40 }, 'sim-multi'],
    ['sim_2d_cap_grav', '2D Capillary + Gravity', 'Cross-sectional Pc-G dynamics.', 'medium',
        F('2D', 'corner', 'uniform', true, true, 'standard', 'pcg-balance'),
        { ny: 1, nz: 21, cellDz: 3, cellDy: 20, gravityEnabled: true, capillaryEnabled: true, capillaryPEntry: 1.0, capillaryLambda: 2.5, producerI: 20, producerJ: 0, injectorI: 0, injectorJ: 0, steps: 30 }, 'sim-multi'],
];

// ═══════════════════════════════════════════════════════════════
//  BENCHMARK CASES (2)
// ═══════════════════════════════════════════════════════════════
const benchmarkCases: RawCase[] = [
    ['wf_pub_a', 'SPE BL Case A', 'Standardized BL verification (k=2000mD, L=960m, q=350).', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'spe-bl'),
        { nx: 96, cellDx: 10, cellDz: 1, uniformPermX: 2000, uniformPermY: 2000, uniformPermZ: 2000, targetInjectorRate: 350, targetProducerRate: 350, producerI: 95, analyticalSolutionMode: 'waterflood' }, 'wf-pub', 'bench-spe', 'Case A'],
    ['wf_pub_b', 'SPE BL Case B', 'Altered viscosities and Corey parameters.', 'fast',
        F('1D', 'end-to-end', 'uniform', false, false, 'standard', 'spe-bl'),
        { nx: 96, cellDx: 10, cellDz: 1, initialSaturation: 0.15, mu_w: 0.6, mu_o: 1.4, s_wc: 0.15, s_or: 0.15, n_w: 2.2, uniformPermX: 2000, uniformPermY: 2000, uniformPermZ: 2000, targetInjectorRate: 350, targetProducerRate: 350, producerI: 95, analyticalSolutionMode: 'waterflood' }, 'wf-pub', 'bench-spe', 'Case B'],
];

// ═══════════════════════════════════════════════════════════════
//  ASSEMBLED CATALOG
// ═══════════════════════════════════════════════════════════════
export const caseCatalog: CaseEntry[] = [
    ...buildCases('depletion', depletionCases),
    ...buildCases('waterflood', waterfloodCases),
    ...buildCases('simulation', simulationCases),
    ...buildCases('benchmark', benchmarkCases),
];

export function findCaseByKey(key: string): CaseEntry | null {
    return caseCatalog.find(c => c.key === key) ?? null;
}

/** Find all cases matching a set of toggle values */
export function findMatchingCases(toggles: Partial<CaseFacets>): CaseEntry[] {
    return caseCatalog.filter(c => {
        for (const [k, v] of Object.entries(toggles)) {
            if (v !== undefined && (c.facets as any)[k] !== v) return false;
        }
        return true;
    });
}
