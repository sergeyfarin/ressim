# SPE5 carried-K seed recovery: complete path and its limit

**Status: experimental, test-only, not promoted. Production flash behavior is unchanged; the
original residual acceptance, equilibrium, physicality, non-triviality and Gibbs checks are
retained.** Measured on `3113aa85` (branch `exp/116-seed-recovery`, stacked on the #133 diagnostic
head), release build.

This follows the #133 finding that the N1 two-phase→liquid switch at `228.126683` is premature:
the upper input still has a physical split, but both stability trials have become trivial and N1
loses its seed. The experiment carries a cell's previous flash `K` as a direct split hint, re-solves
at the current inputs, and accepts only through the ordinary checks.

## Design

- **Hint.** `flash_for_cell(cell, …)` reads a per-cell `K` committed at the previous assembly
  boundary. Flash calls during one assembly stages their returned `K`; the staging map is committed
  only after that assembly, so every caller in the iteration sees one consistent hint. Flashes
  outside an assembly (direct diagnostics) read the committed hint and do not mutate it, so repeated
  evaluation of the same input returns the same state. The hint is passed as the flash's `k_seed`.
- **Complete recovery path.** When the stability verdict is stable, the carried `K` is used directly
  as the two-phase split seed, not only as a stability-trial seed. The split is re-solved at the
  current `(p, T, z)` by the same two-phase solver and must pass `TwoPhase`, `sum ln^2 K >= 1e-5`,
  finite physical compositions and the existing Gibbs guard. Otherwise the standard single-phase
  answer stands.
- **Consistent properties.** The returned `FlashState` is the one used by the accumulation residual,
  both sides of every face flux, well connections and the derivative pass; no caller re-flashes.
- **Determinism / rejection / restart.** The cache resets at the start of every Newton solve. A
  rejected attempt and a restart therefore begin cold, not from a stale hint. The regression
  `comp_spe5_seed_recovery_repeated_flash_and_reset` checks that two consecutive upper-input flashes
  return the same `beta` and `K`, and that a reset makes the same input cold (`SingleLiquid`).
  Nothing is stored in a checkpoint.
- **Production.** `flash_for_cell` delegates to `flash` in non-test builds; the cache and switch are
  `cfg(test)` only.

## Results

### The complete path recovers the switch

At the adjacent switch inputs (`n1_switch_6612_cell33.json`, cell 33, same `z`):

| Mode | Lower input | Upper input |
|---|---|---|
| recovery off | TwoPhase, `beta 0.460167` | **SingleLiquid** |
| recovery on | TwoPhase | **TwoPhase, `beta 0.439250`**, `sum ln^2 K > 1e-5` |

The committed actual-path carried seed gives `beta = 0.458176`, `sum ln^2 K = 1.64531e-5`,
`g_split - g_feed = -3.77e-15`. Two consecutive upper flashes return the same `beta` and `K`; after
a cache reset the same input is cold and returns `SingleLiquid`. The far liquid endpoint remains a
negative control: a split seeded from the far two-phase iterate is trivial (`sum ln^2 K = 4.3e-13`)
and is rejected.

### But the full Newton step skips the switch

The captured call's step jumps from `227.551436` to `228.156518 bar`. The switch at `228.126683` is
never an input of the solve, so carrying `K` from the previous iterate does not change the full
call:

| Mode | Iterations | Best | Last |
|---|---:|---:|---:|
| N1 + refinement | 20 | `3.328197e-3` | `5.046303e-3` |
| N1 + refinement + carried `K` | 20 | `3.328197e-3` | `5.046303e-3` |

A fixed step damping of 0.5 with recovery is worse: it fails the first call with zero accepted
steps at `6.496e-6`. These are local-solve results, not sequential progress.

A bounded continuation from `stall_218_from_scratch.json` with recovery, refinement and N1 still
reaches only two accepted steps and then hits the wall budget at the same day (`6612.836114`).

## Verdict

**DIAGNOSTIC — the complete carried-K recovery path works at the premature switch, but the current
full-step Newton never evaluates it. Seed recovery alone is necessary, not sufficient.**

The next change should combine the carried `K` with a step that evaluates intermediate inputs —
a phase-boundary-aware line search or a short continuation across the switch — and then test
sequential progress. A plain timestep cutback is not the primary fix; fixed damping is not enough.
The far liquid endpoint remains the negative control, and residual acceptance is unchanged.

## Reproduce

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_seed_recovery_carries_the_switch -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_seed_recovery_repeated_flash_and_reset -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_seed_recovery_resolves_the_switch -- --ignored --nocapture

SPE5_REPLAY_OUT=DIR/recovery.json SPE5_REPLAY_SEED=stall_218_from_scratch.json \
    SPE5_FLASH_REFINE=1 SPE5_N1=1 SPE5_SEED_RECOVERY=1 \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_replay_with_timestep_options -- --ignored --nocapture
```
