# Bundle N: Replace the FIM Nonlinear Layer with OPM's, as One Coherent Bundle

Status: Checkpoints 0-6 implemented and gated (inert by default, `OpmAligned` opt-in).
**§5 end-metric evaluation (2026-07-09): heavy case FAILS decisively — DO NOT PROMOTE as-is.**
18,002 substeps (vs the `≤35` gate) traced to a specific, scoped root cause (§5 write-up below)
— not a reason to distrust N1/N2/N4/N5, which each showed real wins in isolation. See worklog
"Bundle N §5 end-metric evaluation (2026-07-09)" for full diagnosis and recommended fix.
Companion cost bundle: "Bundle P" (preconditioner reuse), Part 2 below.
Motivating measurement: `docs/FIM_CONVERGENCE_WORKLOG.md` "Task #41 (2026-07-07)".
Exact formulas: §9 (verified against `opm-simulators` tag `release/2025.10/final`, the same
release as the installed `/usr/bin/flow`; commit `b8b2b9e`). Where §3's prose and §9 differ,
**§9 wins** — notably the timestep controller, whose original sketch here was wrong.

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

ResSim is ~1000x stricter locally than OPM, with no relaxed tier and no PV exemption. The whole
compensating-mechanism stack — entry guards, zero-move acceptance gates, hotspot streak memory,
plateau replay, retry-family classification, the inflection chop's load-bearing role — exists to
manage the fallout of this criterion plus the single global damping scalar (Task #37's root
cause). This is why individually-correct local fixes keep cancelling out
(`FIM-NEWTON-005`/`007`, `FIM-LINEAR-001`/`009`): the mechanisms brace against *each other*,
not against the physics.

**Checkpoint-1 correction (2026-07-07, measured — see worklog "Bundle N checkpoint 1"):** the
inert CNV/MB measurement on the heavy case shows the criteria difference is NOT the direct
source of the 30x on ResSim's *current* trajectories: CNV is never the binding constraint
(0 of 44 Newton blocks), OPM's criteria would have saved ~0 iterations, and 35 of 44 blocks fail
OPM's MB `1e-7` — because the *global-scalar-damped* Newton stalls at MB ≈ `2e-6` at local
plateaus and stops contracting at all. The 30x lives in the update/damping dynamics (N2) and the
retry ladder those stalls trigger (N3); the criteria (N1) matter as the piece that lets a
post-N2 trajectory exit at the right time, not as a standalone win. This is measured
confirmation that the bundle must land as a whole — N1 alone would regress (OPM's MB `1e-7` is
tighter than 23 of 31 current accepted-substep exit states).

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

Exact formulas: §9.1 (ported verbatim from `BlackoilModel::getReservoirConvergence` /
`localConvergenceData` / `getMaxCoeff` / `characteriseCnvPvSplit` at the pinned tag — the
tolerances are meaningless under a home-grown normalization). ResSim already has per-cell pore
volume and `equation_scaling`; the port is a normalization change, not new plumbing.
`material_balance_tolerance` (already separate in `FimNewtonOptions`) becomes the MB check.

### N2 — Per-cell update chopping replaces the global damping scalar

Replace `appleyard_damping_breakdown`'s single global factor with OPM's
`updatePrimaryVariables_()` semantics: clamp **each cell's own update** to `ds-max=0.2`
(absolute saturation change) and `dp-max-rel=0.3` (relative pressure change). No cell restricts
any other cell's movement. Task #37 established the global scalar is the root cause of the
chaotic `k`-sensitivity; this removes that mechanism class entirely. Exact semantics (including
the implied-`So` delta in the per-cell max): §9.2.

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
scheme (`time-step-control=pid+newtoniteration`). **Correction from step 0**: the original
sketch here ("grow 1.25 / decay 0.75") described the *simple* `iterationcount` controller, not
the default. The real default is `dt_next = min(PID estimate, iteration-target estimate)` with
damping-factor formulas — exact port target in §9.3. On genuine nonconvergence: dt x `0.33`
(`solver-restart-factor`), max `10` restarts, growth clamps `3x`/`2x` (§9.4). No failure-family
classification, no hotspot-repeat memory, no plateau replay, no carryover budgets.
`newton-min-iterations=2`, `newton-max-iterations=20` (ResSim's cap already matches).

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
(`linear-solver-ignore-convergence-failure=false` aborts only on truly failed solves). Exact
semantics: §9.5. In the `OpmAligned` path: keep the Newton direction whenever the relaxed
reduction was met (ResSim's `should_accept_near_converged_iterative_step` generalizes to this),
drop the direct-LU fallback ladder from the hot path (keep direct solvers for debug backends
only).

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

### 5.1 Evaluation result (2026-07-09): FAILED — heavy case, root cause identified

Ran natively in `--release` (the wasm diagnostic runner's I/O buffering made this case
inconclusive at checkpoints 3-6): **18,002 substeps**, `176m25s` wall-clock, `linear-bad=7
nonlinear-bad=2` retries. Catastrophically over every efficiency gate above. Per this
section's own rule, **Bundle N does not promote in its current form.**

**Root cause, verified against both the ResSim trace and the OPM source directly (not
assumed):** the run enters a compounding dt-collapse once a producer well hits its BHP limit
and the system reaches steady state (`max_dSat=0.0000 max_dP=0.00`, but `iters=20`, hitting the
Newton cap on the well's rate-vs-BHP complementarity residual, which does not shrink with dt
since it's a discrete control condition, not a smooth PDE term). N3's iteration-count growth
formula (checkpoint 4) applies its full penalty regardless (`its=20 > target=8 → growth=0.4`),
and because the same well-pinned state persists across many consecutive substeps, the penalty
compounds (`0.4^N → 0`).

Confirmed via direct OPM source read that this is a genuine **architecture mismatch**, not a
formula bug: OPM resolves well-control switching through a *dedicated inner iteration loop*
(`WellInterface::iterateWellEquations` / `StandardWell::iterateWellEqWithControl`/
`iterateWellEqWithSwitching`, `opm/simulators/wells/{WellInterface,StandardWell}.hpp`) invoked
*within* a single outer reservoir Newton iteration — its `total_newton_iterations` (the exact
quantity fed to `computeTimeStepSize`, confirmed at `BlackoilModel_impl.hpp:270` and
`AdaptiveTimeStepping_impl.hpp` `getNumIterations_`) counts only outer reservoir iterations;
well-switching cost is invisible to it. ResSim's FIM solver has no such split — one flat Newton
loop over reservoir + well + perforation unknowns (Schur-eliminated at the *linear* level per
Phase 11, not the *nonlinear* level). Porting OPM's growth formula literally, using ResSim's
own combined iteration count, punishes well-switching cost as "physics moving too fast" — a
category error OPM's own architecture never exposes.

This independently re-derives **"Hypothesis A"** from the original Phase 8/9 well-coupling
investigation (`docs/FIM_CONVERGENCE_ARCHIVE_*`) from a completely different angle (timestep
controller, not linear solver) — a second, convergent line of evidence that ResSim's flat
well/reservoir coupling is the real remaining architectural gap, not any single mechanism's
tuning.

**Scope of the finding:** narrow. N1/N2/N4/N5 each showed real, measured wins in isolation on
`22x22x1`/`23x23x1` (checkpoints 5-6) — this does not undo those. N3's specific formula (using
the *combined* count) is the identified defect. The heavy case is disproportionately exposed
because its single producer/injector geometry reaches a well-pinned steady state partway
through the step and stays there; cases without a hard-limited well would not trigger this.

**Follow-up (2026-07-09) — two fix attempts, both ruled out:**

1. *Decouple well/perforation iteration count from N3's growth formula.* Re-examined before
   implementing: N1's acceptance check (`opm_conv.would_accept`) already excludes well/
   perforation rows entirely and is the *only* path to acceptance under `OpmAligned`
   (checkpoint 5 removed every other exit) — so `report.newton_iterations` fed to N3 already
   reflects reservoir-only convergence timing, by construction. Nothing to decouple. **Ruled
   out by code inspection, not implemented.**
2. *Chop the well-BHP update* (OPM's `--dbhp-max-rel`, ported verbatim from
   `StandardWellPrimaryVariables.cpp::updateNewton`). Implemented, gated (no-op under Legacy,
   `22x22x1`/`23x23x1` unchanged under `OpmAligned`), and tested on the heavy case natively:
   **`accepted_substeps=18002` — bit-identical to the unfixed run, down to `min_dt`/`max_dt`
   matching to the exact same floating-point bits.** The chop had zero effect and apparently
   never engaged (the well's raw BHP delta never exceeded the cap in this scenario). **The
   hypothesis that BHP itself was the oscillating variable is refuted.**

Two well-reasoned, OPM-verified fixes ruled out (one by inspection, one empirically at ~5 hours
of compute across two full native runs) without identifying the actual oscillating mechanism.
Continuing to guess at a third fix blind would repeat the trial-and-error pattern this bundle's
own discipline exists to avoid. Untested candidates: the perforation-rate variable (deliberately
left unchopped to match OPM, which also has no `WQTotal` clamp — but ResSim's architecture may
not transfer that choice cleanly); ResSim's own `relax_well_state_toward_local_consistency`
post-processing step (no direct OPM counterpart, runs after every Newton update, never examined
by the OPM-fidelity review). **Recommended next step: build cheaper diagnostic visibility**
(e.g. a native test that writes the full trace to a file for the specific late-time window,
since `max_dt=0.185` days implies the pathology is concentrated very close to the end of the
simulated day) before attempting a third live fix — not another blind multi-hour guess.

Full write-up: `docs/FIM_CONVERGENCE_WORKLOG.md` "Bundle N §5 end-metric evaluation
(2026-07-09)", "Bundle N §5 follow-up: well-BHP update chop (opm-aligned only)",
"Bundle N §5 follow-up — well-BHP chop fix REFUTED (2026-07-09)".

## 6. Build order (checkpoints, not gates)

0. **Port fidelity pass — DONE 2026-07-07**: OPM sources read at the pinned release tag and the
   exact formulas recorded in §9. The installed Flow's `--help-all` defaults (Task #41 worklog)
   are the parameter truth. One design-level correction came out of it (N3's controller formula)
   plus several load-bearing details nobody's memory had: the implied-`So` delta in the chop, the
   `converged-before-solve` iteration structure, the `reduction ≥ relaxed` linear acceptance, and
   the 3%-PV rule applying at *every* iteration (not only on exhaustion).
1. **N1 inert — DONE 2026-07-07**: CNV/MB computed alongside the existing criterion, trace-only
   (`CNV-MB` line per iteration; behavioral no-op verified: control matrix + heavy case
   bit-identical, locked smoke 3/3). Result (worklog "Bundle N checkpoint 1"): criteria-swap
   alone saves ~nothing; the damped-Newton stall at MB≈2e-6 is the measured waste. Build order
   below reordered accordingly (N2 first) — development order only; §5 end gates unchanged.
2. **Flag on (`OpmAligned`) + N2 per-cell chopping — DONE 2026-07-07** (worklog "Bundle N
   checkpoint 2"). Default-Legacy no-op gate passed (control matrix + heavy bit-identical,
   smoke 3/3). Informational `--opm-aligned` heavy run: end-to-end worse under Legacy gates
   (226 substeps — expected for the mismatched intermediate), but the CNV-MB probe shows the
   MB stall is fixed: 95% of solve attempts now reach a full-OPM-rules-acceptable state
   (vs 48% under Legacy damping); median stall MB `2.2e-6 → 2.9e-7`. The fragmentation is now
   the Legacy acceptance layer rejecting OPM-acceptable states — checkpoint 3 harvests it.
3. **N1 acceptance criteria + N5 linear handling — DONE 2026-07-07** (worklog "Bundle N
   checkpoint 3"). Default-Legacy no-op gate passed. A real bug was caught and fixed during
   this checkpoint's own gating (two more Legacy-only acceptance shortcuts — a mid-iteration
   "tiny update" exit and the zero-move-Appleyard rescue — plus the post-loop exhaustion
   check — were not yet gated on `opm_aligned`; a first small-case test run showing zero
   `OPM-CONVERGED` traces caught it before any number was trusted). Informational
   `--opm-aligned` runs: `22x22x1` (bounded, comparable size to heavy) worse than Legacy
   (14 substeps vs 4) — expected, since the Legacy retry ladder doesn't yet know how to drive
   dt for the new acceptance dynamics. **Heavy case timed out past 280s** (not investigated
   further live — an intermediate, known-mismatched state; `MAX_SUBSTEPS=100_000` means slow,
   not hung). Consequence: N3 is now load-bearing, not optional polish — the Legacy retry
   ladder is actively mismatched with N1/N2/N5, not merely neutral to them.
4. **N3 controller — DONE 2026-07-07** (worklog "Bundle N checkpoint 4"). Default-Legacy
   no-op gate passed. Informational `22x22x1` run: **worse than checkpoint 3, not better**
   (20 substeps/12 retries vs 14/7) — the stubborn retry site (`oil@415`) needs a more
   aggressive dt cut than OPM's flat `solver-restart-factor=0.33` to escape, which is exactly
   what Legacy's repeated-hotspot-acceleration (down to `0.2`) was tuned to provide and N3
   deliberately does not replicate. Not chased further live (still an intermediate state).
   Open question carried into N4/§5: does the full bundle resolve this, or is OPM's retry
   backoff genuinely a small permanent trade-off?
5. **N4 mechanism deletion sweep — DONE 2026-07-07 (first real win)** (worklog "Bundle N
   checkpoint 5"). Deleted, `OpmAligned`-only: `candidate_materially_changed` as a validity
   requirement (the dominant fix — checkpoint 4's own forensics showed 11/12 failures on the
   tracked case were this exit firing on a harmless near-zero update); the residual-stagnation
   trend bailout (`stagnation_count>=3`); the preemptive direct-solve bypass ladder (closing a
   gap checkpoint 3 left: `repeated_zero_move_direct_bypass` didn't depend on `used_fallback`
   like the other three bypass flags, so could still silently force a direct solve ahead of
   any FGMRES-CPR attempt). Default-Legacy no-op gate passed. **`22x22x1` informational run: a
   real, measured win** — 12 substeps/1 retry (was 20/12 at checkpoint 4), `DAMPING FAILED`
   dropped from 11 to exactly 0 as predicted, now close to Legacy's 4/2. Two things NOT yet
   resolved: the heavy case still times out (>400s, even at `--diagnostic summary`) — a
   different pathology class (`water@1215`, genuine plateau) from the small case's site; and a
   third case (`23x23x1`) surfaced a NEW failure mode, `linear-bad` retries (not
   `nonlinear-bad`) at `retry_dom=linear-bad:oil@361` — suggesting N5's linear-failure handling
   itself may need a second look before §5, not just N1-N4.
6. **N5 bug fix — DONE 2026-07-07** (worklog "Bundle N checkpoint 6"). The `linear-bad` finding
   was a genuine bug, not a design gap: N5's reduction check read
   `failure.outer_residual_norm` (which can stay pinned at `rhs_norm` on a solve that never
   converges, even after later restarts genuinely improved) instead of
   `FimLinearSolveReport::final_residual_norm` (the actual candidate residual, matching Dune
   ISTL's `result.reduction` semantics). Confirmed directly: every failure on `23x23x1`
   reported `reduction=1.000e0` regardless of residual magnitude. One-line fix. `23x23x1`:
   `26/9` → `12/1` attempts, `LINEAR-ACCEPT` now correctly fires. `22x22x1` unchanged (its
   remaining retry was already `nonlinear-bad`). **Heavy case still times out** — its
   dominant failure is `nonlinear-bad` (Task #37's `water@1215` plateau), a different class
   this fix doesn't touch. No-op gate preserved (code only reachable under `opm_aligned`).
7. End-metric evaluation (§5), A/B against Legacy. Promote → delete Legacy path in a follow-up
   commit; re-derive control-matrix baselines; update `FIM_STATUS`/registry/skill docs.
8. Bundle P (independent; conventional gates).

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
| Deleting the direct-LU ladder loses a step-rescue OPM doesn't have | Real but narrow: exact LU occasionally powers through a step OPM's dt-cut would fragment. Counterweights (2026-07-07 review): the ladder *masked* the 0%-converging `row0-schur` restriction for months (42/45 silent fallbacks, Phase 8) because sims "worked"; LU doesn't scale past benchmark sizes; an exact direction on a bad linearization is confidently wrong. If §5 gates show step fragmentation traced to a solve LU would have survived, reintroduce ONE explicit rescue — never the ladder |
| Big-bang change is hard to review | Flag-gated parallel path; Legacy stays bit-identical until promotion; per-checkpoint commits |
| Port drift from OPM semantics | Step 0 fidelity pass against OPM source (§9, done); parameters pinned to installed-binary defaults |
| Old baselines/registry become misleading | Explicit §7 annotation pass is part of promotion, not optional cleanup |

## 9. Verified OPM formulas (step 0, 2026-07-07)

Source: `opm-simulators` tag `release/2025.10/final` (commit `b8b2b9e`), matching the installed
`/usr/bin/flow` (Flow 2025.10). File/line references below are to that tag. Parameter defaults
were independently confirmed from `/usr/bin/flow --help-all` (Task #41 worklog).

Notation: cells `i`, components `c ∈ {water, oil-component, gas-component}`; `R[i][c]` = OPM's
assembled residual entry (surface-volume rate units — ResSim's residual is the same quantity
scaled; the port must use the RAW residual, not `equation_scaling`-divided values);
`pv_i = referencePorosity_i * cellTotalVolume_i`; `dt` = substep length.

### 9.1 Convergence criteria (`BlackoilModel_impl.hpp`)

Per Newton iteration, from `localConvergenceData` (l.604) + `getMaxCoeff` (l.1083) +
`getReservoirConvergence` (l.740):

```
B_avg[c]    = (1/N_cells) * Σ_i  B_i,c          # B_i,c = 1/invB = FVF of phase c in cell i
R_sum[c]    = Σ_i  R[i][c]                       # SIGNED sum — cancellation intended
maxCoeff[c] = max_i |R[i][c]| / pv_i

CNV[c] = B_avg[c] * dt * maxCoeff[c]                       # local, dimensionless
MB[c]  = |B_avg[c] * R_sum[c]| * dt / pvSum                # global, dimensionless
```

Failure levels (l.870-905): `NaN` residual → hard failure; `res > max-residual-allowed (1e7)` →
TooLarge failure; both make the iteration count as failed (→ dt cut path, §9.4).

**Relaxed-tolerance activation** (l.775-815) — three independent triggers, evaluated every
iteration:

1. `iteration == maxIter` and `min_strict_mb_iter == -1` → use `tolerance-mb-relaxed (1e-6)`.
2. `iteration == maxIter` and `min_strict_cnv_iter == -1` → use `tolerance-cnv-relaxed (1.0)`.
3. **The 3%-PV rule (CNV only, ANY iteration)** — from `characteriseCnvPvSplit` (l.655): per
   cell compute `maxCnv_i = (dt/pv_i) * max_c(|R[i][c]| * B_avg[c])`, bucket cells into
   {≤ tol_cnv, ≤ tol_cnv_relaxed, > relaxed} by pore volume; if
   `PV(bucket2 + bucket3) < relaxed-max-pv-fraction (0.03) * eligiblePV`, the relaxed CNV
   tolerance applies to the whole check *right now*. This is the mechanism that makes isolated
   plateau cells (ResSim's `water@1215` class) a non-event in OPM.

Converged iff every component passes `MB[c] ≤ tol_mb` and `CNV[c] ≤ tol_cnv` (with the active
strict/relaxed tolerances), **and** the well report passes (`getWellConvergence(B_avg)`,
`tolerance-wells=1e-4`; combined at `getConvergence`, l.985), **and** `iteration ≥
newton-min-iterations (2)` (l.183-186: `report.converged = convrep.converged() && iteration >=
minIter`).

**Iteration structure that must be preserved** (l.179-290): convergence is evaluated from the
freshly assembled residual BEFORE any linear solve; a converged iteration costs one assembly and
zero linear solves. Loop condition (`NonlinearSolver.hpp`, l.167):
`while ((!converged && iteration <= newton_max_iter) || iteration <= newton_min_iter)`.
Exhaustion without convergence throws `TooManyIterations` → substep failure (§9.4).

### 9.2 Per-cell update chopping (`opm/models/blackoil/blackoilnewtonmethod.hpp`, l.201-390)

Per cell (no global coupling anywhere), with `update` = raw Newton update and the convention
`next = current - delta`:

```
dSw = update[waterIdx]        (when water primary variable means Sw)
dSg = update[gasIdx]          (when gas primary variable means Sg)
dSo = -(dSw + dSg)                       # implied oil delta COUNTS toward the max
maxSatDelta = max(|dSw|, |dSo|, |dSg|)
satAlpha    = maxSatDelta > dsMax ? dsMax / maxSatDelta : 1     # dsMax = 0.2

saturation deltas   *= satAlpha                                  # one scalar per CELL
pressure delta       = clamp(dp, ±dpMaxRel * p_current)          # dpMaxRel = 0.3, independent
next pressure        = clamp(next, pressMin, pressMax)
```

Notes: `satAlpha` preserves the update direction within the cell (both saturations scale
together); the pressure chop is per-variable and NOT scaled by `satAlpha`; when the gas/water
variable currently means `Rs`/`Rv`/`Rsw` the chop instead only prevents the factor from going
negative (`delta = min(delta, current)`) — relevant once variable substitution exists, harmless
to include now. The trailing "switch primary variable meaning" step (l.390+) is variable
substitution — out of Bundle N scope.

### 9.3 Timestep controller (`TimeStepControl.cpp`)

Default `pid+newtoniteration` = `PIDAndIterationCountTimeStepControl::computeTimeStepSize`
(l.274): **`dt_next = min(dt_PID, dt_iter)`** where, with `its` = Newton iterations of the
accepted substep, `target = time-step-control-target-newton-iterations (8)`,
`decayDamping = 1.0`, `growthDamping = 3.2`:

```
if its > target:  dt_iter = dt / (1 + (its-target)/target * decayDamping)
else:             dt_iter = dt * (1 + (target-its)/target * growthDamping)
```

(e.g. its=2 → dt x 3.4; its=8 → dt x 1.0; its=16 → dt x 0.5. NOT the 1.25/0.75 rates — those
belong to the non-default `iterationcount` controller.)

`dt_PID` (`PIDTimeStepControl::computeTimeStepSize`, l.188), on the error series
`e_0,e_1,e_2` (last three substeps) where `e = relativeChange()` and
`tol = time-step-control-tolerance (0.1)` (times `safety-factor 0.8` where applied by the
harness):

```
if e_2 > tol:        dt_PID = dt * tol / e_2
elif any e_k == 0:   dt_PID = +inf     (no PID constraint)
else:                dt_PID = dt * (e_1/e_2)^0.075 * (tol/e_2)^0.175 * (e_1²/(e_0·e_2))^0.01
```

`relativeChange()` (`BlackoilModel_impl.hpp`, l.371): between the accepted substep solution and
the previous one, `Σ_i [(Δp_i)² + Σ_phases (ΔS_i)²] / Σ_i [p_i² + Σ_phases S_i²]` — one global
scalar, saturations from primary variables with implied So. (OPM's own source carries an
"NB fix me!" about mixing pressure and saturation units; port it as-is per the 95%-track-OPM
policy — do not "improve" it.)

### 9.4 Substep failure & growth clamps (`AdaptiveTimeStepping_impl.hpp`)

- Any thrown failure (`TooManyIterations`, `NumericalProblem`, `LinearSolverProblem`, …) fails
  the substep (l.1165-1186): `dt_next = solver-restart-factor (0.33) * dt` (l.818); terminate
  after `solver-max-restarts (10)` consecutive restarts; abort below `solver-min-time-step
  (1e-12 days)`.
- Growth clamps on the controller's proposal (`maybeRestrictTimeStepGrowth_`, l.1067):
  `dt_next = min(dt_next, solver-max-growth (3.0) * dt)` always; additionally
  `min(solver-growth-factor (2.0) * dt, dt_next)` while `restarts > 0` in the current report
  step.
- There is no failure-family classification, no site memory, and no replay anywhere in this
  path.

### 9.5 Linear-solver failure handling (`AbstractISTLSolver.hpp::checkConvergence`, l.194)

```
if !result.converged:
    if result.reduction < relaxed-linear-solver-reduction (0.01):  # note: '<' on reduction
        → treat as CONVERGED, log a warning, keep the direction    #  achieved (smaller=better)
    elif !linear-solver-ignore-convergence-failure (default false):
        → throw NumericalProblem → substep failure path (§9.4)
```

i.e. a linear solve that hit its iteration cap but achieved the (relaxed) 100x reduction is
simply **used**. There is no direct-solver fallback anywhere in OPM's path — ResSim's
dead-state/zero-move/restart-stagnation direct-LU bypass ladder has no analog and is deleted in
the `OpmAligned` path (N4/N5).

### 9.6 CPR setup reuse — Bundle P target (`ISTLSolver.hpp`, l.515-552)

Recreate-solver decision per linear solve: never recreate if the preconditioner
`hasPerfectUpdate()` (values refreshed in-place); else by `cpr-reuse-setup`: `0` always, `1`
first Newton iteration of each timestep, `2` when the previous solve took >10 iterations,
`3` never, `4` (default) every `cpr-reuse-interval (30)` solve calls. ResSim mapping: the
quasi-IMPES weights + coarse-operator construction + dense coarse factorization are the
"setup" to reuse; block-ILU0 value refactorization each solve is the analog of the in-place
`update()`.

### 9.7 Explicitly excluded (verified default-off or out of scope)

- `convergence-monitoring` (penalty-card early cuts): **default false** — not part of the port.
- `min-time-step-based-on-newton-iterations (0)`, NLDD nonlinear domain decomposition, TUNING
  keyword overrides: out of scope.
- Newton relaxation type `dampen` (`stabilizeNonlinearUpdate`): already ported in Phase 7
  (`FIM-NEWTON-001`/`006`) — unchanged.
