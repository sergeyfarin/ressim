"""50-digit Gibbs comparison at the #116 M0 states.

    uv run --no-project --with mpmath python -I tools/jutul/spe5_m0_gibbs_mp.py POINTS.json OUT.json

POINTS.json is written by the ignored ResSim test `comp_spe5_m0_branch_comparison`
(`SPE5_TRACE_OUT=DIR`, file `m0_gibbs_points.json`): the fluid as ResSim holds it and, for each
flipping-cell state the measurement evaluated, the exact (p, T, z), the label Li's rule gives, the
Float64 feed Gibbs energy on each cubic root, and a two-phase split (ResSim's own where the standard
flash is two-phase, else the seeded split) with its Float64 Gibbs energy.

At each point, in 50-digit arithmetic (the EOS from `spe5_flash_mp.py`, written from the textbook
Peng-Robinson equations, not from ResSim's code):

- every real cubic root of the feed above B, and the feed's g / (R T) on each;
- the two-phase split solved by Newton from the given seed, and its g / (R T);
- g_split - g_feed against the root Li's rule picks and against the lower root.

A seed that does not converge, converges to the trivial solution, or leaves 0 < beta < 1 is
reported as such: it is not evidence that no split exists. The table compares the 50-digit
g_split - g_feed with ResSim's Float64 value, to calibrate the Float64 comparison at these states.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import mpmath as mp  # noqa: E402

from spe5_flash_mp import cubic_roots, setup, solve  # noqa: E402

mp.mp.dps = 50


def ln_phi_at(eos, x, p, Z):
    n = eos["n"]
    Ai = [sum(x[j] * eos["a_ij"][i][j] for j in range(n)) * p for i in range(n)]
    A = sum(x[i] * Ai[i] for i in range(n))
    Bi = [b * p for b in eos["b_i"]]
    B = sum(x[i] * Bi[i] for i in range(n))
    s2 = mp.sqrt(2)
    log_term = mp.log((Z + (1 + s2) * B) / (Z + (1 - s2) * B))
    return [Bi[i] / B * (Z - 1) - mp.log(Z - B)
            - A / (2 * s2 * B) * (2 * Ai[i] / A - Bi[i] / B) * log_term for i in range(n)]


def feed_roots(eos, z, p):
    n = eos["n"]
    A = sum(z[i] * z[j] * eos["a_ij"][i][j] for i in range(n) for j in range(n)) * p
    B = sum(z[i] * eos["b_i"][i] for i in range(n)) * p
    return cubic_roots(A, B)


def gibbs(x, lnphi):
    return sum(x[i] * (mp.log(x[i]) + lnphi[i]) for i in range(len(x)) if x[i] > 0)


def main(in_path, out_path):
    with open(in_path) as f:
        data = json.load(f)
    fluid = data["fluid"]
    if fluid["volume_shifts"] and any(fluid["volume_shifts"]):
        raise NotImplementedError("volume shifts are zero for SPE5; not handled here")
    out = []
    print("| Point | Cell | Standard | Li | Roots | 50-digit g_split − g_feed(Li root) | Float64 | "
          "50-digit g_split − g_feed(lower root) | Split |")
    print("|---|---|---|---|---|---|---|---|---|")
    for pt in data["points"]:
        eos = setup(fluid, pt["temperature_k"])
        n = eos["n"]
        p = mp.mpf(pt["p_pa"])
        z = [mp.mpf(v) for v in pt["z"]]
        total = sum(z)
        z = [v / total for v in z]
        roots = feed_roots(eos, z, p)
        g_roots = [gibbs(z, ln_phi_at(eos, z, p, Z)) for Z in roots]
        g_li = g_roots[0] if pt["li_liquid"] else g_roots[-1]
        g_low = min(g_roots)
        rec = {"tag": pt["tag"], "cell": pt["cell"], "standard_label": pt["standard_label"],
               "li_liquid": pt["li_liquid"], "roots": [mp.nstr(r, 20) for r in roots],
               "g_feed_roots": [mp.nstr(g, 25) for g in g_roots],
               "float64_g_split_minus_li_feed": pt["g_split_minus_li_feed"]}
        split = "no seed"
        dg_li = dg_low = None
        seed = pt.get("split_seed")
        if seed:
            u0 = [mp.mpf(v) for v in seed["ln_k"]] + [mp.mpf(seed["beta"])]
            try:
                u, st, norm, its = solve(eos, z, p, u0)
                beta = st["beta"]
                lnk_sq = sum(v * v for v in u[:n])
                if not (0 < beta < 1):
                    split = f"converged outside (0, 1): beta {mp.nstr(beta, 8)}"
                elif lnk_sq < mp.mpf(10) ** -20:
                    split = "converged to the trivial solution"
                else:
                    lpl = ln_phi_at(eos, st["x"], p, st["Z_L"])
                    lpv = ln_phi_at(eos, st["y"], p, st["Z_V"])
                    g_split = (1 - beta) * gibbs(st["x"], lpl) + beta * gibbs(st["y"], lpv)
                    dg_li, dg_low = g_split - g_li, g_split - g_low
                    split = (f"beta {mp.nstr(beta, 10)}, Σ ln²K {mp.nstr(lnk_sq, 4)}, "
                             f"{its} it")
                    rec.update({"beta": mp.nstr(beta, 25), "sum_ln_k_sq": mp.nstr(lnk_sq, 10),
                                "g_split": mp.nstr(g_split, 25),
                                "g_split_minus_li_feed": mp.nstr(dg_li, 10),
                                "g_split_minus_lower_feed": mp.nstr(dg_low, 10),
                                "ressim_beta_minus_mp": float(mp.mpf(seed["beta"]) - beta)})
            except RuntimeError as err:
                split = f"inconclusive: {err}"
        rec["split"] = split
        out.append(rec)
        f64 = pt["g_split_minus_li_feed"]
        print(f"| {pt['tag']} | {pt['cell']} | {pt['standard_label']} "
              f"| {'L' if pt['li_liquid'] else 'V'} | {len(roots)} "
              f"| {mp.nstr(dg_li, 4) if dg_li is not None else '—'} "
              f"| {f'{f64:.3e}' if f64 is not None else '—'} "
              f"| {mp.nstr(dg_low, 4) if dg_low is not None else '—'} | {split} |")
    with open(out_path, "w") as f:
        json.dump({"points": out}, f, indent=1)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
