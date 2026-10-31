#!/usr/bin/env python3
"""Richardson self-convergence referee for a ResSim grid ladder.

This compares the same physical problem at successive grid levels and needs no external
simulator: it reads the `<case>.<run>.json` records written by
`opm_small_direct_run_ressim`, reduces each level to a few scalar observables, and reports the
successive difference, contraction ratio, observed order and Richardson-extrapolated value.

The shipped ladder is the uniform `bo-1d` depletion column at 10, 20 and 40 cells, so the
arithmetic field mean is also the pore-volume-weighted mean. Cumulative oil comes from the
record's own history, exactly as the cross-solver comparator integrates it. Each level must carry
the complete 20-report/100-day schedule: a truncated run (`OPM_SMALL_REPORT_LIMIT`) is rejected
before any convergence judgment, because three equally incomplete runs are not a grid study.

The three levels must have a constant cell-count ratio; the observed order uses that ratio rather
than assuming a doubling. `--levels 10 10 10` and `10 20 30` are rejected, while `10 20 40` and
`10 30 90` are accepted.

    python3 tools/opm_flow/grid_refinement_referee.py --ressim-dir /tmp/small-grid
    python3 tools/opm_flow/grid_refinement_referee.py --ressim-dir /tmp/small-grid \
        --levels 10 20 40 --solvers sparse impes

Exits non-zero when a sequence does not contract: the finer difference must be at most 0.8 of the
coarser one, the same rule `physics_depletion_grid_convergence_*` uses. An exactly converged
finest pair (`d2 == 0`) passes with zero contraction, as it does in the Rust check.
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


def level_metrics(doc: dict) -> tuple[dict[str, float], float, int]:
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
    return values, float(last["t"]), len(reports)


def refinement_ratio(levels: list[int]) -> float:
    """Validate a three-level ladder and return its constant cell-count ratio."""
    if len(levels) != 3:
        raise SystemExit("exactly three grid levels are required for Richardson order")
    if any(level <= 0 for level in levels):
        raise SystemExit(f"grid levels must be positive: {levels}")
    if not levels[0] < levels[1] < levels[2]:
        raise SystemExit(f"grid levels must be strictly increasing: {levels}")
    ratio = levels[1] / levels[0]
    second = levels[2] / levels[1]
    if abs(ratio - second) > 1e-9 * max(ratio, second):
        raise SystemExit(
            f"grid levels need a constant refinement ratio; {levels} has "
            f"{ratio:g} then {second:g}"
        )
    return ratio


def richardson(values: list[float], ratio: float) -> dict:
    """Contraction, observed order, Richardson extrapolation and the pass verdict.

    `ratio` is the constant cell-count ratio between successive levels. An exactly converged
    finest pair (`d2 == 0`) passes: its contraction and fine gap are zero even though no observed
    order can be estimated, matching `physics_depletion_grid_convergence_*`.
    """
    coarse, medium, fine = values
    d1 = medium - coarse
    d2 = fine - medium
    if d1 == 0.0 and d2 == 0.0:
        return {"d1": 0.0, "d2": 0.0, "contraction": 0.0, "order": None,
                "extrapolated": fine, "fine_gap": 0.0, "monotone": True, "passes": True}
    if d1 == 0.0:
        return {"d1": 0.0, "d2": d2, "contraction": math.inf, "order": None,
                "extrapolated": None, "fine_gap": None, "monotone": False, "passes": False}

    contraction = abs(d2 / d1)
    if d2 == 0.0:
        return {"d1": d1, "d2": 0.0, "contraction": 0.0, "order": None,
                "extrapolated": fine, "fine_gap": 0.0, "monotone": True, "passes": True}

    monotone = d1 * d2 > 0.0
    order = None
    extrapolated = None
    fine_gap = None
    if monotone:
        order = math.log(abs(d1 / d2), ratio)
        denominator = ratio**order - 1.0
        if abs(denominator) > 1e-12:
            extrapolated = fine + (fine - medium) / denominator
            fine_gap = abs(fine - extrapolated) / max(abs(extrapolated), 1e-300)
        elif order == 0.0:
            # Equal successive differences: no contraction, so this cannot pass below.
            fine_gap = None
    passes = monotone and order is not None and contraction <= CONTRACTION_LIMIT
    return {"d1": d1, "d2": d2, "contraction": contraction, "order": order,
            "extrapolated": extrapolated, "fine_gap": fine_gap, "monotone": monotone,
            "passes": passes}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ressim-dir", type=Path, required=True)
    parser.add_argument("--levels", type=int, nargs="+", default=[10, 20, 40])
    parser.add_argument("--template", default="bo-1d-{nx}",
                        help="case key template; {nx} is the grid level (default: bo-1d-{nx})")
    parser.add_argument("--solvers", nargs="+", default=["sparse", "impes"],
                        help="run ids to referee (default: sparse impes)")
    parser.add_argument("--expected-reports", type=int, default=20,
                        help="reports required from each level (default: 20 for the bo-1d ladder)")
    parser.add_argument("--horizon-days", type=float, default=100.0,
                        help="required final time in days (default: 100 for the bo-1d ladder)")
    args = parser.parse_args()
    ratio = refinement_ratio(args.levels)

    failed = False
    for solver in args.solvers:
        cases = [args.template.format(nx=nx) for nx in args.levels]
        records = [load_record(args.ressim_dir / f"{case}.{solver}.json") for case in cases]
        metrics = [level_metrics(doc) for doc in records]
        times = [m[1] for m in metrics]
        counts = [m[2] for m in metrics]
        for case, count in zip(cases, counts):
            if count != args.expected_reports:
                raise SystemExit(
                    f"{solver}/{case}: {count} report(s), expected {args.expected_reports}; "
                    "the schedule is incomplete (OPM_SMALL_REPORT_LIMIT?)"
                )
        if max(times) - min(times) > 1e-9 * max(1.0, max(abs(t) for t in times)):
            raise SystemExit(f"{solver}: grid levels end at different times: {times}")
        if abs(times[-1] - args.horizon_days) > 1e-9 * max(1.0, abs(args.horizon_days)):
            raise SystemExit(
                f"{solver}/{cases[-1]}: ends at {times[-1]:g} d, expected {args.horizon_days:g} d; "
                "the schedule is incomplete"
            )

        print(f"\n{solver}: {', '.join(cases)} at t={times[-1]:.10g} d, ratio {ratio:g}")
        diff1 = f"{args.levels[0]}->{args.levels[1]}"
        diff2 = f"{args.levels[1]}->{args.levels[2]}"
        print(f"  {'observable':18} {diff1:>12} {diff2:>12} {'contraction':>12} "
              f"{'order':>7} {'fine':>13} {'extrapolated':>14} {'fine gap':>9}")
        for label, key, unit in OBSERVABLES:
            values = [m[0][key] for m in metrics]
            result = richardson(values, ratio)
            contraction = result["contraction"]
            order = "—" if result["order"] is None else f"{result['order']:.3f}"
            extrapolated = "—" if result["extrapolated"] is None else f"{result['extrapolated']:.8g}"
            fine_gap = "—" if result["fine_gap"] is None else f"{result['fine_gap']:.2%}"
            failed |= not result["passes"]
            print(f"  {label:18} {result['d1']:>12.6g} {result['d2']:>12.6g} "
                  f"{contraction:>12.3f} {order:>7} {values[-1]:>13.8g} {extrapolated:>14} "
                  f"{fine_gap:>9}  {'ok' if result['passes'] else 'FAIL'} ({unit})")

    if failed:
        print(f"\ngrid-refinement referee: FAILED (a sequence did not contract within "
              f"{CONTRACTION_LIMIT:g} per refinement step)")
        return 1
    print(f"\ngrid-refinement referee: OK (every sequence contracted within "
          f"{CONTRACTION_LIMIT:g} per refinement step)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
