# Flash a list of (p, z) points with the SPE5 reference EOS in MultiComponentFlash (#116
# cross-flash evidence). Run from the repository root:
#
#   julia +1.12 --project=tools/jutul tools/jutul/spe5_cross_flash.jl POINTS.json OUT.json
#
# POINTS.json is a list of {"label", "p_bar", "z"}; z is normalized here. For every point it
# records, with the convergence status of each solve rather than silencing it:
#
# - `tight`: flash_2ph with SSINewtonFlash at tolerance 1e-12 (200000 iterations), the setting the
#   comparison uses; `default`: JutulDarcy's in-simulator setting (SSIFlash, 1e-8, 25000), which
#   the reference run used.
# - For each: vapour fraction V (null when single phase), iteration count, the converged flag,
#   the stability report (stable, per-trial stable/trivial) and every warning the flash logged
#   (stability non-convergence warnings are only reported that way).
# - `forced`: when the tight flash is single phase, a two-phase flash started from V0 = 0.05, 0.3
#   and 0.5 (which bypasses the stability test), and for each split (with its converged flag) the Gibbs energy
#   difference g_split - g_feed in units of RT and the fugacity-equality residual
#   max_i |ln f_i^L - ln f_i^V|. The single-phase Gibbs energy uses MultiComponentFlash's
#   own root selection.
# - `li_label`: the single-phase label from Li's correlation (0 = liquid, 1 = vapour).
include(joinpath(@__DIR__, "spe5_reference.jl"))
using Logging

struct CollectingLogger <: AbstractLogger
    messages::Vector{String}
end
Logging.min_enabled_level(::CollectingLogger) = Logging.Warn
Logging.shouldlog(::CollectingLogger, args...) = true
Logging.catch_exceptions(::CollectingLogger) = true
function Logging.handle_message(l::CollectingLogger, level, message, args...; kwargs...)
    push!(l.messages, string(message))
end

finite_or_nothing(v) = (v isa Real && isfinite(v)) ? v : nothing

function run_flash(cond, method, tolerance, maxiter; V0 = NaN)
    logger = CollectingLogger(String[])
    K = MCF.initial_guess_K(EOS, cond)
    out = with_logger(logger) do
        try
            MCF.flash_2ph(EOS, cond, K, V0; method = method, tolerance = tolerance,
                maxiter = maxiter, extra_out = true)
        catch err
            (NaN, K, (its = 0, converged = false, error = sprint(showerror, err)))
        end
    end
    V, K, rep = out
    stability = haskey(rep, :stability) ? rep.stability : nothing
    record = Dict{String, Any}(
        "V" => (isfinite(V) && 0 < V < 1) ? V : nothing,
        "iterations" => rep.its,
        "converged" => rep.converged,
        "warnings" => logger.messages,
    )
    haskey(rep, :error) && (record["error"] = rep.error)
    if !isnothing(stability)
        record["stability"] = Dict("stable" => stability.stable,
            "liquid_stable" => stability.liquid.stable, "liquid_trivial" => stability.liquid.trivial,
            "vapor_stable" => stability.vapor.stable, "vapor_trivial" => stability.vapor.trivial)
    end
    return record, V, K
end

gibbs(p, z) = sum(z .* log.(MCF.mixture_fugacities(EOS, (p = p, T = T_RES, z = z))))

function split_detail(p, z, V, K)
    x = z ./ (1 .+ V .* (K .- 1))
    y = K .* x
    x ./= sum(x); y ./= sum(y)
    fl = MCF.mixture_fugacities(EOS, (p = p, T = T_RES, z = x))
    fv = MCF.mixture_fugacities(EOS, (p = p, T = T_RES, z = y))
    g_split = (1 - V) * sum(x .* log.(fl)) + V * sum(y .* log.(fv))
    Dict("V" => V, "sum_ln_k_sq" => sum(log.(K) .^ 2),
        "gibbs_split_minus_feed_rt" => g_split - gibbs(p, z),
        "fugacity_residual" => maximum(abs.(log.(fl) .- log.(fv))))
end

function main(points_path, out_path)
    pts = JSON.parsefile(points_path)
    out = Dict[]
    for pt in pts
        p = Float64(pt["p_bar"]) * 1e5
        z = Float64.(pt["z"]); z ./= sum(z)
        cond = (p = p, T = T_RES, z = z)
        tight, V, K = run_flash(cond, MCF.SSINewtonFlash(), 1e-12, 200000)
        default, _, _ = run_flash(cond, MCF.SSIFlash(), 1e-8, 25000)
        rec = Dict{String, Any}("label" => pt["label"], "p_bar" => pt["p_bar"],
            "tight" => tight, "default" => default,
            "li_label" => MCF.single_phase_label(EOS.mixture, cond))
        if isnothing(tight["V"])
            forced = Dict[]
            for V0 in (0.05, 0.3, 0.5)
                f, Vf, Kf = run_flash(cond, MCF.SSINewtonFlash(), 1e-12, 200000; V0 = V0)
                f["V0"] = V0
                # Recorded with its converged flag even when the solve stalled: a stalled
                # near-critical split is evidence too, and its fugacity residual says how far off
                # its Gibbs difference may be.
                if !isnothing(f["V"]) && sum(log.(Kf) .^ 2) > 1e-10
                    merge!(f, split_detail(p, z, Vf, Kf))
                end
                push!(forced, f)
            end
            rec["forced"] = forced
        else
            rec["split"] = split_detail(p, z, V, K)
        end
        push!(out, rec)
    end
    open(out_path, "w") do io
        JSON.print(io, Dict("schema" => "ressim-spe5-jutul-cross-flash/1",
            "jutuldarcy" => string(pkgversion(JutulDarcy)),
            "temperature_k" => T_RES, "points" => out), 1)
    end
end

if abspath(PROGRAM_FILE) == @__FILE__
    main(ARGS[1], ARGS[2])
end
