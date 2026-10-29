# Independent SPE3 thermodynamics and 1D depletion. Run from the repository root:
# julia +1.12 --project=tools/jutul tools/jutul/spe3_reference.jl OUT.json
# Data: Juell, Whitson & Hoda, SPE 121252 (2009), Tables 3–5; separator: SPE 12278 Table 3.
using JutulDarcy, JSON
using JutulDarcy.MultiComponentFlash
using JutulDarcy.Jutul
const MCF = JutulDarcy.MultiComponentFlash
const R = MCF.IDEAL_GAS_CONSTANT
const T = (200.0 + 459.67)*5/9
const Z0 = [0.0121, 0.0194, 0.6599, 0.0869, 0.0591, 0.0967, 0.0472, 0.0153, 0.0034]

# Independently transcribed from Table 3: MW, Tc, Pc, Zc, shift, omega.
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

function flashed(p, temp, z)
    cond = (p = p, T = temp, z = copy(z))
    beta, K, rep = flash_2ph(eos, cond; method = SSINewtonFlash(), tolerance = 1e-11,
        z_min = nothing, extra_out = true)
    @assert rep.converged || rep.stability.stable "unconverged independent flash: $cond, $rep"
    if isnan(beta)
        beta = single_phase_label(eos.mixture, cond)
        x = y = copy(z)
    else
        x = [liquid_mole_fraction(z[i], K[i], beta) for i in 1:9]
        y = x .* K
    end
    forces = force_coefficients(eos, cond)
    zl = mixture_compressibility_factor(eos, (p=p, T=temp, z=x, phase=:liquid), forces)
    zv = mixture_compressibility_factor(eos, (p=p, T=temp, z=y, phase=:vapor), forces)
    vm(z, zz) = molar_volume(eos, p, temp, FlashedPhase(z, zz))
    Dict("p"=>p, "T"=>temp, "z"=>z, "beta"=>beta, "x"=>x, "y"=>y,
        "vl"=>vm(x, zl), "vv"=>vm(y, zv))
end

function separated(primary)
    feed = copy(Z0)
    gas = zeros(9)
    liquid_volume = 0.0
    stages = [(primary, 80.0), (65.0, 80.0), (14.7, 60.0)]
    for (psi, f) in stages
        n = sum(feed)
        n == 0 && break
        r = flashed(psi*6894.757293168, (f+459.67)*5/9, feed/n)
        feed = n*(1-r["beta"])*r["x"]
        gas .+= n*r["beta"]*r["y"]
        liquid_volume = sum(feed)*r["vl"]
    end
    # Meter the collected gas as vapour; do not flash it into a fourth separator.
    p, temp, y = 14.7*6894.757293168, (60.0+459.67)*5/9, gas/sum(gas)
    zv = mixture_compressibility_factor(eos, (p=p, T=temp, z=y, phase=:vapor))
    gas_volume = sum(gas)*molar_volume(eos, p, temp, FlashedPhase(y, zv))
    Dict("primary_psia"=>primary, "liquid"=>feed, "gas"=>gas,
        "liquid_volume"=>liquid_volume, "gas_volume"=>gas_volume)
end

function cvd()
    z = copy(Z0)
    n = 1.0
    start = flashed(260e5, T, z)
    cell_volume = start["vv"]
    records = []
    for bar in [260., 250., 240., 235., 230., 220., 200., 180., 160., 140., 100., 70., 50.]
        r = flashed(bar*1e5, T, z)
        nl, nv = n*(1-r["beta"]), n*r["beta"]
        excess = nl*r["vl"] + nv*r["vv"] - cell_volume
        removed = max(0.0, excess/r["vv"])
        @assert removed <= nv
        remaining = nl*r["x"] + (nv-removed)*r["y"]
        n = sum(remaining)
        z = remaining/n
        push!(records, Dict("p"=>bar*1e5, "z"=>z, "moles"=>n,
            "sl"=>nl*r["vl"]/cell_volume, "removed"=>removed))
    end
    records
end

out = Dict("schema"=>"ressim-spe3-reference/1", "jutuldarcy"=>string(pkgversion(JutulDarcy)),
    "multicomponentflash"=>string(pkgversion(MCF)), "julia"=>string(VERSION), "gas_constant"=>R,
    "source"=>"Juell, Whitson & Hoda (2009), SPE 121252, Tables 3–5; Kenyon & Behie (1987), Table 3",
    "eos"=>"PR, volume translation V=ZRT/p-sum(x*s*b)",
    "states"=>[flashed(p*1e5, T, Z0) for p in [300., 260., 245., 240., 235., 230., 220., 200., 170., 140., 100., 50.]],
    "separators"=>[separated(815.0), separated(315.0)], "cvd"=>cvd())

# A 3-cell fixed-pore-volume column, gas-only BHP withdrawal. This is a verification case,
# not the SPE3 3D reservoir. Liquid is immobile, making the dew-point crossing unambiguous.
function depletion()
    g = CartesianMesh((3, 1, 1), (30.0, 10.0, 10.0))
    domain = reservoir_domain(g, porosity = 0.2, permeability = 10e-3*9.869233e-13, temperature = T)
    # Convert m³*cP/(day*bar) to SI (m³*Pa*s/(s*Pa)). No wellbore storage.
    well = setup_well(domain, 3, name = :Producer, WI = [0.001/(86400*1e8)], simple_well = true)
    sys = MultiPhaseCompositionalSystemLV(eos, (LiquidPhase(), VaporPhase()))
    model, parameters = setup_reservoir_model(domain, sys, wells = [well], extra_out = true)
    kr = BrooksCoreyRelativePermeabilities(sys, [1.0, 1.0], [0.0, 0.0], [0.0, 1.0])
    model = replace_variables!(model, RelativePermeabilities = kr)
    append!(model[:Reservoir].output_variables, [:Saturations, :OverallMoleFractions])
    initial = setup_reservoir_state(model, Pressure = 260e5, OverallMoleFractions = Z0)
    control = Dict(:Producer => ProducerControl(BottomHolePressureTarget(140e5)))
    forces = setup_reservoir_forces(model, control = control)
    result = simulate_reservoir(initial, model, fill(86400.0, 150), parameters = parameters,
        forces = forces, info_level = -1, tol_cnv = 1e-9, tol_mb = 1e-10)
    @assert length(result.states) == 150 "incomplete JutulDarcy depletion"
    reports = [Dict("day"=>i, "p"=>s[:Pressure]./1e5, "sl"=>vec(s[:Saturations][1,:]),
        "z"=>[vec(s[:OverallMoleFractions][:,c]) for c in 1:3]) for (i,s) in enumerate(result.states)]
    println("depletion: final p = ", reports[end]["p"], ", Sl = ", reports[end]["sl"])
    Dict("cells"=>3, "pore_volume_m3"=>200.0, "initial_pressure_bar"=>260.0,
        "bhp_bar"=>140.0, "well_index"=>0.001, "dt_days"=>1.0, "reports"=>reports)
end
out["depletion"] = depletion()
open(ARGS[1], "w") do io
    JSON.print(io, out, 2)
    println(io)
end
