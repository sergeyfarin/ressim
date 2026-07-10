# Bundle P: CPR Preconditioner Setup Reuse (the 24x per-iteration factor)

Status: PLAN (2026-07-10). Registry row: `FIM-BUNDLE-P` (OPEN). Nothing implemented.
Origin: Task #41 factor budget (`docs/FIM_CONVERGENCE_WORKLOG.md`) — preconditioner build is
`pc_ms = 32.9s of 36.7s` (89% of wall-clock) on the heavy Legacy case, because ResSim rebuilds
the full CPR setup at every Newton iteration of every substep of every retry rung. OPM's
default is `--cpr-reuse-setup=4`: reuse the setup, fully recreate every `--cpr-reuse-interval=30`
linear solves (verified from the installed Flow 2025.10 binary and `ISTLSolver.hpp` l.515-552 —
see `docs/FIM_BUNDLE_N_DESIGN.md` §9.6 for the full verified semantics of all 5 modes).

Independent of Bundle N: benefits Legacy and `OpmAligned` alike, and cuts the cost of all
future heavy-case experiments ~10-20x (the two Bundle N §5 confirmation runs cost ~5h each,
~90% of it preconditioner rebuild — experiment cost, not just production cost, is what this
bundle buys back).

## What "the setup" is in ResSim (code facts, verified 2026-07-10)

`build_block_jacobi_preconditioner` (`fim/linear/gmres_block_jacobi.rs:958`), called once per
`solve()` / `solve_with_cpr_fine_smoother` invocation (i.e. once per Newton iteration), builds
`BlockJacobiPreconditioner` (l.312) containing, in build-cost order (order assumed, to be
MEASURED in P0):

1. `pressure_dense_inverse` — explicit dense inverse of the coarse pressure operator via
   `try_inverse()` for ≤512 coarse rows (heavy case: 432x432, O(n³)); else BiCGStab+ILU0 setup
   (`pressure_l_rows`/`pressure_u_diag`/`pressure_u_rows`).
2. `full_ilu` / block-ILU0 fine-smoother factorization (O(nnz) with 3x3 block inversions).
3. Quasi-IMPES restriction/prolongation weights + coarse operator assembly
   (`pressure_restriction`/`pressure_prolongation`/`pressure_rows`, O(nnz)).
4. `cell_block_inverses` (block-Jacobi 3x3 inverses), `tail_inverse` (empty under
   `eliminate_wells=true` — the production path solves the REDUCED, cells-only system; the
   well Schur elimination itself, `well_schur.rs`, has its own small per-iteration `J_WW`
   inverse that is NOT part of this bundle: it is tiny and must stay fresh).

Seam constraints:

- `solve_linearized_system` (`fim/linear/mod.rs:216`) is a pure function — there is nowhere to
  keep state today. The production caller is `fim/newton.rs` (primary solve + direct-fallback
  solve per iteration); the well-elimination path RECURSES into `solve_linearized_system` with
  the reduced system, so the cache must key on the reduced system's shape, not the full one.
- On wasm, `should_force_direct_solve` sends rows ≤ 512 to dense LU — those calls never build a
  CPR setup and must bypass the cache unchanged.
- `BlockJacobiPreconditioner` is private to `gmres_block_jacobi.rs`; the cache type must be an
  opaque `pub(crate)` handle exported from `fim/linear` with internals in `gmres_block_jacobi`.

## P0 — Offline measurement first (mandatory, per fim/linear workflow)

Two `#[ignore]`d additions to `fim/linear/solver_lab.rs`, run over the existing captured
corpora (heavy-case + bounded, `fim-capture-v2`):

1. **Build-cost breakdown**: add a `CprBuildTiming { weights_ms, coarse_assembly_ms,
   coarse_factorization_ms, fine_smoother_ms, block_inverses_ms }` struct populated inside
   `build_block_jacobi_preconditioner` (always-on, timers are cheap; surfaced through
   `FimCprDiagnostics` as an optional field). Lab test prints per-corpus medians. This decides
   whether P2 (LU instead of explicit inverse) matters independently of P1 (reuse).
2. **Stale-preconditioner inflation study**: for consecutive captured systems from the same
   Newton run, build the setup on system `i` and solve systems `i+1..i+k` with it (k up to 30,
   OPM's interval); record iteration count and convergence vs a fresh-setup baseline per
   system. **Offline gate: median inflation ≤ +2 iterations and no new convergence failures at
   k ≤ 30.** If staleness blows up convergence offline, the live wiring is not attempted and
   the row is closed REFUTED — same discipline as `FIM-LINEAR-008`'s offline-first bet.

## P1 — Reuse wiring (inert by default, one flag flip to promote)

- New opaque `FimCprSetupCache` (internals in `gmres_block_jacobi.rs`): holds the built
  `BlockJacobiPreconditioner` plus its key (`rows`, `layout`, `kind`, smoother/restriction
  kinds) and `solves_since_rebuild`.
- New option `FimLinearSolveOptions::cpr_reuse_interval: Option<usize>` — default `None`
  (= rebuild every solve, exactly today's behavior; every existing test and the production
  default stay bit-identical until promotion). `Some(30)` = OPM mode-4 semantics.
- New entry point `solve_linearized_system_cached(..., cache: &mut FimCprSetupCache)`; the
  existing `solve_linearized_system` delegates with a throwaway cache so no test call site
  changes. The cache handle threads: `timestep.rs::step_internal_fim_impl` (owns it — one
  cache per outer step call, so reuse spans substeps AND retry rungs, matching OPM's global
  solve counter) → `run_fim_timestep` → newton's two solve call sites → through
  `well_schur::solve_with_well_elimination`'s recursion (cache applies to the reduced solve).
- Rebuild triggers (in order): key mismatch (rows/layout/kind — correctness, always);
  `solves_since_rebuild >= interval` (OPM mode 4); explicitly NOT on dt change or substep
  boundary (OPM mode 4 spans timesteps; softer mode-1-like invalidation is the recorded
  fallback lever if live gates fail, before abandoning). Optional recorded lever, not default:
  OPM mode-2-style guard (rebuild when the previous solve took > 10 iterations).
- Fallback direct solves and wasm ≤512-row direct path bypass the cache untouched.

## P2 — Coarse factorization instead of explicit inverse (conditional on P0)

Replace `pressure_dense_inverse: Option<DMatrix>` (explicit `try_inverse()`) with a stored LU
factorization applied by triangular solves (same O(n²) apply, ~3-4x cheaper build). Only
implemented if P0's breakdown shows coarse factorization still material after P1's 30x
amortization; otherwise recorded as not-needed with the P0 numbers as evidence.

## Gates (honest: bit-identity is NOT achievable here)

A reused (stale) preconditioner changes Krylov iterates within tolerance, which changes Newton
updates, which — on this system, with its measured trajectory chaos (`FIM-DAMP-004`'s k-sweep)
— can shift substep counts. The control matrix therefore CANNOT be gated on bit-identity for
the promotion flip (it CAN and must be for the inert P1 wiring commit, since `None` default
changes nothing). Promotion bar:

1. **Inert-wiring commit**: control matrix + heavy case bit-identical, locked smoke 3/3
   (default `None` = no behavior change).
2. **Offline gate** (P0.2) passed before any live flip.
3. **Promotion flip (`Some(30)`)**:
   - Locked smoke 3/3; BL benchmarks green.
   - Fine-dt FOPT (April methodology, 16x0.0625) within 0.5% of the current bundle's own
     `3883.47` — physics unchanged.
   - Control matrix re-run: no case regresses substeps by more than ~25% (drift expected,
     collapse not); new counts recorded as the new baselines with this commit hash.
   - **Heavy-case Legacy wall-clock: ≥ 5x improvement (36.9s → ≤ 7s)** — the point of the
     bundle. (Bundle N §5's ≤4s no-P target remains the eventual combined goal.)
   - `pc_ms/outer_ms` fraction drops from ~89% to a minority share.
4. Any gate failing → revert the default flip only (wiring stays, inert), record verdict in
   `FIM-BUNDLE-P`, try the recorded fallback levers (mode-1-like invalidation; mode-2 guard)
   at most once each before closing REFUTED.

## Risks

| Risk | Mitigation |
|---|---|
| Trajectory chaos shifts control-matrix counts | Expected; gate on bounded drift + physics (fine-dt FOPT), not bit-identity; new baselines recorded on promotion |
| Stale setup degrades convergence right after big state changes (first iterations of a new substep at 3x-grown dt) | P0.2 measures exactly this on real consecutive systems; mode-2 guard is the recorded fallback |
| Cache key misses a relevant dimension (e.g. reduced vs full layout under well elimination) | Key = rows + layout + kind + smoother/restriction kinds; correctness rebuild on any mismatch; unit test for the well-elimination recursion path |
| wasm memory for the retained setup (~1.5MB dense inverse + ILU factors at current sizes) | One cache instance, bounded by grid size; acceptable at dev-only FIM scale |
| Signature churn breaking tests | New `_cached` entry point; old signature delegates with throwaway cache — zero test-site changes until promotion |

## Files

- `fim/linear/gmres_block_jacobi.rs` — `CprBuildTiming`, `FimCprSetupCache` internals, reuse
  branch in `solve_with_cpr_fine_smoother`.
- `fim/linear/mod.rs` — `cpr_reuse_interval` option, `solve_linearized_system_cached`, opaque
  cache re-export.
- `fim/linear/well_schur.rs` — thread the cache through the reduced-system recursion.
- `fim/linear/solver_lab.rs` — P0 timing + staleness tests.
- `fim/newton.rs`, `fim/timestep.rs` — cache ownership + threading.
- `docs/FIM_EXPERIMENT_REGISTRY.md` — `FIM-BUNDLE-P` verdict either way.
