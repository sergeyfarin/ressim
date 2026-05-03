# AMG Phase 2 — scirs2-sparse Path-A spike results

**Branch:** `experiment/fim-amg-scirs2`
**Date:** 2026-05-03
**Plan reference:** `docs/FIM_AMG_INTEGRATION_PLAN.md` (committed
2026-05-01 to master at 48da61b).
**Status:** Path-A wired up and measured; result is **partial win
that does not unlock Bundle B promotion**. Decision pending.

## What was done

Wired `scirs2-sparse 0.4.2` into the CPR coarse-solve dispatch:

- Added `scirs2-sparse = "0.4"` and `ndarray = "0.17"` to
  `src/lib/ressim/Cargo.toml`.
- Added `solve_pressure_with_amg(pressure_rows, rhs, max_iters, tol)`
  in `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs`. Builds a
  `CsrArray` from the existing `pressure_rows` triplet representation,
  constructs `AMGPreconditioner` (Ruge-Stüben C/F splitting with
  Gauss-Seidel smoother and V-cycle 1/1 — defaults), and runs outer
  iterations of `correction = AMG.apply(residual)` until tolerance.
- Updated dispatch in `solve_pressure_correction`: try AMG first;
  fall back to BiCGStab+ILU(0) only if AMG fails to construct.
- Direct-dense path (n ≤ 512) untouched — AMG only runs when the
  coarse pressure system is large.

**No caching of the AMG hierarchy across CPR applications** — built
fresh each call. Setup cost dominates on small coarse systems.

## Validation

- All 22 `gmres_block_jacobi` unit tests pass.
- Buckley-Leverett, Dietz PSS, SPE1, Appleyard tests all pass.
- 4-case shortlist runs without diverging.
- Case 3 fine-dt FOPT = 3827 (OPM converged ref 3826, within 0.03%).

## Measured results (4-case shortlist, three configurations)

| case | master (legacy CPR) | AMG only (master restriction) | Bundle B (db06fba, no AMG) | Bundle B + AMG |
|------|--------------------:|------------------------------:|----------------------------:|---------------:|
| 1: medium-water 1 step    | 4 / ~5,500   | 4 / 6,138 (+12%)  | 4 / 3,938 (-28%)   | **4 / 4,167** (-24%) |
| 2: medium-water 6 step    | 30 / ~50,000 | 30 / 53,893 (+8%) | **24 / 23,604** (-53%) | **24 / 26,336** (-47%) |
| 3: heavy-water 12x12x3    | 32 / ~4,300  | 32 / 3,471 (-19%) | 36 / 12,109 (+182%) | 36 / **10,420** (+143%) |
| 4: gas-rate 10x10x3       | 28 / ~1,800  | 28 / 1,510 (-16%) | 28 / 8,629 (+380%) | 28 / **7,323** (+307%) |
| Case 3 fine-dt FOPT       | 3826 (OPM)   | —                 | 3827               | **3827** (within 0.03%) |

Format: substeps / lin_ms (delta vs master).

## Honest read of what AMG bought us

**AMG alone (master Quasi-IMPES restriction): roughly neutral.** Small
case-3/4 wins (~16-19% lin_ms) where the system size and sparsity
play to AMG's strengths; small case-1/2 losses from the AMG setup
overhead exceeding BiCGStab's per-iter cost on the converging
small-mid systems. No headline win.

**AMG paired with Bundle B's summed-IMPES restriction:** small but
real lin_ms wins on the regression cases — case 3: 12,109 → 10,420
(-14%); case 4: 8,629 → 7,323 (-15%). **But the Bundle B regressions
are NOT eliminated** — case 3 is still +143% vs master, case 4 is
still +307% vs master. The case-2 win is also partly eaten by AMG's
per-call setup overhead (-53% → -47%).

The case 3 / case 4 "structural cost" we attributed to the coarse
solver in the Bundle B bisection memo is **only partly** there.
A meaningful fraction lives in the per-Newton-iter assembly /
preconditioner-build overhead that the summed-IMPES restriction
introduces (more nonzeros in the coarse matrix, larger CPR
application). AMG addresses the coarse-spectrum part but not the
assembly-overhead part.

## Bundle size impact

WASM `simulator_bg.wasm`: 783 KB master → 881 KB with AMG = **+98 KB
(+12.5%)**. Tree-shaking eliminated the unused crate when AMG wasn't
called; once AMG is in the call path the optimizer keeps it.

This is a small-but-non-trivial cost for a feature that's not yet
delivering net wins. If we shelve AMG, this 98 KB cost goes away.

## Why Path A doesn't (yet) unlock Bundle B

Three candidate explanations:
1. **scirs2-sparse defaults are wrong for our pressure systems.**
   `theta=0.25` strong-connection threshold and Classical
   interpolation are tuned for elliptic Laplacian benchmarks. Our
   coarse pressure systems with summed-IMPES restriction may have
   different spectral properties (more anisotropic, well-driven).
   A parameter sweep (theta ∈ {0.1, 0.25, 0.4}, max_levels ∈ {3, 5,
   8}, smoother ∈ {GS, JacobiWeighted}) might find a better point.
2. **Per-call setup cost is too high.** AMG hierarchy is rebuilt on
   every CPR application. Caching across applications within one
   Newton iter would cut this; caching across iters is unsafe
   because the Jacobian changes.
3. **The summed-IMPES restriction's case-3 / case-4 cost isn't
   linear-solver-spectrum.** It's per-iter assembly overhead. AMG
   doesn't help with that. Other features in Bundle B (perforation
   row scaling, assembly reuse) are the right levers, and AMG is
   orthogonal.

(1) is testable in 1-2 hours with a parameter sweep. (2) is testable
with a 30-min implementation of a `RefCell` cache. (3) is structural
and means AMG isn't the bottleneck.

## Decision options

Three paths forward:

1. **Tune AMG parameters + add caching** (1-2 days). If a parameter
   sweep shows a config that gives net case-3 / case-4 wins, the
   overall picture shifts. Worth trying before declaring AMG dead.
2. **Pivot to Path B** (port pyAMG SA from scratch, 5-7 days). The
   research brief noted SA is what OPM uses; Ruge-Stüben (what
   scirs2 implements) is a different family. SA might fit reservoir
   pressure systems better. Higher effort, higher upside if it
   works.
3. **Shelve AMG** as the lever. If the case-3/4 Bundle B regressions
   are mostly assembly-overhead (not coarse-solver-spectrum), then
   no AMG variant will help. Pivot to a different direction —
   maybe finally try OPM's Schur-complement well elimination or
   variable substitution on regime transitions (both flagged as
   high-priority in the original audit but never tried).

Recommended: **(1) first** as a 1-2 day bounded experiment, then
(2) only if the parameter sweep shows promise. Skip (3) unless both
(1) and (2) fail.

## Files of record

- This branch's commits.
- `worklog/amg-spike/` — A/B run logs (git-ignored under
  `/worklog/`).
- `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs` — AMG
  helper functions and dispatch update.
- `docs/FIM_AMG_INTEGRATION_PLAN.md` (master 48da61b) — original
  Phase 0/1 plan.
- `docs/FIM_CPR_BUNDLE_BISECTION.md` (experiment branch
  fim-cpr-summed-impes) — Bundle B context.
