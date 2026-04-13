---
name: FIM Slice A — Newton Initial-Guess Extrapolation
description: Observation notes and measured baseline for the Slice A Newton initial-iterate extrapolation attempt
date: 2026-04-13
baseline_commit: fd72bdb
---

# FIM Slice A — Newton Initial-Guess Extrapolation

## Motivation

OPM Flow solves SPE1 with 30-day outer steps and ~2.5 Newton iterations on
average. The equivalent ResSim configurations (12x12x3 / 20x20x3 /
23x23x1 water-pressure at dt≥0.25d, gas-rate at dt=0.25d) sit 1-2 orders of
magnitude away. The current tracker (`TODO.md` "Now" section and the compact
rung telemetry at `TODO.md:120`) names two dominant ladders:

1. **Cold-start chop** on the very first outer step. The initial substep is
   capped hard (`0.25d` or `1d`) and then must grow; every clean-accepted
   substep costs one full-size Newton solve.
2. **Water@1020 regrowth collapse** (and its gas-cell analogue) where the
   solver re-enters the same producer hotspot every time dt grows back up.

Several 2026-04-11/12 slice experiments (reverted) showed that micro-changes
to retry shaping preserve rather than reduce real retry count. The remaining
headroom is to reduce Newton iteration count on the **clean** substeps that
follow the initial ladder — i.e. help Newton converge in fewer iterations on
the smooth portions of the trajectory, which will in turn let dt grow faster
and reach OPM-scale step size earlier.

## Hypothesis

After two clean accepted substeps, the trajectory is locally smooth in dt.
A linear time-extrapolation

```
iterate_0 = prev + (dt_new / dt_prev) * (prev - prev_prev)
```

placed inside the admissible Appleyard-like box (|Δp| ≤ 200 bar, |Δs| ≤ 0.1)
should shed ~1 Newton iteration on clean substeps where Newton otherwise
starts from the previous state. This is exactly what OPM Flow's
`extrapolateInitialGuess` does (`opm-simulators: BlackoilModel.cpp`).

Scope guard: **only** applied on clean substeps (`retry_count == 0`) where
two consecutive clean accepts are available. Retries never extrapolate.

## Pre-change baseline

Rebuilt wasm at clean tip `fd72bdb` (see `git log -1`) and ran the six
validation commands verbatim.

| # | Command | substeps | retries (L/N/M) | dt range (days) | outer_ms | growth |
|---|---------|----------|-----------------|-----------------|----------|--------|
| 1 | `water-pressure --grid 12x12x3 --steps 1 --dt 1` | 16 | 0/9/0 | 4.167e-5 … 3.864e-2 | 1821.0 | hotspot-repeat |
| 2 | `water-pressure --grid 20x20x3 --steps 1 --dt 0.25` | 12 | 0/5/0 | 7.813e-3 … 6.250e-2 | 17667.1 | newton-iters |
| 3 | `water-pressure --grid 22x22x1 --steps 1 --dt 0.25` | 4 | 0/2/0 | 6.250e-2 … 6.250e-2 | 1410.0 | newton-iters |
| 4 | `water-pressure --grid 23x23x1 --steps 1 --dt 0.25` | 4 | 0/2/0 | 6.250e-2 … 6.250e-2 | 1117.2 | newton-iters |
| 5 | `gas-rate --grid 10x10x3 --steps 6 --dt 0.25` (step 6) | 4 | 0/0/0 | 4.412e-2 … 7.494e-2 | 237.7 | newton-iters |
| 6 | `gas-rate --grid 20x20x3 --steps 1 --dt 0.25` | 4 | 0/2/0 | 6.250e-2 … 6.250e-2 | 2660.2 | hotspot-repeat |

`gas-rate --steps 6` per-step ladder (diagnostic=outer):

| step | substeps | retries | dt range | growth | outer_ms |
|------|----------|---------|----------|--------|----------|
| 1 | 8 | 0/3/0 | 4.042e-3 … 4.232e-2 | max-growth | 764.1 |
| 2 | 4 | 0/2/0 | 6.250e-2 | hotspot-repeat | 449.4 |
| 3 | 4 | 0/0/0 | 5.005e-2 … 7.089e-2 | newton-iters | 286.4 |
| 4 | 4 | 0/0/0 | 4.809e-2 … 7.223e-2 | newton-iters | 272.7 |
| 5 | 4 | 0/0/0 | 4.807e-2 … 7.256e-2 | newton-iters | 251.7 |
| 6 | 4 | 0/0/0 | 4.412e-2 … 7.494e-2 | newton-iters | 234.6 |

Observations on baseline shape:

- Runs 3, 4, 5, 6 and gas-rate steps 3-6 are all "4 substeps, newton-iters
  limited" with 0 or near-0 retries. These are precisely the clean smooth
  substeps where initial-guess extrapolation is expected to help.
- Run 1 and run 2 are dominated by cold-start + hotspot-repeat; Slice A is
  not expected to help them on the first several substeps, but any smooth
  stretch after dt re-stabilises should benefit.
- Run 2 (20x20x3) is linear-solver-bound (`lin_ms=17246` of `outer_ms=17667`).
  Even if Slice A shaves a Newton iteration or two, savings scale with the
  linear-solve cost, so this is the single run most sensitive to the change.

## Change

Implemented in `src/lib/ressim/src/fim/timestep.rs` (substep loop only). The
Newton entry point `run_fim_timestep()` already accepts a separate
`initial_iterate` argument — previously always set equal to `previous_state`.
Slice A replaces that argument with a bounded extrapolation when the guards
are met.

Guards:

1. `retry_count == 0` (never on retries).
2. History of two consecutive clean accepts present (i.e. the
   last-accepted substep did not itself come from a retry, and one accept
   earlier than it is also known).
3. `last_accepted_dt_days > 0`.

Per-cell extrapolation with conservative bounds:

- Pressure: `p + alpha * dp`, clamped to |delta| ≤ `max_pressure_change_bar`
  (200 bar) and pressure ≥ 0.1 bar.
- Water saturation: `sw + alpha * dsw`, clamped to |delta| ≤
  `max_saturation_change` (0.1) and `sw ∈ [0.001, 0.999]`.
- Hydrocarbon variable: only extrapolated when both endpoint regimes match
  the current (most-recent-accepted) regime. In Saturated regime, |delta|
  ≤ `max_saturation_change`; in Undersaturated regime, |delta| ≤
  `max_rs_change_fraction * max(|Rs|, 1)`. On regime mismatch, fall back to
  `current.hydrocarbon_var`.
- Well BHP: `bhp + alpha * dbhp`, clamped as for pressure.
- Perforation rates: not extrapolated (kept at the most-recent-accepted
  value). These re-stabilise within one Newton step.

On retry-triggered rejections, the history is preserved (the most-recently
accepted state is still valid); the next clean substep retry may still
extrapolate. On retries themselves the initial iterate reverts to
`previous_state` (current behaviour).

## Post-change measurements

Two iterations were needed. The first unguarded pass was an obvious
regression; a second pass added a "plateau / stationary" guard to preserve
the replay optimization. Both results are recorded here.

### Iteration 1 — unguarded extrapolation

| # | Baseline outer_ms / substeps / retries | Post1 | Verdict |
|---|---------------------------------------|-------|---------|
| 1 | 1821.0 / 16 / 9  | 14797.6 / 148 / 45 | **8x slower** |
| 2 | 17667.1 / 12 / 5 | 12483.4 / 7 / 3 | 1.4x faster |
| 3 | 1410.0 / 4 / 2 | 938.0 / 4 / 2 | 1.5x faster |
| 4 | 1117.2 / 4 / 2 | 935.5 / 4 / 2 | 1.2x faster |
| 5 step 1 | 764.1 / 8 / 3 | 773.5 / 8 / 4 | +1 retry |
| 5 step 3 | 286.4 / 4 / 0 | 281.3 / 4 / 1 | +1 retry |
| 5 step 4 | 272.7 / 4 / 0 | 370.9 / 4 / 2 | 1.4x slower |
| 5 step 5 | 251.7 / 4 / 0 | 239.8 / 4 / 1 | +1 retry |
| 5 step 6 | 234.6 / 4 / 0 | 359.9 / 4 / 2 | 1.5x slower |
| 6 | 2660.2 / 4 / 2 | 2681.0 / 4 / 2 | wash |

Root cause: on run 1 the solver normally produces
`accepts=15+4+8354` — 15 real Newton accepts plus 8358 "replayed" accepts.
The replay path requires `iterate_has_material_change(previous_state,
accepted_state) == false` at 1e-12 epsilon. Extrapolation moves the Newton
starting iterate, so Newton converges to a state that differs from
`previous_state` by roughly `residual_tolerance ≈ 1e-5` — well above the
1e-12 replay epsilon. Replay is disabled, and every plateau tick becomes
a real Newton solve.

### Iteration 2 — stationary + plateau + alpha-ceiling guards

Guards added:

- Alpha ceiling at 2.0 to prevent amplified extrapolation when dt grows.
- Skip history save when the previous accept was stationary (max_dsat <
  1e-4 AND max_dp < 0.01 bar).
- Skip history save when the previous accept's growth limiter was
  `hotspot-repeat` or `retry-hold` (plateau regime).

| # | Baseline outer_ms / substeps / retries | Post3 | Verdict |
|---|---------------------------------------|-------|---------|
| 1 | 1821.0 / 16 / 9 | 4492.6 / 37 / 20 | **2.5x slower** |
| 2 | 17667.1 / 12 / 5 | 16213.1 / 10 / 5 | 1.09x faster |
| 3 | 1410.0 / 4 / 2 | 883.7 / 4 / 2 | 1.6x faster |
| 4 | 1117.2 / 4 / 2 | 934.3 / 4 / 2 | 1.2x faster |
| 5 step 1 | 764.1 / 8 / 3 | 788.4 / 8 / 4 | +1 retry |
| 5 step 3 | 286.4 / 4 / 0 | 286.4 / 4 / 1 | +1 retry |
| 5 step 4 | 272.7 / 4 / 0 | 464.1 / 4 / 2 | 1.7x slower |
| 5 step 5 | 251.7 / 4 / 0 | 246.6 / 4 / 1 | +1 retry |
| 5 step 6 | 234.6 / 4 / 0 | 416.2 / 4 / 2 | 1.8x slower |
| 6 | 2660.2 / 4 / 2 | 2609.4 / 4 / 2 | wash |

The plateau guard dropped run 1 from 8x to 2.5x slower, but did not
eliminate the regression. More importantly, on the clean gas-rate
trajectory (step 3–6 of run 5), where extrapolation was **supposed** to
help, it introduced new retries. The extrapolated iterate is landing
outside the Newton convergence basin on these cells, forcing retries that
don't exist in the baseline.

## Correctness checks

All three locked-baseline tests pass (correctness preserved, only
performance regressed):

- `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas` — ok
- `spe1_fim_first_steps_converge_without_stall` — ok (23.79s)
- `spe1_fim_gas_injection_creates_free_gas` — ok (37.51s)

## Verdict

**Revert.** Slice A as designed is a net regression in the current
ResSim baseline. The two structural obstacles revealed by the experiment:

1. The replay optimization (`accepts=R+C+P`) is an order-of-magnitude
   performance lever that depends on Newton converging to a state
   bitwise-equal to `previous_state` on plateau substeps. Any initial
   iterate that isn't `previous_state` breaks this, because residual
   tolerance (1e-5) ≫ replay equality epsilon (1e-12). The plateau
   guard narrows but doesn't eliminate the breakage.
2. On nominally clean substeps (gas-rate step 3+ smooth region), the
   extrapolation pushes Newton far enough that new retries are
   triggered. This suggests the extrapolated state lands outside the
   correct convergence basin for some cells — most likely the cells
   near the gas-oil contact or the producer hotspot where the local
   nonlinearity is strongest.

Future revisit of this idea should condition on:

- Running the extrapolation only in a "truly smooth" regime (e.g.,
  no hotspot-repeat in the last K accepts, no retries in the last M
  substeps, no regime flips in any cell).
- Applying extrapolation **per-cell** rather than globally — extrapolate
  only in cells that moved smoothly and at well-behaved magnitude, keep
  hotspot cells anchored to `previous_state`.
- Making the replay-equality epsilon (currently 1e-12) dt-aware so a
  tiny smooth extrapolation doesn't disable replay — but this is a
  separate, larger plumbing change that touches the replay path.

Slice B (DSMAX/DPMAX trial_dt shaping) and Slice C (in-Newton variable
switching) were not attempted in this experiment per the user's scope.
