Please review all md files related to FIM, especially most recent and propose what code should be investigated to identify why there are significant issues with convergence

Reviewed the FIM Markdown surface, with the most weight on the newest July 3 worklog/TODO entries plus the active status, convergence worklog, CPR/linear audits, bypass audit, Jacobian reuse report, and migration/cleanup docs.

**Main Read**
The current evidence says the convergence problem is no longer “unknown FIM weirdness.” It is concentrated in the interaction between Newton state management and the CPR/linear path:

1. **Top priority: CPR pressure extraction / preconditioner shape**
   Investigate [gmres_block_jacobi.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:625), especially `build_pressure_transfer_weights`, `build_block_jacobi_preconditioner`, and `solve_with_cpr_fine_smoother`.

   Why: recent Phase 8 found 42/45 iterative attempts on the heavy case fail as real dead states, not premature bailout. Blanket row scaling regressed `31 -> 241` substeps, so the next plausible path is not global scaling, but CPR-specific, equation-family-aware pressure weighting, likely closer to quasi-IMPES / dynamic row-sum weighting.

2. **Add better linear-offender diagnostics**
   Investigate [newton.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/newton.rs:2975) and the failure diagnostics emitted from [gmres_block_jacobi.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:1142).

   Why: Phase 8’s `FAIL-SITE-DETAIL` reports the nonlinear residual peak, not necessarily the row where Krylov/preconditioning is failing. Before another fix, identify whether the true linear offender is the same as `water@0` / `oil@430` / `water@1215` / `perf@1299`, or whether the current proxy is misleading.

3. **Newton state management around hard reservoir hotspots**
   Investigate [newton.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/newton.rs:711), [newton.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/newton.rs:1170), [newton.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/newton.rs:1279), and [newton.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/newton.rs:3161).

   Focus on Appleyard damping, OPM-style oscillation relaxation, nonlinear history stabilization, and residual-stagnation rejection. Do not start by loosening acceptance thresholds: the docs repeatedly show that widening stagnation acceptance or shaping retry factors regressed or no-oped.

4. **Timestep hotspot cooldown/replay only after the above**
   Investigate [timestep.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/timestep.rs:1017), [timestep.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/timestep.rs:1181), and [timestep.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/timestep.rs:1286).

   Why: many cooldown/memory edits were already tried. Releasing hotspot memory earlier made `water@1020` fail more often. The right question is why the hotspot re-fails after release, not how to clear memory faster.

5. **Well-source dominated cell rows**
   Investigate [wells_ad.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/wells_ad.rs:142), [assembly_ad.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/assembly_ad.rs:354), and [scaling.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/scaling.rs:24).

   Why: Phase 8 found well-hosting cell mass-balance rows dominated 2-3 orders of magnitude by well terms. The failed row-scaling experiment says “not blanket scaling,” but it does not close targeted well-aware CPR weighting or a well-hosting-cell treatment inside pressure extraction.

**Do Not Re-Try First**
Avoid these without new evidence: blanket row/column scaling, Jacobian reuse, per-cell/global extrapolation, water cross-step carryover, generic growth caps, softer retry factors, and above-tolerance stagnation acceptance. The docs have enough negative evidence there.

I did not change code or run diagnostics; this was a read-only review and investigation proposal.

------------------

Start with 1. Investigate [gmres_block_jacobi.rs (line 625)](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:625), especially build_pressure_transfer_weights, build_block_jacobi_preconditioner, and solve_with_cpr_fine_smoother.


I investigated `gmres_block_jacobi.rs` around the CPR path. No code changes made.

**Key Findings**
1. `build_pressure_transfer_weights` is the main suspect.
   At [gmres_block_jacobi.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:625), CPR always anchors the pressure restriction on local row `0`:

   `restriction[0] = 1.0`

   Then it locally Schur-eliminates rows `1..` using the cell transport block. That is mathematically coherent, but it is still “water-row first.” It is not summed-IMPES, dynamic row-sum, or quasi-IMPES weighting. This matches the older audit’s warning: if water mass balance is a poor pressure proxy near producer/front/well cells, CPR can be pressure-like in name but weak in the exact cells driving convergence trouble.

2. `build_block_jacobi_preconditioner` does promote well BHP rows and Schur-eliminate perforation-rate tails.
   See [gmres_block_jacobi.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:662). This is more advanced than the old “CPR incomplete” comment suggests. But it still builds the cell pressure coarse rows from those local water-row-derived restriction/prolongation weights.

3. Coarse pressure solve quality is diagnostic-only.
   At [gmres_block_jacobi.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:609), exact dense is used only up to 512 rows; larger coarse systems use BiCGSTAB+ILU0. Reduction ratios are recorded, but [solve_with_cpr_fine_smoother](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:1142) never changes behavior when pressure correction is poor. So bad CPR can keep feeding GMRES until `dead-state` / `max-iters`, then Newton falls back.

4. `FgmresCpr` currently uses full ILU0 as fine smoother, except over-threshold pressure systems use the historical `ilu0/post-bj` path.
   That policy is encoded at [gmres_block_jacobi.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:917). It may be justified by past regressions, but it is another important interaction point if CPR is failing specifically after coarse correction.

**Recommended Next Investigation**
Do not start with blanket scaling again. Start with a measurement-only probe comparing current row-0 Schur restriction against alternative pressure restrictions on the same assembled Jacobian:

- current: local row-0 Schur
- summed rows: `[1, 1, 1]`
- reservoir-volume / formation-volume weighted sum
- dominant accumulation row per cell
- quasi-IMPES-style weights derived from accumulation block

For each, log coarse RHS norm, coarse residual reduction, pressure correction norm, and post-preconditioner residual norm on the failing heavy case. That will tell us whether CPR is failing because the pressure equation is the wrong one, before touching Newton or timestep logic.

No tests run, since this was a read-only code audit.


-----------------------------------

A lot of tests were done already a lot of them were documented, how to keep trck of all of that not to repeat them again and again?

The cleanest way is to make a **single experiment registry** that every future FIM session must check before touching code.

Right now the knowledge is spread across `TODO.md`, `docs/FIM_CONVERGENCE_WORKLOG.md`, `docs/FIM_LINEAR_SOLVER_AUDIT.md`, `docs/FIM_BYPASS_AUDIT.md`, etc. That is why repeated attempts happen. The fix is not more narrative docs; it is an indexed ledger.

I’d create something like:

`docs/FIM_EXPERIMENT_REGISTRY.md`

With one row per attempted lever:

```md
| ID | Area | Hypothesis | Files touched | Cases run | Result | Verdict | Do not retry unless |
|---|---|---|---|---|---|---|---|
| FIM-ROW-SCALE-001 | Linear/CPR | Row infinity scaling improves conditioning | gmres_block_jacobi.rs | 12x12x3 dt=1, control matrix | 31 -> 241 substeps | REVERTED | only with equation-family-aware CPR weighting |
| FIM-JAC-REUSE-001 | Linear/direct | Reuse sparse LU within Newton rung | newton.rs, linear/mod.rs | medium/heavy/gas shortlist | lin_ms +2-7%, permissive +26% | REVERTED | bypass architecture changes first |
| FIM-STAG-WIDEN-001 | Newton | Accept near-tolerance stagnation | newton.rs | 20x20x3 water | fewer retries but more substeps/plateau | REVERTED | new residual-plateau cause identified |
```

Each entry should be short and searchable. The detailed story can stay in the old docs, but the registry is the gatekeeper.

I’d also add this rule to `.claude/skills/fim-solver-debug/SKILL.md`:

> Before proposing or implementing a FIM convergence change, search `docs/FIM_EXPERIMENT_REGISTRY.md` for the mechanism, files, and target case. If an equivalent experiment exists, do not repeat it unless the registry’s “Do not retry unless” condition is satisfied.

My recommended structure:

- `docs/FIM_STATUS.md`: current truth only.
- `docs/FIM_EXPERIMENT_REGISTRY.md`: all attempted levers, one-line outcomes.
- `docs/FIM_CONVERGENCE_WORKLOG.md`: active current-session traces.
- Old deep docs: evidence archive, linked from registry rows.
- `TODO.md`: short active tasks only, not experiment history.

This gives future agents one place to check before they accidentally rediscover the same wall with a different hat on.


----------------------------

Please do as per recommendations, add skill rule, and apply recommended docs structure


Implemented the FIM docs structure cleanup.

Added [docs/FIM_EXPERIMENT_REGISTRY.md](/home/coder/Repos/ressim/docs/FIM_EXPERIMENT_REGISTRY.md) as the searchable anti-repeat ledger, seeded with the high-risk promoted/reverted/refuted/open FIM experiments and retry conditions.

Updated:
- [.claude/skills/fim-solver-debug/SKILL.md](/home/coder/Repos/ressim/.claude/skills/fim-solver-debug/SKILL.md) to require checking/updating the registry.
- [docs/FIM_STATUS.md](/home/coder/Repos/ressim/docs/FIM_STATUS.md) with the new canonical source map and working rules.
- [docs/DOCUMENTATION_INDEX.md](/home/coder/Repos/ressim/docs/DOCUMENTATION_INDEX.md) with FIM doc ownership.
- [docs/FIM_CLEANUP_PLAN.md](/home/coder/Repos/ressim/docs/FIM_CLEANUP_PLAN.md) to include the registry in the cleanup end state.
- [TODO.md](/home/coder/Repos/ressim/TODO.md) with a short completed tracker note.

No tests run; this was Markdown/instruction-only.


------------------


Now work on

Key Findings
build_pressure_transfer_weights is the main suspect.
At [gmres_block_jacobi.rs (line 625)](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:625), CPR always anchors the pressure restriction on local row 0:
restriction[0] = 1.0
Then it locally Schur-eliminates rows 1.. using the cell transport block. That is mathematically coherent, but it is still “water-row first.” It is not summed-IMPES, dynamic row-sum, or quasi-IMPES weighting. This matches the older audit’s warning: if water mass balance is a poor pressure proxy near producer/front/well cells, CPR can be pressure-like in name but weak in the exact cells driving convergence trouble.

build_block_jacobi_preconditioner does promote well BHP rows and Schur-eliminate perforation-rate tails.
See [gmres_block_jacobi.rs (line 662)](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:662). This is more advanced than the old “CPR incomplete” comment suggests. But it still builds the cell pressure coarse rows from those local water-row-derived restriction/prolongation weights.

Coarse pressure solve quality is diagnostic-only.
At [gmres_block_jacobi.rs (line 609)](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:609), exact dense is used only up to 512 rows; larger coarse systems use BiCGSTAB+ILU0. Reduction ratios are recorded, but [solve_with_cpr_fine_smoother (line 1142)](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:1142) never changes behavior when pressure correction is poor. So bad CPR can keep feeding GMRES until dead-state / max-iters, then Newton falls back.

FgmresCpr currently uses full ILU0 as fine smoother, except over-threshold pressure systems use the historical ilu0/post-bj path.
That policy is encoded at [gmres_block_jacobi.rs (line 917)](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:917). It may be justified by past regressions, but it is another important interaction point if CPR is failing specifically after coarse correction.

Recommended Next Investigation
Do not start with blanket scaling again. Start with a measurement-only probe comparing current row-0 Schur restriction against alternative pressure restrictions on the same assembled Jacobian:
current: local row-0 Schur
summed rows: [1, 1, 1]
reservoir-volume / formation-volume weighted sum
dominant accumulation row per cell
quasi-IMPES-style weights derived from accumulation block
For each, log coarse RHS norm, coarse residual reduction, pressure correction norm, and post-preconditioner residual norm on the failing heavy case. That will tell us whether CPR is failing because the pressure equation is the wrong one, before touching Newton or timestep logic.


Implemented the measurement-only CPR restriction probe.

Changed:
- [gmres_block_jacobi.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs): added restriction variants and diagnostics for:
  `row0-schur`, `sum-rows`, `diag-balanced-sum`, `dominant-diag-row`, `local-schur-balanced`.
- [mod.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/linear/mod.rs): added `restriction_probe` option and diagnostic payload fields.
- [newton.rs](/home/coder/Repos/ressim/src/lib/ressim/src/fim/newton.rs): enables the probe only for verbose `FgmresCpr` Newton solves and emits compact `cpr_probe=[...]` trace entries.
- [FIM_EXPERIMENT_REGISTRY.md](/home/coder/Repos/ressim/docs/FIM_EXPERIMENT_REGISTRY.md): marked `FIM-LINEAR-005` as `DIAGNOSTIC`.

Important: the live CPR preconditioner still uses current `row0-schur`; this only measures alternatives on the same Jacobian/RHS.

Validation run:
- `cargo test --manifest-path src/lib/ressim/Cargo.toml gmres_block_jacobi -- --nocapture` passed, 24 tests.
- `cargo test --manifest-path src/lib/ressim/Cargo.toml failure_classification -- --nocapture` passed, 5 tests.


---------------------------

