# water-heavy-step1

OPM Flow reference deck for `FIM-DIAG-003` D3 (`docs/FIM_DIAG_003_PLAN.md`). Mirrors the native
heavy-case repro driver:

```
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture
```

12x12x3, perms 2000/2000/200 mD, corner wells (injector BHP 500, producer BHP 100), `TSTEP 1.0`.
Adapted from `water-medium-step1` (`origin/fim-opm-continuation-plan`) with `DIMENS 12 12 3` and
well radius `0.1` (not that template's `0.2` — verified against the actual `add_well` call sites
in both the repro driver and the `fim-wasm-diagnostic.mjs` preset, which both use `0.1`).

Run (copy to a scratch directory first so Flow's restart/summary/report output files don't dirty
the tree):

```bash
mkdir -p /tmp/opm-water-heavy-step1 && cp CASE.DATA /tmp/opm-water-heavy-step1/
cd /tmp/opm-water-heavy-step1
/usr/bin/flow CASE.DATA --output-extra-convergence-info=steps,iterations
```

Produces `CASE.INFOSTEP` (per-report-step Newton/linear iteration counts) and `CASE.INFOITER`
(per-Newton-iteration CNV/MB/well-status trajectory) alongside the usual restart/summary files.

Result recorded in `docs/FIM_CONVERGENCE_WORKLOG.md` "FIM-DIAG-003 checkpoint D3": OPM solves the
whole interval in one Newton solve (11 iterations, zero timestep cuts), and its own per-iteration
MB trajectory transits the exact `1e-7`-`2e-7` magnitude ResSim is frozen at with one further
clean Newton step (2-3 orders of magnitude drop) — independent oracle confirmation of `FIM-DIAG-003` H1.
