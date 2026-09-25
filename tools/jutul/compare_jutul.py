#!/usr/bin/env python3
"""Turn JutulDarcy runs into benchmark records: a second independent simulator next to OPM Flow (#54).

    python3 tools/jutul/compare_jutul.py --jutul-dir J --cross-dir C --records records.jsonl

`--jutul-dir` holds `<case>.json` from `run_decks.jl`. `--cross-dir` is a
`validate-cross-solver.sh` work directory (`ressim/` and `flow/` inside), for the small-direct
decks. Writes, per small-direct deck, JutulDarcy against Flow and ResSim FIM (sparse LU) against
JutulDarcy: worst cell differences over all report steps and signed cumulative differences. For
the SPE1 and gas_drive decks, JutulDarcy's checkpoint series, which `benchmarks.py` compares with
the ResSim and Flow series recorded by the Rust tests.

Runs with the system `python3`, like compare_small_direct.py, whose Flow readers it reuses.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "opm_flow"))
from compare_small_direct import field_error, flow_fields  # noqa: E402
from opm.io.ecl import ESmry  # noqa: E402

# Series decks: JutulDarcy case name -> (benchmarks section, case, {quantity: summary key}, times).
# Only state and rate quantities: JutulDarcy 0.3.7's summary cumulatives are the end-of-step rate
# times the report interval (gas_drive FOPT at 10 d is FOPR(10 d) x 10 d exactly), not an integral
# over its internal substeps, so they are wrong wherever rates change within a report step.
# SPE1 is not run: JutulDarcy ignores DRSDT, so it cannot run Case 1 (it runs Case 2 physics).
SERIES = {
    "gas_drive": ("three_phase", "gas_drive", {"FPR": "FPR", "FOPR": "FOPR", "FGOR": "FGOR"},
                  (10, 20, 30, 50, 100, 150, 200, 300, 400, 500, 600)),
}
# End-of-run rates compared on the small-direct decks: Flow summary key -> ResSim history column.
RATES = {"FOPR": 1, "FWPR": 2, "FGPR": 3}
INJECTION = ("FWIR", "FGIR")
# A keyword JutulDarcy ignores that changes the discrete model: the comparison is then shown but
# is not a same-model one, so it is not flagged as a gap.
MODEL_CHANGING = {"DRSDT"}


def reference(run: dict) -> str:
    ignored = run["ignored_keywords"]
    note = f", ignores {', '.join(ignored)}" if ignored else ""
    return f"JutulDarcy {run['jutuldarcy']}{note}"


def jutul_fields(run: dict) -> list[dict]:
    return [{k: np.array(f[k]) for k in ("p", "sw", "sg")} for f in run["fields"]]


def final_rates(field: dict) -> dict:
    return {k: float(v[-1]) for k, v in field.items() if k in (*RATES, *INJECTION)}


def signed_rel(a: float, b: float) -> float:
    return (a - b) / max(abs(b), 1e-12)


def small_direct_records(case: str, run: dict, cross: Path) -> list[dict]:
    out = []
    ref = reference(run)
    same = not (set(run["ignored_keywords"]) & MODEL_CHANGING)

    def rec(name, value, unit, reference_):
        out.append({"kind": "metric", "section": "jutul", "case": case, "metric": name, "value": value,
                    "band": None, "unit": unit, "reference": reference_, "same_model": same, "at": ""})

    ours = jutul_fields(run)
    rates = final_rates(run["field"])
    # Rates compared as percentages: those above 0.1 % of the case's largest (as for cumulatives
    # in compare_small_direct.py), so a trace rate's percentage is not graded.
    largest = max((abs(v) for v in rates.values()), default=0.0)
    graded = {k for k, v in rates.items() if abs(v) > 1e-3 * largest}
    flow_out = cross / "flow" / case
    if flow_out.exists():
        flow = flow_fields(flow_out)
        if len(flow) == len(ours):
            err = field_error(ours, flow)["worst"]
            for key in ("p", "sw", "sg"):
                rec(f"jutul_vs_flow.worst.{key}", err[key], "bar" if key == "p" else "frac",
                    f"OPM Flow vs {ref}")
        smry = ESmry(str(flow_out / "CASE.SMSPEC"))
        for key in sorted(graded & set(smry.keys())):
            rec(f"jutul_vs_flow.final_{key}_rel_err", signed_rel(rates[key], float(smry[key][-1])), "frac",
                f"OPM Flow vs {ref}")
    fim = cross / "ressim" / f"{case}.sparse.json"
    if fim.exists():
        ressim = json.loads(fim.read_text())
        fields = [{k: np.array(r[k]) for k in ("p", "sw", "sg")} for r in ressim["reports"]]
        if len(fields) == len(ours):
            err = field_error(fields, ours)["worst"]
            for key in ("p", "sw", "sg"):
                rec(f"fim_vs_jutul.worst.{key}", err[key], "bar" if key == "p" else "frac", ref)
        last = ressim["history"][-1]
        ressim_rates = {k: last[col] for k, col in RATES.items()}
        # The history's injection column is whatever the case injects, at surface conditions.
        ressim_rates["FGIR" if ressim.get("injected") == "gas" else "FWIR"] = last[4]
        for key in sorted(graded & set(ressim_rates)):
            rec(f"fim_vs_jutul.final_{key}_rel_err", signed_rel(ressim_rates[key], rates[key]), "frac", ref)
    rec("wall_s", run["wall_s"], "s", ref)
    return out


def series_records(name: str, run: dict) -> list[dict]:
    section, case, quantities, times = SERIES[name]
    t = np.array(run["time_days"])
    picked = [int(np.argmin(abs(t - x))) for x in times]
    return [{"kind": "series", "section": section, "case": case, "source": "jutul", "quantity": q,
             "t": [float(t[i]) for i in picked], "v": [float(run["field"][key][i]) for i in picked]}
            for q, key in quantities.items()]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--jutul-dir", type=Path, required=True)
    parser.add_argument("--cross-dir", type=Path, required=True)
    parser.add_argument("--records", type=Path, required=True)
    args = parser.parse_args()
    records = []
    for path in sorted(args.jutul_dir.glob("*.json")):
        run = json.loads(path.read_text())
        name = run["case"]
        records += series_records(name, run) if name in SERIES else small_direct_records(name, run, args.cross_dir)
    with args.records.open("a") as f:
        for r in records:
            f.write(json.dumps(r) + "\n")
    print(f"jutul: {len(records)} records from {len(list(args.jutul_dir.glob('*.json')))} runs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
