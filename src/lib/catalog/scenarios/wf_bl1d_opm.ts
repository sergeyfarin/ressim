import type { Scenario } from '../scenarios';

/**
 * E7 pre-run demonstrator — "1D Waterflood, OPM Flow (precomputed)".
 *
 * The first `runMode: 'prerun-artifacts'` scenario: it ships entirely
 * precomputed. There is no live worker run — its single variant maps to the
 * bundled `wf_bl1d` OPM Flow artifact (real `flow 2026.04` output, already in
 * the repo), rendered as the chart's primary content. The 3D view is off and
 * the parameter panel is read-only (predefined scenarios don't expose param
 * editing anyway).
 *
 * This exists to prove the pre-run scenario class end to end against real
 * bundled data; it is expected to be superseded as the "first real" Tier-6
 * exhibit once SPE11 (published inter-simulator data) is curated.
 *
 * Params below document the same 1D BL waterflood deck as the live `wf_bl1d`
 * scenario (independent copy — no shared params object per the params
 * convention); they are inert here since nothing runs.
 */
export const wf_bl1d_opm: Scenario = {
    key: 'wf_bl1d_opm',
    label: '1D Waterflood — OPM Flow (precomputed)',
    description: 'Precomputed OPM Flow reference for the 1D Buckley-Leverett waterflood deck — oil/water rates, cumulatives, and average pressure from a real `flow 2026.04` run, shipped as a bundled artifact. No live simulation: this is a pre-run exhibit demonstrating the precomputed-artifact scenario class.',
    analyticalMethodSummary: 'Precomputed OPM Flow output — the bundled artifact is the exhibit content; no live analytical overlay is computed.',
    analyticalMethodReference: 'OPM Flow 2026.04 (Open Porous Media); deck per Buckley & Leverett (1942).',
    chartLayoutKey: 'waterflood',
    opmFlowReferenceArtifactKeys: ['wf_bl1d'],
    capabilities: {
        analyticalMethod: 'none',
        showSweepPanel: false,
        hasInjector: true,
        default3DScalar: null,
        requiresThreePhaseMode: false,
        runMode: 'prerun-artifacts',
    },
    params: {
        mu_w: 0.5,
        mu_o: 1.0,
        c_o: 1e-5,
        c_w: 3e-6,
        rock_compressibility: 1e-6,
        depth_reference: 0,
        volume_expansion_o: 1,
        volume_expansion_w: 1,
        rho_w: 1000,
        rho_o: 800,
        reservoirPorosity: 0.2,
        s_wc: 0.1,
        s_or: 0.1,
        n_w: 2,
        n_o: 2,
        k_rw_max: 1,
        k_ro_max: 1,
        capillaryEnabled: false,
        capillaryPEntry: 0,
        capillaryLambda: 2,
        nx: 96,
        ny: 1,
        nz: 1,
        cellDx: 10,
        cellDy: 10,
        cellDz: 1,
        permMode: 'uniform',
        uniformPermX: 2000,
        uniformPermY: 2000,
        uniformPermZ: 200,
        initialPressure: 300,
        initialSaturation: 0.1,
        injectorEnabled: true,
        injectorControlMode: 'pressure',
        producerControlMode: 'pressure',
        injectorBhp: 500,
        producerBhp: 100,
        targetInjectorRate: 0,
        targetProducerRate: 0,
        injectorI: 0,
        injectorJ: 0,
        producerI: 95,
        producerJ: 0,
        well_radius: 0.1,
        well_skin: 0,
        fimEnabled: false,
        delta_t_days: 0.25,
        steps: 200,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: false,
    },
    liveChartPanels: undefined,
    sensitivities: [],
};
