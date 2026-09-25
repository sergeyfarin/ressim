# Benchmarks

The current benchmark scorecard: every external or analytical reference ResSim is graded against,
the acceptance band, and the error measured on a committed revision. **Owning documents** hold the
case definitions, methodology and history; this page holds only the current numbers and how to
replay them.

**Every number in a table here is generated** (#54). The tests and scripts behind each section
write records; `scripts/benchmarks.sh update` collects them into
[`benchmarks/benchmarks.json`](benchmarks/benchmarks.json), stamps each section with the commit it
was measured on, and rewrites the blocks between `GENERATED` markers. Never edit a generated
block by hand; `bash scripts/benchmarks.sh render --check` fails when the page and the records
disagree.

```bash
bash scripts/benchmarks.sh check            # measure everything, compare with the records (~2-3 min)
bash scripts/benchmarks.sh update           # re-measure and re-record, on a committed tree only
bash scripts/benchmarks.sh check --tier fast   # without OPM Flow or flowexp_comp
bash scripts/benchmarks.sh update --only spe1  # one section
```

`check` fails when a banded error grows by more than a tenth of its band or leaves it, or when a
recorded measurement is no longer produced. Any other change is listed, to be recorded
deliberately with `update` in the commit that caused it. A section run without a reference
simulator it needs (Flow, `flowexp_comp`) keeps its previous records and stamp.

Tolerances are acceptance criteria with deliberate headroom over the measured error. They live in
the tests, and the records carry them from there. They are not tuned to the build and are not to
be widened to make a change pass. A regression that breaks one is a physics or solver finding.

## Summary

Per section, the banded criterion closest to its band, and how much of the band it uses.

<!-- GENERATED:summary -->
| Area | Reference | Tightest criterion | Band | Band used | Measured on |
|---|---|---|---|---|---|
| 1D waterflood breakthrough | Buckley-Leverett + Welge | BL-Case-A nx=24: breakthrough_rel_err 9.44 % | 25.0 % | 38% | `21f25f8` |
| SPE1 Case 1, 10 years | Published SPE1 / Flow | 10x10x3: pressure_rel_err 1.60 % | 3.0 % | 53% | `21f25f8` |
| Three-phase gas drive and injection | OPM Flow | gas_drive: cum_oil_rel_err 4.31 % | 8.0 % | 54% | `57a0532` |
| Black-oil depletion column | Grid self-convergence, Flow | IMPES: sat_gas_finest_pair_gap 1.21 % | 1.5 % | 80% | `21f25f8` |
| Eight small decks, three solvers | OPM Flow, generated decks | no banded criterion | — | — | `501327e` |
| Native vs wasm bindings | Each other | favorable-mobility (IMPES): rates_max_abs_diff 1.25e-10 abs | 1e-09 abs | 12% | `21f25f8` |
| FIM convergence, long horizons | Substeps per report step | no banded criterion | — | — | `21f25f8` |
| Compositional, matched timestep | OPM flowexp_comp | 1D plain: worst_pressure_diff 1.68 bar | 2 bar | 84% | `21f25f8` |
| Second simulator on the same decks | JutulDarcy vs Flow and FIM | no banded criterion | — | — | `57a0532` |
<!-- /GENERATED:summary -->

## Signals

What should change priorities, computed from the committed records and
[`benchmarks/explained.json`](benchmarks/explained.json). An entry there marks a signal as
`explained` (the mechanism and evidence are written up where `ref` points) or `tracked` (an open
issue owns it). An unexplained, untracked gap needs an issue. Staleness depends on the current
commit, so it is not on this page: `python3 tools/benchmarks/benchmarks.py signals` prints it
together with everything below, and `benchmarks.sh check` warns about it.

**Nightly signals.** On the development machine, a user crontab entry runs
`scripts/benchmarks-nightly.sh` every night. It runs the full tier on `origin/master` in its own
worktree (`~/.cache/ressim-nightly`) and rewrites one pinned issue, "Benchmark signals (nightly)",
with the drift check and the signals. It comments there only when the state changes, and it never
commits. `NIGHTLY_DRY_RUN=1` prints the issue body instead.

<!-- GENERATED:signals -->
**Same-model gaps** — the reference solves the same discrete model, so a difference over 1 % (or 0.5 bar) is a finding until explained. 0 unexplained and untracked, 10 tracked, 14 explained.

| Section | Case | Metric | Value | Where | Status |
|---|---|---|---|---|---|
| three_phase | gas_drive | jutul_vs_flow.FOPR_rel_err | +3.10 % | t=20 d | tracked: #55 |
| cross_solver | dep-pvt-correlation | sparse.cum.FGPT | +1.06 % |  | tracked: #55 |
| cross_solver | dep-pvt-correlation | dense.cum.FGPT | +1.06 % |  | tracked: #55 |
| cross_solver | dep-pvt-lab-report | sparse.cum.FGPT | +1.13 % |  | tracked: #55 |
| cross_solver | dep-pvt-lab-report | dense.cum.FGPT | +1.13 % |  | tracked: #55 |
| jutul | dep-pvt-correlation | jutul_vs_flow.final_FGPR_rel_err | -2.21 % |  | tracked: #55 |
| jutul | dep-pvt-correlation | fim_vs_jutul.final_FGPR_rel_err | +1.67 % |  | tracked: #55 |
| jutul | dep-pvt-lab-report | jutul_vs_flow.final_FGPR_rel_err | -4.14 % |  | tracked: #55 |
| jutul | dep-pvt-lab-report | fim_vs_jutul.final_FGPR_rel_err | +2.80 % |  | tracked: #55 |
| jutul | ow-1d-96 | fim_vs_jutul.final_FOPR_rel_err | +1.46 % |  | tracked: #55 |
| spe1 | 10x10x3 | vs_flow.FOPR_rel_err | +2.73 % | t=3510 d | explained: BLACK_OIL_VALIDATION.md §1 (#55) |
| spe1 | 10x10x3 | vs_flow.FPR_rel_err | -3.06 % | t=900 d | explained: BLACK_OIL_VALIDATION.md §1 (#55) |
| spe1 | 10x10x3 | vs_flow.WGOR_rel_err | +7.13 % | t=810 d | explained: BLACK_OIL_VALIDATION.md §1 (#55) |
| spe1 | 20x20x3 | vs_flow.FOPR_rel_err | +2.81 % | t=3600 d | explained: BLACK_OIL_VALIDATION.md §1 (#55) |
| spe1 | 20x20x3 | vs_flow.FPR_rel_err | -3.03 % | t=900 d | explained: BLACK_OIL_VALIDATION.md §1 (#55) |
| spe1 | 20x20x3 | vs_flow.WGOR_rel_err | +7.17 % | t=720 d | explained: BLACK_OIL_VALIDATION.md §1 (#55) |
| three_phase | gas_drive | pressure_rel_err | +1.59 % | t=50 d | explained: THREE_PHASE_VALIDATION.md §6 (#55) |
| three_phase | gas_drive | gor_rel_err | -6.08 % | t=10 d | explained: THREE_PHASE_VALIDATION.md §6 (#55) |
| three_phase | gas_drive | cum_oil_rel_err | +4.31 % | t=600 d | explained: THREE_PHASE_VALIDATION.md §6 (#55) |
| three_phase | gas_drive | oil_rate_rel_err | +4.61 % | t=20 d | explained: THREE_PHASE_VALIDATION.md §6 (#55) |
| three_phase | gas_drive | vs_jutul.FGOR_rel_err | -6.04 % | t=20 d | explained: THREE_PHASE_VALIDATION.md §6 (#55) |
| three_phase | gas_drive | vs_jutul.FOPR_rel_err | +11.14 % | t=600 d | explained: THREE_PHASE_VALIDATION.md §6 (#55) |
| three_phase | gas_drive | vs_jutul.FPR_rel_err | +1.47 % | t=50 d | explained: THREE_PHASE_VALIDATION.md §6 (#55) |
| compositional | 1D plain | worst_pressure_diff | 1.68 bar | t = 0.19 d, cell 4: 59.4244 vs 61.1053 bar | explained: COMPOSITIONAL_VALIDATION.md §8 (C12) |

**Near the band** — more than 70% of an acceptance band used, so one modest regression from failing:

| Section | Case | Metric | Value | Band | Used | Status |
|---|---|---|---|---|---|---|
| compositional | 1D plain | worst_pressure_diff | 1.68 bar | 2 bar | 84% | explained: COMPOSITIONAL_VALIDATION.md §8 (C12) |
| depletion | IMPES | sat_gas_finest_pair_gap | +1.21 % | 1.5 % | 80% | explained: depletion_grid_convergence.rs, FINEST_PAIR_TOLERANCE_SAT_GAS (#11) |
| depletion | FIM | sat_gas_finest_pair_gap | +1.20 % | 1.5 % | 80% | explained: depletion_grid_convergence.rs, FINEST_PAIR_TOLERANCE_SAT_GAS (#11) |

**Loose bands** — criteria using less than 33% of their band would not notice a threefold regression. Tighten one only with a written justification in the same commit.

| Section | Loose, open | Loose, explained | Loosest | Used | Status |
|---|---|---|---|---|---|
| buckley | 3 of 4 | — | BL-Case-B-dt0.50: report_interval_spread | 0.3% | — |
| spe1 | 3 of 6 | — | 10x10x3: plateau_rel_err | < 0.1 % | — |
| three_phase | 5 of 9 | — | gas_drive: mb_drift_oil | < 0.1 % | — |
| depletion | 2 of 8 | — | FIM: pressure_finest_pair_gap | 15.5% | — |
| parity | 0 of 15 | 15 | buckley-fim (FIM): rates_max_abs_diff | < 0.1 % | explained: crates/ressim-py/parity/compare_native.py, TOL |
| compositional | 4 of 8 | — | 1D skin 60: cum_injection_rel_err | 2.7% | — |
<!-- /GENERATED:signals -->

## 1. Buckley–Leverett breakthrough (analytical)

Owning document: [`P4_TWO_PHASE_BENCHMARKS.md`](P4_TWO_PHASE_BENCHMARKS.md). IMPES, 24 cells,
BHP-controlled wells, breakthrough at 1 % water cut, compared with the Welge shock
`PV_BT = 1 / f_w'(S_w,shock)`. The error is signed: negative is early, as expected for first-order
upstream smearing, and refining the grid brings it closer to the reference.

<!-- GENERATED:buckley -->
*Measured on `21f25f8` (clean tree), 2026-09-25.*

| Case | PV_BT sim | PV_BT ref | Rel. error | Band |
|---|---|---|---|---|
| A | 0.5307 | 0.586 | -9.4 % | 25 % |
| B | 0.4657 | 0.5074 | -8.2 % | 30 % |

Grid refinement (breakthrough error; nx = 96 and 192 from `benchmark_buckley_leverett_grid_sweep_replay`):

| Case | nx = 24 | 48 | 96 | 192 |
|---|---|---|---|---|
| A | -9.4 % | -5.4 % | -3.0 % | -1.7 % |
| B | -8.2 % | -4.5 % | -2.7 % | -1.4 % |

Report interval 0.5 → 0.25 d moves breakthrough by A 0.12 %, B 3.5e-03 % (band 1 %).
<!-- /GENERATED:buckley -->

IMPES picks its own substeps, so `dt` is only the report interval, and changing it must not move
breakthrough (`benchmark_buckley_leverett_breakthrough_is_independent_of_report_interval`).
`benchmark_buckley_leverett_grid_refinement_improves_alignment` asserts nx = 24 → 48, and
`benchmark_buckley_leverett_grid_sweep_replay` continues to 192. `wf_numerics` shows the same
convergence in the app over a 40× cell-size range.

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib benchmark_buckley -- --include-ignored --nocapture
```

## 2. SPE1 comparative solution

Owning document: [`BLACK_OIL_VALIDATION.md`](BLACK_OIL_VALIDATION.md) §1. Odeh (1981) Case 1,
10×10×3, FIM, 30-day report steps, against the published/Flow series embedded in
`tests/spe1_acceptance.rs`. No solver warnings are allowed at any step.

<!-- GENERATED:spe1 -->
*Measured on `21f25f8` (clean tree), 2026-09-25, flow `flow 2026.04`.*

Against the published series:

| Criterion | Band | Worst measured |
|---|---|---|
| Field average pressure, yearly to 3650 d | 3.0 % | 1.597 % (t=1095 d) |
| Producer oil rate, yearly to 3650 d | 8.0 % | 3.149 % (t=3285 d) |
| Producing GOR, yearly to 3650 d | 12.0 % | 4.295 % (t=3285 d) |
| Oil-rate plateau while the reference is on plateau (≤ 730 d) | 0.5 % | 4.6e-07 % (t=730 d) |
| Oil material-balance drift | 1.0 % | 4.4e-05 % (t=3650 d) |
| Gas material-balance drift | 1.0 % | 1.7e-04 % (t=3650 d) |

Characterization against the published 10×10×3 series (no band):

| Grid | p | q_o | GOR |
|---|---|---|---|
| 10x10x3 | 1.60 % | 3.15 % | 4.31 % |
| 20x20x3 | 2.71 % | 6.33 % | 31.09 % |

Against OPM Flow run on the same grid (`tools/opm_flow/spe1_refinement_oracle.py`, 90-day checkpoints, worst signed ResSim − Flow):

| Grid | p | q_o | GOR |
|---|---|---|---|
| 10x10x3 | -3.06 % | +2.73 % | +7.13 % |
| 20x20x3 | -3.03 % | +2.81 % | +7.17 % |
<!-- /GENERATED:spe1 -->

Areal refinement (20×20×3) is a characterization, not a criterion: against the 10×10×3 published
series the refined front breaks through earlier and sharper. Flow shows the same thing when it is
run on the same refined grid, which is why the "vs Flow" columns matter more than the "vs
published" ones there (`BLACK_OIL_VALIDATION.md` §1). The Flow deck is hand-mapped from the
engine setup, not generated from it (unlike §5), so a gap in those columns is either a mapping
difference or a model difference, and is not yet explained.

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib spe1_full_horizon_matches_published_reference -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib spe1_areal_refinement_reference_error_replay -- --ignored --nocapture
python3 tools/opm_flow/spe1_refinement_oracle.py --out /tmp/spe1-refinement
```

## 3. Three-phase: solution gas drive and gas injection

Owning document: [`THREE_PHASE_VALIDATION.md`](THREE_PHASE_VALIDATION.md). Both cases share their
PVT table, SCAL, grid and wells with the Flow deck by construction. The gas-drive Flow deck is
hand-mapped; the gas-injection twin (`small-direct/go-1d-50`) is written by the simulator itself.
Errors are signed (ResSim − Flow), so a one-sided bias reads as one; the gas-drive cumulative-oil
bias is inside its band but unexplained (`THREE_PHASE_VALIDATION.md` §6).

<!-- GENERATED:three_phase -->
*Measured on `57a0532` (clean tree), 2026-09-25.*

**Solution gas drive** (`gas_drive`, 20 cells, FIM, 60 × 10 d; signed ResSim − Flow):

| Criterion | Band | Worst measured |
|---|---|---|
| Field average pressure, 11 checkpoints | 3.0 % | +1.588 % (t=50 d) |
| Producing GOR | 12.0 % | -6.076 % (t=10 d) |
| Cumulative surface oil | 8.0 % | +4.310 % (t=600 d) |
| Producer oil rate while reference ≥ 10 Sm³/d | 10.0 % | +4.609 % (t=20 d) |
| Oil material-balance drift | 1.0 % | +6.3e-06 % (t=500 d) |
| Gas material-balance drift | 1.0 % | +8.2e-05 % (t=500 d) |

Against JutulDarcy on the same deck (worst signed difference over the 11 checkpoints; JutulDarcy ignores `STONE2`):

| Pair | p | q_o | GOR |
|---|---|---|---|
| ResSim − JutulDarcy | +1.47 % | +11.14 % | -6.04 % |
| JutulDarcy − Flow | +0.27 % | +3.10 % | +0.41 % |

**1D gas injection** (`gas_injection`'s Flow twin, `small-direct/go-1d-50`; signed):

| Criterion | Band | Worst measured |
|---|---|---|
| Cumulative oil | 0.2 % | +0.023 % (t=200 d) |
| Cumulative gas injected | 0.2 % | -0.038 % (t=200 d) |
| Cumulative gas produced, after breakthrough | 1.5 % | -0.288 % (t=200 d) |

**Gas-front behavior** (20-cell gas flood): breakthrough at 4 d with dt = 1.0 and 4 d with dt = 0.5.
<!-- /GENERATED:three_phase -->

The gas-front bands (breakthrough 2–8 d, ≤ 1.5 d movement when dt is halved, saturation monotone
in space and non-decreasing in time) are asserted by `physics::gas_flood`.

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib three_phase_acceptance_error_replay -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib three_phase_gas -- --nocapture
```

## 4. Black-oil depletion grid convergence

Owning document: [`BLACK_OIL_VALIDATION.md`](BLACK_OIL_VALIDATION.md) §2. A 1D column depleted
below its bubble point, 20 × 5 d, at 5, 10, 20 and 40 cells. Pore-volume-weighted pressure [bar]
and free-gas saturation at 100 d. The Flow column is the final report of the small-direct
`bo-1d-10` and `bo-1d-40` decks (§5), which are the same column.

<!-- GENERATED:depletion -->
*Measured on `21f25f8` (clean tree), 2026-09-25, flow `flow 2026.04`.*

| nx | IMPES p / Sg | FIM p / Sg | Flow p / Sg |
|---|---|---|---|
| 5 | 129.0697 / 0.024349 | 129.1342 / 0.024281 |  |
| 10 | 129.7699 / 0.023405 | 129.8390 / 0.023330 | 129.8389 / 0.023330 |
| 20 | 130.1497 / 0.022896 | 130.2210 / 0.022816 |  |
| 40 | 130.3533 / 0.022623 | 130.4234 / 0.022545 | 130.4151 / 0.022556 |

| Solver | Quantity | Worst contraction ratio | Finest-pair gap |
|---|---|---|---|
| IMPES | pressure | 0.542 (≤ 0.8) | 0.16 % (≤ 1.0 %) |
| IMPES | sat_gas | 0.540 (≤ 0.8) | 1.21 % (≤ 1.5 %) |
| FIM | pressure | 0.542 (≤ 0.8) | 0.16 % (≤ 1.0 %) |
| FIM | sat_gas | 0.540 (≤ 0.8) | 1.20 % (≤ 1.5 %) |
<!-- /GENERATED:depletion -->

Both solvers must contract monotonically in every quantity. Agreement with Flow at the same 5-day
step is not accuracy: FIM and Flow share an implicit scheme and its time error (the owning
document compares both against a time-refined Flow run).

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib physics_depletion_ -- --nocapture --test-threads=1
```

## 5. Cross-solver scorecard (small-direct decks)

Owning documents: [`opm/reference-decks/small-direct/README.md`](../opm/reference-decks/small-direct/README.md)
and the committed `scorecard.json`, the regression ratchet for this table (its own bands, its own
`--update`). Eight decks written by the simulator itself, run through FIM (sparse and dense LU),
IMPES and Flow.

<!-- GENERATED:cross_solver -->
*Measured on `501327e` (clean tree), 2026-09-25, flow `flow 2026.04`.*

| Case | Simulator | Substeps | Newton | Retries / cuts | Wall ms | max \|Δp\| bar | max \|ΔSw\| | max \|ΔSg\| | Cumulatives vs Flow |
|---|---|---|---|---|---|---|---|---|---|
| bo-1d-10 | Flow | 22 | 52 | 0 |  |  |  |  |  |
|  | ResSim FIM sparse | 22 | 84 | 0 | 10 | 0.000946 | 1.4e-08 | 2.1e-06 | FOPT 1.4e-03 %, FGPT 1.3e-03 % |
|  | ResSim FIM dense | 22 | 84 | 0 | 7 | 0.000488 | 1.4e-08 | 1.5e-06 | FOPT 1.7e-03 %, FGPT 1.6e-03 % |
|  | ResSim IMPES | 25 | — | — | 4 | 0.468 | 1.4e-07 | 0.00049 | FOPT 0.29 %, FGPT 2.56 % |
| bo-1d-40 | Flow | 23 | 54 | 0 |  |  |  |  |  |
|  | ResSim FIM sparse | 22 | 91 | 0 | 28 | 0.274 | 8e-08 | 0.00033 | FOPT 0.05 %, FGPT 0.12 % |
|  | ResSim FIM dense | 22 | 90 | 0 | 39 | 0.274 | 8e-08 | 0.00033 | FOPT 0.05 %, FGPT 0.12 % |
|  | ResSim IMPES | 28 | — | — | 18 | 0.511 | 1.5e-07 | 0.00047 | FOPT 0.28 %, FGPT 2.27 % |
| dep-pvt-correlation | Flow | 300 | 608 | 0 |  |  |  |  |  |
|  | ResSim FIM sparse | 301 | 908 | 0 | 426 | 0.307 | 2.4e-07 | 0.00023 | FOPT 2.3e-08 %, FGPT 1.06 % |
|  | ResSim FIM dense | 301 | 908 | 0 | 610 | 0.307 | 2.4e-07 | 0.00023 | FOPT 2.3e-08 %, FGPT 1.06 % |
|  | ResSim IMPES | 300 | — | — | 200 | 0.322 | 1.8e-07 | 0.00026 | FOPT 0.00 %, FGPT 2.10 % |
| dep-pvt-lab-report | Flow | 300 | 607 | 0 |  |  |  |  |  |
|  | ResSim FIM sparse | 301 | 906 | 0 | 420 | 0.221 | 2.1e-07 | 0.00025 | FOPT 5.0e-08 %, FGPT 1.13 % |
|  | ResSim FIM dense | 301 | 906 | 0 | 621 | 0.221 | 2.1e-07 | 0.00025 | FOPT 5.0e-08 %, FGPT 1.13 % |
|  | ResSim IMPES | 300 | — | — | 190 | 0.149 | 1.9e-07 | 0.00028 | FOPT 0.00 %, FGPT 1.59 % |
| go-1d-50 | Flow | 151 | 339 | 0 |  |  |  |  |  |
|  | ResSim FIM sparse | 151 | 486 | 0 | 150 | 0.496 | 3e-06 | 0.005 | FOPT 0.01 %, FGPT 0.03 %, FGIT 0.02 % |
|  | ResSim FIM dense | 151 | 486 | 0 | 260 | 0.496 | 3e-06 | 0.005 | FOPT 0.01 %, FGPT 0.03 %, FGIT 0.02 % |
|  | ResSim IMPES | 213 | — | — | 87 | 12.6 | 8.5e-06 | 0.074 | FOPT 0.81 %, FGPT 0.57 %, FGIT 0.67 % |
| ow-1d-50-adverse | Flow | 13 | 33 | 0 |  |  |  |  |  |
|  | ResSim FIM sparse | 12 | 48 | 0 | 11 | 0.379 | 0.0042 | 0 | FOPT 0.08 %, FWIT 0.10 % |
|  | ResSim FIM dense | 12 | 48 | 0 | 28 | 0.379 | 0.0042 | 0 | FOPT 0.08 %, FWIT 0.10 % |
|  | ResSim IMPES | 28 | — | — | 3 | 6.15 | 0.04 | 0 | FOPT 0.45 %, FWIT 0.08 % |
| ow-1d-96 | Flow | 121 | 259 | 0 |  |  |  |  |  |
|  | ResSim FIM sparse | 120 | 376 | 0 | 168 | 0.943 | 0.032 | 0 | FOPT 0.05 %, FWPT 0.10 %, FWIT 0.07 % |
|  | ResSim FIM dense | 120 | 376 | 0 | 930 | 0.943 | 0.032 | 0 | FOPT 0.05 %, FWPT 0.10 %, FWIT 0.07 % |
|  | ResSim IMPES | 631 | — | — | 122 | 7.51 | 0.19 | 0 | FOPT 0.64 %, FWPT 4.84 %, FWIT 2.82 % |
| ow-2d-12x12 | Flow | 40 | 114 | 0 |  |  |  |  |  |
|  | ResSim FIM sparse | 40 | 133 | 0 | 181 | 0.752 | 0.0079 | 0 | FOPT 0.08 %, FWPT 0.81 %, FWIT 0.21 % |
|  | ResSim FIM dense | 40 | 133 | 0 | 1092 | 0.752 | 0.0079 | 0 | FOPT 0.08 %, FWPT 0.81 %, FWIT 0.21 % |
|  | ResSim IMPES | 225 | — | — | 133 | 23.9 | 0.15 | 0 | FOPT 2.25 %, FWPT 6.16 %, FWIT 3.18 % |
<!-- /GENERATED:cross_solver -->

How to read it: FIM should track Flow's substep count closely and take somewhat more Newton
iterations. IMPES is further from Flow on the waterfloods because it takes explicit steps on a
different scheme, not because it is wrong; the owning README compares all three against
time-refined runs. Wall times are single native runs.

```bash
bash scripts/validate-cross-solver.sh --markdown   # needs flow and its opm.io Python package
```

## 6. Native/wasm parity

`crates/ressim-py` (native, PyO3) against the browser's wasm build on the committed Buckley
fixture: worst absolute difference in cell fields, rate history and grid state. Checkpoint
restore must be bit-identical. Runs in PR CI.

<!-- GENERATED:parity -->
*Measured on `21f25f8` (clean tree), 2026-09-25.*

| Case | Cells | Rates | Grid state | Band |
|---|---|---|---|---|
| buckley-impes (IMPES) | 1.9e-12 | 2.9e-11 | 1.9e-12 | 1e-09 |
| buckley-fim (FIM) | 5.7e-14 | 0 | 5.7e-14 | 1e-09 |
| favorable-mobility (IMPES) | 1.8e-12 | 1.2e-10 | 1.8e-12 | 1e-09 |
| adverse-mobility-fim (FIM) | 5.7e-14 | 1.1e-13 | 5.7e-14 | 1e-09 |
| coarse-grid (IMPES) | 0 | 0 | 0 | 1e-09 |
<!-- /GENERATED:parity -->

```bash
bash scripts/validate-native-binding.sh
```

## 7. FIM convergence (wasm)

Owning document: [`FIM_STATUS.md`](FIM_STATUS.md). Long horizons, because short runs can hide
fragmentation. Measured through the wasm build with the default nonlinear flavor. Newton counts
accepted substeps; iterations spent in retried attempts are counted separately. Wall times are
single runs, so treat differences under 100 ms as noise; they are never checked for drift. Since
FIM-DIRECT-001 the linear routing is the same on every target, so these counts are not
wasm-specific (§6).

<!-- GENERATED:fim_wasm -->
*Measured on `21f25f8` (clean tree), 2026-09-25, node `v24.18.0`.*

| Case | Report steps | Substeps | Ratio | Newton | Retries lin/nonlin/mixed | Newton in retries | FIM ms | Linear + precond. |
|---|---|---|---|---|---|---|---|---|
| water-pressure 20x20x3 dt 0.25 | 20 | 20 | 1.00 | 117 | 0/0/0 | 0 | 2681 | 52% |
| water-pressure 22x22x1 dt 0.25 | 20 | 22 | 1.10 | 126 | 0/1/0 | 20 | 1022 | 48% |
| water-pressure 23x23x1 dt 0.25 | 20 | 22 | 1.10 | 123 | 0/1/0 | 20 | 1090 | 48% |
| water-pressure 12x12x3 dt 1 (heavy) | 20 | 23 | 1.15 | 116 | 0/0/0 | 0 | 890 | 46% |
| gas-rate 10x10x3 dt 0.25 | 24 | 27 | 1.12 | 129 | 0/1/0 | 20 | 1239 | 21% |
| gas-pressure 10x10x3 dt 0.25 | 20 | 25 | 1.25 | 147 | 0/2/0 | 40 | 1578 | 25% |
| sweep-areal 21x21x1 dt 0.25 | 20 | 20 | 1.00 | 63 | 0/0/0 | 0 | 343 | 43% |
<!-- /GENERATED:fim_wasm -->

```bash
bash scripts/build-wasm.sh
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --dt 1 --steps 20 --diagnostic quiet --json
```

## 8. Compositional engine

Owning document: [`COMPOSITIONAL_VALIDATION.md`](COMPOSITIONAL_VALIDATION.md) §8, C12, declared
2026-09-18. ResSim against OPM `flowexp_comp` on the CO₂/C₁/C₁₀ cases at matched temporal
resolution, from the committed fixtures. The full tier also re-derives those fixtures from a
locally built `flowexp_comp` and checks they still reproduce. The plain 1D deck's band is loose on
purpose: its sub-day `TSTEP`s are where the reference cut extra steps, so matching is approximate
there. The compositional engine is not yet offered in the app (`comp_co2_1d` is withheld pending
chart sourcing, #29).

<!-- GENERATED:compositional -->
*Measured on `21f25f8` (clean tree), 2026-09-25.*

| Case | Metric | Measured | Band | Where |
|---|---|---|---|---|
| depletion BHP | cum_production_rel_err | 0.0407 % | 1.00 % |  |
| depletion BHP | cum_production_worst_component_rel_err | 0.0879 % | 1.00 % | component 2 |
| depletion BHP | worst_pressure_diff | 0.0095 bar | 0.02 bar | t = 2.0 d: 91.7794 vs 91.7889 bar |
| depletion BHP | worst_sg_diff | 0.0181 % | 0.05 % |  |
| 1D skin 60 | worst_pressure_diff | 0.0074 bar | 0.02 bar | t = 5.11 d, cell 3: 97.2283 vs 97.2357 bar |
| 1D skin 60 | cum_injection_rel_err | 0.0014 % | 0.05 % |  |
| 1D plain | worst_pressure_diff | 1.6810 bar | 2 bar | t = 0.19 d, cell 4: 59.4244 vs 61.1053 bar |
| 1D plain | cum_injection_rel_err | 0.0418 % | 0.20 % |  |
<!-- /GENERATED:compositional -->

```bash
bash scripts/validate-compositional.sh reference   # skips with a note without flowexp_comp
```

## 9. Second simulator: JutulDarcy

[JutulDarcy](https://github.com/sintefmath/JutulDarcy.jl), an independent implementation in Julia,
runs the same decks as Flow: the eight small-direct decks and `gas_drive`. When two independent
codes agree and ResSim does not, the difference is ResSim's; when ResSim sits between them, the
reference itself is uncertain. The pinned project is `tools/jutul` (JutulDarcy 0.3.7 on Julia
1.12; it fails on 1.13). What it can and cannot referee:

- **State and rates only.** JutulDarcy's summary cumulatives are the end-of-step rate times the
  report interval, not an integral over its internal substeps, so they are wrong wherever rates
  change within a report step. Compared: cell pressure and saturation fields at every report
  step, end-of-run rates, and for `gas_drive` the FPR/FOPR/FGOR series (§3).
- **Not SPE1.** It ignores `DRSDT`, so it cannot run Case 1; it would run Case 2 physics. The
  bo-1d decks also set `DRSDT` and are shown, but not flagged as same-model gaps.
- **`STONE2` is ignored.** Every deck that sets it has water at connate saturation, where
  three-phase oil relative permeability reduces to the gas-oil table either way.
- A one-line shim in `run_decks.jl` works around a 0.3.7 bug that fails any live-oil deck
  starting with free gas (`gas_drive`); it changes no physics.

<!-- GENERATED:jutul -->
*Measured on `57a0532` (clean tree), 2026-09-25, flow `flow 2026.04`, jutuldarcy `0.3.7`, julia `julia version 1.12.7`.*

| Deck | JutulDarcy ignores | Jutul − Flow: max \|Δp\| bar | max \|ΔS\| | final rates | FIM − Jutul: max \|Δp\| bar | final rates |
|---|---|---|---|---|---|---|
| bo-1d-10 | STONE2, DRSDT | 0.283 | 0.00035 | FGPR -0.08 %, FOPR -0.09 % | 0.283 | FGPR +0.08 %, FOPR +0.09 % |
| bo-1d-40 | STONE2, DRSDT | 0.00617 | 6.1e-06 | FGPR +0.01 %, FOPR -3.1e-03 % | 0.273 | FGPR +0.08 %, FOPR +0.09 % |
| dep-pvt-correlation | STONE2 | 0.684 | 0.00041 | FGPR -2.21 %, FOPR +0.00 % | 0.682 | FGPR +1.67 %, FOPR +0.00 % |
| dep-pvt-lab-report | STONE2 | 0.371 | 0.00022 | FGPR -4.14 %, FOPR +0.00 % | 0.342 | FGPR +2.80 %, FOPR +0.00 % |
| go-1d-50 | STONE2 | 0.95 | 0.0047 | FGIR -0.06 %, FGPR -0.08 %, FOPR +0.04 % | 0.481 | FGIR +0.06 %, FGPR +0.08 %, FOPR -0.11 % |
| ow-1d-50-adverse | — | 1.26 | 0.016 | FOPR -0.20 %, FWIR -0.20 % | 1.28 | FOPR +0.30 %, FWIR +0.31 % |
| ow-1d-96 | — | 1.04 | 0.038 | FOPR -0.08 %, FWIR +0.03 %, FWPR +0.04 % | 1.71 | FOPR +1.46 %, FWIR +0.12 %, FWPR +0.01 % |
| ow-2d-12x12 | — | 4.4 | 0.032 | FOPR -0.27 %, FWIR +0.12 %, FWPR +0.28 % | 4.23 | FOPR +0.48 %, FWIR +0.33 %, FWPR +0.27 % |
<!-- /GENERATED:jutul -->

```bash
juliaup add 1.12   # once
julia +1.12 --project=tools/jutul tools/jutul/run_decks.jl /tmp/jutul bo-1d-10=opm/reference-decks/small-direct/bo-1d-10/CASE.DATA
```

## Scenario-level references

Most scenarios grade their own analytical or Flow reference in
`src/lib/catalog/scenarios/<key>.test.ts`, run by `pnpm run test:scenarios` (part of
`validate:product`); the Coverage grid below lists the ones that have only the catalog-wide
contract tests. Those tests assert each case's claim, for example that `dep_pss` recovers the
Dietz shape factor within 3 % or that `wf_numerics` converges at first order. The bands are
documented in each test. They are not restated here, because the test is the record.

## Coverage

Every scenario in the catalog, generated from `src/lib/catalog/scenarios/` and the OPM Flow
artifact cases in `tools/opm_flow/opm_flow_tool/cases.py`. "Engine benchmark" is a section of this
page that grades the scenario's physics; a scenario is mapped to one in `SCENARIO_SECTIONS`
(`tools/benchmarks/benchmarks.py`). A refinement dimension is a sensitivity that varies grid or
time step, which is where discretization error shows.

<!-- GENERATED:coverage -->
| Scenario | Analytical | OPM Flow artifact | Engine benchmark | Refinement dimension | Second simulator | Own `<key>.test.ts` |
|---|---|---|---|---|---|---|
| `comp_co2_1d` (withheld) | — | — | §8 | grid_refinement, timestep | — | yes |
| `dep_arps` | depletion | — | — | — | — | yes |
| `dep_decline` | depletion | — | — | timestep, grid_refinement | — | yes |
| `dep_gas_pz` | gas-material-balance | yes | — | — | — | yes |
| `dep_pss` | well-test | — | — | — | — | yes |
| `dep_pvt` (withheld) | — | yes | §5, §9 | — | JutulDarcy | yes |
| `dep_welltest` | well-test | — | — | — | — | yes |
| `gas_drive` | — | yes | §3 | — | JutulDarcy | yes |
| `gas_injection` | gas-oil-bl | yes | §3, §5, §9 | grid | JutulDarcy | yes |
| `spe1_gas_injection` | digitized-reference | yes | §2 | — | — | **none** |
| `sweep_areal` | sweep | — | — | grid_resolution | — | **none** |
| `sweep_combined` | sweep | — | — | — | — | **none** |
| `sweep_crossflow` | sweep | — | — | — | — | yes |
| `sweep_vertical` | sweep | — | — | — | — | yes |
| `wf_bl1d` | buckley-leverett | yes | §1 | — | — | yes |
| `wf_capillary` | buckley-leverett | — | — | — | — | yes |
| `wf_gravity` | buckley-leverett | yes | — | — | — | yes |
| `wf_gravity_stability` | buckley-leverett | — | — | resolution | — | yes |
| `wf_numerics` | buckley-leverett | yes | — | grid_refinement, time_truncation | — | yes |

**10 scenario(s) have no numerical reference** (neither a Flow artifact nor an engine benchmark), only an analytical one or none: `dep_arps`, `dep_decline`, `dep_pss`, `dep_welltest`, `sweep_areal`, `sweep_combined`, `sweep_crossflow`, `sweep_vertical`, `wf_capillary`, `wf_gravity_stability`. 3 scenario(s) are graded against a second independent simulator. 3 scenario(s) have no test file of their own, only the catalog-wide contract tests: `spe1_gas_injection`, `sweep_areal`, `sweep_combined`.
<!-- /GENERATED:coverage -->

## Not benchmarked

- No three-phase analytical reference. Three-phase grading is numerical (Flow, SPE1).
- No SPE case beyond SPE1 (SPE3, SPE5 and SPE9 are not run; the compositional roadmap is #52).
- JutulDarcy (§9) is the second simulator for the small-direct decks and `gas_drive` only: not
  SPE1 (it ignores `DRSDT`), not compositional, and not for cumulatives.
- The large wasm presets (`opm/reference-decks/{gas-rate,water-*}`) are hand-mapped decks and are
  not on the cross-solver scorecard. Their history is in `SOLVER_COMPARISON_SUMMARY.md`.

## Superseded baselines

Kept only so an older number can be traced. Do not cite these as current.

| Where | Old baseline | Why superseded |
|---|---|---|
| This page, before #54 | Hand-copied tables, baseline `5c29e0e` (§1 `92a57ec`); §8 quoted, not re-run | Replaced by tables generated from `docs/benchmarks/benchmarks.json`. The hand-copied figures had drifted from their sources in five places (#54) |
| `P4_TWO_PHASE_BENCHMARKS.md` | 2026-02-15: BL errors 4.0 % / 9.0 %; "refined" 3.1 % / 2.5 % | Engine changes since; the refined-discretization test no longer exists (now the grid sweep in §1) |
| §1 of this page | `5c29e0e`: BL +4.1 % / +9.7 %; dt sweep A 4.1 → 2.1 %, B 40.1 → 9.7 % | Harness artifact. `step` records one rate point per IMPES substep, and the harness integrated only the last one over the whole outer step and checked water cut only at step ends. Breakthrough falls inside the first 2–5 outer steps (17–74 substeps each), so the result tracked the report interval, not the physics. Fixed in `92a57ec` (#53) |
| `BLACK_OIL_VALIDATION.md` §1 | `0cfead9` + tests, 2026-07-24: SPE1 worst 1.73 / 3.33 / 4.39 % | Provisional (tests uncommitted at the time); re-run here |
| `THREE_PHASE_VALIDATION.md` §5 | `a651c02` + tests, 2026-07-25 | Provisional; re-run here. Errors unchanged to 3 decimals; oil/gas balance drift now ~0 after #37 and #42 |
| `SOLVER_COMPARISON_SUMMARY.md` | `663e380`, 2026-07-24, 1-step control matrix | Linear retries it records no longer fire; long-horizon table in §7 |
| `FIM_STATUS.md` | `6be6d08`, 2026-09-15, wasm | Reproduced here to within one substep, on the unified linear routing |
