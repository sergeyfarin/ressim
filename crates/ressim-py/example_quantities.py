#!/usr/bin/env python3
"""End-to-end: run a case natively, then read the quantities the application plots.

This is S6's deliverable in miniature — the shape a notebook takes. Nothing here draws; the point
is that `label` and `unit` come from `contracts/run-quantities.json`, which is generated from the
TypeScript registry, so an axis titled here says exactly what the app's axis says. Swap the print
loop for `matplotlib` and it is a figure.

Run:  python3 example_quantities.py        (needs ressim.so on the path; see parity/)
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
sys.path.insert(0, str(Path(__file__).parent / "parity"))

import ressim
import ressim_quantities as rq


def main() -> int:
    sim = ressim.Simulator(30, 1, 1, 0.2)
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
    sim.set_initial_saturation(0.1)
    sim.set_permeability_random_seeded(2000.0, 2000.0, 42)
    sim.set_stability_params(0.05, 75.0, 0.75)
    sim.set_capillary_params(0.0, 2.0)
    sim.set_fluid_properties(1.0, 0.5)
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, True)
    sim.add_well(29, 0, 0, 100.0, 0.1, 0.0, False)
    for _ in range(5):
        sim.step(1.0)

    series = rq.quantities(sim.rate_history())

    print(f"  {len(series)} of {len(rq.quantity_ids())} contract quantities computed\n")
    for qid in ("oil-rate", "cumulative-oil", "gas-cut", "average-water-saturation"):
        q = series[qid]
        last = q["values"][-1]
        unit = f" {q['unit']}" if q["unit"] else ""
        print(f"  {q['label']:<16} {last:>12.4f}{unit}   (t = {q['time'][-1]:.2f} d)")

    print("\n  needing inputs a rate history does not carry:")
    for qid, need in sorted(rq.missing_inputs().items()):
        print(f"    {qid:<26} {need}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
