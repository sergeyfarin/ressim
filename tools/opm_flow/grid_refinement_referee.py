#!/usr/bin/env python3
"""Richardson self-convergence referee for a ResSim grid ladder.

This compares the same physical problem at successive grid levels and needs no external
simulator: it reads the `<case>.<run>.json` records written by
`opm_small_direct_run_ressim`, reduces each level to a few scalar observables, and reports the
successive difference, contraction ratio, observed order and Richardson-extrapolated value.

The shipped ladder is the uniform `bo-1d` depletion column at 10, 20 and 40 cells, so the
arithmetic field mean is also the pore-volume-weighted mean. Cumulative oil comes from the
record's own history, exactly as the cross-solver comparator integrates it.

    python3 tools/opm_flow/grid_refinement_referee.py --ressim-dir /tmp/small-grid
    python3 tools/opm_flow/grid_refinement_referee.py --ressim-dir /tmp/small-grid \
        --levels 10 20 40 --solvers sparse impes

Exits non-zero when a sequence does not contract: the finer difference must be at most 0.8 of the
coarser one, the same rule `physics_depletion_grid_convergence_*` uses.
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

import numpy as np

from compare_small_direct import ressim_cumulatives

CONTRACTION_LIMIT = 0.8
OBSERVABLES = (
    ("pressure", "p", "bar"),
    ("gas saturation", "sg", "frac"),
    ("dissolved gas", "rs", "Sm3/Sm3"),
    ("cumulative oil", "FOPT", "Sm3"),
)


def load_record(path: Path) -> dict:
    if not path.exists():
        raise SystemExit(f"missing grid-level record: {path}")
    return json.loads(path.read_text())


def level_metrics(doc: dict) -> tuple[dict[str, float], float]:
    reports = doc.get("reports") or []
    if not reports:
        raise SystemExit("grid-level record has no reports")
    last = reports[-1]
    missing = [key for key in ("p", "sg", "rs") if key not in last]
    if missing:
        raise SystemExit(f"grid-level record is missing {', '.join(missing)}")
    cumulative = ressim_cumulatives(doc)
    values = {
        "p": float(np.mean(last["p"])),
        "sg": float(np.mean(last["sg"])),
        "rs": float(np.mean(last["rs"])),
        "FOPT": cumulative["FOPT"],
    }
    return values, float(last["t"])


def richardson(values: list[float]) -> dict:
    """Contraction, observed order and Richardson extrapolation for three levels."""
    coarse, medium, fine = values
    d1 = medium - coarse
    d2 = fine - medium
    if d1 == 0.0 and d2 == 0.0:
        return {"d1": 0.0, "d2": 0.0, "contraction": 0.0, "order": None,
                "extrapolated": fine, "fine_gap": 0.0, "monotone": True}
    if d1 == 0.0:
        return {"d1": 0.0, "d2": d2, "contraction": math.inf, "order": None,
                "extrapolated": None, "fine_gap": None, "monotone": False}

    contraction = abs(d2 / d1)
    monotone = d1 * d2 > 0.0
    order = None
    extrapolated = None
    fine_gap = None
    if d2 == 0.0:
        extrapolated = fine
        fine_gap = 0.0
    elif monotone:
        order = math.log(abs(d1 / d2), 2.0)
        denominator = 2.0**order - 1.0
        if abs(denominator) > 1e-12:
            extrapolated = fine + (fine - medium) / denominator
            fine_gap = abs(fine - extrapolated) / max(abs(extrapolated), 1e-300)
    return {"d1": d1, "d2": d2, "contraction": contraction, "order": order,
            "extrapolated": extrapolated, "fine_gap": fine_gap, "monotone": monotone}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ressim-dir", type=Path, required=True)
    parser.add_argument("--levels", type=int, nargs="+", default=[10, 20, 40])
    parser.add_argument("--template", default="bo-1d-{nx}",
                        help="case key template; {nx} is the grid level (default: bo-1d-{nx})")
    parser.add_argument("--solvers", nargs="+", default=["sparse", "impes"],
                        help="run ids to referee (default: sparse impes)")
    args = parser.parse_args()
    if len(args.levels) != 3:
        parser.error("exactly three grid levels are required for Richardson order")

    failed = False
    for solver in args.solvers:
        cases = [args.template.format(nx=nx) for nx in args.levels]
        records = [load_record(args.ressim_dir / f"{case}.{solver}.json") for case in cases]
        metrics = [level_metrics(doc) for doc in records]
        times = [t for _, t in metrics]
        if max(times) - min(times) > 1e-9 * max(1.0, max(abs(t) for t in times)):
            raise SystemExit(f"{solver}: grid levels end at different times: {times}")

        print(f"\n{solver}: {', '.join(cases)} at t={times[-1]:.10g} d")
        diff1 = f"{args.levels[0]}->{args.levels[1]}"
        diff2 = f"{args.levels[1]}->{args.levels[2]}"
        print(f"  {'observable':18} {diff1:>12} {diff2:>12} {'contraction':>12} "
              f"{'order':>7} {'fine':>13} {'extrapolated':>14} {'fine gap':>9}")
        for label, key, unit in OBSERVABLES:
            values = [m[0][key] for m in metrics]
            result = richardson(values)
            contraction = result["contraction"]
            order = "—" if result["order"] is None else f"{result['order']:.3f}"
            extrapolated = "—" if result["extrapolated"] is None else f"{result['extrapolated']:.8g}"
            fine_gap = "—" if result["fine_gap"] is None else f"{result['fine_gap']:.2%}"
            ok = result["monotone"] and contraction <= CONTRACTION_LIMIT
            failed |= not ok
            print(f"  {label:18} {result['d1']:>12.6g} {result['d2']:>12.6g} "
                  f"{contraction:>12.3f} {order:>7} {values[-1]:>13.8g} {extrapolated:>14} "
                  f"{fine_gap:>9}  {'ok' if ok else 'FAIL'} ({unit})")

    if failed:
        print(f"\ngrid-refinement referee: FAILED (a sequence did not contract within "
              f"{CONTRACTION_LIMIT:g} per halving)")
        return 1
    print(f"\ngrid-refinement referee: OK (every sequence contracted within "
          f"{CONTRACTION_LIMIT:g} per halving)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
