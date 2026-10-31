#!/usr/bin/env python3
"""Compare every ResSim solver against OPM Flow, and against each other, on one record shape.

The black-oil decks and the ResSim runs come from one source,
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

The same record and the same scorecard also carry compositional trajectories. They are graded
against the committed `flowexp_comp` fixtures in `opm/compositional/` rather than a live Flow
restart, so `--comp-reference-dir` replaces `--flow-dir` and `opm.io` is not imported:

    python3 tools/opm_flow/compare_small_direct.py --ressim-dir /tmp/comp-records \\
        --comp-reference-dir opm/compositional [--case 1d_comp] [--json out.json] [--markdown]

A committed scorecard turns both families into one ratchet:

    ... --scorecard opm/reference-decks/small-direct/scorecard.json --check
    ... --scorecard opm/reference-decks/small-direct/scorecard.json --write-scorecard

`--check` fails when an accuracy or work metric regresses past its band (see `BANDS`), when a run
the scorecard covers is missing, or when a run raised a solver warning the scorecard did not
record. It reports improvements past the same band, so a better baseline is written deliberately
rather than left unrecorded. `--write-scorecard` merges the cases measured by this invocation into
the file, so the black-oil and compositional gates can each update their own family; pass
`--prune-scorecard` to replace the file instead. `scripts/validate-cross-solver.sh` and
`scripts/validate-compositional.sh` wrap the two pipelines.

`--ressim-dir` must hold `<case>.<run>.json` from `opm_small_direct_run_ressim`, where `<run>` is
`sparse` / `dense` (FIM, per small-system LU) or `impes`, or `<case>.comp.json` from
`comp_reference_run_records`.
"""
from __future__ import annotations

import argparse
import datetime
import json
import re
import subprocess
import sys
import time
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[2]
DECKS = REPO / "opm" / "reference-decks" / "small-direct"
# Report order. Any other `<case>.<run>.json` found is reported after these.
KNOWN_RUNS = ("sparse", "dense", "impes", "comp")
LABELS = {"sparse": "FIM sparse", "dense": "FIM dense", "impes": "IMPES", "comp": "Compositional"}
# Pairs of ResSim runs compared cell by cell. `invariant` pairs must stay at their scorecard
# agreement; the others show a physical gap and are reported, not gated.
PAIRS = (("sparse", "dense", True), ("sparse", "impes", False))
CUM_KEYS = ("FOPT", "FWPT", "FWIT", "FGPT", "FGIT")
# Fields a case's reference can carry, in table order. A compositional fixture has no water, so
# it carries `z` (overall composition) where a black-oil restart carries `sw`.
FIELD_KEYS = ("p", "sw", "sg", "z")

# Regression bands for `--check`: a metric fails when `new > base * (1 + rel) + abs`, and counts
# as an improvement when `new < base * (1 - rel) - abs`. The absolute floor keeps a near-zero
# baseline (sparse-vs-dense at 1e-10 bar, a cumulative at 0.00%) from turning roundoff into a
# failure; the relative part lets a larger baseline move a little without a rewrite. Wall time is
# never gated: single observations on a shared machine are noise.
BANDS = {
    "p": (0.25, 0.05),  # bar
    "sw": (0.25, 0.002),
    "sg": (0.25, 0.002),
    "z": (0.25, 1e-4),  # overall mole fraction, absolute; fixture print precision is ~1e-6
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
    from opm.io.ecl import ERst

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
    from opm.io.ecl import ESmry

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


def inventory_relative_error(run: dict, cum: dict) -> dict | None:
    """Independent surface inventory plus production minus injection (#63).

    Normalize oil by production, water by initial inventory and gas by its
    largest inventory/throughput scale. Older records lack inventory capture.
    """
    if "initial_inventory_sc" not in run or "final_inventory_sc" not in run:
        return None
    initial = np.array(run["initial_inventory_sc"])
    final = np.array(run["final_inventory_sc"])
    produced = np.array([cum["FWPT"], cum["FOPT"], cum["FGPT"]])
    injected = np.array([cum.get("FWIT", 0.0), 0.0, cum.get("FGIT", 0.0)])
    scale = np.array([
        max(abs(initial[0]), 1.0),
        max(abs(produced[1]), 1.0),
        max(abs(initial[2]), abs(produced[2]), abs(injected[2]), 1.0),
    ])
    error = np.abs(final + produced - injected - initial) / scale
    return dict(zip(("water", "oil", "gas"), map(float, error)))


def checked_fields(
    fields: list[dict], where: str, ndim: dict[str, int] | None = None
) -> list[dict]:
    """Convert every field to float, reject non-finite entries, and validate its shape.

    `json.loads` turns a serialized NaN/Infinity (or a Rust `null`) into a float nan. Checking
    `np.argwhere` alone misses a whole-field `null`: it becomes a 0-d array and `argwhere` returns
    shape `(1, 0)`, so `bad.size` is zero. Test `np.all(np.isfinite(array))` directly and require
    the field's declared rank; a whole-field `sw: null` is not a per-cell array.
    """
    expected_ndim = ndim or {}
    for index, field in enumerate(fields):
        cells = None
        for key, values in field.items():
            try:
                array = np.asarray(values, dtype=float)
            except (TypeError, ValueError) as exc:
                raise SystemExit(f"{where}: report {index} field {key} is not numeric: {exc}") from exc
            if not np.all(np.isfinite(array)):
                bad = np.argwhere(~np.isfinite(array))
                if bad.size:
                    coord = ", ".join(str(int(x)) for x in bad[0])
                    detail = f"index ({coord})"
                else:
                    detail = "the whole field"
                raise SystemExit(f"{where}: report {index} field {key} is non-finite at {detail}")
            rank = expected_ndim.get(key, 1)
            if array.ndim != rank:
                raise SystemExit(
                    f"{where}: report {index} field {key} must be {rank}-d, got shape {array.shape}"
                )
            if array.size == 0:
                raise SystemExit(f"{where}: report {index} field {key} is empty")
            if array.ndim == 1:
                if cells is None:
                    cells = array.shape[0]
                elif array.shape[0] != cells:
                    raise SystemExit(
                        f"{where}: report {index} field {key} has {array.shape[0]} cells, expected {cells}"
                    )
            field[key] = array
        if cells is not None:
            for key, array in field.items():
                if array.ndim == 2 and array.shape[1] != cells:
                    raise SystemExit(
                        f"{where}: report {index} field {key} has {array.shape[1]} cells, expected {cells}"
                    )
    return fields


def field_error(a: list[dict], b: list[dict], keys: tuple[str, ...] = ("p", "sw", "sg")) -> dict:
    """Worst cell difference over all report steps, and at the final one."""
    worst = {key: 0.0 for key in keys}
    for fa, fb in zip(a, b):
        for key in keys:
            if not (np.all(np.isfinite(fa[key])) and np.all(np.isfinite(fb[key]))):
                raise SystemExit(f"non-finite {key} reached field_error")
            worst[key] = max(worst[key], float(np.max(np.abs(fa[key] - fb[key]))))
    final = {key: float(np.max(np.abs(a[-1][key] - b[-1][key]))) for key in keys}
    return {"worst": worst, "final": final}


def rel(a: float, b: float) -> float:
    return abs(a - b) / max(abs(b), 1e-12)


def discover_runs(ressim_dir: Path, case: str) -> list[str]:
    found = {p.name[len(case) + 1 : -len(".json")] for p in ressim_dir.glob(f"{case}.*.json")}
    return [r for r in KNOWN_RUNS if r in found] + sorted(found - set(KNOWN_RUNS))


def comp_reference_fields(path: Path) -> tuple[list[dict], list[float]]:
    """Read a `flowexp_comp` trajectory fixture as the same fields a run record carries.

    The fixture stores overall composition component-major as `zmf1 .. zmfN`, which is also the
    run record's `z` layout. Water is absent from every compositional fixture, so there is no
    `sw` key: the record's `sw` is extra and is not compared.
    """
    doc = json.loads(path.read_text())
    schema = doc.get("schema", "")
    if not schema.startswith("ressim-compositional-reference/"):
        raise SystemExit(f"{path}: unexpected schema {schema!r}")
    steps = doc["report_steps"]
    components = sorted(int(m.group(1)) for key in steps[0] if (m := re.fullmatch(r"zmf(\d+)", key)))
    if components != list(range(1, len(components) + 1)):
        raise SystemExit(f"{path}: zmf components are not contiguous: {components}")
    fields = [
        {
            "p": np.array(step["pressure"], dtype=float),
            "sg": np.array(step["sgas"], dtype=float),
            "z": np.array([step[f"zmf{i}"] for i in range(1, len(components) + 1)], dtype=float),
        }
        for step in steps
    ]
    times = [float(t) for t in doc["summary"]["TIME"]]
    if len(times) != len(fields):
        raise SystemExit(f"{path}: {len(fields)} report steps but {len(times)} summary times")
    return checked_fields(fields, str(path), {"p": 1, "sg": 1, "z": 2}), times


def comp_manifest(comp_dir: Path) -> dict[str, Path]:
    """`trajectory_cases.json` as case -> reference fixture path."""
    manifest = json.loads((comp_dir / "trajectory_cases.json").read_text())
    cases = {}
    for entry in manifest["cases"]:
        reference = Path(entry["reference"])
        cases[entry["case"]] = reference if reference.is_absolute() else REPO / reference
    return cases


def compare_comp_case(case: str, reference_path: Path, args: argparse.Namespace) -> dict:
    run_path = args.ressim_dir / f"{case}.comp.json"
    if not run_path.exists():
        raise SystemExit(f"{case}: no {run_path.name} in {args.ressim_dir} (set COMP_REFERENCE_OUT)")
    run = json.loads(run_path.read_text())
    reference, times = comp_reference_fields(reference_path)
    reports = run["reports"]
    if len(reports) != len(reference):
        raise SystemExit(f"{case}: {len(reports)} ResSim reports vs {len(reference)} reference")
    for index, (report, expected) in enumerate(zip(reports, times)):
        if abs(report["t"] - expected) > 1e-6 * max(1.0, abs(expected)):
            raise SystemExit(
                f"{case}: report {index} is at {report['t']} d, the reference says {expected} d"
            )
    fields = checked_fields(
        [
            {
                "p": np.array(report["p"], dtype=float),
                "sw": np.array(report["sw"], dtype=float),
                "sg": np.array(report["sg"], dtype=float),
                "z": np.array(report["z"], dtype=float),
            }
            for report in reports
        ],
        f"{case}/comp",
        {"p": 1, "sw": 1, "sg": 1, "z": 2},
    )
    for index, (f, r) in enumerate(zip(fields, reference)):
        for key in ("p", "sg", "z"):
            if f[key].shape != r[key].shape:
                raise SystemExit(
                    f"{case}: report {index} field {key} has shape {f[key].shape}, "
                    f"reference {r[key].shape}"
                )
    conv = ressim_convergence(run)
    return {
        "flow": {
            "source": "fixture",
            "conv": {k: None for k in ("substeps", "newton", "retries", "retry_newton")},
            "wall_ms": None,
            "cum": {},
        },
        "runs": {
            "comp": {
                "solver": run.get("solver", "compositional"),
                "conv": conv,
                "wall_ms": run["wall_ms"],
                "warnings": run.get("warnings", []),
                "inventory_relative_error": None,
                # The reference has no surface cumulative in its fixture (the Rust tests check
                # material balance); the scorecard gates the cell trajectory only.
                "vs_flow": field_error(fields, reference, ("p", "sg", "z")),
                "cum_rel_vs_flow": {},
            }
        },
        "pairs": {},
    }


def compare_case(case: str, args: argparse.Namespace) -> dict:
    out = args.flow_dir / case
    flow_ms = run_flow(case, out, args.deck_dir)
    flow = {"fields": flow_fields(out), "cum": flow_cumulatives(out), "conv": flow_convergence(out)}
    flow["fields"] = checked_fields(flow["fields"], f"{case}/Flow")
    entry: dict = {
        "flow": {"source": "flow", "conv": flow["conv"], "wall_ms": flow_ms, "cum": flow["cum"]},
        "runs": {},
    }
    fields_by_run = {}
    for run_id in discover_runs(args.ressim_dir, case):
        run = json.loads((args.ressim_dir / f"{case}.{run_id}.json").read_text())
        fields = checked_fields(
            [{k: np.array(r[k]) for k in ("p", "sw", "sg")} for r in run["reports"]],
            f"{case}/{run_id}",
        )
        if len(fields) != len(flow["fields"]):
            raise SystemExit(f"{case}/{run_id}: {len(fields)} ResSim reports vs {len(flow['fields'])} Flow")
        for index, (fa, fb) in enumerate(zip(fields, flow["fields"])):
            for key in ("p", "sw", "sg"):
                if fa[key].shape != fb[key].shape:
                    raise SystemExit(
                        f"{case}/{run_id}: report {index} field {key} has shape {fa[key].shape}, "
                        f"Flow {fb[key].shape}"
                    )
        fields_by_run[run_id] = fields
        cum = ressim_cumulatives(run)
        largest = max(abs(x) for x in flow["cum"].values())
        entry["runs"][run_id] = {
            "solver": run.get("solver", "fim"),
            "conv": ressim_convergence(run),
            "wall_ms": run["wall_ms"],
            "warnings": run.get("warnings", []),
            "inventory_relative_error": inventory_relative_error(run, cum),
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


def provenance(command: str) -> dict:
    try:
        flow_version = subprocess.run(
            ["flow", "--version"], capture_output=True, text=True
        ).stdout.strip() or None
    except FileNotFoundError:
        flow_version = None
    return {
        "commit": git("rev-parse", "--short", "HEAD"),
        "dirty": bool(git("status", "--porcelain", "--untracked-files=no")),
        "date": datetime.date.today().isoformat(),
        "flow": flow_version,
        "command": command,
    }


def scorecard_metrics(entry: dict) -> dict:
    """The gated subset of a case's report: what the scorecard stores and `--check` compares."""
    runs = {}
    for run_id, r in entry["runs"].items():
        # Worst over reports catches a transient (IMPES's first-substep pressure response is one);
        # final catches a drifted answer that a transient elsewhere would mask.
        fields = [k for k in FIELD_KEYS if k in r["vs_flow"]["worst"]]
        metrics = {k: r["vs_flow"]["worst"][k] for k in fields}
        metrics.update({f"final.{k}": r["vs_flow"]["final"][k] for k in fields})
        metrics.update({f"cum.{k}": v for k, v in r["cum_rel_vs_flow"].items()})
        metrics.update({k: r["conv"][k] for k in ("substeps", "newton") if r["conv"][k] is not None})
        runs[run_id] = {"metrics": metrics, "warnings": len(r["warnings"]), "wall_ms": round(r["wall_ms"], 1)}
    pairs = {k: {"invariant": v["invariant"], **v["worst"]} for k, v in entry["pairs"].items()}
    return {"flow": {k: entry["flow"]["conv"][k] for k in ("substeps", "newton")}, "runs": runs, "pairs": pairs}


def merge_scorecard(
    existing: dict | None, measured: dict, prov: dict, prune: bool
) -> tuple[dict, int]:
    """Merge this run's cases into an existing scorecard without losing old provenance.

    A scorecard written before per-case `measured` blocks existed only had the top-level
    provenance, which described every case in the file. Copy that legacy block into each
    preserved case before the top-level block is replaced by this write's provenance, so a
    partial update cannot silently re-attribute an earlier measurement.
    """
    preserved = {}
    if existing is not None and not prune:
        legacy = existing.get("provenance")
        if legacy:
            legacy = {k: v for k, v in legacy.items() if k in ("commit", "dirty", "date", "flow", "command")}
        for case, entry in existing.get("cases", {}).items():
            entry = dict(entry)
            if "measured" not in entry and legacy:
                entry["measured"] = dict(legacy)
            preserved[case] = entry
    card = {
        "$comment": "Written by compare_small_direct.py --write-scorecard; do not edit by hand. "
        "Bands are BANDS in that script. A write merges the cases it measured and keeps the "
        "rest, because the black-oil and compositional gates update different families. See "
        "opm/reference-decks/small-direct/README.md.",
        "provenance": {**prov, "cases": sorted(measured)},
        "cases": {**preserved, **measured},
    }
    return card, len(set(preserved) - set(measured))


def case_family(entry: dict) -> str:
    """Which solver family a scorecard entry belongs to, from the run ids it covers."""
    runs = entry.get("runs", {})
    if any(run in runs for run in ("sparse", "dense", "impes")):
        return "black-oil"
    if "comp" in runs:
        return "compositional"
    return "unknown"


def coverage_error(report: dict, expected: set[str], family: str, partial: bool) -> str | None:
    """Refuse a complete run that silently dropped a case; `--case` opts into a partial run."""
    if partial:
        return None
    if not expected:
        return f"no {family} cases selected"
    missing = sorted(expected - set(report))
    if missing:
        return (
            f"incomplete {family} coverage: missing {', '.join(missing)}. "
            "Run the whole family or pass --case for an intentional partial run."
        )
    return None


def band_for(metric: str) -> tuple[float, float]:
    family, _, name = metric.rpartition(".")
    return BANDS["cum" if family == "cum" else name]


def check(report: dict, scorecard: dict) -> tuple[list[str], list[str], list[str]]:
    failures, improvements, notes = [], [], []
    missing = []

    def judge(where: str, metric: str, new: float, base: float) -> None:
        r, a = band_for(metric)
        if new > base * (1 + r) + a:
            failures.append(f"{where} {metric}: {fmt(new)} vs scorecard {fmt(base)}")
        elif new < base * (1 - r) - a:
            improvements.append(f"{where} {metric}: {fmt(new)} vs scorecard {fmt(base)}")

    for case, base_case in scorecard["cases"].items():
        if case not in report:
            missing.append(case)
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
    for case in report.keys() - scorecard["cases"].keys():
        notes.append(f"{case}: not in the scorecard (ungated until --write-scorecard)")
    if missing:
        notes.append(
            f"{len(missing)} scorecard case(s) not measured in this invocation, kept: {', '.join(missing)}"
        )
    return failures, improvements, notes


def fmt(v: float) -> str:
    return f"{v:.4g}" if isinstance(v, float) else str(v)


# ---- output ---------------------------------------------------------------------------------

FIELD_LABEL = {"p": "max|dp| bar", "sw": "max|dSw|", "sg": "max|dSg|", "z": "max|dz|"}
FIELD_WIDTH = {"p": 12, "sw": 9, "sg": 9, "z": 9}
FIELD_FORMAT = {"p": ".4f", "sw": ".5f", "sg": ".5f", "z": ".3e"}


def dash(v) -> str:
    return "—" if v is None else str(v)


def case_fields(entry: dict) -> list[str]:
    """The reference fields this case compares, in table order."""
    present = set()
    for run in entry["runs"].values():
        present.update(run["vs_flow"]["worst"])
    return [key for key in FIELD_KEYS if key in present]


def wall_text(entry: dict) -> str:
    wall = entry["flow"]["wall_ms"]
    if entry["flow"].get("source") == "fixture":
        return "fixture"
    return "(cached)" if wall is None else f"{wall:.1f}"


def print_text(report: dict) -> None:
    for case, e in report.items():
        f = e["flow"]["conv"]
        fields = case_fields(e)
        field_header = " ".join(f"{FIELD_LABEL[k]:>{FIELD_WIDTH[k]}}" for k in fields)
        print(f"\n{case}")
        print(f"  {'':11} {'substeps':>9} {'newton':>8} {'cuts/retries':>13} {'wall ms':>9}"
              f" {field_header}  cumulative rel. diff vs Flow")
        print(f"  {'Flow':11} {dash(f['substeps']):>9} {dash(f['newton']):>8} {dash(f['retries']):>13} "
              f"{wall_text(e):>9}")
        for run_id, b in e["runs"].items():
            w = b["vs_flow"]["worst"]
            c = b["conv"]
            cum = "  ".join(f"{k} {v:.2%}" for k, v in b["cum_rel_vs_flow"].items())
            values = " ".join(f"{w[k]:>{FIELD_WIDTH[k]}{FIELD_FORMAT[k]}}" for k in fields)
            print(f"  {LABELS.get(run_id, run_id):11} {dash(c['substeps']):>9} {dash(c['newton']):>8} "
                  f"{dash(c['retries']):>13} {b['wall_ms']:>9.1f} {values}  {cum}")
            for warning in b["warnings"][:3]:
                print(f"  {'':11} warning: {warning}")
        for pair, p in e["pairs"].items():
            w = p["worst"]
            print(f"  {pair.replace('_', ' ')}, worst over reports: |dp| {w['p']:.2e} bar  "
                  f"|dSw| {w['sw']:.2e}  |dSg| {w['sg']:.2e}")


def markdown_field(key: str) -> str:
    name = {"p": "p", "sw": "Sw", "sg": "Sg", "z": "z"}[key]
    return f"max \\|Δ{name}\\| bar" if key == "p" else f"max \\|Δ{name}\\|"


def print_markdown(report: dict) -> None:
    fields = next((case_fields(e) for e in report.values() if e["runs"]), [])
    print("| Case | Simulator | Substeps | Newton | Retries / cuts | Wall ms | "
          + " | ".join(markdown_field(k) for k in fields) + " | Cumulatives vs Flow |")
    print("|---|---|---|---|---|---|" + "---|" * len(fields) + "---|")
    for case, e in report.items():
        f = e["flow"]["conv"]
        wall = e["flow"]["wall_ms"]
        empty = " | ".join("" for _ in fields)
        print(f"| {case} | Flow | {dash(f['substeps'])} | {dash(f['newton'])} | {dash(f['retries'])} | "
              f"{'' if wall is None else f'{wall:.0f}'} | {empty} | |")
        for run_id, b in e["runs"].items():
            w = b["vs_flow"]["worst"]
            c = b["conv"]
            cum = ", ".join(f"{k} {v:.2%}" for k, v in b["cum_rel_vs_flow"].items())
            values = " | ".join(f"{w[k]:.3g}" if k == "p" else f"{w[k]:.2g}" for k in fields)
            print(f"| | ResSim {LABELS.get(run_id, run_id)} | {dash(c['substeps'])} | "
                  f"{dash(c['newton'])} | {dash(c['retries'])} | {b['wall_ms']:.0f} | {values} | {cum} |")
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
    parser.add_argument("--flow-dir", type=Path,
                        help="Flow output root; a case whose deck is unchanged is not re-run")
    parser.add_argument("--comp-reference-dir", type=Path,
                        help="compositional fixture root (holds trajectory_cases.json); replaces --flow-dir")
    parser.add_argument("--case", action="append")
    parser.add_argument("--json", type=Path)
    parser.add_argument("--markdown", action="store_true", help="print tables ready for a README")
    parser.add_argument("--inventory-check", action="store_true",
                        help="require independent IMPES component accounting to close within 1e-8")
    parser.add_argument("--deck-dir", type=Path, default=DECKS,
                        help="decks written with OPM_SMALL_REPORT_DT, for a report-step ladder")
    parser.add_argument("--scorecard", type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="fail on a regression against --scorecard")
    mode.add_argument("--write-scorecard", action="store_true",
                      help="merge this run's cases into --scorecard")
    parser.add_argument("--prune-scorecard", action="store_true",
                        help="with --write-scorecard, drop cases this run did not measure")
    parser.add_argument("--provenance-command", default="bash scripts/validate-cross-solver.sh --update",
                        help="command recorded as having measured the scorecard")
    args = parser.parse_args()
    if (args.check or args.write_scorecard) and not args.scorecard:
        parser.error("--check and --write-scorecard need --scorecard")
    if args.prune_scorecard and not args.write_scorecard:
        parser.error("--prune-scorecard needs --write-scorecard")
    if args.comp_reference_dir is None and args.flow_dir is None:
        parser.error("--flow-dir or --comp-reference-dir is required")

    scorecard = None
    if args.scorecard and args.scorecard.exists():
        scorecard = json.loads(args.scorecard.read_text())

    if args.comp_reference_dir:
        family = "compositional"
        manifest = comp_manifest(args.comp_reference_dir)
        candidates = set(manifest)
        cases = args.case or list(manifest)
        unknown = sorted(set(cases) - candidates)
        if unknown:
            parser.error(f"not in trajectory_cases.json: {', '.join(unknown)}")
    else:
        family = "black-oil"
        candidates = {p.name for p in args.deck_dir.iterdir() if (p / "CASE.DATA").exists()}
        cases = args.case or sorted(candidates)

    # A complete run must cover the whole family. A case can be absent from this invocation's
    # manifest/deck tree and still be in the committed scorecard; silently dropping it would turn
    # the ratchet off for that case. `--case` is the explicit opt-in to a partial run, and
    # `--prune-scorecard` is the explicit opt-in to retiring cases.
    expected = set(cases) if args.case else set(candidates)
    if scorecard is not None and not args.prune_scorecard:
        expected |= {
            case for case, entry in scorecard.get("cases", {}).items()
            if case_family(entry) == family
        }

    if args.comp_reference_dir:
        report = {case: compare_comp_case(case, manifest[case], args) for case in cases}
    else:
        report = {case: compare_case(case, args) for case in cases}

    error = coverage_error(report, expected, family, bool(args.case))
    if error:
        raise SystemExit(error)

    if args.markdown:
        print_markdown(report)
    else:
        print_text(report)
    if args.json:
        args.json.write_text(json.dumps(report, indent=2) + "\n")

    if args.inventory_check:
        failures = []
        checked = 0
        if args.markdown:
            print("\n| Case | IMPES water / initial | IMPES oil / produced | IMPES gas / scale |")
            print("|---|---|---|---|")
        for case, entry in report.items():
            for run_id, run in entry["runs"].items():
                if run["solver"] != "impes":
                    continue
                checked += 1
                error = run["inventory_relative_error"]
                if error is None:
                    failures.append(f"{case}/{run_id}: missing independent inventory capture")
                    continue
                if args.markdown:
                    print(f"| {case} | {error['water']:.3e} | {error['oil']:.3e} | {error['gas']:.3e} |")
                for phase, value in error.items():
                    if not np.isfinite(value) or value > 1e-8:
                        failures.append(f"{case}/{run_id}: {phase} inventory error {value:.3e}")
        if checked == 0:
            failures.append("no IMPES run was checked")
        if failures:
            print("\nInventory check: FAILED\n" + "\n".join(failures))
            return 1
        print("\nInventory check: OK")

    if args.write_scorecard:
        if args.case:
            parser.error("--write-scorecard records every case; drop --case")
        prov = provenance(args.provenance_command)
        measured = {case: scorecard_metrics(e) for case, e in report.items()}
        for entry in measured.values():
            entry["measured"] = dict(prov)
        card, kept = merge_scorecard(scorecard, measured, prov, args.prune_scorecard)
        if prov["dirty"]:
            print("\nwarning: tracked files are modified; this scorecard is provisional until "
                  "re-written on a committed tree", file=sys.stderr)
        if kept:
            print(f"\nkept {kept} case(s) from the existing scorecard")
        args.scorecard.write_text(json.dumps(card, indent=2) + "\n")
        print(f"\nscorecard written: {args.scorecard}")
    elif args.check:
        if scorecard is None:
            parser.error(f"{args.scorecard}: no scorecard to check")
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
