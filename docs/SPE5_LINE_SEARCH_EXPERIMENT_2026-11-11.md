# SPE5 line search with and without carried K: accepted progress?

**Status: diagnostic, test-only, not promoted. Production Newton behavior is unchanged.** Measured
on `da98153c` (branch `exp/116-line-search`), release build. The four-mode comparison and the
continuation table were rerun on that commit after the tree was cleaned; the comparison reproduced
exactly, and the continuation is the clean-tree measurement below.

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
the cold flash. The report gains `line_search_history` with each trial's `tau`, residual and
accept/reject flag. The unit test `comp_newton_line_search_backtracks_across_the_switch` pins the
mechanism on a one-cell near-critical depletion (full step `1.0`, then `0.5`, then `1.0`).

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
more Newton steps, but the monotone search still stops at the phase-boundary residual jump. It does
not produce an accepted timestep, and it does not reach the unchanged `1e-8` test. A 60-iteration
cap reproduces the same 14/18 stops, so this is not an iteration-budget artifact.

## Sequential continuation

A supporting run from the same seed, capped at the same `0.04402126141911289 d`, with N1 +
refinement and a 600 s wall budget per mode. Measured on `da98153c` after the tree was cleaned.
These are cost-normalized results, not a fixed-horizon comparison.

| Line search | Recovery | Accepted steps | Day reached | Failed attempts | Accepted Newton iterations |
|---|---|---:|---:|---:|---:|
| off | off | 15 | `6612.878674` | 92 | 200 |
| off | on | 15 | `6612.878674` | 92 | 200 |
| on | off | 8 | `6612.877212` | 36 | 74 |
| on | on | 7 | `6612.877040` | 28 | 65 |

Without a line search, recovery on and off are identical on this capped continuation, consistent
with #134's full-step skip. With the line search, recovery on reduces failed attempts and Newton
work, but not the simulated distance reached. The line search reduces Newton iterations per
accepted step (`74/8 = 9.25` versus `200/15 = 13.3`) but its trial assemblies cost more wall time,
so it advances less simulated time inside the same budget. No line-search mode converts the barrier
into sustained accepted progress on this horizon.

## Verdict

**DIAGNOSTIC — CARRYING K LOWERS THE MONOTONE LINE-SEARCH BARRIER BUT DOES NOT DELIVER AN ACCEPTED
TIMESTEP AT `1e-8`; LINE SEARCH ALONE DOES NOT CROSS THE CUTOFF.**

The experiment answers the question as stated: at the same timestep and tolerance, carrying `K`
contributes local residual reduction inside a failed attempt, not accepted progress. The mechanism
that is still missing is a step that crosses the narrow cutoff band, not another evaluation of the
near side. A bounded non-monotone step or a short continuation across the cutoff, and the separate
N2/cutoff-rule work, are the next candidates; the `1e-8` acceptance test and the far liquid negative
control stay unchanged.

## Reproduce

```bash
# Four-mode comparison at the captured timestep: the table above.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_line_search_recovery_comparison -- --ignored --nocapture

# Line-search mechanism (fast, not ignored).
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_newton_line_search_backtracks_across_the_switch -- --nocapture

# Supporting continuation; run once with recovery off and once with `SPE5_SEED_RECOVERY=1`.
SPE5_REPLAY_OUT=DIR/ls.json SPE5_REPLAY_SEED=stall_n1_6612_836.json \
    SPE5_MAX_DT_DAYS=0.04402126141911289 SPE5_FLASH_REFINE=1 SPE5_N1=1 \
    SPE5_LINE_SEARCH=1 SPE5_LAST_MONTH=219 SPE5_WALL_BUDGET_S=600 \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_replay_with_timestep_options -- --ignored --nocapture
```

`SPE5_LINE_SEARCH` is a test-only replay-driver flag. It is absent from production builds, and the
default Newton path is bit-identical to #134.
