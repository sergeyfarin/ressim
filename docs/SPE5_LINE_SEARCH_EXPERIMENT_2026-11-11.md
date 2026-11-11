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

The experiment answers the question as stated: at the same timestep and tolerance, carrying `K`
contributes local residual reduction inside a failed attempt, not accepted progress. The trial
traces also show Newton converging onto the near side of a discontinuity: the full steps keep
changing cells 33/39, while the accepted half-steps stay on the current branch and close half the
remaining distance each time.

**Next: check whether this timestep has a root at all before adding another step-control mechanism.**
At the stall seed, freeze the cell 33/39 labels to two-phase (using the N1 split), solve, and check
that the flash agrees at the solution; repeat with single-phase frozen. If neither branch agrees
with itself, there is no root at this timestep under the current rules, and non-monotone steps or
continuations cannot fix it. That result decides between making the model continuous (the cutoff
rule for 33/39, N2 for 25/31 and 16, or near-critical relative permeability) and an explicit
acceptance rule with a bounded signed mass imbalance. The `1e-8` acceptance test and the far liquid
negative control stay unchanged until that decision.

## Reproduce

```bash
# Four-mode comparison at the captured timestep: the table above.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_line_search_recovery_comparison -- --ignored --nocapture

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
