# water-heavy-step1

OPM Flow reference deck for `FIM-DIAG-003` D3 (`docs/FIM_DIAG_003_PLAN.md`). Mirrors the native
heavy-case repro driver:

```
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture
```

12x12x3, perms 2000/2000/200 mD, corner wells (injector BHP 500, producer BHP 100), `TSTEP 1.0`.
Adapted from `water-medium-step1` (`origin/fim-opm-continuation-plan`) with `DIMENS 12 12 3` and
ResSim well radius `0.1`. Eclipse `COMPDAT` specifies well **diameter**, so the matching deck
value is `0.2`. The earlier `0.1` deck value accidentally modeled radius `0.05`; its water-heavy
iteration and rate comparisons are superseded by the corrected-oracle rerun.

Run (copy to a scratch directory first so Flow's restart/summary/report output files don't dirty
the tree):

```bash
mkdir -p /tmp/opm-water-heavy-step1 && cp CASE.DATA /tmp/opm-water-heavy-step1/
cd /tmp/opm-water-heavy-step1
/usr/bin/flow CASE.DATA --output-extra-convergence-info=steps,iterations
```

Produces `CASE.INFOSTEP` (per-report-step Newton/linear iteration counts) and `CASE.INFOITER`
(per-Newton-iteration CNV/MB/well-status trajectory) alongside the usual restart/summary files.

Corrected-oracle result recorded in `docs/FIM_CONVERGENCE_WORKLOG.md` WATER-001: Flow solves the
whole interval in one step (11 Newton, 12 linearizations, 13 linear iterations, zero cuts).
Evaluation-0 water MB/CNV is `0.83078/358.90`, matching ResSim `0.8308/358.9`; the trajectories
separate only after the first update. The earlier radius-0.05 run's `14` linear iterations and
`0.67425` initial MB are retained only as superseded history.

WATER-002 maps the corrected evaluation-0 matrix and raw update. Flow's exported solve and its
own maximum-update diagnostic agree on injector `dp=+110.703 bar` and raw `dSw=+160.361` before
chopping. ResSim instead computes `dp=-196.932 bar`, then applies its `-90 bar` pressure safeguard.
The first mapped discrepancy is the BHP-well linearization lifecycle; see the worklog and
`opm/diagnostics/README.md` for replay commands and units.

WATER-003 identified the specific property contract: Flow's SWOF law is constant-extended at
the exact `Sw=0.10` endpoint, so its AD derivative is zero. ResSim's scalar Corey derivative was
also zero there, but its generic AD oil derivative remained live. The default-off coherent replay
matches Flow's evaluation-1 water MB (`0.313756` versus `0.31375`) without removing well coupling.
