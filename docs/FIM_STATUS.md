# FIM Status

This is the consolidated current-state summary for the Rust FIM solver.
Last full rewrite: 2026-07-05 (previous version predated the AD migration and Phases 5-11 and
described a state from ~Q1 2026).

Use this file for:

- current implementation state
- current baselines (with exact replay commands)
- current blockers and the recommended next steps
- canonical validation and diagnostic entry points

Do not use this file as a detailed experiment log. Active reproductions and temporary hypotheses
belong in `docs/FIM_CONVERGENCE_WORKLOG.md`. Do not use this file as an experiment index —
promoted/reverted/refuted/open levers belong in `docs/FIM_EXPERIMENT_REGISTRY.md` (search it **by
mechanism name**, not just target case, before proposing any solver change; see the
`FIM-NEWTON-007`→`FIM-DAMP-004` lesson).

## Current State (2026-07-05)

FIM is dev-only (public scenario runs use IMPES, `docs/FIM_DEFERRED_BACKLOG.md`). The solver is
now substantially OPM-aligned, assembled over Phases 0-11:

- **Assembly**: exact AD Jacobian (`fim/assembly_ad.rs`) is the live path; the legacy
  hand-derivative assembler is kept `#[cfg(test)]` as the bit-parity oracle. Parity gates:
  `cargo test --manifest-path src/lib/ressim/Cargo.toml assembly_ad`.
- **Linear stack** (all matching OPM's shipped `cprw` recipe, each gated before promotion):
  loose relative tolerance `5e-3` with iteration budget `20` (`FIM-LINEAR-008`), block-ILU0 fine
  smoother on natural 3x3 cell blocks (Step 10.2), quasi-IMPES CPR pressure restriction
  (`FIM-LINEAR-005`), well-BHP/perforation-rate Schur elimination each Newton iteration
  (`FIM-LINEAR-010`, `fim/linear/well_schur.rs` — OPM's `StandardWellEquations` shape).
- **Newton globalization**: OPM-ported oscillation detector + persistent relaxation scalar,
  covering all five equation families including well/perforation (`FIM-NEWTON-001`/`006`);
  Appleyard damping with fw-inflection trust region at `k=1.25` (`FIM-DAMP-004`); site-keyed
  history stabilization kept (measured tighter than the OPM scalar at its sites,
  `FIM-NEWTON-002`).
- **Offline solver lab** (Phase 9): env-gated capture of real failing linear systems
  (`FIM_CAPTURE_DIR`, `fim/linear/capture.rs`, `fim-capture-v2` format) + full-solve comparison
  tests over captured corpora (`fim/linear/solver_lab.rs`, `#[ignore]`d). New linear-solver
  hypotheses get tested here in seconds before any live change — this is mandatory workflow, not
  optional (`fim-solver-debug` skill).

## Current Baselines (re-derived 2026-07-05, commit `43c6a1d`)

Heavy target case:

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json
→ substeps=32 accepts=31+4+1764 retries=0/13/0 hotspot_newton_caps=7 retry_dom=nonlinear-bad:water@1215
```

Control matrix (must stay bit-identical under any solver change not explicitly about them):

```
water-pressure 20x20x3 dt=0.25 → substeps=8,  retries=0/3/0
water-pressure 22x22x1 dt=0.25 → substeps=4,  retries=0/2/0
water-pressure 23x23x1 dt=0.25 → substeps=4,  retries=0/2/0
gas-rate       20x20x3 dt=0.25 → substeps=2,  retries=0/1/0
gas-rate       10x10x3 dt=0.25 x6 steps → 2 substeps/step steady state
```

Historical heavy-case trajectory for context: `26` (pre-Phase-10, tight-tolerance era) → `59`
(Phase 10 bundle alone) → `160` (+ well elimination alone) → `62` (+ OSC-DETECT widening) → `32`
(+ `k=1.25`). The remaining `32` vs `26` gap is not directly comparable substep-for-substep —
the current bundle does far cheaper linear solves per substep — but no controlled wall-clock
comparison against the pre-Phase-10 configuration has been recorded.

The dominant remaining retry pattern on the heavy case (`water@1215` local-plateau retry ladders,
`DAMPING FAILED — invalid bounded Appleyard candidate`) is **understood and benign**: a genuine
local steady-state region colliding with intentionally-strict entry/zero-move acceptance gates;
the ladder resolves it correctly by dt-halving. Do not "fix" it locally — three attempts all
regressed (`FIM-NEWTON-007`), root cause is the single-global-scalar damping architecture.

## Known Open Gaps (priority order)

1. **`k=1.25` has a measured, real physics-accuracy cost — not resolved, just quantified
   (2026-07-06).** The fine-dt FOPT check the April `FIM-DAMP-003` methodology required (skipped
   by the 2026-07-05 re-sweep) is now done: current-bundle `k=1.25` fine-dt FOPT is `3883.47` vs.
   OPM's converged reference `3826.12` (+1.50%), vs. April's validated `3826.36` (+0.01%). Isolated
   via a same-bundle `k=1.2` rerun (`3845.38`, +0.50%): roughly half the drift (+0.5 pp) is the
   Phase 10/11 bundle itself (tolerance/budget/block-ILU0/well-elim — never fine-dt-checked when
   promoted), and `k=1.25` specifically adds another ~1.0 pp on top. The `62→32` substep win is
   real but not accuracy-neutral. Kept `k=1.25` live (reverting to `1.2` only buys back partial
   accuracy at 2x the substeps — not an unambiguous win). See
   `docs/FIM_CONVERGENCE_WORKLOG.md` "Task #38 (continued)" for full numbers. Open follow-ups,
   none yet attempted: check whether the bundle-level 0.5% drift is itself fixable (likely the
   bigger, more foundational gap); a finer `k` sweep between `1.1`/`1.25` with fine-dt checks at
   each point.
2. **AMG coarse solver for CPR ("Bundle C")** — the last major OPM architecture gap
   (`FIM-LINEAR-006`). Everything else in the linear stack is aligned. Constraint: no mature
   wasm32-compatible pure-Rust AMG crate; hand-roll ~1500-2000 LOC. Design skeleton:
   `docs/FIM_CPR_IMPROVEMENT_PLAN.md` Phase 3. Per
   `docs/FIM_OPM_ALIGNMENT_STRATEGY_2026-04-26.md`, per-cell damping (OPM `dsMax` semantics) and
   dropping the inflection chop should be revisited only **after** AMG lands.
3. **Remaining OPM-gap items** from `docs/FIM_OPM_GAP_ANALYSIS_SPE1.md` (triaged 2026-07-05:
   4 of 6 closed): variable substitution (regime switching inside Newton instead of frozen-regime
   + post-hoc reclassify), and OPM-style post-step dSat/dP-proportional timestep growth limiting.
4. **Wall-clock accounting**: substep counts improved dramatically but `lin_ms` still dominates
   runtime on the heavy case; no recorded apples-to-apples wall-clock baseline across the
   Phase 10/11 promotions.

## Canonical Sources

- Experiment registry / anti-repeat ledger (**check first, by mechanism name**):
  `docs/FIM_EXPERIMENT_REGISTRY.md`
- Active investigation log (Phase 9 onward): `docs/FIM_CONVERGENCE_WORKLOG.md`
- Strategy: `docs/FIM_OPM_ALIGNMENT_STRATEGY_2026-04-26.md` (95%-track-OPM policy, Bundle A/B/C
  sequencing + 2026-07-05 status), `docs/FIM_OPM_GAP_ANALYSIS_SPE1.md` (gap decomposition +
  2026-07-05 triage)
- Archives: `docs/FIM_CONVERGENCE_ARCHIVE_2026-04-08_to_2026-07-03.md` (shelf investigations,
  AD cutover, Phases 5-8), `docs/FIM_CONVERGENCE_ARCHIVE_2026-03_to_2026-04-06.md`,
  `docs/FIM_HISTORY_2026-03.md`
- CPR/AMG design skeleton: `docs/FIM_CPR_IMPROVEMENT_PLAN.md`
- Workflow: `.claude/skills/fim-solver-debug/SKILL.md` (control matrix, promotion discipline,
  known-reverted lever classes)

## Locked Day-to-Day Baseline

Fast smoke set (run before committing any FIM change):

- `cargo test --manifest-path src/lib/ressim/Cargo.toml drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas -- --nocapture`
- `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture`
- `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas -- --nocapture`

Deeper convergence work: rebuild wasm first (`bash scripts/build-wasm.sh`), then use
`scripts/fim-wasm-diagnostic.mjs` (`--diagnostic summary|outer|step`). Full command set and
reading guide: `fim-solver-debug` skill. Offline linear-solver hypotheses: capture a corpus with
`FIM_CAPTURE_DIR=<dir> cargo test --release --lib -- --ignored repro_water_pressure_12x12x3`,
then run the `solver_lab_*` tests against it.

## Current Working Rules

- Search `docs/FIM_EXPERIMENT_REGISTRY.md` **by mechanism name and by file** before proposing any
  convergence change; respect each row's `Retry only if` condition.
- Offline lab before live change for anything in `fim/linear/`; full control matrix + locked
  smoke before promoting anything.
- One registry row per lever, honest verdict either way; negative results are recorded, not
  discarded.
- Keep `TODO.md` short and action-oriented; long narratives go to the worklog.
- Systemic steer (user, standing): track OPM's overall approach consistently rather than fixing
  mechanisms piecemeal — individually-correct local fixes on an OPM-inconsistent base have
  repeatedly regressed (`FIM-NEWTON-005`/`007`, `FIM-LINEAR-001`/`009`).
