# FIM CPR Preconditioner Improvement Plan

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

**Three independent improvements, in priority order:**

| Phase | Target | Current test impact | Scale-up impact |
|-------|--------|---------------------|-----------------|
| 1 | Replace block-Jacobi with ILU(0) on the full Jacobian | High — fixes 10–20 iter count | High |
| 2 | Fix coarse ILU(0) for >512-row systems | None (all test cases use dense) | Required |
| 3 | Two-level AMG on the coarse pressure matrix | None | Scale-up to 10k+ cells |

---

## Phase 1: Full-System ILU(0) Fine Smoother

**Goal**: Replace the per-block block-Jacobi smoother with ILU(0) on the full
`sprs::CsMat<f64>` Jacobian. Highest-leverage change because it directly targets the
bottleneck on all current test cases.

**Technical basis**: The Jacobian is already `sprs::CsMat<f64>` (`FimAssembly.jacobian`,
`assembly.rs:19`). The existing `factorize_pressure_ilu0` (`gmres_block_jacobi.rs:300`)
already implements ILU(0) on `Vec<Vec<(usize, f64)>>`. The infrastructure exists — it
needs to be adapted to the full system and wired into the preconditioner.

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

### Step 1.3 — Extend `BlockJacobiPreconditioner` to hold ILU factors

Add one field to the struct (line 17):

```rust
full_ilu: Option<FimIlu0Factors>,   // None → fall back to block-Jacobi
```

Build at the end of `build_block_jacobi_preconditioner` (line 443), after the coarse
system assembly, using the `matrix: &CsMat<f64>` parameter already passed in:

```rust
let full_ilu = factorize_full_ilu0(matrix);
// None if mat is too large or factorization encounters singular diagonal
```

### Step 1.4 — Replace `apply_stage_one` with ILU-aware dispatch

Change `apply_stage_one` (lines 77–98):

```rust
fn apply_stage_one(&self, vector: &DVector<f64>, result: &mut DVector<f64>) {
    if let Some(ilu) = &self.full_ilu {
        *result = ilu.apply(vector);
        return;
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

### Step 1.6 — Tests

Add to the test block in `gmres_block_jacobi.rs`:

1. `full_ilu0_lower_times_upper_recovers_original_matrix` — for a small 4-row
   sparse test matrix, verify that the reconstructed L * U matches A at every
   nonzero position of A.

2. `full_ilu0_apply_solves_diagonal_system_exactly` — if A is diagonal, ILU(0)
   is exact: `ilu.apply(A * x) == x` for any x.

3. `cpr_with_full_ilu_smoother_reduces_fgmres_iteration_count` — create a small
   2-cell FIM system (6 unknowns), run `solve_linearized_system` twice with
   `FgmresCpr` (block-Jacobi path) and the new ILU path. Assert the ILU path
   converges in fewer iterations.

### Validation gate after Phase 1

- All locked baselines must pass: `spe1_fim_first_steps_converge_without_stall`,
  `spe1_fim_gas_injection_creates_free_gas`, `pressure_correction_uses_exact_dense_inverse_when_small`,
  `pressure_transfer_weights_follow_local_schur_elimination`.
- Run canonical wasm diagnostic:
  ```
  node scripts/fim-wasm-diagnostic.mjs \
    --preset water-pressure --grid 12x12x3 \
    --steps 1 --dt 1 --diagnostic summary --no-json
  ```
  Target: FGMRES linear iterations per Newton step reduced from 10–20 to ≤5 average.
- SPE1 gas: first-step substep count must remain ≤20 (existing test budget).

---

## Phase 2: Fix the Coarse ILU(0) for Large Systems

**Goal**: The coarse ILU(0) (lines 300–362) has two weaknesses that matter when
reservoirs exceed 512 cells: (1) the `row_entry` linear scan is O(nnz) per element,
making factorization O(n × nnz²); (2) 8 iterations at 1e-2 tolerance is far too weak.

This phase has **no impact on current test cases** (coarse ≤ 512 rows → exact dense
solve) but is required before the solver can handle production-scale reservoirs.

### Step 2.1 — Sort `pressure_rows` after construction

`pressure_rows: Vec<Vec<(usize, f64)>>` currently has no guaranteed column ordering.
Sort each row by column index immediately after the assembly loop at line 674:

```rust
for row in &mut pressure_rows {
    row.sort_unstable_by_key(|&(col, _)| col);
}
```

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

BiCGSTAB with ILU(0) preconditioner converges in O(√κ) iterations rather than O(κ)
for Richardson, where κ is the condition number of the preconditioned system.

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

## Phase 3: Two-Level AMG on the Coarse Pressure Matrix

**Goal**: For reservoirs with more than ~1500 cells (coarse pressure system > 512 rows),
provide an O(n log n) coarse solver via a two-level aggregation AMG hierarchy.

This phase has no impact on current test cases but is required for production-scale
grids (tens of thousands of cells).

### Step 3.1 — Represent the coarse pressure matrix in sorted CSR

After Phase 2.1, `pressure_rows` is already sorted. Add a helper:

```rust
fn pressure_rows_to_sprs(rows: &[Vec<(usize, f64)>]) -> sprs::CsMat<f64>
```

This gives access to sprs's sparse-matrix utilities (matrix-vector multiply, triple
product for the Galerkin operator) without reimplementing them.

### Step 3.2 — Aggregation-based coarsening

For the pressure stencil on a Cartesian grid (primarily nearest-neighbor coupling),
use **plain aggregation** — greedily group cells with their strongest-connected
unaggregated neighbor:

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

### Validation gate after Phase 3

- All Phase 1 and Phase 2 validation gates must still pass.
- Add a unit test: `amg_vcycle_reduces_residual_by_factor_on_1d_pressure_problem` —
  build a tridiagonal 100-row pressure system (1D diffusion), apply one AMG V-cycle,
  assert residual reduces by > 10×.
- Run a large diagnostic:
  ```
  node scripts/fim-wasm-diagnostic.mjs \
    --preset water-pressure --grid 30x30x3 \
    --steps 1 --dt 1 --diagnostic summary --no-json
  ```
  (2700 cells → coarse > 512 rows, AMG path active.)
  Compare FGMRES iteration counts: AMG path vs BiCGSTAB path on the same case.
  AMG should give ≤5 FGMRES iterations; iteration count should stay flat as grid grows.

---

## Data Structure Changes Summary

| Location | Change |
|----------|--------|
| `BlockJacobiPreconditioner` (`gmres_block_jacobi.rs:17`) | Add `full_ilu: Option<FimIlu0Factors>` |
| `FimPressureCoarseSolverKind` (`mod.rs`) | Add `BiCGStab` and `Amg` variants |
| `pressure_rows` construction (line 674) | Sort each row after build |
| `FimCprDiagnostics` | Add `smoother_label: &'static str` |
| `row_entry` helper (lines 292–298) | Replace linear scan with binary search |

---

## Expected Outcomes

| Metric | Current | After Phase 1 | After Phase 2+3 |
|--------|---------|---------------|-----------------|
| FGMRES iters/Newton (12×12×3 waterflood) | 10–20 | 2–5 | 2–4 |
| FGMRES iters/Newton (SPE1, 300 cells) | 10–20 | 2–5 | ~2 |
| OPM reference (SPE1) | — | — | ~1.4 |
| Coarse solve for >512-row systems | 8 ILU iters, 1e-2 tol | BiCGStab, 1e-6 tol | AMG V-cycle |
| 5000-cell reservoir solve quality | Degrades severely | Degrades | O(n log n) |

Phase 1 alone closes most of the current SPE1 and waterflood gap. Phases 2 and 3 are
prerequisites for scaling beyond the current benchmark grid sizes.

---

## Files to Modify

| File | Phase |
|------|-------|
| `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs` | 1, 2, 3 |
| `src/lib/ressim/src/fim/linear/mod.rs` | 2, 3 (new enum variants) |

No other files need to change. The Newton solver, assembly, and timestep controller
are not touched by this plan.
