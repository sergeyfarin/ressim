"""Compare ResSim's flash, unrefined and refined, with the 50-digit solution (#116).

    python3 -I tools/jutul/spe5_flash_refinement_compare.py STATES.json MP.json

STATES.json is written by the ignored ResSim test `comp_spe5_flash_refinement_states`
(`SPE5_TRACE_OUT=DIR`, file `flash_refinement_states.json`); MP.json by
`tools/jutul/spe5_flash_mp.py STATES.json MP.json 1`. Each sweep (cell x kind) is one row per
mode. Prints markdown:

- the largest error against the 50-digit split of beta, of x and y, of the two molar volumes
  (relative) and of the vapour volume fraction, and the mean beta error (a systematic offset);
- mobility scatter, for which no 50-digit value exists: over the `pressure_ulps` and
  `composition` sweeps the exact state moves by < 3e-10 in beta, so the relative spread of each
  phase mobility is its noise; over `pressure_0.01pa` the exact state moves smoothly, and the
  largest relative deviation from a straight-line fit in pressure is reported instead;
- the residual measures at the returned split.
"""
import json
import sys


def linear_fit_deviation(xs, ys):
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx if sxx else 0.0
    return max(abs(y - (my + slope * (x - mx))) for x, y in zip(xs, ys))


def main(states_path, mp_path):
    states = json.load(open(states_path))
    exact = json.load(open(mp_path))["cells"]
    gas = states["fluid"]["gas_constant_j_per_mol_k"]
    print("| Cell | Sweep | Mode | max \\|Δβ\\| | mean Δβ | max \\|Δx\\|, \\|Δy\\| | max rel. Δv_L, Δv_V "
          "| max \\|ΔS_V\\| | mobility noise L / V (rel.) | difference residual | standard residual |")
    print("|---|---|---|---|---|---|---|---|---|---|---|")
    for sweep, ex in zip(states["cells"], exact):
        assert sweep["cell"] == ex["summary"]["cell"] and sweep["kind"] == ex["summary"]["kind"]
        t = sweep["temperature_k"]
        points = sweep["ulp_sweep"]["points"]
        assert len(points) == len(ex["points"])
        for mode in ("unrefined", "refined"):
            db, dxy, dv, ds, dres, sres = [], [], [], [], [], []
            mob_l, mob_v, ps = [], [], []
            for pt, q in zip(points, ex["points"]):
                r = pt[mode]
                if r.get("error") or r["phase"] != "TwoPhase":
                    continue
                p = pt["p_pa"]
                db.append(r["beta"] - float(q["beta"]))
                dxy.append(max(max(abs(a - float(b)) for a, b in zip(r["x"], q["x"])),
                               max(abs(a - float(b)) for a, b in zip(r["y"], q["y"]))))
                vl = float(q["Z_L"]) * gas * t / p
                vv = float(q["Z_V"]) * gas * t / p
                dv.append(max(abs(r["liquid_molar_volume"] - vl) / vl,
                              abs(r["vapour_molar_volume"] - vv) / vv))
                ds.append(abs(r["vapour_volume_fraction"] - float(q["vapour_volume_fraction"])))
                dres.append(r["difference_residual"])
                sres.append(r["equilibrium_residual"])
                mob_l.append(r["liquid_mobility"])
                mob_v.append(r["vapour_mobility"])
                ps.append(p)
            if not db:
                print(f"| {sweep['cell']} | {sweep['kind']} | {mode} | no two-phase points |")
                continue

            def noise(m):
                mean = sum(m) / len(m)
                if sweep["kind"] == "pressure_0.01pa":
                    return linear_fit_deviation(ps, m) / abs(mean)
                return (max(m) - min(m)) / abs(mean)
            print(f"| {sweep['cell']} | {sweep['kind']} | {mode} "
                  f"| {max(abs(v) for v in db):.2e} | {sum(db) / len(db):+.1e} | {max(dxy):.1e} "
                  f"| {max(dv):.1e} | {max(ds):.1e} | {noise(mob_l):.1e} / {noise(mob_v):.1e} "
                  f"| {max(dres):.1e} | {max(sres):.1e} |")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
