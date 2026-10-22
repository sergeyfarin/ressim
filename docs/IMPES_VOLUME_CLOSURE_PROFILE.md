# Conservative IMPES volume-closure cost (#71)

The two-phase component inventory contract introduced in #63 remains the constraint:
transport surface water and oil independently, then converge their pressure-dependent fluid
volume to pore volume. The combined-sweep scenario continues to default to FIM. This work
changes the cost of the native/browser IMPES implementation, not that solver policy.

## Attribution and mechanism

The opt-in small-direct profiler is compiled only in Rust tests. Enable it with
`OPM_SMALL_PROFILE=1`; `OPM_SMALL_REPORT_LIMIT=1` optionally measures the first scheduled
report without changing its timestep. A prefix capture is diagnostic evidence, not an input
to the full-schedule Flow/inventory gate. Backend timings are nested inside initial/correction
solve timings and must not be added to them.

On committed `c0604f3`, the full `sweep-combined` replay accepted 2,134 substeps without a
warning. There were 4,420 pressure trials, 14,380 linear corrections, 18,572 flash/storage
evaluations and 22,992 transport evaluations. The measured phase attribution was:

| Phase | Wall time (ms) | Share of replay |
|---|---:|---:|
| Pressure assembly | 22,180 | 5.0% |
| Initial matrix and solve | 24,546 | 5.5% |
| Correction matrix assembly | 16,956 | 3.8% |
| Correction solve | 308,889 | 69.0% |
| Transport and FVF evaluation | 64,684 | 14.4% |
| Flash and storage slope | 6,890 | 1.5% |

Replay command (from the repository root):

```bash
OPM_SMALL_PROFILE=1 OPM_SMALL_SOLVER=impes OPM_SMALL_CASE=sweep-combined \
  OPM_SMALL_OUT=/tmp/ressim-71/profile-combined-1 \
  cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  opm_small_direct_run_ressim -- --ignored --nocapture
```

This exploratory run overlapped other test/compile work. Its 447,973 ms total is an attribution
measurement, **not** a controlled speedup baseline. The earlier uninstrumented `6a49d2c`
capture also accepted 2,134 substeps without warnings. Historical issue #71 timings on
`c6f3c0e` are not a controlled timing comparator either.

Committed `160822f` adds backend attribution. Its first-report replay accepted 126 substeps
and attempted 3,020 corrections: 3,398 BiCGSTAB calls and 2,265 sparse-LU calls. Use the same
command above with `OPM_SMALL_REPORT_LIMIT=1` to reproduce that diagnostic. This isolates the
dominant mechanism: BiCGSTAB used machine epsilon as an **absolute** cutoff on dot products
such as `r_hat·r` and `t·t`. Their sizes depend on the RHS; a small closure residual could
force LU before the requested relative residual was reached.

`ilu0_bicgstab_small_corrections_meet_true_relative_residual` exercises one Cartesian system
with RHS scales 1, 1e-4, 1e-8 and 1e-12 and measures `||rhs − A·x|| / ||rhs||` independently.
The former implementation fails this test even at scale 1 during its final corrections.
The repaired implementation rejects zero/nonfinite denominators and validates the finite
solution against the true full-system residual at the unchanged requested tolerance. LU
remains the fallback for actual breakdown, stagnation or a failed residual check.

Two redundant operations were also removed: closure iterations now reuse the fixed CSR
structure and replace its storage diagonal, and return the current transport evaluation
when terminating instead of evaluating it again. The active set and off-diagonal entries
are fixed during that iteration. The small direct-solver waterflood benefits from these
changes even though it does not use BiCGSTAB.

## Runtime context and validation

Measurements were made on 2026-10-22 on Linux 6.8.0-147-generic, x86-64 KVM, an AMD EPYC 3251
host exposing ten CPUs, one thread per exposed core, with no CPU quota in the session cgroup.
The runtime is native Rust release mode, rustc 1.99.0 (`b940084d7`, 2026-09-28), with the
repository's locked dependencies and no custom Rust flags. `perf_event_paranoid=4` prevents
system sampling; the test-only timers supply phase attribution instead.

The independent 1e-8 inventory gate and trajectory/work ratchet remain owned by
[`validate-cross-solver.sh`](../scripts/validate-cross-solver.sh) and the
[small-direct scorecard](../opm/reference-decks/small-direct/README.md). Scientific benchmark
values remain generated in [BENCHMARKS.md](BENCHMARKS.md). The native/browser parity fixture
now includes a 24×24 IMPES case, crossing the 512-row iterative-solver threshold.


## Paired replay results on committed revisions

The [machine-readable measurements](benchmarks/impes-volume-closure-profile.json) contain
binary SHA-256 identifiers, phase timings/calls, inventory errors and report deltas. Tables in
this document are rendered from those measurements. The baseline is committed `160822f` and
the optimization is committed `1f22b82`. This establishes the #71 runtime comparison; the
#63 historical wall times remain context.

Each process was pinned to CPU 9. Solver coverage and the full benchmark workload used CPUs
0–8; the first baseline also overlapped the final product gate. This was a shared host, and
background work accounts for part of the timing spread. The two full pairs ran in baseline /
optimized order. Ten small-waterflood pairs ran between them, alternating baseline-first and
optimized-first order. Every run used the full original report schedule.

| Replay | Baseline (s) | Optimized (s) | Speedup |
|---|---:|---:|---:|
| Combined, pair 1 | 407.5 | 222.2 | 1.83× |
| Combined, pair 2 | 383.7 | 222.3 | 1.73× |
| 12×12 waterflood, median of ten | 0.584 | 0.517 | 1.13× |

Both combined pairs retain all 75 reports, 2,134 accepted substeps, identical per-report
substep counts, 4,420 pressure trials, 14,380 corrections and zero warnings. Sparse-LU calls
fall from 13,110 to zero; transport evaluations fall from 22,992 to 18,800. Worst report pressure
difference is 2.01e-10 bar (rounded upward); worst water-saturation difference is 1.30e-12.
The independently integrated water/oil inventory errors stay below 1.5e-13 of their stated
scales. All ten small pairs retain 40 reports and 227 substeps and are exactly equal after
removing wall time from their captures. Every optimized small run is faster than its paired
baseline.

Build each revision on a clean tree with `cargo test --release --manifest-path
src/lib/ressim/Cargo.toml --lib --no-run` and save the printed release test executable before
switching revisions. The saved local copies were `before-backends` and `after`; their hashes
are recorded in the JSON. The full measurement invocation for the first baseline was:

```bash
OPM_SMALL_PROFILE=1 OPM_SMALL_SOLVER=impes OPM_SMALL_CASE=sweep-combined \
  OPM_SMALL_OUT=/tmp/ressim-71/timing/before-1 \
  taskset -c 9 /tmp/ressim-71/before-backends \
  opm_small_direct_run_ressim --ignored --nocapture \
  > /tmp/ressim-71/timing/before-1.log 2>&1
```

The optimized invocation substitutes `/tmp/ressim-71/after` and `after-1` in the output paths.
The second pair uses suffix `-2`. Small pairs use `OPM_SMALL_CASE=ow-2d-12x12` and output/log
paths under `/tmp/ressim-71/timing/small/{before,after}-{1..10}`. The original schedules are
75 × 5 days for combined and 40 × 0.25 day for the small waterflood.

## Acceptance gates

The implementation passed:

- `cargo test --manifest-path src/lib/ressim/Cargo.toml --lib solvers::` — ten solver contracts,
  including the small-RHS regression and true-residual oracle.
- `bash scripts/validate-solver-coverage.sh all` — 48 executing gate filters, including the
  newly gated `solvers::` module; no ignore-only filter is accepted.
- `cargo test --manifest-path src/lib/ressim/Cargo.toml benchmark_buckley -- --nocapture`.
- `bash scripts/validate-cross-solver.sh` — complete Flow trajectory/work scorecard and
  independent 1e-8 inventories, with the committed scorecard unchanged.
- `bash scripts/validate-native-binding.sh` — all six strict cases have zero array, rate and
  grid differences, including the 576-cell iterative case; restore also passes.
- `pnpm run validate:product` — 1,042 frontend/scenario tests pass, 15 are skipped, followed by
  the IMPES bucket and production build. Node subprocess tests require execution outside the
  sandbox that returned `spawnSync EPERM`; the unrestricted full gate passes.
- `bash scripts/benchmarks.sh check` with the full default tier — all nine sections ran,
  including 20 JutulDarcy decks, the FIM WASM matrix and rederived `flowexp_comp` fixtures.

The full committed replay was run with:

```bash
WASM_PACK_CACHE=/tmp/ressim-71-wasm-pack UV_CACHE_DIR=/tmp/ressim-71-uv \
  BENCH_OUT=/tmp/ressim-71-benchmarks taskset -c 0-8 \
  bash scripts/benchmarks.sh check
```

Its collected records replace the generated benchmark snapshot from `6c65d8f` with `1f22b82`,
so the page represents this implementation and the expanded parity matrix. They were collected
on the clean committed tree using `tools/benchmarks/benchmarks.py collect --run
/tmp/ressim-71-benchmarks` and `render`, the same collection/render steps used by
`benchmarks.sh update`. All prior bands are unchanged. The collected run checks green against
the new records and `bash scripts/benchmarks.sh render --check` passes.

These are native runtime measurements. The combined-sweep FIM default remains in place;
changing that policy requires separate browser validation.
