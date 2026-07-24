# OPM Flow vs ResSim FIM vs ResSim IMPES

Convergence numbers re-baselined **2026-07-24 on clean committed tree `663e380`** (wasm rebuilt
via `scripts/build-wasm.sh` first). This **supersedes the provisional dirty-tree `1db2db8`
(2026-07-22) baseline**, whose water-heavy row reported `50 substeps` for the `--opm-aligned` path;
on the clean tree — where `OpmAligned` is now the default nonlinear flavor (WATER-026) — the same
case converges in `4 substeps / 0 retries`. The stale figures were an intermediate-experiment
artifact, exactly the case the baseline-discipline rule exists to catch. This is a decision table,
not a claim that every row is a three-way parity oracle. A solver is marked `N/A` when the
repository lacks the same control/physics mapping; `not qualified` when it runs but has no
quantitative correctness band against the named reference.

## Browser memory footprint

ResSim runs in a browser WASM sandbox, so this baseline deliberately stays within grids that fit a
constrained heap. Every case below is `≤ 1200` cells (largest: `23×23×1 = 529`; three-phase heavy
`12×12×3 = 432`; SPE1-like `10×10×3 = 300`). No corner-point, no full-field (SPE10/PUNQ/Egg-scale)
case is exercised here — those are deferred to the offline OPM pipeline, not the browser solver.
Do not add `> ~1500`-cell convergence cases to this matrix without a memory-budget check.

## Convergence re-baseline — clean tree `663e380` (2026-07-24)

FIM = default `OpmAligned` flavor unless marked `(Legacy)`. Commands are
`node scripts/fim-wasm-diagnostic.mjs --preset <P> --grid <G> --steps <S> --dt <D> --diagnostic summary --no-json`;
IMPES adds `--solver impes` (its `substeps` = explicit adaptive internal steps, reported as
`history+=N`). Single-run WASM observations; treat sub-100 ms differences as noise.

### Two-phase waterflood (control matrix)

| Case | FIM substeps / retries (l/n/m) | FIM wall | Legacy substeps / retries | IMPES substeps | Note |
| --- | --- | --- | --- | --- | --- |
| water-pressure 20×20×3 dt=0.25 | 3 / 0-1-0 | 1.6 s | — | 58 | clean |
| water-pressure 22×22×1 dt=0.25 | 11 / **8**-0-0 | 1.1 s | 4 / 0-2-0 | — | `linear-bad` = singularity backstop firing |
| water-pressure 23×23×1 dt=0.25 | 6 / **4**-0-0 | 0.9 s | 4 / 0-2-0 | — | same backstop pattern |
| water-pressure **12×12×3 dt=1 (heavy)** | **4 / 0-0-0** | ~1.0 s | 24 / 0-4-0 (4.6 s) | 128 | OPM Flow oracle: 1 substep / 11 Newton |

### Three-phase gas injection

| Case | FIM substeps / retries | FIM wall | IMPES substeps | Final `Sg_max` (FIM / IMPES) | Note |
| --- | --- | --- | --- | --- | --- |
| gas-rate 20×20×3 dt=0.25 | 1 / 0-0-0 | 0.9 s | — | 0.231 / — | clean, uncut |
| gas-rate 10×10×3 dt=0.25 ×6 | 1 per step / 0-0-0 | 0.04-0.4 s/step | ~4/step | 0.438 / 0.329 | steady GOR=80; FIM≠IMPES final Sg (not interchangeable) |
| gas-pressure 10×10×3 dt=0.25 | 6 / 0-2-0 | 1.4 s | 39 | 0.477 / 0.316 | `nonlinear-bad:gas` transient |
| gas-pressure 20×20×3 dt=0.25 | 6 / 0-2-0 | 3.9 s | — | 0.489 / — | |
| gas-pressure 12×12×3 dt=1 | 9 / 0-2-0 | 2.0 s | — | 0.559 / — | heavier three-phase, no cuts |

### Areal sweep

| Case | FIM substeps / retries | FIM wall | IMPES substeps | Note |
| --- | --- | --- | --- | --- |
| sweep-areal 21×21×1 dt=0.25 | 6 / **4**-0-0 | 0.7 s | 37 | `linear-bad` singularity backstop again |

### SPE-suite (SPE1-like, 10×10×3, native `cargo --release`)

| Test | Result | Convergence detail |
| --- | --- | --- |
| `spe1_fim_first_steps_converge_without_stall` (5×1 d) | pass | step 1 = **2 substeps**, steps 2-5 = **1 substep each**, 0 retries, dt reaches full 1.0 uncut — OPM-class |
| `spe1_fim_gas_injection_creates_free_gas` (10×1 d) | pass | free gas created, MB-closed, no solver warnings |
| `spe1_fim_producer_gas_breakthrough_smoke` (4×4×3) | pass | breakthrough reached, no warnings |

Suite: `3 passed; 0 failed` in 0.32 s. (Per-step substep counts read via a temporary one-line
`stats` print; the committed test asserts only the envelope substeps≤20 / nonlinear_bad≤2 /
min_dt≥5e-3, all satisfied with wide margin.)

### What the re-baseline changes

1. **The heavy water case is essentially solved on the default path**: `4 substeps / 0 retries`
   vs OPM Flow's `1`, and `6×` fewer substeps than `Legacy` (24). The `50`-substep figure in the
   old baseline is stale.
2. **The default (`OpmAligned` + WATER-025 raw saturations) trades this for `linear-bad` retries
   on small well-dominated cases** (`22×22×1`: 8; `23×23×1`: 4; `sweep-areal`: 4). Each is the
   deferred relperm-endpoint singularity backstop (`solve_linearized_system` iterative fallback)
   catching a singular Jacobian after the fact — `Legacy` has zero `linear-bad` on these. This is
   the fragility flagged in `c2167f2` / `TODO.md:709`, now quantified.
3. **Gas / three-phase converges cleanly** (gas-rate uncut at 1 substep; gas-pressure 6-9 substeps,
   0 `linear-bad`), and SPE1 is at OPM-class efficiency.
4. **FIM and IMPES remain non-interchangeable** as correctness oracles (e.g. gas-rate `Sg_max`
   0.438 vs 0.329; heavy-water final oil differs) — the documented explicit-vs-implicit gap.

## Current result table

| Scenario | OPM Flow 2026.04 | ResSim FIM | ResSim IMPES | Correctness status |
| --- | --- | --- | --- | --- |
| Exact gas RESV, 10x10x3, 6 x 0.25 d | `0.08-0.10 s` simulation; 6 steps, 26 Newton updates, 32 residual evaluations, 27 Krylov iterations, 0 cuts | `0.191-0.207 s` native; 6 steps, 23 updates, 29 evaluations, 23 reservoir solves, 55 Krylov iterations, 0 cuts | N/A: product IMPES has no typed surface-u RESV injector lifecycle matching this deck | Strong same-state/local parity: first correction agrees to about `0.03%` or better and evaluation-1 terms to about `0.1-0.7%`. No six-step production-curve acceptance band yet. |
| Water-heavy pressure case, 12x12x3, 1 d | Corrected well-diameter oracle: `0.03-0.04 s` simulation; 1 step, 11 Newton, 12 evaluations, 13 Krylov iterations, 0 cuts | Default (`OpmAligned`), clean `663e380`: `~1.0 s` WASM, **4 substeps / 0 retries**, dt=[0.18,0.33] (`Legacy`: 24 substeps / 4 nonlinear retries, 4.6 s) | `2.35 s` WASM; 128 adaptive explicit substeps, finite/bounded state | FIM now within `4×` of Flow's single-substep solve (was `50` on the dirty-tree baseline). Final FIM pressure/rates still differ from Flow; IMPES remains a separate explicit-stability mechanism. |
| Default gas-rate control, 10x10x3, 6 x 0.25 d | Not comparable: tracked Flow deck uses the typed RESV/unlimited-redissolution lifecycle absent from this browser preset | Clean `663e380`: 1 substep/step, 0 retries; steady GOR=80; `Sg_max=0.438` | `0.09 s` WASM last step; ~4 adaptive substeps/step; `Sg_max=0.329` | Smoke/conservation evidence only. The materially different final gas saturation proves FIM and IMPES are not interchangeable correctness oracles here. |
| Areal sweep smoke, 21x21x1, 0.25 d | N/A: no tracked OPM deck | Clean `663e380`: `0.7 s` WASM; 6 substeps / 4 `linear-bad` (singularity backstop); `Sw_max=0.354` | `0.60 s` WASM; 37 adaptive substeps; `Sw_max=0.414` | Finite/bounded smoke only; no OPM or analytical field-level acceptance band. |
| Buckley-Leverett benchmarks | Bundled `wf_bl1d` artifact is parsed and physically active, but no solver-to-Flow acceptance band is defined | Not qualified on the BL benchmark gate | `2.78 s` for the three-test debug suite; breakthrough relative error `4.1%` (Case A) and `9.1%` (Case B); finer dt improves both | IMPES has the strongest quantitative correctness evidence here because the oracle is analytical. Do not use the OPM artifact as a numerical acceptance gate until bands are defined. |

## Timing contract

- OPM numbers are Flow's reported **simulation time**, excluding its approximately `0.07 s`
  deck/setup cost and output. Fresh process wall times were about `0.65 s` gas and `0.60 s` water.
- Native FIM times exclude Rust compilation. WASM times exclude module initialization and include
  only the requested `step()` calls. Native and WASM values should not be ratioed without naming
  the runtime surface.
- The convergence re-baseline rows (control matrix, three-phase, areal, SPE) are single-run
  observations on the **clean committed tree `663e380`** and are reproducible via the commands
  above. The `Exact gas RESV` row and the `G4c6` reconciliation below still carry their earlier
  `1db2db8` provenance until re-run; do not treat sub-100 ms differences as meaningful.

## G4c6 count reconciliation

The exact gas comparison previously called ResSim's `55` value “linear applications.” Explicit
counters show that was wrong: it is the sum of Krylov iterations across exactly 23 reservoir solve
calls and 23 applied Newton updates. Flow reports 27 Krylov iterations across 26 Newton updates;
its 32 `linearizations` are residual/Jacobian evaluations, not solve calls.

| Exact gas work unit | OPM Flow | ResSim FIM | Ratio / interpretation |
| --- | ---: | ---: | --- |
| Residual/Jacobian evaluations | 32 | 29 | ResSim `0.91x`; not the gap |
| Applied Newton updates | 26 | 23 | ResSim `0.88x`; not the gap |
| Reservoir solve calls | one per update (26, inferred from Flow Newton structure) | 23, directly counted | No duplicate/retry solves in ResSim |
| Krylov iterations | 27 | 55 | `2.04x` total |
| Krylov iterations / solve | `1.04` | `2.39` | Remaining linear-efficiency gap |

The nested local well solve runs during state update and does not contribute to the 55 Krylov
iterations. The existing full Flow-linear-lifecycle diagnostic is not a remedy: on the current
physics it uses 28 updates, 45 Krylov iterations, and `0.646 s`, slower than the G4c5 path. This
matches the earlier `FIM-Y2D6` verdict and is not grounds to repeat or promote that experiment.

## Decision

Stop treating exact gas as a greater-than-10x emergency: it is now roughly 2x Flow and has the
best correctness oracle in the repository. The heavy water trajectory, previously the headline gap
(`50` FIM substeps versus Flow's one), is now `4 substeps / 0 retries` on the clean-tree default —
its remaining `4→1` gap is no longer the dominant convergence problem. The highest-value next
convergence work is instead the **`linear-bad` singularity backstop on small well-dominated cases**
(`22×22×1`, `23×23×1`, `sweep-areal`): the WATER-025 raw-saturation default drives cells onto a
zero relperm-derivative endpoint, forming a singular Jacobian block that the load-bearing iterative
fallback only catches after the fact. The scoped fix is the deferred relperm-endpoint
regularization (`TODO.md:709`, `c2167f2`). IMPES remains the product solver and the preferred
two-phase analytical path. Re-open exact-gas linear policy only with a new same-preconditioner
capture showing a bounded recurrence or preconditioner change not already covered by `FIM-Y2D4`
through `FIM-Y2D6`.

## Replay commands

```bash
# Clean-tree convergence re-baseline (rebuild wasm first)
bash scripts/build-wasm.sh
D() { node scripts/fim-wasm-diagnostic.mjs "$@" --diagnostic summary --no-json; }
# two-phase control matrix
D --preset water-pressure --grid 20x20x3 --steps 1 --dt 0.25
D --preset water-pressure --grid 22x22x1 --steps 1 --dt 0.25
D --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25
D --preset water-pressure --grid 12x12x3 --steps 1 --dt 1        # heavy; add --legacy for the contrast
# three-phase
D --preset gas-rate     --grid 20x20x3 --steps 1 --dt 0.25
D --preset gas-rate     --grid 10x10x3 --steps 6 --dt 0.25
D --preset gas-pressure --grid 10x10x3 --steps 1 --dt 0.25
D --preset gas-pressure --grid 12x12x3 --steps 1 --dt 1
D --preset sweep-areal  --grid 21x21x1 --steps 1 --dt 0.25
# IMPES contrast: append --solver impes to any of the above
# SPE-suite (native)
cargo test --manifest-path src/lib/ressim/Cargo.toml --release spe1_fim -- --nocapture
```

```text
# Exact gas FIM native
FIM_Y1J_GRID=10 FIM_Y1J_FLAVOR=opm FIM_Y1J_STEPS=6 \
FIM_Y1J_GAS_REDISSOLUTION=1 FIM_Y2B_RAW_SATURATION=1 \
FIM_FLOW_RESV_INJECTOR=1 FIM_NESTED_WELL_SOLVE=1 \
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  fim::timestep::phase5_repro::repro_gas_rate_10x10x3_y1j \
  -- --ignored --nocapture --exact

# Water-heavy same WASM fixture, both ResSim solvers
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 \
  --dt 1 --steps 1 --gravity false --solver fim --opm-aligned --diagnostic quiet
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 \
  --dt 1 --steps 1 --gravity false --solver impes --diagnostic quiet

# General FIM/IMPES smoke comparison
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 10x10x3 \
  --dt 0.25 --steps 6 --solver impes --diagnostic quiet
node scripts/fim-wasm-diagnostic.mjs --preset sweep-areal --grid 21x21x1 \
  --dt 0.25 --steps 1 --solver impes --diagnostic quiet
```
