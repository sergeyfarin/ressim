import { waterfloodLivePanels } from '../chartPanels/waterfloodLivePanels';
import type { Scenario } from '../scenarios';
import { waterfloodBLDef } from '../analyticalAdapters';

/**
 * Gravity override in a vertical cross-section, measured against the
 * Buckley-Leverett solution that ignores it.
 *
 * Well completions are single-layer, and now by design rather than by
 * necessity. The engine carries each completion's BHP from the well's datum
 * down the wellbore (`Well::head_offset_bar`), so a fully perforated well under
 * gravity no longer allocates flow by depth as an artefact. What single-layer
 * completions buy this case is the `completion_strategy` dimension: with one
 * perforation per well, moving the producer from the base of the section to the
 * top turns gravity from a loss into a gain, which is the point that dimension
 * makes. The recorded numbers throughout this file were measured with these
 * completions and are unaffected by the datum correction, which is identically
 * zero for a well with a single completion.
 */
export const wf_gravity: Scenario = {
    key: 'wf_gravity',
    label: 'Gravity Override (Dietz Tongue)',
    catalog: {
        // Sweep, not 1D displacement: the flow path here is two-dimensional and
        // what the case measures is how much of the section the flood contacts.
        // Buckley-Leverett is its reference the way it is inside any sweep
        // correlation — the displacement-efficiency term, with the shortfall
        // against it being the vertical sweep the gravity tongue costs.
        group: 'sweep-efficiency',
        role: 'simulation',
        caseMode: 'wf',
        parameterSummary: '2D vertical section · vertical sweep lost to a gravity tongue · fixed gravity-free BL reference',
    },
    description: 'A 300 m long, 40 m thick vertical cross-section flooded from a single perforation at the base of the injector. Buckley-Leverett has no gravity term, so its curve is the same in every variant here; what moves is the simulation. Turn gravity off and the section behaves as a one-dimensional tube — breakthrough at 0.500 PVI against BL\'s 0.586, recovery at one pore volume injected 0.699 against BL\'s 0.715, the remaining gap being numerical dispersion and the convergent flow into the single perforations. Turn gravity on and dense water slumps to the base and runs ahead of the front as a tongue, leaving oil above it: at the base rate breakthrough falls to 0.253 PVI and recovery to 0.585, and in the gravity-dominated rung BL over-predicts recovery at one PVI by 88 %. Read in sweep terms, the analytical curve is the displacement efficiency E_D of a perfectly contacted flood and the shortfall below it is the vertical sweep the tongue costs — the same quantity `sweep_vertical` measures for permeability contrast, here driven by density instead of geology. The controlling group is the ratio of buoyant head across the section to viscous pressure drop along it, N_g = Δρ·g·H·k / (μ_o·u_t·L), which the rate ladder sweeps from 0.14 to 2.23. Nothing about this is numerical error, and the case does not ask to be taken on trust: the base run is also shipped as an OPM Flow deck, and the two simulators agree to 0.4 % on recovery at one pore volume injected while both sit 18 % below BL.',
    analyticalMethodSummary: 'Buckley-Leverett with Welge shock construction, shown as the gravity-free reference. The fractional-flow function used here carries only viscous terms, so the analytical curve is identical across every rate, density contrast and completion in this scenario — the departure of the simulation from it is the measured quantity.',
    analyticalMethodReference: 'Buckley and Leverett (1942); Welge (1952); Dietz (1953), Proc. Koninklijke Nederlandse Akademie van Wetenschappen B56; Dake (1978), Fundamentals of Reservoir Engineering, ch. 10; Shook, Li and Lake (1992), In Situ 16(4) — scaling groups.',
    chartLayoutKey: 'waterflood',
    defaultSensitivityDimensionKey: 'gravity_number',
    capabilities: {
        analyticalMethod: 'buckley-leverett',
        hasInjector: true,
        default3DScalar: 'saturation_water',
        // Opens along the flood direction, where the Buckley-Leverett saturation
        // profile can be drawn beside the simulated one — the departure is the
        // whole case. With more than one layer the profile averages the column
        // by default, which is the quantity BL actually predicts. The tongue's
        // vertical structure is one dropdown away: switch the axis to K, or pick
        // a single layer to see how far the water has run along the base.
        spatialProfile: { defaultAxis: 'i' },
        requiresThreePhaseMode: false,
    },
    solverPolicy: {
        defaultSolver: 'impes',
        rationale: 'IMPES is the validated default for this two-phase oil/water section; the case varies physics, not formulation.',
    },
    params: {
        // Fluid — 200 kg/m³ density contrast is the base gravity driver
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
        // Rock / relative permeability — same curves as wf_bl1d, so the BL
        // reference is literally the same solution that case is judged against.
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
        // Grid: 30 × 1 × 20 vertical section, 300 m long × 20 m wide × 40 m thick
        nx: 30,
        ny: 1,
        nz: 20,
        cellDx: 10,
        cellDy: 20,
        cellDz: 2,
        // Isotropic by default: with k_z = k_x the only thing standing between
        // the simulation and the BL reference is gravity itself.
        permMode: 'uniform',
        uniformPermX: 5000,
        uniformPermY: 5000,
        uniformPermZ: 5000,
        // Initial conditions
        initialPressure: 300,
        initialSaturation: 0.1,
        // Wells — one perforation each, both at the base of the section.
        injectorEnabled: true,
        injectorControlMode: 'rate',
        producerControlMode: 'pressure',
        injectorBhp: 600,
        producerBhp: 200,
        // 160 reservoir m³/day over the 800 m² cross-section is u_t = 0.2 m/day,
        // which puts the base case at N_g ≈ 0.56 — gravity and viscous forces
        // of the same order, the regime where the tongue is unmistakable but
        // the flood still finishes.
        targetInjectorRate: 160,
        targetProducerRate: 0,
        injectorI: 0,
        injectorJ: 0,
        injectorKLayers: [19],
        producerI: 29,
        producerJ: 0,
        producerKLayers: [19],
        well_radius: 0.1,
        well_skin: 0,
        // Numerics: 48,000 m³ pore volume, so 420 days is 1.4 PVI at base rate.
        fimEnabled: false,
        delta_t_days: 2,
        steps: 210,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: true,
    },
    analyticalDef: waterfloodBLDef,
    liveChartPanels: waterfloodLivePanels,
    // The bundled deck is this scenario's base case, cell for cell: same grid,
    // rock curves, densities, single-layer completions, 160 m³/day injection and
    // 420-day schedule. It is the only reference here that contains gravity, so
    // it is what says the tongue is physics rather than an IMPES artefact.
    // The artifact carries its own time -> PVI mapping (FVIT over the deck's
    // pore volume), so the curves land correctly on the scenario's PVI axis.
    referenceSources: [{ kind: 'opm-flow', artifactKeys: ['wf_gravity'] }],
    sensitivities: [
        {
            key: 'gravity_number',
            label: 'Gravity Number  N_g',
            description: 'One control and three rates. The control runs the identical model with gravity switched off and lands on the BL curve to within the grid resolution — this is the rung that says the rest of the departure is physics, not error. Each slower rate raises N_g = Δρ·g·H·k / (μ_o·u_t·L), the buoyant head across the section divided by the viscous drop along it, and the water tongue runs further ahead of the front. Measured recovery at one pore volume injected: 0.699 (gravity off), 0.689 (N_g = 0.14), 0.585 (N_g = 0.56), 0.380 (N_g = 2.23), against BL\'s 0.715. Read on the PVI axis, because each rung takes a different number of days to inject the same pore volume.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'ng_off',
                    label: 'Gravity off  (control)',
                    description: 'The same section, same rate, gravity disabled. Breakthrough 0.500 PVI against BL\'s 0.586 and recovery 0.699 against 0.715 — the residual gap is numerical dispersion plus the convergent flow into single perforations, and it is what "agreement" looks like for this grid.',
                    paramPatch: { gravityEnabled: false },
                    affectsAnalytical: false,
                },
                {
                    key: 'ng_viscous',
                    label: 'N_g ≈ 0.14  (640 m³/d, viscous-dominated)',
                    description: 'Four times the base rate. Viscous forces dominate buoyancy and the flood is nearly one-dimensional again: recovery 0.689, within 1.5 % of the gravity-off control. Gravity has not gone away — it has been outrun.',
                    paramPatch: { targetInjectorRate: 640, delta_t_days: 0.5, steps: 220 },
                    affectsAnalytical: false,
                },
                {
                    key: 'ng_base',
                    label: 'N_g ≈ 0.56  (160 m³/d, base)',
                    description: 'Buoyancy and viscous drive of the same order. The tongue is clearly formed in the 3D view, breakthrough halves to 0.253 PVI, and recovery at one PVI falls to 0.585 — 18 % below the analytical solution the flood would follow if the two fluids weighed the same.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'ng_gravity',
                    label: 'N_g ≈ 2.23  (40 m³/d, gravity-dominated)',
                    description: 'A quarter of the base rate, and the displacement has become a thin water layer sliding along the base of the section. Breakthrough at 0.167 PVI, recovery 0.380 — BL over-predicts by 88 %. Slower is not always better: the same pore volume injected recovers far less oil when it is injected slowly enough for gravity to segregate it.',
                    paramPatch: { targetInjectorRate: 40, delta_t_days: 8, steps: 213 },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'density_contrast',
            label: 'Density Contrast  Δρ',
            description: 'The same flood at the base rate with the oil density moved from 1000 to 600 kg/m³. At Δρ = 0 the buoyancy term vanishes and the simulation returns to the gravity-off control (0.704 against 0.699) even though gravity is still enabled — a useful check that the effect is the density difference and not the presence of a gravity term. Recovery at one PVI then falls monotonically: 0.704, 0.650, 0.585, 0.491. A 200 kg/m³ error in the fluid model is worth more here than most rock-curve uncertainties.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'drho_zero',
                    label: 'Δρ = 0  (ρ_o = 1000, control)',
                    description: 'Equal densities. Gravity is enabled but has nothing to act on, and the flood tracks the gravity-off control.',
                    paramPatch: { rho_o: 1000 },
                    affectsAnalytical: false,
                },
                {
                    key: 'drho_light',
                    label: 'Δρ = 100  (ρ_o = 900)',
                    description: 'A heavy oil against fresh water. The tongue is present but shallow; recovery 0.650.',
                    paramPatch: { rho_o: 900 },
                    affectsAnalytical: false,
                },
                {
                    key: 'drho_base',
                    label: 'Δρ = 200  (ρ_o = 800, base)',
                    description: 'A typical light-oil / brine contrast. Recovery 0.585.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'drho_strong',
                    label: 'Δρ = 400  (ρ_o = 600)',
                    description: 'A very light oil or a dense brine. Recovery 0.491 — the flood loses nearly a third of the recovery BL promises, from the fluid densities alone.',
                    paramPatch: { rho_o: 600 },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'vertical_communication',
            label: 'Vertical Communication × Gravity',
            description: 'Gravity can only segregate the fluids if the rock lets them move vertically. Two vertical permeabilities, each run with gravity on and off, so the gravity penalty can be read as a pair rather than guessed from an absolute number. At k_z/k_x = 1 gravity costs 0.114 of recovery (0.699 → 0.585); at k_z/k_x = 0.01 it costs only 0.033 (0.426 → 0.393). The second lesson is in the other direction: the low-k_z pair sits far below BL whether or not gravity is on, because bottom perforations in a poorly communicating section simply never contact the upper layers. Not every departure from Buckley-Leverett is gravity.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'kv_iso_gravity',
                    label: 'k_z/k_x = 1, gravity on  (base)',
                    description: 'Full vertical communication and buoyancy free to act. Recovery 0.585.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'kv_iso_nogravity',
                    label: 'k_z/k_x = 1, gravity off',
                    description: 'Same rock, no buoyancy. Recovery 0.699 — the 0.114 difference is the entire cost of gravity in a well-communicating section.',
                    paramPatch: { gravityEnabled: false },
                    affectsAnalytical: false,
                },
                {
                    key: 'kv_tight_gravity',
                    label: 'k_z/k_x = 0.01, gravity on',
                    description: 'Vertical flow is suppressed and the tongue cannot develop. Recovery 0.393 — low, but barely lower than the same rock without gravity.',
                    paramPatch: { uniformPermZ: 50 },
                    affectsAnalytical: false,
                },
                {
                    key: 'kv_tight_nogravity',
                    label: 'k_z/k_x = 0.01, gravity off',
                    description: 'Recovery 0.426. Compare this pair with the isotropic one: the gravity penalty has fallen from 0.114 to 0.033, while the total shortfall against BL has grown. The two mechanisms are not additive and cannot be separated from a single curve.',
                    paramPatch: { uniformPermZ: 50, gravityEnabled: false },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'completion_strategy',
            label: 'Producer Completion × Gravity',
            description: 'Where the producer is perforated decides whether gravity is a loss or a gain, and BL cannot express the question at all. Perforating at the base of the section — where the water tongue arrives — costs 0.114 of recovery relative to no gravity. Perforating the same producer at the top, so that oil is drawn off above the advancing water, turns the same buoyancy into a benefit: 0.737 with gravity against 0.703 without, and above the gravity-free BL-like control. The injector stays at the base throughout; only the producer moves.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'comp_bottom_gravity',
                    label: 'Producer at base, gravity on  (base)',
                    description: 'The tongue runs straight into the perforation. Breakthrough 0.253 PVI, recovery 0.585.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'comp_bottom_nogravity',
                    label: 'Producer at base, gravity off',
                    description: 'The reference pair for the completion at the base: breakthrough 0.500 PVI, recovery 0.699.',
                    paramPatch: { gravityEnabled: false },
                    affectsAnalytical: false,
                },
                {
                    key: 'comp_top_gravity',
                    label: 'Producer at top, gravity on',
                    description: 'Water underruns beneath the producing interval instead of into it. Breakthrough 0.467 PVI and recovery 0.737 — the highest in this scenario, and higher than any gravity-free run. Gravity-assisted drainage, arrived at by moving one perforation.',
                    paramPatch: { producerKLayers: [0] },
                    affectsAnalytical: false,
                },
                {
                    key: 'comp_top_nogravity',
                    label: 'Producer at top, gravity off',
                    description: 'Recovery 0.703. Without buoyancy the completion depth hardly matters (0.699 at the base); with it, the same choice is worth 0.152 of recovery. The decision only exists in the physics BL leaves out.',
                    paramPatch: { producerKLayers: [0], gravityEnabled: false },
                    affectsAnalytical: false,
                },
            ],
        },
        {
            key: 'opm_cross_check',
            label: 'OPM Flow Cross-Check',
            description: 'The base case is also shipped as an OPM Flow deck — the same grid, rock curves, densities, completions, injection rate and 420-day schedule — and the two simulators are run independently of each other. OPM breaks through at 0.227 PVI and recovers 0.583 of oil in place at one pore volume injected; ResSim gives 0.253 and 0.585 — 0.4 % apart on recovery and 12 % apart on breakthrough, both 18 % below Buckley-Leverett, whose own breakthrough is 158 % late against OPM. That is the argument this scenario rests on: an independent industrial simulator reproduces the departure, so the departure is the physics BL omits and not a defect in this engine. The two rungs vary only the ResSim report step. The OPM curves are placed on the pore-volume axis through the deck run\'s own injected volume, so they can be read against the simulation on either axis.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'opm_step_2d',
                    label: 'ResSim, 2-day report step  (deck schedule)',
                    description: 'The scenario default, matching the deck\'s own 210 × 2-day TSTEP. IMPES subdivides internally for stability, so the report step sets how often well controls are re-solved, not the transport step.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'opm_step_05d',
                    label: 'ResSim, 0.5-day report step',
                    description: 'The same case reported four times as often. Recovery moves from 0.5850 to 0.5865 — 0.3 %, an order of magnitude smaller than the gap to Buckley-Leverett, so the report step is not what separates this engine from OPM Flow.',
                    paramPatch: { delta_t_days: 0.5, steps: 840 },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
