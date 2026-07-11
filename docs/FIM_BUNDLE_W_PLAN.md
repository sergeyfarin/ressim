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

- **W1 — local system extraction + the agreement test.** New `fim/wells_inner.rs` (or a module
  in `wells_ad.rs`): per-well local residual/Jacobian from the existing AD building blocks.
  THE key unit test, directly encoding the §2 constraint: for a constructed state, the local
  residual entries bit-match the corresponding rows of the full `assemble_fim_system_ad`
  residual, and the local Jacobian matches the corresponding global sub-block (single-perf and
  ≥2-perf physical well cases).
- **W2 — inner Newton loop.** Damping/chop, scaled convergence, iteration bound, failure
  reporting. Unit tests: BHP-pinned trivial case converges in 1 iteration; rate-controlled FB
  case converges and lands on slack-feasible (bhp, q); a deliberately infeasible case reports
  non-convergence without panicking.
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
