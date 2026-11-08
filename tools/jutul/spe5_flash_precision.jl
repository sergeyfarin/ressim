# Independent check of ResSim's near-critical flash at failure B's cells (#116). Run from the
# repository root:
#
#   julia +1.12 --project=tools/jutul tools/jutul/spe5_flash_precision.jl CONDITIONING.json OUT.json [MP.json]
#
# CONDITIONING.json is written by the ignored ResSim test `comp_spe5_failure_b_flash_conditioning`
# (SPE5_TRACE_OUT=DIR, file failure_b_flash_conditioning.json): for cells 33, 39 and 40 of failure
# B's best iterate, the exact (p, z) and ResSim's full split at 64 consecutive pressure ulps.
#
# Every one of those inputs is flashed here with MultiComponentFlash, the flash JutulDarcy uses,
# which shares no code with ResSim: SSINewtonFlash at tolerance 1e-13, in Float64. For each point:
# V, x, y, phase Z factors and molar volumes, vapour volume fraction, the fugacity residual and the
# convergence report. Where MultiComponentFlash's own stability test calls the point single-phase
# (cells 33 and 39), the split is forced from ResSim's (K, beta) and flagged `forced`. Each is compared with ResSim's split at the same
# input and with the 50-digit solution of tools/jutul/spe5_flash_mp.py.
#
# Two further measurements separate "ResSim's flash is loose" from "this formulation cannot pin the
# split in Float64":
# - `starts`: forced splits from Wilson's K with V0 = 0.3 and 0.5, so MultiComponentFlash finds
#   its own answer instead of accepting ResSim's starting point;
# - `at_exact`: with MP.json (tools/jutul/spe5_flash_mp.py), the Float64 fugacity residual that
#   MultiComponentFlash evaluates at the 50-digit solution rounded to Float64, and at ResSim's split.
#   If both are at round-off, Float64 fugacity equality cannot tell the two splits apart.
#
# MultiComponentFlash cannot run in BigFloat: its flash storage is allocated as Float64
# (`zeros(n)` and Float64 ForwardDiff duals), so BigFloat values would be rounded into Float64
# buffers. The high-precision reference is therefore the separate mpmath solver.
include(joinpath(@__DIR__, "spe5_reference.jl"))

function split_record(cond, ressim; tolerance, maxiter = 200000, eos = EOS)
    K0 = MCF.initial_guess_K(eos, cond)
    V, K, rep = MCF.flash_2ph(eos, cond, K0; method = MCF.SSINewtonFlash(),
        tolerance = tolerance, maxiter = maxiter, extra_out = true)
    rec = Dict{String, Any}("converged" => rep.converged, "iterations" => rep.its,
        "forced" => false)
    if !(isfinite(V) && 0 < V < 1)
        # Single phase by MultiComponentFlash's stability test. Force the split from ResSim's own
        # (K, beta), which bypasses that test, as the cross-flash's forced splits do.
        rec["stability_says_single_phase"] = true
        Kr = Float64.(ressim["y"]) ./ Float64.(ressim["x"])
        V, K, rep = MCF.flash_2ph(eos, cond, Kr, Float64(ressim["beta"]);
            method = MCF.SSINewtonFlash(), tolerance = tolerance, maxiter = maxiter,
            extra_out = true)
        rec["forced"] = true
        rec["converged"] = rep.converged
        rec["iterations"] = rep.its
        if !(isfinite(V) && 0 < V < 1)
            rec["V"] = nothing
            return rec
        end
    end
    p, T, z = cond.p, cond.T, cond.z
    x = z ./ (1 .+ V .* (K .- 1))
    y = K .* x
    x ./= sum(x)
    y ./= sum(y)
    zfac(c, phase) = begin
        forces = MCF.force_coefficients(eos, c)
        scalars = MCF.force_scalars(eos, c, forces)
        MCF.mixture_compressibility_factor(eos, c, forces, scalars, phase)
    end
    Zl = zfac((p = p, T = T, z = x), :liquid)
    Zv = zfac((p = p, T = T, z = y), :vapor)
    vl = MCF.IDEAL_GAS_CONSTANT * T * Zl / p
    vv = MCF.IDEAL_GAS_CONSTANT * T * Zv / p
    lnfl = log.(MCF.mixture_fugacities(eos, (p = p, T = T, z = x)))
    lnfv = log.(MCF.mixture_fugacities(eos, (p = p, T = T, z = y)))
    merge!(rec, Dict(
        "V" => V, "x" => x, "y" => y, "Z_L" => Zl, "Z_V" => Zv,
        "liquid_molar_volume" => vl, "vapour_molar_volume" => vv,
        "vapour_volume_fraction" => V * vv / (V * vv + (1 - V) * vl),
        "fugacity_residual" => maximum(abs.(lnfl .- lnfv)),
        "sum_ln_k_sq" => sum(log.(K) .^ 2),
    ))
    return rec
end

function fugacity_residual(eos, p, T, x, y)
    lnfl = log.(MCF.mixture_fugacities(eos, (p = p, T = T, z = x)))
    lnfv = log.(MCF.mixture_fugacities(eos, (p = p, T = T, z = y)))
    maximum(abs.(lnfl .- lnfv))
end

function forced_from(cond, K0, V0; tolerance)
    V, K, rep = MCF.flash_2ph(EOS, cond, K0, V0; method = MCF.SSINewtonFlash(),
        tolerance = tolerance, maxiter = 200000, extra_out = true)
    ok = isfinite(V) && 0 < V < 1
    Dict{String, Any}("V0" => V0, "V" => ok ? V : nothing, "converged" => rep.converged,
        "iterations" => rep.its, "sum_ln_k_sq" => ok ? sum(log.(K) .^ 2) : nothing)
end

function main(in_path, out_path, mp_path = nothing)
    data = JSON.parsefile(in_path)
    exact = Dict{Tuple{Int, Float64}, Any}()
    if !isnothing(mp_path)
        for c in JSON.parsefile(mp_path)["cells"], q in c["points"]
            exact[(c["summary"]["cell"], Float64(q["p_pa"]))] = q
        end
    end
    cells = Dict[]
    for c in data["cells"]
        z = Float64.(c["z"])
        recs = Dict[]
        for pt in c["ulp_sweep"]["points"]
            p = Float64(pt["p_pa"])
            f64 = split_record((p = p, T = T_RES, z = copy(z)), pt; tolerance = 1e-13)
            cond = (p = p, T = T_RES, z = copy(z))
            wilson = MCF.initial_guess_K(EOS, cond)
            starts = [forced_from(cond, copy(wilson), V0; tolerance = 1e-13) for V0 in (0.3, 0.5)]
            rec = Dict{String, Any}("p_pa" => p, "ressim" => pt, "float64" => f64,
                "starts" => starts,
                "ressim_fugacity_residual_by_mcf" => fugacity_residual(EOS, p, T_RES,
                    Float64.(pt["x"]), Float64.(pt["y"])))
            key = (Int(c["cell"]), p)
            if haskey(exact, key)
                q = exact[key]
                xe = parse.(Float64, q["x"])
                ye = parse.(Float64, q["y"])
                rec["at_exact"] = Dict("beta" => parse(Float64, q["beta"]),
                    "fugacity_residual_by_mcf" => fugacity_residual(EOS, p, T_RES, xe ./ sum(xe),
                        ye ./ sum(ye)))
            end
            push!(recs, rec)
        end
        spread(key, side) = begin
            v = [r[side][key] for r in recs if r[side]["V"] !== nothing]
            isempty(v) ? nothing : maximum(v) - minimum(v)
        end
        summary = Dict{String, Any}(side => Dict{String, Any}(k => spread(k, side) for k in
                ("V", "vapour_volume_fraction", "Z_L", "Z_V"))
            for side in ("float64",))
        for side in ("float64",)
            summary[side]["two_phase_points"] = count(r -> r[side]["V"] !== nothing, recs)
            summary[side]["converged_points"] = count(r -> r[side]["converged"] == true, recs)
            summary[side]["forced_points"] = count(r -> r[side]["forced"] == true, recs)
        end
        rb = [r["ressim"]["beta"] for r in recs]
        summary["ressim"] = Dict{String, Any}("V" => maximum(rb) - minimum(rb))
        println("cell $(c["cell"]): V spread over 64 ulps  ResSim $(summary["ressim"]["V"])  " *
            "MCF Float64 $(summary["float64"]["V"])")
        push!(cells, Dict("cell" => c["cell"], "pressure_bar" => c["pressure_bar"], "z" => z,
            "spread_over_ulps" => summary, "points" => recs))
    end
    open(out_path, "w") do io
        JSON.print(io, Dict("schema" => "ressim-spe5-flash-precision/1",
            "jutuldarcy" => string(pkgversion(JutulDarcy)),
            "multicomponentflash" => string(pkgversion(MCF)),
            "cells" => cells), 1)
    end
end

if abspath(PROGRAM_FILE) == @__FILE__
    main(ARGS[1], ARGS[2], length(ARGS) >= 3 ? ARGS[3] : nothing)
end
