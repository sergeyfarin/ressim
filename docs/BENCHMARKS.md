# Benchmarks

The current benchmark scorecard: every external or analytical reference ResSim is graded against,
the acceptance band, and the error measured on a committed revision. **Owning documents** hold the
case definitions, methodology and history; this page holds only the current numbers and how to
replay them.

**Baseline:** commit `5c29e0e` (clean tree), measured 2026-09-25, native `x86_64` release unless a
row says wasm. Reference simulator: OPM `flow 2026.04`. Everything below was reproduced on that
commit with the command next to it. It supersedes the older figures listed under
[Superseded baselines](#superseded-baselines).

Tolerances are acceptance criteria with deliberate headroom over the measured error. They are not
tuned to the build and are not to be widened to make a change pass. A regression that breaks one is
a physics or solver finding.

## Summary

| Area | Reference | Worst measured error | Band | Section |
|---|---|---|---|---|
| 1D waterflood breakthrough | Buckley–Leverett + Welge | −9.4 % (case A), −8.2 % (case B) early | 25 % / 30 % | [1](#1-buckleyleverett-breakthrough-analytical) |
| SPE1 Case 1, 10 years | Published SPE1 / Flow | p 1.60 %, q_o 3.15 %, GOR 4.30 % | 3 % / 8 % / 12 % | [2](#2-spe1-comparative-solution) |
| Solution gas drive, 600 d | Flow, identical deck | p 1.59 %, GOR 6.08 %, cum. oil 4.31 % | 3 % / 12 % / 8 % | [3](#3-three-phase-solution-gas-drive-and-gas-injection) |
| 1D gas injection, 300 d | Flow, identical deck | cum. oil and gas injected ≤ 0.05 % | 0.2 % | [3](#3-three-phase-solution-gas-drive-and-gas-injection) |
| Black-oil depletion column | Flow, identical deck | FIM 0.008 bar at nx = 40 | converging sequence | [4](#4-black-oil-depletion-grid-convergence) |
| Eight small decks, three solvers | Flow, generated decks | FIM: oil ≤ 0.08 %, injection ≤ 0.21 %, produced water or gas ≤ 1.13 % | scorecard bands | [5](#5-cross-solver-scorecard-small-direct-decks) |
| Native vs wasm bindings | Each other | 5.7e-14 (FIM), 1.9e-12 (IMPES) | strict | [6](#6-nativewasm-parity) |
| FIM convergence, 20-step horizons | Substeps per report step | ratio 1.00–1.25, 0 linear retries | no stall | [7](#7-fim-convergence-wasm) |
| Compositional, 1D CO₂ flood | `flowexp_comp` | 0.0074 bar, cum. 0.041 % | §5 of its doc | [8](#8-compositional-engine-recorded-not-re-run) |

## 1. Buckley–Leverett breakthrough (analytical)

Owning document: [`P4_TWO_PHASE_BENCHMARKS.md`](P4_TWO_PHASE_BENCHMARKS.md). IMPES, 24 cells,
BHP-controlled wells, breakthrough at 1 % water cut, compared with the Welge shock
`PV_BT = 1 / f_w'(S_w,shock)`.

**Measured on `92a57ec`** (clean tree), which fixed the harness (#53). It is the only section on
this page stamped later than the page baseline. The engine is the same as at `5c29e0e`.

| Case | dt [d] | PV_BT sim | PV_BT ref | Rel. error | Band |
|---|---|---|---|---|---|
| A (favorable, μo/μw = 2) | 0.50 | 0.5307 | 0.5860 | −9.4 % | 25 % |
| B (adverse, μo/μw = 2.33) | 0.50 | 0.4657 | 0.5074 | −8.2 % | 30 % |

IMPES picks its own substeps, so `dt` here is only the report interval. Changing it from 0.5 to 0.25
moves breakthrough by at most 0.12 %
(`benchmark_buckley_leverett_breakthrough_is_independent_of_report_interval`). Breakthrough
comes early, as expected for first-order upstream smearing, and refining the grid brings it closer
to the reference (`benchmark_buckley_leverett_grid_refinement_improves_alignment` asserts
nx = 24 → 48):

| Case | nx = 24 | 48 | 96 | 192 |
|---|---|---|---|---|
| A | −9.4 % | −5.4 % | −3.0 % | −1.7 % |
| B | −8.2 % | −4.5 % | −2.7 % | −1.4 % |

The nx = 96 and 192 columns are provisional. They come from a one-off run of the same harness on
`5c29e0e`, and no committed test replays them.

`wf_numerics` shows the same convergence in the app over a 40× cell-size range.

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib benchmark_buckley -- --nocapture
```

## 2. SPE1 comparative solution

Owning document: [`BLACK_OIL_VALIDATION.md`](BLACK_OIL_VALIDATION.md) §1. Odeh (1981) Case 1,
10×10×3, FIM, 30-day report steps, against the published/Flow series embedded in
`tests/spe1_acceptance.rs`.

| Criterion | Band | Worst measured |
|---|---|---|
| Field average pressure, yearly to 3650 d | 3 % | 1.597 % (1095 d) |
| Producer oil rate, yearly to 3650 d | 8 % | 3.149 % (3285 d) |
| Producing GOR, yearly to 3650 d | 12 % | 4.295 % (3285 d) |
| Oil-rate plateau held while the reference is on plateau (≤ 730 d) | 0.5 % | met |
| Oil and gas material-balance drift | 1 % each | met at every checkpoint |
| Solver warnings | none | none |

```text
t=  365.0 pressure_err= 0.297% oil_rate_err= 0.001% gor_err= 1.023%
t=  730.0 pressure_err= 0.952% oil_rate_err= 0.001% gor_err= 2.252%
t= 1095.0 pressure_err= 1.597% oil_rate_err= 0.477% gor_err= 1.095%
t= 1460.0 pressure_err= 1.263% oil_rate_err= 0.410% gor_err= 0.348%
t= 1825.0 pressure_err= 0.982% oil_rate_err= 1.541% gor_err= 1.081%
t= 2190.0 pressure_err= 1.031% oil_rate_err= 3.139% gor_err= 3.470%
t= 2555.0 pressure_err= 0.968% oil_rate_err= 3.088% gor_err= 3.550%
t= 2920.0 pressure_err= 0.801% oil_rate_err= 3.028% gor_err= 3.819%
t= 3285.0 pressure_err= 0.842% oil_rate_err= 3.149% gor_err= 4.295%
t= 3650.0 pressure_err= 0.918% oil_rate_err= 3.129% gor_err= 3.586%
SPE1 worst-case errors: pressure=1.597% oil_rate=3.149% gor=4.295%
```

Areal refinement (20×20×3) is a characterization, not a criterion. Against the 10×10×3 published
series it reads GOR 31.1 % at 730 d and 1.1 % at 3650 d, because the refined front breaks through
earlier and sharper. Flow shows the same thing when it is run on the same refined grid
(`BLACK_OIL_VALIDATION.md` §1).

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib spe1_full_horizon_matches_published_reference -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib spe1_areal_refinement_reference_error_replay -- --ignored --nocapture
```

## 3. Three-phase: solution gas drive and gas injection

Owning document: [`THREE_PHASE_VALIDATION.md`](THREE_PHASE_VALIDATION.md). Both cases share their
PVT table, SCAL, grid and wells with the Flow deck by construction.

**Solution gas drive** (`gas_drive` scenario, 20 cells, FIM, 60 × 10 d):

| Criterion | Band | Worst measured |
|---|---|---|
| Field average pressure, 11 checkpoints | 3 % | 1.588 % (50 d) |
| Producing GOR | 12 % | 6.076 % (10 d) |
| Cumulative surface oil | 8 % | 4.310 % (600 d) |
| Producer oil rate while reference ≥ 10 Sm³/d | 10 % | 4.609 % (20 d) |
| Oil / gas material-balance drift | 1 % | ≤ 0.0001 % each |

**1D gas injection** (`gas_injection` scenario's Flow twin, `small-direct/go-1d-50`):
cumulative oil and gas injected within 0.2 % at 100, 200 and 300 d; gas produced within 1.5 %
after breakthrough. Gas breaks through in the same 170–180 d window in both simulators.

**Gas-front behavior** (20-cell gas flood): breakthrough at 4.0 d (band 2–8 d), unchanged when dt
is halved (band ≤ 1.5 d); gas saturation monotone in space and non-decreasing in time.

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib three_phase_acceptance_error_replay -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib three_phase_gas -- --nocapture
```

## 4. Black-oil depletion grid convergence

Owning document: [`BLACK_OIL_VALIDATION.md`](BLACK_OIL_VALIDATION.md) §2. A 1D column depleted
below its bubble point, 20 × 5 d, at 5, 10, 20 and 40 cells. Pore-volume-weighted pressure [bar]
and free-gas saturation at 100 d:

| nx | IMPES p / Sg | FIM p / Sg | Flow p / Sg |
|---|---|---|---|
| 5 | 129.0697 / 0.024349 | 129.1342 / 0.024281 | |
| 10 | 129.7699 / 0.023405 | 129.8390 / 0.023330 | 129.8389 / 0.023330 |
| 20 | 130.1497 / 0.022896 | 130.2210 / 0.022816 | |
| 40 | 130.3533 / 0.022623 | 130.4234 / 0.022545 | 130.4151 / 0.022556 |

Both solvers contract monotonically in every quantity; IMPES closes oil, gas and water to
roundoff. Agreement with Flow at the same 5-day step is not accuracy: FIM and Flow share an
implicit scheme and its time error (the owning document compares both against a time-refined Flow
run).

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib physics_depletion_ -- --nocapture --test-threads=1
```

## 5. Cross-solver scorecard (small-direct decks)

Owning documents: [`opm/reference-decks/small-direct/README.md`](../opm/reference-decks/small-direct/README.md)
and the committed `scorecard.json` (measured at `58231be`). Eight decks written by the simulator
itself, run through FIM (sparse and dense LU), IMPES and Flow. The gate passed on `5c29e0e`. The
table is the script's `--markdown` output, unedited:

| Case | Simulator | Substeps | Newton | Retries / cuts | Wall ms | max \|Δp\| bar | max \|ΔSw\| | max \|ΔSg\| | Cumulatives vs Flow |
|---|---|---|---|---|---|---|---|---|---|
| bo-1d-10 | Flow | 22 | 52 | 0 |  | | | | |
| | ResSim FIM sparse | 22 | 84 | 0 | 9 | 0.000946 | 1.4e-08 | 2.1e-06 | FOPT 0.00%, FGPT 0.00% |
| | ResSim FIM dense | 22 | 84 | 0 | 8 | 0.000488 | 1.4e-08 | 1.5e-06 | FOPT 0.00%, FGPT 0.00% |
| | ResSim IMPES | 25 | — | — | 4 | 0.468 | 1.4e-07 | 0.00049 | FOPT 0.29%, FGPT 2.56% |
| bo-1d-40 | Flow | 23 | 54 | 0 |  | | | | |
| | ResSim FIM sparse | 22 | 91 | 0 | 28 | 0.274 | 8e-08 | 0.00033 | FOPT 0.05%, FGPT 0.12% |
| | ResSim FIM dense | 22 | 90 | 0 | 39 | 0.274 | 8e-08 | 0.00033 | FOPT 0.05%, FGPT 0.12% |
| | ResSim IMPES | 28 | — | — | 18 | 0.511 | 1.5e-07 | 0.00047 | FOPT 0.28%, FGPT 2.27% |
| dep-pvt-correlation | Flow | 300 | 608 | 0 |  | | | | |
| | ResSim FIM sparse | 301 | 908 | 0 | 409 | 0.307 | 2.4e-07 | 0.00023 | FOPT 0.00%, FGPT 1.06% |
| | ResSim FIM dense | 301 | 908 | 0 | 603 | 0.307 | 2.4e-07 | 0.00023 | FOPT 0.00%, FGPT 1.06% |
| | ResSim IMPES | 300 | — | — | 199 | 0.322 | 1.8e-07 | 0.00026 | FOPT 0.00%, FGPT 2.10% |
| dep-pvt-lab-report | Flow | 300 | 607 | 0 |  | | | | |
| | ResSim FIM sparse | 301 | 906 | 0 | 414 | 0.221 | 2.1e-07 | 0.00025 | FOPT 0.00%, FGPT 1.13% |
| | ResSim FIM dense | 301 | 906 | 0 | 608 | 0.221 | 2.1e-07 | 0.00025 | FOPT 0.00%, FGPT 1.13% |
| | ResSim IMPES | 300 | — | — | 190 | 0.149 | 1.9e-07 | 0.00028 | FOPT 0.00%, FGPT 1.59% |
| go-1d-50 | Flow | 151 | 339 | 0 |  | | | | |
| | ResSim FIM sparse | 151 | 486 | 0 | 137 | 0.496 | 3e-06 | 0.005 | FOPT 0.01%, FGPT 0.03%, FGIT 0.02% |
| | ResSim FIM dense | 151 | 486 | 0 | 256 | 0.496 | 3e-06 | 0.005 | FOPT 0.01%, FGPT 0.03%, FGIT 0.02% |
| | ResSim IMPES | 213 | — | — | 86 | 12.6 | 8.5e-06 | 0.074 | FOPT 0.81%, FGPT 0.57%, FGIT 0.67% |
| ow-1d-50-adverse | Flow | 13 | 33 | 0 |  | | | | |
| | ResSim FIM sparse | 12 | 48 | 0 | 11 | 0.379 | 0.0042 | 0 | FOPT 0.08%, FWIT 0.10% |
| | ResSim FIM dense | 12 | 48 | 0 | 24 | 0.379 | 0.0042 | 0 | FOPT 0.08%, FWIT 0.10% |
| | ResSim IMPES | 28 | — | — | 3 | 6.15 | 0.04 | 0 | FOPT 0.45%, FWIT 0.08% |
| ow-1d-96 | Flow | 121 | 259 | 0 |  | | | | |
| | ResSim FIM sparse | 120 | 376 | 0 | 165 | 0.943 | 0.032 | 0 | FOPT 0.05%, FWPT 0.10%, FWIT 0.07% |
| | ResSim FIM dense | 120 | 376 | 0 | 897 | 0.943 | 0.032 | 0 | FOPT 0.05%, FWPT 0.10%, FWIT 0.07% |
| | ResSim IMPES | 631 | — | — | 119 | 7.51 | 0.19 | 0 | FOPT 0.64%, FWPT 4.84%, FWIT 2.82% |
| ow-2d-12x12 | Flow | 40 | 114 | 0 |  | | | | |
| | ResSim FIM sparse | 40 | 133 | 0 | 176 | 0.752 | 0.0079 | 0 | FOPT 0.08%, FWPT 0.81%, FWIT 0.21% |
| | ResSim FIM dense | 40 | 133 | 0 | 1061 | 0.752 | 0.0079 | 0 | FOPT 0.08%, FWPT 0.81%, FWIT 0.21% |
| | ResSim IMPES | 225 | — | — | 130 | 23.9 | 0.15 | 0 | FOPT 2.25%, FWPT 6.16%, FWIT 3.18% |

How to read it: FIM tracks Flow's substep count almost exactly and takes about 1.2–1.6× its Newton
iterations. IMPES is further from Flow on the waterfloods because it takes explicit steps on a
different scheme, not because it is wrong. The owning README compares all three against
time-refined runs. Wall times are single native runs, and Flow's are not recorded.

```bash
bash scripts/validate-cross-solver.sh --markdown   # needs flow and its opm.io Python package
```

## 6. Native/wasm parity

`crates/ressim-py` (native, PyO3) against the browser's wasm build on the committed Buckley
fixture. Five strict cases, all agreeing: FIM cells 5.7e-14, IMPES cells ≤ 1.9e-12, and
checkpoint restore bit-identical. Runs in PR CI.

```bash
bash scripts/validate-native-binding.sh
```

## 7. FIM convergence (wasm)

Owning document: [`FIM_STATUS.md`](FIM_STATUS.md). Long horizons, because short runs can hide
fragmentation. Measured through the wasm build with the default nonlinear flavor.

| Case | Report steps | Substeps | Ratio | Newton | Retries lin/nonlin/mixed | FIM ms | Linear + precond. |
|---|---|---|---|---|---|---|---|
| water-pressure 20×20×3, dt 0.25 | 20 | 20 | 1.00 | 117 | 0/0/0 | 2709 | 52 % |
| water-pressure 22×22×1, dt 0.25 | 20 | 22 | 1.10 | 126 | 0/1/0 | 1023 | 48 % |
| water-pressure 23×23×1, dt 0.25 | 20 | 22 | 1.10 | 123 | 0/1/0 | 1084 | 48 % |
| water-pressure 12×12×3, dt 1 (heavy) | 20 | 23 | 1.15 | 116 | 0/0/0 | 872 | 45 % |
| gas-rate 10×10×3, dt 0.25 | 24 | 27 | 1.12 | 129 | 0/1/0 | 1266 | 21 % |
| gas-pressure 10×10×3, dt 0.25 | 20 | 25 | 1.25 | 147 | 0/2/0 | 1547 | 25 % |
| sweep-areal 21×21×1, dt 0.25 | 20 | 20 | 1.00 | 63 | 0/0/0 | 348 | 45 % |

No case stalls and none takes a linear retry. Wall times are single runs, so treat differences
under 100 ms as noise. Since FIM-DIRECT-001 the linear routing is the same on every target, so
these counts are no longer wasm-specific. Section 6 shows the two builds agree.

```bash
bash scripts/build-wasm.sh
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --dt 1 --steps 20 --diagnostic quiet --json
# the same for each row: --preset <P> --grid <G> --dt <D> --steps <N>; sum stepRecords[].fimAcceptedSubsteps
```

## 8. Compositional engine (recorded, not re-run)

Owning document: [`COMPOSITIONAL_VALIDATION.md`](COMPOSITIONAL_VALIDATION.md) §8, C12, declared
2026-09-18. It is quoted here, not re-measured, because the reference needs a locally built
`flowexp_comp`. At matched temporal resolution against OPM `flowexp_comp` on the CO₂/C₁/C₁₀ 1D
flood: 0.0074 bar trajectory, 0.0014 % cumulative injection, 0.041 % cumulative production, flash
along the reference's trajectory to 1.3e-7. The compositional engine is not yet offered in the app
(`comp_co2_1d` is withheld pending chart sourcing, #29).

```bash
bash scripts/validate-compositional.sh reference   # skips with a note without flowexp_comp
```

## Scenario-level references

Every scenario in the picker grades its own analytical or Flow reference in
`src/lib/catalog/scenarios/<key>.test.ts`, run by `pnpm run test:scenarios` (part of
`validate:product`). Those tests assert each case's claim, for example that `dep_pss` recovers the
Dietz shape factor within 3 % or that `wf_numerics` converges at first order. The bands are
documented in each test. They are not restated here, because the test is the record.

## Not benchmarked

- No three-phase analytical reference. Three-phase grading is numerical (Flow, SPE1).
- No SPE case beyond SPE1 (SPE3, SPE5 and SPE9 are not run; the compositional roadmap is #52).
- Flow is the only external simulator. A second one (JutulDarcy) is not wired in
  (`OPEN_ITEMS_2026-09-21.md` §9).
- The large wasm presets (`opm/reference-decks/{gas-rate,water-*}`) are hand-mapped decks and are
  not on the cross-solver scorecard. Their history is in `SOLVER_COMPARISON_SUMMARY.md`.

## Superseded baselines

Kept only so an older number can be traced. Do not cite these as current.

| Where | Old baseline | Why superseded |
|---|---|---|
| `P4_TWO_PHASE_BENCHMARKS.md` | 2026-02-15: BL errors 4.0 % / 9.0 %; "refined" 3.1 % / 2.5 % | Engine changes since; the refined-discretization test no longer exists (now the grid sweep in §1) |
| §1 of this page | `5c29e0e`: BL +4.1 % / +9.7 %; dt sweep A 4.1 → 2.1 %, B 40.1 → 9.7 % | Harness artifact. `step` records one rate point per IMPES substep, and the harness integrated only the last one over the whole outer step and checked water cut only at step ends. Breakthrough falls inside the first 2–5 outer steps (17–74 substeps each), so the result tracked the report interval, not the physics. Fixed in `92a57ec` (#53) |
| `BLACK_OIL_VALIDATION.md` §1 | `0cfead9` + tests, 2026-07-24: SPE1 worst 1.73 / 3.33 / 4.39 % | Provisional (tests uncommitted at the time); re-run here |
| `THREE_PHASE_VALIDATION.md` §5 | `a651c02` + tests, 2026-07-25 | Provisional; re-run here. Errors unchanged to 3 decimals; oil/gas balance drift now ~0 after #37 and #42 |
| `SOLVER_COMPARISON_SUMMARY.md` | `663e380`, 2026-07-24, 1-step control matrix | Linear retries it records no longer fire; long-horizon table in §7 |
| `FIM_STATUS.md` | `6be6d08`, 2026-09-15, wasm | Reproduced here to within one substep, on the unified linear routing |
