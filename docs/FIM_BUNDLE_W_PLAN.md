# Bundle W: Nested Well-Equation Solve (replace `relax_well_state_toward_local_consistency`)

Status: PLANNED (2026-07-11). Registry: `FIM-BUNDLE-W` (OPEN).
Prerequisite evidence: `FIM-DIAG-002` (`docs/FIM_CONVERGENCE_WORKLOG.md` "Late-window trace
diagnostic on the 18k pathology (2026-07-11)").

## 1. Why this bundle exists (the evidence, stated precisely)

Three independent lines converge on ResSim's flat well/reservoir Newton coupling as the deepest
remaining architecture gap to OPM — but they are not interchangeable, and the history matters:

1. **Phase 8 (2026-04/05, archive)**: the original "Hypothesis A" (Fischer-Burmeister
   slack/crossover in the perforation constraint) found **no supporting evidence** in 55
   `FAIL-SITE-DETAIL` observations. What DID emerge (families #2/#3, later "Hypothesis C
   territory") was *well-source-dominated cell rows* — well terms 2-1000x the reservoir-flux
   coupling in the same row. Well coupling was implicated; the specific NCP-crossover mechanism
   was not.
2. **Bundle N §5 (2026-07-09, `docs/FIM_BUNDLE_N_DESIGN.md` §5.1)**: under `OpmAligned`, the
   heavy case collapses to ~18k substeps because a well/perforation residual that does not
   shrink with dt costs `iters=20` every substep and the OPM-ported controller compounds
   `0.4^N`. Verified against OPM source: OPM structurally cannot exhibit this because well
   convergence happens inside `WellInterface::iterateWellEquations`, invisible to the outer
   iteration count feeding its timestep controller.
3. **`FIM-DIAG-002` (2026-07-11) — the mechanism, per-iteration**: the stuck variable is the
   BHP-limited producer's perforation rate. The raw Newton correction to `q` settles at a
   non-vanishing per-substep plateau (0.28-0.64 m³/day) and
   `relax_well_state_toward_local_consistency` cancels it near-exactly every iteration; the
   `perforation_flow` residual plateaus above tolerance (floor ∝ the same plateau, ratio
   ≈8.63e-5 across 5 substeps) instead of vanishing. BHP's raw correction is exactly `0.0`
   every iteration. This is a **standoff between two independently-derived formulas for the
   same quantity** — the AD-assembled `rate_consistency` residual's implied `q` vs.
   `connection_rate_for_bhp`'s blended/trust-radius-clamped `q` — not a classical oscillation
   (invisible to `FIM-NEWTON-006`'s OSC-DETECT by construction) and not a tuning gap.

## 2. The one design constraint the diagnosis imposes

**The inner well solve must drive the SAME discrete residual equations the global assembly
uses** (the `well_constraint` + `rate_consistency` rows as assembled by `fim/assembly_ad.rs` /
`fim/wells_ad.rs`), evaluated at the frozen candidate reservoir state — so that "wells
converged" in the inner loop means, by construction, "well residual rows are zero in the next
outer assembly". Any inner solve built on a separate rate formula (as
`relax_well_state_toward_local_consistency` is, via `wells::connection_rate_for_bhp` + blend +
trust radius) re-creates exactly the standoff `FIM-DIAG-002` measured. This is the root-cause
fix; do NOT substitute a retuned blend/trust-radius (`WELL_BHP_MANIFOLD_BLEND`,
`WELL_RATE_MANIFOLD_BLEND`, `WELL_RATE_TRUST_RADIUS_*`) — that axis is diagnosed as the wrong
shape of fix.

## 3. Code facts (verified 2026-07-11, commit `a362e29`)

- **Unknown layout** (`fim/state.rs`): per-well BHP (`well_bhp_unknown_offset`) + per-perforation
  rate (`perforation_rate_unknown_offset`), appended after the 3-per-cell reservoir unknowns.
- **Well equations** (as assembled): per-perforation `rate_consistency = q −
  connection_rate_generic(bhp, cell_state)` (`wells_ad.rs::perforation_residual_generic` row 0);
  per-well `well_constraint` = `bhp − bhp_target` for BHP-controlled/disabled wells, or the
  Fischer-Burmeister complementarity of (bhp_slack, rate_slack) for rate-controlled wells
  (`well_constraint_residual_fb_generic`, mirroring `wells.rs::constraint_residual`).
- **`relax_well_state_toward_local_consistency`** ([state.rs:307](../src/lib/ressim/src/fim/state.rs:307))
  has exactly ONE call site: `apply_raw_update(..., relax_well_state=true)`
  ([state.rs:424](../src/lib/ressim/src/fim/state.rs:424)), reached from
  `apply_newton_update_frozen` — i.e. it runs after every Newton update application, under BOTH
  `Legacy` and `OpmAligned` flavors. Replacing at that single site covers everything.
- **Existing building blocks, reusable as-is or nearly**:
  - `wells_ad.rs::perforation_jacobian` — 5x5 AD Jacobian of
    `[rate_consistency, well_constraint, water/oil/gas source]` w.r.t.
    `[p, sw, hydrocarbon_var, bhp, q]` for a single-perforation well. The inner solve's 2x2
    local system (single-perf well, frozen cell) is rows {0,1} × cols {3,4} of this.
  - Multi-perforation wells (`physical_well_id` grouping, `FimPhysicalWell.perforation_indices`)
    form a bordered "arrow" system: each perf's `rate_consistency` row couples only
    (bhp, q_perf); the well's constraint row couples bhp and all q_perfs. Assemble from
    per-perforation AD blocks (same scatter pattern the global assembler already uses); solve
    the (1+n)×(1+n) dense system directly. No new AD width needed.
  - The `dbhp-max-rel` BHP chop from the refuted Bundle N §5 follow-up
    (`opm_per_cell_chopped_update`, `fim/newton.rs`) — refuted as an OUTER-loop fix, but in OPM
    this chop lives INSIDE the inner well solve (`StandardWellPrimaryVariables::updateNewton`,
    called from `iterateWellEquations`). Bundle W is its correct home; reuse the ported,
    unit-tested formula there.
  - `fim/linear/well_schur.rs` (`FIM-LINEAR-010`, live): exact Schur elimination of well rows in
    the linear solve, with exact back-substitution. Composes with W: the back-substituted well
    update is the raw outer Newton Δ(bhp,q), which becomes the inner solve's warm start.
- **N1's known fidelity gap** (`docs/FIM_STATUS.md`): `OpmAligned` acceptance
  (`cnv_mb_diagnostics::would_accept`) checks reservoir families only, with no counterpart of
  OPM's `getWellConvergence` (`tolerance-wells=1e-4`, recorded in Bundle N §9). OPM affords the
  light well check because its inner solve converges wells by construction each outer
  iteration — W supplies exactly that precondition.
- **Baseline to beat** (commit `a362e29`, `FIM-DIAG-002` re-baseline):
  ```
  cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    fim::timestep::phase5_repro::repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture --exact
  → accepted_substeps=17990 linear_bad=7 nonlinear_bad=1 mixed=1
    min_dt≈1.03e-6 max_dt=0.185 wall≈21.5min
  ```
  Bounded `OpmAligned` cases: `22x22x1` = 12 substeps/1 retry, `23x23x1` = 12/1 (Legacy 4/2).
  Legacy heavy: 52 substeps (`FIM-LINEAR-011` baseline). Fine-dt FOPT: `3847.59` (+0.56% vs OPM
  `3826.12`).

## 4. W0 — OPM source verification pass (mandatory before building, Bundle N step-0 pattern)

Verify at the pinned opm-simulators 2025.10 source, and record findings in a §-style appendix
here (do not build on recollection; Bundle N §9 already verified some of this — extend, don't
re-derive):

1. `WellInterface::iterateWellEquations` / `StandardWell::iterateWellEqWithControl` /
   `iterateWellEqWithSwitching`: exact loop structure, where control switching happens, and
   where it is invoked relative to reservoir assembly within one outer iteration.
2. Inner-loop convergence test: which residuals, which scaling, confirm `tolerance-wells=1e-4`
   role, and the max inner-iteration count (do not assert a number until read).
3. `StandardWellPrimaryVariables::updateNewton`: confirm the `dbhp-max-rel` chop application
   point and any other per-variable chops inside the inner loop (e.g. rate-variable handling —
   Bundle N follow-up already confirmed `WQTotal` has NO magnitude clamp, only sign-consistency).
4. Failure policy: what OPM does when the inner well solve does not converge (mark well failed?
   propagate to outer? cut dt?).
5. Confirm wells' exclusion from the outer convergence criteria and from the iteration count
   fed to the timestep controller (already established in §5.1; re-cite file:line here).

## 5. Design

One new mechanism, flag-gated and inert by default (`nested_well_solve: bool` on
`FimNewtonOptions` + a wasm/diagnostic setter, following the `OpmAligned`/`eliminate_wells`
precedent — an independent flag, NOT folded into `FimNonlinearFlavor`, so it can be evaluated
under both flavors):

1. **Where**: in `apply_raw_update`, when the flag is on, the `relax_well_state` branch calls
   the inner solve instead of `relax_well_state_toward_local_consistency`. Warm start: the
   outer update's Δ(bhp, q) has already been applied (matching OPM's recover-then-iterate
   order). `enforce_control_bounds` still runs after, unchanged.
2. **Per physical well**: assemble the (1+n_perf)×(1+n_perf) local system from the same AD
   residual functions the global assembler uses (`well_constraint` row + one
   `rate_consistency` row per perf), reservoir cell states frozen at the candidate. Newton with:
   - `dbhp-max-rel` chop on the BHP update (reused ported formula), no magnitude clamp on q
     (matching OPM), sign handling left to the equations;
   - convergence when the well's scaled residual rows are ≤ the well tolerance (use the same
     `EquationScaling` family scales the global convergence test uses, so "inner converged" and
     "outer sees zero" are the same statement; tolerance value per W0, expected `1e-4`);
   - bounded iterations (count per W0); on non-convergence, keep the last iterate and report
     not-converged — the outer well-convergence check then fails and the existing retry ladder
     handles it. Do NOT widen acceptance to paper over inner failures (`FIM-NEWTON-005` lesson).
3. **Outer criteria** (`OpmAligned` path): add the `getWellConvergence` analog — wells checked
   separately at the well tolerance, closing N1's recorded fidelity gap. Reservoir-only CNV/MB
   acceptance unchanged. Iteration count fed to N3 unchanged (already reservoir-only by
   construction, per the §5 follow-up finding). Legacy criteria untouched in this bundle.
4. **What gets deleted (when promoted)**: `relax_well_state_toward_local_consistency` and its
   constants. Until promotion, it remains the flag-off path.

### Explicitly NOT in Bundle W

- No change to assembly, the linear stack, or `eliminate_wells` (they compose as-is).
- No outer-loop acceptance widening of any kind (`FIM-NEWTON-004`/`005` are REVERTED/REFUTED).
- No retuning of the Appleyard/inflection chop (`FIM-DAMP-002/003/004`, `FIM-NEWTON-007`).
- No discrete control-switching loop replacing the FB complementarity row — ResSim's FB form is
  the existing assembled equation; keep it (constraint #2 above). If W0 reveals OPM's
  switching materially disagrees with FB behavior at the switch point, record it as a follow-on
  question, don't scope-creep it in.
- No Legacy-flavor promotion decision inside this bundle (see §7).

## 6. Build order (checkpoints, each no-op gated with the flag off)

- **W1 — DONE (2026-07-11).** `fim/wells_inner.rs`: `assemble_well_local_system(sim, state,
  topology, well_idx) -> FimWellLocalSystem` (`residual: DVector`, `jacobian: DMatrix`, local
  row 0 = `well_constraint`/`bhp`, rows `1..=n` = each perforation's `rate_consistency`/`q`).
  Built by calling the exact same shared primitives `assembly_ad.rs`'s
  `add_well_residual_terms`/`add_well_jacobian_terms` call for these rows
  (`well_constraint_residual_fb_generic`, `well_constraint_bhp_column_and_fb_gradient`,
  `well_constraint_own_perforation_rate_jacobian`, `connection_rate_generic`,
  `rate_consistency_cell_bhp_jacobian`, `producer_fractions_generic`) — not a reimplementation.
  Two small `assembly_ad.rs` helpers (`well_cell_input`, `well_control_generic`) promoted from
  private to `pub(crate)` for reuse; zero behavior change (verified: `assembly_ad` parity 10/10
  unaffected). Agreement tests (4, all passing) directly encode plan §2's constraint: BHP-
  controlled and rate-controlled (Fischer-Burmeister) two-well fixtures, including a state
  deliberately away from convergence (not just near-zero-residual, where a formula bug could
  hide) — local residual bit-identical to the corresponding global row, local Jacobian entries
  match the corresponding global sub-block within `1e-12` (the global sparse assembler's
  `add_if_nonzero` drops `|value|≤1e-14` entries as implicit zeros; the dense local Jacobian
  doesn't — a storage-convention difference, not a formula divergence, confirmed by one test
  failure at `~9.4e-16` before the tolerance fix). A no-cross-coupling test confirms one perf's
  `rate_consistency` row never touches another perf's `q`, matching the global assembler's
  per-perforation `tri.add_triplet(perf_row, q_col, 1.0)`. Full no-op gate: control-flow
  unaffected (code is unreachable until W3 wires it in — wasm build green, `assembly_ad`
  parity 10/10, locked smoke 3/3, `fim::wells` 18/18).

  **Closed-form observation surfaced while building W1** (not yet exploited — flag for W2's
  design): `connection_rate_generic` takes `(bhp, cell)` and does *not* depend on `q` — the
  `rate_consistency` row's dependence on `q` is the trivial identity (coefficient `1.0`,
  confirmed in the Jacobian: `tri.add_triplet(perf_row, q_col, 1.0)`). For a BHP-controlled
  well (`well_constraint` = pure `bhp − bhp_target`, no `q` dependence at all — exactly why
  `FIM-DIAG-002` measured `raw_dbhp` at exactly `0.0` every iteration) with a frozen reservoir
  cell, `q = connection_rate_generic(bhp_target, cell)` is a **one-shot closed-form
  evaluation**, not an iterative fixed point — there is nothing for a Newton loop to actually
  iterate on once bhp and cell state are fixed. This means the `FIM-DIAG-002` standoff (`q`'s
  raw correction stuck at a non-vanishing plateau every iteration) was very likely an artifact
  of the *coupled global iterative linear solve*'s imprecision on that specific row/unknown
  pair, not genuine nonlinear difficulty in the well subsystem itself — isolating the well
  system into its own small dense direct solve should converge it in very few iterations for
  the BHP-controlled case (this is exactly what `FimStepStats`' `iters=20` compares against —
  OPM's own well solves typically settle in 1-3 inner iterations, `docs/FIM_BUNDLE_N_DESIGN.md`
  §1's "~2.5 Newton iterations/step" OPM reference target). Rate-controlled wells remain
  genuinely nonlinear (the FB complementarity row *does* depend on `q` through the rate slack),
  so W2 still needs a real bounded Newton loop — this observation is about why the BHP-limited
  heavy case specifically should resolve cleanly, not a reason to special-case the code.
- **W2 — DONE (2026-07-11).** `fim/wells_inner.rs`: `solve_well_locally`/`solve_wells_locally`,
  a bounded chopped Newton loop over `assemble_well_local_system` (W1). `dbhp-max-rel` chop
  (`chop_bhp_update`, OPM's exact formula incl. the `1 bar − 1 Pa` floor); no magnitude clamp on
  `q` (matches OPM's `WQTotal`); convergence uses `fim/scaling.rs`'s `well_constraint_scale`/
  `perforation_flow_scale` — extracted from `build_equation_scaling` into standalone `pub(crate)`
  functions (zero-behavior-change refactor, verified via `fim::scaling` + `assembly_ad` parity)
  so the inner-solve convergence test and the global assembly's convergence test are provably
  the same formula, not two hand-matched copies. Added OPM's `WrongFlowDirection` check (W0
  appendix C) scoped to pressure-controlled wells, per-perforation (ResSim has no aggregate
  `WQTotal`) — residual-converged-but-wrong-sign correctly reports not-converged, not silently
  accepted. On a singular local Jacobian or exhausted budget: last iterate kept, `converged:
  false` reported, no panic, no acceptance-widening (`FIM-NEWTON-005` lesson).

  **10 tests, all passing.** Confirms the W1 closed-form observation empirically, not just in
  theory: `bhp_controlled_well_converges_from_perturbed_state` converges in exactly **1
  iteration** (residual ~1e-16, machine epsilon) starting from a state perturbed 800 m³/day away
  from consistency. `rate_controlled_well_converges_to_slack_feasible_state` (genuinely
  nonlinear FB case) converges to a feasible (bhp, q). `exhausted_budget_reports_not_converged_
  without_panicking` (0-iteration budget) exercises the give-up path deterministically, standing
  in for a contrived physically-infeasible scenario per the plan's original wording. Plus chop
  and sign-check unit tests.

  No-op gate: full `fim::` suite 277 passed / 3 failed — the failures are byte-identical to the
  pre-existing 2026-07-07 known failures (`fim::timestep::tests::changing_hotspot_resets_extra_
  growth_cooldown_budget`, `repeated_same_hotspot_extends_growth_cooldown_budget`,
  `fim_enabled_step_advances_time_and_records_history_for_closed_system`, documented in
  `TODO.md`), confirmed by exact name match, not a new regression. `assembly_ad` parity 10/10,
  wasm build green (new code still unreachable until W3 wires it in).
- **W3 — wiring behind the flag.** Replace the relax call under the flag; add the well
  convergence check to the `OpmAligned` acceptance; wasm setter + `--nested-well-solve`
  diagnostic flag (runner passthrough like `--opm-aligned`). Gates: flag off ⇒ full control
  matrix + heavy case bit-identical, locked smoke 3/3, `assembly_ad` parity, wasm build green.
- **W4 — evaluation (end metrics only, Bundle N lesson: no per-mechanism baselining against the
  old architecture).** Order matters — cheapest, most-diagnostic first:
  1. **Mechanism check** (minutes): windowed `FIM-DIAG-002` rerun (`FIM_TRACE_FILE` +
     `FIM_TRACE_SUBSTEP_START` + `FIM_MAX_SUBSTEPS`) with `OpmAligned`+W on the heavy case.
     PASS = the `WELLTRACE` standoff signature is gone: post−(pre+raw) for the producer's q no
     longer cancels the raw update at a persistent plateau, and `res_pf` drops below tolerance
     within the inner-converged iterations instead of flooring. If the standoff persists, STOP
     and re-diagnose before burning the full run.
  2. **§5 re-run** (~22 min): full uncapped heavy case `OpmAligned`+W, exact command in §3.
     Gates per the original Bundle N §5: substep/cut behavior in the ≤35-substep class, not
     18k-class. Partial-credit outcomes (e.g. 100-500 substeps) are a real finding — record
     honestly, do not promote.
  3. **Bounded cases**: `22x22x1`/`23x23x1` `OpmAligned`+W (target: ≤ the current 12/1; watch
     whether the unexplained ~3x-vs-Legacy gap from Bundle N §10 obs. 6 narrows).
  4. **Physics**: fine-dt FOPT on the heavy case under the evaluated config vs OPM `3826.12`
     (current accepted +0.56%; do not regress materially).
  5. **Full control matrix + locked smoke + parity** on the final tree, flag off (bit-identity)
     AND flag on under Legacy (informational — expect trajectory changes; this is the §7 input,
     not a gate).
- **W5 — Bundle N §5 verdict.** With W in: re-apply the original Bundle N promotion rule
  (promote N1-N5+W as one bundle, or record precisely why not). Update `FIM-BUNDLE-N` and
  `FIM-BUNDLE-W` registry rows either way; on promotion, annotate the superseded rows listed in
  the `FIM-BUNDLE-N` row and delete the relax mechanism + Legacy compensators per N4's original
  scope.

## 7. Open question deliberately deferred: Legacy adoption

W changes `apply_raw_update` behavior under BOTH flavors when the flag is on, and the relax
standoff exists under Legacy too (Legacy heavy case's historical `perf@1299` mixed-retry class
is plausibly the same mechanism — `FIM-LINEAR-011`'s new "mixed" retries sit at perf rows).
But Legacy's damping/acceptance stack was tuned WITH relax in the loop; flipping W on under
Legacy is a separate experiment with its own full gate pass (control matrix will move — that's
expected, not a bug). Do it as its own registry row after W4, only if the `OpmAligned`
evaluation validates the mechanism.

## 8. Risks

- **FB row conditioning in the tiny solve**: near the control-switch point the FB derivative can
  be poorly conditioned; the same equation already lives in the global Jacobian (no new
  physics), but a 2x2 Newton has no Krylov regularization around it. Mitigation: chopped,
  bounded inner loop + honest failure reporting (W2 test three).
- **Frozen-reservoir direction quality**: converging wells against frozen cells each iteration
  changes the outer Newton's effective direction; OPM's architecture demonstrates this is
  workable, but ResSim trajectories will shift everywhere the flag is on. Hence end-metric-only
  gates and the §7 deferral.
- **Hidden relax dependencies**: anything implicitly relying on relax's trust-radius smoothing
  (e.g. hotspot-streak bookkeeping keyed on well sites) may behave differently. The W3 flag-off
  bit-identity gate protects the default path; flag-on surprises are evaluation findings.
- **Multi-perf coverage**: all current control-matrix wells are single-perforation; the arrow
  solve's ≥2-perf path is exercised only by the W1 unit test until a multi-perf scenario runs.
  Say so in the registry row rather than overclaiming.

## 9. Documentation consequences

- Registry: add `FIM-BUNDLE-W` (OPEN) now; verdict + numbers at W4/W5.
- Worklog: one entry per checkpoint, numbers verbatim, per the standing discipline.
- On promotion: update `docs/FIM_STATUS.md` (gap #3 closes; baselines superseded — name which),
  `docs/FIM_BUNDLE_N_DESIGN.md` §10 sequencing, `TODO.md` item 3.
- `FIM-DIAG-002`'s tooling is the verification instrument for W4 step 1 — keep it intact.

## Appendix: W0 OPM source verification (2026-07-11)

Verified against the pinned local checkout `OPM/opm-simulators`
(`062cb19986aa8f11cffc30351fd2fee355d0ccb4`, tag `interim_release/2024.12-4152-g062cb1998`,
authored 2026-07-01). **Correction to prior citations**: this checkout has the reservoir Newton
model class renamed from `BlackoilModel`/`BlackoilModel_impl.hpp` (cited in
`docs/FIM_BUNDLE_N_DESIGN.md` §9.1, `docs/FIM_STATUS.md`'s Bundle N section) to
`NonlinearSystemBlackOilReservoir`/`NonlinearSystemBlackOilReservoir_impl.hpp` — an upstream
rename that postdates whichever checkout those docs originally verified against. Re-verify any
future OPM citation against the checkout actually in the tree at the time, not by file-name
pattern-matching to older docs.

### A. Loop structure — `iterateWellEquations` and where it fits in one outer iteration

- Per-well entry point: `WellInterface::iterateWellEquations`
  (`opm/simulators/wells/WellInterface_impl.hpp:532`). Dispatches to
  `iterateWellEqWithControl` or (default config) `iterateWellEqWithSwitching`
  (`StandardWell_impl.hpp:2458`), based on `local_well_solver_control_switching_`
  (`LocalWellSolveControlSwitching`, default `true` — the switching path is the one that
  matters for a default-config comparison).
- Called from `WellInterface::prepareWellBeforeAssembling`
  (`WellInterface_impl.hpp:1018`, call site `:1066`), itself called once per outer iteration
  from `updateWellControlsAndNetworkIteration` → `updateWellControlsAndNetwork`, invoked from
  `BlackoilWellModel::assemble()` **before** `assembleWellEqWithoutIteration(dt)`
  (`BlackoilWellModel_impl.hpp:1186` — the call sequence documents itself: wells are iterated
  to (bounded) convergence first, then linearized "without iteration" into the global system).
  This confirms the plan's assumed shape: well convergence happens *inside* one outer
  reservoir-Newton iteration, ahead of the global assembly that iteration produces.
- **Gated, not unconditional**: `prepareWellBeforeAssembling` only calls `iterateWellEquations`
  when `iterCtx.shouldRunInnerWellIterations(max_niter_inner_well_iter_)`
  (`WellInterface_impl.hpp:1032`; gate defined `NewtonIterationContext.hpp:95` — true while
  `globalIteration_ < maxIter` and never during a local/NLDD solve). Default
  `max_niter_inner_well_iter_` (`MaxNewtonIterationsWithInnerWellIterations`,
  `BlackoilModelParameters.hpp:117`) = **99** — effectively unconditional for realistic outer
  iteration counts (rarely exceeds ~20), but not a hardcoded "always"; a config with a very low
  value would fall back to assembling wells at their last-reached state with no further inner
  solve for later outer iterations. Not scoped as a Bundle W knob; note the mechanism exists.

### B. Inner loop body (`iterateWellEqWithSwitching`, `StandardWell_impl.hpp:2458`)

`do { ... } while (it < max_iter)`, `max_iter = max_inner_iter_wells_`
(`MaxInnerIterWells`, `BlackoilModelParameters.hpp` = **50** for standard wells, 100 for MSW):

1. Every `min_its_after_switch` (= 4, hardcoded) iterations since the last control/status
   switch, `updateWellControlAndStatusLocalIteration` checks/applies **discrete** control-mode
   switching (rate↔BHP↔THP, open↔stop) — bounded by `max_well_status_switch_inner_iter_`
   (`MaxWellStatusSwitchInInnerIterWells` = 99, effectively unlimited). **This is structurally
   different from ResSim's continuous Fischer-Burmeister complementarity row** — OPM does
   periodic discrete re-evaluation of which control mode is active, not a smooth relaxation of
   an NCP residual. Per plan §5 "Explicitly NOT in Bundle W", ResSim's FB row is kept as-is;
   this is documented as a known structural divergence, not something to port.
2. `assembleWellEqWithoutIteration(...)` — linearizes the *local* well system at current
   primary variables.
3. Convergence check: `getWellConvergence(...)` (below). After `it > strict_inner_iter_wells_`
   (`StrictInnerIterWells` = 40), `relax_convergence = true` switches to the looser tolerance.
4. If converged and a switch happened recently (within `min_its_after_switch`), one more
   "final_check" pass runs before accepting, to make sure the post-switch state itself is
   consistent — else `break`.
5. `solveEqAndUpdateWellState(...)` — solves the local linear system and calls
   `updateNewton` (§C) to apply the chopped update; loop continues.

### C. Convergence test (`StandardWellEval::getWellConvergence`, `StandardWellEval.cpp:156`)

Two **separately-tolergranced** checks, not one blanket number — corrects the "tolerance-wells
= 1e-4" shorthand used loosely elsewhere in these docs to what it actually is:

- **Flux/mass-balance rows** (`res[eq_idx] = |linSys_.residual()[0][eq_idx]|` for
  `eq_idx` over the conservation-quantity indices, scaled by `B_avg[compIdx]`): checked against
  `tol_wells` = `ToleranceWells<Scalar>` = **`1e-4`** (`BlackoilModelParameters.hpp`), or
  `relaxed_tolerance_flow_well_` (`RelaxedWellFlowTol` = **`1e-3`**) once `relax_convergence`
  is set; hard fail above `max_residual_allowed_` (`MaxResidualAllowed` = **`1e7`**).
- **Control-equation row** (`checkConvergenceControlEq`, `WellConvergence.cpp:39`, residual =
  `|linSys_.residual()[0][Bhp]|`): tolerance **depends on the well's current active control
  mode** — `{rates: 1e3, grup: 1e4, bhp: 1e-4, thp: 1e-6}` (`StandardWellEval.cpp:211`,
  hardcoded literals, not named params). So "`tolerance-wells=1e-4`" is correct only for the
  BHP-controlled case (which is what the heavy repro case uses, both wells pinned at their BHP
  limits) and for the flux rows generally — a rate-controlled well's control-row tolerance is
  four orders of magnitude looser in absolute terms. **For Bundle W's `tolerance-wells` outer
  check (plan §5 step 3), use `1e-4` since the target cases are BHP-limited — but do not
  generalize that single number to rate-controlled wells without re-deriving it.**
- Additional check for BHP/THP-controlled wells: flow-direction sign consistency (producer
  must not have positive `WQTotal`, injector not negative) — a hard `WrongFlowDirection`
  failure, not a tolerance. ResSim's equivalent sign handling already lives in
  `apply_raw_update`/`enforce_control_bounds`.

### D. `updateNewton` chop (`StandardWellPrimaryVariables.cpp:262`, called via
`updatePrimaryVariablesNewton`, `StandardWell_impl.hpp:791`, from `updateWellState` →
`solveEqAndUpdateWellState`, i.e. every inner iteration)

- **BHP**: `dx1_limited = sign * min(|dwells[Bhp]|, |value_[Bhp]| * dbhp_max_rel_)`, floored at
  `bhp_lower_limit = 1 bar − 1 Pa`. `dbhp_max_rel_` (`DbhpMaxRel<Scalar>`) = **`1.0`** — matches
  exactly what the refuted Bundle N §5 follow-up ported verbatim
  (`opm_per_cell_chopped_update`'s well-BHP clamp). **Confirms plan §3's claim: this refuted
  fix's correct home is inside the inner well loop, not the outer Newton loop** — it was tested
  in the wrong place, not built wrong.
- **`WQTotal` (total well rate)**: `value_[WQTotal] -= dwells[0][WQTotal]` — **no magnitude
  clamp whatsoever**, only a post-hoc sign floor (injector ≥ 0, producer ≤ 0) and a hard zero
  for stopped/zero-rate-target wells. Reconfirms the already-established finding (worklog
  "Bundle N §5 follow-up", 2026-07-09) with a fresh, exact citation.
- **Fraction variables** (`WFrac`/`GFrac`, OPM's phase-split unknowns): chopped at
  `dwell_fraction_max_` (`DwellFractionMax<Scalar>` = **`0.2`**) with an extra
  producer-only relaxation factor. **No ResSim counterpart** — see note E below on the
  parametrization difference; not applicable to Bundle W's design as-is.

### E. Primary-variable parametrization difference (informative, does not change the design)

OPM's `StandardWell` (no polymer/MSW) uses **`[WQTotal, WFrac, GFrac, Bhp]`** — one *lumped*
total-rate unknown per well plus phase-fraction unknowns, **not one rate unknown per
perforation**. Per-perforation rates are *derived* from `WQTotal` + fractions + per-connection
transmissibility/mobility at solve time (`computePerfRate`), not solved as independent
unknowns. ResSim's `FimState` has one `perforation_rates_m3_day` entry per perforation
(confirmed `fim/state.rs`) — a different, more granular parametrization already baked into the
existing AD assembly (`wells_ad.rs`) that Bundle N/`FIM-LINEAR-010`/`FIM-DIAG-002` were all
built against. **This does not block Bundle W**: plan §2's constraint is that the inner solve
drives ResSim's *own* assembled residual rows, not that it replicates OPM's exact unknown
choice. It does mean the "bordered arrow system" shape described in plan §3 for multi-perf
wells is a ResSim-specific consequence of ResSim's own parametrization, not something mirrored
from OPM — call this out to any future reader diffing Bundle W's code against OPM 1:1. Only the
*structural* pattern (nested bounded Newton, converged before global assembly, excluded from
outer iteration count) transfers from OPM; the unknown parametrization does not.

### F. Failure policy

Non-convergence of `iterateWellEquations` (`WellInterface_impl.hpp:1081`, the `else` branch of
the `converged` check) sets `operability_status_.solvable = false` when
`shut_unsolvable_wells_` (`ShutUnsolvableWells` = **`true`**, default) — which the surrounding
`well_operable` check (`:1109`) turns into `stopWell()` + `solveWellWithZeroRate(...)`. **OPM
does not accept an under-converged well state and move on** — it forces the well to a
well-defined degraded state (zero rate) rather than propagating a stale/oscillating rate
forward. Confirms plan §5 step 2's "keep the last iterate and report not-converged... do not
widen acceptance" instinct, but sharpens it: the honest OPM-aligned behavior on inner-solve
failure is closer to "stop the well for this outer iteration" than "retry the outer substep" —
worth considering directly in W2's failure-reporting design rather than only bubbling up to the
existing outer retry ladder. Flag as an open design question for W2, not resolved here.

### G. Outer-criteria well exclusion — reconfirmed, one correction

- **Iteration count**: `SubStepIteration::getNumIterations_`
  (`AdaptiveTimeStepping_impl.hpp:1186`) reads `substep_report.total_newton_iterations`, which
  is accumulated once per call to `nonlinearIterationNewton`
  (`NonlinearSystemBlackOilReservoir_impl.hpp:237`, `report.total_newton_iterations = 1` per
  call = one *outer* iteration). Since all of a well's inner iterations (up to 50) happen
  *inside* the `assemble()` call within that one outer iteration, they never separately
  increment this counter. **Reconfirms the core Bundle N §5 claim** ("well-switching cost
  invisible to the outer iteration count feeding the timestep controller") with current
  citations, replacing the stale `BlackoilModel_impl.hpp:270` reference.
- **Correction**: the OUTER **convergence** check (as opposed to the iteration *count*) is NOT
  well-blind. `NonlinearSystemBlackOilReservoir::getConvergence`
  (`NonlinearSystemBlackOilReservoir_impl.hpp:1008`) computes
  `report = getReservoirConvergence(...)` then `report += wellModel().getWellConvergence(...)`
  — the aggregate outer convergence report *does* include a well-convergence term. In practice
  this rarely blocks anything additional (wells already converged via their own inner loop by
  the time this runs), but it means "N1's acceptance excludes well/perforation rows entirely"
  (`docs/FIM_BUNDLE_N_DESIGN.md` §5.1) describes ResSim's simplification, not literally OPM's
  structure. This is exactly the gap plan §5 step 3 already proposes closing (the
  `tolerance-wells` outer check) — now backed by a precise citation instead of an inference.

### H. Numeric defaults collected (for W2 implementation)

| Constant | OPM name | Value | Citation |
|---|---|---|---|
| Inner well iteration cap | `MaxInnerIterWells` | 50 | `BlackoilModelParameters.hpp` |
| Strict→relaxed inner switch | `StrictInnerIterWells` | 40 | same |
| Inner well flux tolerance | `ToleranceWells` | 1e-4 | same |
| Relaxed inner flux tolerance | `RelaxedWellFlowTol` | 1e-3 | same |
| Max residual (hard fail) | `MaxResidualAllowed` | 1e7 | same |
| BHP control-row tolerance | (hardcoded) | 1e-4 | `StandardWellEval.cpp:211` |
| BHP chop | `DbhpMaxRel` | 1.0 | `BlackoilModelParameters.hpp` |
| Outer iterations with inner well solve | `MaxNewtonIterationsWithInnerWellIterations` | 99 | same |
| Outer strict→relaxed well switch | `StrictOuterIterWells` | 6 | same |
| Shut on inner non-convergence | `ShutUnsolvableWells` | true | same |
| Min iterations before allowing a switch | (hardcoded `min_its_after_switch`) | 4 | `StandardWell_impl.hpp:2482` |
