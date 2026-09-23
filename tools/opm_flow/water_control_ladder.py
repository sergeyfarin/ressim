#!/usr/bin/env python3
"""Cumulative oil and injection, ResSim vs OPM Flow, on a report-step ladder (#21).

Runs the tracked `opm/reference-decks/water-pressure-<grid>` decks through Flow with only `TSTEP`
rewritten, and the matching `scripts/fim-wasm-diagnostic.mjs --preset water-pressure` run through
the shipped wasm engine, integrating ResSim's rate history (rate x accepted substep) rather than
multiplying an end-of-step rate by the report step. The latter is what produced the historical
"8-10%" oil gap.

    python3 tools/opm_flow/water_control_ladder.py '[["23x23x1", 0.25, [0.25, 0.05, 0.01]]]'
    RS_EXTRA="--corey-table-points 9" python3 tools/opm_flow/water_control_ladder.py ...

`RS_EXTRA` passes extra diagnostic flags; `--corey-table-points 9` makes ResSim evaluate the same
9-knot SWOF the decks carry. Outputs go under `$LADDER_OUT` (default /tmp/water-control-ladder).
Uses `opm.io`, the Python package shipped with Flow, so it runs with `python3`, not uv.
"""
import json, re, subprocess, sys, os, time
from pathlib import Path
from opm.io.ecl import ESmry

REPO = Path(__file__).resolve().parents[2]
EXTRA = os.environ.get("RS_EXTRA", "").split()
OUT = Path(os.environ.get("LADDER_OUT", "/tmp/water-control-ladder")) / os.environ.get("RS_TAG", "default")
OUT.mkdir(parents=True, exist_ok=True)

def ressim(grid, steps, dt):
    ck = OUT / f"rs_{grid}_{steps}x{dt}.json"
    t0 = time.perf_counter()
    subprocess.run(["node", "scripts/fim-wasm-diagnostic.mjs", "--preset", "water-pressure", "--grid", grid,
                    "--steps", str(steps), "--dt", str(dt), "--diagnostic", "summary", "--no-json",
                    "--checkpoint-out", str(ck)] + EXTRA, cwd=REPO, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    wall = time.perf_counter() - t0
    hist = json.loads(ck.read_text())["rateHistory"]
    t_prev, oil, inj = 0.0, 0.0, 0.0
    for r in hist:
        h = r["time"] - t_prev
        oil += r["total_production_oil"] * h
        inj += r["total_injection"] * h
        t_prev = r["time"]
    return {"FOPT": oil, "FWIT": inj, "substeps": len(hist), "t": t_prev, "wall": wall}

def flow(grid, steps, dt):
    deck = (REPO / f"opm/reference-decks/water-pressure-{grid}/CASE.DATA").read_text()
    deck = re.sub(r"TSTEP\s*\n\s*[^/]*/", f"TSTEP\n   {steps}*{dt} /", deck, count=1)
    d = OUT / f"flow_{grid}_{steps}x{dt}"
    d.mkdir(exist_ok=True)
    (d / "CASE.DATA").write_text(deck)
    subprocess.run(["flow", "CASE.DATA", "--enable-gravity=false", "--output-extra-convergence-info=steps,iterations"],
                   cwd=d, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    s = ESmry(str(d / "CASE.SMSPEC"))
    rows = [l.split() for l in (d / "CASE.INFOSTEP").read_text().splitlines()[1:] if l.strip()]
    return {"FOPT": float(s["FOPT"][-1]), "FWIT": float(s["FWIT"][-1]) if "FWIT" in s.keys() else float("nan"),
            "substeps": sum(1 for r in rows if r[-1] == "1"), "t": float(s["TIME"][-1])}

ladders = json.loads(sys.argv[1])  # [[grid, T, [dt,...]], ...]
results = []
for grid, T, dts in ladders:
    for dt in dts:
        steps = round(T / dt)
        r, f = ressim(grid, steps, dt), flow(grid, steps, dt)
        results.append({"grid": grid, "T": T, "dt": dt, "ressim": r, "flow": f})
        print(f"{grid:9} T={T:<5} {steps:4d}x{dt:<6} ResSim FOPT {r['FOPT']:10.3f} ({r['substeps']:4d} sub, t={r['t']:.3f})   "
              f"Flow FOPT {f['FOPT']:10.3f} ({f['substeps']:4d} sub)   diff {100*(r['FOPT']/f['FOPT']-1):+7.3f}%   "
              f"inj diff {100*(r['FWIT']/f['FWIT']-1):+7.3f}%", flush=True)
(OUT / f"results_{int(time.time())}.json").write_text(json.dumps(results, indent=1))
