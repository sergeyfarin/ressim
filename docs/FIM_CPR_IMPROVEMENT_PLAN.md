FIM CPR Preconditioner Improvement Plan

## Diagnosis: Where the 10–20 FGMRES Iterations Come From

For every current benchmark case (SPE1 = 300 cells, 12×12×3 = 432 cells), the coarse
pressure system is ≤512 rows → the dense exact inverse path is taken
(`pressure_dense_inverse`, `PRESSURE_DIRECT_SOLVE_ROW_THRESHOLD = 512`).
The worklog confirms the coarse stage already reduces to ~1e-13.

The 10–20 FGMRES iterations are caused by the **fine-level smoother**, not the coarse
solve. The block-Jacobi smoother (`apply_stage_one`, `gmres_block_jacobi.rs:77`) processes
each 3×3 cell block independently. After the CPR stage eliminates pressure-space error,
the remaining saturation/transport error couples cells through inter-cell flux Jacobian
terms — a pattern that block-Jacobi cannot see. Each FGMRES iteration then has to span
that coupled error space explicitly.

OPM's ~1.4 linear iterations/Newton comes from a fine-level ILU (or block-ILU) smoother
that, combined with the coarse pressure correction, reduces the residual by ~10× per
application.

**Three improvements, in implementation order:**

| Phase | Target | Current test impact | Scale-up impact |
|-------|--------|---------------------|-----------------|
| 1 | Add selectable CPR fine smoother and implement full-system ILU(0) | High — most likely lever on current 10–20 iter cases | High |
| 2 | Fix coarse ILU(0) behavior for >512-row systems | None on current benchmark-sized shelves | Required |
| 3 | Consider AMG only after coarse ILU/BiCGSTAB is characterized | None | Possible future scale-up path |

## Current Status - 2026-04-08

- Phase 1 is implemented and parked. The full-system ILU(0) CPR fine smoother is in place and
  test-green, but it is not being promoted yet as a clean runtime win because the shipped gas
  replay did not improve.
- Phase 2 is now implemented on current head. The non-dense coarse path reports
  `solver=bicgstab` and the bounded `23x23x1` probe shows much stronger coarse-stage reduction
  ratios than the old ILU defect-correction path.
- Bounded over-threshold follow-up now also closed the first Krylov-plateau diagnosis. One real
  GMRES issue was discarding restart-boundary candidate progress unless convergence or breakdown
  happened inside the restart. After fixing that and replaying the same `23x23x1` bounded case,
  the shelf shape stayed the same (`substeps=8`, `retries=0/3/0`) but fallback burden dropped
  again, from `46` rejected CPR solves / `56` dense-LU uses to `28` rejected CPR solves /
  `36` dense-LU uses.
- A second bounded Krylov-tail fix is now also in on current head. The restarted GMRES path was
  still treating an over-optimistic preconditioned residual estimate as an immediate restart
  trigger even when the true candidate residual disagreed and Krylov space could still grow.
  Keeping that restart alive reduced the same `23x23x1` bounded replay further, from
  `28` rejected CPR solves / `36` dense-LU uses to `15` rejected CPR solves /
  `21` dense-LU uses, again with the same bounded shelf shape (`substeps=8`, `retries=0/3/0`).
- A third bounded tail slice is now in as well. Tiny-residual tails that have already reached the
  post-improvement asymptote no longer spend the full five restart windows proving that the current
  iterate cannot be improved. On the same `23x23x1` bounded replay this reduced fallback burden one
  more step, from `15` rejected CPR solves / `21` dense-LU uses to `14` rejected CPR solves /
  `20` dense-LU uses, still with the same bounded shelf shape (`substeps=8`, `retries=0/3/0`).
- The bounded threshold pair still lands on the same shelf shape, though:
  `22x22x1` (exact-dense control) and `23x23x1` (over-threshold probe) both end at
  `substeps=8`, `retries=0/3/0`.
- Current interpretation: Phase 2 improved coarse solve quality, but not the overall bounded
  step outcome, because Newton still falls back to `dense-lu` on the full system almost
  immediately after the first CPR-backed iteration on each retry rung. The next slice inside
  Phase 2 should therefore target iterative-backend persistence / fallback-burden reduction,
  not another coarse residual-quality change by itself.
- Updated interpretation after the bounded Krylov diagnosis: the remaining plateau is no longer a
  missing-restart-update problem. Current restart traces now show 2–3 strongly improving restarts,
  followed by a short-restart asymptotic tail where `outer_res`, `prec_res`, and `est_res` are
  already tiny but the restart candidate stops improving (`upd=n`) and can come back worse than the
  current iterate. The next Phase 2 slice should therefore target this post-coarse asymptotic
  Krylov tail, not coarse-pressure quality and not restart bookkeeping.
- Updated interpretation after the latest tail fix: the old false-estimate restart trigger is no
  longer the dominant plateau source either. Remaining `23x23x1` failures now split into a much
  smaller tiny-residual tail, where the first 2–3 restarts improve strongly before later restart
  candidates stall or worsen, plus a smaller set of genuinely hard nonlinear states where restart 1
  never improves at all (`upd=n` from the first cycle). The next Phase 2 slice should therefore be
  one of two bounded choices only: tighten acceptance/termination around the tiny-residual tail, or
  detect and bypass the clearly non-improving hard states earlier instead of spending all five
  restart cycles.
- Updated interpretation after the current termination tweak: the visible new win comes mainly from
  early `restart-stagnation` exit on the tiny tail, not from a broad acceptance expansion. Several
  former `max-iters` tails now stop after restart 3 or 4 (`68-103` total iterations in the bounded
  replay) once the current iterate is already tiny and later candidates are only worse. The clearly
  hard states remain unchanged: they still show `upd=n` from restart 1 and still spend the full
  `150`-iteration budget as `max-iters`. The next bounded Phase 2 slice should therefore move off
  the tiny tail and focus on those immediately non-improving hard states.
- Updated interpretation after the dead-state detector: the bounded `23x23x1` shelf count stayed the
  same (`14` rejected CPR solves / `20` dense-LU uses), but the remaining hard-state family no
  longer spends the full Krylov budget proving failure. The `upd=n` from restart 1 cases now stop
  after the first full restart as `reason=dead-state` at `30` iterations instead of running out to
  `150` iterations as `max-iters`. That means the remaining over-threshold issue is no longer wasted
  tail work and no longer wasted dead-state proof work; it is the fallback burden itself.
- Updated interpretation after the Newton-side dead-state bypass: the bounded fallback count is still
  unchanged (`20` dense-LU uses), but repeated futile CPR attempts inside the same substep are now
  suppressed after the first proven dead-state. On the same `23x23x1` replay, rejected CPR solves
  dropped from `14` to `9` while dense fallback stayed flat at `20`. The remaining issue is now
  narrower again: not repeated CPR rediscovery, but the fact that those hard states still need the
  direct solve once they are detected.
- First direct-fallback cleanup is now in on the wasm path. The dense fallback no longer clones the
  assembled dense matrix before LU and no longer uses a dense matrix-vector multiply just to check
  the residual; it now reuses the sparse Jacobian for residual evaluation. The bounded `23x23x1`
  replay stayed behavior-identical (`9` rejected CPR solves / `20` dense-LU uses, same
  `substeps=8`, `retries=0/3/0`), so this landed as a semantics-preserving direct-path cost cut.
- Large-row wasm direct-fallback A/B is now also complete. Temporarily restoring dense LU on the
  same bounded `23x23x1` replay kept the earlier behavior class (`9` rejected CPR solves /
  `20` dense-LU uses, still `substeps=8`, `retries=0/3/0`) but took about `10866.6 ms` outer /
  `10764.0 ms` linear. Switching the large-row wasm direct path to sparse LU changed the bounded
  counts only slightly (`11` rejected CPR solves / `22` sparse-LU uses, same shelf shape) while
  cutting runtime to about `1326.5 ms` outer / `1219.0 ms` linear. The selector should therefore
  stay on sparse LU for large wasm direct fallbacks; the next Phase 2 slice should reduce fallback
  incidence, not restore dense LU on those hard states.
- Sparse-LU refinement follow-up is now also in on that same direct path. Adding a short
  iterative-refinement loop to the large-row sparse backend kept the same bounded replay counts on
  `23x23x1` (`11` rejected CPR solves / `22` sparse-LU uses / `5` dead-state bypasses, same
  `substeps=8`, `retries=0/3/0`) but improved runtime again to about `1289.6 ms` outer /
  `1182.0 ms` linear. Keep this as a second direct-path cost cut, but do not mistake it for the
  next convergence lever: the active Phase 2 question is still how to reduce fallback incidence,
  not how to swap or polish the direct backend further.
- Repeated-`restart-stagnation` bypass is now also in at the Newton layer. After two consecutive
  iterative failures with `reason=restart-stagnation` inside the same substep, later Newton
  iterations now bypass CPR and go straight to the row-selected direct backend for the rest of that
  substep. On the same bounded `23x23x1` replay this fired twice, cut rejected CPR solves from
  `11` down to `9`, kept sparse direct fallback uses flat at `22`, preserved the same bounded
  shelf (`substeps=8`, `retries=0/3/0`), and improved runtime again to about `1174.6 ms` outer /
  `1074.0 ms` linear. Keep it as another bounded rediscovery cut. The active Phase 2 question is
  now narrower again: not repeated dead-state proof, not repeated restart-stagnation proof, and
  not direct-backend cost, but the remaining `22` direct fallback uses themselves.
- Zero-move fallback bypass is now also in and is the new current head. If a fallback-backed Newton
  iteration produces only an effectively zero state move, using the existing effective-move floor
  (`<5e-3 bar`, `<5e-5` saturation), the next Newton iteration now bypasses CPR and goes straight
  to the row-selected direct backend instead of rerunning the same iterative solve on the same
  unchanged state. Two confirming `23x23x1` replays now agree on the new bounded control counts:
  `6` rejected CPR solves, `19` sparse direct fallbacks, `5` dead-state bypasses, and `2`
  zero-move bypasses, with the same bounded shelf (`substeps=8`, `retries=0/3/0`). Runtime stays
  in the same improved band (`outer_ms≈1188-1257`, `lin_ms≈1091-1152`). Keep it. The active Phase
  2 question is now narrower again: not repeated dead-state proof, not repeated restart-stagnation
  proof, not unchanged-state fallback rediscovery, and not direct-backend cost, but the remaining
  `19` direct fallback uses themselves.
- Near-converged iterative accept is now also in and is the new current head. If the CPR solve
  stops in a small-residual `restart-stagnation` or `max-iters` tail but is still close enough to
  tolerance (`outer_res <= 16x tol`, candidate residual no worse than `8x` the current iterate,
  and at least one restart improved the iterate), Newton now accepts that iterative step instead
  of paying for a direct fallback. Two confirming `23x23x1` replays now agree on the new bounded
  control counts: `6` rejected CPR solves, `18` sparse direct fallbacks, `5` dead-state bypasses,
  `2` zero-move bypasses, and `1` near-converged iterative accept, with the same bounded shelf
  (`substeps=8`, `retries=0/3/0`). Runtime stays in the same improved band
  (`outer_ms≈1209-1231`, `lin_ms≈1106-1121`). Keep it. The active Phase 2 question is now even
  narrower: not dead-state rediscovery, not restart-stagnation rediscovery, not unchanged-state
  rediscovery, not small-residual near-converged tails, and not direct-backend cost, but the
  remaining `18` direct fallback uses themselves.

## Guardrails Before Any Code Change

The current draft needs three corrections before implementation:

1. **Do not change `GmresIlu0` semantics accidentally.** Both `FgmresCpr` and `GmresIlu0`
  currently share the same preconditioner builder and stage-one apply path. Replacing
  `apply_stage_one` unconditionally would silently redefine the separate non-CPR iterative
  backend instead of only changing CPR's fine smoother.

2. **Do not add a redundant coarse-row sort pass.** `pressure_rows` is already assembled
  from `BTreeMap` accumulators, so each row is emitted in sorted column order today.
  Replacing the linear `row_entry` lookup with binary search is still valid, but the extra
  sort step is unnecessary.

3. **Do not treat Phase 3 AMG as ready-to-implement.** The current coarse operator includes
  explicit well-BHP rows in addition to cell-pressure rows. Any AMG design must define how
  those well rows are handled; a naive cell-style aggregation over all coarse rows is not a
  safe default.

---

## Phase 1: Selectable CPR Fine Smoother + Full-System ILU(0)

**Goal**: Add an explicit CPR fine-smoother selection point, keep the existing
block-Jacobi path as the baseline, and add full-system ILU(0) as the new CPR-only
fine smoother. This is the highest-leverage change because it targets the bottleneck on
all current benchmark-sized cases while preserving a clean A/B path.

**Technical basis**: The Jacobian is already `sprs::CsMat<f64>` (`FimAssembly.jacobian`,
`assembly.rs:19`). The existing `factorize_pressure_ilu0` (`gmres_block_jacobi.rs:300`)
already implements ILU(0) on `Vec<Vec<(usize, f64)>>`. The infrastructure exists — it
needs to be adapted to the full system and wired into the CPR preconditioner without
changing the separate `GmresIlu0` backend contract.

### Step 1.0 — Add explicit smoother selection

Introduce an internal enum in `gmres_block_jacobi.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CprFineSmootherKind {
    BlockJacobi,
    FullIlu0,
}
```

Use it to make the baseline and experimental paths explicit:

- `FimLinearSolverKind::FgmresCpr` selects a CPR preconditioner build with an explicit
  smoother choice.
- `FimLinearSolverKind::GmresIlu0` keeps its current meaning: no pressure-correction stage,
  no silent switch to CPR full-system ILU.
- During rollout, keep both CPR smoother paths buildable so tests and wasm diagnostics can
  compare `block-jacobi` versus `ilu0` on the same solver family.

This can stay internal to `gmres_block_jacobi.rs`; no public UI or runtime config is needed.

### Step 1.1 — Add `FimIlu0Factors` struct

Add to `gmres_block_jacobi.rs`:

```rust
struct FimIlu0Factors {
    n: usize,
    l_rows: Vec<Vec<(usize, f64)>>,   // strictly lower triangular
    u_diag: Vec<f64>,                  // diagonal of U
    u_rows: Vec<Vec<(usize, f64)>>,   // strictly upper triangular
}

impl FimIlu0Factors {
    fn apply(&self, rhs: &DVector<f64>) -> DVector<f64> {
        // forward substitution (L), then backward substitution (U)
        // identical algorithm to apply_pressure_ilu (lines 265–289)
    }
}
```

### Step 1.2 — Add `factorize_full_ilu0(mat: &CsMat<f64>) -> Option<FimIlu0Factors>`

Write a companion function to `factorize_pressure_ilu0` that takes `sprs::CsMat<f64>`
directly. sprs provides `mat.outer_iterator()` (row iteration) and sorted column indices
per row, eliminating the O(nnz) linear scan of `row_entry` (lines 292–298). Replace with
direct index lookup since sprs guarantees sorted columns.

Key differences from the coarse ILU:
- Input is `&CsMat<f64>`, not `Vec<Vec<(usize, f64)>>`
- Use `row.iter()` from sprs for (col, val) pairs — already sorted
- Nonzero lookup during elimination: `row.get(col)` → O(log nnz_per_row)
- Handle near-zero diagonal with the same fallback as `factorize_pressure_ilu0`
- Return `None` if `mat.rows() > FULL_ILU_ROW_LIMIT` (constant, e.g. 4096) or if
  factorization encounters a singular diagonal — falls back to block-Jacobi

### Step 1.3 — Extend the CPR preconditioner to hold ILU factors and the selected smoother

Add one field to the struct (line 17):

```rust
full_ilu: Option<FimIlu0Factors>,   // None → fall back to block-Jacobi
fine_smoother_kind: CprFineSmootherKind,
```

Update `build_block_jacobi_preconditioner(...)` to take the selected smoother kind.
Build the ILU factors only when that kind requests them:

```rust
let full_ilu = match fine_smoother_kind {
  CprFineSmootherKind::BlockJacobi => None,
  CprFineSmootherKind::FullIlu0 => factorize_full_ilu0(matrix),
};
```

This preserves a clean baseline path and prevents `GmresIlu0` from being redefined by
implementation accident.

### Step 1.4 — Make stage one dispatch on the selected smoother

Change `apply_stage_one` (lines 77–98):

```rust
fn apply_stage_one(&self, vector: &DVector<f64>, result: &mut DVector<f64>) {
  match self.fine_smoother_kind {
    CprFineSmootherKind::FullIlu0 => {
      if let Some(ilu) = &self.full_ilu {
        *result = ilu.apply(vector);
        return;
      }
    }
    CprFineSmootherKind::BlockJacobi => {}
    }
    // existing block-Jacobi path unchanged — kept as fallback
    for (block_idx, inverse) in self.cell_block_inverses.iter().enumerate() { ... }
    for (tail_idx, inv_diag) in self.scalar_inv_diag.iter().enumerate() { ... }
}
```

The ILU apply is one forward + one backward substitution, O(nnz) total. For a 1300-row
system with ~20 nonzeros/row, this is ~26k multiplications — extremely cheap per
FGMRES inner iteration.

### Step 1.5 — Add smoother label to CPR diagnostics

Extend `FimCprDiagnostics` with `smoother_label: &'static str` (`"ilu0"` or
`"block-jacobi"`). Report in the existing `cpr=[...]` wasm trace line so the diagnostic
output shows which smoother path is active without requiring a test rebuild.

This is required because current timestep classification already consumes CPR diagnostics;
the new label must be additive and must not disturb the existing `coarse_solver`,
`average_reduction_ratio`, or `last_reduction_ratio` semantics.

### Step 1.6 — Tests

Add to the test block in `gmres_block_jacobi.rs`:

1. `full_ilu0_lower_times_upper_recovers_original_matrix` — for a small 4-row
   sparse test matrix, verify that the reconstructed L * U matches A at every
   nonzero position of A.

2. `full_ilu0_apply_solves_diagonal_system_exactly` — if A is diagonal, ILU(0)
   is exact: `ilu.apply(A * x) == x` for any x.

3. `cpr_with_full_ilu_smoother_reduces_fgmres_iteration_count` — create a small
   2-cell FIM system (6 unknowns), run the CPR solve twice with explicit smoother
   selection (`block-jacobi` baseline vs `ilu0`). Assert the ILU path converges in
   fewer iterations.

4. `gmres_ilu0_backend_keeps_non_cpr_semantics` — ensure the separate `GmresIlu0`
   path still runs without pressure-correction diagnostics and is unaffected by the
   new CPR smoother-selection plumbing.

### Validation gate after Phase 1

Use the matrix in the **Validation Matrix** section below. Phase 1 should not be considered
done until both the baseline-preservation rows and the new-smoother rows pass.

---

## Phase 2: Fix the Coarse ILU(0) for Large Systems

**Goal**: The original coarse ILU(0) path (lines 300–362 before the Phase 2 change) had two weaknesses that mattered when
reservoirs exceed 512 cells: (1) the `row_entry` linear scan is O(nnz) per element,
making factorization O(n × nnz²); (2) 8 iterations at 1e-2 tolerance is far too weak.

This phase has **no impact on current test cases** (coarse ≤ 512 rows → exact dense
solve) but is required before the solver can handle production-scale reservoirs.

### Step 2.1 — Replace linear coarse-row lookup with binary search

Replace the `row_entry` linear scan (lines 292–298) with a binary search:

```rust
fn row_entry(row: &[(usize, f64)], col: usize) -> f64 {
    match row.binary_search_by_key(&col, |&(c, _)| c) {
        Ok(idx) => row[idx].1,
        Err(_) => 0.0,
    }
}
```

This changes factorization from O(n × nnz²) to O(n × nnz × log nnz).

No extra sort pass is needed because the current `pressure_rows` builder already emits
sorted rows via `BTreeMap` accumulation.

### Step 2.2 — Replace defect-correction loop with inner BiCGSTAB

The current Richardson defect correction (lines 221–238) runs at most 8 steps at 1e-2
tolerance. Replace with ILU(0)-preconditioned BiCGSTAB:

```rust
fn solve_pressure_with_bicgstab(
    pressure_rows: &[Vec<(usize, f64)>],
    ilu: &PressureIlu,
    rhs: &DVector<f64>,
    max_iters: usize,
    tol: f64,
) -> (DVector<f64>, f64)
```

The expected benefit is empirical, not guaranteed. On current head, this part is implemented and
the first half of that bet already happened: `23x23x1` now shows much smaller coarse reduction
ratios than the old ILU defect-correction baseline. The remaining acceptance bar is practical:
keep the `23x23x1` bounded shelf shape unchanged while reducing iterative-backend fallback burden
or total runtime versus the current BiCGSTAB coarse path.

Update constants:
```rust
const PRESSURE_DEFECT_CORRECTION_MAX_ITERS: usize = 50;   // was 8
const PRESSURE_DEFECT_CORRECTION_REL_TOL: f64 = 1e-6;      // was 1e-2
```

### Step 2.3 — ILU(1) option (optional, profile-driven)

ILU(0) restricts fill to the original sparsity pattern. For a 7-point pressure stencil
this can leave large off-diagonal errors. If profiling after Phase 2.1–2.2 shows the
coarse solve is still limiting, add `factorize_pressure_ilu1` with one level of
fill-in. Gate behind a `FimPressureCoarseSolverKind::Ilu1` variant. Only implement if
the Phase 2.1–2.2 improvements are insufficient.

### Validation gate after Phase 2

- All Phase 1 validation gates must still pass.
- Run a larger case not covered by current tests:
  ```
  node scripts/fim-wasm-diagnostic.mjs \
    --preset water-pressure --grid 20x20x3 \
    --steps 1 --dt 1 --diagnostic summary --no-json
  ```
  (1200 cells → 1202-row coarse system, hits the ILU path.)
  Verify coarse BiCGSTAB converges within 32 iterations and coarse residual
  reaches < 1e-4 relative tolerance.

---

## Phase 3: AMG Is A Later Design Track, Not The Next Slice

**Goal**: Only revisit AMG after Phase 2 proves that coarse pressure quality, not fallback
policy or full-system backend persistence, is the remaining limiter on over-threshold cases.

This phase has no impact on current test cases. It is intentionally deferred because the
current coarse operator includes explicit well-BHP rows, and the design for handling those
rows in an aggregation hierarchy has not been validated yet.

### Step 3.1 — Represent the coarse pressure matrix in sorted CSR

After Phase 2.1, `pressure_rows` is already sorted. Add a helper:

```rust
fn pressure_rows_to_sprs(rows: &[Vec<(usize, f64)>]) -> sprs::CsMat<f64>
```

This gives access to sprs's sparse-matrix utilities (matrix-vector multiply, triple
product for the Galerkin operator) without reimplementing them.

### Step 3.2 — Aggregation-based coarsening (design prerequisite)

Before implementing this, decide whether explicit well-BHP rows are:

1. excluded from aggregation and carried as a separate coarse block,
2. pinned as singleton aggregates with special transfer operators, or
3. folded into a different well-aware CPRW-style coarse construction.

Without that decision, a plain cell-style aggregation is not an implementation-ready plan.

For the reservoir-cell subset only, a possible starting point is **plain aggregation** —
greedily group cells with their strongest-connected unaggregated neighbor:

```rust
fn build_aggregates(
    pressure_rows: &[Vec<(usize, f64)>],
    theta: f64,   // strength threshold, 0.25 is standard
) -> Vec<usize>   // for each fine node: its coarse aggregate index
```

Algorithm:
1. For each node `i`, a neighbor `j` is strong if
   `|A_ij| / max_k |A_ik| > theta`.
2. Iterate nodes in order. If node `i` is unaggregated: create a new aggregate
   containing `i` and its strongest unaggregated strong neighbor.
3. Remaining isolated nodes form singleton aggregates.

Target coarsening ratio: 4–8×. For a 5000-cell reservoir: 5000 → ~800 coarse nodes →
exact dense solve at the coarsest level.

### Step 3.3 — Piece-wise constant restriction and prolongation

Given `aggregates: Vec<usize>` (fine node → coarse index):

- **Prolongation** P: `P[i, aggregates[i]] = 1.0` (broadcast coarse correction to all
  fine nodes in aggregate). Stored as `Vec<usize>` — no explicit matrix needed.
- **Restriction** R = Pᵀ: sum of fine residuals in each aggregate. Also stored as
  `Vec<usize>`.

Apply operations:
```rust
fn restrict(fine_vec: &DVector<f64>, agg: &[usize], n_coarse: usize) -> DVector<f64>;
fn prolongate(coarse_vec: &DVector<f64>, agg: &[usize], n_fine: usize) -> DVector<f64>;
```

### Step 3.4 — Galerkin coarse-of-coarse operator

Build `A_cc = R * A_fine * P` using the aggregation map. For each entry
`A_fine[i, j] = v`, let `k = agg[i]`, `l = agg[j]`:

```rust
fn build_galerkin_coarse(
    a_fine: &[Vec<(usize, f64)>],
    agg: &[usize],
    n_coarse: usize,
) -> Vec<Vec<(usize, f64)>>  // then sort each row
{
    // A_cc[k][l] += A_fine[i][j]  for each fine entry (i,j)
    // O(nnz_fine) scatter — trivially cheap
}
```

The coarse-of-coarse system is small enough (n_coarse ≈ n_fine / 6) to store as a dense
matrix and invert exactly for the coarsest-level solve.

### Step 3.5 — AMG level data structure

```rust
struct AmgLevel {
    a_rows: Vec<Vec<(usize, f64)>>,  // fine-level pressure matrix at this level
    ilu: PressureIlu,                 // ILU(0) smoother for pre/post-smoothing
    agg: Vec<usize>,                  // fine → coarse aggregate map
    n_coarse: usize,
}

struct AmgHierarchy {
    levels: Vec<AmgLevel>,
    coarsest_dense_inverse: DMatrix<f64>,  // exact solve at coarsest level
}
```

### Step 3.6 — V-cycle application

```rust
fn apply_amg_vcycle(
    hierarchy: &AmgHierarchy,
    level_idx: usize,
    rhs: &DVector<f64>,
) -> DVector<f64> {
    let level = &hierarchy.levels[level_idx];

    // Base case: exact coarsest-level solve
    if level_idx + 1 == hierarchy.levels.len() {
        return &hierarchy.coarsest_dense_inverse * rhs;
    }

    // Pre-smooth: one ILU(0) application
    let mut u = apply_pressure_ilu(&level.ilu, rhs);

    // Restrict residual
    let r_fine = rhs - pressure_mat_vec(&level.a_rows, &u);
    let r_coarse = restrict(&r_fine, &level.agg, level.n_coarse);

    // Solve coarse error (recursive V-cycle)
    let e_coarse = apply_amg_vcycle(hierarchy, level_idx + 1, &r_coarse);

    // Prolongate and correct
    u += prolongate(&e_coarse, &level.agg, rhs.len());

    // Post-smooth
    let residual_after = rhs - pressure_mat_vec(&level.a_rows, &u);
    u += apply_pressure_ilu(&level.ilu, &residual_after);

    u
}
```

### Step 3.7 — Integration into `solve_pressure_correction`

Add `FimPressureCoarseSolverKind::Amg` variant (alongside existing `DenseInverse` and
`IluDefectCorrection`). Activate when `n_coarse > PRESSURE_DIRECT_SOLVE_ROW_THRESHOLD`:

```rust
fn solve_pressure_correction(&self, rhs: &DVector<f64>) -> (DVector<f64>, f64) {
    match &self.coarse_solver {
        DenseInverse(inv)       => (inv * rhs, rr),
        BiCGStab { ilu, .. }    => solve_bicgstab(rhs, ilu),
        Amg(hierarchy)          => (apply_amg_vcycle(hierarchy, 0, rhs), rr),
    }
}
```

### Exit criterion for Phase 3 planning

Do not implement AMG until all of the following are true:

1. Phase 1 is merged and materially improves current exact-dense coarse shelves.
2. Phase 2 is merged and the bounded `23x23x1` probe still shows coarse pressure as the
   dominant remaining penalty.
3. A well-row handling design is written down and reviewed against the current coarse operator.

---

## Data Structure Changes Summary

| Location | Change |
|----------|--------|
| `BlockJacobiPreconditioner` (`gmres_block_jacobi.rs:17`) | Add `full_ilu: Option<FimIlu0Factors>` and `fine_smoother_kind` |
| `gmres_block_jacobi.rs` solve/build path | Add internal CPR smoother selection so `FgmresCpr` can compare `block-jacobi` vs `ilu0` without changing `GmresIlu0` semantics |
| `FimPressureCoarseSolverKind` (`mod.rs`) | Phase 2 only: consider `BiCGStab`; do not add `Amg` until Phase 3 design is real |
| `FimCprDiagnostics` | Add `smoother_label: &'static str` without changing existing reduction-ratio semantics |
| `row_entry` helper (lines 292–298) | Replace linear scan with binary search |

---

## Expected Outcomes

| Metric | Current | Phase 1 target | Phase 2 target |
|--------|---------|----------------|----------------|
| FGMRES iters/Newton (12×12×3 waterflood) | 10–20 | materially lower, target ≤5 average | preserve or slightly improve |
| FGMRES iters/Newton (SPE1, 300 cells) | 10–20 | materially lower, target ≤5 average | preserve |
| Current `GmresIlu0` backend behavior | stable | unchanged semantics | unchanged semantics |
| Coarse solve for >512-row systems | 8 ILU iters, 1e-2 tol | unchanged | stronger coarse residual reduction and lower fallback burden |
| 5000-cell reservoir solve quality | degrades severely | unchanged | still open; AMG remains future work |

Phase 1 is implemented and test-green, but it is currently parked rather than promoted as a
runtime win because the shipped gas replay drift was not attributable to the new smoother and
still needs separate current-head explanation. Phase 2 is now the active over-threshold track.
Phase 3 stays deferred until the coarse well-row story is explicit.

## Validation Matrix

Use existing tests and the shipped wasm diagnostic suite only. A phase is not complete until
every row in its gate passes.

### Phase 1 gate: CPR fine-smoother change

| Purpose | Command / test | Expected result |
|---------|----------------|-----------------|
| Locked CPR coarse correctness | `cargo test pressure_correction_uses_exact_dense_inverse_when_small --lib` | Pass |
| Locked transfer-weight correctness | `cargo test pressure_transfer_weights_follow_local_schur_elimination --lib` | Pass |
| Locked FIM smoke: early SPE1 stability | `cargo test spe1_fim_first_steps_converge_without_stall --lib` | Pass |
| Locked FIM smoke: gas creation path | `cargo test spe1_fim_gas_injection_creates_free_gas --lib` | Pass |
| New baseline-preservation unit | `cargo test gmres_ilu0_backend_keeps_non_cpr_semantics --lib` | Pass |
| New factorization unit | `cargo test full_ilu0_lower_times_upper_recovers_original_matrix --lib` | Pass |
| New exactness unit | `cargo test full_ilu0_apply_solves_diagonal_system_exactly --lib` | Pass |
| New A/B iteration unit | `cargo test cpr_with_full_ilu_smoother_reduces_fgmres_iteration_count --lib` | Pass |
| Representative exact-dense water shelf | `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json` | CPR trace reports `solver=dense`; `smoother=ilu0` on the new path; average FGMRES iterations materially lower than current baseline; no worse accepted-substep class than current `129`/`0/35/0` shelf |
| Representative gas shelf non-regression | `node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic outer --no-json` | First-step budget remains within current smoke expectations; no regression in shipped replay stability |

### Phase 2 gate: over-threshold coarse solver change

| Purpose | Command / test | Expected result |
|---------|----------------|-----------------|
| Phase 1 regression pack | All Phase 1 rows above | Still pass |
| Exact-dense control below threshold | `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 22x22x1 --steps 1 --dt 0.25 --diagnostic step --no-json | rg -m 8 "cpr=\[|FIM retry summary|FIM step done|Newton: dt="` | Control remains `rows=486 solver=dense`; bounded shelf shape stays `substeps=8`, `retries=0/3/0` |
| Over-threshold probe | `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 --diagnostic step --no-json | rg -m 8 "cpr=\[|FIM retry summary|FIM step done|Newton: dt="` | Probe remains bounded at `substeps=8`, `retries=0/3/0` while coarse residual reduction, runtime class, or fallback burden improves versus current ILU defect-correction baseline |
| Optional larger coarse case | `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 20x20x3 --steps 1 --dt 1 --diagnostic summary --no-json` | Coarse solver behavior is measurable above threshold and does not force an earlier nonlinear regression |

### Promotion rules

1. **Promote Phase 1** only if the exact-dense benchmark shelves improve materially and all
  locked tests remain green.
2. **Promote Phase 2** only if the `23x23x1` over-threshold probe improves without worsening
  the bounded shelf shape relative to the `22x22x1` control.
3. **Do not start Phase 3** until Phase 2 still leaves coarse-pressure quality as the clear
  dominant cost on over-threshold cases.

---

## Files to Modify

| File | Phase |
|------|-------|
| `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs` | 1, 2 |
| `src/lib/ressim/src/fim/linear/mod.rs` | 1 for additive diagnostics fields; 2 for any coarse-solver enum changes |
| `src/lib/ressim/src/fim/newton.rs` | 1 for additive CPR diagnostic trace field only |

No physics, assembly, or timestep-control behavior should change in Phase 1 beyond the
linear solve path and additive diagnostics.
