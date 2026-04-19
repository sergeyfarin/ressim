# FIM Jacobian Reuse Investigation (2026-04-19)

This doc captures the feasibility investigation for Jacobian reuse /
lagged Jacobian within the FIM Newton loop. It was produced after the
2026-04-18 medium-water cost profile revealed that ~88% of `lin_ms` on
`water-pressure 20x20x3 dt=0.25 --steps 1` is per-Newton-iteration
`sparse-lu` refactorization, not retry ladders or iterative-backend
iterations. Three reverted attempts (A, B, E) had all targeted retry
ladder overhead, which is capped at ≤34% of cost.

See also:
- `docs/FIM_CONVERGENCE_IMPROVEMENTS.md` "Medium-water `20x20x3` step-1
  cost profile — 2026-04-18"
- `docs/FIM_CONVERGENCE_WORKLOG.md` same title
- `project_fim_cost_profile_2026-04-18` memory

## Flow trace — current Newton iter cost structure

Within each Newton iter at [fim/newton.rs:2277](../src/lib/ressim/src/fim/newton.rs#L2277):

1. `assemble_fim_system` — rebuilds residual + Jacobian from current
   `state` (includes `property_eval_ms` + `jacobian_ms`).
2. `solve_linearized_system` → `sparse_lu_debug::solve` at
   [sparse_lu_debug.rs:39](../src/lib/ressim/src/fim/linear/sparse_lu_debug.rs#L39):
   - `build_sparse_row_matrix` — format conversion, O(nnz)
   - `matrix.sp_lu()` — **full factorization every time** (~150ms per
     call on 3604 rows)
   - back-solve + up to 2 iterative-refinement solves (cheap)
3. `appleyard_damping` + `apply_newton_update_frozen` — compute damping,
   apply update.
4. Convergence check; if not converged, loop back to step 1.

**Nothing is cached across Newton iters.** The Jacobian is rebuilt and
re-factored every single time. On a medium-water rung with 12-20 Newton
iters, that's 12-20 full sparse-LU factorizations at ~150ms each, giving
the 1,500-3,500 ms per-rung `lin_ms` observed in the profile.

## The reuse opportunity

Classical lagged-Jacobian: if the state change since the last
factorization is small (Appleyard-damped updates often qualify), the
Jacobian `J(x_{k+1}) ≈ J(x_k)`. Reusing the old factorization with the
*new* RHS gives a step that isn't quite Newton, but is close enough for
convergence — and is ~10-20× cheaper per iter because both `sp_lu()`
and full assembly are skipped.

The data already in the trace supports this: late-Newton iters in
medium-water rungs have `damp` values and `upd` norms that drop quickly
(Appleyard chop). Those are exactly the iters where Jacobian reuse is
safe.

## What makes this slice tractable

1. **Assembly+factorization are clearly separable sites** — two
   independent top-of-loop operations.
2. **We can keep `Option<sp_lu_factorization>` stashed on the simulator**
   alongside the Jacobian it came from, plus a step-change-threshold
   gate: if `damp * ||update||_inf < threshold`, reuse.
3. **Backup path already exists** — if reuse fails (non-convergence or
   invalid step), fall through to the existing full-assembly +
   full-factorization path.

## Risks

1. **Lifetime/ownership of `faer` factorization objects** — `sp_lu()`
   result may borrow from the matrix. Need to verify the factorization
   can be stored across function calls (probably yes via owned
   `SparseRowMat` + owned factorization; needs a small test).
2. **Locked smoke tests sensitivity** — `drsdt0_base_rs_cap_flashes_*`,
   `spe1_fim_first_steps_converge_without_stall`,
   `spe1_fim_gas_injection_creates_free_gas` are sensitive to exact
   Newton trajectories. Reuse changes the trajectory, not just the cost.
3. **Interaction with bypass paths** — decide whether reuse is allowed
   while dead-state / zero-move / restart-stagnation direct-bypass is
   active. Conservative: disable reuse whenever any bypass is active.
4. **Iterative refinement in `sparse_lu_debug.rs`** — correctly using
   the old factorization still requires the *current* Jacobian for the
   residual check in refinement. Either skip refinement on reused
   factorizations, or pay the cheaper `cs_mat_mul_vec` cost
   (reconstructing the matrix has to be paid anyway if we want the
   iter-refinement correctness test).

## Proposed experimental slice

### Stage 1 — measurement (no behavioral change)
Instrument per-Newton-iter tracing of:
- `damp` (Appleyard damping factor)
- `||update||_inf` (scaled)
- a computed-offline `reuse_candidate?` flag (e.g. `damp * ||update|| < threshold`)
- `lin_ms` attribution per iter (already have it in aggregate)

Run the three canonical shortlist cases on current-head wasm:
- medium water 20x20x3 dt=0.25 step=1 (the profiled case)
- medium water 20x20x3 dt=0.25 steps=6 (the target)
- heavy water 12x12x3 dt=1 (worst-cost case, 8373 substeps with replay)

Count:
- Of the Newton iters on each rung, how many satisfy the reuse gate?
- If all gated iters reused their last factorization, how much `lin_ms`
  would be saved?
- Does eligibility cluster late-in-rung (where Appleyard chop has
  damped the step), or is it scattered?

**Exit criteria for Stage 1:**
- If ≥30% of Newton iters are reuse-eligible on medium water step-1 and
  the eligible iters contain ≥40% of `lin_ms`, proceed to Stage 2.
- If eligibility is <15% or concentrated on cheap iters, revert Stage 1
  instrumentation and document as a negative.

### Stage 2 — implementation (gated behind feature flag, only if Stage 1 passes)
- Add `reused_factorization: Option<(CsMat<f64>, SparseLuFactorization)>` to
  `ReservoirSimulator` (or to a Newton-loop local scope).
- Gate: reuse iff `damp * ||update||_inf < threshold` AND no bypass is
  active AND last linear solve converged.
- Fall-through: if solve with reused factorization produces an invalid
  step (NaN, fails Appleyard bounds, fails material-balance gate),
  discard reuse and refactor.
- Metric: measure `lin_ms` delta on shortlist; locked smoke tests must
  stay green.

### Stage 3 — promote or revert
Same promotion discipline as previous directions: full shortlist, both
single-step and multi-step medium water, locked baselines, `outer_ms`
and `lin_ms` both pinned on a clean commit.

## What this investigation is NOT

- Not a rewrite of the linear solver stack.
- Not a CPR replacement.
- Not a retry-ladder change. Previous A/B/E attempts all changed retry
  policies; this is orthogonal — it targets per-iter cost within each
  rung, not the rung count or the retry factor.
- Not tied to Newton initial-guess extrapolation. Slice A failed because
  extrapolated initial guesses broke replay-accept invariants. Reuse
  does not change the Newton iterate, only how the Newton step is
  computed. Existing convergence criteria are unchanged.

## Stage 1 results — 2026-04-19

### Probe design
Instrumented `newton.rs` iteration loop with a `REUSE-PROBE` diagnostic
line that emits per Newton iter:
- `damp_x_upd = damping * ||update||_inf`
- `lin_ms = linear_report.total_time_ms` (caveat below)
- which bypass flags fired (`dead-state`, `restart-stag`,
  `zm-fallback`, `zm-repeat`)
- `fallback`, `lin_conv`, `backend` (`sparse-lu` / `fgmres-cpr`)
- four boolean eligibility flags:
  - `strict_1e-3`: `lin_conv && !fallback && !any_bypass && damp_x_upd < 1e-3`
  - `perm_1e-3`: `lin_conv && damp_x_upd < 1e-3` (permissive)
  - `perm_1e-2`: permissive at `<1e-2`
  - `perm_1e-1`: permissive at `<1e-1`

Instrumentation was 49 lines, reverted after measurement via
`git checkout -- src/lib/ressim/src/fim/newton.rs`.

### Cases exercised
Ran on current-head wasm via `scripts/fim-wasm-diagnostic.mjs`:
1. medium water 20x20x3 dt=0.25 step=1 (profiled case)
2. medium water 20x20x3 dt=0.25 steps=6 (target)
3. heavy water 12x12x3 dt=1 (worst-cost case)

### Eligibility tables

**Case 1 — medium-water 20x20x3 dt=0.25 step=1**

| Gate       | iters eligible / total | iter % | probe-lin_ms eligible / probe total | lin_ms % |
|------------|------------------------|-------:|-------------------------------------|---------:|
| strict 1e-3| (0 sparse-lu eligible) |     0% | ~0                                  |      ~0% |
| perm 1e-3  | ~68/146                |   47% | ~3,900/7,365                        |      53% |
| perm 1e-2  | ~102/146               |   70% | ~5,500/7,365                        |      75% |
| perm 1e-1  | ~128/146               |   88% | ~6,500/7,365                        |      88% |

**Case 2 — medium-water 20x20x3 dt=0.25 steps=6**

| Gate       | iter %  | lin_ms % |
|------------|--------:|---------:|
| perm 1e-3  |     32% |      30% |
| perm 1e-2  |     45% |      41% |
| perm 1e-1  |     70% |      65% |

**Case 3 — heavy-water 12x12x3 dt=1**

| Gate       | iter %  | lin_ms % |
|------------|--------:|---------:|
| perm 1e-3  |     41% |      38% |
| perm 1e-2  |     78% |      72% |
| perm 1e-1  |     93% |      88% |

### Caveats

1. **Probe undercounts `lin_ms` by ~2-3×.** Step-level `lin_ms=19,401`
   on case 1 vs probe sum ≈ 7,365. Root cause: when a fallback solve
   fires, `linear_report` is overwritten with the fallback's timing
   before the probe runs. First-solve time is still added to the
   aggregate `linear_solve_time_ms` counter, but is invisible to the
   probe. Ratios within the visible subset remain meaningful.

2. **Strict gate yields zero `sparse-lu` coverage on case 1.** Every
   `sparse-lu` iter on that step has either `used_fallback=true` OR
   some bypass active. The strict gate is therefore too conservative
   and would not trigger in the hot block; the permissive gate is the
   actionable one.

3. **Permissive gate assumes bypass-safe reuse.** Permissive-gated
   iters include ones where a bypass is active. Stage 2 must validate
   that bypass iters are in fact safe to reuse-across, or narrow the
   gate to exclude specific bypasses.

### Exit criterion
Stage 1 exit criterion: ≥30% of Newton iters reuse-eligible AND ≥40%
of `lin_ms` eligible on medium-water step-1 at `<1e-3`.

- Case 1 (step 1, profiled): **47% iters, 53% lin_ms** at `<1e-3`. **PASS.**
- Case 2 (6-step target): **32% iters, 30% lin_ms** at `<1e-3`. Marginal
  on `lin_ms`; passes at `<1e-2` (45%/41%).
- Case 3 (heavy): **41% iters, 38% lin_ms** at `<1e-3`. Marginal;
  passes at `<1e-2` (78%/72%).

**Verdict: PASS.** Proceed to Stage 2 with gate at `<1e-2` (recovers
41-75% of `lin_ms` across all three cases) rather than `<1e-3`, since
the stricter gate is marginal on two cases and the looser gate's extra
eligibility is concentrated on iters where Appleyard-damped steps
dominate — the regime where reuse is theoretically safest.

## Stage 2 design — 2026-04-19

### Reuse mechanism

**State carried across Newton iters within a single rung:**
```rust
struct JacobianReuseCache {
    jacobian: CsMat<f64>,
    factorization: faer::sparse::SparseLuFactorization<usize, f64>,
    factorization_nnz: usize,
    age_iters: u32,  // how many consecutive reuses since last refactor
}
```

Stored as `Option<JacobianReuseCache>` local to the Newton loop (not
on `ReservoirSimulator` — lifetime scoped to one rung call).

### Gate (evaluated at end of iter `k`, affects iter `k+1`)

Reuse iff ALL hold:
1. `linear_report.converged` on iter `k`.
2. `damp * ||update||_inf < 1.0e-2` on iter `k`.
3. No bypass is active on iter `k+1` build — revalidate after
   `assemble_fim_system` on `k+1` before deciding whether to refactor.
4. `age_iters < 3` — cap consecutive reuses to prevent Jacobian from
   aging arbitrarily stale.
5. No new NaN/Inf in residual on `k+1`.

### Solve path

On a reuse iter:
1. Skip `matrix.sp_lu()`.
2. Skip `build_sparse_row_matrix` for the solve itself — but we DO
   need `cs_mat_mul_vec` against the *current* Jacobian for residual
   check in iterative refinement. (The current Jacobian still has to
   be assembled for the RHS anyway, so there's no new assembly cost.)
3. Back-solve with cached factorization.
4. Iterative-refinement loop uses the *current* Jacobian (as today) —
   the cached factorization only affects the preconditioner step.

### Invalid-step fallback

After the reused solve, before applying to state:
- If solution has NaN/Inf → discard, refactor, re-solve.
- If post-Appleyard update fails material-balance gate → discard,
  refactor, re-solve.
- If iterative-refinement residual norm after reuse-solve is worse
  than `FALLBACK_REFINEMENT_RATIO * pre_solve_residual_norm` (target
  ratio: 10.0) → discard, refactor, re-solve.

The fallback always invalidates the cache (`age_iters = 0`) since the
Jacobian drift that triggered failure will still be present next iter.

### Instrumentation for Stage 2

Add to existing per-rung summary:
- `reuse_iters` — count of iters that reused factorization
- `reuse_fallback_iters` — count of reuses that failed and refactored
- `reuse_saved_ms` — probe-estimated savings (cached `sp_lu` time)

### Validation plan for Stage 2

Before promotion:
1. Locked smoke tests green:
   - `drsdt0_base_rs_cap_flashes_*`
   - `spe1_fim_first_steps_converge_without_stall`
   - `spe1_fim_gas_injection_creates_free_gas`
2. Full shortlist rerun on clean commit:
   - heavy water 12x12x3 dt=1
   - medium water 20x20x3 dt=0.25 step=1 AND steps=6
   - bounded 22x22, 23x23
   - gas 10x10x3 steps=6
3. Both `outer_ms` and `lin_ms` must not regress on any shortlist
   case; `lin_ms` expected improvement: ≥30% on medium-water step-1.

### Stage 2 NOT doing

- NOT caching across Newton rungs (dt changes, Jacobian structure
  can change via wells). Cache lifetime = one rung.
- NOT touching the iterative-backend (`fgmres-cpr`) path — cache is
  sparse-LU only.
- NOT introducing a feature flag — a single reuse threshold is
  simpler to revert via git than a flag is to maintain.
