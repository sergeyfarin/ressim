# FIM repair execution plan

Date: 2026-09-14. Planning base: `f9dd22e`, whose Rust implementation is unchanged from
`ffaf18f30f6d1cea5c7b097a26dc10d4d496303e`. **This is a plan, not a record of completed fixes.**

> **Documentation boundary (2026-09-19).** This plan is a completed historical record and its
> instructions are preserved as issued, including the milestone name
> `FIM-COMPOSITIONAL-SEAM-READY`, which was declared at `6be6d08` and is referenced by name as a
> satisfied prerequisite elsewhere. It is not renamed.
>
> Going forward the FIM documentation set describes the black-oil solver and the interfaces it
> publishes, in terms of "a second fluid model". It does not name, sequence, gate or review any
> particular downstream model: that work, its backlog and its validation evidence are owned by
> the model's own documents and branch. New FIM docs should not reintroduce the coupling this
> record contains.

## Purpose, authority and completion boundary

Make the existing black-oil FIM path a trustworthy foundation for new fluid models. Start with
reproduced contract failures, then validate diagnostics, state lifecycle, conservation and
accuracy before extracting reusable solver interfaces. Do not optimize iteration counts first.

This document owns the repair sequence. The [assessment](COMPOSITIONAL_READINESS_ASSESSMENT_2026-09-14.md)
owns audit findings; GitHub Issues owns task status; `FIM_EXPERIMENT_REGISTRY.md` and
`FIM_CONVERGENCE_WORKLOG.md` own experiments. The older
[OPM execution plan](FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md) remains evidence for its historical
research slices, not an instruction to restart WATER-011 or parked G4/G5 work.

Two explicit milestones:

- **FIM-REPAIR-READY:** F0–F6 complete, scoped correctness problems adjudicated, required gates
  green, and no missing observable used to justify a convergence decision.
- **FIM-COMPOSITIONAL-SEAM-READY:** F7–F8 also complete, with an interface contract that separates
  fluid-specific equations/updates from reusable linear/timestep machinery.

Standalone EOS/flash work in the [fluid-model plan](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md)
may start immediately. Global compositional FIM integration must wait for the second milestone.
Neither milestone requires solving every unrelated IMPES issue or matching every Flow Newton
count. A failure affecting a reused path cannot be waived merely because it predates this plan.

## Instructions for the executing model

1. Execute **one F-number task at a time**. Read its prerequisites and finish its evidence record
   before starting the next. Split a task into several commits when it changes independent
   mechanisms; never bundle a mechanical extraction with a physics fix.
2. Read `.github/copilot-instructions.md`, `.claude/skills/ressim-validation/SKILL.md`,
   `.claude/skills/engine-physics-change/SKILL.md`, and
   `.claude/skills/fim-solver-debug/SKILL.md`. Read the OPM skill for reference runs. Follow
   current user authorization; a decision checkpoint below is a technical evidence requirement,
   not an automatic request for permission.
3. Inspect `git status --short --branch` and `git rev-parse HEAD`. Preserve unrelated changes.
   Use a clean, isolated worktree if needed. Do not pull, push or publish unless requested.
4. Do not assume this plan's test counts remain current. Run the named tests and verify each
   filter matches at least one test. A renamed filter must be resolved by reading source.
5. Never run unrestricted `cargo test`. Never loosen existing tolerances, ignore a failing test,
   re-enable the old nine-cell mobility average, change public defaults, or tune the controller
   merely to turn a test green.
6. If the required source equation or oracle is unavailable, record **INCONCLUSIVE** with the
   missing quantity. Continue independent work only. Do not invent an OPM behavior.
7. A task that needs a new numerical choice must write the equation, units, variable meanings,
   source reference and acceptance test before implementation. If evidence cannot choose between
   alternatives, stop that task and request a focused numerical review with the evidence packet.
8. After validation, commit the scoped change. Link the commit and results to the owning issue.
   Do not close an issue unless all of that issue's acceptance criteria are satisfied.

## Task map

| Task | Depends on | Owning issue | Deliverable |
| --- | --- | --- | --- |
| F0 | None | #24 / #27 / #28 | Current evidence packet and reconciled entry guidance |
| F1 | F0 | #28 | Source- and FD-supported well influence stencil |
| F2 | F0; preferably F1 | #27 | Consistent gas surface-rate derivative contract |
| F3 | F1, F2 | #13 | Local and CI gates include the repaired contracts |
| F4 | F3 | #23 | Verified full-system linear diagnostic contract |
| F5 | F4 | #12; separate defect issues if found | State/rollback/phase-transition and inventory gates |
| F6 | F5 | #10 / #11 / #12 / #21 / #25 | Case-scoped accuracy and applicability decisions |
| F7 | F6 | #22, coordinated with #29 | Minimal reusable solver seam, unchanged black-oil behavior |
| F8 | F7 | #29 and applicable repair issues | Committed handoff baseline and readiness declaration |

Issue URLs use `https://github.com/sergeyfarin/ressim/issues/<number>`. These task IDs organize
execution; do not add a parallel status checklist to `TODO.md`.

## F0 — Freeze facts and reconcile the starting point

**Read:** assessment; `FIM_STATUS.md`; latest applicable registry rows, particularly
`FIM-STATE-001`, `FIM-FLAVOR-001`, `FIM-RELPERM-001`, `FIM-LINEAR-014`; worklog WATER-028;
issues #27/#28 and their latest comments. Read the opening and relevant section of the old OPM
execution plan, recognizing that its opening status predates later promotions.

**Actions:**

1. Record commit, branch, compiler/tool versions and clean-tree state. Establish an evidence
   directory outside tracked source, e.g. `/tmp/ressim-fim-repair-F0-<commit>`; do not overwrite
   an older run. Record exact commands, stdout/stderr, exit status and wall time.
2. Run G0 below. The audit observed 5/5 integration-well tests, 16/18 local-well tests and 13/13
   AD assembly tests. Treat those as historical expectations, not current results.
3. Run the locked tests and bounded WASM controls once on the committed pre-change engine
   (G1 and G3). Record baseline failures before editing. A build changing generated WASM files
   must be explained; a mismatched source/package run is provisional, not a clean baseline.
4. Reconcile only stale entry statements in `FIM_STATUS.md`, the old execution-plan header,
   and affected skill summaries: FIM ships; OpmAligned is currently default; direct oil
   inventory reporting exists; WATER-028 corrected the studied oil-bias attribution. Link the
   latest evidence rather than rewriting historical experiment verdicts or copying old counts.
5. Write a short baseline section in the worklog. Capture issue ownership for every unexpected
   failure; do not silently make the known-failure list larger.

**Exit:** precise initial failure list and source/default map; no runtime changes. Commit docs
separately. If baseline controls time out, record the last completed step and do not call it a
passing baseline. This need not block F1/F2's local diagnosis.

## F1 — Resolve the local well influence stencil (#28)

**Files:** `src/lib/ressim/src/fim/wells.rs`, `wells_ad.rs`, `assembly_ad.rs`, `assembly.rs`.
Read `producer_control_state`, `perforation_control_cells`, `control_influence_cells`,
`producer_rate_sensitivity`, and test
`local_block_perforation_control_cells_match_existing_control_stencil`.

**Evidence already visible:** `perforation_control_cells` returns only the connected cell.
Comments attribute this to Bundle X and OPM `WellInterface::getMobility`; the test still expects
all nine cells of a 3x3 grid. This strongly suggests a stale test, but verify actual consumers.

**Steps:**

1. Read `.archive/docs/FIM_BUNDLE_X_PLAN.md`, its registry/worklog result, and the pinned local
   OPM `WellInterface_impl.hpp` implementation. Use `rg` to locate symbols rather than trusting
   line numbers. Record that mobility locality does not imply the entire coupled well system
   has no inter-perforation/pressure dependence.
2. Build a deterministic heterogeneous 3x3 fixture with different neighbor saturation and
   pressure values. Uniform states cannot reveal unintended averaging.
3. Hold BHP and perforation-rate unknowns fixed. Perturb each neighbor primary independently
   and evaluate the local fraction/source/control helpers. Then inspect the corresponding
   **unreduced** assembled well rows. Neighbor reservoir flux rows will change; they are not
   the observable under test. Schur reduction can introduce additional coupling and is not
   a substitute for this locality test.
4. If source and production AD both show connected-cell locality, replace the obsolete
   nine-cell assertion with a named connected-cell invariant plus an independence regression
   test. Document why changing the expectation corrects a stale oracle. Do not restore averaging.
5. If a live consumer still averages neighbors, fix all consumers of that same physical
   quantity together, including reporting if affected, and preserve AD/scalar agreement.
   If the well control intentionally aggregates several completions, test that aggregation
   separately instead of deleting those dependencies.

**Validation:** G0; `cargo test --manifest-path src/lib/ressim/Cargo.toml wells_ad -- --nocapture`;
then G1/G2. Run G3/G4 if production behavior changes. Any production change must record a
source/FD-backed verdict; a test-only correction is not a convergence promotion.

**Exit:** locality and multi-completion aggregation are separately specified and tested; #28
has the implementing commit and validation. #27 may remain the sole known local-suite failure
until F2, explicitly recorded rather than ignored by the test command.

## F2 — Resolve gas-injector surface-pressure derivatives (#27)

**Files/symbols:** `fim/wells.rs`:
`perforation_surface_rate_pressure_derivative`, `perforation_source_pressure_derivatives_sc_day`,
`perforation_surface_rate_sc_day`, `FimWellLocalBlock::total_rate_from_unknowns`,
`physical_well_control`; `fim/wells_ad.rs` generic counterparts; `fim/state.rs` update methods;
`pvt.rs` and `get_d_bg_d_p_for_state`.

The failing hand derivative is currently `#[cfg(test)]`. Do not claim a production derivative
bug from that failure alone. The current FD test uses `apply_newton_update`; first verify that
its perturbation does not also project state, switch regimes or change another unknown.

**Steps:**

1. Write a small quantity table: signed reservoir connection q, positive injection surface
   rate, signed reservoir component source, well-control residual, BHP, pressure and Bg.
   Record units and which variables are held fixed. Distinguish derivative of a rate from
   derivative of the residual containing that rate.
2. Trace the actual control selection (`enabled`, injector type, `uses_surface_target`) for
   the failing fixture. Verify the API settings create the intended surface-rate control.
3. For the unclipped injection branch q<0, surface injection is `-q/Bg`; at fixed q its
   pressure derivative is `q * dBg/dp / Bg^2`. A signed source can have the opposite sign.
   Compare this derivation with the exact function the FD test evaluates, not just its name.
4. Clone the base state and perturb only `pressure_bar` directly at an interior PVT point;
   hold q, BHP, regime and other primaries fixed. Compare central differences with relative
   pressure steps `1e-4`, `1e-5`, `1e-6`, `1e-7` using `max(abs(p),1 bar)` as the scale.
   Require a stable error region; roundoff or a crossed table knot is not a verdict.
5. Compare four quantities at that same state: independently derived scalar derivative,
   test-only hand helper, generic AD derivative, and the actual assembled well-row entry
   after accounting for residual sign/scaling. Print raw values on failure.
6. Repeat for PVT interpolation and its smooth constant-property limit; for q=0 or a table
   knot use the documented one-sided/active-branch contract, not a central FD crossing a kink.
   Add BHP-controlled/disabled control cases to distinguish zero control derivatives from
   nonzero physical-source derivatives. Keep the scoped typed RESV route separate.
7. Correct the side proven wrong. If only the test helper or fixture is wrong, do not edit
   production assembly. If production is wrong, update scalar/AD consumers and tests in the
   same causal patch. Never take absolute values merely to erase a sign mismatch.

**Validation:** named failing test, local well suite, `wells_ad`, `assembly_ad`, then G1/G2;
G3/G4 for a production fix. Retain the existing `1e-3` threshold unless a separately documented
numerical analysis justifies a change; changing it is not the proposed fix.

**Exit:** named source, control and residual derivatives agree on the same frozen state;
both formerly failing tests pass; #27 updated with root-cause classification and commit.
If the FD/AD/source contract remains ambiguous, stop F2 and provide the four-column comparison.

## F3 — Make failures visible to local validation and CI (#13)

**Files:** `scripts/validate-solver-coverage.sh`, `.github/workflows/pr-tests.yml`,
`.claude/skills/ressim-validation/SKILL.md`, relevant README validation section.

1. Add `fim::wells::tests::` to the FIM bucket without removing `fim::tests::wells::`.
   Include `wells_ad` and `assembly_ad` in the appropriate FIM regression bucket if not already
   selected elsewhere. Inventory existing filters to avoid unnecessary repeated work.
2. Keep the script's nonzero-match check. Make an ignored-only match fail: current counting
   sums passed and ignored tests, so counting ignored tests is insufficient evidence of
   execution. Validate this behavior with a bounded script harness or controlled test fixture,
   without committing deliberately failing Rust tests or temporarily deleting production tests.
3. Run the resulting FIM bucket and inspect each `gate ok` line. The suite must actually
   execute the repaired tests; filtering their names out to obtain green is forbidden.
4. Make PR workflow coverage explicit: run shared/FIM/IMPES Rust buckets and BL, typecheck,
   lint, cycle check, build and full Vitest once. Reuse the existing coverage run for full
   Vitest. Keep expensive release-only scientific replays as explicit release/relevant-change
   gates rather than adding unbounded jobs to every PR.
5. Document total local command set and measured CI/local costs separately. A workflow edit
   alone cannot prove remote checks or branch-protection settings were activated. Do not push
   to trigger CI unless requested; record local validation and remote status honestly.

**Validation:** shell syntax (`bash -n scripts/validate-solver-coverage.sh`), zero/ignored-match
behavior, G4; inspect workflow/script references and duplicated test invocations.

**Exit:** no silent omission or zero-execution success for the repaired contracts. Local
validation is green; any unexecuted remote workflow remains explicitly unverified.

## F4 — Verify the linear diagnostic and recovery contract

**Files:** `fim/linear/mod.rs`, `well_schur.rs`, `gmres_block_jacobi.rs`,
`solvers/faer_sparse_lu.rs`, `fim/linear/capture.rs`, `solver_lab.rs`.

1. Run existing `well_elimination_matches_direct_full_system_solve` and
   `well_schur_report_uses_full_system_norms` tests. Read `recover_full_system_report` and all
   backend return paths, including failure and singular-direct fallback.
2. Make a coverage table for iterative/direct, with/without well tail, finite/nonfinite
   correction, no-tail passthrough and singular/rejected solve. Reuse existing fixtures.
3. For every returned correction, independently compute `r = rhs - J*dx` on the **original
   full system**. Report RHS norm, full residual norm, reduction, finite status, reservoir and
   well partitions, actual backend, and acceptance reason. A backend that returns no correction
   must say unavailable; do not substitute zeros or a reduced norm.
4. Test report values against the independent calculation. Check that Schur back-substitution
   reconstructs the full correction and that fallback reports its actual backend.
5. If any observable is missing, repair that reporting defect before using it for a convergence
   conclusion. Do not change stopping tolerances or preconditioning in this task.
6. For any captured-system comparison, retain dimensions, row/column meanings, units, scaling,
   state identity, source commit and capture version. Compare correction vectors only after
   all mappings are explicit. A singular compatible system is not a valid ordinary-LU oracle.

**Validation:** focused new/existing report tests, G1/G2; targeted captured-system replay only
when a report defect requires it. Use G3/G4 if a runtime return/acceptance path changes.

**Exit:** a populated diagnostic coverage table and tested recovery contract. If coverage is
already complete, document that result and add no redundant abstraction or duplicate tests.

## F5 — Audit phase lifecycle, rollback and conservation

**Files:** `fim/state.rs`, `flash.rs`, `flash_ad.rs`, `properties.rs`, `newton.rs`,
`newton/convergence.rs`, `timestep.rs`, `reporting.rs`, `tests/physics/pvt_flash.rs`.

1. Map stored primary -> phase classification -> property evaluation -> accumulation ->
   Newton update -> accepted-state check -> simulator commit. Mark OPM semantics as matched,
   deliberately different, or missing. Keep raw intermediate state distinct from final-state
   admissibility. Do not globally clamp all primaries to make them appear physical.
2. Reuse existing Sg/Rs transition fixtures and test both disappearance and appearance,
   DRSDT0 caps, endpoint extension, and pressure-only perturbations. At smooth states compare
   Jacobians to FD; at switches test one-sided semantics and component inventory continuity.
3. Verify that the residual checked for acceptance is for the state actually committed.
   Post-Newton phase conversion, well relaxation or projection must not create an unmeasured
   accepted residual. Audit both OpmAligned and supported Legacy behavior.
4. Add a deterministic reject/retry test, using a test-only injected failure if necessary.
   Compare a retried step to a clean run starting with the accepted smaller dt. No rejected
   attempt may advance time, component totals, cumulative production/injection or history.
   Explicitly separate intentional retry-controller memory from physical state rollback.
5. Compute initial/final water, oil and gas inventories independently in tests and integrate
   accepted sources with actual accepted dt. Do not define an error from the same cumulative
   reporting field being tested. Record signed and absolute errors, normalization, source
   convention, and final equation residual.
6. If a defect is found, create/link its issue and add a new registry row before a behavioral
   experiment. Fix the smallest complete lifecycle; do not mix with damping or controller work.

**Validation:** relevant existing transition tests (including
`y2b3b_one_cell_transitions_keep_primary_column_live_and_factorable`), new rollback/acceptance
contracts, G1–G4 for production changes; BL for shared-property changes. New tests must use
unique `fim_repair_` names so they can be run as a bounded group, then be added to curated gates.

**Exit:** no unmeasured accepted-state mutation or rejected-step inventory leak on the selected
fixtures; source/inventory and derivative contracts pass. State the supported physical envelope.

## F6 — Establish accuracy and classify remaining physics work

**Read:** `BLACK_OIL_VALIDATION.md`, `THREE_PHASE_VALIDATION.md`,
`SOLVER_COMPARISON_SUMMARY.md`, issue bodies #10/#11/#12/#21/#25 and latest evidence.

1. Build an applicability table for closed depletion, gas injection/appearance, waterflood,
   multi-completion gravity, and gas reporting. Columns: issue, exact reproduction, current
   result, reused FIM path, external oracle, blocker/independent/deferred with reason.
2. Reproduce #11 and #25 if they affect the supported model envelope; compare cell inventory,
   accepted source integration and reported totals before changing PVT or well rates.
   For #10, distinguish an IMPES-only pressure failure from a shared well geometry error.
   Existing issue titles are not proof of either cause.
3. Reproduce a matched case from #21 with fixed grid/properties/wells and a timestep ladder.
   WATER-028 is prior evidence that its coarse-step oil difference was temporal; do not reopen
   the old pressure-volume hypothesis without contradictory refined results.
4. Compare pressure, cumulative production/injection and component inventory at the same
   physical times. Use at least three dt levels with identical reporting/integration semantics;
   separate grid refinement from dt refinement. Convergence to an external trajectory matters
   more than identical substep counts. Do not assume error must decrease monotonically near a
   phase event; inspect event timing and refined limits.
5. Reuse published acceptance thresholds from the owning validation documents. For a new
   oracle, define quantities and tolerances before seeing the candidate result. Missing or
   mismatched decks, state data or units are INCONCLUSIVE.
6. Every failing applicable case gets a separate bounded repair task with a test and issue.
   Do not silently expand this stage into controller tuning or a full OPM port. Continue to F7
   only once failures affecting its reused foundation are fixed or proven outside its scope.

**Validation:** G5 for black-oil acceptance when applicable, G4 for fixes, and exact issue
replays with captured commands. OPM work uses the reference-pipeline skill.

**Exit:** FIM-REPAIR-READY record with an evidence-backed applicability table. Unrelated deferred
work has named issues and does not get described as fixed.

## F7 — Expose a minimal model/solver boundary (#22)

**Read first:** `fim/newton/damping.rs`, `convergence.rs`, `diagnostics.rs` already exist.
Do not repeat an extraction already completed since issue #22 was written.

1. Inventory remaining direct `ReservoirSimulator`/three-component dependencies in Newton,
   timestep orchestration and linear layout. Classify each as geometry, fluid equation, update
   policy, convergence policy, well model, or reporting.
2. Write a short interface design for: assembly result/layout; linear correction report;
   model-specific trial update/admissibility; accepted-state residual evaluation; step snapshot,
   commit and rollback. The black-oil adapter must preserve current operation order and defaults.
3. Extract only what the compositional plan consumes. Start with pure data/layout and report
   interfaces, then lifecycle orchestration if needed. Keep pressure/Sw/Rs damping and phase
   switching in the black-oil implementation. Do not make all physics generic preemptively.
4. For each extraction, compare pre/post deterministic residuals, corrections, accepted states
   and trace counts on held fixtures. Wall time is informational; timings are not bitwise data.
   Unexpected arithmetic-order changes require explanation and equivalence evidence.
5. Do not remove legacy/scalar reference assembly or specialize shared code to compositional
   behavior. Keep this task free of fluid-model implementation and new numerical policy.

**Validation:** focused extracted-policy tests, `assembly_ad`, G1–G4. If a behavior delta occurs,
stop and isolate/revert only this task's edit; do not adjust tolerances to hide it.

**Exit:** existing behavior preserved and the compositional executor can name concrete reusable
interfaces. Update #22 by its actual remaining scope; do not close it merely for writing design.

## F8 — Final replay and handoff

1. Complete scoped implementation commits and remove only task-owned temporary probes.
2. Run G0–G4 on the final committed engine, and G5 for applicable scientific changes. If a gate
   leads to another code change, rerun affected gates on that final state before promotion.
3. Record exact commit, package provenance, commands and verbatim summaries. Update the owning
   validation docs, experiment registry/worklog and issue links. Label any incomplete replay
   provisional; do not publish a new baseline from an intermediate tree.
4. Produce a handoff table: repaired contracts, validated envelope, outstanding excluded issues,
   reusable interfaces, schema/unit conventions, and the first unblocked C-task in the fluid plan.
5. Declare FIM-COMPOSITIONAL-SEAM-READY only if all applicable prerequisites passed. Commit the
   evidence/documentation. Do not push unless requested.

## Validation command catalog

Run from repository root. Commands here exist at the planning revision. Check nonzero test
counts. Use a task-specific log directory; avoid output pipes that discard the command exit code.
Use `timeout 600s` for an unexpectedly slow focused test/control and `timeout 1800s` for an
initial full gate or release replay. These are investigation safety caps, not performance targets.
Exit 124 means incomplete; inspect before increasing the cap. Do not repeatedly rerun a hanging
test or reinterpret a timeout as a physics refutation.

### G0 — Initial and repaired well/AD contracts

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml local_block_perforation_control_cells_are_the_connected_cell_only -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml perforation_control_quantities_ignore_neighbor_cell_state -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml gas_injector_surface_pressure_derivatives_match_local_fd -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml fim::tests::wells:: -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml fim::wells::tests:: -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml assembly_ad -- --nocapture
```

If F1 legitimately renames its obsolete test, update this command and the filter inventory in
the same commit; do not keep a zero-match command as a passing gate.

### G1 — Locked FIM contracts

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas -- --nocapture
```

### G2 — Curated FIM/shared gates

```bash
bash scripts/validate-solver-coverage.sh fim
bash scripts/validate-solver-coverage.sh shared
```

### G3 — Rebuilt WASM behavior matrix

```bash
bash scripts/build-wasm.sh
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 22x22x1 --steps 1 --dt 0.25 --diagnostic summary --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 --diagnostic summary --no-json
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic outer --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json
```

Record completion time, real/replayed accepts, retries by cause, applied Newton updates versus
residual evaluations, linear work, outputs and inventory errors. Never compare Flow applied
updates directly to ResSim residual evaluations. Use complete sequential runs for carryover;
isolated checkpoint replay does not preserve every cross-step controller state. Add the exact
affected scenario if these controls do not exercise a fix. Compare Legacy as well when its path
changes, using the runner's existing `--legacy` option.

### G4 — Pre-commit production/shared validation

```bash
cargo fmt --manifest-path src/lib/ressim/Cargo.toml -- --check
bash scripts/build-wasm.sh
pnpm run validate:full
cargo test --manifest-path src/lib/ressim/Cargo.toml benchmark_buckley -- --nocapture
git diff --check
```

`validate:full` includes product validation and all curated Rust buckets; do not rerun those
buckets immediately just to duplicate G2. The explicit initial WASM build prevents frontend
simulation tests from consuming stale bindings before the script's final build step.

### G5 — Relevant scientific acceptance, not the default inner loop

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml spe1_full_horizon_matches_published_reference -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml spe1_areal_refinement_reference_error_replay -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml physics_depletion_grid_convergence_fim -- --ignored --nocapture
bash scripts/opm-ressim-compare.sh --case gas-rate-10x10x3 --out-dir /tmp/ressim-fim-repair-opm
```

Use a fresh output directory per run. The comparison script supports the named gas case only;
it is not a generic runner for arbitrary water or compositional cases. Read its fixture checker
and compare deck physics/units before interpreting output.

## Required end-of-task handoff

```text
Task: F<number>; issue:
Starting commit / final implementation commit:
Hypothesis or contract; source equation and units:
Changed files and why:
Oracle; missing or deliberately held-constant semantics:
Exact commands, exit status, matched test counts, artifact paths:
Before/after outputs; control differences and interpretation:
Verdict: completed contract / PROMOTED / REVERTED / REFUTED / INCONCLUSIVE / DIAGNOSTIC
Remaining blocker and next permitted task:
```

Do not report “FIM fixed” without identifying the validated envelope and remaining exclusions.

## Copyable executor prompt

```text
Execute the next uncompleted task in docs/FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md,
starting with F0 if there is no committed handoff. Read the task's owning issue and the
repository skills first. Do not assume unchecked issue criteria or plan tasks are complete.
Work only within that task's scope; use its exact gates and nonzero test-count checks.
Before changing a derivative or test expectation, record the source equation, variable/sign
convention and independent oracle. Never loosen tolerances to remove a failure.
If the evidence is insufficient, produce the smallest failing fixture and a precise review
question; do not start a speculative solver change. Commit validated changes, update the
owning issue and leave the required handoff with the next permitted task. Do not push.
```
