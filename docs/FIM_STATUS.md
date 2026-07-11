# FIM Status

This is the consolidated current-state summary for the Rust FIM solver.
Last full rewrite: 2026-07-05; Bundle N section + gap reprioritization added 2026-07-10.

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

## Bundle N: OPM-shaped nonlinear layer (2026-07-07..10) — built, evaluated, NOT promoted

The full OPM nonlinear layer (CNV/MB acceptance with relaxed tiers, per-cell `dsMax`/`dpMaxRel`
chopping incl. a `dbhp-max-rel` well-BHP chop, `pid+newtoniteration` controller, OPM
linear-failure handling, deletion of the Legacy compensating-mechanism stack) is fully
implemented behind `FimNonlinearFlavor::OpmAligned` — **default `Legacy`, verified bit-identical
no-op on every checkpoint** (`setFimOpmAlignedNonlinear` wasm setter / `--opm-aligned`
diagnostic flag). Design + per-checkpoint evidence: `docs/FIM_BUNDLE_N_DESIGN.md`; registry row
`FIM-BUNDLE-N` (**REWORK REQUIRED**).

Outcome in one paragraph: the ported mechanisms each did their job in isolation — per-cell
chopping fixed the measured MB-stall (95% of Newton solve attempts now reach a state OPM's full
rules would accept, vs 48% under Legacy damping), and the bounded cases improved checkpoint over
checkpoint once three Legacy leftovers and one real N5 bug were found and fixed via log
forensics. But the §5 end-metric evaluation on the heavy case failed decisively: **18,002
substeps vs the ≤35 gate** (native `--release`, verified twice, not a tracing artifact — trace
overhead isolated separately). The pathology: once the producer pins at its BHP limit near
steady state, a well/perforation residual that does NOT shrink with dt forces `iters=20` every
substep, and the OPM-ported controller compounds `growth=0.4` into a dt collapse
(`min_dt≈1e-7` days). Verified against OPM source: OPM structurally cannot hit this because
well-control switching resolves inside a *nested well iteration*
(`WellInterface::iterateWellEquations`) invisible to the outer count feeding its controller;
ResSim's flat single-level Newton loop (wells Schur-eliminated at the linear level only, Phase
11) has no equivalent. Two follow-up fixes were ruled out honestly: iteration-count decoupling
(no-op by code inspection — N1 already counts reservoir-only convergence) and the verbatim
`dbhp-max-rel` BHP chop (zero effect, bit-identical rerun — BHP is not the oscillating
variable). The actual oscillating quantity is still unidentified; candidates are the
perforation-rate variable and ResSim's own `relax_well_state_toward_local_consistency`
post-processing (no OPM counterpart, never examined).

Two independent investigations now converge on the same conclusion — the old Phase 8/9
"Hypothesis A" (linear-solver angle) and this §5 failure (controller angle): **the deepest
remaining architecture gap to OPM is the missing nested well-equation solve**, not any
mechanism's tuning. Known OpmAligned fidelity gap recorded for that future work: N1 acceptance
currently has no counterpart of OPM's `getWellConvergence` (`tolerance-wells=1e-4`) — OPM gets
away with a light well check because its inner well solve converges wells by construction each
outer iteration.

Current `OpmAligned` numbers for reference (all with Legacy defaults unaffected): `22x22x1` =
12 substeps/1 retry, `23x23x1` = 12/1 (Legacy: 4/2 each — close on attempts, not yet better),
heavy `12x12x3` = 18,002 (Legacy: 32). Wall-clock under `OpmAligned` is additionally dominated
by the per-iteration preconditioner rebuild — the independent 24x factor Bundle P addresses.

## Current Baselines (re-derived 2026-07-05, commit `43c6a1d`; heavy case superseded 2026-07-10, see below)

Heavy target case:

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json
→ substeps=32 accepts=31+4+1764 retries=0/13/0 hotspot_newton_caps=7 retry_dom=nonlinear-bad:water@1215
```

**Superseded 2026-07-10 by `FIM-LINEAR-011`** (coarse-factorization cost lever, `PRESSURE_DIRECT_SOLVE_ROW_THRESHOLD` `512→300`): the heavy case (432 coarse rows) moved from the explicit dense inverse onto the already-production BiCGStab+ILU0 coarse path. This is a linear-solver cost/precision change, not a nonlinear-controller change, so the substep trajectory shifted (this case is known chaos-sensitive to solver precision, cf. the `k`-sweep in Task #37):

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json
→ substeps=52 (new "mixed" retry class), wall-clock 36.9s→6.8s (5.4x faster)
```

Fine-dt FOPT physics gate re-checked under the new config: `3847.59` vs OPM's `3826.12` (+0.56%) — **better** than the currently-accepted bundle's own +1.50% (`3883.47`, `k=1.25`/`FIM-DAMP-004`). Full comparison and gate results: `docs/FIM_CONVERGENCE_WORKLOG.md` "Coarse-factorization cost lever (2026-07-10)"; registry row `FIM-LINEAR-011` (PROMOTED).

Control matrix (must stay bit-identical under any solver change not explicitly about them; reconfirmed bit-identical under `FIM-LINEAR-011` on all 5 non-heavy cases below, including `22x22x1`, whose 484 coarse rows also crossed the new 300-row threshold):

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

1. **`k=1.25`'s ~1.5% fine-dt FOPT drift vs. OPM is accepted, not pursued further (user decision,
   2026-07-06).** Measured 2026-07-06: current-bundle `k=1.25` fine-dt FOPT is `3883.47` vs. OPM's
   converged reference `3826.12` (+1.50%), vs. April's validated `3826.36` (+0.01%); isolated via a
   same-bundle `k=1.2` rerun (`3845.38`, +0.50%) into a ~0.5pp bundle-level cost (Phase 10/11
   tolerance/budget/block-ILU0/well-elim, never fine-dt-checked when promoted) plus a further
   ~1.0pp from `k=1.25` itself. User call: 1% is not material given the current overall gap to OPM
   Flow (~2.5 Newton iters/step, zero cuts — ResSim is far from that regardless of this ~1%); do
   not spend further sessions fine-tuning `k` or chasing this specific drift. `k=1.25` stays live.
   See `docs/FIM_CONVERGENCE_WORKLOG.md` "Task #38 (continued)" for full numbers. **Do not reopen
   this gap without new evidence that the drift has grown or that it's blocking something else** —
   prioritize the larger architectural gaps below instead.
2. **The 738x wall-clock gap to OPM Flow: measured (Task #41), partially addressed, partially blocked.**
   The ~30x nonlinear factor was attacked by Bundle N (section above): mechanisms validated,
   end-to-end promotion blocked by the flat well/reservoir coupling. The ~24x per-iteration
   factor (preconditioner rebuilt every Newton iteration = 89% of wall-clock) split into two
   independent sub-levers: **reuse** (`FIM-BUNDLE-P`, REFUTED at P0 2026-07-10 — no fixed
   reuse interval is failure-free, not even `k=1`) and **cheaper fresh build**
   (`FIM-LINEAR-011`, PROMOTED 2026-07-10 — the coarse block's explicit dense inverse, not its
   ILU0 setup, was the dominant cost; BiCGStab+ILU0, already production code above the old
   512-row threshold, is 100-170x cheaper with zero convergence failures on 599 captured
   systems; threshold lowered `512→300`). Net effect: heavy-case wall-clock `36.9s→6.8s`
   (5.4x), independent of and stacking with whatever Bundle N eventually resolves on the
   nonlinear side. Remaining unaddressed cost within the ~24x factor: nothing further scoped;
   the reuse half of the lever is closed unless a materially different invalidation scheme is
   proposed (see `FIM-BUNDLE-P`'s retry condition).
3. **Nested well-equation solve ("Bundle W" candidate — the twice-confirmed architecture gap).**
   Phase 8/9's "Hypothesis A" and Bundle N's §5 failure independently converge on ResSim's flat
   well/reservoir Newton coupling as the root blocker. The natural shape (from OPM):
   per outer Newton iteration, solve each well's tiny local system (BHP + perforation rates —
   1-2 wells, 1 perf each on current benchmark cases) to convergence against the frozen
   reservoir state, replacing `relax_well_state_toward_local_consistency`; check wells
   separately at `tolerance-wells` instead of folding them into the outer criteria; keep
   well-switching cost invisible to the timestep controller. Before building it, spend the now
   cheap (post-Bundle-P) diagnostic pass identifying the actual oscillating variable in the
   18k-substep pathology — two blind fixes already refuted; a third guess without visibility is
   not acceptable.
4. **AMG coarse solver for CPR ("Bundle C", `FIM-LINEAR-006`)** — still deferred, and the Task
   #41 traces confirm the deferral: coarse-stage per-application quality is already ~1e-7 at
   current sizes. AMG is a scale-up item, not part of closing the current measured gap.
5. **Variable substitution** (regime switching inside Newton; `docs/FIM_OPM_GAP_ANALYSIS_SPE1.md`
   gap #5) — deliberately excluded from Bundle N; candidate follow-on after the well-coupling
   question settles.

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
