# Independent SPE5 Case 1 reference, run with JutulDarcy. Run from the repository root:
# julia +1.12 --project=tools/jutul tools/jutul/spe5_reference.jl OUT.json
#
# The fluid, grid, relperm, well indices and schedule are read from
# opm/compositional/spe5/case.json, the same payload the ResSim scenario sends. The six-component
# EOS properties are transcribed from OPM's Spe5FluidSystem (the table in
# docs/COMPOSITIONAL_VALIDATION.md) because they are not part of the case payload.
#
# This is the reference asked for by #116: does a second compositional simulator cross the
# near-critical miscible-front saturation jump at month 218?
using JutulDarcy, JSON
using JutulDarcy.MultiComponentFlash
using JutulDarcy.Jutul
const MCF = JutulDarcy.MultiComponentFlash

const CASE_PATH = "opm/compositional/spe5/case.json"
const CASE = JSON.parsefile(CASE_PATH)
const R_OPM = 8.314472
const R_GAS = MCF.IDEAL_GAS_CONSTANT
const T_RES = (160.0 + 459.67) * 5 / 9
const Z0 = Float64.(CASE["initial_composition"])

# OPM Spe5FluidSystem: C1, C3, C6, C10, C15, C20.
const MW = [16.04e-3, 44.10e-3, 86.18e-3, 142.29e-3, 206.00e-3, 282.00e-3]
const TC = [343.0, 665.7, 913.4, 1111.8, 1270.0, 1380.0] .* 5 / 9
const PC = [667.8, 616.3, 436.9, 304.0, 200.0, 162.0] .* 6894.7573
const ZC = [0.290, 0.277, 0.264, 0.257, 0.245, 0.235]
const OMEGA = [0.0130, 0.1524, 0.3007, 0.4885, 0.6500, 0.8500]
const NAMES = ["C1", "C3", "C6", "C10", "C15", "C20"]
const VC = ZC .* R_OPM .* TC ./ PC
const PROPS = [MolecularProperty(MW[i], PC[i], TC[i], VC[i], OMEGA[i]) for i in 1:6]
const BIC = let b = zeros(6, 6)
    b[1, 5] = b[5, 1] = 0.05
    b[1, 6] = b[6, 1] = 0.05
    b[2, 5] = b[5, 2] = 0.005
    b[2, 6] = b[6, 2] = 0.005
    b
end
const EOS = GenericCubicEOS(
    MultiComponentMixture(PROPS, A_ij = BIC, names = NAMES),
    # PRCORR: C15 (omega 0.65) and C20 (0.85) take the 1978 quartic above omega = 0.49.
    PengRobinsonCorrected(),
)

const GRID = CASE["grid"]
const NX, NY, NZ = GRID["box_grid"]["nx"], GRID["box_grid"]["ny"], GRID["box_grid"]["nz"]
const CELLS = GRID["cells"]
const DX, DY = Float64(GRID["dx_m"]), Float64(GRID["dy_m"])
const DZ = Float64.(GRID["box_grid"]["layer_dz_m"])
const PORO = Float64(GRID["porosity"])
const KX = Float64.(GRID["box_grid"]["layer_kx_md"])
const KY = Float64.(GRID["box_grid"]["layer_ky_md"])
const KZ = Float64.(GRID["box_grid"]["layer_kz_md"])
const TOP = Float64(GRID["box_grid"]["top_depth_m"])
const P_TOP_BAR = Float64(CASE["initial_pressure_bar"])
const P_GRAD = Float64(CASE["initial_pressure_gradient_bar_per_m"])
const SW0 = Float64(CASE["water"]["initial_saturation"])
const WATER = CASE["water"]
const RHO_W = Float64(WATER["surface_density_kg_per_m3"])
const PVTW = (Float64(WATER["reference_pressure_bar"]), Float64(WATER["formation_volume_factor"]),
    Float64(WATER["compressibility_per_bar"]), Float64(WATER["viscosity_cp"]),
    Float64(WATER["viscosibility_per_bar"]))

# Jutul expects m² permeability. One row per direction, one column per cell.
function permeability_matrix()
    k = zeros(3, CELLS)
    for layer in 1:NZ, j in 1:NY, i in 1:NX
        cell = (layer - 1) * NX * NY + (j - 1) * NX + i
        k[:, cell] .= [KX[layer], KY[layer], KZ[layer]] .* 9.869233e-16
    end
    k
end

function surface_rates(q)
    # q is a ResSim surface volume [m³/day], Jutul wants [m³/s].
    q / 86400.0
end

# JutulDarcy 0.3.7's StoneIIMethod evaluates both two-phase oil curves at the three-phase oil
# saturation (`val_ow = krowd(so)`, `val_og = krogd(so)`). OPM's EclStone2Material, and ResSim's
# `k_ro_stone2_generic`, instead use `krow(Sw)` and `krog(1 - Swc - Sg)`, which is the gas-oil
# table evaluated at its own gas saturation. The dispatch point does not receive `krog`, so the
# curve and the connate water saturation travel with the method.
struct OPMStoneII{OG} <: JutulDarcy.AbstractThreePhaseOilMethod
    og::OG
    swcon::Float64
end

function JutulDarcy.compute_three_phase_oil_kr(method::OPMStoneII, val_ow, val_og, val_w, val_g,
        swcon, sg, sw, krowd)
    # krowd is Jutul's ow curve, indexed by oil saturation: krow(1 - Sw).
    krow = krowd(1 - sw)
    # method.og is indexed by gas-oil oil saturation: So = 1 - Swc - Sg.
    krog = method.og(1 - method.swcon - sg)
    krocw = krowd.k_max
    kro = krocw*((krow/krocw + val_w)*(krog/krocw + val_g) - (val_w + val_g))
    # ResSim's StoneTwoRelPerm clamps to [0, krocw]; OPM clamps only the lower bound.
    return clamp(kro, zero(kro), oftype(kro, krocw))
end

function relperms()
    swof = CASE["relperm"]["swof"]
    sgof = CASE["relperm"]["sgof"]
    sw_raw = Float64[r["sw"] for r in swof]
    krw_raw = Float64[r["krw"] for r in swof]
    krow_raw = Float64[r["krow"] for r in swof]
    sw = vcat(0.0, sw_raw)
    krw = vcat(0.0, krw_raw)
    krow = vcat(1.0, krow_raw)
    # krow is tabulated on water saturation in SWOF; Jutul's ow curve is on oil saturation.
    ow = PhaseRelativePermeability(reverse(1.0 .- sw), reverse(krow), label = :ow)

    sg = Float64[r["sg"] for r in sgof]
    krg = Float64[r["krg"] for r in sgof]
    krog = Float64[r["krog"] for r in sgof]
    so = reverse(1.0 .- SW0 .- sg)
    og = PhaseRelativePermeability(so, reverse(krog), label = :og)

    w = PhaseRelativePermeability(sw, krw, label = :w)
    g = PhaseRelativePermeability(sg, krg, label = :g)
    JutulDarcy.ReservoirRelativePermeabilities(w = w, g = g, ow = ow, og = og,
        three_phase_method = OPMStoneII(og, sw_raw[1]))
end

function build_model(; well_volume_m3 = 1e-4)
    mesh = CartesianMesh((NX, NY, NZ), (NX * DX, NY * DY, DZ),
        origin = [0.0, 0.0, TOP])
    domain = reservoir_domain(mesh; porosity = PORO, permeability = permeability_matrix(),
        temperature = T_RES)
    wi = CASE["wells"][1]["completions"][1]["well_index"] / (86400.0 * 1e8)
    volume = isnothing(well_volume_m3) ? missing : [well_volume_m3]
    producer = setup_well(domain, CELLS, name = :PROD, WI = [wi], simple_well = true,
        volumes = volume)
    water_injector = setup_well(domain, 1, name = :INJW, WI = [wi], simple_well = true,
        volumes = volume)
    gas_injector = setup_well(domain, 1, name = :INJG, WI = [wi], simple_well = true,
        volumes = volume)
    system = MultiPhaseCompositionalSystemLV(EOS,
        (AqueousPhase(), LiquidPhase(), VaporPhase()),
        reference_densities = (RHO_W, 1.0, 1.0))
    model, parameters = setup_reservoir_model(domain, system,
        wells = [producer, water_injector, gas_injector], extra_out = true)
    p_ref, bw, cw, mu, cmu = PVTW
    water = ConstMuBTable(p_ref * 1e5, 1.0 / bw, cw / 1e5, mu * 1e-3, cmu / 1e5)
    model = replace_variables!(model,
        RelativePermeabilities = relperms(),
        PhaseViscosities = ThreePhaseLBCViscositiesLV(water),
        PhaseMassDensities = ThreePhaseCompositionalDensitiesLV(water))
    # The case carries the SPE5 rock compressibility (5e-6/psi). JutulDarcy turns this into a
    # secondary FluidVolume and moves the static pore volume into the parameters, so the
    # parameter container from setup_reservoir_model has to be rebuilt afterwards.
    JutulDarcy.set_rock_compressibility!(model,
        reference_pressure = Float64(GRID["rock_reference_pressure_bar"]) * 1e5,
        compressibility = Float64(GRID["rock_compressibility_per_bar"]) / 1e5)
    parameters = setup_parameters(model)
    append!(model[:Reservoir].output_variables,
        [:Saturations, :OverallMoleFractions, :ImmiscibleSaturation, :PhaseMobilities, :RelativePermeabilities, :PhaseViscosities])
    return model, parameters, mesh
end

function initial_state(model, mesh)
    geo = tpfv_geometry(mesh)
    depth = geo.cell_centroids[3, :]
    p = (P_TOP_BAR .+ P_GRAD .* (depth .- TOP)) .* 1e5
    z = repeat(reshape(Z0, 6, 1), 1, CELLS)
    setup_reservoir_state(model, Pressure = p, OverallMoleFractions = z,
        ImmiscibleSaturation = fill(SW0, CELLS))
end

function limits()
    Dict{Symbol, Any}(
        :PROD => (bhp = CASE["wells"][1]["bhp_limit_bar"] * 1e5,),
        # The injectors are shut in at t=0; their schedule events carry the 10000 psia ceiling.
        :INJW => (bhp = 10000 * 6894.7573,),
        :INJG => (bhp = 10000 * 6894.7573,),
    )
end

const Q_OIL = surface_rates(Float64(CASE["wells"][1]["target_m3_per_day"]))
const Q_WATER = surface_rates(12000 * 0.158987294928)
const Q_GAS = surface_rates(12000 * 1000 * 0.028316846592)
const Z_GAS = Float64.(CASE["wells"][3]["injection_composition"])
const M_GAS = sum(Z_GAS .* MW)
# ResSim pins the SPE5 surface conditions at 101325 Pa, 288.71 K (pinned::spe5). The injection
# mass rate is surface volume x density, and this density is the EOS density, not ideal gas.
const RHO_GAS = let
    cond = (p = 101325.0, T = 288.71, z = Z_GAS)
    Z = MCF.mixture_compressibility_factor(EOS, cond)
    M_GAS / (Z * R_GAS * 288.71 / 101325.0)
end
const MASS_GAS = let w = Z_GAS .* MW; vcat(w ./ sum(w), 0.0) end
const MASS_WATER = vcat(zeros(6), 1.0)

function controls(kind)
    producer = ProducerControl(SurfaceOilRateTarget(Q_OIL))
    if kind === :none
        return Dict(:PROD => producer, :INJW => DisabledControl(), :INJG => DisabledControl())
    elseif kind === :water
        return Dict(:PROD => producer,
            :INJW => InjectorControl(SurfaceWaterRateTarget(Q_WATER), MASS_WATER, density = RHO_W),
            :INJG => DisabledControl())
    else
        # InjectorControl defaults to phases = ((1, 1.0),), i.e. it books the whole injection
        # stream to the aqueous surface phase. A gas injector must declare the vapor phase or the
        # SurfaceGasRateTarget equation has no free variable and the Jacobian is singular.
        return Dict(:PROD => producer,
            :INJW => DisabledControl(),
            :INJG => InjectorControl(SurfaceGasRateTarget(Q_GAS), MASS_GAS, density = RHO_GAS,
                phases = ((3, 1.0),)))
    end
end


# Same thresholds the ResSim scenario declares: 10 Mscf/STB in separator m3/m3, and water cut
# 5/6 (WOR 5 STB/STB).
const GOR_LIMIT_M3_PER_M3 = 1781.0760667903526
const WATERCUT_LIMIT = 5.0 / 6.0

# JutulDarcy's surface phase rates for an aqueous+hydrocarbon wellstream assign the aqueous
# reservoir volume fraction (Sw * rhoWW/rhoWS) as a fraction of the *surface* volume. The gas
# expansion factor then inflates the reported water volume; at SPE5's producer it is ~80x. The
# component mass rates are conserved, so rebuild the surface rates from them: water from its mass
# rate and surface density, hydrocarbons from the surface flash split (recovered from
# SurfaceWellConditions) and the remaining mass.
function corrected_surface_rates(result, i)
    wells = result.wells[:PROD]
    cond = only(result.result.states[i][:PROD][:SurfaceWellConditions])
    rho_s = cond.density
    vol = cond.volume_fractions
    q_mass = abs(wells[:mass_rate][i])
    q_water_mass = abs(wells[:Water_mass_rate][i])
    q_hc_mass = max(q_mass - q_water_mass, 0.0)
    hc_vol = vol[2] + vol[3]
    if hc_vol > 0
        s_l = vol[2]/hc_vol
        s_v = vol[3]/hc_vol
        rho_hc = s_l*rho_s[2] + s_v*rho_s[3]
    else
        s_l = s_v = rho_hc = 0.0
    end
    q_hc = rho_hc > 0 ? q_hc_mass/rho_hc : 0.0
    (oil = s_l*q_hc, gas = s_v*q_hc, water = q_water_mass/rho_s[1])
end

# Per-report Newton/timestep record. Jutul's `SimResult.reports` holds one entry per accepted
# report step: `:ministeps` is the list of accepted substeps, each carrying its Newton `:steps`.
# The last step of the last ministep is the accepted state; its reservoir CNV/MB maxima are the
# convergence residuals #116 asks for.
function report_diagnostics(result, i)
    rep = result.result.reports[i]
    if ismissing(rep) || !haskey(rep, :ministeps)
        return Dict("ministeps" => nothing, "newton_iterations" => nothing,
            "failed_ministeps" => nothing, "final_reservoir_cnv" => nothing,
            "final_reservoir_mb" => nothing)
    end
    ms = rep[:ministeps]
    newton = 0
    failed = 0
    for m in ms
        haskey(m, :steps) && (newton += length(m[:steps]))
        failed += m[:success] ? 0 : 1
    end
    cnv = nothing
    mb = nothing
    if !isempty(ms)
        last = ms[end]
        if haskey(last, :steps) && !isempty(last[:steps]) &&
                haskey(last[:steps][end], :errors) && haskey(last[:steps][end][:errors], :Reservoir)
            cnv_values = Float64[]
            mb_values = Float64[]
            for e in last[:steps][end][:errors][:Reservoir]
                haskey(e, :criterions) || continue
                haskey(e.criterions, :CNV) && append!(cnv_values, e.criterions.CNV.errors)
                haskey(e.criterions, :MB) && append!(mb_values, e.criterions.MB.errors)
            end
            cnv = isempty(cnv_values) ? nothing : maximum(cnv_values)
            mb = isempty(mb_values) ? nothing : maximum(mb_values)
        end
    end
    Dict("ministeps" => length(ms), "newton_iterations" => newton,
        "failed_ministeps" => failed, "final_reservoir_cnv" => cnv,
        "final_reservoir_mb" => mb)
end

function period_reports(result, year0)
    out = Dict[]
    wells = result.wells[:PROD]
    cell33 = 34  # zero-based 33 in the payload
    for i in eachindex(result.states)
        s = result.states[i]
        p = s[:Pressure] ./ 1e5
        sat = s[:Saturations]
        rates = corrected_surface_rates(result, i)
        qo = rates.oil
        qg = rates.gas
        qw = rates.water
        gor = max(abs(qg), abs(qo)) > 1e-14 ? abs(qg) / max(abs(qo), 1e-14) : nothing
        watercut = abs(qw) / max(abs(qw) + abs(qo), 1e-14)
        diag = report_diagnostics(result, i)
        push!(out, Dict(
            "day" => year0 * 365.25 + result.wells.time[i] / 86400.0,
            "mean_p_bar" => sum(p) / length(p),
            "min_p_bar" => minimum(p),
            "max_p_bar" => maximum(p),
            "cell33_p_bar" => p[cell33],
            "cell33_sw" => sat[1, cell33],
            "cell33_sg" => sat[3, cell33],
            "producer_oil_m3_day" => qo * 86400.0,
            "producer_gas_m3_day" => qg * 86400.0,
            "producer_water_m3_day" => qw * 86400.0,
            "producer_gor_m3_m3" => gor,
            "producer_watercut" => watercut,
            "raw_jutul_producer_water_m3_day" => abs(wells[:wrat][i]) * 86400.0,
            "raw_jutul_producer_watercut" => wells[:wcut][i],
            "producer_control" => string(wells[:control][i]),
            "producer_bhp_bar" => wells[:bhp][i] / 1e5,
            "diagnostics" => diag,
        ))
    end
    out
end

function stop_record(reports)
    for r in reports
        gor = r["producer_gor_m3_m3"]
        watercut = r["producer_watercut"]
        gor_hit = !isnothing(gor) && gor >= GOR_LIMIT_M3_PER_M3
        water_hit = watercut >= WATERCUT_LIMIT
        if gor_hit || water_hit
            return Dict("day" => r["day"], "gor_m3_m3" => gor, "watercut" => watercut,
                "reason" => gor_hit ? "gor" : "watercut", "report" => r)
        end
    end
    nothing
end

function main(out_path)
    model, parameters, mesh = build_model()
    state = initial_state(model, mesh)
    lim = limits()
    dt = 30.4375 * 86400.0
    periods = [(0, 2, :none)]
    for cycle in 0:8
        push!(periods, (2 + 2 * cycle, 3 + 2 * cycle, :water))
        push!(periods, (3 + 2 * cycle, 4 + 2 * cycle, :gas))
    end
    reports = Dict[]
    completed = true
    failed_at = nothing
    stopped = nothing
    for (year0, year1, kind) in periods
        steps = round(Int, (year1 - year0) * 365.25 / 30.4375)
        forces = setup_reservoir_forces(model; control = controls(kind), limits = lim)
        println("period $(year0)-$(year1) $(kind), $(steps) steps")
        result = try
            simulate_reservoir(state, model, fill(dt, steps), parameters = parameters,
                forces = forces, info_level = -1)
        catch err
            completed = false
            failed_at = Dict("year0" => year0, "year1" => year1, "period" => string(kind),
                "error" => sprint(showerror, err))
            println("FAILED in $(year0)-$(year1) $(kind): ", sprint(showerror, err))
            Base.showerror(stderr, err, catch_backtrace())
            break
        end
        append!(reports, period_reports(result, year0))
        state = result.result.states[end]
        # The declared stop is scenario metadata, not part of the case. Record the first report that
        # would fire it, but keep simulating: #116 needs the same physical front on both sides of
        # month 218, and the comparison truncates at the recorded report instead.
        if isnothing(stopped)
            stopped = stop_record(reports)
            if !isnothing(stopped)
                println("  declared stop fires at $(stopped["day"]) d by $(stopped["reason"]); " *
                    "continuing to the end of the schedule")
            end
        end
        last_report = reports[end]
        println("  reached $(last_report["day"]) d, mean p $(last_report["mean_p_bar"]), " *
            "GOR $(last_report["producer_gor_m3_m3"]), watercut $(last_report["producer_watercut"])")
        flush(stdout)
    end
    out = Dict(
        "schema" => "ressim-spe5-reference/1",
        "jutuldarcy" => string(pkgversion(JutulDarcy)),
        "julia" => string(VERSION),
        "case" => CASE_PATH,
        "completed" => completed,
        "reached_day" => isempty(reports) ? 0.0 : reports[end]["day"],
        "failed_at" => failed_at,
        "stopped" => stopped,
        "gor_limit_m3_m3" => GOR_LIMIT_M3_PER_M3,
        "watercut_limit" => WATERCUT_LIMIT,
        "tolerances" => Dict("tol_cnv" => 1e-3, "tol_mb" => 1e-7,
            "note" => "JutulDarcy defaults; not the ResSim 1e-8 per-cell mole balance"),
        "rate_correction" => "producer surface rates are rebuilt from component mass rates: " *
            "water = Water_mass_rate/rhoWS, hydrocarbons = remaining mass split by the " *
            "SurfaceWellConditions flash. JutulDarcy 0.3.7 reports wrat from the aqueous reservoir " *
            "volume fraction, which gas expansion inflates (~80x at SPE5's producer).",
        "reports" => reports,
    )
    open(out_path, "w") do io
        JSON.print(io, out, 2)
        println(io)
    end
end

if abspath(PROGRAM_FILE) == @__FILE__
    main(ARGS[1])
end
