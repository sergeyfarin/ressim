#!/usr/bin/env python3
"""OPM Flow oracle for SPE1 under areal refinement (#12).

Derives a 20x20x3 deck from the committed 10x10x3 SPE1 case (`opm_flow_tool.cases.
SPE1_GAS_INJECTION`): same 3048 m x 3048 m domain, half-size cells, producer moved to the far
corner, everything else untouched. Runs Flow on both and prints field pressure, oil rate and the
producer's GOR every 90 days, for comparison with the ResSim replay
`spe1_areal_refinement_reference_error_replay`.

Runs with the system `python3` (it reads Flow's binary summary through `opm.io`, which ships with
Flow), like `compare_small_direct.py`:

    python3 tools/opm_flow/spe1_refinement_oracle.py --out /tmp/spe1-refinement
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

import numpy as np
from opm.io.ecl import ESmry

sys.path.insert(0, str(Path(__file__).resolve().parent))
from opm_flow_tool.cases import SPE1_GAS_INJECTION  # noqa: E402


def refine(deck: str) -> str:
    """The same SPE1 domain at 20x20x3."""
    replacements = [
        ("DIMENS\n  10 10 3 /", "DIMENS\n  20 20 3 /"),
        ("DXV\n  10*304.8 /", "DXV\n  20*152.4 /"),
        ("DYV\n  10*304.8 /", "DYV\n  20*152.4 /"),
        ("DZ\n  100*6.096 100*9.144 100*15.24 /", "DZ\n  400*6.096 400*9.144 400*15.24 /"),
        ("TOPS\n  100*2537.46 /", "TOPS\n  400*2537.46 /"),
        ("PORO\n  300*0.3 /", "PORO\n  1200*0.3 /"),
    ] + [
        (f"{kw}\n  100*500 100*50 100*200 /", f"{kw}\n  400*500 400*50 400*200 /")
        for kw in ("PERMX", "PERMY", "PERMZ")
    ]
    for old, new in replacements:
        if old not in deck:
            raise SystemExit(f"SPE1 deck no longer contains {old!r}; update refine()")
        deck = deck.replace(old, new)
    deck, n_welspecs = re.subn(r"'PROD' 'G' 10 10", "'PROD' 'G' 20 20", deck)
    deck, n_compdat = re.subn(r"'PROD' 10 10 3 3", "'PROD' 20 20 3 3", deck)
    if (n_welspecs, n_compdat) != (1, 1):
        raise SystemExit("could not relocate the producer; update refine()")
    return deck


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    decks = {10: SPE1_GAS_INJECTION.deck, 20: refine(SPE1_GAS_INJECTION.deck)}

    print(f"{'nx':>3} {'t':>7} {'FPR':>9} {'FOPR':>9} {'WGOR:PROD':>10}")
    for nx, deck in decks.items():
        run = args.out / f"spe1_{nx}"
        run.mkdir(parents=True, exist_ok=True)
        (run / "CASE.DATA").write_text(deck)
        subprocess.run(
            ["flow", str(run / "CASE.DATA"), f"--output-dir={run}"],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        summary = ESmry(str(run / "CASE.SMSPEC"))
        time = np.array(summary["TIME"])
        for t in range(90, 3651, 90):
            i = int(np.argmin(abs(time - t)))
            print(f"{nx:3d} {time[i]:7.1f} {summary['FPR'][i]:9.4f} {summary['FOPR'][i]:9.3f} "
                  f"{summary['WGOR:PROD'][i]:10.3f}")


if __name__ == "__main__":
    main()
