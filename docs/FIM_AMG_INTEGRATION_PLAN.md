# Fix 4: AMG Coarse Solver Integration Plan

**Audit reference:** `docs/FIM_LINEAR_SOLVER_AUDIT.md` "Fix 4 (MEDIUM)".
**Blocking:** Bundle B (CPR + helpers) — refer to
`docs/FIM_CPR_BUNDLE_BISECTION.md` and
`project_fim_cpr_bundle_bisection_2026-05-01.md`.
**Date:** 2026-05-01.

## Why now

The four damping/chop experiments and the Bundle B CPR experiment
all hit the same wall: ressim's CPR with **BiCGStab + ILU(0)** as the
coarse solver fails for ~50% of medium-water case 2 iters and is
materially slower on three-phase / undersaturated cases (case 4
+380% lin_ms with summed-IMPES restriction). The case-3 / case-4
regressions in the bundle are structural — they trace to the
coarse solver's spectrum, not the Newton kernel or restriction
operator.

**OPM Flow's `cprw` uses smoothed-aggregation AMG (Vanek-Mandel-
Brezina) with ILU(0) smoother as its coarse solver.** This is what
makes summed-IMPES restriction safe across all cases and what
gives OPM the 7-substep / 0.07s case-2 result we still trail by
~4× substeps and ~700× wall time.

Replacing BiCGStab+ILU(0) with AMG is the single largest
remaining gap to OPM. After this lands, the entire Bundle B work
becomes re-evaluable and likely promotable on all cases
simultaneously.

## What ressim has today

`src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs`:
- `BlockJacobiPreconditioner::pressure_coarse_solver_kind` returns
  `ExactDense` (n ≤ 512) or `BiCgStab` (n > 512).
- `solve_pressure_correction(rhs)` dispatches: dense inverse for
  small, `solve_pressure_with_bicgstab` (BiCGStab + sequential
  ILU0 application) for large.
- `pressure_rows`, `pressure_l_rows`, `pressure_u_rows`,
  `pressure_u_diag` carry the coarse system's L, U factors plus
  CSR-style coarse `A`.

The integration point for AMG is a third arm in
`solve_pressure_correction`: call AMG's solve when the coarse
matrix is "large" or "ill-conditioned enough" that ILU(0) isn't
enough. Initial heuristic: AMG when `n > 512`.

## Path A: scirs2-sparse SmoothedAggregationAMG

**Source:** Apache-2.0 licensed pure-Rust crate published April
2026 by cool-japan/scirs2. Provides
`scirs2_sparse::linalg::algebraic_multigrid::SmoothedAggregationAMG`
with `setup(&A, opts) → solve(&b, tol, max_iters) → x`.

**Pros:**
- Drop-in-compatible API; ~1 day spike to wire up.
- Pure Rust, no FFI, builds for `wasm32-unknown-unknown`.
- Apache-2.0 license (compatible with current crate licensing).

**Cons:**
- v0.4.x is dated April 2026 — alpha-quality reference port, not
  battle-tested on reservoir pressure systems.
- API churn risk between minor versions.
- Uses scirs2's own CSR (`CsrArray`); needs ~30-line shim from
  `sprs::CsMat`.
- Defaults likely tuned for elliptic Laplacian benchmarks, not
  anisotropic well-driven reservoir pressure systems.

**Path A scope (1-day spike):**
1. Add `scirs2-sparse = "0.4"` (or pinned version) to
   `src/lib/ressim/Cargo.toml` behind a feature flag if needed.
2. Write `sprs::CsMat<f64> → scirs2 CsrArray<f64>` adapter.
3. Add `Amg` variant to `FimPressureCoarseSolverKind`.
4. Extend `solve_pressure_correction` to dispatch to AMG when
   `n > 512` (replace the BiCGStab path) and fall through to
   BiCGStab if AMG fails or diverges.
5. Run 4-case shortlist + Buckley/Dietz/SPE1/appleyard tests on
   master baseline and on AMG-enabled branch.
6. Run with the Bundle B summed-IMPES restriction (cherry-pick
   from `experiment/fim-cpr-summed-impes` HEAD `db06fba` —
   `gmres_block_jacobi.rs` restriction change only) to measure
   the combined "OPM-aligned CPR + AMG" wins.

**Decision criteria for promoting Path A:**
- Case 2 lin_ms drops at least 30% vs current master (case-2 was
  the win source in Bundle B).
- Case 3 lin_ms within 10% of master (the regression we couldn't
  close in Bundle B).
- Case 4 lin_ms within 20% of master (the structural-cost case).
- All 298 Rust tests still pass; no new failures.
- Case 3 fine-dt FOPT within 1% of OPM converged ref (3826).

If those hold, promote. If case 3 / case 4 still regress, scirs2's
SA defaults aren't right for our pressure systems and Path B is
needed.

## Path B: from-scratch SA-AMG port

If Path A fails, port pyAMG's smoothed-aggregation into a new
`src/lib/ressim/src/fim/linear/amg/` module. ~600-900 LOC for the
core SA + Gauss-Seidel smoother variant; +200 LOC if we also port
ILU(0)-as-smoother to match OPM's `cprw` exactly.

Files to port (from `pyamg/aggregation/aggregation.py` and
`pyamg/amg_core/smoothed_aggregation.h`):

- `strength.rs`: symmetric strength-of-connection (~80 LOC). Test:
  produces sparse subset of A with magnitude-based threshold.
- `aggregation.rs`: standard greedy multi-pass aggregation
  (~120 LOC). Test: every node gets exactly one aggregate.
- `tentative.rs`: fit-candidates for constant near-nullspace
  (~60 LOC). Test: tentative prolongator T satisfies T^T·1 = 1.
- `smooth.rs`: Jacobi-smoothed prolongator
  `P = (I - ω D⁻¹ A) T` (~40 LOC). Test: P has correct sparsity.
- `galerkin.rs`: Galerkin product `A_c = Pᵀ A P` (~10 LOC; sprs
  has built-in sparse mat-mat-mat).
- `smoother.rs`: forward/backward Gauss-Seidel on CSR (~40 LOC).
  Test: applied to symmetric A, residual reduces.
- `cycle.rs`: recursive V-cycle, coarsest-level direct solve
  (~80 LOC). Test: 2-grid V-cycle on 100×100 Laplacian converges
  in O(1) iters.
- `solver.rs`: top-level `setup(A) → SaAmg`, `solve(b, tol,
  max_iters) → x` (~80 LOC). Plug-compatible with the dispatch
  in `solve_pressure_correction`.

**Path B scope:** 5-7 days for core implementation + 2-3 days
for testing + tuning on the 4-case shortlist.

## Phasing

**Phase 0 (DONE):** Research, document this plan.

**Phase 1 (next):** Path A spike on branch
`experiment/fim-amg-scirs2`. Time-box: 1 day. Deliverable: 4-case
shortlist measurements (with and without summed-IMPES restriction
combined). Decision: promote / pivot to Path B / shelve.

**Phase 2 (if Phase 1 promotes):** Combine with cherry-picked
summed-IMPES restriction from `experiment/fim-cpr-summed-impes`
(commit `db06fba`, file `linear/gmres_block_jacobi.rs` only —
NOT the Newton-side bundle helpers, which were compensators).
Run 4-case + fine-dt physics + Buckley/Dietz/SPE1. Promote if
all green.

**Phase 3 (if Phase 1 fails):** Path B port. Use pyAMG's
smoothed-aggregation as reference. Same validation gates. Promote
if all green.

## Validation matrix

For each phase, every promotion candidate must:

| Test | Pass criterion |
|------|---------------|
| `cargo test --lib appleyard` | 3/3 pass |
| `cargo test --lib benchmark_buckley` | 3/3 pass |
| `cargo test --lib dep_pss` | 4/4 pass |
| `cargo test --lib spe1_fim` | 3/3 pass |
| `cargo test --lib pressure_transfer` | all pass |
| 4-case shortlist substeps | no case +20% vs master |
| 4-case shortlist lin_ms | total −10% or better vs master |
| Case 3 fine-dt FOPT | within 1% of OPM converged ref (3826) |
| Case 2 fine-dt FOPT | within 1% of master fine-dt (3609.73) |

## Files of record (when work begins)

- This document.
- `experiment/fim-amg-scirs2` branch (Phase 1).
- `experiment/fim-amg-from-scratch` branch (Phase 3 if needed).
- New memory: `project_fim_amg_phase_1_<date>.md` after Phase 1
  decision.

## Risk register

1. **scirs2-sparse alpha quality.** Mitigation: time-boxed 1-day
   spike; abandon promptly if convergence is unstable.
2. **AMG hyperparameters need tuning per problem class.** Even
   with a working library, defaults might not be optimal for
   reservoir pressure systems. Mitigation: parameter sweep
   (smoothing ω ∈ {2/3, 4/3, 1.6}, max_levels ∈ {3, 5, 8},
   coarsest_n ∈ {50, 200, 500}) on case 2.
3. **Path B effort overrun.** Mitigation: scope-bounded —
   Gauss-Seidel smoother first, ILU0-smoother only if GS is
   insufficient. Skip Ruge-Stüben coarsening; SA only.
4. **WASM bundle size.** scirs2-sparse is 94k SLoC; pulling it in
   may inflate `simulator_bg.wasm` significantly. Mitigation:
   measure post-build size delta as part of Phase 1; consider
   feature-flagging AMG to non-WASM only if size is prohibitive.
