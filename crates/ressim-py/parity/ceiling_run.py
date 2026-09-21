#!/usr/bin/env python3
"""Run a case larger than the browser policy allows, and report what it actually did.

`docs/SOLVER_COMPARISON_SUMMARY.md` holds every browser convergence case to <= 1200 cells and
says not to add cases above ~1500 without a memory-budget check. That ceiling is a property of
the WASM sandbox, not of the solver. This runs the same engine natively to show the limit is
gone, and prints numbers rather than a claim -- "theoretically reachable" is not evidence.

Build in release before timing anything: `cargo build -p ressim-py --release`, then copy
`target/release/libressim.so` here as `ressim.so`. The debug build is roughly two orders of
magnitude slower, and a timing taken from it would say nothing about the engine.

Usage: python3 ceiling_run.py [nx] [ny] [nz] [steps]
"""
from __future__ import annotations

import sys
import time

import ressim


def main() -> int:
    nx = int(sys.argv[1]) if len(sys.argv) > 1 else 100
    ny = int(sys.argv[2]) if len(sys.argv) > 2 else 100
    nz = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    steps = int(sys.argv[4]) if len(sys.argv) > 4 else 5
    cells = nx * ny * nz

    sim = ressim.Simulator(nx, ny, nz, 0.2)
    sim.set_fim_enabled(False)
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
    sim.set_initial_saturation(0.1)
    sim.set_permeability_random_seeded(2000.0, 2000.0, 42)
    sim.set_stability_params(0.05, 75.0, 0.75)
    sim.set_capillary_params(0.0, 2.0)
    sim.set_fluid_properties(1.0, 0.5)
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, True)
    sim.add_well(nx - 1, ny - 1, nz - 1, 100.0, 0.1, 0.0, False)

    t0 = time.perf_counter()
    for _ in range(steps):
        sim.step(1.0)
    elapsed = time.perf_counter() - t0

    sw = sim.sat_water()
    p = sim.pressures()
    swept = sum(1 for s in sw if s > 0.1 + 1e-6)

    print(f"  grid          {nx}x{ny}x{nz} = {cells} cells")
    print(f"  steps         {steps} x 1.0 day")
    print(f"  wall time     {elapsed:.2f} s  ({elapsed / steps:.3f} s/step)")
    print(f"  swept cells   {swept} of {cells}")
    print(f"  pressure      min {min(p):.2f} bar, max {max(p):.2f} bar")
    print(f"  saturation    min {min(sw):.4f}, max {max(sw):.4f}")

    # The run has to be physical, or the size is meaningless.
    ok = (
        all(0.0 <= s <= 1.0 for s in sw)
        and all(p_i > 0.0 for p_i in p)
        and 0 < swept < cells
        and cells > 1500
    )
    print(f"\n  {'OK' if ok else 'FAILED'}: {cells} cells, above the ~1500-cell browser policy")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
