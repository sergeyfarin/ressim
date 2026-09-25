#!/usr/bin/env python3
"""Collect, render and check the benchmark records behind docs/BENCHMARKS.md (#54).

`scripts/benchmarks.sh` runs the producers and calls this script; it is rarely run directly.

    collect  --run DIR --into docs/benchmarks/benchmarks.json   merge a run into the committed file
    render   [--check]                                          regenerate the page's tables
    check    --run DIR                                          compare a run with the committed file

A run directory holds whatever the producers wrote:

    records.jsonl          metric and series records (Rust tests, parity, the SPE1 Flow oracle)
    cross_solver.json      compare_small_direct.py --json report
    fim_wasm/<case>.json   fim-wasm-diagnostic.mjs --json output
    status.json            {section: {"status": "ran" | "skipped" | "failed", "note": ..., "command": ...}}

Only the text between `<!-- GENERATED:<name> -->` and `<!-- /GENERATED:<name> -->` in the page is
rewritten; the prose around it stays hand-written. Every number in a generated block comes from
`benchmarks.json`, which records per section the commit it was measured on.

Standard library only, so it runs anywhere `python3` does (CI included).
"""
from __future__ import annotations

import argparse
import datetime
import json
import math
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RECORDS = REPO / "docs" / "benchmarks" / "benchmarks.json"
PAGE = REPO / "docs" / "BENCHMARKS.md"

# Section key -> (title for the summary, reference). Order is page order.
SECTIONS = {
    "buckley": ("1D waterflood breakthrough", "Buckley-Leverett + Welge"),
    "spe1": ("SPE1 Case 1, 10 years", "Published SPE1 / Flow"),
    "three_phase": ("Three-phase gas drive and injection", "OPM Flow"),
    "depletion": ("Black-oil depletion column", "Grid self-convergence, Flow"),
    "cross_solver": ("Eight small decks, three solvers", "OPM Flow, generated decks"),
    "parity": ("Native vs wasm bindings", "Each other"),
    "fim_wasm": ("FIM convergence, long horizons", "Substeps per report step"),
    "compositional": ("Compositional, matched timestep", "OPM flowexp_comp"),
}

# The long-horizon FIM cases, in page order: (case id, preset, grid, dt [d], report steps).
# `benchmarks.py plan-fim-wasm` hands this list to scripts/benchmarks.sh, so it lives only here.
FIM_WASM_CASES = [
    ("water-pressure 20x20x3 dt 0.25", "water-pressure", "20x20x3", "0.25", 20),
    ("water-pressure 22x22x1 dt 0.25", "water-pressure", "22x22x1", "0.25", 20),
    ("water-pressure 23x23x1 dt 0.25", "water-pressure", "23x23x1", "0.25", 20),
    ("water-pressure 12x12x3 dt 1 (heavy)", "water-pressure", "12x12x3", "1", 20),
    ("gas-rate 10x10x3 dt 0.25", "gas-rate", "10x10x3", "0.25", 24),
    ("gas-pressure 10x10x3 dt 0.25", "gas-pressure", "10x10x3", "0.25", 20),
    ("sweep-areal 21x21x1 dt 0.25", "sweep-areal", "21x21x1", "0.25", 20),
]

# Units whose values are run-to-run noise, never drift: wall time and the share of it.
TIMING_UNITS = {"ms", "share"}

# A reference simulator a record depends on, by the text of its `reference`. A section run
# without that simulator reports the records as not produced, not as regressed.
ORACLES = {"flow": "OPM Flow"}


# ---- provenance -------------------------------------------------------------------------------


def git(*argv: str) -> str:
    return subprocess.run(["git", "-C", str(REPO), *argv], capture_output=True, text=True).stdout.strip()


def provenance(command: str) -> dict:
    return {
        "commit": git("rev-parse", "--short", "HEAD"),
        "dirty": bool(git("status", "--porcelain", "--untracked-files=no")),
        "date": datetime.date.today().isoformat(),
        "command": command,
    }


# ---- collecting a run -------------------------------------------------------------------------


def metric(section: str, case: str, name: str, value, band=None, unit="", reference="",
           same_model=False, at="") -> dict:
    return {"kind": "metric", "section": section, "case": case, "metric": name, "value": value,
            "band": band, "unit": unit, "reference": reference, "same_model": same_model, "at": at}


def read_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def cross_solver_records(report: dict) -> list[dict]:
    """compare_small_direct.py's report as records: the §5 table, one metric per cell."""
    out = []
    for case, entry in report.items():
        conv = entry["flow"]["conv"]
        for key in ("substeps", "newton", "retries"):
            out.append(metric("cross_solver", case, f"flow.{key}", conv[key], unit="count",
                              reference="OPM Flow"))
        for run_id, run in entry["runs"].items():
            m = lambda name, value, unit: out.append(metric(  # noqa: E731
                "cross_solver", case, f"{run_id}.{name}", value, unit=unit,
                reference="OPM Flow, generated deck", same_model=True))
            for key in ("substeps", "newton", "retries"):
                m(key, run["conv"][key], "count")
            m("wall_ms", run["wall_ms"], "ms")
            m("warnings", len(run["warnings"]), "count")
            for key in ("p", "sw", "sg"):
                m(f"worst.{key}", run["vs_flow"]["worst"][key], "bar" if key == "p" else "frac")
            for key, value in run["cum_rel_vs_flow"].items():
                m(f"cum.{key}", value, "frac")
        for pair, p in entry["pairs"].items():
            for key in ("p", "sw", "sg"):
                out.append(metric("cross_solver", case, f"pair.{pair}.{key}", p["worst"][key],
                                  unit="bar" if key == "p" else "frac", reference=pair,
                                  same_model=p["invariant"]))
    return out


def fim_wasm_records(run_dir: Path) -> list[dict]:
    """fim-wasm-diagnostic.mjs --json outputs, summed over report steps."""
    out = []
    for case, _preset, _grid, _dt, steps in FIM_WASM_CASES:
        path = run_dir / "fim_wasm" / f"{slug(case)}.json"
        if not path.exists():
            continue
        records = json.loads(path.read_text())["stepRecords"]
        substeps = sum(r["fimAcceptedSubsteps"] for r in records)
        accepted = [rung for r in records for rung in r["fimAcceptedRungs"]]
        retried = [rung for r in records for rung in r["fimRetryRungs"]]
        solver_ms = sum(r["fimSolverMs"] for r in records)
        linear_ms = sum(r["fimLinearSolveMs"] + r["fimLinearPreconditionerMs"] for r in records)
        values = [
            ("report_steps", len(records), "count"),
            ("substeps", substeps, "count"),
            ("ratio", substeps / max(len(records), 1), "ratio"),
            # Accepted substeps only, like Flow's and the cross-solver scorecard's counts.
            ("newton", sum(rung.get("newton_iterations", 0) for rung in accepted), "count"),
            ("retry_newton", sum(rung.get("newton_iterations", 0) for rung in retried), "count"),
            ("retries.linear", sum(r["fimLinearBadRetries"] for r in records), "count"),
            ("retries.nonlinear", sum(r["fimNonlinearBadRetries"] for r in records), "count"),
            ("retries.mixed", sum(r["fimMixedRetries"] for r in records), "count"),
            ("warnings", sum(1 for r in records if r["warning"]), "count"),
            ("fim_ms", solver_ms, "ms"),
            ("linear_share", linear_ms / solver_ms if solver_ms else 0.0, "share"),
        ]
        if len(records) != steps:
            values.append(("steps_expected", steps, "count"))
        for name, value, unit in values:
            out.append(metric("fim_wasm", case, name, value, unit=unit, reference="wasm build"))
    return out


def slug(case: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", case.lower()).strip("-")


def series_comparisons(records: list[dict]) -> list[dict]:
    """ResSim series against Flow series of the same section, case and quantity: worst gap."""
    series = {(r["section"], r["case"], r["quantity"], r["source"]): r
              for r in records if r["kind"] == "series"}
    out = []
    for (section, case, quantity, source), ours in sorted(series.items()):
        theirs = series.get((section, case, quantity, "flow"))
        if source != "ressim" or theirs is None:
            continue
        at_flow = dict(zip(theirs["t"], theirs["v"]))
        worst, worst_t = 0.0, None
        for t, v in zip(ours["t"], ours["v"]):
            ref = at_flow.get(t)
            if ref is None or abs(ref) < 1e-12:
                continue
            gap = (v - ref) / abs(ref)
            if abs(gap) > abs(worst):
                worst, worst_t = gap, t
        out.append(metric(section, case, f"vs_flow.{quantity}_rel_err", worst, unit="frac",
                          reference="OPM Flow, same grid, hand-mapped deck", same_model=True,
                          at="" if worst_t is None else f"t={worst_t:g} d"))
    return out


def collect_run(run_dir: Path) -> dict[str, list[dict]]:
    """Every record in a run directory, grouped by section. Series stay out of the page."""
    raw = read_jsonl(run_dir / "records.jsonl")
    records = [r for r in raw if r["kind"] == "metric"] + series_comparisons(raw)
    report = run_dir / "cross_solver.json"
    if report.exists():
        records += cross_solver_records(json.loads(report.read_text()))
    records += fim_wasm_records(run_dir)
    by_section: dict[str, list[dict]] = {}
    for r in records:
        if r["section"] not in SECTIONS:
            raise SystemExit(f"record for unknown section {r['section']!r}; add it to SECTIONS")
        r = {k: v for k, v in r.items() if k != "kind"}
        by_section.setdefault(r.pop("section"), []).append(r)
    return by_section


def cmd_collect(args: argparse.Namespace) -> int:
    status = json.loads((args.run / "status.json").read_text())
    by_section = collect_run(args.run)
    doc = json.loads(args.into.read_text()) if args.into.exists() else {"sections": {}}
    doc["$comment"] = ("Written by tools/benchmarks/benchmarks.py via scripts/benchmarks.sh update; "
                       "do not edit by hand. See docs/BENCHMARKS.md and #54.")
    for section, s in status.items():
        if s["status"] != "ran" or s.get("missing"):
            # Keep the last measurement and its stamp: an absent oracle is not a new result, and
            # half a section would drop the half that needs it.
            why = s.get("note") or f"without {', '.join(s.get('missing', []))}"
            print(f"  {section}: {s['status']} ({why}); keeping the committed records")
            continue
        if not by_section.get(section):
            raise SystemExit(f"{section} ran but produced no records; did a test filter stop matching?")
        doc["sections"][section] = {
            "provenance": provenance(s["command"]) | {k: v for k, v in s.items()
                                                      if k not in ("status", "command", "missing")},
            "records": by_section.get(section, []),
        }
        print(f"  {section}: {len(by_section.get(section, []))} records")
    doc["sections"] = {k: doc["sections"][k] for k in SECTIONS if k in doc["sections"]}
    args.into.parent.mkdir(parents=True, exist_ok=True)
    args.into.write_text(json.dumps(doc, indent=1) + "\n")
    return 0


# ---- checking a run against the committed records ---------------------------------------------


def key(r: dict) -> tuple:
    return (r["case"], r["metric"])


def cmd_check(args: argparse.Namespace) -> int:
    """Fail when a banded error grew by more than a tenth of its band, or crossed it; note any
    other change so it gets recorded deliberately with `update`."""
    status = json.loads((args.run / "status.json").read_text())
    committed = json.loads(RECORDS.read_text())["sections"] if RECORDS.exists() else {}
    fresh = collect_run(args.run)
    failures, notes, changed = [], [], 0
    for section, s in status.items():
        if s["status"] != "ran":
            notes.append(f"{section}: {s['status']} ({s.get('note', '')})")
            continue
        old = {key(r): r for r in committed.get(section, {}).get("records", [])}
        new = {key(r): r for r in fresh.get(section, [])}
        missing = [ORACLES[o] for o in s.get("missing", [])]
        for k in sorted(old.keys() - new.keys()):
            if any(m in old[k]["reference"] for m in missing):
                notes.append(f"{section}/{k[0]}/{k[1]}: not checked, needs {old[k]['reference']}")
            else:
                failures.append(f"{section}/{k[0]}/{k[1]}: recorded before, not produced now")
        for k in sorted(new.keys() - old.keys()):
            notes.append(f"{section}/{k[0]}/{k[1]}: new, not yet recorded")
            changed += 1
        for k in sorted(new.keys() & old.keys()):
            a, b = old[k]["value"], new[k]["value"]
            label = f"{section}/{k[0]}/{k[1]}"
            if new[k]["unit"] in TIMING_UNITS or a == b:
                continue
            if not (isinstance(a, (int, float)) and isinstance(b, (int, float))):
                notes.append(f"{label}: {a!r} -> {b!r}")
                changed += 1
                continue
            band = new[k]["band"]
            if band is not None and abs(b) > band:
                failures.append(f"{label}: {b:.4g} is outside its band {band:.4g}")
            elif band is not None and abs(b) - abs(a) > 0.1 * band:
                failures.append(f"{label}: {a:.4g} -> {b:.4g}, worse by more than a tenth of "
                                f"its band {band:.4g}")
            elif abs(b - a) > 1e-9 * max(abs(a), 1.0):
                notes.append(f"{label}: {a:.6g} -> {b:.6g}")
                changed += 1
    for n in notes:
        print(f"  note: {n}")
    for f in failures:
        print(f"  DRIFT: {f}")
    if failures:
        print(f"benchmark check: FAILED ({len(failures)})")
        return 1
    print("benchmark check: OK" + (f" ({changed} change(s) noted above: record them with update)"
                                   if changed else ""))
    return 0


# ---- rendering ------------------------------------------------------------------------------


def pct(v, digits=2, sign="") -> str:
    if v is None:
        return "—"
    # A balance drift of 1e-7 is not "0.000 %": below what `digits` can show, say how small.
    if v != 0 and abs(v * 100) < 0.5 * 10 ** -digits:
        return f"{v * 100:{sign}.1e} %"
    return f"{v * 100:{sign}.{digits}f} %"


def signed_pct(v, digits=1) -> str:
    return pct(v, digits, sign="+")


def g(v, digits=3) -> str:
    if v is None:
        return "—"
    if isinstance(v, int):
        return str(v)
    if v == 0:
        return "0"
    return f"{v:.{digits}g}"


def table(header: list[str], rows: list[list[str]]) -> str:
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    lines += ["| " + " | ".join(row) + " |" for row in rows]
    return "\n".join(lines)


class Section:
    def __init__(self, data: dict | None):
        self.data = data or {"records": [], "provenance": None}
        self.index = {key(r): r for r in self.data["records"]}

    def get(self, case: str, name: str) -> dict | None:
        return self.index.get((case, name))

    def value(self, case: str, name: str):
        r = self.get(case, name)
        return None if r is None else r["value"]

    def cases(self) -> list[str]:
        return list(dict.fromkeys(r["case"] for r in self.data["records"]))

    def stamp(self) -> str:
        p = self.data["provenance"]
        if not p:
            return "*Not measured yet.*"
        state = "dirty tree, provisional" if p["dirty"] else "clean tree"
        extra = "".join(f", {k} `{v}`" for k, v in p.items()
                        if k not in ("commit", "dirty", "date", "command", "note"))
        return f"*Measured on `{p['commit']}` ({state}), {p['date']}{extra}.*"


def criteria_rows(s: Section, case: str, labels: list[tuple[str, str]], signed=False) -> list[list[str]]:
    rows = []
    for name, label in labels:
        r = s.get(case, name)
        if r is None:
            continue
        value = signed_pct(r["value"], 3) if signed else pct(abs(r["value"]), 3)
        at = f" ({r['at']})" if r["at"] else ""
        rows.append([label, pct(r["band"], 1) if r["band"] is not None else "—", value + at])
    return rows


def render_summary(sections: dict[str, Section]) -> str:
    """One row per section: its tightest banded criterion, i.e. the most band used."""
    rows = []
    for name, (title, reference) in SECTIONS.items():
        s = sections[name]
        banded = [r for r in s.data["records"] if r["band"] and isinstance(r["value"], (int, float))
                  and not math.isnan(r["value"])]
        p = s.data["provenance"]
        stamp = f"`{p['commit']}`" + (" (dirty)" if p and p["dirty"] else "") if p else "—"
        if not banded:
            rows.append([title, reference, "no banded criterion", "—", "—", stamp])
            continue
        r = max(banded, key=lambda r: abs(r["value"]) / r["band"])
        shown = pct(abs(r["value"])) if r["unit"] == "frac" else f"{g(abs(r['value']))} {r['unit']}"
        band = pct(r["band"], 1) if r["unit"] == "frac" else f"{g(r['band'])} {r['unit']}"
        rows.append([title, reference, f"{r['case']}: {r['metric']} {shown}", band,
                     f"{abs(r['value']) / r['band']:.0%}", stamp])
    return table(["Area", "Reference", "Tightest criterion", "Band", "Band used", "Measured on"], rows)


def render_buckley(s: Section) -> str:
    rows = []
    for case in ("BL-Case-A", "BL-Case-B"):
        c = f"{case} nx=24"
        rows.append([case[-1], g(s.value(c, "pv_bt_sim"), 4), g(s.value(c, "pv_bt_ref"), 4),
                     signed_pct(s.value(c, "breakthrough_rel_err")),
                     pct((s.get(c, "breakthrough_rel_err") or {}).get("band"), 0)])
    out = [s.stamp(), "", table(["Case", "PV_BT sim", "PV_BT ref", "Rel. error", "Band"], rows), ""]
    grid = [[case[-1]] + [signed_pct(s.value(f"{case} nx={nx}", "breakthrough_rel_err"))
                          for nx in (24, 48, 96, 192)] for case in ("BL-Case-A", "BL-Case-B")]
    out += ["Grid refinement (breakthrough error; nx = 96 and 192 from "
            "`benchmark_buckley_leverett_grid_sweep_replay`):", "",
            table(["Case", "nx = 24", "48", "96", "192"], grid), ""]
    spreads = [f"{c[-8]} {pct(s.value(c, 'report_interval_spread'), 2)}"
               for c in ("BL-Case-A-dt0.50", "BL-Case-B-dt0.50")]
    band = (s.get("BL-Case-A-dt0.50", "report_interval_spread") or {}).get("band")
    out.append(f"Report interval 0.5 → 0.25 d moves breakthrough by {', '.join(spreads)} "
               f"(band {pct(band, 0)}).")
    return "\n".join(out)


def render_spe1(s: Section) -> str:
    labels = [("pressure_rel_err", "Field average pressure, yearly to 3650 d"),
              ("oil_rate_rel_err", "Producer oil rate, yearly to 3650 d"),
              ("gor_rel_err", "Producing GOR, yearly to 3650 d"),
              ("plateau_rel_err", "Oil-rate plateau while the reference is on plateau (≤ 730 d)"),
              ("mb_drift_oil", "Oil material-balance drift"),
              ("mb_drift_gas", "Gas material-balance drift")]
    out = [s.stamp(), "", "Against the published series:", "",
           table(["Criterion", "Band", "Worst measured"], criteria_rows(s, "10x10x3", labels)), ""]
    rows = []
    for case in ("10x10x3", "20x20x3"):
        rows.append([case] + [
            pct(s.value(case, f"published_{q}_rel_err")) for q in ("pressure", "oil_rate", "gor")
        ] + [signed_pct(s.value(case, f"vs_flow.{q}_rel_err"), 2) for q in ("FPR", "FOPR", "WGOR")])
    out += ["Characterization, no band: the published 10×10×3 series, and OPM Flow run on the same "
            "grid (`tools/opm_flow/spe1_refinement_oracle.py`, 90-day checkpoints, signed "
            "ResSim − Flow):", "",
            table(["Grid", "vs published: p", "q_o", "GOR", "vs Flow: p", "q_o", "GOR"], rows)]
    return "\n".join(out)


def render_three_phase(s: Section) -> str:
    labels = [("pressure_rel_err", "Field average pressure, 11 checkpoints"),
              ("gor_rel_err", "Producing GOR"),
              ("cum_oil_rel_err", "Cumulative surface oil"),
              ("oil_rate_rel_err", "Producer oil rate while reference ≥ 10 Sm³/d"),
              ("mb_drift_oil", "Oil material-balance drift"),
              ("mb_drift_gas", "Gas material-balance drift")]
    twin = [("cum_oil_rel_err", "Cumulative oil"),
            ("cum_gas_injected_rel_err", "Cumulative gas injected"),
            ("cum_gas_produced_rel_err", "Cumulative gas produced, after breakthrough")]
    bt = [g(s.value(f"gas_flood dt={dt}", "breakthrough_days")) for dt in ("1.0", "0.5")]
    return "\n".join([
        s.stamp(), "", "**Solution gas drive** (`gas_drive`, 20 cells, FIM, 60 × 10 d; signed ResSim − Flow):", "",
        table(["Criterion", "Band", "Worst measured"], criteria_rows(s, "gas_drive", labels, signed=True)), "",
        "**1D gas injection** (`gas_injection`'s Flow twin, `small-direct/go-1d-50`; signed):", "",
        table(["Criterion", "Band", "Worst measured"],
              criteria_rows(s, "gas_injection (go-1d-50)", twin, signed=True)), "",
        f"**Gas-front behavior** (20-cell gas flood): breakthrough at {bt[0]} d with dt = 1.0 and "
        f"{bt[1]} d with dt = 0.5.",
    ])


def render_depletion(s: Section) -> str:
    rows = []
    for nx in (5, 10, 20, 40):
        cells = []
        for solver in ("IMPES", "FIM", "Flow"):
            p, sg = s.value(f"{solver} nx={nx}", "avg_pressure"), s.value(f"{solver} nx={nx}", "avg_sat_gas")
            cells.append("" if p is None else f"{p:.4f} / {sg:.6f}")
        rows.append([str(nx)] + cells)
    conv = []
    for solver in ("IMPES", "FIM"):
        for q in ("pressure", "sat_gas"):
            c, f = s.get(solver, f"{q}_contraction"), s.get(solver, f"{q}_finest_pair_gap")
            if c and f:
                conv.append([solver, q, f"{c['value']:.3f} (≤ {c['band']})",
                             f"{pct(f['value'])} (≤ {pct(f['band'], 1)})"])
    return "\n".join([
        s.stamp(), "", table(["nx", "IMPES p / Sg", "FIM p / Sg", "Flow p / Sg"], rows), "",
        table(["Solver", "Quantity", "Worst contraction ratio", "Finest-pair gap"], conv)])


LABELS = {"sparse": "FIM sparse", "dense": "FIM dense", "impes": "IMPES"}


def render_cross_solver(s: Section) -> str:
    rows = []
    for case in s.cases():
        rows.append([case, "Flow"] + [g(s.value(case, f"flow.{k}")) for k in ("substeps", "newton", "retries")]
                    + ["", "", "", "", ""])
        runs = dict.fromkeys(r["metric"].split(".")[0] for r in s.data["records"]
                             if r["case"] == case and r["metric"].split(".")[0] in LABELS)
        for run in runs:
            v = lambda name: s.value(case, f"{run}.{name}")  # noqa: E731
            cums = ", ".join(f"{r['metric'].split('.')[-1]} {pct(r['value'])}"
                             for r in s.data["records"]
                             if r["case"] == case and r["metric"].startswith(f"{run}.cum."))
            rows.append(["", f"ResSim {LABELS[run]}", g(v("substeps")), g(v("newton")), g(v("retries")),
                         f"{v('wall_ms'):.0f}", g(v("worst.p")), g(v("worst.sw"), 2), g(v("worst.sg"), 2), cums])
    return "\n".join([s.stamp(), "", table(
        ["Case", "Simulator", "Substeps", "Newton", "Retries / cuts", "Wall ms", "max \\|Δp\\| bar",
         "max \\|ΔSw\\|", "max \\|ΔSg\\|", "Cumulatives vs Flow"], rows)])


def render_parity(s: Section) -> str:
    rows = []
    for case in s.cases():
        band = (s.get(case, "cells_max_abs_diff") or {}).get("band")
        rows.append([case] + [g(s.value(case, f"{q}_max_abs_diff"), 2) for q in ("cells", "rates", "grid")]
                    + [g(band) if band else "reported only"])
    return "\n".join([s.stamp(), "", table(["Case", "Cells", "Rates", "Grid state", "Band"], rows)])


def render_fim_wasm(s: Section) -> str:
    rows = []
    for case, *_ in FIM_WASM_CASES:
        v = lambda name: s.value(case, name)  # noqa: E731
        if v("substeps") is None:
            continue
        retries = "/".join(g(v(f"retries.{k}")) for k in ("linear", "nonlinear", "mixed"))
        rows.append([case, g(v("report_steps")), g(v("substeps")), f"{v('ratio'):.2f}", g(v("newton")),
                     retries, g(v("retry_newton")), f"{v('fim_ms'):.0f}", f"{v('linear_share'):.0%}"])
    return "\n".join([s.stamp(), "", table(
        ["Case", "Report steps", "Substeps", "Ratio", "Newton", "Retries lin/nonlin/mixed",
         "Newton in retries", "FIM ms", "Linear + precond."], rows)])


def render_compositional(s: Section) -> str:
    rows = []
    for r in s.data["records"]:
        value = pct(r["value"], 4) if r["unit"] == "frac" else f"{r['value']:.4f} {r['unit']}"
        band = pct(r["band"], 2) if r["unit"] == "frac" else f"{g(r['band'])} {r['unit']}"
        rows.append([r["case"], r["metric"], value, band, r["at"] or ""])
    return "\n".join([s.stamp(), "", table(["Case", "Metric", "Measured", "Band", "Where"], rows)])


RENDERERS = {
    "buckley": render_buckley,
    "spe1": render_spe1,
    "three_phase": render_three_phase,
    "depletion": render_depletion,
    "cross_solver": render_cross_solver,
    "parity": render_parity,
    "fim_wasm": render_fim_wasm,
    "compositional": render_compositional,
}

BLOCK = re.compile(r"(<!-- GENERATED:(\w+) -->\n).*?(<!-- /GENERATED:\2 -->)", re.S)


def render_page(page: str, doc: dict) -> str:
    sections = {k: Section(doc["sections"].get(k)) for k in SECTIONS}

    def block(m: re.Match) -> str:
        name = m.group(2)
        if name == "summary":
            body = render_summary(sections)
        elif name in RENDERERS:
            body = RENDERERS[name](sections[name])
        else:
            raise SystemExit(f"unknown generated block {name!r} in {PAGE}")
        return m.group(1) + body + "\n" + m.group(3)

    return BLOCK.sub(block, page)


def cmd_render(args: argparse.Namespace) -> int:
    doc = json.loads(RECORDS.read_text())
    page = PAGE.read_text()
    rendered = render_page(page, doc)
    if args.check:
        if rendered != page:
            print(f"{PAGE.relative_to(REPO)} differs from what {RECORDS.relative_to(REPO)} generates; "
                  "run `bash scripts/benchmarks.sh render` (never edit a GENERATED block by hand)")
            return 1
        print("benchmark page: in sync with its records")
        return 0
    PAGE.write_text(rendered)
    print(f"rendered {PAGE.relative_to(REPO)}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("collect")
    p.add_argument("--run", type=Path, required=True)
    p.add_argument("--into", type=Path, default=RECORDS)
    p = sub.add_parser("check")
    p.add_argument("--run", type=Path, required=True)
    p = sub.add_parser("render")
    p.add_argument("--check", action="store_true")
    sub.add_parser("plan-fim-wasm", help="print 'file preset grid dt steps' per FIM long-horizon case")
    args = parser.parse_args()
    if args.command == "plan-fim-wasm":
        for case, preset, grid, dt, steps in FIM_WASM_CASES:
            print(slug(case), preset, grid, dt, steps)
        return 0
    return {"collect": cmd_collect, "check": cmd_check, "render": cmd_render}[args.command](args)


if __name__ == "__main__":
    sys.exit(main())
