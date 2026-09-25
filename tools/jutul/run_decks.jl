# Run OPM decks through JutulDarcy, the second independent simulator behind docs/BENCHMARKS.md (#54).
#
#     julia +1.12 --project=tools/jutul tools/jutul/run_decks.jl OUT_DIR NAME=DECK.DATA ...
#
# Writes OUT_DIR/NAME.json per deck: report times [d], field summary vectors converted to the units
# the rest of the pipeline uses (bar, Sm³, Sm³/d), cell pressure and saturations per report step,
# the deck keywords JutulDarcy ignores, and the wall time. tools/jutul/compare_jutul.py turns
# those into benchmark records.
#
# The project pins JutulDarcy 0.3.7, which needs Julia 1.12: under 1.13 it fails with "iteration
# is deliberately unsupported for CartesianIndex".

using Pkg
Pkg.activate(@__DIR__)
Pkg.offline(true)
Pkg.instantiate()

using JutulDarcy
using JSON

# JutulDarcy 0.3.7 bug: initialising a live-oil deck from PRESSURE/SWAT/SGAS hands
# `blackoil_unknown_init` the per-PVT-region tuple of Rs(p) tables instead of one table, and fails
# as soon as a cell starts with free gas ("objects of type Tuple{LinearInterpolant} are not
# callable"). Our decks have one PVT region, so unwrap it. This changes no physics; remove it when
# the pinned version is upgraded past the fix.
# (Only the dry-gas signature, `F_rv::Nothing`: none of our decks vaporizes oil.)
function JutulDarcy.blackoil_unknown_init(F_rs::Tuple{Any}, F_rv::Nothing, sw, so, sg, rs, rv, p)
    return JutulDarcy.blackoil_unknown_init(only(F_rs), F_rv, sw, so, sg, rs, rv, p)
end

# Keywords present in our decks that JutulDarcy's parser warns it ignores. A comparison on a deck
# using one of them is not a same-model comparison for the physics the keyword controls.
const IGNORED = ("STONE1", "STONE2", "DRSDT", "VAPPARS")

const DAY = 86400.0
const BAR = 1e5

function phase_names(result)
    system = result.extra[:case].model[:Reservoir].system
    map(JutulDarcy.get_phases(system)) do phase
        name = string(nameof(typeof(phase)))
        name == "AqueousPhase" ? "sw" : name == "LiquidPhase" ? "so" : name == "VaporPhase" ? "sg" : name
    end
end

function field_vectors(result)
    field = result.summary["VALUES"]["FIELD"]
    out = Dict{String, Vector{Float64}}()
    for (key, values) in field
        if key == "FPR"
            out[key] = values ./ BAR
        elseif endswith(key, "R") && startswith(key, "F") && key ∉ ("FGOR",)
            out[key] = values .* DAY  # rates: Sm³/s -> Sm³/d
        else
            out[key] = copy(values)  # totals [Sm³], ratios
        end
    end
    out
end

function run_deck(name, path, out_dir)
    deck_text = read(path, String)
    ignored = [k for k in IGNORED if occursin(Regex("^\\s*$k\\b", "m"), deck_text)]
    started = time()
    result = simulate_data_file(path, info_level = -1)
    wall = time() - started
    phases = phase_names(result)
    states = result.states
    fields = map(states) do state
        entry = Dict{String, Any}("p" => state[:Pressure] ./ BAR)
        saturations = state[:Saturations]
        for (row, phase) in enumerate(phases)
            entry[phase] = saturations[row, :]
        end
        haskey(entry, "sg") || (entry["sg"] = zeros(length(entry["p"])))
        haskey(entry, "sw") || (entry["sw"] = zeros(length(entry["p"])))
        entry
    end
    record = Dict(
        "case" => name,
        "deck" => path,
        "jutuldarcy" => string(pkgversion(JutulDarcy)),
        "julia" => string(VERSION),
        "ignored_keywords" => ignored,
        "wall_s" => wall,
        "time_days" => result.summary["TIME"].seconds ./ DAY,
        "field" => field_vectors(result),
        "fields" => fields,
    )
    open(joinpath(out_dir, "$name.json"), "w") do io
        JSON.print(io, record)
    end
    println("jutul $name: $(length(states)) reports, $(round(wall, digits = 1)) s",
            isempty(ignored) ? "" : ", ignores $(join(ignored, ", "))")
end

function main(args)
    length(args) >= 2 || error("usage: run_decks.jl OUT_DIR NAME=DECK.DATA ...")
    out_dir = args[1]
    mkpath(out_dir)
    failed = String[]
    for spec in args[2:end]
        name, path = split(spec, "=", limit = 2)
        try
            run_deck(String(name), String(path), out_dir)
        catch error
            println("jutul $name: FAILED: ", sprint(showerror, error)[1:min(end, 400)])
            push!(failed, String(name))
        end
    end
    isempty(failed) || exit(1)
end

main(ARGS)
