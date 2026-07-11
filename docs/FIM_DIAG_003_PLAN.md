# FIM-DIAG-003: The Last Frozen Criterion (MB plateau under `OpmAligned`)

Status: PLANNED (2026-07-11). Registry: `FIM-DIAG-003` (DIAGNOSTIC, open).
Prerequisite evidence: `docs/FIM_CONVERGENCE_WORKLOG.md` "Week retrospective (2026-07-11)"
(commit `ec47e62`) and "Bundle W checkpoint W4/W5" (commit `c916c87`).

## 1. Context — what is established, what is not

The heavy case (`water-pressure 12x12x3 --dt 1` under `OpmAligned`, with or without
`nested_well_solve`) fails its `≤35`-substep gate at ~18k substeps. The week retrospective
established, from recorded traces plus fresh source checks:

- The failure was always a **conjunction** of independently-frozen criteria. Bundle W removed
  one term (the perforation standoff, now at machine epsilon); exactly one remains.
- The survivor: **MB alone binds** — `mb=[1.412e-7, 1.423e-7]` vs strict `1e-7` (1.41x over),
  while CNV passes by 160x (`6.1e-5` vs `1e-2`). Frozen at 4 significant figures across 18
  Newton iterations = an **invariant point of the modified iteration map** (Newton step +
  per-cell chop + nested well solve + bounds), not slow convergence. Accepted only by the
  final-iteration relaxed tier (`1e-6`), at which point N3 sees `iters=20` and collapses dt.
- Acceptance tiers are NOT the divergence from OPM: `ToleranceMb=1e-7`,
  `ToleranceMbRelaxed=1e-6` applied at the final iteration only (`MinStrictMbIter=-1`) —
  verified identical in the pinned OPM source and our port (`newton.rs:1794-1797`). Since OPM
  solves this case at ~2.5 iters/step with the same rules, **OPM's MB genuinely drops below
  `1e-7` where ours freezes at `1.41e-7`.** Why is the open question.

Three ranked hypotheses (retrospective §3):
- **H1 — displaced standoff.** Enforcing the perforation equation exactly (Bundle W) displaced
  the same well/reservoir inconsistency into the well-cell mass-balance rows. Signature already
  on record: the coupled linear solve proposes `dq≈+0.58` every iteration (its way of zeroing
  those rows via the source term); the nested solve vetoes it; nobody can fix the rows.
- **H2 — linear-precision floor.** The loose `5e-3` outer linear tolerance (`FIM-LINEAR-008`)
  caps the achievable MB reduction near steady state.
- **H3 — MB formula fidelity.** Our `cnv_mb_diagnostics` runs ~1.4x hot vs OPM's
  `getReservoirConvergence` at the same state (a pore-volume/B_avg/dt-factor discrepancy).
  Would ALSO explain the never-explained 3x bounded-case gap (`OpmAligned` 12/1 vs Legacy 4/2,
  Bundle N §10 obs. 6): if MB reads hot, every `OpmAligned` case pays extra iterations.

These are not exclusive — H1 and H3 could both contribute to the same 1.41x. The point of this
plan is to *discriminate with evidence before any fix* (the `FIM-DIAG-002` discipline; zero
guessing budget has been spent on this mechanism).

**Explicitly out of scope until H1-H3 are discriminated**: OPM's `min_strict_mb_iter` knob
(relaxed MB after N iterations instead of final-only). It exists and is OPM-shipped, but OPM's
*defaults* solve this case without it — reaching for it first would be acceptance-widening in
OPM clothing (`FIM-NEWTON-005` lesson).

## 2. Bookkeeping frame (from the retrospective)

- The **candidate stack** is `OpmAligned` + `nested_well_solve`. Its baseline:
  `accepted_substeps=18015`, wall `1235.5s`, commit `c916c87`, replay command in
  `docs/FIM_BUNDLE_W_PLAN.md` §6 W4.
- Progress metric between fixes: **binding-constraint margin** (currently MB `1.41e-7` vs
  `1e-7`) and substeps-to-`t=0.9` on capped runs (`FIM_MAX_SUBSTEPS=1000`, ~70s each) — NOT the
  final substep count, which is a step function over the conjunction.
- Promotion decision happens once, at stack level, when the chain is exhausted; the gate then
  is the original Bundle N §5 gate (heavy `≤35`-substep class + fine-dt FOPT + control matrix +
  bounded cases not worse than Legacy).

## 3. Checkpoints

### D0 — instrumentation (no-op gated, same discipline as `FIM-DIAG-002`'s sink)

1. **Binding-criterion trace**: extend the per-iteration `CNV-MB` trace line (or add a sibling
   line, window-gated like `WELLTRACE`) to name *which* criterion blocks acceptance and *which
   cell* owns the peak MB value (`fim/newton.rs` — the `ResidualFamilyPeak` / FAIL-SITE-DETAIL
   machinery already carries row/cell indices; MB is computed per family in
   `cnv_mb_diagnostics`, so the peak-contributing cell needs exposing there).
2. **Forced-direct-linear switch**: `FIM_FORCE_DIRECT_LINEAR` env var, read in the native repro
   driver (same pattern as `FIM_NESTED_WELL_SOLVE`) via a dev setter that sets
   `newton_options.linear.kind = FimLinearSolverKind::SparseLuDebug` — every Newton iteration
   solves exactly, no CPR/GMRES. Native/diagnostic only; no wasm surface needed.

Gates: flag/env off ⇒ control matrix bit-identical + locked smoke 3/3 + wasm build green.

### D1 — discriminate H1 vs H2 (capped ~70s runs, native `--release`)

1. Windowed capped run (`FIM_NESTED_WELL_SOLVE=1`, `FIM_TRACE_SUBSTEP_START≈980`,
   `FIM_MAX_SUBSTEPS=1000`) with the D0 binding-cell trace: **where does the frozen `1.41e-7`
   live?** Producer perforation cell (143) or its column ⇒ H1 confirmed. Distributed across
   non-well cells ⇒ H1 weakened.
2. Same capped run with `FIM_FORCE_DIRECT_LINEAR=1`: **does the freeze break with exact linear
   solves?** Yes (MB drops below `1e-7`, `iters=20` alternation disappears, substeps-to-`t=0.9`
   improves materially) ⇒ H2 confirmed — the fix direction becomes linear-tolerance policy near
   steady state (adaptive tightening, an OPM-comparison question of its own), NOT acceptance
   changes. No ⇒ H2 eliminated cheaply.
3. Cross: forced-direct + binding-cell trace together disambiguates the H1∧H2 case (exact
   solves with the residual still parked at the well cells ⇒ pure H1).

### D2 — H3: MB formula audit (static, W0-style)

Line-by-line comparison of `cnv_mb_diagnostics` (`fim/newton.rs`) against
`NonlinearSystemBlackOilReservoir_impl.hpp::getReservoirConvergence` (pinned checkout
`062cb1998`): B_avg construction, pore-volume summation (total vs eligible, aquifer exclusion),
the dt factor, per-phase vs per-component grouping, and the exact norm (sum vs volume-weighted
sum). Record findings as an appendix here with file:line citations. A ~1.4x-shaped discrepancy
⇒ fix it (that's a fidelity bug, not tuning), then re-run the capped heavy + both bounded cases
— expect the bounded 3x gap to move if H3 is real. Also re-verify the `NewtonMinIterations=2`
vs `OPM_NEWTON_MIN_ITERATION_INDEX=1` off-by-one flagged in the retrospective while in there.

### D3 — OPM Flow differential trajectory (the oracle, ~a day)

`/usr/bin/flow` is installed; the deck harness exists on `origin/fim-opm-continuation-plan`
(`opm/reference-decks/`, `scripts/opm-ressim-compare.sh`; `water-medium-step1` = the 20x20x3
preset and is the template). A 12x12x3 deck existed in April (it produced the `3826.12`
converged FOPT reference, `docs/FIM_CHOP_WIDEN_EXPERIMENT.md`) but is not among the tracked
decks — recreate it by adapting the template (DIMENS `12 12 3`, perms 2000/2000/200 mD, corner
wells BHP 500/100, `TSTEP 1.0` for the step-1 variant) and track it as
`opm/reference-decks/water-heavy-step1/`.

Run with `--output-extra-convergence-info=steps,iterations` (verified present in the pinned
source, `FlowMain.hpp:427`; produces `INFOSTEP`/`INFOITER`) and diff against our `LEDGER`:
- OPM's dt sequence and iterations-per-step through the steady tail (`t≈0.83-1.0`) — does OPM
  hold `dt=0.185`-class steps at 2-3 iterations where we collapse?
- OPM's per-iteration MB values at comparable states — is its MB at these states `~1e-8`
  (⇒ ours reads hot, H3) or `~1.4e-7`-but-still-converging (⇒ its Newton/linear stack genuinely
  reduces it, H2) or does it also touch its relaxed tier (⇒ our whole framing changes)?

This is also the pilot for adopting **trajectory-level differential comparison as a standing
method** (retrospective §4) — worth doing even if D1/D2 already discriminate, as the
verification instrument for whatever fix follows.

### D4 — combination coverage (cheap, parallel to the above)

1. **Legacy + `nested_well_solve` on the heavy case** — never run. Legacy heavy currently `52`
   substeps with `mixed:perf@1299`-class retries that are well-adjacent. If Legacy+W < 52 with
   physics intact, that's an independently promotable Legacy-side win (own full gate per
   `docs/FIM_BUNDLE_W_PLAN.md` §7 — control matrix will move by design, so fine-dt FOPT + BL
   benchmarks + locked smoke required).
2. The `22x22x1` OpmAligned+W `12→24` regression: one windowed trace to classify it (same
   MB-plateau story or something else). The `23x23x1` first-substep `linear-bad:oil@1585` ride
   along if cheap.

### D5 — decision point

With H1/H2/H3 discriminated and the OPM trajectory in hand: either (a) a scoped fix bundle for
the confirmed mechanism (new registry row, same checkpoint discipline), then re-run the stack
gate; or (b) if the evidence says the plateau is genuinely benign-and-OPM-accepts-it-differently,
a criteria-fidelity fix; or (c) if something structural emerges, re-plan honestly. Update
`FIM-DIAG-003` with the verdict either way; the stack promotion question stays open until the
chain is exhausted.

## 4. Cost estimate

D0 ~2-3h (instrumentation + gates); D1 ~1h (three ~70s runs + analysis); D2 ~2-4h (audit +
re-runs if it hits); D3 ~a day (deck + run + parse + diff); D4 ~1h capped / +25 min if the full
Legacy+W run is warranted. Everything except D3's full runs fits in capped-run economics
(`FIM-LINEAR-011` made this program feasible — a week ago D1 alone would have been ~15 hours).

## 5. Documentation consequences

- Worklog entry per checkpoint, numbers verbatim; registry `FIM-DIAG-003` updated with the
  verdict; `docs/FIM_STATUS.md` gap #4 updated on resolution.
- If D2 or D3 finds an MB formula fidelity bug: that fix gets its own registry row (it changes
  acceptance behavior everywhere under `OpmAligned`) with the full bounded-case + fine-dt gates.
- If D4's Legacy+W wins: separate registry row + full Legacy promotion gate, independent of the
  stack question.
