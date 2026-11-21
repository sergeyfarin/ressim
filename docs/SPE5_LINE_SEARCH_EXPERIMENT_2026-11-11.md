# SPE5 line search with and without carried K: accepted progress?

**Status: diagnostic, test-only, not promoted. Production Newton behavior is unchanged.** Measured
on `ac248a64` (branch `exp/116-line-search`), release build. The four-mode comparison reproduces
the earlier table exactly. The continuation is **INCONCLUSIVE** after review: the measured build
assembled the winning trial twice, so its wall-budget comparison is not valid (see below).

This is the experiment #134's conclusion asked for: pair carried-`K` seed recovery with a step that
evaluates intermediate inputs, then test whether the recovered phase path turns into accepted
progress. The comparison holds the seed, timestep, tolerance, N1 and difference refinement fixed:

| Setting | Value |
|---|---|
| Seed | `opm/compositional/spe5/stall_n1_6612_836.json` |
| Timestep | `0.04402126141911289 d` (the captured failing call) |
| Acceptance | `1e-8` scaled residual, unchanged |
| Flash | difference refinement on, N1 on |
| Recovery | per-cell carried `K`, attempt-owned, on or off |
| Line search | monotone backtracking on or off |

## What the line search does

Test-only `with_line_search_for_test` in `compositional/newton.rs`. After the ordinary
fraction-to-boundary step it assembles the candidate state at `tau = 1`, then halves `tau` up to 20
times. A trial is accepted only when the full scaled residual satisfies an Armijo-style decrease,

```text
R(x + tau dx) <= R(x) (1 - 1e-4 tau).
```

Every trial is assembled with the attempt's fixed carried-`K` hint, so evaluating a candidate
cannot change the operator the next candidate sees. When recovery is off, the same trial states use
the cold flash. The report gains `line_search_history` with each trial's `tau`, residual, the cells
whose phase label differs from the current iterate, and the accept/reject flag. The winning trial's
assembly is reused at the next iteration, so an accepted step is assembled once, not twice. The
unit test `comp_newton_line_search_backtracks_across_the_switch` pins the mechanism on a one-cell
near-critical depletion (full step `1.0`, then `0.5`, then `1.0`) and checks that the final state is
on the two-phase branch.

## Result at the captured timestep

| Line search | Recovery | Accepted step | Iterations | Best residual | Final residual |
|---|---|---|---:|---:|---:|
| off | off | no | 20 | `3.328116e-3` | `5.041079e-3` |
| off | on | no | 20 | `3.328116e-3` | `5.041079e-3` |
| on | off | no | 14 | `2.403354e-6` | `2.403354e-6` |
| on | **on** | no | 18 | **`1.594464e-6`** | **`1.594464e-6`** |

The no-line-search rows reproduce #134: recovery changes nothing because the full step jumps from
`227.551` to `228.156 bar` and never evaluates the premature switch.

The line-search rows are different. Both accept many decreasing Newton steps (13 for recovery off,
17 for recovery on) and reduce the residual by three orders of magnitude, but neither reaches
`1e-8`. The final iteration is a failed line search, not an iteration-budget stop:

- **Recovery off, iteration 14** (current residual `2.403354e-6`): all 20 halvings are above the
  current residual. The closest candidate is `4.147286e-6` at `tau = 3.906e-3`; the full step is
  `1.087134e-5`.
- **Recovery on, iteration 18** (current residual `1.594464e-6`): all 20 halvings are above the
  current residual. The closest candidate is `5.687248e-6` at `tau = 3.815e-6`; the full step is
  `1.229775e-5`.

So carrying `K` lowers the line-search barrier by about 1.5x and keeps the descent going for four
more Newton steps, but the monotone search still stops at a residual barrier. It does not produce an
accepted timestep, and it does not reach the unchanged `1e-8` test. A 60-iteration cap reproduces
the same 14/18 stops, so this is not an iteration-budget artifact.

The per-trial phase trace separates two things the earlier summary conflated:

- In iterations 1–9, the full-step trials change cells **33/39** from the current iterate's phase
  labels. Those are the trials that cross the premature switch; most are rejected, and the accepted
  half-step stays on the near side.
- At the final failed iteration (14 with recovery off, 18 with recovery on) **no trial changes a
  phase label**. The stop is therefore a residual barrier on the current branch, not a label switch
  inside the trial. This experiment does not establish whether a root exists on either branch, or
  which mechanism makes the residual rough there (the failure-B Taylor test already found
  non-smooth face/well terms at cells 33/39).

## Sequential continuation — INCONCLUSIVE

The 600 s wall-budget continuation was measured before the accepted-trial reuse fix. The winning
trial was assembled twice, so the line-search modes paid an artificial extra assembly per accepted
step and the wall-budget table is not a valid comparison. It is withdrawn.

The per-accepted-step work is unaffected by that duplicate assembly and still shows the line search
doing less work:

| Mode | Failed attempts / accepted step | Accepted Newton iterations / accepted step |
|---|---:|---:|
| no line search (recovery off/on) | `92/15 = 6.1` | `200/15 = 13.3` |
| line search, recovery off | `36/8 = 4.5` | `74/8 = 9.3` |
| line search, recovery on | `28/7 = 4.0` | `65/7 = 9.3` |

So the earlier "no sustained accepted-progress advantage" reading is withdrawn with the table.
Sustained progress needs a fair rerun after the reuse fix; it does not decide the direction of this
PR.

## Verdict

**DIAGNOSTIC — CARRYING K LOWERS THE MONOTONE LINE-SEARCH BARRIER BUT DOES NOT DELIVER AN ACCEPTED
TIMESTEP AT `1e-8`. The final stop is a residual barrier with no trial phase-label change; line
search alone neither crosses the cutoff nor establishes that a root exists.**

At the same timestep and tolerance, carrying `K` reduces the residual within a failed attempt.
The early full steps change cells 33/39. The final search stays on the two-phase branch at both
cells. The earlier barrier probe reported that the dominant scaled row changes at cells 33 and 39
remained after subtracting accumulation. The captured case has well completions only in cells 0
and 146, so the remainder at cells 33 and 39 is attributed to faces. The probe was run on a
separate diagnostic branch. The paired flash measurement below uses its own replay and checks
thermodynamic equilibrium directly. [Barrier-probe review in #116](https://github.com/sergeyfarin/ressim/issues/116#issuecomment-6813995330)

## Vapour-fraction precision at the final search

Measured on clean code revision `451f222d7cf4e8237b1cbf8054a1a42c00cc44f2` with mpmath 1.4.1.
The release replay uses `stall_n1_6612_836.json`, `dt = 0.04402126141911289 d`, N1 and the
difference-form refinement. Both recovery modes use the same `1e-8` Newton tolerance. The test
exports cells 33 and 39 at the failed search's current iterate and at `tau = 1e-6`, reconstructed
from the same Newton direction. Re-evaluation holds the attempt's carried-`K` hint when recovery
is on. The float64 flashes are two-phase at all eight inputs. The largest input changes are
`4.88e-7 Pa` in pressure and `2.22e-16` in any overall mole fraction.

The independent Peng–Robinson solver uses the same binary64 pressure and composition inputs and
normalizes each composition in high precision. It solves for equilibrium `ln K` and `beta` at
50 digits, with fugacity/Rachford–Rice residuals below `3.6e-41` across the eight solves.
Each solution has `0 < beta < 1`, `sum ln² K > 1e-5` and lower Gibbs energy than the feed.
Repeating at 70 digits and seeding each trial from its corresponding current equilibrium
reproduces every solved coordinate within `1e-20`.

| Carried K | Cell | Float64 Δβ | 50-digit Δβ | Float64 max abs Δln K | 50-digit max abs Δln K | Float64 flash iterations, current → trial | Float64 equilibrium residual, current → trial |
|---|---:|---:|---:|---:|---:|---|---|
| off | 33 | `+3.5908e-3` | `+2.4183e-10` | `7.7023e-7` | `4.7729e-11` | 15 → 15 | `3.878e-12 → 5.443e-12` |
| off | 39 | `-2.3391e-3` | `-6.2857e-10` | `6.1070e-7` | `1.1956e-10` | 20 → 15 | `8.882e-16 → 4.448e-12` |
| on | 33 | `-1.5895e-3` | `-1.8655e-10` | `4.8507e-7` | `3.4126e-11` | 20 → 16 | `5.329e-15 → 1.013e-12` |
| on | 39 | `-2.3701e-7` | `+3.8945e-10` | `1.0480e-10` | `6.8349e-11` | 20 → 19 | `4.441e-15 → 8.882e-15` |

The export records the refinement decision at each point. It also repeats the flashes at the
same inputs with refinement disabled and with just the test-only move cap raised from `1e-3` to
`1e-1`. The cap bounds the allowed change in β or in any component of `ln K`.

| Carried K | Cell | Default refinement, current → trial | Default abs β error | Unrefined abs β error | Wider-cap abs β error | Wider-cap Δβ |
|---|---:|---|---|---|---|---:|
| off | 33 | capped → capped | `1.346e-3 → 2.245e-3` | `1.346e-3 → 2.245e-3` | `1.357e-8 → 8.556e-7` | `-8.418e-7` |
| off | 39 | kept → capped | `1.873e-7 → 2.339e-3` | `5.268e-4 → 2.339e-3` | `1.873e-7 → 1.179e-6` | `+9.909e-7` |
| on | 33 | kept → capped | `2.646e-6 → 1.592e-3` | `2.942e-4 → 1.592e-3` | `2.646e-6 → 1.806e-6` | `+4.452e-6` |
| on | 39 | kept → kept | `2.665e-6 → 2.902e-6` | `9.913e-4 → 2.386e-4` | `2.665e-6 → 2.902e-6` | `-2.370e-7` |

**DIAGNOSTIC.** The default cap discards the refinement at one or both ends of all three
large-jump pairs. The wider cap retains all eight refinements and reduces the largest paired
change from `3.59e-3` to `4.45e-6`. The sampled unrefined β errors are `2.39e-4` to `2.34e-3`
with N1 and the stated recovery settings. At these near-critical inputs, meeting the fugacity
tolerance alone allows appreciably larger β errors.

Both refinements were already kept at cell 39 with carried K, and its `-2.37e-7` change remains
with the wider cap. The high-precision change has the opposite sign. The reported unsigned
maximum of `Δln K` across components cannot separate errors in `K` from their propagation into β
through the ill-conditioned Rachford–Rice equation.

First rerun the whole captured `0.04402126141911289 d` call with the test-only `1e-1` cap, line
search, both carried-K modes and Newton tolerance `1e-8`. That will test whether the fixed-input
improvement carries over to the trajectory before considering a relative-permeability blend.
The 20-year replay and real-case shut-in remain open under #116 and #98.

## Reproduce

```bash
# Four-mode comparison at the captured timestep: the table above.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_line_search_recovery_comparison -- --ignored --nocapture

# Paired flash precision measurement.
spe5_trace=$(mktemp -d)
SPE5_TRACE_OUT="$spe5_trace" cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_line_search_recovery_comparison -- --ignored --nocapture
uv run --no-project --with mpmath==1.4.1 python -I tools/jutul/spe5_flash_mp.py --line-search \
    "$spe5_trace/spe5_line_search_flash.json" "$spe5_trace/precision.json"

# Line-search mechanism (fast, not ignored).
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_newton_line_search_backtracks_across_the_switch -- --nocapture

# Continuation is INCONCLUSIVE until rerun after the accepted-trial reuse fix; not a gate.
SPE5_REPLAY_OUT=DIR/ls.json SPE5_REPLAY_SEED=stall_n1_6612_836.json \
    SPE5_MAX_DT_DAYS=0.04402126141911289 SPE5_FLASH_REFINE=1 SPE5_N1=1 \
    SPE5_LINE_SEARCH=1 SPE5_LAST_MONTH=219 SPE5_WALL_BUDGET_S=600 \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_replay_with_timestep_options -- --ignored --nocapture
```

`SPE5_LINE_SEARCH` is a test-only replay-driver flag. It is absent from production builds. The
default Newton path's results and its `step_scale_history` on the failure exits are unchanged from
#134; only test-only reporting fields and the line-search path changed.
