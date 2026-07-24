# FIM Relperm-Endpoint Singularity — Scoping Analysis

Date: 2026-07-24. Tree: clean `eee4c14` (post convergence re-baseline). Status: **scoping only,
no code change**. This doc backs the `TODO.md` "relperm-endpoint singularity" root-cause item and
corrects a mis-scoping in its original framing.

## TL;DR

- The default FIM path evaluates relative permeability from a **21-point tabulated SWOF-style law**
  (`DEFAULT_FIM_COREY_TABLE_POINTS = 21`, WATER-020/021 OPM alignment), **not** the analytic Corey
  clamp. The live zero-derivative that forms the singular Jacobian block is in
  `corey_table_derivatives` / `corey_table_generic`, so the TODO's original fix location
  (`relperm.rs::k_rw`/`k_ro`'s `s_eff.clamp(0,1)`) is the **wrong target** for the shipped default —
  editing it alone would not remove the measured `linear-bad` retries.
- Every affected case **already converges** (green tests via the iterative fallback). This is a
  robustness/cleanliness item, not a correctness or red→green fix.
- OPM has the identical flat SWOF endpoints and never hits this, because its linear stack is
  iterative and never demands a full-rank exact factorization. ResSim only trips because of its
  **forced direct sparse-LU on small systems**. The honest root cause is that linear-routing choice,
  not the Corey endpoints — the "backstop" fallback is the OPM-consistent behavior.
- **Recommendation: do not perform the relperm-tail regularization as scoped.** Correct the TODO
  (done), keep the load-bearing fallback, and — only if this is ever prioritized — prefer the
  linear-routing fix (Option A below) over bending the validated SWOF curves (Option B).

## Mechanism (corrected)

Default relperm dispatch (`src/lib/ressim/src/mobility.rs:161-225`):

```
if self.fim_corey_table_points > 0 {           // default 21
    self.scal.corey_table(sw, points)                    // value
    self.scal.corey_table_derivatives(sw, points)        // f64 Jacobian slope
    self.scal.corey_table_generic(sw, points)            // AD Jacobian slope
}
```

`corey_table_derivatives` (`src/lib/ressim/src/relperm.rs:637`) returns `(0.0, 0.0)` for
`s_w <= s_wc || s_w >= 1 - s_or`. Chain:

1. `OpmAligned` (default flavor, WATER-026) + WATER-025 raw saturations let a well-connected cell
   sit at or below `Swc` (or at/above `1-Sor`).
2. The tabulated endpoint slope is exactly `0`, so that cell's mobility contributes no
   saturation-derivative — the corresponding Jacobian block is rank-deficient.
3. The forced direct sparse-LU used for systems under the direct-solve threshold cannot factor a
   singular block; it reports `reduction = 1.0`.
4. The iterative block-Jacobi/CPR fallback in `solve_linearized_system` (`fim/linear/mod.rs`,
   hardened in `c2167f2`) catches the failure and re-solves without an exact factorization. The
   step converges, but the attempt is logged as `linear-bad` and the substep fragments.

The analytic `k_rw`/`k_ro` clamp the TODO named is only reached when `fim_corey_table_points == 0`
(non-default). Both f64 and AD-generic mirrors, in both `RockFluidProps` and
`RockFluidPropsThreePhase`, share the same endpoint-flat structure.

## Evidence (clean tree `eee4c14`, browser wasm default = 21 table points)

`linear-bad` retries observed on the re-baseline (see `docs/SOLVER_COMPARISON_SUMMARY.md`), each the
backstop firing:

| Case | FIM (OpmAligned default) | Legacy | Interpretation |
| --- | --- | --- | --- |
| water-pressure 22×22×1 dt=0.25 | 11 substeps / **8** linear-bad | 4 / 0 | endpoint singularity, direct-LU fail → fallback |
| water-pressure 23×23×1 dt=0.25 | 6 / **4** linear-bad | 4 / 0 | same |
| sweep-areal 21×21×1 dt=0.25 | 6 / **4** linear-bad | — | same |
| water-pressure 12×12×3 dt=1 (heavy) | 4 / 0 | 24 / 0-4-0 | not endpoint-bound; unaffected |

Named regression tests, all **currently green via the fallback** (verified 2026-07-24):
`simple_pressure_control_public_step_has_same_stable_contract_on_both_solvers`,
`benchmark_like_substepping_completes_requested_dt`,
`shared_block_multiwell_public_step_remains_finite_on_both_solvers`.

## Is it worth it?

Low priority. It removes `linear-bad` log noise and minor substep fragmentation on a few small
well-dominated cases that **already converge and pass their contract tests**. It is not a
correctness fix and does not turn any red gate green. Weighed against the standing steer (track OPM
consistently; do not chase small deltas), perturbing validated OPM-aligned SWOF physics to rescue a
ResSim-specific direct-solve path is the wrong trade.

## Will it cause other issues? (risks of the relperm-tail approach)

1. **Touches OPM-alignment-critical physics.** The tabulated law is exactly what WATER-005/025 tuned
   so the injector water MB matches Flow (`0.313756` vs `0.31375`). A nonzero connate/residual-region
   slope introduces small nonphysical mobility ahead of the front and will move that number and the
   WATER-025 areal win.
2. **Bit-parity trap.** A value-flat-but-slope-nonzero floor breaks AD=FD consistency; a genuine
   value tail changes the residual. Either way the f64 and AD-generic mirrors must stay bit-identical
   across **both** `RockFluidProps` and `RockFluidPropsThreePhase`, plus the Stone2 composition where
   the endpoint tails compound inside a product of factors.
3. **Wrong-path trap (the original mis-scoping).** Fixing only `relperm.rs::k_rw`'s analytic clamp
   leaves the default (table) path unchanged; fixing only the table leaves the analytic path
   singular. A complete fix must cover the live table path first.
4. **Re-validation cost.** BL breakthrough gate, relperm unit gates, WATER-025 areal baseline, the
   8-10% oil-gap comparison, and the full wasm control matrix all need re-running.

## Options

| Option | What | Physics risk | Removes retries | Notes |
| --- | --- | --- | --- | --- |
| **A — linear routing (preferred if prioritized)** | Route small well-dominated / endpoint-singular systems to the iterative CPR path proactively instead of forcing direct LU; make the fallback the *designed* path. | None (linear routing only) | Yes | OPM-consistent; the fallback already proves the iterative path solves these. |
| **B — relperm endpoint tail** | Small ε-slope tail on the **tabulated** endpoints (`corey_table_derivatives` + `corey_table_generic`), value/derivative consistent, then the analytic clamp for the non-default path. | Higher — moves validated OPM SWOF | Yes | The TODO's intent, re-targeted to the correct functions. |
| **C — do nothing (current recommendation)** | Keep the load-bearing fallback and its `fim/linear/mod.rs` comment. Correct the mis-scoped TODO. | None | No (cases already pass) | The fallback is the OPM-consistent behavior. |

## Recommendation

**C now; A if/when prioritized; not B.** The fragility the TODO worried about — "if linear routing
is simplified away, the singularity resurfaces" — is best addressed by making iterative routing
explicit for these cases (A), which changes no physics, rather than by regularizing the SWOF curves
(B). Keep the fallback and its explanatory comment until A lands.
