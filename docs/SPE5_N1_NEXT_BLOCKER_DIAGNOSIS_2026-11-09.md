# SPE5 N1 next blocker: branch selection, seed loss or cutoff?

**Status: diagnostic, no production behavior change. The next change is not chosen or implemented
here.** This note uses the failing call captured after N1 + difference refinement to separate the
three mechanisms before picking the next candidate. Measured on `a1bf7315` (branch
`diag/116-n1-next-blocker`), release build; the seeds and diagnostic tests are committed with it.

## Inputs

The bounded continuation in [SPE5_SPLIT_PHASE_DECISION_N1_2026-11-09.md](SPE5_SPLIT_PHASE_DECISION_N1_2026-11-09.md)
fails at:

| Seed | Day | Failing call `dt` | Previous attempt |
|---|---:|---:|---|
| `stall_n1_6612_836.json` | 6612.836114153414 | 0.04402126141911289 d | N1 + refinement, month 218, 10 attempts |
| `stall_n1_6612_869.json` | 6612.869119352101 | 0.0439567981404 d | same, `SPE5_MAX_ATTEMPTS=12` |

The seeds are committed from the driver's `SPE5_REPLAY_CHECKPOINT_OUT`; provenance is in
`opm/compositional/spe5/README.md`. Both are frozen failing-call states, not validated references.

## What the failing call does

At the first seed, the last two Newton iterates for cells 33/39 are:

| Iterate | Pressure [bar] | Standard flash | N1 flash | `sum ln^2 K` | Notes |
|---|---:|---|---|---:|---|
| 19 | 227.551557973 | TwoPhase, `beta = 0.517568` | TwoPhase | 0.2362 | below the bubble boundary |
| 20 | 228.156636560 | SingleLiquid | SingleLiquid | 488 (Wilson K) | above the boundary |

The residual alternates between `3.328e-3` and `5.041e-3` for iterations 3-20. The M1 continuation
puts cell 33's bubble-side collapse at `228.124845 bar`; the L iterate is `0.0318 bar` above that,
so single liquid is the correct branch there. The 2P iterate is `0.573 bar` below it, where the
split is physical. The two iterates are therefore **locally correct on opposite sides of a genuine
phase boundary**, not two labels for the same state.

### Branch selection

Yes, but not a wrong branch decision. The two-phase iterate has `sum ln^2 K = 0.236` and the
single-phase iterate is above the collapse pressure. Forcing either branch onto the other side
would be physically wrong. The Taylor test from iterate 19 to 20 is quadratic within the branch
(`error/h^2` about `1.7e-4` from `h = 1e-1` to `1e-5`) and only breaks at the full step, which
crosses the boundary; the scaled step reaches `dp = 0.615 bar`. The cycle is Newton's full step
crossing the phase boundary, not N1 choosing the wrong branch.

### Seed loss

Not causal here. At the L iterate, `test_stability_with_margin_seed` returns no margin seed, which
is expected: the feed is above the bubble boundary. Seeding the two-phase solver from the opposite
iterate's `K` at the L state returns only a trivial split (`beta = 0.5447`, `sum ln^2 K = 3.78e-13`,
`g_split - g_feed = +/-1.1e-15`, not a converged physical split). There is no physical split to
recover, so there is no seed-loss defect to fix at this call.

### Cutoff

Not causal here. The 2P iterate's `sum ln^2 K` is `0.236` (and the near-boundary 2P state at
`228.12627 bar` is `5.8e-5`), both above the `1e-5` trivial cutoff. The only candidate at the L
iterate is the trivial split above, which the cutoff correctly rejects. Lowering the cutoff would
admit a trivial state, not recover a physical split.

### N2

Not the first blocker for this call. The L state is on the bubble side, where single liquid is the
correct identity; N2's dew-side problem is a different cell/window. It remains a separate candidate
for the other crossing cells.

## Timestep evidence

The same failing-call checkpoint was traced over a dt ladder with N1 + refinement:

| Call | Last failing `dt` | First converging `dt` | Behaviour |
|---|---:|---:|---|
| first seed (`6612.836`) | 0.044021 d | 0.022011 d | 20-iteration 33/39 two-cycle at the failing dt; 4 iterations, `6.45e-10` at half |
| second seed (`6612.869`) | 0.043957 d | 0.000687 d | two-cycle persists through `1.37e-3 d`; 12 iterations, `1.57e-9` at `6.87e-4 d` |

A larger budget alone is not enough: `SPE5_MAX_ATTEMPTS=16` advances seven accepted steps from the
first seed to day `6612.878047`, then fails another month-218 call at `2.74e-7`; the N1 flash-override
count rises from 186 to 3,468. A fixed Newton step damping probe reaches `6.50e-6` at factor 0.5
but still stalls at 20 iterations (and at 40 iterations, `5.55e-6`), so uniform damping alone is not
the fix.

## Decision

The evidence points to **branch crossing under the current timestep/Newton controller** as the next
change, not N2, seed recovery or the trivial cutoff:

1. **Recommended:** a phase-boundary-aware timestep controller. Detect a persistent phase-label
   two-cycle (or a phase flip in the worst cells) and take an extra dt cutback, instead of spending
   the full 20-iteration budget and exhausting the retry ladder. The dt evidence shows the problem
   disappears as dt falls; this is the direct lever.
2. **If a solver-local change is preferred:** investigate a phase-boundary-aware step size or
   continuation. Uniform damping is insufficient on its own, so this needs a real crossing rule,
   not a blanket relaxation.
3. **Not next:** N2 (this call is bubble-side and its identity is correct), seed recovery (no
   physical split at the L state) and cutoff lowering (only a trivial candidate exists).
4. The larger retry budget is a useful diagnostic but not a policy: it advances a few steps while
   N1 overrides grow by an order of magnitude, then hits another boundary call.

## Reproduce

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_n1_next_call_phase_diagnostics -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_n1_next_call_step_damping_probe -- --ignored --nocapture

SPE5_TRACE_SEED=opm/compositional/spe5/stall_n1_6612_836.json \
    SPE5_FLASH_REFINE=1 SPE5_N1=1 SPE5_TRACE_HALVINGS=12 \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_trace_seed_ladder -- --ignored --nocapture
```

The first test writes `n1_next_call_diagnostics.json` with the flash/stability/seed/Gibbs records;
the second prints the damped-step sweep. Both are ignored diagnostics and change no production
path.
