# OPM-Compatible FIM Reimplementation Continuation Plan

Date: 2026-05-18
Status: canonical continuation plan for clean `master`; no solver behavior is
promoted by this document alone.

## Summary

ResSim's FIM convergence work should continue from clean `master`, not from
`experiment/fim-amg-scirs2`. That experiment branch is evidence only. Do not
promote its refuted solver behavior, OPM Quasi-IMPES weights applied to the raw
ResSim Jacobian, scirs2 AMG dependency, generated WASM, or dependency churn.

The strategic rule is: no more isolated OPM copies. OPM Flow's black-oil
robustness comes from a coupled numerical contract. Any OPM-derived solver work
must move the matching pieces together: scaling, pressure restriction, well
coupling, nonlinear update policy, and validation against OPM references that
have been checked by timestep refinement.

This document is the canonical roadmap for that continuation.

## Current Baseline

Baseline branch:

- Start implementation work from `master`.
- Treat `experiment/fim-amg-scirs2` and related experiment branches as
  research records unless a later clean-master slice independently passes the
  validation matrix.

Promoted master context to preserve:

- Iterative-failure short-circuit from the bypass-audit work.
- Zero-move stagnation attribution and the fix that prevents hotspot
  zero-move iterations from consuming the normal stagnation budget.
- `FW_INFLECTION_OVERSHOOT_FACTOR = 1.2`, which keeps the
  Wang-Tchelepi-style inflection guard but avoids over-chopping marginal
  front crossings.
- Diagnostic traces such as STAG-TREND, DAMP-BREAKDOWN, basin/upwind probes,
  and per-rung timing summaries.

Non-promotable evidence:

- Standalone global or per-cell Newton extrapolation.
- Dt-aware replay tolerance without relaxing the exact-zero replay gate.
- Water cross-step carryover.
- Intra-rung Jacobian reuse.
- Broad stagnation-gate widening.
- Standalone row/column scaling.
- OPM Quasi-IMPES weights applied to ResSim's raw Jacobian.
- scirs2 AMG Path A as a promoted default.

Lesson from the failed OPM-copy attempts:

- OPM's restriction weights only make sense in OPM's scaling and assembly
  basis. Applying one OPM component to ResSim's current raw-Jacobian basis can
  lower linear time while worsening physics. That pattern must never be used
  as a promotion signal.

## Phase 0: Canonical Baseline And References

Status: licensing, harness scaffolding, tracked OPM parity decks, and
same/dt4/dt16 OPM reference tables are present for the Phase-0 water/gas
parity cases.
Metric-specific promotion tolerances still need to be stated before any ResSim
solver change can claim physics equivalence.

Implementation:

- Keep root `LICENSE` as GPLv3 text.
- Add `GPL-3.0-or-later` license metadata to:
  - root npm package metadata,
  - root package-lock package entry,
  - Rust crate metadata in `src/lib/ressim/Cargo.toml`.
- Keep root `NOTICE` explaining that any direct translated OPM material must
  retain source-file and release/commit attribution.
- Add `scripts/opm-ressim-compare.sh` as the canonical comparison harness.
- Expose the harness through `npm run compare:opm`.
- Keep Phase-0 OPM parity decks under `opm/reference-decks/` so deck inputs
  are reviewable and versioned.
- Keep same/dt4/dt16 OPM summary tables in
  `docs/opm-reference-results/phase0-same-dt.md` as the Phase-0 reference
  record.

Reference harness behavior:

- ResSim diagnostics run through `scripts/fim-wasm-diagnostic.mjs`.
- OPM Flow runs only when a matching deck exists and `FLOW_BIN` points to a
  runnable Flow executable.
- The harness copies each OPM deck into the comparison output directory before
  invoking Flow, keeping generated `.PRT`, `.UNSMRY`, restart, and grid files
  out of the tracked deck tree.
- Missing decks are recorded as `missing-deck`, not treated as harness
  failures.
- Default output goes under `worklog/opm-ressim-compare/<utc-stamp>/`.

Remaining Phase 0 work:

- Define metric-specific tolerances for dt/16-vs-ResSim comparisons:
  - field pressure (`FPR` versus ResSim average reservoir pressure),
  - cumulative oil production (`FOPT`),
  - cumulative injection (`FWIT`/`FGIT`),
  - gas production/inventory metrics for gas-rate cases.
- Keep existing heavy-water deck wiring for:
  - `heavy-water-12x12x3`,
  - `heavy-water-finedt`.
- Promote the historical heavy-water decks from `worklog/opm-case3/` into the
  tracked reference-deck tree or regenerate them there.
- For every OPM physics metric used as a reference, record same-dt, dt/4, and
  dt/16 runs before declaring the OPM value converged.

Reference rules:

- Treat OPM default-dt output as a performance reference, not automatically as
  a converged physics reference.
- Store dt-refinement tables next to the metric being compared.
- Record `substeps`, `retries`, Newton iterations, linear iterations,
  `lin_ms`, `assembly_ms`, total wall time, and production/pressure metrics.

## Phase 1: Scaling Plus Restriction Bundle

Objective:

Build the OPM-compatible linear-system basis as one gated bundle:

```text
J_scaled   = D_r * J * D_c
rhs_scaled = D_r * rhs
J_scaled * y = rhs_scaled
dx_physical = D_c * y
```

Then build CPR pressure restriction from the same scaled local cell blocks.
Do not enable diagonal scaling by itself and do not reintroduce OPM
Quasi-IMPES weights on the raw Jacobian.

Implementation requirements:

- Use `EquationScaling` for row factors and `VariableScaling` for column
  factors.
- Build local 3x3 cell pressure-transfer weights from `D_r * J * D_c`.
- Recover physical updates before Appleyard damping and state mutation.
- Keep the path behind a clearly named internal option until the full
  validation matrix passes.
- Keep existing direct sparse/dense fallbacks available for validation and
  emergency fallback.
- Keep one coarse pressure unknown per cell pressure and one explicit coarse
  pressure unknown per physical well BHP.
- Keep perforation-rate unknowns outside the first coarse pressure slice; they
  may participate only through the existing tail/Schur handling in this phase.

Acceptance gates:

- Locked Rust smoke tests stay green.
- Case-3 fine-dt FOPT stays within the dt-refined OPM reference tolerance.
- At least one hard case improves in substeps, retries, or total wall time.
- A candidate that improves linear time but worsens physics is rejected.

## Phase 2: CPRW Well Block Path

Objective:

Move from post-update well relaxation toward an explicit local well block
solve/post-solve flow modeled after OPM's reservoir-well block treatment.

Implementation requirements:

- Use existing `FimWellLocalBlock` and `FimPerforationLocalBlock` as the
  internal integration surface.
- Add explicit well-BHP participation in the coarse pressure system.
- Add a well post-solve recovery step after the global linear solve and before
  bounded Newton update acceptance.
- Treat local well block consistency as an algebraic recovery step, not only
  as relaxation after raw state mutation.
- Implement Schur-style well recovery only after the Phase 1 scaled CPR basis
  is stable.

Out of scope:

- Eclipse deck machinery.
- MPI/domain decomposition.
- Group controls, THP, network wells, and multisegment wells.

Acceptance gates:

- Existing well-control and SPE1/FIM tests remain green.
- Well BHP and perforation-rate updates remain finite and respect existing
  control bounds.
- Medium-water and gas-rate cases show fewer well-driven retries or neutral
  retry count with lower total runtime.

## Phase 3: Nonlinear Update Policy

Objective:

Replace accumulated tactical hotspot behavior with a coherent OPM-style Newton
update policy after Phases 1-2 create a stable scaled and well-aware linear
operator.

Implementation requirements:

- Preserve ResSim's CNV plus material-balance acceptance semantics.
- Keep current hotspot bailouts as tactical fallback, not as the primary
  globalization mechanism.
- Review primary-variable switching and regime hysteresis together,
  especially undersaturated-to-saturated gas transitions.
- Keep bounded component updates for pressure, saturation, hydrocarbon
  variable, BHP, and perforation rates.
- Keep the inflection chop at `k=1.2` until scaled CPR evidence shows it can
  be relaxed without case-3 physics drift.
- Do not retry previously refuted global/per-cell extrapolation or broad
  stagnation widening without a new diagnostic signal.

Acceptance gates:

- Fewer timestep cuts on the canonical water parity case.
- No gas-inventory loss around Rs/Rs_sat transitions.
- No acceptance of unchanged or post-classification-damaged states.
- Fine-dt physics remains aligned with dt-refined OPM references.

## Phase 4: AMG Revisit

Objective:

Revisit AMG only after the scaled, CPRW-aware pressure operator is stable.

Implementation requirements:

- Do not treat scirs2 defaults as OPM-equivalent.
- Prefer OPM-compatible smoother/coarsening behavior over isolated parameter
  tuning.
- Cache AMG hierarchy only within a safe Jacobian lifetime; never reuse a
  hierarchy across changed Jacobians.
- Measure WASM bundle size, setup cost, linear time, and total wall time.
- Remove or feature-gate AMG paths that add bundle size without net benchmark
  wins.

Acceptance gates:

- AMG improves an over-threshold pressure-system case after setup cost.
- AMG does not regress small cases where dense or BiCGSTAB coarse solve is
  cheaper.
- AMG does not reproduce the scirs2 Path A pattern of small local wins with
  no net promotable bundle outcome.

## Validation Matrix

Locked Rust smoke tests:

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas -- --nocapture
```

WASM/OPM shortlist:

- `water-pressure 20x20x3 --steps 1 --dt 0.25`
- `water-pressure 20x20x3 --steps 6 --dt 0.25`
- `water-pressure 12x12x3 --steps 1 --dt 1`
- `water-pressure 12x12x3 --steps 16 --dt 0.0625`
- `gas-rate 10x10x3 --steps 6 --dt 0.25`
- `water-pressure 22x22x1 --steps 1 --dt 0.25`
- `water-pressure 23x23x1 --steps 1 --dt 0.25`

Promotion criteria:

- No locked-smoke regression.
- No fine-dt physics drift versus dt-refined OPM references.
- At least one hard case improves in substeps, retries, or total runtime.
- Every performance claim reports substeps, retry classes, linear time,
  assembly time, total wall time, and production/pressure metrics.
- Any solver behavior that only improves local linear metrics while degrading
  physics is rejected.

## Implementation Order

1. Land this plan, GPL metadata, `NOTICE`, and the comparison harness on clean
   `master`.
2. Generate the missing OPM parity decks and record dt-refinement tables.
3. Implement Phase 1 scaling plus restriction as a single clean-master branch.
4. Implement Phase 2 CPRW well handling only after Phase 1 has either passed
   or been deliberately kept gated with a documented reason.
5. Implement Phase 3 nonlinear policy only after the linear/well basis is
   stable enough that Newton-direction quality is no longer dominated by known
   CPR gaps.
6. Revisit AMG only after Phases 1-3 clarify the remaining pressure-stage
   bottleneck.

## Assumptions

- Direct OPM-derived implementation is allowed only after GPL-compatible
  metadata and attribution are present.
- ResSim remains browser/WASM-first.
- OPM Flow remains an external reference executable/source during validation.
- The target is a simplified Cartesian-grid black-oil kernel, not full OPM
  product parity.
- The experiment branch remains a research record unless a later clean-master
  slice independently passes this plan's validation matrix.
