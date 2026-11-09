# SPE5 split-based phase decision at the #116 crossings: N1

**Status: experimental, test-only, not promoted. Production flash behavior is unchanged. Acceptance
is still the original equations at `1e-8`; no held-label flux or relaxed acceptance is involved.**

Measured on `98ec066e` (branch `feat/116-n1-split-phase-decision`), release build, using the M0
and M1 inputs regenerated on the same branch (recorded in
[SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md](SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md)). The replay comparison
is two single runs, N1 off and on; only their wall-clock fields differ.

N1 asks the phase decision to follow a converged split instead of the stability test's margin. M0
showed that the crossing label changes where the deciding Michelsen trial sum crosses
`1 + TRIAL_SUM_TOLERANCE` (`1e-12`), although a lower-Gibbs split still exists. N1 attempts that
split when the stable verdict is close to the margin and one trial is non-trivial. It accepts only a
physical, non-trivial split whose Gibbs energy does not exceed the feed's within the existing
`1e-10 RT` guard.

## Design

- **Eligibility.** The standard stability test must still say `Stable`, and:
  - at least one Michelsen trial is non-trivial (a trivial pair carries no phase information);
  - the larger trial sum is at least `1 - 1e-9` (`SPLIT_DECISION_BAND`).
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
`1e-9` value is the smallest band that also leaves room for the cell-16 single-side trial.

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
- **From-scratch replay.** With `SPE5_N1=1`, the driver records **12 N1 acceptances** over the
  attempted 240 months, but every monthly snapshot, step count, Newton count, balance and the
  failure are identical to the N1-off baseline (only wall-clock fields differ). The run still stops
  at month 218, day `6612.703445986848`, scaled residual `6.393474812685191e-8`.
  N1 is therefore a **local correction that did not advance this replay**. The comparison is a
  single pair of release builds, not a timing benchmark.

## Verdict

**DIAGNOSTIC — N1 FIRES AND CORRECTS LOCAL PHASE DECISIONS, BUT THE FROM-SCRATCH REPLAY IS
UNCHANGED. NOT PROMOTED.**

N1 is not refuted as a local correction: it does return lower-Gibbs splits where the margin cut
them. It is insufficient on its own for #116 because (a) the remaining labels still need N2 and a
validated cutoff rule, and (b) the stateless flash loses the seed when the trials collapse to the
trivial solution. A warm-start/continuation design is a separate candidate.

## Reproduce

```bash
# M0 inputs and the M1 continuation (see docs/SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md)
SPE5_M0_POINTS=DIR/m0_gibbs_points.json cargo test --release \
    --manifest-path src/lib/ressim/Cargo.toml --lib comp_spe5_n1_margin_decisions -- --ignored --nocapture
SPE5_M0_POINTS=DIR/m0_gibbs_points.json cargo test --release \
    --manifest-path src/lib/ressim/Cargo.toml --lib comp_spe5_n1_band_sweep -- --ignored --nocapture
SPE5_M1_STATES=DIR/m1_split_mp.json cargo test --release \
    --manifest-path src/lib/ressim/Cargo.toml --lib comp_spe5_n1_m1_states -- --ignored --nocapture

SPE5_REPLAY_OUT=DIR/n1.json SPE5_N1=1 \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_replay_with_timestep_options -- --ignored --nocapture
```

The default path is checked by `bash scripts/validate-compositional.sh thermo` and the committed
cross-flash fixture; the N1 tests are ignored diagnostics.

## Next

1. **N2:** deterministic single-phase identity at the dew point.
2. **A validated equilibrium/thermodynamic cutoff rule** for the memory cells, not a lowered
   `1e-5`.
3. **A warm-start/continuation design** for the stateless-flash seed loss, if N1 is to act beyond
   the first label switch.
4. Re-run the from-scratch month-240 replay and the #98 probe only after a combination is approved.
