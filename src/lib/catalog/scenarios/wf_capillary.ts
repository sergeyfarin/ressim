import { waterfloodLivePanels } from '../chartPanels/waterfloodLivePanels';
import type { Scenario } from '../scenarios';
import { waterfloodBLDef } from '../analyticalAdapters';

/**
 * Capillary pressure — the physics Buckley-Leverett leaves out.
 *
 * Every other scenario in the catalog runs with `capillaryEnabled: false`.
 * The engine has had a validated Brooks-Corey capillary model since early on
 * (`src/lib/ressim/src/capillary.rs`, differentiated through the FIM AD path)
 * and nothing in the case library ever switched it on. This scenario is that
 * missing exercise, and it is built around the one thing capillarity does that
 * the analytical reference cannot represent.
 *
 * Physics
 * -------
 * Buckley-Leverett is the zero-capillary limit of two-phase displacement: it
 * solves a hyperbolic conservation law whose only solution is a sharp shock.
 * Adding P_c(S_w) adds a saturation-gradient (diffusive) term to the flux,
 * turning the equation parabolic. The shock becomes a front of finite width,
 * set by the balance of capillary and viscous forces — for a Brooks-Corey
 * curve of entry pressure P_e over a system with pressure drop dP, the front
 * width scales roughly with P_e/dP. Water also imbibes ahead of where pure
 * viscous flow would carry it, so first water arrives *earlier* than the BL
 * shock while the water cut climbs more gradually afterwards.
 *
 * The teaching point is not "capillarity is important" — it is that a
 * capillary-smeared front and a numerically-smeared front look alike on a
 * water-cut curve. That confusion is the subject of the second dimension.
 *
 * Construction
 * ------------
 * A 1D horizontal slab deliberately kept close to `wf_bl1d` (same rock,
 * fluids, well controls and analytical overlay) so the two cases are directly
 * comparable — this scenario is the P_c > 0 branch of that one. Gravity stays
 * off: in a horizontal 1D slab it does nothing, and leaving it off keeps the
 * capillary term as the single physical difference from `wf_bl1d`.
 *
 * The engine's Brooks-Corey P_c is capped at 20x the entry pressure
 * (`CapillaryPressure::capillary_pressure`) to stop the curve's connate-water
 * singularity from overpowering gravity in tall columns. The entry pressures
 * used here (<= 8 bar against a 40 bar drawdown, so P_c stays below 160 bar)
 * stay far below any regime where that cap changes the answer.
 *
 * Analytical overlay: Buckley-Leverett, shared across the capillary ladder.
 * This is deliberate and it is the case's whole argument — BL has no P_c term,
 * so it *cannot* respond to these variants. Every capillary variant is marked
 * `affectsAnalytical: false` for exactly that reason: the fixed BL curve is the
 * zero-capillary reference the simulated fronts are measured against, and the
 * growing departure from it is the result.
 *
 * References: Leverett, M.C. (1941) "Capillary Behavior in Porous Solids",
 * Trans. AIME 142; Brooks, R.H. & Corey, A.T. (1964) "Hydraulic Properties of
 * Porous Media", Colorado State Univ. Hydrology Paper 3; Buckley & Leverett
 * (1942); Welge (1952); Lake, "Enhanced Oil Recovery" ch. 5 (capillary
 * end effects and the viscous/capillary force balance).
 *
 * NOT covered here: the gravity-capillary equilibrium *transition zone* — the
 * hydrostatic P_c(S_w) = drho.g.h profile and its Leverett J-function scaling.
 * That comparison is a saturation-versus-depth profile, and the chart stack is
 * time-series only (`SwProfileChart.svelte` is dormant and unwired). Tracked as
 * the open half of T7.4 in docs/CASE_LIBRARY_ROADMAP.md.
 */
export const wf_capillary: Scenario = {
    key: 'wf_capillary',
    label: 'Capillary Pressure vs. Buckley-Leverett',
    description: 'The same 1D waterflood as the Buckley-Leverett case, run with capillary pressure switched on. BL is the zero-capillary limit — it solves for a sharp shock and has no term that can respond to P_c at all, so its curve stays fixed while the simulated fronts move away from it. Stronger capillarity spreads the front: water imbibes ahead of where viscous flow alone would carry it, so first water arrives earlier, and the water cut then climbs more gradually. The second dimension is the one worth sitting with: a coarse grid with no capillarity produces a smeared front that looks very much like a fine grid with capillarity. One is physics and one is truncation error, and the water-cut curve alone does not tell you which you are looking at.',
    analyticalMethodSummary: 'Buckley-Leverett with Welge shock construction, shown as the zero-capillary reference. BL neglects the capillary term by construction, so the analytical curve is identical for every capillary variant — the departure of the simulated front from it is the measured quantity, not an error.',
    analyticalMethodReference: 'Buckley and Leverett (1942); Welge (1952); Leverett (1941), Trans. AIME 142; Brooks and Corey (1964), CSU Hydrology Paper 3.',
    chartLayoutKey: 'waterflood',
    defaultSensitivityDimensionKey: 'capillary_strength',
    capabilities: {
        analyticalMethod: 'buckley-leverett',
        showSweepPanel: false,
        hasInjector: true,
        default3DScalar: 'saturation_water',
        requiresThreePhaseMode: false,
    },
    params: {
        // Fluid properties — matched to wf_bl1d so the two cases are comparable
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
        // Rock / relative permeability — matched to wf_bl1d
        reservoirPorosity: 0.2,
        s_wc: 0.1,
        s_or: 0.1,
        n_w: 2,
        n_o: 2,
        k_rw_max: 1,
        k_ro_max: 1,
        // Capillary pressure — ON. This is the only scenario in the catalog
        // where it is, and the base case is a moderate entry pressure so the
        // effect is visible without dominating the displacement.
        capillaryEnabled: true,
        capillaryPEntry: 3.0,
        capillaryLambda: 2,
        // Grid: 96-cell 1D slab, 960 m total length (as wf_bl1d)
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
        // Initial conditions
        initialPressure: 300,
        initialSaturation: 0.1,
        // Wells: pressure-controlled injector + producer (as wf_bl1d).
        // The 40 bar drawdown sets the viscous force the capillary
        // entry pressure competes against. Measured 2026-07-24: at the 400 bar
        // drawdown wf_bl1d uses, even an 8 bar entry pressure moves the front
        // width by only ~3%, because the case is then viscous-dominated to an
        // unrealistic degree. 40 bar across 960 m is a representative
        // interwell drawdown and makes the capillary/viscous ratio physical.
        injectorEnabled: true,
        injectorControlMode: 'pressure',
        producerControlMode: 'pressure',
        injectorBhp: 320,
        producerBhp: 280,
        targetInjectorRate: 0,
        targetProducerRate: 0,
        injectorI: 0,
        injectorJ: 0,
        producerI: 95,
        producerJ: 0,
        well_radius: 0.1,
        well_skin: 0,
        // Numerics
        fimEnabled: false,
        delta_t_days: 0.25,
        steps: 2000,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        // Physics toggles — gravity off: in a horizontal 1D slab it does
        // nothing, and leaving it off keeps P_c the single difference from
        // wf_bl1d.
        gravityEnabled: false,
    },
    analyticalDef: waterfloodBLDef,
    liveChartPanels: waterfloodLivePanels,
    sensitivities: [
        {
            key: 'capillary_strength',
            label: 'Capillary Entry Pressure  P_e',
            description: 'A ladder of Brooks-Corey entry pressures against a fixed 40 bar viscous drawdown. P_e = 0 reproduces the Buckley-Leverett case exactly and should sit on the analytical curve; each step up spreads the front further. Note the two-sided signature that identifies capillary smearing rather than a shift in front speed: first water arrives *earlier* than the BL shock (imbibition runs ahead of the viscous front) while the late water cut approaches 100% more slowly. The analytical curve does not move — BL has no capillary term — so every departure you see is the simulation, not the reference.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'pc_off',
                    label: 'P_e = 0  (no capillarity)',
                    description: 'Capillary pressure disabled — the pure Buckley-Leverett limit. This is the control: it should track the analytical curve to within the grid resolution of the base case.',
                    paramPatch: { capillaryEnabled: false, capillaryPEntry: 0 },
                    affectsAnalytical: false,
                },
                {
                    key: 'pc_weak',
                    label: 'P_e = 1 bar  (weak)',
                    description: 'A weakly capillary rock — high permeability, large pores. The front spreads slightly; the water-cut curve is still close to BL.',
                    paramPatch: { capillaryEnabled: true, capillaryPEntry: 1.0 },
                    affectsAnalytical: false,
                },
                {
                    key: 'pc_base',
                    label: 'P_e = 3 bar  (moderate, base)',
                    description: 'The base case. Capillary forces are now a clearly visible fraction of the viscous drive and the departure from the analytical shock is unambiguous.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'pc_strong',
                    label: 'P_e = 8 bar  (strong)',
                    description: 'A tight, strongly water-wet rock. The front is broad enough that "breakthrough time" stops being a well-defined quantity — which is itself the point: the sharp-shock idealisation BL is built on has stopped applying to this rock.',
                    paramPatch: { capillaryEnabled: true, capillaryPEntry: 8.0 },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'capillary_vs_numerical',
            label: 'Physics or Truncation Error?',
            description: 'Four runs chosen so two of them nearly coincide for entirely different reasons. A coarse grid with no capillarity smears the front through numerical dispersion; a fine grid with capillarity smears it through physics. On a water-cut curve the two are hard to tell apart — but they respond differently to refinement, and that is the only way to separate them from output alone: refine the grid and watch which curve moves. The numerically-smeared one converges; the capillary one does not, because its front width is set by the rock and the pressure drop, not by the mesh.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'cvn_coarse_nopc',
                    label: '12 cells, no P_c  (numerical smearing)',
                    description: 'A deliberately coarse grid with capillarity off. The front is smeared purely by truncation error in the saturation transport.',
                    paramPatch: {
                        capillaryEnabled: false, capillaryPEntry: 0,
                        nx: 12, cellDx: 80, producerI: 11,
                    },
                    affectsAnalytical: false,
                },
                {
                    key: 'cvn_fine_nopc',
                    label: '96 cells, no P_c  (converged reference)',
                    description: 'The same physics on the base grid. This is what the coarse run is converging towards, and it is close to the analytical shock.',
                    paramPatch: { capillaryEnabled: false, capillaryPEntry: 0 },
                    affectsAnalytical: false,
                },
                {
                    key: 'cvn_fine_pc',
                    label: '96 cells, P_e = 8 bar  (physical smearing)',
                    description: 'A well-resolved grid with strong capillarity. The front is broad because the rock makes it broad — refining further will not sharpen it.',
                    paramPatch: { capillaryEnabled: true, capillaryPEntry: 8.0 },
                    affectsAnalytical: false,
                },
                {
                    key: 'cvn_finer_pc',
                    label: '192 cells, P_e = 8 bar  (refinement test)',
                    description: 'The same strong-capillarity case at double the resolution. If the smearing were numerical this curve would move towards the sharp shock; it does not move much, which is how you attribute the spreading to physics.',
                    paramPatch: {
                        capillaryEnabled: true, capillaryPEntry: 8.0,
                        nx: 192, cellDx: 5, producerI: 191,
                    },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
