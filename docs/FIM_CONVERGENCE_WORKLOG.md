# FIM Convergence Worklog

This file is the active investigation log for live FIM convergence work.
Use `docs/FIM_STATUS.md` for the current consolidated solver status.
Use this worklog only for active observations, reproductions, traces, and next hypotheses while an issue is still live.

Historical narrative was trimmed out of this file on 2026-04-08.
- March 2026 tracker history from `TODO.md`: `docs/FIM_HISTORY_2026-03.md`
- Former full live worklog snapshot through 2026-04-06: `docs/FIM_CONVERGENCE_ARCHIVE_2026-03_to_2026-04-06.md`

## Active Scope
- Keep this file limited to current-head repros, latest measurements, and next solver questions.
- Treat resolved correctness hardening and old exploratory branches as archival unless they reopen on current head.
- Current active repro set:
  - hard water shelf: `water-pressure --grid 12x12x3 --steps 1 --dt 1`
  - shipped gas shelf: `gas-rate --grid 10x10x3 --steps 6 --dt 0.25`
  - over-threshold CPR probe: `water-pressure --grid 23x23x1 --steps 1 --dt 0.25`

## Current Findings - 2026-04-08

### Water shelf
- The active hard water shelf is reservoir-row dominated, not well/perforation dominated.
- Repeated-hotspot cooldown now caps any accepted-step regrowth above `1.0`, not just `max-growth`.
- Latest canonical result:
  - before: `substeps=134`, `retries=0/44/0`, `outer_ms=121254.6`, `retry_ms=32917.0`
  - current: `substeps=129`, `retries=0/35/0`, `outer_ms=115710.8`, `retry_ms=19333.0`, `retry_dom=nonlinear-bad:oil@430`
  - current metric surface also reports `hotspot_newton_caps=12`
- Current interpretation:
  - this controller slice is a real water-side improvement
  - the dominant hotspot did not move, so the remaining issue is still the same reservoir-row shelf rather than a new failure family

### Gas shelf
- The shipped gas shelf is still a nonlinear hotspot problem, but its active identity and damping path are now visible and stable.
- Landed current-head changes already active on this shelf:
  - fallback-assisted Newton iterations now participate in nonlinear-history damping
  - alternating symmetric injector-side gas cells share one hotspot site key
  - cooldown memory is broader than exact-cell identity and now holds on `hotspot-repeat` consistently after step 1
  - converged fallback shelves are classified as `nonlinear-bad`, not falsely as `linear-bad`
- Latest shipped replay:
  - step 1: `retries=0/3/0`, `growth=max-growth`, `hotspot_newton_caps=0`
  - steps 2-6: `retries=0/2/0`, `growth=hotspot-repeat`, `hotspot_newton_caps=4`
  - wall clock improved from the old `1:14.59` class to about `1:11.18`
- Current interpretation:
  - the controller is now engaging on the intended gas-side path
  - retry counts are still flat, so this remains a partial fix rather than a convergence win

### Coarse pressure solver
- The old coarse-solver note was too broad.
- Representative hard 3D shelf (`12x12x3`) still uses the exact-dense coarse path and is not limited by coarse-stage reduction quality:
  - `cpr=[rows=434 solver=dense apps=12 avg_rr=1.148e-13 last_rr=3.754e-14]`
- Over-threshold CPR cases are still open on current head:
  - bounded `23x23x1` probe shows `cpr=[rows=531 solver=ilu apps=11 avg_rr=7.557e-2 last_rr=7.896e-2]`
  - the same probe later hits repeated linear failure and dense-LU fallback on the `1591`-row system
- Current interpretation:
  - coarse-pressure-solver quality is not the active blocker on the representative exact-dense shelf
  - it remains an open issue for larger over-threshold CPR cases

## Active Conclusions
- Current-head hard shelves are best described as reservoir-row nonlinear problems with partial controller improvements already landed.
- The water shelf has a measurable win from hotspot-aware cooldown and broader growth suppression.
- The shipped gas shelf has better identity, damping activation, and classification, but still needs a stronger policy lever to reduce retries.
- Coarse-pressure-solver quality should remain open only for over-threshold CPR cases; do not generalize the exact-dense representative-case result beyond that scope.

## Next Questions
1. Gas shelf: should repeated gas-region failures retain stronger cutback memory across adjacent outer steps instead of resetting after one clean accept?
2. Gas shelf: is a broader memory key still needed beyond the current injector-region grouping, or is the next lever purely timestep-policy strength?
3. Over-threshold CPR: should the next linear-backend slice target the `>512` coarse path directly with a stable bounded probe and explicit budget?

## Validation Shortlist
- Water shelf summary:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json`
- Gas shelf outer replay:
  - `node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic outer --no-json`
- Over-threshold coarse probe:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 --diagnostic step --no-json | rg -m 8 "cpr=\[|FIM retry summary|FIM step done|Newton: dt="`
