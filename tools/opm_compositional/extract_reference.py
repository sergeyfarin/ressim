#!/usr/bin/env python3
"""Extract a compositional reference run into a small machine-readable fixture.

Reads the formatted restart (`.FUNRST`, produced by `convertECL`) and the summary, and writes one
JSON file holding the per-cell trajectory C12 compares against.

Deliberately stdlib-only. A parser for an ASCII format this regular does not justify a dependency,
and the fixture has to be reproducible from a bare checkout.

The formatted restart is a sequence of records:

    'KEYWORD '   <count> '<TYPE>'
    <count values, free-form across lines>

Report steps are delimited by `SEQNUM`. Everything between one `SEQNUM` and the next belongs to
that step.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

# Per-cell arrays worth carrying. Names are the restart's; the units are the deck's METRIC ones:
# pressure in bar, densities in kg/m3, viscosities in cP, saturations and mole fractions
# dimensionless.
CELL_KEYWORDS = [
    "PRESSURE",
    "SGAS",
    "SOIL",
    "XMF1", "XMF2", "XMF3",
    "YMF1", "YMF2", "YMF3",
    "ZMF1", "ZMF2", "ZMF3",
    "OIL_DEN", "GAS_DEN",
    "OIL_VISC", "GAS_VISC",
]

HEADER = re.compile(r"^\s*'(?P<name>.{1,8}?)\s*'\s+(?P<count>\d+)\s+'(?P<type>\w+)\s*'\s*$")


def parse_formatted_restart(path: Path) -> list[dict]:
    """Return one dict of keyword -> list per report step."""
    steps: list[dict] = []
    current: dict | None = None
    pending_name: str | None = None
    pending_count = 0
    pending_type = ""
    values: list = []

    def flush() -> None:
        nonlocal pending_name, values
        if pending_name is None:
            return
        if pending_name == "SEQNUM":
            steps.append({"SEQNUM": values[0] if values else None})
        elif current is not None and pending_name in CELL_KEYWORDS:
            current[pending_name] = values
        pending_name = None
        values = []

    with path.open() as handle:
        for line in handle:
            match = HEADER.match(line)
            if match:
                flush()
                pending_name = match.group("name").strip()
                pending_count = int(match.group("count"))
                pending_type = match.group("type").strip()
                values = []
                if pending_name == "SEQNUM":
                    # The next record belongs to a new step; start it now so the flush lands right.
                    pass
                continue
            if pending_name is None:
                continue
            if pending_type in ("INTE",):
                values.extend(int(tok) for tok in line.split())
            elif pending_type in ("REAL", "DOUB"):
                values.extend(float(tok.replace("D", "E")) for tok in line.split())
            else:
                # CHAR / LOGI / MESS: not needed, and their tokenization is quote-sensitive.
                pass
            if len(values) >= pending_count and pending_name != "SEQNUM":
                flush()
            elif pending_name == "SEQNUM" and len(values) >= pending_count:
                steps.append({"SEQNUM": values[0]})
                current = steps[-1]
                pending_name = None
                values = []
    flush()
    return steps


def read_summary(smspec: Path, vectors: list[str]) -> dict[str, list[float]]:
    """Read summary vectors through opm-common's `summary` tool.

    `TIME` is requested first so every column is aligned to it. Vectors that come back identically
    zero are reported to stderr rather than silently carried: `flowexp_comp` does not populate the
    block vectors or `FPR`, and a column of zeros that looks like data is worse than an absent one.
    """
    requested = ["TIME", *vectors]
    result = subprocess.run(
        ["summary", "-n", str(smspec), *requested],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(f"summary failed: {result.stderr.strip()}")

    out: dict[str, list[float]] = {name: [] for name in requested}
    for line in result.stdout.splitlines():
        parts = line.split()
        if len(parts) != len(requested):
            continue
        for index, name in enumerate(requested):
            out[name].append(float(parts[index]))

    for name, values in out.items():
        if values and all(v == 0.0 for v in values):
            print(
                f"note: {name} is identically zero — flowexp_comp does not populate it",
                file=sys.stderr,
            )
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("case", type=Path, help="case stem, e.g. path/to/1D_COMP")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--cells", type=int, required=True)
    parser.add_argument(
        "--summary-vectors",
        nargs="*",
        default=["WBHP:INJ", "WBHP:PROD", "FGIT", "FGPT", "FOPT", "FGIR", "FGPR", "FOPR"],
    )
    args = parser.parse_args()

    funrst = args.case.with_suffix(".FUNRST")
    if not funrst.exists():
        unrst = args.case.with_suffix(".UNRST")
        if not unrst.exists():
            raise SystemExit(f"neither {funrst} nor {unrst} exists")
        subprocess.run(["convertECL", str(unrst)], check=True, capture_output=True)

    steps = parse_formatted_restart(funrst)
    steps = [s for s in steps if "PRESSURE" in s]
    if not steps:
        raise SystemExit("no report step carried a PRESSURE array")

    for index, step in enumerate(steps):
        for keyword in CELL_KEYWORDS:
            if keyword not in step:
                raise SystemExit(f"step {index} is missing {keyword}")
            if len(step[keyword]) != args.cells:
                raise SystemExit(
                    f"step {index} {keyword} has {len(step[keyword])} values, "
                    f"expected {args.cells}"
                )

    summary = read_summary(args.case.with_suffix(".SMSPEC"), args.summary_vectors)

    document = {
        "schema": "ressim-compositional-reference/1",
        "case": args.case.name,
        "cells": args.cells,
        "units": {
            "pressure": "bar",
            "density": "kg/m3",
            "viscosity": "cP",
            "time": "days",
        },
        "note": (
            "Generated from OPM's flowexp_comp. Mole fractions are XMF (liquid), YMF (vapour) "
            "and ZMF (overall), indexed 1..N in the deck's CNAMES order. OIL_DEN, GAS_DEN, "
            "OIL_VISC and GAS_VISC are requested by the deck's RPTRST but are written as zeros "
            "by this simulator, so they are NOT usable as reference values; the same is true of "
            "FPR and every block summary vector."
        ),
        "report_steps": [
            {
                "sequence": step["SEQNUM"],
                **{keyword.lower(): step[keyword] for keyword in CELL_KEYWORDS},
            }
            for step in steps
        ],
        "summary": summary,
    }

    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w") as handle:
        json.dump(document, handle, indent=1, sort_keys=False)
        handle.write("\n")
    print(f"wrote {args.out}: {len(steps)} report steps, {args.cells} cells", file=sys.stderr)


if __name__ == "__main__":
    main()
