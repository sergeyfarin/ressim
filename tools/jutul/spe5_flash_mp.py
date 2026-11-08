"""High-precision two-phase PR flash at failure B's near-critical cells (#116).

    uv run --no-project --with mpmath python -I tools/jutul/spe5_flash_mp.py CONDITIONING.json OUT.json [STRIDE]

CONDITIONING.json is written by the ignored ResSim test `comp_spe5_failure_b_flash_conditioning`
(`SPE5_TRACE_OUT=DIR`, file `failure_b_flash_conditioning.json`). It carries the fluid exactly as
ResSim holds it (critical properties, acentric factors, interactions, the PR/PRCORR constants) and,
for cells 33, 39 and 40, the exact (p, z) and ResSim's split at 64 consecutive pressure ulps.

This is an independent implementation of the same problem, written from the textbook PR
equations, not from ResSim's code, and solved in 50-digit arithmetic:

    unknowns  ln K_i (i = 1..n) and beta (vapour mole fraction)
    x_i = z_i / (1 + beta (K_i - 1)),  y_i = K_i x_i
    F_i     = ln K_i + ln phi_i^V(y) - ln phi_i^L(x) = 0
    F_{n+1} = sum_i z_i (K_i - 1) / (1 + beta (K_i - 1)) = 0     (Rachford-Rice)

Newton with a central-difference Jacobian (h = 1e-30), started from ResSim's own split, iterated
until max |F| < 1e-40. The liquid takes the smallest cubic root above B, the vapour the largest.
Every STRIDE-th ulp point is solved (default 4).

The question: does the converged split move smoothly with pressure at these states (then the
1e-5-level scatter in ResSim's beta across ulps is an implementation property), or is it itself
that sensitive (then it is the problem's conditioning)? Output: per point, beta, x, y, Z_L, Z_V,
molar volumes and the vapour volume fraction to full precision, and the spread over the sweep.
"""
import json
import sys

import mpmath as mp

mp.mp.dps = 50


def setup(fluid, temperature):
    comps = fluid["components"]
    n = len(comps)
    corrected = fluid["eos"] == "PengRobinsonCorrected"
    T = mp.mpf(temperature)
    a_i, b_i = [], []
    for c in comps:
        w = mp.mpf(c["acentric_factor"])
        tc, pc = mp.mpf(c["critical_temperature_k"]), mp.mpf(c["critical_pressure_pa"])
        if corrected and c["acentric_factor"] > fluid["corrected_above_acentric"]:
            f = mp.mpf("0.379642") + w * (mp.mpf("1.48503") + w * (mp.mpf("-0.164423")
                                                                + w * mp.mpf("0.016666")))
        else:
            f = mp.mpf("0.37464") + w * (mp.mpf("1.54226") + w * mp.mpf("-0.26992"))
        tr = T / tc
        omega_a = mp.mpf(fluid["omega_a"]) * (1 + f * (1 - mp.sqrt(tr))) ** 2
        # Per unit pressure: A_i = Omega_a p_r / T_r^2, B_i = Omega_b p_r / T_r.
        a_i.append(omega_a / (pc * tr * tr))
        b_i.append(mp.mpf(fluid["omega_b"]) / (pc * tr))
    k = [[mp.mpf(v) for v in row] for row in fluid["interaction"]]
    a_ij = [[mp.sqrt(a_i[i] * a_i[j]) * (1 - k[i][j]) for j in range(n)] for i in range(n)]
    return {"n": n, "a_ij": a_ij, "b_i": b_i, "T": T,
            "R": mp.mpf(fluid["gas_constant_j_per_mol_k"])}


def cubic_roots(A, B):
    # Z^3 - (1 - B) Z^2 + (A - 3B^2 - 2B) Z - (AB - B^2 - B^3) = 0
    roots = mp.polyroots([1, -(1 - B), A - 3 * B * B - 2 * B, -(A * B - B * B - B ** 3)],
                         maxsteps=200, extraprec=200)
    real = sorted(mp.re(r) for r in roots if abs(mp.im(r)) < mp.mpf(10) ** -30)
    return [r for r in real if r > B]


def ln_phi(eos, x, p, phase):
    n = eos["n"]
    Ai = [sum(x[j] * eos["a_ij"][i][j] for j in range(n)) * p for i in range(n)]
    A = sum(x[i] * Ai[i] for i in range(n))
    Bi = [b * p for b in eos["b_i"]]
    B = sum(x[i] * Bi[i] for i in range(n))
    roots = cubic_roots(A, B)
    Z = roots[0] if phase == "liquid" else roots[-1]
    s2 = mp.sqrt(2)
    log_term = mp.log((Z + (1 + s2) * B) / (Z + (1 - s2) * B))
    out = [Bi[i] / B * (Z - 1) - mp.log(Z - B)
           - A / (2 * s2 * B) * (2 * Ai[i] / A - Bi[i] / B) * log_term for i in range(n)]
    return out, Z


def residual(eos, z, p, u):
    n = eos["n"]
    lnk, beta = u[:n], u[n]
    K = [mp.exp(v) for v in lnk]
    d = [1 + beta * (K[i] - 1) for i in range(n)]
    x = [z[i] / d[i] for i in range(n)]
    y = [K[i] * x[i] for i in range(n)]
    lpl, zl = ln_phi(eos, x, p, "liquid")
    lpv, zv = ln_phi(eos, y, p, "vapour")
    F = [lnk[i] + lpv[i] - lpl[i] for i in range(n)]
    F.append(sum(z[i] * (K[i] - 1) / d[i] for i in range(n)))
    return F, {"x": x, "y": y, "Z_L": zl, "Z_V": zv, "beta": beta}


def solve(eos, z, p, u0):
    u = list(u0)
    h = mp.mpf(10) ** -30
    for it in range(60):
        F, state = residual(eos, z, p, u)
        norm = max(abs(f) for f in F)
        if norm < mp.mpf(10) ** -40:
            return u, state, norm, it
        m = len(u)
        J = mp.matrix(m, m)
        for j in range(m):
            up, dn = list(u), list(u)
            up[j] += h
            dn[j] -= h
            Fp, _ = residual(eos, z, p, up)
            Fm, _ = residual(eos, z, p, dn)
            for i in range(m):
                J[i, j] = (Fp[i] - Fm[i]) / (2 * h)
        du = mp.lu_solve(J, mp.matrix([-f for f in F]))
        u = [u[i] + du[i] for i in range(m)]
    raise RuntimeError(f"no convergence at p={p}: |F|={norm}")


def main(in_path, out_path, stride=4):
    data = json.load(open(in_path))
    fluid = data["fluid"]
    out = []
    for cell in data["cells"]:
        eos = setup(fluid, cell["temperature_k"])
        n = eos["n"]
        z = [mp.mpf(v) for v in cell["z"]]
        total = sum(z)
        z = [v / total for v in z]
        shifts = [mp.mpf(v) for v in fluid["volume_shifts"]] if fluid["volume_shifts"] else None
        points = []
        for k, pt in enumerate(cell["ulp_sweep"]["points"]):
            if k % stride:
                continue
            p = mp.mpf(pt["p_pa"])
            K0 = [mp.mpf(yv) / mp.mpf(xv) for xv, yv in zip(pt["x"], pt["y"])]
            u0 = [mp.log(v) for v in K0] + [mp.mpf(pt["beta"])]
            u, st, norm, its = solve(eos, z, p, u0)
            vl = st["Z_L"] * eos["R"] * eos["T"] / p
            vv = st["Z_V"] * eos["R"] * eos["T"] / p
            if shifts:
                raise NotImplementedError("volume shifts are zero for SPE5; not handled here")
            beta = st["beta"]
            svf = beta * vv / (beta * vv + (1 - beta) * vl)
            points.append({
                "p_pa": pt["p_pa"], "ulp_index": k, "newton_iterations": its,
                "residual": mp.nstr(norm, 5),
                "beta": mp.nstr(beta, 30), "x": [mp.nstr(v, 25) for v in st["x"]],
                "y": [mp.nstr(v, 25) for v in st["y"]],
                "Z_L": mp.nstr(st["Z_L"], 25), "Z_V": mp.nstr(st["Z_V"], 25),
                "vapour_volume_fraction": mp.nstr(svf, 25),
                "sum_ln_k_sq": mp.nstr(sum(v * v for v in u[:n]), 10),
                "ressim_beta": pt["beta"],
                "ressim_minus_mp_beta": float(mp.mpf(pt["beta"]) - beta),
                "ressim_minus_mp_x_max": float(max(abs(mp.mpf(a) - b)
                                                   for a, b in zip(pt["x"], st["x"]))),
            })
        betas = [mp.mpf(q["beta"]) for q in points]
        rb = [q["ressim_beta"] for q in points]
        summary = {
            "cell": cell["cell"], "points": len(points),
            "mp_beta_spread": mp.nstr(max(betas) - min(betas), 5),
            "ressim_beta_spread": max(rb) - min(rb),
            "max_abs_ressim_minus_mp_beta": max(abs(q["ressim_minus_mp_beta"]) for q in points),
            "max_abs_ressim_minus_mp_x": max(q["ressim_minus_mp_x_max"] for q in points),
        }
        print(json.dumps(summary))
        out.append({"summary": summary, "points": points})
    json.dump({"schema": "ressim-spe5-flash-mp/1", "digits": mp.mp.dps, "cells": out},
              open(out_path, "w"), indent=1)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 4)
