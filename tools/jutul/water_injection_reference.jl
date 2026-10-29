# #46: 1-D water injection into SPE3 gas, immiscible water, against JutulDarcy. Run from the
# repository root:
# julia +1.12 --project=tools/jutul tools/jutul/water_injection_reference.jl OUT.json
#
# The case stays above the fluid's dew point everywhere, so no hydrocarbon liquid forms: what is
# being compared is the water phase, its mobility against the gas, the PVTW water and the
# connection law of a water injector and a gas producer. Both simulators run Stone II, but it is
# never exercised here, and the fixture records that every state stayed single-phase vapour.
#
# Time convergence is part of the fixture (COMPOSITIONAL_C12_FORENSICS.md: an oracle is only an
# oracle once it is shown to converge in time). The same case runs at four report steps; the
# differences between successive refinements are written next to the trajectories.
using JutulDarcy, JSON
using JutulDarcy.MultiComponentFlash
using JutulDarcy.Jutul
const MCF = JutulDarcy.MultiComponentFlash
const R = MCF.IDEAL_GAS_CONSTANT
const T = (200.0 + 459.67)*5/9
const Z0 = [0.0121, 0.0194, 0.6599, 0.0869, 0.0591, 0.0967, 0.0472, 0.0153, 0.0034]

# SPE3, as transcribed in tools/jutul/spe3_reference.jl from Juell, Whitson & Hoda, SPE 121252
# (2009), Tables 3–5: MW, Tc, Pc, Zc, shift, omega.
rows = [
    (44.01, 304.21, 73.82, .27400, -.00089, .225),
    (28.02, 126.27, 33.90, .29000, -.16453, .040),
    (16.04, 186.61, 46.20, .28800, -.17817, .013),
    (30.07, 305.33, 48.80, .28500, -.06456, .098),
    (44.10, 369.85, 42.50, .28100, -.06439, .152),
    (67.28, 396.22, 34.35, .27228, -.18129, .234),
    (110.90, 572.50, 25.94, .26444, .12080, .332),
    (170.90, 630.22, 16.92, .25140, .23442, .495),
    (282.10, 862.61, 8.61, .22436, .54479, .833),
]
names = ["CO2", "N2", "C1", "C2", "C3", "C4-6", "C7+1", "C7+2", "C7+3"]
props = [MolecularProperty(mw/1000, pc*1e5, tc, zc*R*tc/(pc*1e5), w) for (mw, tc, pc, zc, s, w) in rows]
lower = [[], [0.0], [.13, .036], [.135, .05, 0], [.1277, .08, 0, 0],
    [.1, .1002, .09281, 0, 0], [.1, .1, 0, .00385, .00385, 0],
    [.1, .1, 0, .0063, .0063, 0, 0], [.1, .1, .1392, .006, .006, 0, 0, 0]]
bic = zeros(9, 9)
for i in 1:9, j in 1:i-1
    bic[i, j] = bic[j, i] = lower[i][j]
end
eos = GenericCubicEOS(MultiComponentMixture(props, A_ij = bic, names = names), PengRobinson(), volume_shift = [r[5] for r in rows])

# --- The case, in the units the fixture records them in -------------------------------------
const CELLS = 10
const DX, DY, DZ = 20.0, 10.0, 10.0
const POROSITY = 0.2
const PERM_MD = 100.0
const P0_BAR = 300.0
const SW0 = 0.2
const INJ_BHP_BAR = 330.0
const PROD_BHP_BAR = 270.0
const WI = 0.05                 # m³·cP/(day·bar)
const WELL_VOLUME_M3 = 1e-4
const HORIZON_DAYS = 60.0
# PVTW: p_ref [bar], Bw, cw [1/bar], mu [cP], viscosibility [1/bar]; surface density [kg/m³].
const PVTW = (200.0, 1.02, 4.5e-5, 0.5, 0.0)
const RHO_W_SURFACE = 1020.0
# SWOF (sw, krw, krow) and SGOF (sg, krg, krog). Zero capillary pressure.
const SWOF = [(0.2, 0.0, 0.9), (0.3, 0.02, 0.6), (0.4, 0.06, 0.38), (0.5, 0.12, 0.22),
    (0.6, 0.2, 0.1), (0.7, 0.3, 0.03), (0.8, 0.42, 0.0), (0.9, 0.56, 0.0), (1.0, 0.7, 0.0)]
const SGOF = [(0.0, 0.0, 0.9), (0.1, 0.0, 0.6), (0.2, 0.05, 0.4), (0.3, 0.12, 0.25),
    (0.4, 0.22, 0.12), (0.5, 0.34, 0.04), (0.6, 0.47, 0.0), (0.7, 0.6, 0.0), (0.8, 0.72, 0.0)]

# m³·cP/(day·bar) -> SI, as spe3_reference.jl converts it.
wi_si(wi) = wi/(86400*1e8)

function relperms(sys)
    # Jutul tabulates each curve on its own phase's saturation; the oil curves are on So.
    kr(s, k, label) = PhaseRelativePermeability(collect(s), collect(k), label = label)
    sw = [0.0; [r[1] for r in SWOF]]
    krw = kr(sw, [0.0; [r[2] for r in SWOF]], :w)
    # Both tables already reach So = 0 (sw = 1 and sg = 1 - swc), so no zero row is prepended.
    krow = kr(reverse([1.0 - r[1] for r in SWOF]), reverse([r[3] for r in SWOF]), :ow)
    sg = [r[1] for r in SGOF]
    krg = kr(sg, [r[2] for r in SGOF], :g)
    so_g = reverse([1.0 - SWOF[1][1] - r[1] for r in SGOF])
    krog = kr(so_g, reverse([r[3] for r in SGOF]), :og)
    return JutulDarcy.ReservoirRelativePermeabilities(w = krw, g = krg, ow = krow, og = krog,
        three_phase_method = StoneIIMethod())
end

function run(dt_days)
    g = CartesianMesh((CELLS, 1, 1), (CELLS*DX, DY, DZ))
    domain = reservoir_domain(g, porosity = POROSITY, permeability = PERM_MD*9.869233e-16, temperature = T)
    # ResSim's wells carry no wellbore storage. Jutul's simple-well node defaults to the hole
    # volume (~0.3 m³ here), starts full of reservoir gas and injects that before any water: a 1%
    # first-day shortfall that persists as a 5e-4 Sw offset. A negligible node volume removes it.
    injector = setup_well(domain, 1, name = :Injector, WI = [wi_si(WI)], simple_well = true,
        volumes = [WELL_VOLUME_M3])
    producer = setup_well(domain, CELLS, name = :Producer, WI = [wi_si(WI)], simple_well = true,
        volumes = [WELL_VOLUME_M3])
    sys = MultiPhaseCompositionalSystemLV(eos, (AqueousPhase(), LiquidPhase(), VaporPhase()),
        reference_densities = (RHO_W_SURFACE, 1.0, 1.0))
    model, parameters = setup_reservoir_model(domain, sys, wells = [injector, producer], extra_out = true)
    p_ref, bw, cw, mu, cmu = PVTW
    water = ConstMuBTable(p_ref*1e5, 1.0/bw, cw/1e5, mu*1e-3, cmu/1e5)
    model = replace_variables!(model, RelativePermeabilities = relperms(sys),
        PhaseViscosities = ThreePhaseLBCViscositiesLV(water),
        PhaseMassDensities = ThreePhaseCompositionalDensitiesLV(water))
    append!(model[:Reservoir].output_variables, [:Saturations, :OverallMoleFractions, :ImmiscibleSaturation])
    initial = setup_reservoir_state(model, Pressure = P0_BAR*1e5, OverallMoleFractions = Z0,
        ImmiscibleSaturation = SW0)
    mix = [zeros(9); 1.0]
    control = Dict(
        :Injector => InjectorControl(BottomHolePressureTarget(INJ_BHP_BAR*1e5), mix, density = RHO_W_SURFACE),
        :Producer => ProducerControl(BottomHolePressureTarget(PROD_BHP_BAR*1e5)))
    forces = setup_reservoir_forces(model, control = control)
    steps = round(Int, HORIZON_DAYS/dt_days)
    result = simulate_reservoir(initial, model, fill(dt_days*86400.0, steps), parameters = parameters,
        forces = forces, info_level = -1, tol_cnv = 1e-9, tol_mb = 1e-10)
    @assert length(result.states) == steps "incomplete JutulDarcy run at dt = $dt_days d"
    reports = []
    single_vapour = true
    max_dz = 0.0
    for (i, s) in enumerate(result.states)
        # Phase order is (aqueous, liquid, vapour). With no liquid the hydrocarbons move as one
        # phase and their composition cannot change, so only p and Sw are recorded; the
        # composition drift is recorded as a single number instead.
        single_vapour &= all(s[:Saturations][2, :] .< 1e-12)
        max_dz = max(max_dz, maximum(abs.(s[:OverallMoleFractions] .- Z0)))
        push!(reports, Dict("day" => i*dt_days, "p" => s[:Pressure]./1e5,
            "sw" => vec(s[:ImmiscibleSaturation])))
    end
    return Dict("dt_days" => dt_days, "reports" => reports, "single_vapour" => single_vapour,
        "max_composition_drift" => max_dz)
end

runs = [run(dt) for dt in (2.0, 1.0, 0.5, 0.25)]

# Successive-refinement differences at the coarse run's report days, in p [bar] and Sw.
function at_day(r, day)
    only(filter(x -> isapprox(x["day"], day), r["reports"]))
end
convergence = []
for k in 1:length(runs)-1
    coarse, fine = runs[k], runs[k+1]
    dp = 0.0
    dsw = 0.0
    for rep in coarse["reports"]
        f = at_day(fine, rep["day"])
        dp = max(dp, maximum(abs.(rep["p"] .- f["p"])))
        dsw = max(dsw, maximum(abs.(rep["sw"] .- f["sw"])))
    end
    push!(convergence, Dict("coarse_dt_days" => coarse["dt_days"], "fine_dt_days" => fine["dt_days"],
        "max_dp_bar" => dp, "max_dsw" => dsw))
    println("dt $(coarse["dt_days"]) -> $(fine["dt_days"]): max dp $dp bar, max dSw $dsw")
end

out = Dict("schema" => "ressim-water-injection-reference/1",
    "jutuldarcy" => string(pkgversion(JutulDarcy)), "julia" => string(VERSION),
    "fluid" => "SPE3 (Juell, Whitson & Hoda 2009), 200 F", "initial_composition" => Z0,
    "cells" => CELLS, "dx_m" => DX, "dy_m" => DY, "dz_m" => DZ, "porosity" => POROSITY,
    "permeability_md" => PERM_MD, "initial_pressure_bar" => P0_BAR,
    "initial_water_saturation" => SW0, "injector_bhp_bar" => INJ_BHP_BAR,
    "producer_bhp_bar" => PROD_BHP_BAR, "well_index" => WI, "well_volume_m3" => WELL_VOLUME_M3, "horizon_days" => HORIZON_DAYS,
    "pvtw" => Dict("reference_pressure_bar" => PVTW[1], "formation_volume_factor" => PVTW[2],
        "compressibility_per_bar" => PVTW[3], "viscosity_cp" => PVTW[4],
        "viscosibility_per_bar" => PVTW[5], "surface_density_kg_per_m3" => RHO_W_SURFACE),
    "swof" => [collect(r) for r in SWOF], "sgof" => [collect(r) for r in SGOF],
    # The matched run (ResSim's report step) in full, the finest at whole days, and for every
    # run the facts the Rust test asserts on.
    "matched" => runs[2], "finest" => Dict("dt_days" => runs[4]["dt_days"],
        "reports" => filter(r -> isinteger(r["day"]), runs[4]["reports"])),
    "runs" => [Dict("dt_days" => r["dt_days"], "single_vapour" => r["single_vapour"],
        "max_composition_drift" => r["max_composition_drift"]) for r in runs],
    "convergence" => convergence)
open(ARGS[1], "w") do io
    JSON.print(io, out, 2)
    println(io)
end
