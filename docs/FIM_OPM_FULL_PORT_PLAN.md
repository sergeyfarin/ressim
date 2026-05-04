# OPM-Compatible FIM Solver Port Plan

Date: 2026-05-04
Status: durable implementation plan; no solver behavior is promoted by this
document alone.

## Current Status

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 0: License | Complete | GPL-3.0-or-later metadata, root `LICENSE`, and root `NOTICE` are in place. |
| Phase 0: Reference Harness | Partial | `scripts/opm-ressim-compare.sh` now provides repeatable ResSim/OPM commands; water and gas OPM decks still need to be generated. |
| Phase 1: Scaling + CPR Bundle | Partial / gated | Row+column scaling and physical-update recovery are implemented behind `FimLinearSolveOptions::opm_linear_scaling`; default remains `false` because the current branch does not pass the performance/promotion gates when enabled globally. |
| Phase 2: Well Block Port | Not started | Waits for Phase 1 promotion or a deliberate decision to proceed with Phase 1 still gated. |
| Phase 3: Nonlinear Update Policy | Not started | Waits for Phase 1/2 direction. |
| Phase 4: AMG Revisit | Not started | Explicitly deferred until Phases 1-3 create a stable scaled/well-aware CPR operator. |

## Summary

This plan makes ResSim's Rust/WASM FIM solver follow OPM Flow's coupled
numerical contract for simplified Cartesian black-oil cases. The goal is not a
piecemeal solver tweak. The goal is to move the related pieces together:
linear-system scaling, CPR restriction, well block handling, nonlinear update
policy, and validation against OPM reference runs.

The current `experiment/fim-amg-scirs2` branch is evidence, not a promotion
candidate. Phase A showed that copying OPM Quasi-IMPES weights and AMG
defaults without OPM-compatible scaling can create a wrong Newton direction,
case-3 physics drift, and severe case-3/case-4 regressions.

ResSim is now allowed to take a GPL-compatible route. Before direct
line-by-line OPM-derived source is introduced, the repo must carry
GPL-3.0-or-later metadata and attribution for any translated OPM material.

## Phase 0: License And Reference Harness

Status: licensing complete; executable reference harness partially complete.

Objectives:

- Make the repository GPL-compatible for direct OPM-derived implementation.
- Define canonical OPM parity cases before changing solver behavior.
- Make OPM-vs-ResSim comparisons reproducible and resistant to false
  conclusions from OPM's own timestep discretization.

Implementation:

- Add root `LICENSE` containing GPLv3 text.
- Add `license = "GPL-3.0-or-later"` to Rust crate metadata.
- Add `"license": "GPL-3.0-or-later"` to npm package metadata and the root
  package-lock entry.
- Add root `NOTICE` explaining that future OPM-derived implementation must
  retain source-file and release/commit attribution.
- Keep this plan in `docs/FIM_OPM_FULL_PORT_PLAN.md` as the durable roadmap.

Remaining Phase 0 work:

- `scripts/opm-ressim-compare.sh` is the canonical command harness. It runs
  ResSim diagnostics for all configured parity cases and runs OPM Flow when
  the matching deck exists.
- `npm run compare:opm -- --dry-run --no-build-wasm` prints the full command
  matrix without running expensive simulations.
- Generate and validate OPM decks for:
  - `water-medium-step1`
  - `water-medium-6step`
  - `gas-rate-10x10x3`
- Existing OPM decks are wired for:
  - `heavy-water-12x12x3`
  - `heavy-water-finedt`
- Store comparison outputs under `worklog/` or another ignored run-log
  location, with the dt-refinement table next to each physics metric.

Reference cases:

- Water parity case: medium-water `20x20x3`, single-step and six-step
  variants already used by the FIM shortlist.
- Gas parity case: the existing SPE1/FIM gas path plus the gas-rate
  `10x10x3` shortlist case.
- Heavy physics guard: heavy-water `12x12x3` with fine-dt FOPT compared to
  the converged OPM reference, currently documented around FOPT 3826.

Reference rules:

- For any physics metric, run OPM at the same dt as ResSim and at refined
  dt levels such as dt/4 and dt/16 before declaring OPM output to be the
  converged reference.
- Record `substeps`, `retries`, `Newton iterations`, `linear iterations`,
  `lin_ms`, `assembly_ms`, total wall time, and production metrics in the
  comparison log.
- Treat OPM default-dt output as a performance reference, not automatically
  as a converged physics reference.

## Phase 1: Scaling And CPR Bundle

Status: implementation machinery present; not complete or promoted.

Objective:

Implement the OPM-compatible linear-system basis as one bundle:

```text
J_scaled   = D_r * J * D_c
rhs_scaled = D_r * rhs
J_scaled * y = rhs_scaled
dx_physical = D_c * y
```

This must be paired with CPR pressure restriction built from the scaled local
cell blocks. Do not reintroduce OPM Quasi-IMPES weights on the raw Jacobian.

Implementation status:

- The row/column scaling machinery is implemented behind
  `FimLinearSolveOptions::opm_linear_scaling`.
- When enabled for the iterative backend, Newton builds the CPR
  preconditioner from the scaled matrix and recovers the physical update
  before applying damping and state updates.
- The option defaults to `false` because enabling it globally on the current
  `experiment/fim-amg-scirs2` solver state exceeds the locked SPE1 smoke
  runtime envelope. Promotion requires the acceptance gates below.
- Last focused checks:
  - `row_column_scaling_recovers_physical_solution_for_mixed_scale_system`
    passed.
  - `default_fim_linear_solver_targets_fgmres_cpr` passed and asserts the
    option defaults off.
  - `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas` passed.
  - `spe1_fim_first_steps_converge_without_stall` passed with a 300s guard
    but exceeded a 120s smoke guard on this branch.

Remaining Phase 1 work:

- Run the medium-water single-step and six-step shortlist with
  `opm_linear_scaling=true`.
- Run heavy-water `12x12x3` fine-dt FOPT against the converged OPM reference.
- Run gas-rate `10x10x3` and the SPE1 gas-injection smoke with the option
  enabled.
- Promote the option only if the acceptance gates below pass; otherwise keep
  it gated and continue with the next structural lever.

Implementation details:

- Extend the linear solve path so iterative FIM backends can receive both row
  and column scaling, solve the scaled system, and return a physical update.
- Use `EquationScaling` for `D_r` and `VariableScaling` for `D_c`.
- Build the local 3x3 cell block for pressure-transfer weights from
  `D_r * J * D_c`, not from raw `J`.
- Keep one coarse pressure unknown per cell pressure and one explicit coarse
  pressure unknown per physical well BHP.
- Keep perforation-rate unknowns outside the first coarse pressure slice; they
  may enter through the existing tail/Schur handling but must not become coarse
  pressure variables in Phase 1.
- Keep direct sparse/dense fallback behavior available for validation and
  emergency fallback.

Acceptance gates:

- Locked smoke tests remain green.
- Case-3 fine-dt FOPT stays within the current converged OPM tolerance.
- At least one hard case improves in substeps or retries, not only lin_ms.
- No result may be promoted if it reproduces the Phase A pattern of lower
  linear time but worse physics.

## Phase 2: Explicit Well Block Port

Objective:

Move from post-update well relaxation toward an explicit local well block
solve/post-solve flow modeled after OPM's reservoir-well block treatment.

Implementation details:

- Keep `FimWellLocalBlock` and `FimPerforationLocalBlock` as the public
  internal integration surface.
- Add a well post-solve recovery step after the global linear solve and before
  the bounded Newton update is accepted.
- Use local well block consistency as an algebraic recovery step, not only as
  relaxation after raw state mutation.
- Implement Schur-style well recovery only after Phase 1 scaling is stable.
- Do not broaden to OPM's full product surface: no Eclipse deck machinery,
  MPI/domain decomposition, group controls, THP, network wells, or
  multisegment wells in this project phase.

Acceptance gates:

- Existing well-control and SPE1/FIM tests remain green.
- Well BHP and perforation-rate updates remain finite and respect existing
  control bounds.
- Medium-water and gas-rate cases show fewer well-driven retries or neutral
  retry count with lower runtime.

## Phase 3: Nonlinear Update Policy

Objective:

Replace accumulated tactical hotspot behavior with a coherent OPM-style
Newton update policy while preserving ResSim's CNV plus material-balance
acceptance semantics.

Implementation details:

- Keep hotspot-specific bailout logic as a tactical fallback layer, not the
  primary globalization mechanism.
- Implement residual-history adaptive relaxation for repeated oscillation or
  weak progress.
- Keep bounded component updates for pressure, saturation, hydrocarbon
  variable, BHP, and perforation rates.
- Review the current inflection-point chop against OPM parity evidence before
  making it default behavior in the OPM-compatible path.
- Treat primary-variable switching and regime hysteresis as part of the same
  nonlinear policy, especially for undersaturated-to-saturated gas transition.
- Preserve final acceptance only when scaled residual and global material
  balance both satisfy their gates.

Acceptance gates:

- Fewer timestep cuts on the canonical water parity case.
- No gas-inventory loss around Rs/Rs_sat transitions.
- No acceptance of unchanged or post-classification-damaged states.

## Phase 4: AMG Revisit

Objective:

Revisit AMG only after Phases 1-3 create a scaled, well-aware, stable CPR
operator.

Implementation details:

- Do not treat scirs2 Ruge-Stueben defaults as OPM-equivalent.
- Prefer OPM-compatible smoother/coarsening behavior over isolated parameter
  tuning.
- Cache AMG hierarchy only within a safe Jacobian lifetime. Never reuse an AMG
  hierarchy across changed Jacobians.
- Measure WASM bundle size for any AMG dependency or in-house AMG code.
- Remove or feature-gate AMG paths that add bundle size without net benchmark
  wins.

Acceptance gates:

- AMG must improve an over-threshold pressure-system case after setup cost.
- AMG must not regress small cases where dense or BiCGStab coarse solve is
  cheaper.
- Any AMG promotion reports both linear time and total wall time.

## Test Plan

Locked smoke tests:

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas -- --nocapture
```

Benchmark gates:

- Medium-water single-step shortlist.
- Medium-water six-step shortlist.
- Heavy-water `12x12x3` with fine-dt FOPT versus converged OPM reference.
- Gas-rate `10x10x3` runtime, substeps, and retry count.
- WASM diagnostic replay through `scripts/fim-wasm-diagnostic.mjs`.

Promotion criteria:

- No accepted-step physics drift versus converged OPM references.
- No locked smoke regression.
- At least one hard case shows substep or retry reduction.
- Every performance claim reports `substeps`, `retries`, `lin_ms`,
  `assembly_ms`, and total wall time.

## Implementation Order

1. Land licensing metadata and this durable plan.
2. Return to clean mainline before solver implementation. Do not continue from
   the unpromoted `experiment/fim-amg-scirs2` solver state except for
   cherry-picked documentation or validated helper code.
3. Build Phase 0 comparison harness and refresh OPM reference outputs.
4. Implement Phase 1 scaling plus CPR as a single feature branch.
5. Only after Phase 1 passes, implement Phase 2 well post-solve recovery.
6. Only after Phase 2 passes, simplify and replace nonlinear update policy in
   Phase 3.
7. Only after Phases 1-3 pass, revisit AMG in Phase 4.

## Assumptions

- Direct OPM-derived implementation is allowed after GPL-compatible license
  metadata is in place.
- ResSim remains browser/WASM-first.
- OPM Flow remains an external reference executable/source during validation.
- The first successful target is a narrower OPM-compatible simplified
  black-oil kernel, not full OPM product parity.
