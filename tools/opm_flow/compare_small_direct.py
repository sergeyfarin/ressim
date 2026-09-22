#!/usr/bin/env python3
"""Compare ResSim FIM against OPM Flow on the small-direct decks (every case under 512 rows).

The decks and the ResSim runs come from one source,
`src/lib/ressim/src/tests/opm_small_direct.rs`, so the inputs cannot silently disagree.
This script runs Flow, reads its binary output through `opm.io` (the system package shipped with
Flow, which is why it runs with `python3` rather than inside the uv project), and reports:

* accuracy: cell pressure, Sw and Sg against Flow at every report step, and cumulative
  production at the end;
* convergence: accepted substeps, Newton iterations and cuts, per simulator;
* backend agreement: ResSim sparse against ResSim dense.

    python3 tools/opm_flow/compare_small_direct.py --ressim-dir /tmp/small-direct \\
        --flow-dir /tmp/small-direct-flow [--case bo-1d-10] [--json out.json]

`--ressim-dir` must hold `<case>.<backend>.json` from `opm_small_direct_run_ressim`.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import time
from pathlib import Path

import numpy as np
from opm.io.ecl import ERst, ESmry

REPO = Path(__file__).resolve().parents[2]
DECKS = REPO / "opm" / "reference-decks" / "small-direct"
BACKENDS = ("sparse", "dense")


def run_flow(case: str, out: Path) -> float:
    out.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    subprocess.run(
        [
            "flow",
            str(DECKS / case / "CASE.DATA"),
            f"--output-dir={out}",
            "--enable-gravity=false",
            "--output-extra-convergence-info=steps,iterations",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return (time.perf_counter() - started) * 1e3


def flow_convergence(out: Path) -> dict:
    rows = [line.split() for line in (out / "CASE.INFOSTEP").read_text().splitlines()[1:] if line.strip()]
    # Columns: Time TStep Assembly LSetup LSolve LocSol Update Output WellIt Lins NewtIt LinIt Conv
    accepted = [r for r in rows if r[-1] == "1"]
    return {
        "substeps": len(accepted),
        "newton": sum(int(r[-3]) for r in accepted),
        "cuts": len(rows) - len(accepted),
        "cut_newton": sum(int(r[-3]) for r in rows if r[-1] != "1"),
    }


def flow_fields(out: Path) -> list[dict]:
    rst = ERst(str(out / "CASE.UNRST"))
    fields = []
    for step in rst.report_steps[1:]:
        entry = {"p": np.array(rst["PRESSURE", step]), "sw": np.array(rst["SWAT", step])}
        try:
            entry["sg"] = np.array(rst["SGAS", step])
        except Exception:  # noqa: BLE001 - oil-water decks have no SGAS
            entry["sg"] = np.zeros_like(entry["p"])
        fields.append(entry)
    return fields


def flow_cumulatives(out: Path) -> dict:
    smry = ESmry(str(out / "CASE.SMSPEC"))
    keys = set(smry.keys())
    return {k: float(smry[k][-1]) for k in ("FOPT", "FWPT", "FWIT", "FGPT") if k in keys}


def ressim_cumulatives(run: dict) -> dict:
    hist = np.array(run["history"])
    t = np.concatenate([[0.0], hist[:, 0]])
    dt = np.diff(t)
    return {
        "FOPT": float(np.sum(hist[:, 1] * dt)),
        "FWPT": float(np.sum(hist[:, 2] * dt)),
        "FGPT": float(np.sum(hist[:, 3] * dt)),
        "FWIT": float(np.sum(hist[:, 4] * dt)),
    }


def field_error(a: list[dict], b: list[dict]) -> dict:
    """Worst cell difference over all report steps, and at the final one."""
    worst = {"p": 0.0, "sw": 0.0, "sg": 0.0}
    for fa, fb in zip(a, b):
        for key in worst:
            worst[key] = max(worst[key], float(np.max(np.abs(fa[key] - fb[key]))))
    final = {key: float(np.max(np.abs(a[-1][key] - b[-1][key]))) for key in worst}
    return {"worst": worst, "final": final}


def rel(a: float, b: float) -> float:
    return abs(a - b) / max(abs(b), 1e-12)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ressim-dir", required=True, type=Path)
    parser.add_argument("--flow-dir", required=True, type=Path)
    parser.add_argument("--case", action="append")
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    cases = args.case or sorted(p.name for p in DECKS.iterdir() if (p / "CASE.DATA").exists())
    report = {}
    for case in cases:
        out = args.flow_dir / case
        flow_ms = run_flow(case, out)
        flow = {
            "fields": flow_fields(out),
            "cum": flow_cumulatives(out),
            "conv": flow_convergence(out),
            "wall_ms": flow_ms,
        }
        entry = {"flow": {"conv": flow["conv"], "wall_ms": flow_ms, "cum": flow["cum"]}}
        runs = {}
        for backend in BACKENDS:
            path = args.ressim_dir / f"{case}.{backend}.json"
            if not path.exists():
                continue
            run = json.loads(path.read_text())
            fields = [
                {k: np.array(r[k]) for k in ("p", "sw", "sg")} for r in run["reports"]
            ]
            if len(fields) != len(flow["fields"]):
                raise SystemExit(f"{case}: {len(fields)} ResSim reports vs {len(flow['fields'])} Flow")
            runs[backend] = fields
            cum = ressim_cumulatives(run)
            entry[backend] = {
                "conv": {
                    "substeps": sum(r["substeps"] for r in run["reports"]),
                    "newton": sum(r["newton"] for r in run["reports"]),
                    "retries": sum(r["retries"] for r in run["reports"]),
                    "retry_newton": sum(r["retry_newton"] for r in run["reports"]),
                },
                "wall_ms": run["wall_ms"],
                "vs_flow": field_error(fields, flow["fields"]),
                # A cumulative below 0.1% of the case's largest is a trace volume (immobile
                # connate water, pre-breakthrough water): a percentage of it means nothing.
                "cum_rel_vs_flow": {
                    k: rel(cum[k], v)
                    for k, v in flow["cum"].items()
                    if abs(v) > 1e-3 * max(abs(x) for x in flow["cum"].values())
                },
            }
        if len(runs) == 2:
            entry["sparse_vs_dense"] = field_error(runs["sparse"], runs["dense"])
        report[case] = entry

    for case, e in report.items():
        f = e["flow"]["conv"]
        print(f"\n{case}")
        print(f"  {'':8} {'substeps':>9} {'newton':>8} {'cuts/retries':>13} {'wall ms':>9}"
              f" {'max|dp| bar':>12} {'max|dSw|':>9} {'max|dSg|':>9}  cumulative rel. diff vs Flow")
        print(f"  {'flow':8} {f['substeps']:>9} {f['newton']:>8} {f['cuts']:>13} {e['flow']['wall_ms']:>9.1f}")
        for backend in BACKENDS:
            if backend not in e:
                continue
            b = e[backend]
            w = b["vs_flow"]["worst"]
            cum = "  ".join(f"{k} {v:.2%}" for k, v in b["cum_rel_vs_flow"].items())
            print(f"  {backend:8} {b['conv']['substeps']:>9} {b['conv']['newton']:>8} "
                  f"{b['conv']['retries']:>13} {b['wall_ms']:>9.1f} {w['p']:>12.4f} {w['sw']:>9.5f} "
                  f"{w['sg']:>9.5f}  {cum}")
        if "sparse_vs_dense" in e:
            w = e["sparse_vs_dense"]["worst"]
            print(f"  sparse vs dense, worst over reports: |dp| {w['p']:.2e} bar  |dSw| {w['sw']:.2e}"
                  f"  |dSg| {w['sg']:.2e}")
    if args.json:
        args.json.write_text(json.dumps(report, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
