# Bundle N: Replace the FIM Nonlinear Layer with OPM's, as One Coherent Bundle

Status: DESIGN (2026-07-07). Nothing here is implemented.
Companion cost bundle: "Bundle P" (preconditioner reuse), Part 2 below.
Motivating measurement: `docs/FIM_CONVERGENCE_WORKLOG.md` "Task #41 (2026-07-07)".

## 1. Why this bundle exists (the measured gap)

Heavy case (`water-pressure 12x12x3 --dt 1`), same machine, same day, commit `468a103`:

| | OPM Flow 2025.10 | ResSim FIM | Ratio |
|---|---:|---:|---:|
| Wall-clock for the 1-day step | 0.05 s | 36.9 s | **738x** |
| Timesteps taken | 1 (zero cuts) | 32 substeps + 13 retry rungs | — |
| Newton iterations | 11 | 336 | **30.5x** |
| Wall-clock per Newton iteration | 4.5 ms | 109 ms | **24x** |
| Work discarded (retries) | 0% | 32% of wall-clock | — |

`738 ≈ 30.5 x 24`: the gap splits cleanly into a **nonlinear-architecture factor (~30x)** and a
**per-iteration-cost factor (~24x)**. ResSim's per-attempt Newton effort (7.6 iters avg) is the
same order as OPM's per-day effort (11) — Newton itself is fine; the *multiplication* of attempts
is the problem. Bundle N addresses the 30x; Bundle P addresses the 24x. They are independent and
separately gateable.

### Root cause of the 30x, named precisely

ResSim's Newton acceptance (`scaled_residual_inf_norm`, `fim/newton.rs:1425`, tolerance `1e-5`)
requires **the single worst cell in the grid** to pass `1e-5`. OPM's shipped defaults (verified
from `/usr/bin/flow --help-all`, not from memory):

| Criterion | OPM default | ResSim today |
|---|---|---|
| Global mass balance (`tolerance-mb`) | `1e-7`, **volume-averaged**, relaxed `1e-6` | `1e-5` inf-norm (no averaging) |
| Local error (`tolerance-cnv`) | `0.01` per cell, **max of local saturation errors** | `1e-5` per cell (same inf-norm) |
| Relaxed local tier (`tolerance-cnv-relaxed`) | `1` — used once the Newton budget is exhausted (`min-strict-cnv-iter=-1`) | none |
| Pore-volume exemption (`relaxed-max-pv-fraction`) | `0.03` — 3% of PV may violate CNV **even during strict iterations** | none |
| Well equations (`tolerance-wells`) | `1e-4` | folded into the same `1e-5` inf-norm |

ResSim is ~1000x stricter locally than OPM, with no relaxed tier and no PV exemption. Every
plateau-cell retry ladder in the diagnostics (`water@1215` etc.) is a single cell above `1e-5`
that OPM's criteria accept. The whole compensating-mechanism stack — entry guards, zero-move
acceptance gates, hotspot streak memory, plateau replay, retry-family classification, the
inflection chop's load-bearing role — exists to manage the fallout of this one over-strict
criterion plus the single global damping scalar (Task #37's root cause). This is why
individually-correct local fixes keep cancelling out (`FIM-NEWTON-005`/`007`,
`FIM-LINEAR-001`/`009`): the mechanisms brace against *each other*, not against the physics.

## 2. Design principle (standing user directive)

> Do not do trial-and-error; ensure ResSim consistently implements OPM Flow logic. Individual
> fixes may not work if the approach is not consistent — errors cancel out, and individually
> correct fixes regress. Step-by-step trial and immediate revert do not help.

Consequences for this bundle:

1. **The bundle is judged only at its end state, on end metrics** (§5). Intermediate sub-steps
   MAY regress current-architecture baselines — that is expected, not a stop signal. The
   promote-or-revert decision happens once, for the whole bundle.
2. **The current control matrix's substep counts are baselines of the old architecture.** They
   are diagnostic context during development, never gates for this bundle. New baselines get
   derived after promotion.
3. **Old registry verdicts do not veto bundle items.** `FIM-NEWTON-004` ("do not widen stagnation
   acceptance") and the graveyard of acceptance-related reverts were verdicts against *piecemeal
   widening inside the old architecture*; replacing the criterion wholesale with OPM's is exactly
   the "different angle" those rows deferred to. Annotate, don't obey blindly (§7).
4. **Implementation is flag-gated** (`FimNonlinearFlavor::{Legacy, OpmAligned}` on
   `FimNewtonOptions`), so Legacy remains bit-identical and A/B comparison at end metrics is one
   flag flip. Legacy is deleted only after promotion, in a separate commit.

## 3. Bundle N contents (the nonlinear layer, ~30x factor)

### N1 — OPM convergence criteria (MB + CNV + relaxed tier)

Replace the single `scaled_residual_inf_norm <= 1e-5` acceptance with OPM's two-criterion test,
per component (water / oil-component / gas-component):

- **MB (global)**: pore-volume-weighted average of the *signed* residuals, relative to mass in
  place; tolerance `1e-7`, relaxed `1e-6`.
- **CNV (local)**: per-cell residual normalized by that cell's pore volume over dt (a
  dimensionless local saturation-error); tolerance `0.01` on the max, EXCEPT cells jointly
  holding up to `3%` of total pore volume, which are exempt. Relaxed tier `1.0` applies when the
  Newton budget is exhausted (`min-strict-cnv-iter=-1` semantics).
- **Wells**: `tolerance-wells=1e-4` on the well-constraint/perforation families (they leave the
  cell inf-norm entirely).

Implementation note: the exact CNV/MB formulas must be ported from `opm-simulators`
(`BlackoilModel::getReservoirConvergence` and `localConvergenceData`), not re-derived — the
tolerances above are meaningless under a home-grown normalization. ResSim already has per-cell
pore volume and `equation_scaling`; the port is a normalization change, not new plumbing.
`material_balance_tolerance` (already separate in `FimNewtonOptions`) becomes the MB check.

### N2 — Per-cell update chopping replaces the global damping scalar

Replace `appleyard_damping_breakdown`'s single global factor with OPM's
`updatePrimaryVariables_()` semantics: clamp **each cell's own update** to `ds-max=0.2`
(absolute saturation change) and `dp-max-rel=0.3` (relative pressure change). No cell restricts
any other cell's movement. Task #37 established the global scalar is the root cause of the
chaotic `k`-sensitivity; this removes that mechanism class entirely.

- The **fw-inflection chop is deleted** in the same change. `FIM-DAMP-002` (April) proved
  removing it *under the old architecture* lost both speed and accuracy — but that architecture
  used global damping and strict acceptance, where the chop was load-bearing. Under per-cell
  chopping + CNV acceptance its role is expected to be covered by `ds-max`; the fine-dt FOPT gate
  (§5) is the arbiter. If accuracy fails the gate, re-adding the chop *per-cell* is the recorded
  fallback (one bounded retry, not a tuning campaign).
- The Phase-7 oscillation detector + relaxation scalar (`detect_oscillation`,
  `FIM-NEWTON-001`/`006`) **stays** — it is already the OPM port and composes with per-cell
  chopping in OPM itself.

### N3 — OPM timestep controller replaces the retry ladder

Replace the retry-ladder/hotspot-memory controller (`fim/timestep.rs`) with OPM's shipped
scheme (`time-step-control=pid+newtoniteration`):

- PID-style dt proposal from relative state change, plus the Newton-iteration target: grow by
  `1.25` when iterations < `target-newton-iterations=8`, decay by `0.75` when above.
- On genuine nonconvergence (budget exhausted AND relaxed criteria still failing): dt x `0.33`
  (`solver-restart-factor`), max `10` restarts. No failure-family classification, no
  hotspot-repeat memory, no plateau replay, no carryover budgets.
- `newton-min-iterations=2`, `newton-max-iterations=20` (ResSim's cap already matches).

### N4 — Delete the compensating mechanisms (same change, not one at a time)

With N1-N3 in place, delete from the `OpmAligned` path: entry guards
(`NOOP_ENTRY_EXACT_FACTOR`/`ENTRY_RESIDUAL_GUARD_FACTOR`), zero-move Appleyard acceptance,
residual-stagnation bailout tiers, producer-hotspot bailout, hotspot streak tracking + site-keyed
history stabilization remnants, plateau-replay bookkeeping, retry-family classification
(`classify_retry_failure_with_site`) as a *control* input (keep as trace-only diagnostics), and
the direct-solve bypass ladder (see N5). Deleting them piecemeal is the already-failed pattern;
they go together because their reason to exist (over-strict acceptance + global damping) goes.

### N5 — OPM linear-failure handling

Today a non-converged iterative solve triggers a direct sparse-LU fallback (expensive) or a
retry-rung. OPM instead: accepts the iterative result if it achieved
`relaxed-linear-solver-reduction=0.01`; otherwise the *Newton/timestep* layer handles it
(`linear-solver-ignore-convergence-failure=false` aborts only on truly failed solves). In the
`OpmAligned` path: keep the Newton direction whenever the relaxed reduction was met (ResSim's
`should_accept_near_converged_iterative_step` generalizes to this), drop the direct-LU fallback
ladder from the hot path (keep direct solvers for debug backends only).

### Explicitly NOT in Bundle N

- **Variable substitution** (OPM's regime switching inside Newton; gap #5 of
  `docs/FIM_OPM_GAP_ANALYSIS_SPE1.md`): assembly-layer change, matters most for three-phase gas
  cases, orthogonal to the measured 30x. Candidate follow-on ("Bundle N2") only after Bundle N
  settles.
- **AMG**: per-application coarse quality is already ~1e-7 at current sizes (Task #41 traces);
  AMG is a scale-up item, unchanged from `FIM-LINEAR-006`'s deferral.
- IMPES, assembly/AD, well Schur elimination, CPR restriction — all untouched.

## 4. Part 2, "Bundle P": per-iteration cost (~24x factor, independent)

89% of heavy-case wall-clock is preconditioner **build** (`pc_ms=32867/36706`): quasi-IMPES
weights + block-ILU0 + an O(n³) dense coarse inverse, rebuilt every Newton iteration. OPM's
default is `cpr-reuse-setup=4`: reuse the CPR setup, fully recreate every `cpr-reuse-interval=30`
linear solves (option 2: recreate when a solve exceeds 10 iterations — simpler and also shipped).

- **P1**: cache `FimCprPreconditioner` across Newton iterations within a substep; recreate on
  OPM's triggers (every N solves, or after a >10-iteration solve, or on dt change). The Jacobian
  changes between iterations but CPR setup tolerates lagged preconditioners by design — this is
  literally what OPM ships.
- **P2**: if the dense coarse path survives P1 profiling: replace the explicit `try_inverse()`
  with an LU factorization (solve per application instead of explicit inverse; ~3x cheaper build,
  same apply cost).

Bundle P is gated conventionally (it must be behavior-preserving up to linear-solver noise):
control matrix + locked smoke + wall-clock. It can land before or after Bundle N. Expected
combined effect of N+P on the heavy case: 36.9 s → well under 1 s.

## 5. Gates (end metrics only)

Bundle N promotes if, on the final post-cleanup tree:

1. **Physics — hard gates:**
   - Locked smoke 3/3 (`spe1_fim_first_steps_converge_without_stall`,
     `spe1_fim_gas_injection_creates_free_gas`, `drsdt0_base_rs_cap_flashes_...`).
   - Buckley-Leverett benchmark tests green, unchanged tolerances.
   - Fine-dt FOPT (April methodology, `--steps 16 --dt 0.0625`) within **0.5%** of the current
     bundle's own fine-dt answer (3883.47) AND no further from OPM's converged 3826.12 than
     today's +1.50%. (CNV `1e-2` is looser locally; MB `1e-7` is *tighter* globally than today's
     `1e-5` — net accuracy drift must be measured, not assumed.)
   - Coarse-dt vs fine-dt self-consistency: heavy-case `dt=1` FOPT within ~2% of its own fine-dt
     answer.
2. **Efficiency — the point of the bundle:**
   - Heavy case Newton iterations for the 1-day step: **≤ 35** (~3x OPM's 11; today: 336).
   - Timestep cuts: **≤ 1** (today: 13 retry rungs).
   - Wall-clock: **≤ 4 s** without Bundle P; **≤ 1 s** with it (today: 36.9 s).
3. **Breadth:** the other 5 control-matrix cases and the SPE1 FIM path complete without new
   failure modes (substep counts are *re-derived as new baselines*, not compared against old
   ones), and production totals stay within the same ~2% band vs their own fine-dt references.

Any gate failing → the bundle as a whole is reworked or reverted; no salvaging individual pieces
into the Legacy path (that is the piecemeal pattern this design exists to end).

## 6. Build order (checkpoints, not gates)

0. **Port fidelity pass**: read the OPM sources for each item (`getReservoirConvergence`,
   `updatePrimaryVariables_`, `AdaptiveTimeStepping`/`PIDAndIterationCountTimeStepControl`,
   linear-failure handling) and write the exact formulas into this doc before coding. The
   installed Flow's `--help-all` defaults (Task #41 worklog) are the parameter truth.
1. **N1 inert**: compute CNV/MB alongside the existing criterion, trace-only
   (`CNV-MB would_accept_iter=K actual_iter=M`). One heavy-case run quantifies how many
   iterations/substeps the old criterion wastes — the direct empirical check on §1's root-cause
   claim, before any behavior changes.
2. **Flag on** (`OpmAligned`): N1 acceptance + N5 linear handling.
3. N3 controller.
4. N2 per-cell chopping (+ inflection-chop deletion).
5. N4 mechanism deletion sweep.
6. End-metric evaluation (§5), A/B against Legacy. Promote → delete Legacy path in a follow-up
   commit; re-derive control-matrix baselines; update `FIM_STATUS`/registry/skill docs.
7. Bundle P (independent; conventional gates).

Checkpoints 2-5 are commits on a branch with the flag defaulting to Legacy; only step 6 flips
the default. No intermediate checkpoint is judged on old-architecture baselines.

## 7. Documentation consequences

- Registry rows whose verdicts assumed the old acceptance/damping architecture get an annotation
  ("verdict under pre-Bundle-N architecture") at promotion time — notably `FIM-NEWTON-004`
  (acceptance widening), `FIM-DAMP-002/003/004` (inflection-chop tuning), `FIM-NEWTON-007`.
  `FIM-DAMP-004`'s `k=1.25` constant is deleted along with the chop (N2).
- The `fim-solver-debug` skill's control matrix and "known-reverted lever classes" get rewritten
  against the new architecture after promotion.
- One registry row for the whole bundle: `FIM-BUNDLE-N` (plus `FIM-BUNDLE-P`), honest verdict
  either way.

## 8. Risks

| Risk | Mitigation |
|---|---|
| CNV 1e-2 loosening degrades physics accuracy | MB 1e-7 is tighter than today globally; fine-dt FOPT hard gate (§5.1); relaxed tier only fires on budget exhaustion |
| Per-cell chopping without the inflection chop overshoots fw basins | `ds-max=0.2` bounds saturation moves; recorded single-retry fallback: per-cell inflection chop |
| Big-bang change is hard to review | Flag-gated parallel path; Legacy stays bit-identical until promotion; per-checkpoint commits |
| Port drift from OPM semantics | Step 0 fidelity pass against OPM source; parameters pinned to installed-binary defaults |
| Old baselines/registry become misleading | Explicit §7 annotation pass is part of promotion, not optional cleanup |
