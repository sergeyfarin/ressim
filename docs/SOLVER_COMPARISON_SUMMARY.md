# OPM Flow vs ResSim FIM vs ResSim IMPES

Updated 2026-07-22 on provisional dirty tree `1db2db8`. This is a decision table, not a claim
that every row is a three-way parity oracle. A solver is marked `N/A` when the repository lacks
the same control/physics mapping; it is marked `not qualified` when it runs but has no quantitative
correctness band against the named reference.

## Current result table

| Scenario | OPM Flow 2026.04 | ResSim FIM | ResSim IMPES | Correctness status |
| --- | --- | --- | --- | --- |
| Exact gas RESV, 10x10x3, 6 x 0.25 d | `0.08-0.10 s` simulation; 6 steps, 26 Newton updates, 32 residual evaluations, 27 Krylov iterations, 0 cuts | `0.191-0.207 s` native; 6 steps, 23 updates, 29 evaluations, 23 reservoir solves, 55 Krylov iterations, 0 cuts | N/A: product IMPES has no typed surface-u RESV injector lifecycle matching this deck | Strong same-state/local parity: first correction agrees to about `0.03%` or better and evaluation-1 terms to about `0.1-0.7%`. No six-step production-curve acceptance band yet. |
| Water-heavy pressure case, 12x12x3, 1 d | Corrected well-diameter oracle: `0.03-0.04 s` simulation; 1 step, 11 Newton, 12 evaluations, 13 Krylov iterations, 0 cuts | Production/default: `5.78 s` WASM (`4.38 s` native), 50 substeps/2 retries. WATER-003 default-off: `0.78 s` native, 10 substeps/0 retries | `1.58 s` WASM; 129 adaptive explicit substeps, finite/bounded state | WATER-003 closes update 1: injector `p=390,Sw=.3`, water MB `.313756` vs Flow `.31375`. Not promoted: the direct day exceeds 20 updates and final FIM pressure/rates differ from Flow. IMPES remains a separate explicit-stability mechanism. |
| Default gas-rate control, 10x10x3, 6 x 0.25 d | Not comparable: tracked Flow deck uses the typed RESV/unlimited-redissolution lifecycle absent from this browser preset | `3.47 s` WASM; 28 substeps, 13 nonlinear retries; `Sg_max=0.4570` | `0.60 s` WASM; 26 adaptive substeps; `Sg_max=0.3291` | Smoke/conservation evidence only. The materially different final gas saturation proves FIM and IMPES are not interchangeable correctness oracles here. |
| Areal sweep smoke, 21x21x1, 0.25 d | N/A: no tracked OPM deck | `0.33 s` WASM; 1 substep; `Sw_max=0.3537` | `0.51 s` WASM; 37 adaptive substeps; `Sw_max=0.4137` | Finite/bounded smoke only; no OPM or analytical field-level acceptance band. |
| Buckley-Leverett benchmarks | Bundled `wf_bl1d` artifact is parsed and physically active, but no solver-to-Flow acceptance band is defined | Not qualified on the BL benchmark gate | `2.78 s` for the three-test debug suite; breakthrough relative error `4.1%` (Case A) and `9.1%` (Case B); finer dt improves both | IMPES has the strongest quantitative correctness evidence here because the oracle is analytical. Do not use the OPM artifact as a numerical acceptance gate until bands are defined. |

## Timing contract

- OPM numbers are Flow's reported **simulation time**, excluding its approximately `0.07 s`
  deck/setup cost and output. Fresh process wall times were about `0.65 s` gas and `0.60 s` water.
- Native FIM times exclude Rust compilation. WASM times exclude module initialization and include
  only the requested `step()` calls. Native and WASM values should not be ratioed without naming
  the runtime surface.
- Values are single-run observations on a dirty tree and therefore provisional. Use ranges where
  a prior same-tree-equivalent run exists; do not treat sub-millisecond differences as meaningful.

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
best correctness oracle in the repository. The highest-value next convergence work is the
water-heavy trajectory (`50` FIM substeps versus Flow's one), while IMPES remains the product
solver and the preferred two-phase analytical path. Re-open exact-gas linear policy only with a
new same-preconditioner capture showing a bounded recurrence or preconditioner change that was not
already covered by `FIM-Y2D4` through `FIM-Y2D6`.

## Replay commands

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
