#!/usr/bin/env python3
"""Run the shared case through the native Python bindings and compare against the wasm reference.

The claim under test is narrow and worth stating exactly: *the native binding exposes the same
engine as the browser binding*. It is not a physics check — `benchmark_buckley` in the Rust suite
owns that, and this case is its fixture precisely so the two cannot drift apart.

Both runs are the same Rust code compiled for two targets, so the bar is tight rather than
statistical. Any real disagreement means a binding is lying about what it forwards.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).parent
TOL = 1e-9  # same source, two targets; this is a floating-point courtesy, not a tolerance budget


def main() -> int:
    case = json.loads((HERE / "case.json").read_text())
    reference_path = HERE / "reference_wasm.json"
    if not reference_path.exists():
        print(f"missing {reference_path.name}; run run_wasm.mjs first", file=sys.stderr)
        return 2
    reference = json.loads(reference_path.read_text())

    import ressim

    sim = ressim.Simulator(case["nx"], 1, 1, case["porosity"])
    sim.set_fim_enabled(False)
    sim.set_rel_perm_props(case["s_wc"], case["s_or"], case["n_w"], case["n_o"], 1.0, 1.0)
    sim.set_initial_saturation(case["s_wc"])
    sim.set_permeability_random_seeded(case["permeability_md"], case["permeability_md"], case["seed"])
    sim.set_stability_params(0.05, 75.0, 0.75)
    sim.set_capillary_params(0.0, 2.0)
    sim.set_fluid_properties(case["mu_o"], case["mu_w"])
    sim.add_well(0, 0, 0, case["injector_bhp"], 0.1, 0.0, True)
    sim.add_well(case["nx"] - 1, 0, 0, case["producer_bhp"], 0.1, 0.0, False)

    for _ in range(case["steps"]):
        sim.step(case["dt_days"])

    failures: list[str] = []
    for name, native, expected in (
        ("pressure", sim.pressures(), reference["pressures"]),
        ("sat_water", sim.sat_water(), reference["satWater"]),
    ):
        if len(native) != len(expected):
            failures.append(f"{name}: {len(native)} cells natively vs {len(expected)} in wasm")
            continue
        worst, where = 0.0, -1
        for i, (a, b) in enumerate(zip(native, expected)):
            d = abs(a - b)
            if d > worst:
                worst, where = d, i
        print(f"  {name:9s} max |native - wasm| = {worst:.3e}  (cell {where})")
        if worst > TOL:
            failures.append(f"{name}: {worst:.3e} at cell {where} exceeds {TOL:.0e}")

    # Reporting payloads. These were JsValue-only until Phase 1, so a native consumer could run a
    # case but not say what came out of it; comparing them is what makes this gate cover the part
    # of the engine a reservoir engineer actually reads.
    if tuple(sim.dimensions()) != tuple(reference["dimensions"]):
        failures.append(f"dimensions: {sim.dimensions()} natively vs {reference['dimensions']} in wasm")

    native_history = sim.rate_history()
    wasm_history = reference["rateHistory"]
    if len(native_history) != len(wasm_history):
        failures.append(f"rate history: {len(native_history)} points natively vs {len(wasm_history)} in wasm")
    else:
        worst_field, worst_delta = None, 0.0
        for i, (a, b) in enumerate(zip(native_history, wasm_history)):
            if a.keys() != b.keys():
                failures.append(f"rate history point {i}: field sets differ")
                break
            for key in a:
                if not isinstance(a[key], (int, float)) or isinstance(a[key], bool):
                    if a[key] != b[key]:
                        failures.append(f"rate history point {i} field {key}: {a[key]!r} vs {b[key]!r}")
                    continue
                d = abs(a[key] - b[key])
                if d > worst_delta:
                    worst_delta, worst_field = d, f"{key} (point {i})"
        print(f"  rates     max |native - wasm| = {worst_delta:.3e}  ({worst_field})")
        print(f"  history   {len(native_history)} points, {len(native_history[0])} fields each")
        if worst_delta > TOL:
            failures.append(f"rate history: {worst_delta:.3e} in {worst_field} exceeds {TOL:.0e}")

    # Parity has to be measured on a field that varies. A static field agrees trivially, and so
    # does a fully swept one; the case is tuned so the front sits mid-domain at the last step.
    moved = sum(1 for s in sim.sat_water() if s > case["s_wc"] + 1e-6)
    print(f"  front        {moved} of {case['nx']} cells above connate water")
    if not 2 <= moved < case["nx"]:
        failures.append(
            f"front swept {moved} of {case['nx']} cells: parity on a uniform field proves nothing. "
            "Retune case.json's step count so the front stays inside the grid."
        )

    if failures:
        print("\nFAIL:", file=sys.stderr)
        for f in failures:
            print(f"  {f}", file=sys.stderr)
        return 1
    print("\nnative binding matches the wasm bindings on the committed Buckley case A fixture")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
