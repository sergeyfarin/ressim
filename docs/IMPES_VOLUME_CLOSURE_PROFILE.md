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
