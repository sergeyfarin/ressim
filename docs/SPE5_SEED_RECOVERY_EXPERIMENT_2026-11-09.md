# SPE5 carried-K seed recovery: complete path and its limit

**Status: experimental, test-only, not promoted. Production flash behavior is unchanged; the
original residual acceptance, equilibrium, physicality, non-triviality and Gibbs checks are
retained.** Measured on `67a6e6d3` (branch `exp/116-seed-recovery`), release build.

This follows the #133 finding that the N1 two-phase→liquid switch at `228.126683` is premature:
the upper input still has a physical split, but both stability trials have become trivial and N1
loses its seed. The experiment carries a cell's previous flash `K` as a direct split hint, re-solves
at the current inputs, and accepts only through the ordinary checks.

## Design

- **Hint.** `flash_for_cell(cell, …)` reads a per-cell `K` that is initialized once per Newton
  attempt from the accepted state and then held fixed for every iteration. Assemblies do not publish
  a new hint, so repeated assembly at identical primaries uses the same seed and cannot change the
  residual. Direct diagnostics seed the hint explicitly. The hint is passed as the flash's
  `k_seed`.
- **Complete recovery path.** When the stability verdict is stable, the carried `K` is used directly
  as the two-phase split seed, not only as a stability-trial seed. The split is re-solved at the
  current `(p, T, z)` by the same two-phase solver and must pass `TwoPhase`, `sum ln^2 K >= 1e-5`,
  finite physical compositions and the existing Gibbs guard. Otherwise the standard single-phase
  answer stands.
- **Consistent properties.** The returned `FlashState` is the one used by the accumulation residual,
  both sides of every face flux, well connections and the derivative pass; no caller re-flashes.
- **Determinism / rejection / restart.** The hint is attempt-owned: it is rebuilt from the
  accepted state at the start of every attempt and discarded afterwards. The step resets it before
  reconstructing the accepted inventory, so a failed attempt cannot influence the next step.
  Checkpoint restore reproduces the same accepted inventory with an empty cache. At convergence, a
  cold re-evaluation must agree with the assembled residual within the unchanged `1e-8`; otherwise
  the attempt is rejected. Nothing is stored in a checkpoint beyond the accepted primaries.
- **Production.** `flash_for_cell` delegates to `flash` in non-test builds; the cache and switch are
  `cfg(test)` only.

## Results

### The complete path recovers the switch

At the adjacent switch inputs (`n1_switch_6612_cell33.json`, cell 33, same `z`):

| Mode | Lower input | Upper input |
|---|---|---|
| recovery off | TwoPhase, `beta 0.460167` | **SingleLiquid** |
| recovery on | TwoPhase | **TwoPhase, `beta 0.436209`**, `sum ln^2 K > 1e-5` |

The committed actual-path carried seed gives `beta = 0.458176`, `sum ln^2 K = 1.64531e-5`,
`g_split - g_feed = -3.77e-15`. Two consecutive upper flashes return the same `beta` and `K`; after
a cache reset the same input is cold and returns `SingleLiquid`. The far liquid endpoint remains a
negative control: a split seeded from the far two-phase iterate is trivial (`sum ln^2 K = 4.3e-13`)
and is rejected.

### Consistency contracts (from the #134 review)

- `comp_spe5_seed_recovery_assembly_is_stable_across_commits`: three whole assemblies at identical
  primaries, BHP, previous inventory and `dt` return the **same** scaled residual
  (`0.0021630455070`), while a cold re-evaluation is the negative control
  (`0.0026570952600`, a `4.94e-4` difference). The fixed hint is not republished by an assembly.
- `comp_spe5_seed_recovery_attempt_and_restart_lifecycle`: two consecutive solves from the same
  accepted state reproduce the same residual history despite the first leaving a hint; a
  checkpoint restart reproduces the accepted inventory with an empty cache.
- At acceptance, the cold re-evaluation must agree with the assembled residual to `1e-8`; otherwise
  the attempt is rejected. This is why a recovered state at the premature switch cannot be accepted
  unless it is also the cold state.

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
    comp_spe5_seed_recovery_assembly_is_stable_across_commits -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_seed_recovery_attempt_and_restart_lifecycle -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_seed_recovery_resolves_the_switch -- --ignored --nocapture

SPE5_REPLAY_OUT=DIR/recovery.json SPE5_REPLAY_SEED=stall_218_from_scratch.json \
    SPE5_FLASH_REFINE=1 SPE5_N1=1 SPE5_SEED_RECOVERY=1 \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_replay_with_timestep_options -- --ignored --nocapture
```
