#!/usr/bin/env python3
"""Compare every ResSim solver against OPM Flow, and against each other, on the small-direct decks.

The decks and the ResSim runs come from one source,
`src/lib/ressim/src/tests/opm_small_direct.rs`, so the inputs cannot silently disagree.
This script runs Flow, reads its binary output through `opm.io` (the system package shipped with
Flow, which is why it runs with `python3` rather than inside the uv project), and reports:

* accuracy: cell pressure, Sw and Sg against Flow at every report step, and cumulative
  production at the end, for every ResSim run present (FIM sparse, FIM dense, IMPES);
* convergence: accepted substeps, Newton iterations and cuts, per simulator;
* agreement between ResSim runs: sparse against dense (an invariant: same scheme, two LUs) and
  FIM against IMPES (the time-discretization gap, informational).

    python3 tools/opm_flow/compare_small_direct.py --ressim-dir /tmp/small-direct \\
        --flow-dir /tmp/small-direct-flow [--case bo-1d-10] [--json out.json] [--markdown]

A committed scorecard turns the comparison into a ratchet:

    ... --scorecard opm/reference-decks/small-direct/scorecard.json --check
    ... --scorecard opm/reference-decks/small-direct/scorecard.json --write-scorecard

`--check` fails when an accuracy or work metric regresses past its band (see `BANDS`), when a run
the scorecard covers is missing, or when a run raised a solver warning the scorecard did not
record. It reports improvements past the same band, so a better baseline is written deliberately
rather than left unrecorded. `scripts/validate-cross-solver.sh` wraps the whole pipeline.

`--ressim-dir` must hold `<case>.<run>.json` from `opm_small_direct_run_ressim`, where `<run>` is
`sparse` / `dense` (FIM, per small-system LU) or `impes`.
"""
from __future__ import annotations

import argparse
import datetime
import json
import subprocess
import sys
import time
from pathlib import Path

import numpy as np
from opm.io.ecl import ERst, ESmry

REPO = Path(__file__).resolve().parents[2]
DECKS = REPO / "opm" / "reference-decks" / "small-direct"
# Report order. Any other `<case>.<run>.json` found is reported after these.
KNOWN_RUNS = ("sparse", "dense", "impes")
LABELS = {"sparse": "FIM sparse", "dense": "FIM dense", "impes": "IMPES"}
# Pairs of ResSim runs compared cell by cell. `invariant` pairs must stay at their scorecard
# agreement; the others show a physical gap and are reported, not gated.
PAIRS = (("sparse", "dense", True), ("sparse", "impes", False))
CUM_KEYS = ("FOPT", "FWPT", "FWIT", "FGPT", "FGIT")

# Regression bands for `--check`: a metric fails when `new > base * (1 + rel) + abs`, and counts
# as an improvement when `new < base * (1 - rel) - abs`. The absolute floor keeps a near-zero
# baseline (sparse-vs-dense at 1e-10 bar, a cumulative at 0.00%) from turning roundoff into a
# failure; the relative part lets a larger baseline move a little without a rewrite. Wall time is
# never gated: single observations on a shared machine are noise.
BANDS = {
    "p": (0.25, 0.05),  # bar
    "sw": (0.25, 0.002),
    "sg": (0.25, 0.002),
    "cum": (0.25, 0.001),  # relative difference, i.e. 0.1 percentage point
    "substeps": (0.10, 2),
    "newton": (0.25, 5),
}


def deck_flow_args(deck: Path) -> list[str]:
    """The options the deck says it must run with: its `-- Run with: flow CASE.DATA ...` header,
    written by opm_small_direct.rs (gravity-off cases add --enable-gravity=false)."""
    for line in deck.read_text().splitlines():
        if line.startswith("-- Run with: flow CASE.DATA"):
            return line.removeprefix("-- Run with: flow CASE.DATA").split()
    raise SystemExit(f"{deck}: no '-- Run with: flow CASE.DATA' header")


def run_flow(case: str, out: Path, decks: Path = DECKS) -> float | None:
    """Run Flow unless `out` already holds a finished run of this exact deck."""
    deck = decks / case / "CASE.DATA"
    stamp = out / "deck.sha"
    digest = subprocess.run(["sha256sum", str(deck)], check=True, capture_output=True, text=True).stdout.split()[0]
    if (out / "CASE.INFOSTEP").exists() and stamp.exists() and stamp.read_text().strip() == digest:
        return None
    out.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    subprocess.run(
        [
            "flow",
            str(deck),
            f"--output-dir={out}",
            *deck_flow_args(deck),
            "--output-extra-convergence-info=steps,iterations",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    stamp.write_text(digest + "\n")
    return (time.perf_counter() - started) * 1e3


def flow_convergence(out: Path) -> dict:
    rows = [line.split() for line in (out / "CASE.INFOSTEP").read_text().splitlines()[1:] if line.strip()]
    # Columns: Time TStep Assembly LSetup LSolve LocSol Update Output WellIt Lins NewtIt LinIt Conv
    accepted = [r for r in rows if r[-1] == "1"]
    return {
        "substeps": len(accepted),
        "newton": sum(int(r[-3]) for r in accepted),
        "retries": len(rows) - len(accepted),
        "retry_newton": sum(int(r[-3]) for r in rows if r[-1] != "1"),
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
    return {k: float(smry[k][-1]) for k in CUM_KEYS if k in keys}


def ressim_cumulatives(run: dict) -> dict:
    hist = np.array(run["history"])
    t = np.concatenate([[0.0], hist[:, 0]])
    dt = np.diff(t)
    return {
        "FOPT": float(np.sum(hist[:, 1] * dt)),
        "FWPT": float(np.sum(hist[:, 2] * dt)),
        "FGPT": float(np.sum(hist[:, 3] * dt)),
        # The history's injection column is whatever the case injects, at surface conditions.
        ("FGIT" if run.get("injected") == "gas" else "FWIT"): float(np.sum(hist[:, 4] * dt)),
    }


def ressim_convergence(run: dict) -> dict:
    """Totals over reports. IMPES has no Newton loop: its counters are None, not zero."""
    reports = run["reports"]

    def total(key: str) -> int | None:
        values = [r.get(key) for r in reports]
        return None if any(v is None for v in values) else sum(values)

    return {k: total(k) for k in ("substeps", "newton", "retries", "retry_newton")}


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


def discover_runs(ressim_dir: Path, case: str) -> list[str]:
    found = {p.name[len(case) + 1 : -len(".json")] for p in ressim_dir.glob(f"{case}.*.json")}
    return [r for r in KNOWN_RUNS if r in found] + sorted(found - set(KNOWN_RUNS))


def compare_case(case: str, args: argparse.Namespace) -> dict:
    out = args.flow_dir / case
    flow_ms = run_flow(case, out, args.deck_dir)
    flow = {"fields": flow_fields(out), "cum": flow_cumulatives(out), "conv": flow_convergence(out)}
    entry: dict = {"flow": {"conv": flow["conv"], "wall_ms": flow_ms, "cum": flow["cum"]}, "runs": {}}
    fields_by_run = {}
    for run_id in discover_runs(args.ressim_dir, case):
        run = json.loads((args.ressim_dir / f"{case}.{run_id}.json").read_text())
        fields = [{k: np.array(r[k]) for k in ("p", "sw", "sg")} for r in run["reports"]]
        if len(fields) != len(flow["fields"]):
            raise SystemExit(f"{case}/{run_id}: {len(fields)} ResSim reports vs {len(flow['fields'])} Flow")
        fields_by_run[run_id] = fields
        cum = ressim_cumulatives(run)
        largest = max(abs(x) for x in flow["cum"].values())
        entry["runs"][run_id] = {
            "solver": run.get("solver", "fim"),
            "conv": ressim_convergence(run),
            "wall_ms": run["wall_ms"],
            "warnings": run.get("warnings", []),
            "vs_flow": field_error(fields, flow["fields"]),
            # A cumulative below 0.1% of the case's largest is a trace volume (immobile
            # connate water, pre-breakthrough water): a percentage of it means nothing. Oil is
            # always graded: surface gas volumes dwarf it (SPE1 injects ~1e10 Sm3), not trace it.
            "cum_rel_vs_flow": {
                k: rel(cum[k], v)
                for k, v in flow["cum"].items()
                if k in cum and (k == "FOPT" or abs(v) > 1e-3 * largest)
            },
        }
    entry["pairs"] = {
        f"{a}_vs_{b}": {"invariant": inv, **field_error(fields_by_run[a], fields_by_run[b])}
        for a, b, inv in PAIRS
        if a in fields_by_run and b in fields_by_run
    }
    return entry


# ---- scorecard ------------------------------------------------------------------------------


def git(*argv: str) -> str:
    return subprocess.run(["git", "-C", str(REPO), *argv], capture_output=True, text=True).stdout.strip()


def provenance() -> dict:
    flow_version = subprocess.run(["flow", "--version"], capture_output=True, text=True).stdout.strip()
    return {
        "commit": git("rev-parse", "--short", "HEAD"),
        "dirty": bool(git("status", "--porcelain", "--untracked-files=no")),
        "date": datetime.date.today().isoformat(),
        "flow": flow_version,
        "command": "bash scripts/validate-cross-solver.sh --update",
    }


def scorecard_metrics(entry: dict) -> dict:
    """The gated subset of a case's report: what the scorecard stores and `--check` compares."""
    runs = {}
    for run_id, r in entry["runs"].items():
        # Worst over reports catches a transient (IMPES's first-substep pressure response is one);
        # final catches a drifted answer that a transient elsewhere would mask.
        metrics = {k: r["vs_flow"]["worst"][k] for k in ("p", "sw", "sg")}
        metrics.update({f"final.{k}": r["vs_flow"]["final"][k] for k in ("p", "sw", "sg")})
        metrics.update({f"cum.{k}": v for k, v in r["cum_rel_vs_flow"].items()})
        metrics.update({k: r["conv"][k] for k in ("substeps", "newton") if r["conv"][k] is not None})
        runs[run_id] = {"metrics": metrics, "warnings": len(r["warnings"]), "wall_ms": round(r["wall_ms"], 1)}
    pairs = {k: {"invariant": v["invariant"], **v["worst"]} for k, v in entry["pairs"].items()}
    return {"flow": {k: entry["flow"]["conv"][k] for k in ("substeps", "newton")}, "runs": runs, "pairs": pairs}


def band_for(metric: str) -> tuple[float, float]:
    family, _, name = metric.rpartition(".")
    return BANDS["cum" if family == "cum" else name]


def check(report: dict, scorecard: dict) -> tuple[list[str], list[str], list[str]]:
    failures, improvements, notes = [], [], []

    def judge(where: str, metric: str, new: float, base: float) -> None:
        r, a = band_for(metric)
        if new > base * (1 + r) + a:
            failures.append(f"{where} {metric}: {fmt(new)} vs scorecard {fmt(base)}")
        elif new < base * (1 - r) - a:
            improvements.append(f"{where} {metric}: {fmt(new)} vs scorecard {fmt(base)}")

    for case, base_case in scorecard["cases"].items():
        if case not in report:
            notes.append(f"{case}: in the scorecard but not run (--case filter?)")
            continue
        now = scorecard_metrics(report[case])
        if now["flow"] != base_case["flow"]:
            notes.append(
                f"{case}: Flow itself changed ({now['flow']} vs {base_case['flow']}); "
                f"scorecard was measured with {scorecard['provenance'].get('flow')}"
            )
        for run_id, base_run in base_case["runs"].items():
            run = now["runs"].get(run_id)
            if run is None:
                failures.append(f"{case}/{run_id}: covered by the scorecard but no ResSim output")
                continue
            if run["warnings"] > base_run["warnings"]:
                failures.append(
                    f"{case}/{run_id}: {run['warnings']} solver warning(s), scorecard has {base_run['warnings']}: "
                    + "; ".join(report[case]["runs"][run_id]["warnings"][:3])
                )
            for metric, base in base_run["metrics"].items():
                new = run["metrics"].get(metric)
                if new is None:
                    failures.append(f"{case}/{run_id} {metric}: no longer reported")
                    continue
                judge(f"{case}/{run_id}", metric, new, base)
        for run_id in now["runs"].keys() - base_case["runs"].keys():
            notes.append(f"{case}/{run_id}: not in the scorecard (ungated until --write-scorecard)")
        for pair, base_pair in base_case.get("pairs", {}).items():
            if not base_pair["invariant"] or pair not in now["pairs"]:
                continue
            for metric in ("p", "sw", "sg"):
                judge(f"{case}/{pair}", metric, now["pairs"][pair][metric], base_pair[metric])
    return failures, improvements, notes


def fmt(v: float) -> str:
    return f"{v:.4g}" if isinstance(v, float) else str(v)


# ---- output ---------------------------------------------------------------------------------


def dash(v) -> str:
    return "—" if v is None else str(v)


def print_text(report: dict) -> None:
    for case, e in report.items():
        f = e["flow"]["conv"]
        wall = e["flow"]["wall_ms"]
        print(f"\n{case}")
        print(f"  {'':11} {'substeps':>9} {'newton':>8} {'cuts/retries':>13} {'wall ms':>9}"
              f" {'max|dp| bar':>12} {'max|dSw|':>9} {'max|dSg|':>9}  cumulative rel. diff vs Flow")
        print(f"  {'Flow':11} {f['substeps']:>9} {f['newton']:>8} {f['retries']:>13} "
              f"{'(cached)' if wall is None else f'{wall:.1f}':>9}")
        for run_id, b in e["runs"].items():
            w = b["vs_flow"]["worst"]
            c = b["conv"]
            cum = "  ".join(f"{k} {v:.2%}" for k, v in b["cum_rel_vs_flow"].items())
            print(f"  {LABELS.get(run_id, run_id):11} {c['substeps']:>9} {dash(c['newton']):>8} "
                  f"{dash(c['retries']):>13} {b['wall_ms']:>9.1f} {w['p']:>12.4f} {w['sw']:>9.5f} "
                  f"{w['sg']:>9.5f}  {cum}")
            for warning in b["warnings"][:3]:
                print(f"  {'':11} warning: {warning}")
        for pair, p in e["pairs"].items():
            w = p["worst"]
            print(f"  {pair.replace('_', ' ')}, worst over reports: |dp| {w['p']:.2e} bar  "
                  f"|dSw| {w['sw']:.2e}  |dSg| {w['sg']:.2e}")


def print_markdown(report: dict) -> None:
    print("| Case | Simulator | Substeps | Newton | Retries / cuts | Wall ms | max \\|Δp\\| bar "
          "| max \\|ΔSw\\| | max \\|ΔSg\\| | Cumulatives vs Flow |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for case, e in report.items():
        f = e["flow"]["conv"]
        wall = e["flow"]["wall_ms"]
        print(f"| {case} | Flow | {f['substeps']} | {f['newton']} | {f['retries']} | "
              f"{'' if wall is None else f'{wall:.0f}'} | | | | |")
        for run_id, b in e["runs"].items():
            w = b["vs_flow"]["worst"]
            c = b["conv"]
            cum = ", ".join(f"{k} {v:.2%}" for k, v in b["cum_rel_vs_flow"].items())
            print(f"| | ResSim {LABELS.get(run_id, run_id)} | {c['substeps']} | {dash(c['newton'])} | "
                  f"{dash(c['retries'])} | {b['wall_ms']:.0f} | {w['p']:.3g} | {w['sw']:.2g} | "
                  f"{w['sg']:.2g} | {cum} |")
    print()
    print("| Case | Pair | max \\|Δp\\| bar | max \\|ΔSw\\| | max \\|ΔSg\\| |")
    print("|---|---|---|---|---|")
    for case, e in report.items():
        for pair, p in e["pairs"].items():
            w = p["worst"]
            print(f"| {case} | {pair.replace('_', ' ')} | {w['p']:.2g} | {w['sw']:.2g} | {w['sg']:.2g} |")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ressim-dir", required=True, type=Path)
    parser.add_argument("--flow-dir", required=True, type=Path,
                        help="Flow output root; a case whose deck is unchanged is not re-run")
    parser.add_argument("--case", action="append")
    parser.add_argument("--json", type=Path)
    parser.add_argument("--markdown", action="store_true", help="print tables ready for a README")
    parser.add_argument("--deck-dir", type=Path, default=DECKS,
                        help="decks written with OPM_SMALL_REPORT_DT, for a report-step ladder")
    parser.add_argument("--scorecard", type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="fail on a regression against --scorecard")
    mode.add_argument("--write-scorecard", action="store_true", help="replace --scorecard with this run")
    args = parser.parse_args()
    if (args.check or args.write_scorecard) and not args.scorecard:
        parser.error("--check and --write-scorecard need --scorecard")

    cases = args.case or sorted(p.name for p in args.deck_dir.iterdir() if (p / "CASE.DATA").exists())
    report = {case: compare_case(case, args) for case in cases}

    if args.markdown:
        print_markdown(report)
    else:
        print_text(report)
    if args.json:
        args.json.write_text(json.dumps(report, indent=2) + "\n")

    if args.write_scorecard:
        if args.case:
            parser.error("--write-scorecard records every case; drop --case")
        card = {
            "$comment": "Written by compare_small_direct.py --write-scorecard; do not edit by hand. "
            "Bands are BANDS in that script. See opm/reference-decks/small-direct/README.md.",
            "provenance": provenance(),
            "cases": {case: scorecard_metrics(e) for case, e in report.items()},
        }
        if card["provenance"]["dirty"]:
            print("\nwarning: tracked files are modified; this scorecard is provisional until "
                  "re-written on a committed tree", file=sys.stderr)
        args.scorecard.write_text(json.dumps(card, indent=2) + "\n")
        print(f"\nscorecard written: {args.scorecard}")
    elif args.check:
        scorecard = json.loads(args.scorecard.read_text())
        failures, improvements, notes = check(report, scorecard)
        p = scorecard["provenance"]
        print(f"\nscorecard: {args.scorecard} (measured at {p['commit']}{' dirty' if p['dirty'] else ''}, "
              f"{p['date']}, {p['flow']})")
        for n in notes:
            print(f"  note: {n}")
        for i in improvements:
            print(f"  improved: {i}")
        for f in failures:
            print(f"  REGRESSION: {f}")
        if improvements and not failures:
            print("  Improvements past the band: record them with --write-scorecard on a committed tree.")
        if failures:
            print(f"cross-solver check: FAILED ({len(failures)} regression(s))")
            return 1
        print("cross-solver check: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
