# FIM Status

This is the consolidated current-state summary for the Rust FIM solver.
Last full rewrite: 2026-07-05; Bundle N section + gap reprioritization added 2026-07-10; gap #3
updated with the late-window trace diagnostic finding 2026-07-11; gap #3 closed out (Bundle W
evaluated, not promoted) and new gap #4 (reservoir CNV plateau) added 2026-07-11; gap #4 updated
2026-07-12 with the `FIM-DIAG-003` D0-D5 diagnostic verdict (H1 confirmed, H2/H3 refuted,
mechanism located but not yet fixed; `FIM-NEWTON-008` promoted).

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
3. **Nested well-equation solve ("Bundle W") — COMPLETE, mechanism validated, NOT PROMOTED.**
   Phase 8/9's "Hypothesis A" and Bundle N's §5 failure independently converged on ResSim's flat
   well/reservoir Newton coupling as a root blocker; `FIM-DIAG-002` (2026-07-11) diagnosed the
   exact mechanism (a persistent per-iteration disagreement between the raw Newton correction and
   `relax_well_state_toward_local_consistency`'s independently-derived rate, not a classical
   oscillation); `docs/FIM_BUNDLE_W_PLAN.md` (W0-W5, all complete 2026-07-11) built and evaluated
   the fix. **Result**: the diagnosed standoff is genuinely fixed — a windowed trace on the real
   heavy-case trajectory confirms the well residual converges to machine epsilon within one
   iteration (was floored at a non-vanishing ~5e-5). **But the heavy-case `≤35`-substep gate
   still fails** (`18,015` substeps, essentially unchanged from the `17,990` pre-fix baseline):
   fixing the well side exposed a *second, previously-masked* reservoir-side CNV/MB entry-
   criterion plateau (gap #4 below) that now drives the identical `iters=20`/dt-collapse pattern
   for a different reason. `nested_well_solve` stays in the tree, default off, fully validated —
   the disposition mirrors Bundle N's own (real mechanism, insufficient alone). Full result:
   `docs/FIM_BUNDLE_W_PLAN.md` §6 W4/W5; `docs/FIM_CONVERGENCE_WORKLOG.md` "Bundle W checkpoint
   W0-W5"; registry `FIM-BUNDLE-W`.
4. **Reservoir-side CNV/MB entry-criterion plateau under `OpmAligned` (new, `FIM-DIAG-003`,
   first observed 2026-07-11).** Exposed by closing gap #3: once wells stopped being the
   bottleneck, a heavy-case substep's `cnv`/`mb` values (`≈6.1e-5`/`≈1.4e-7`) were found frozen
   —unchanged past the 4th significant digit — across ~19 Newton iterations, only accepted via
   the final-iteration relaxed tier. Consistent with (not proven identical to) the already-
   documented "benign" Legacy `water@1215` plateau below — a genuine near-steady-state region
   colliding with strict acceptance criteria, but manifesting through the `OpmAligned` CNV entry
   criterion rather than Legacy's Appleyard-damping retry ladder. Sharpened by the 2026-07-11
   week retrospective (worklog): **MB alone binds** (CNV passes by 160x; MB fails strict by
   1.41x, frozen = an invariant point of the iteration map), tiers verified identical to OPM's
   pinned source — so OPM's MB genuinely converges below `1e-7` here where ours cannot.
   **Diagnostic complete (`docs/FIM_DIAG_003_PLAN.md` D0-D5, closed 2026-07-12), verdict
   unanimous: H1 (displaced standoff into well-cell MB rows) CONFIRMED, H2 (linear-precision
   floor) and H3 (MB formula fidelity) REFUTED.** D1's binding-cell trace: 100% of the frozen-MB
   iterations bind at the producer's own perforation cell (91%) or its immediate neighbor (9%),
   unaffected by forcing exact linear solves. D2's line-by-line audit of the MB/CNV formula
   against the pinned OPM source found no fidelity bug (independently found and fixed a small,
   orthogonal off-by-one, `FIM-NEWTON-008`: `OPM_NEWTON_MIN_ITERATION_INDEX` `1→2`). D3's OPM
   Flow oracle run (new tracked deck `opm/reference-decks/water-heavy-step1/`): solves the whole
   interval in one 11-iteration Newton solve, its own MB trajectory transits the exact magnitude
   ResSim is frozen at with one clean further iteration (2-3 order-of-magnitude drop) — proving
   the zone is not an inherent numerical floor. D4 averted a false "win" (Legacy+
   `nested_well_solve` on the heavy case looks better on the raw substep ledger but is a genuine
   regression once `real_accepted_substeps` is read correctly) and retracted the stale
   "`22x22x1` regression" claim (does not reproduce at current HEAD). **The pathology is now
   precisely located but not yet fixed** — the fix bundle is planned:
   **`docs/FIM_BUNDLE_X_PLAN.md` (`FIM-BUNDLE-X`, 2026-07-12)**, built on a source-verified
   structural diff vs OPM (OPM: inner well solve *before* linearization + coupled/back-substituted
   well update applied after the solve with no re-solve; ResSim+W: coupled update applied then
   its well component *overridden* post-application — the first-order veto that generates the
   invariant point). Candidate stack (`OpmAligned`+`nested_well_solve`) baseline: `18,015` substeps
   @ `c916c87`; stack promotion stays open until that fix exists and clears the original Bundle N
   §5 gate. `min_strict_mb_iter` remains explicitly out of scope — H1 is a genuine structural
   fixed point (bit-identical residual across 18 iterations), not slow convergence, so relaxing
   *when* the relaxed tier applies would not fix anything.
5. **AMG coarse solver for CPR ("Bundle C", `FIM-LINEAR-006`)** — still deferred, and the Task
   #41 traces confirm the deferral: coarse-stage per-application quality is already ~1e-7 at
   current sizes. AMG is a scale-up item, not part of closing the current measured gap.
6. **Variable substitution** (regime switching inside Newton; `docs/FIM_OPM_GAP_ANALYSIS_SPE1.md`
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
