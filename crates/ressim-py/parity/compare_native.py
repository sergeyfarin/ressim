#!/usr/bin/env python3
"""Run the parity matrix through the native bindings and compare against the wasm reference.

The claim under test is narrow and worth stating exactly: *the native binding exposes the same
engine as the browser binding*. It is not a physics check — `benchmark_buckley` in the Rust suite
owns that, and these cases are built from its fixture precisely so the two cannot drift apart.

Both runs are the same Rust compiled for two targets, so the bar is tight rather than statistical.
Any real disagreement means a binding is lying about what it forwards.

The matrix in `cases.json` varies one axis per case — solver, mobility ratio, grid size — so a
failure names the axis rather than just the fact. Coverage is bounded by what `ressim-py` can
configure, not by what is worth checking; the gap is recorded in
`docs/ARCHITECTURE_SPLIT_PLAN_2026-09-19.md`.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

HERE = Path(__file__).parent
TOL = 1e-9  # same source, two targets; a floating-point courtesy, not a tolerance budget

import ressim  # noqa: E402  - the extension module is copied in beside this file


def run_case(c: dict) -> "ressim.Simulator":
    """Configure and run one case. Keep the call order identical to run_wasm.mjs."""
    sim = ressim.Simulator(c["nx"], c["ny"], c["nz"], c["porosity"])
    sim.set_fim_enabled(bool(c.get("fim")))
    sim.set_rel_perm_props(c["s_wc"], c["s_or"], c["n_w"], c["n_o"], 1.0, 1.0)
    sim.set_initial_saturation(c["s_wc"])
    sim.set_permeability_random_seeded(c["permeability_md"], c["permeability_md"], c["seed"])
    sim.set_stability_params(0.05, 75.0, 0.75)
    sim.set_capillary_params(0.0, 2.0)
    sim.set_fluid_properties(c["mu_o"], c["mu_w"])
    # `add_well_with_id`, not `add_well`: the id selects how build_well_topology groups
    # completions into physical wells, so a client using the other one is not running the same
    # case. The browser only exposes this form, so both runners use it.
    sim.add_well_with_id(0, 0, 0, c["injector_bhp"], 0.1, 0.0, True, "inj")
    sim.add_well_with_id(c["nx"] - 1, 0, 0, c["producer_bhp"], 0.1, 0.0, False, "prod")
    for _ in range(c["steps"]):
        sim.step(c["dt_days"])
    return sim


def worst_array_delta(native, expected) -> tuple[float, int]:
    worst, where = 0.0, -1
    for i, (a, b) in enumerate(zip(native, expected)):
        d = abs(a - b)
        if d > worst:
            worst, where = d, i
    return worst, where


def check_case(c: dict, reference: dict, failures: list[str]) -> None:
    """Compare one case. A non-strict case reports its divergence instead of failing.

    Non-strict existed because wasm32 and x86-64 substepped the FIM cases differently. That was
    FIM-DIRECT-001, now fixed, and every committed case is strict. The mode stays for the next
    measured divergence: reporting keeps its size in front of whoever runs the gate, where
    deleting the case would not.
    """
    name = c["id"]
    strict = bool(c.get("strict", True))
    observations: list[str] = []
    sink = failures if strict else observations
    sim = run_case(c)

    for label, native, expected in (
        ("pressure", sim.pressures(), reference["pressures"]),
        ("sat_water", sim.sat_water(), reference["satWater"]),
    ):
        if len(native) != len(expected):
            sink.append(f"{name}/{label}: {len(native)} cells natively vs {len(expected)} in wasm")
            continue
        worst, where = worst_array_delta(native, expected)
        if worst > TOL:
            sink.append(f"{name}/{label}: {worst:.3e} at cell {where} exceeds {TOL:.0e}")

    if tuple(sim.dimensions()) != tuple(reference["dimensions"]):
        sink.append(f"{name}/dimensions: {sim.dimensions()} vs {reference['dimensions']}")

    # Rate history: the part of the engine a reservoir engineer actually reads. JsValue-only until
    # Phase 1 of the payload-boundary design, so this axis is newly checkable.
    native_history, wasm_history = sim.rate_history(), reference["rateHistory"]
    worst_rate = 0.0
    if len(native_history) != len(wasm_history):
        sink.append(
            f"{name}/rates: {len(native_history)} points natively vs {len(wasm_history)} in wasm"
        )
    else:
        for i, (a, b) in enumerate(zip(native_history, wasm_history)):
            if a.keys() != b.keys():
                sink.append(f"{name}/rates: field sets differ at point {i}")
                break
            for key in a:
                if isinstance(a[key], bool) or not isinstance(a[key], (int, float)):
                    if a[key] != b[key]:
                        sink.append(f"{name}/rates: {key} differs at point {i}")
                    continue
                worst_rate = max(worst_rate, abs(a[key] - b[key]))
        if worst_rate > TOL:
            sink.append(f"{name}/rates: {worst_rate:.3e} exceeds {TOL:.0e}")

    # The portable grid state against the browser's zero-copy one. Exactly the kind of
    # optimization that drifts from its schema unnoticed.
    native_grid, wasm_grid = sim.grid_state(), reference["gridState"]
    worst_grid = 0.0
    if set(native_grid) != set(wasm_grid):
        sink.append(
            f"{name}/grid: fields differ, native {sorted(native_grid)} vs wasm {sorted(wasm_grid)}"
        )
    else:
        for key in native_grid:
            a, b = native_grid[key], wasm_grid[key]
            if a is None or len(a) != len(b):
                sink.append(f"{name}/grid: {key} is {len(a) if a else None} vs {len(b)} values")
                continue
            worst_grid = max(worst_grid, worst_array_delta(a, b)[0])
        if worst_grid > TOL:
            sink.append(f"{name}/grid: {worst_grid:.3e} exceeds {TOL:.0e}")

    # Parity must be measured on a field that varies. A static field agrees trivially, and so does
    # a fully swept one.
    swept = sum(1 for s in sim.sat_water() if s > c["s_wc"] + 1e-6)
    if not 2 <= swept < c["nx"]:
        sink.append(
            f"{name}: front swept {swept} of {c['nx']} cells; parity on a uniform field proves "
            "nothing. Retune this case's step count in cases.json."
        )

    solver = "FIM  " if c.get("fim") else "IMPES"
    worst_cell = max(
        worst_array_delta(sim.pressures(), reference["pressures"])[0],
        worst_array_delta(sim.sat_water(), reference["satWater"])[0],
    )
    mark = "" if strict else "  KNOWN CROSS-TARGET DIVERGENCE"
    print(
        f"  {name:<22} {solver}  cells {worst_cell:.2e}  rates {worst_rate:.2e}  "
        f"grid {worst_grid:.2e}  front {swept}/{c['nx']}{mark}"
    )
    for note in observations:
        print(f"      {note}")
    bench_record(name, "FIM" if c.get("fim") else "IMPES", strict, worst_cell, worst_rate, worst_grid)


def bench_record(name: str, solver: str, strict: bool, cells: float, rates: float, grid: float) -> None:
    """Append this case's worst differences to `$RESSIM_BENCH_OUT/records.jsonl` (#54), if set."""
    out = os.environ.get("RESSIM_BENCH_OUT")
    if not out:
        return
    Path(out).mkdir(parents=True, exist_ok=True)
    with open(Path(out) / "records.jsonl", "a") as f:
        for metric, value in (("cells_max_abs_diff", cells), ("rates_max_abs_diff", rates),
                              ("grid_max_abs_diff", grid)):
            f.write(json.dumps({
                "kind": "metric", "section": "parity", "case": f"{name} ({solver})", "metric": metric,
                "value": value, "band": TOL if strict else None, "unit": "abs",
                "reference": "wasm bindings", "same_model": True, "at": "",
            }) + "\n")


def check_restore(c: dict, failures: list[str]) -> None:
    """Capture a run's state, restore it into a fresh simulator, require indistinguishability."""
    sim = run_case(c)
    restored = ressim.Simulator(c["nx"], c["ny"], c["nz"], c["porosity"])
    try:
        restored.load_state(sim.time_days(), sim.grid_state(), sim.wells(), sim.rate_history())
    except Exception as exc:  # noqa: BLE001 - the failure text is the useful part
        failures.append(f"restore raised: {exc}")
        return
    if restored.pressures() != sim.pressures():
        failures.append("restore: pressures differ from the source run")
    if restored.sat_water() != sim.sat_water():
        failures.append("restore: sat_water differs from the source run")
    if restored.time_days() != sim.time_days():
        failures.append("restore: clock differs")
    print(f"  restore                 {len(restored.rate_history())} points, arrays identical")

    # A restore that accepted a wrong-sized grid would be worse than one that failed.
    mismatched = ressim.Simulator(c["nx"] + 1, c["ny"], c["nz"], c["porosity"])
    try:
        mismatched.load_state(sim.time_days(), sim.grid_state(), sim.wells(), sim.rate_history())
    except ValueError as exc:
        if "Mismatch grid size" not in str(exc):
            failures.append(f"restore rejected a bad grid with an unexpected message: {exc}")
    else:
        failures.append("restore accepted a grid of the wrong size")


def main() -> int:
    spec = json.loads((HERE / "cases.json").read_text())
    reference_path = HERE / "reference_wasm.json"
    if not reference_path.exists():
        print(f"missing {reference_path.name}; run run_wasm.mjs first", file=sys.stderr)
        return 2
    reference = json.loads(reference_path.read_text())

    failures: list[str] = []
    cases = [{**spec["base"], **override} for override in spec["cases"]]

    # Every declared case must be in the reference, or a silently-skipped case looks like a pass.
    missing = [c["id"] for c in cases if c["id"] not in reference]
    if missing:
        print(f"reference is missing cases: {missing}; re-run run_wasm.mjs", file=sys.stderr)
        return 2

    for c in cases:
        check_case(c, reference[c["id"]], failures)
    check_restore(cases[0], failures)

    if failures:
        print("\nFAIL:", file=sys.stderr)
        for f in failures:
            print(f"  {f}", file=sys.stderr)
        return 1

    strict_cases = [c for c in cases if c.get("strict", True)]
    print(
        f"\nnative and wasm bindings agree on {len(strict_cases)} strict cases of {len(cases)}, "
        "built from the committed Buckley case A fixture."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
