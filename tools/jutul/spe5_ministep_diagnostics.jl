# Per-ministep Newton diagnostics for the SPE5 JutulDarcy reference across the month-218 window
# (#116). Run from the repository root:
#
#   julia +1.12 --project=tools/jutul tools/jutul/spe5_ministep_diagnostics.jl OUT.json [FIRST LAST]
#
# It runs the committed reference model (tools/jutul/spe5_reference.jl, same case, EOS, relperm,
# wells and schedule) from t = 0 and records every ministep of reports FIRST..LAST (default
# 205..220): its dt, success flag and, for every Newton step, the reservoir CNV and MB per
# component as Jutul computes them. The simulation stops after report LAST.
#
# Jutul's criteria (JutulDarcy multicomponent.jl, `compositional_criterion`) are not ResSim's:
#   CNV_c = dt |R_c| (1 - Sw) / (V_pore rho_hc)   per cell, with rho_hc the hydrocarbon density at
#           saturations renormalized by (1 - Sw). Since V_pore rho_hc = m_hc / (1 - Sw), this is
#           the component's mass imbalance relative to the cell's hydrocarbon mass, times
#           (1 - Sw)^2; tolerance 1e-3.
#   MB_c  = dt |sum_cells R_c| / M_domain          signed sum over cells, relative to the whole
#           domain's total mass (water included); tolerance 1e-7.
# Water is different: its CNV compares the signed dt R_w / (rho_w V_pore), so a negative water
# residual never raises it, and its "MB" sums those per-cell volume-scaled values and multiplies by
# dt again before dividing by M_domain. That water MB is not a mass balance; tables built from this
# output (tools/jutul/spe5_compare.py --ministeps) keep it apart from the hydrocarbon MB.
# ResSim's test is per cell and per component against that component's own previous moles, 1e-8.
# A ministep also needs every well node (CNV, tolerance 1e-2) and every facility equation (BHP,
# surface rates, control; 1e-3) converged; those are recorded under `other_groups` with only the
# criteria that carry a finite tolerance, so a ministep that failed with the reservoir converged
# shows which equation held it.
# Compare the two only after converting one into the other's normalization.
include(joinpath(@__DIR__, "spe5_reference.jl"))

function step_errors(step)
    out = Dict{String, Any}()
    haskey(step, :errors) || return out
    haskey(step, :converged) && (out["converged"] = step[:converged])
    # Why Jutul rejected the step, when it says (a linear-solver or update failure, not a
    # tolerance miss); recorded as text.
    haskey(step, :failure) && (out["failure"] = string(step[:failure]))
    if haskey(step[:errors], :Reservoir)
        for e in step[:errors][:Reservoir]
            haskey(e, :criterions) || continue
            for key in (:CNV, :MB)
                haskey(e.criterions, key) || continue
                out[string(key)] = Dict(string(n) => v for (n, v) in
                    zip(e.criterions[key].names, e.criterions[key].errors))
            end
        end
    end
    # Every other group (well nodes and the facility) as group -> equation -> criterion ->
    # [max error, tolerance]: a ministep can fail with the reservoir converged.
    others = Dict{String, Any}()
    for (group, errs) in step[:errors]
        group == :Reservoir && continue
        for e in errs
            haskey(e, :criterions) || continue
            for (ck, cv) in pairs(e.criterions)
                tol = get(e.tolerances, ck, Inf)
                isfinite(tol) || continue
                others["$(group).$(e.name).$(ck)"] = [maximum(cv.errors), tol]
            end
        end
    end
    out["other_groups"] = others
    out
end

function ministep_records(result, year0, first, last)
    out = Dict[]
    for (i, rep) in enumerate(result.result.reports)
        month = round(Int, year0 * 12 + i)
        (first <= month <= last) || continue
        ismissing(rep) && continue
        for (k, m) in enumerate(get(rep, :ministeps, []))
            steps = get(m, :steps, [])
            push!(out, Dict(
                "month" => month,
                "ministep" => k,
                "dt_days" => m[:dt] / 86400.0,
                "success" => m[:success],
                "newton" => [step_errors(s) for s in steps],
            ))
        end
    end
    out
end

function main(out_path, first, last)
    model, parameters, mesh = build_model()
    state = initial_state(model, mesh)
    lim = limits()
    dt = 30.4375 * 86400.0
    periods = [(0, 2, :none)]
    for cycle in 0:8
        push!(periods, (2 + 2 * cycle, 3 + 2 * cycle, :water))
        push!(periods, (3 + 2 * cycle, 4 + 2 * cycle, :gas))
    end
    records = Dict[]
    for (year0, year1, kind) in periods
        year0 * 12 >= last && break
        steps = min(round(Int, (year1 - year0) * 12), last - year0 * 12)
        forces = setup_reservoir_forces(model; control = controls(kind), limits = lim)
        result = simulate_reservoir(state, model, fill(dt, steps); parameters = parameters,
            forces = forces, info_level = -1, error_on_incomplete = true, simulate_kwargs()...)
        append!(records, ministep_records(result, year0, first, last))
        state = result.result.states[end]
        println("period $(year0)-$(year1) $(kind): $(steps) reports"); flush(stdout)
    end
    open(out_path, "w") do io
        JSON.print(io, Dict(
            "schema" => "ressim-spe5-jutul-ministeps/2",
            "jutuldarcy" => string(pkgversion(JutulDarcy)),
            "case" => CASE_PATH,
            "reports" => [first, last],
            "configuration" => model_configuration(model, parameters),
            "tolerances" => Dict("tol_cnv" => 1e-3, "tol_mb" => 1e-7),
            "ministeps" => records,
        ))
    end
end

if abspath(PROGRAM_FILE) == @__FILE__
    first = length(ARGS) >= 3 ? parse(Int, ARGS[2]) : 205
    last = length(ARGS) >= 3 ? parse(Int, ARGS[3]) : 220
    main(ARGS[1], first, last)
end
