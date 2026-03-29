# FIM Cleanup Plan

Purpose: reduce FIM-related noise before more convergence work so new edits are evaluated against one clear baseline instead of a mix of active code, stale probes, and overlapping notes.

## Cleanup Goals

- keep all solver-relevant findings
- separate production regressions from diagnostics
- remove stale artifacts and ambiguous backup files
- reduce duplicate status reporting across `TODO.md`, `docs/`, and ad hoc notes
- make it obvious which files are canonical for current FIM status, active investigation, and future architecture

## Canonical Sources After Cleanup

- `TODO.md`
  - active tasks only
  - short status bullets only
  - no long historical FIM narrative
- `docs/FIM_STATUS.md`
  - current FIM implementation state
  - known parity gaps and current blockers
  - links to the active worklog and migration plan
- `docs/FIM_CONVERGENCE_WORKLOG.md`
  - active investigation log only
  - dated experiments and temporary findings while an issue is live
- `docs/FIM_MIGRATION_PLAN.md`
  - target architecture and intended end state
  - not used as a live status tracker

## Current Problems

### Documentation sprawl

- `TODO.md` contains too much historical FIM narrative and is hard to use as a live tracker.
- `docs/FIM_CONVERGENCE_WORKLOG.md` mixes resolved items, active hypotheses, and historical context.
- `docs/FIM_MIGRATION_PLAN.md` is correct in role but needs to stay separate from implementation status.

### Diagnostic clutter

- `src/lib/ressim/src/lib.rs` still contains ignored debug helpers for SPE1 and injector-balance probes.
- `src/lib/ressim/src/tests/fim_spe1_bug.rs` appears stale and not part of the canonical test surface.

### Artifact clutter

- `src/lib/ressim/src/fim/scaling.rs.orig` is a dead backup file.
- repository-root `*.patch` and `patch_*.sh` files are ambiguous experimental artifacts and should not remain undocumented in the root.

### Code-surface ambiguity

- `src/lib/ressim/src/fim/mod.rs` still uses `#![allow(dead_code)]`, which hides unused FIM code instead of making it explicit.
- the canonical wasm diagnostic path now owns deep solver traces, so older references to native-only verbose tooling should be treated as historical context, not active workflow.

## Recommended Cleanup Sequence

## Phase 1: Documentation Cleanup

1. Create `docs/FIM_STATUS.md`.
   - include a short summary of the current FIM state
   - list known open parity or convergence gaps
   - list the canonical regression tests and diagnostic entry points

2. Compress the FIM history in `TODO.md`.
   - keep active tasks and short status bullets only
   - move long historical narrative to a dated archive doc if needed

3. Add a short header to `docs/FIM_CONVERGENCE_WORKLOG.md`.
   - state that it is an active investigation log
   - point readers to `docs/FIM_STATUS.md` for the current consolidated status

4. Keep `docs/FIM_MIGRATION_PLAN.md` architecture-only.
   - avoid adding live debugging status there

## Phase 2: Test and Diagnostic Classification

Execution plan for review before implementation: `docs/FIM_PHASE2_EXECUTION_PLAN.md`.

Classify each FIM-related test into exactly one of these buckets:

- production regression
- ignored diagnostic
- obsolete probe

Review apporach to rust native target. It was used for testing, but often slightly different inputs used which causes wrong investigation of wasm failing, while never confirmed always was logic issue. Native target not required for production was used for testing only. It is useful sometimes for diagnostic, but if it causes more issues, then let's always diagnose using wasm

Approved direction: wasm is now the default diagnostic target. Native-only diagnostics should be removed unless they provide unique value that cannot be exposed from wasm.

And remember there are also frontend scenarios `src/lib/catalog/scenarios` that could be used for testing, but some of them slow to run, especially for long periods (`src/lib/catalog/scenarios/spe1_gas_injection.ts` and `src/lib/catalog/scenarios/sweep_combined.ts` are useful but will be very very slow)

Some tests are overly verbose, printing every substep, every iteration - make it difficult to use for diagnostic.

Approved direction: keep diagnostics switchable and structured, but do not make per-substep and per-iteration spam the default behavior.



Target files:

- `src/lib/ressim/src/tests/spe1_fim.rs`
  - keep as the home for stable SPE1/FIM regressions
- `src/lib/ressim/src/tests/fim_spe1_bug.rs`
  - either delete if stale, move useful content into `spe1_fim.rs`, or convert into a documented ignored diagnostic
- `src/lib/ressim/src/lib.rs` ignored debug helpers
  - move them into a dedicated diagnostics file or remove them if superseded
  - also files `test_import.mjs`, `test-wasm-spe1-short.sh`, `test-wasm-spe1.js`, `test-wasm.sh`, `test.sh`

End-state rule: no debug-only probe should live in `lib.rs` and tests should not sit in `lib.rs` they should be in dedicated files not to clutter it, tests and probes should all be in separate dedicated files.

Preferred script end-state: one canonical wasm diagnostic script with switches for scenarios, grids, settings, and diagnostic verbosity. Delete broken scratch files immediately.


## Phase 3: Artifact Cleanup

1. Delete `src/lib/ressim/src/fim/scaling.rs.orig`.

2. Audit root patch artifacts:
   - `assembly.patch`
   - `assembly_fb.patch`
   - `newton.patch`
   - `scaling.patch`
   - `scaling2.patch`
   - `wells.patch`
   - `patch_assembly.sh`
   - `patch_newton.sh`
   - `patch_scaling.sh`
   - `patch_wells.sh`

For each one, choose exactly one outcome:

- delete because already merged
- delete because abandoned
- move to `docs/experimental-patches/` with a short README explaining why it remains

End-state rule: no undocumented experimental patch files in the repository root.

## Phase 4: Code Surface Cleanup

1. Remove or narrow blanket dead-code suppressions.
   - start with `src/lib/ressim/src/fim/mod.rs`
   - replace broad suppression with targeted `allow(dead_code)` only where justified

2. Document the wasm-first deep diagnostic path.
  - `test-wasm.sh` and `scripts/fim-wasm-diagnostic.mjs` should be the documented entry points for structured summaries and per-Newton traces

3. Add a short module-level note to `src/lib/ressim/src/fim/mod.rs`.
   - state current role of the module tree
   - point to `docs/FIM_STATUS.md` for solver status and active gaps

## Phase 5: Regression Baseline Definition

Define one short canonical solver-validation set for day-to-day FIM work.

Suggested baseline categories:

- flash/regime unit regressions
- assembly/Jacobian exactness tests
- one or two short SPE1/FIM smoke regressions
- one diagnostic path that is wasm-first; native is only acceptable if it still provides unique value

End-state rule: when an edit makes convergence better or worse, the comparison set is fixed and documented.

## Keep / Move / Delete Inventory

Keep:

- `src/lib/ressim/src/fim/`
- `src/lib/ressim/src/tests/spe1_fim.rs`
- `docs/FIM_CONVERGENCE_WORKLOG.md`
- `docs/FIM_MIGRATION_PLAN.md`

Move or consolidate:

- ignored debug helpers from `src/lib/ressim/src/lib.rs`
- any useful content from `src/lib/ressim/src/tests/fim_spe1_bug.rs`
- long historical FIM status text currently embedded in `TODO.md`

Delete unless an audit proves otherwise:

- `src/lib/ressim/src/fim/scaling.rs.orig`
- stale root-level patch files already merged or abandoned

## Exit Criteria For Cleanup Pass

Cleanup is complete when all of the following are true:

- one current-status FIM doc exists
- `TODO.md` FIM section is short and action-oriented
- all FIM tests are clearly categorized as regression or diagnostic
- no debug-only probe remains embedded in `lib.rs`
- no unexplained backup or patch artifact remains in the repository root
- the default workflow for FIM debugging and validation is written down in one place and points to the wasm-first diagnostic runner

## Recommended Order Of Execution

1. docs cleanup
2. test classification
3. move or delete debug probes
4. artifact cleanup
5. dead-code and module-surface cleanup
6. define and document the canonical regression baseline

Do not start another round of convergence tuning until this pass is complete.