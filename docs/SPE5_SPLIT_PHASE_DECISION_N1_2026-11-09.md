# SPE5 split-based phase decision at the #116 crossings: N1

**Status: experimental, test-only, not promoted. Production flash behavior is unchanged. Acceptance
is still the original equations at `1e-8`; no held-label flux or relaxed acceptance is involved.**

Measured on `f6fa1d3c` (branch `feat/116-n1-split-phase-decision`), release build, using the M0
and M1 inputs regenerated on the same branch (recorded in
[SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md](SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md)). The unrefined replay
comparison is two single runs, N1 off and on; only their wall-clock fields differ. The coupled
controls and the bounded continuation are separate release runs.

N1 asks the phase decision to follow a converged split instead of the stability test's margin. M0
showed that the crossing label changes where the deciding Michelsen trial sum crosses
`1 + TRIAL_SUM_TOLERANCE` (`1e-12`), although a lower-Gibbs split still exists. N1 attempts that
split when the stable verdict is close to the margin and one trial is non-trivial. It accepts only a
physical, non-trivial split whose Gibbs energy does not exceed the feed's within the existing
`1e-10 RT` guard.

## Design

- **Eligibility.** The standard stability test must still say `Stable`, and:
  - at least one Michelsen trial is non-trivial (a trivial pair carries no phase information);
  - the largest **non-trivial** trial sum is at least `1 - 1e-9` (`SPLIT_DECISION_BAND`).
  A trivial trial has `sum ~ 1` by construction and is excluded from both the gate and the ranking;
  otherwise its sum could qualify a non-trivial companion that is outside the band.
- **Seed.** The trial pair's `K = y/x` estimate (`pair_k_estimate`), with Wilson values only for
  absent components, exactly as the unstable path seeds the flash today.
- **Solve.** The same two-phase equilibrium solver as the unstable path, with no second stability
  test.
- **Acceptance.** The answer must be `TwoPhase`, have `sum ln^2 K >= 1e-5`, and pass the existing
  Gibbs guard (`g_split <= g_feed + 1e-10 RT`). Otherwise the standard single-phase answer stands.
- **Gating.** `with_split_phase_decision_for_test` is `cfg(test)` only; the stability margin seed
  is computed only under `cfg!(test)`. Non-test builds take the original path.

The band is not a tuned threshold: the M0 phase decisions change at the same 21 of 80 points for
every band from `1e-12` to `1e-2` (the usable seeds are set by the trials, not the band width). The
`1e-9` value is the smallest band that also leaves room for the cell-16 single-side trial. A
regression pins the review's counterexample: at the M0 `cap05` cell-16 single-side composition with
pressure increased by 100 Pa, the trivial vapour sum is `1.0000000000000004` while the only
informative trial is `0.9999999666475153` (3.34e-8 below one). The informative score is used, so
the pair is out of band and no margin seed is returned.

## What N1 changes

- **M0 points.** 21 of 80 phase decisions change when N1 is on. Examples:
  - `cap5` switch `+1e-15`, cell 25: `SingleLiquid` → `TwoPhase`, `beta = 0.730645`;
  - `cap05` cell 16 at the switch: `SingleLiquid` → `TwoPhase`, `beta -> 1`;
  - memory cell 33/39 at the single-side iterate: `SingleLiquid` → `TwoPhase`,
    `beta = 0.4766/0.4765`.
- **M1 states.** 10 of 18 change: the `cap5` start/collapse states and the cell 16 start/collapse
  states become two-phase. The memory trivial-cutoff and collapse states do **not** change: both
  Michelsen trials have collapsed to the trivial solution there, so N1 has no seed. This narrows
  M1's earlier hypothetical: N1 with a stateless flash cannot recover a split once the trial pair
  is gone. That needs N2, a validated cutoff rule, or a warm-start/continuation design.
- **From-scratch replay, refinement off.** With `SPE5_N1=1` and the difference refinement **off**,
  the driver records **12 N1 flash overrides** over the attempted 240 months, but every monthly
  snapshot, step count, Newton count, balance and the failure are identical to the N1-off baseline
  (only wall-clock fields differ). The run still stops at month 218, day `6612.703445986848`,
  scaled residual `6.393474812685191e-8`, the already-diagnosed failure-B precision floor.
  This pair shows only that N1 alone does not advance the unrefined run. It does **not** identify
  the blocker for N1 combined with refinement; the 12 overrides are flash evaluations, not
  accepted physical steps. The comparison is a single pair of release builds, not a timing
  benchmark.

### Coupled controls: N1 + difference refinement

Matched-inventory solves at the captured memory seed (`stall_6612_memory.json`,
`dt = 4.8828125e-6 d`), with one problem/previous inventory/start/wells/sources/dt and only
refinement and N1 changing:

| Refinement | N1 | Iterations | Best scaled residual |
|---|---|---:|---:|
| off | off | 20 | 8.915658e-7 |
| off | on | 4 | 1.183447e-10 |
| on | off | 20 | 1.459579e-6 |
| on | on | 4 | 3.770079e-13 |

The `8.9e-7` figure is the **unrefined** N1-off floor, not “refinement alone”; refined N1 off is
`1.46e-6`. The coupled `3.77e-13` result reproduces. Failure B's last rung (`0.0443 d`) also
converges in four iterations with refinement in either N1 mode (`4.087646e-10`). These are local
solves, preserved by `comp_spe5_n1_refinement_memory_seed_controls` and
`comp_spe5_n1_refinement_failure_b_last_rung_controls`.

### Bounded sequential continuation: N1 + refinement

Starting from `stall_218_from_scratch.json` with refinement and N1 on, the replay advances **two
accepted steps** to day `6612.836114153414`, then fails in month 218 after ten attempts with scaled
residual `5.0410785730772645e-3`. The driver recorded 186 N1 flash overrides across those calls
(again, flash evaluations, not accepted steps).

Tracing the failing call (`dt = 0.0440212614 d`) shows a 20-iteration two-cycle on cells **33/39**:
the residual alternates between `3.328e-3` and `5.041e-3` and the same two cells flip phase every
iteration. The next blocker after N1 + refinement is therefore the same near-critical phase-label
discontinuity, not a precision floor. N1's local correction is not enough to remove it.

## Verdict

**DIAGNOSTIC — LOCAL CORRECTION; N1 + REFINEMENT ADVANCES TWO STEPS, THEN HITS A NEW PHASE-LABEL
TWO-CYCLE. NOT PROMOTED.**

N1 is not refuted as a local correction: it does return lower-Gibbs splits where the margin cut
them, and it resolves the captured memory-seed solve when combined with refinement. It is
insufficient for #116 because (a) the next call still flips cells 33/39 every iteration, and (b) the
stateless flash loses the seed when the trials collapse to the trivial solution. A warm-start or
continuation design, N2 and a validated cutoff rule remain separate candidates. The unrefined
replay says nothing about the combined run's blocker.

## Reproduce

```bash
# M0 inputs and the M1 continuation (see docs/SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md)
SPE5_M0_POINTS=DIR/m0_gibbs_points.json cargo test --release \
    --manifest-path src/lib/ressim/Cargo.toml --lib comp_spe5_n1_margin_decisions -- --ignored --nocapture
SPE5_M0_POINTS=DIR/m0_gibbs_points.json cargo test --release \
    --manifest-path src/lib/ressim/Cargo.toml --lib comp_spe5_n1_band_sweep -- --ignored --nocapture
SPE5_M1_STATES=DIR/m1_split_mp.json cargo test --release \
    --manifest-path src/lib/ressim/Cargo.toml --lib comp_spe5_n1_m1_states -- --ignored --nocapture
SPE5_M0_POINTS=DIR/m0_gibbs_points.json cargo test --release \
    --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_n1_out_of_band_companion_is_not_a_margin_seed -- --ignored --nocapture

# matched-inventory controls and failure B's last rung
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_n1_refinement_memory_seed_controls -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_n1_refinement_failure_b_last_rung_controls -- --ignored --nocapture

# unrefined N1-only replay
SPE5_REPLAY_OUT=DIR/n1.json SPE5_N1=1 \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_replay_with_timestep_options -- --ignored --nocapture

# bounded sequential continuation from failure B with N1 + refinement
SPE5_REPLAY_OUT=DIR/n1_b.json SPE5_REPLAY_SEED=stall_218_from_scratch.json \
    SPE5_FLASH_REFINE=1 SPE5_N1=1 SPE5_LAST_MONTH=240 SPE5_WALL_BUDGET_S=1800 \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_replay_with_timestep_options -- --ignored --nocapture
```

The default path is checked by `bash scripts/validate-compositional.sh thermo` and the committed
cross-flash fixture; the N1 tests are ignored diagnostics.

## Next

The captured next failing call is diagnosed separately in
[SPE5_N1_NEXT_BLOCKER_DIAGNOSIS_2026-11-09.md](SPE5_N1_NEXT_BLOCKER_DIAGNOSIS_2026-11-09.md):
the far endpoints are individually reasonable, but the N1 two-phase→liquid switch at
`228.126683` is premature. The upper input still has a physical split (`beta 0.458178`,
`sum ln^2 K 1.645e-5`) while both stability trials have become trivial, so N1 loses its seed. A
carried seed from the lower side converges to a physical split above the `1e-5` acceptance cutoff.

1. **Seed continuation/eligibility through the switch:** carry the adjacent iterate's/previous
   flash's `K` or a validated fallback, with the committed carried-seed regression.
2. **Evaluate an early two-cycle detector/cutback separately** for cost and sustained advancement,
   not as the primary fix.
3. **N2:** deterministic single-phase identity at the dew point, for the other crossing cells.
4. Re-run the from-scratch month-240 replay and the #98 probe only after a combination is approved.
