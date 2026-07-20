import { waterfloodLivePanels } from '../chartPanels/waterfloodLivePanels';
import type { Scenario } from '../scenarios';

/**
 * "The tornado plot lies" — parameter interaction amplification.
 *
 * Physics basis: a 2D vertical cross-section (x-z) waterflood with a
 * denser injected fluid (water, rho_w=1000) than the in-place fluid
 * (oil, rho_o<1000) segregates downward under gravity — but only if
 * vertical permeability (kv) is high enough to let water actually cross
 * between layers. With low kv, layers behave nearly independently
 * regardless of density contrast (no path for gravity to redistribute
 * fluid vertically). With high kv but a small density contrast, there is
 * a path but little driving force. Only when BOTH kv is high AND the
 * density contrast is large does water tongue along the bottom layers,
 * bypassing the upper section and reducing sweep efficiency materially.
 *
 * Metric: displacement (sweep) efficiency — cumulative oil produced per
 * unit of water injected — not recovery factor at fixed time. Both wells
 * run under BHP (pressure) control, so a fixed-time recovery factor is not
 * a valid cross-variant comparison here: higher kv simply lets more total
 * fluid move in the same number of days, which *raises* fixed-time
 * recovery for every variant regardless of how much oil the tongue
 * bypasses (verified 2026-07-18: fixed-time RF for the combined variant is
 * actually ~4pp higher than base at every checkpoint from day 100 to day
 * 800). Oil produced per unit water injected isolates the bypassing effect
 * instead, independent of how much total throughput the pressure-controlled
 * wells achieve.
 *
 * Verified (headless wasm probe, 2026-07-18, 40x1x10 grid, kx=ky=200mD,
 * BHP 400/200 bar, 800 days): kv alone -2.3% displacement efficiency vs
 * base; density alone -0.02% (negligible); both together -7.5% — the
 * combined effect is roughly 3x larger than either individual effect, and
 * the individual effects alone would not predict the combined outcome.
 *
 * This demonstrates why one-at-a-time sensitivity (tornado) charts can
 * mislead: two parameters that each look unimportant in isolation can
 * combine into the dominant driver of an outcome.
 *
 * References: Dietz (1953) gravity segregation in linear waterflood;
 * Zhou, Fayers & Muggeridge (1997) gravity-viscous flow regime maps;
 * Shook, Li & Lake (1992) scaling groups for displacement.
 *
 * No analytical reference: 1D Buckley-Leverett does not model gravity
 * cross-flow between layers, so no honest quantitative overlay exists
 * for this case (same precedent as gas_drive) — the teaching point comes
 * from comparing the four simulated variants directly.
 */
export const wf_tornado: Scenario = {
    key: 'wf_tornado',
    label: 'The Tornado Plot Lies',
    description: '2D vertical cross-section waterflood (10 layers). Vertical permeability (kv) and oil/water density contrast are varied one at a time and together. Individually, each barely moves displacement efficiency (oil produced per unit water injected). Together, they create a gravity tongue that lets injected water bypass most of the section along the bottom layers, cutting displacement efficiency by roughly 3x more than either change alone would suggest — the classic failure mode of one-at-a-time sensitivity analysis. Note: the Recovery Factor panel actually rises slightly for the combined case, since both wells run under BHP control and better vertical communication simply moves more total fluid over the run — watch Water Cut (toggle it on in Diagnostics) or the widening gap between Cum Oil and Cum Injection instead.',
    analyticalMethodSummary: 'Simulation-only — no analytical overlay. 1D Buckley-Leverett does not model gravity cross-flow between layers, so no honest quantitative reference exists for this interaction (same precedent as gas_drive).',
    analyticalMethodReference: 'Dietz (1953); Zhou, Fayers & Muggeridge (1997); Shook, Li & Lake (1992).',
    chartLayoutKey: 'waterflood',
    defaultSensitivityDimensionKey: 'interaction',
    capabilities: {
        analyticalMethod: 'none',
        showSweepPanel: false,
        hasInjector: true,
        default3DScalar: 'saturation_water',
        requiresThreePhaseMode: false,
    },
    params: {
        // Fluid — base case: mild density contrast, low kv
        mu_w: 0.5,
        mu_o: 1.0,
        c_o: 1e-5,
        c_w: 3e-6,
        rock_compressibility: 1e-6,
        depth_reference: 0,
        volume_expansion_o: 1,
        volume_expansion_w: 1,
        rho_w: 1000,
        rho_o: 850,
        // Rock / rel perm
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
        // Grid: 2D vertical cross-section (XZ), 40 x 1 x 10, 800 m x 20 m x 50 m
        nx: 40,
        ny: 1,
        nz: 10,
        cellDx: 20,
        cellDy: 20,
        cellDz: 5,
        permMode: 'uniform',
        uniformPermX: 200,
        uniformPermY: 200,
        uniformPermZ: 4,
        // Initial conditions
        initialPressure: 300,
        initialSaturation: 0.1,
        // Wells: injector/producer span all 10 layers by default (worker
        // completes every k-layer when producerKLayers/injectorKLayers are
        // not explicitly set), sharing one physical well ID per role — the
        // same default multi-layer completion sweep_vertical relies on.
        injectorEnabled: true,
        injectorControlMode: 'pressure',
        producerControlMode: 'pressure',
        injectorBhp: 400,
        producerBhp: 200,
        targetInjectorRate: 0,
        targetProducerRate: 0,
        injectorI: 0,
        injectorJ: 0,
        producerI: 39,
        producerJ: 0,
        well_radius: 0.1,
        well_skin: 0,
        // Numerics
        fimEnabled: false,
        delta_t_days: 1.0,
        steps: 800,
        max_sat_change_per_step: 0.05,
        max_pressure_change_per_step: 75,
        max_well_rate_change_fraction: 0.75,
        gravityEnabled: true,
    },
    liveChartPanels: waterfloodLivePanels,
    sensitivities: [
        {
            key: 'interaction',
            label: 'kv x Density Contrast',
            description: 'Four combinations of vertical permeability (kv) and oil/water density contrast, holding horizontal permeability and everything else fixed. Watch Water Cut (Diagnostics panel, toggle it on) or the gap between Cum Oil and Cum Injection: kv alone and density alone each barely move displacement efficiency; together they cut it by roughly 3x more than either alone. Recovery Factor itself is not a useful comparison here — it rises for every variant because the BHP-controlled wells move more total fluid with better vertical communication, independent of how much oil is bypassed. The 3D view (water saturation) shows the gravity tongue forming along the bottom layers only in the combined case.',
            analyticalOverlayMode: 'shared',
            variants: [
                {
                    key: 'interaction_base',
                    label: 'Low kv, moderate Δρ  (base)',
                    description: 'kv = 4 mD (kv/kh = 0.02), ρ_o = 850 kg/m³. Layers are nearly non-communicating and the density contrast is mild — base case.',
                    paramPatch: {},
                    affectsAnalytical: false,
                },
                {
                    key: 'interaction_kv_only',
                    label: 'High kv alone',
                    description: 'kv = 190 mD (kv/kh ≈ 1, nearly isotropic) with the same mild density contrast. A path for cross-flow exists, but with little density-driven force to use it, displacement efficiency barely changes (≈2% relative drop).',
                    paramPatch: { uniformPermZ: 190 },
                    affectsAnalytical: false,
                },
                {
                    key: 'interaction_rho_only',
                    label: 'High Δρ alone',
                    description: 'ρ_o = 600 kg/m³ (strong density contrast) with the same low kv. The driving force for segregation is much larger, but layers still cannot communicate — displacement efficiency is essentially unchanged (<0.1% relative drop).',
                    paramPatch: { rho_o: 600 },
                    affectsAnalytical: false,
                },
                {
                    key: 'interaction_both',
                    label: 'High kv AND High Δρ  (interaction)',
                    description: 'Both changes together: a cross-flow path AND a strong driving force. Water tongues along the bottom layers, bypassing much of the section — displacement efficiency drops by roughly 7.5%, about 3x more than either change predicted alone.',
                    paramPatch: { uniformPermZ: 190, rho_o: 600 },
                    affectsAnalytical: false,
                },
            ],
        },
    ],
};
