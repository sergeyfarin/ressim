# Dump the SPE5 JutulDarcy reference's full cell state (pressure, overall composition,
# saturations) at monthly reports FIRST..LAST (#116 cross-flash evidence). Run from the
# repository root:
#
#   julia +1.12 --project=tools/jutul tools/jutul/spe5_state_dump.jl OUT.json [FIRST LAST]
#
# The model is the committed reference (tools/jutul/spe5_reference.jl). Defaults: reports 205..216.
# Cell order is the payload's (zero-based cell c is Jutul cell c + 1). Pressure is in bar;
# saturations are [water, liquid, vapour].
include(joinpath(@__DIR__, "spe5_reference.jl"))

function dump_states(out_path, first, last)
    model, parameters, mesh = build_model()
    state = initial_state(model, mesh)
    lim = limits()
    dt = 30.4375 * 86400.0
    periods = [(0, 2, :none)]
    for cycle in 0:8
        push!(periods, (2 + 2 * cycle, 3 + 2 * cycle, :water))
        push!(periods, (3 + 2 * cycle, 4 + 2 * cycle, :gas))
    end
    dumps = Dict[]
    for (year0, year1, kind) in periods
        year0 * 12 >= last && break
        steps = min(round(Int, (year1 - year0) * 12), last - year0 * 12)
        forces = setup_reservoir_forces(model; control = controls(kind), limits = lim)
        result = simulate_reservoir(state, model, fill(dt, steps); parameters = parameters,
            forces = forces, info_level = -1, error_on_incomplete = true, simulate_kwargs()...)
        for i in eachindex(result.states)
            month = round(Int, year0 * 12 + i)
            (first <= month <= last) || continue
            s = result.states[i]
            push!(dumps, Dict("month" => month,
                "day" => year0 * 365.25 + result.wells.time[i] / 86400.0,
                "p_bar" => s[:Pressure] ./ 1e5,
                "z" => [s[:OverallMoleFractions][:, c] for c in 1:CELLS],
                "sat" => [s[:Saturations][:, c] for c in 1:CELLS]))
        end
        state = result.result.states[end]
        println("period $(year0)-$(year1) $(kind): $(steps) reports"); flush(stdout)
    end
    open(out_path, "w") do io
        JSON.print(io, Dict("schema" => "ressim-spe5-jutul-states/1",
            "jutuldarcy" => string(pkgversion(JutulDarcy)), "case" => CASE_PATH,
            "configuration" => model_configuration(model, parameters),
            "states" => dumps))
    end
end

if abspath(PROGRAM_FILE) == @__FILE__
    first = length(ARGS) >= 3 ? parse(Int, ARGS[2]) : 205
    last = length(ARGS) >= 3 ? parse(Int, ARGS[3]) : 216
    dump_states(ARGS[1], first, last)
end
