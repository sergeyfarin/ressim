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
- Shared outer-step carryover prototype result:
  - kept the headline shelf counts at `substeps=129`, `retries=0/35/0`, `hotspot_newton_caps=12`
  - but runtime moved the wrong way: `outer_ms=120294.2`, `retry_ms=22175.0`
- Narrowed one-substep seed-cap refinement result:
  - preserved the same headline shelf counts at `substeps=129`, `retries=0/35/0`, `hotspot_newton_caps=12`
  - but water runtime regressed further to `outer_ms=137259.1`, `retry_ms=26023.0`
- Reverted current-head state:
  - the cross-outer-step carryover experiment was removed after the refinement still failed the water guard
  - post-revert canonical replay returned to the prior water shelf class: `substeps=129`, `retries=0/35/0`, `hotspot_newton_caps=12`, `outer_ms=119991.9`, `retry_ms=22092.0`, `retry_dom=nonlinear-bad:oil@430`
- Current interpretation:
  - this controller slice is a real water-side improvement
  - the dominant hotspot did not move, so the remaining issue is still the same reservoir-row shelf rather than a new failure family
  - carrying a hard dt cap across outer-step boundaries is not a clean shared-policy lever here; both carryover variants held the water shelf too conservatively and were reverted

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
- Shared outer-step carryover prototype result:
  - step 4 improved from `retries=0/2/0` to `retries=0/0/0`
  - steps 2, 3, 5, and 6 stayed at `retries=0/2/0`
  - `hotspot_newton_caps` stayed active (`4` on the retrying gas steps, `3` on the clean step-4 replay)
- Reverted current-head state:
  - after removing cross-outer-step carryover, the shipped replay is back to the known shared-policy baseline: `0/3/0`, `0/2/0`, `0/2/0`, `0/0/0`, `0/2/0`, `0/2/0`
  - `hotspot_newton_caps` remains active on the retrying gas steps without the reverted carryover path
- Current interpretation:
  - the controller is now engaging on the intended gas-side path
  - outer-step carryover was directionally correct for gas because it could suppress one repeated retry shelf without losing the hotspot-repeat signal
  - it is still not the right shared implementation because the rest of the gas replay remained in the same `0/2/0` regime while water regressed

### Coarse pressure solver
- The old coarse-solver note was too broad.
- Representative hard 3D shelf (`12x12x3`) still uses the exact-dense coarse path and is not limited by coarse-stage reduction quality:
  - `cpr=[rows=434 solver=dense apps=12 avg_rr=1.148e-13 last_rr=3.754e-14]`
- Over-threshold CPR cases are still open on current head:
  - bounded `23x23x1` probe shows `cpr=[rows=531 solver=ilu apps=11 avg_rr=7.557e-2 last_rr=7.896e-2]`
  - the same probe later hits repeated linear failure and dense-LU fallback on the `1591`-row system
- Isolated threshold comparison:
  - exact-dense control `22x22x1` stays below the coarse threshold (`rows=486 solver=dense`) and still lands on the same bounded shelf shape: `substeps=8`, `retries=0/3/0`, `retry_dom=nonlinear-bad:oil@1450`, `outer_ms=18580.2`, `retry_ms=1229.0`
  - over-threshold `23x23x1` crosses the coarse threshold (`rows=531 solver=ilu`) but keeps the same bounded shelf counts: `substeps=8`, `retries=0/3/0`, `retry_dom=nonlinear-bad:oil@1585`, `outer_ms=27243.3`, `retry_ms=8656.0`
  - current code-backed threshold is explicit: coarse rows `<= 512` use exact-dense inversion and `> 512` switches to ILU defect correction
- Current linear-track interpretation:
  - this bounded pair isolates a real backend penalty: the over-threshold case does not add more retries or accepted substeps, but it spends much more time in linear work before converging the same shelf
  - on `23x23x1`, the coarse stage starts on the ILU path but Newton falls back to `dense-lu` on the full `1591`-row system almost immediately after the first coarse-backed iteration on each retry ladder
  - that means the next over-threshold slice should target fallback frequency and iterative-backend persistence first, before mixing the result back into the shared nonlinear-controller track
- Current interpretation:
  - coarse-pressure-solver quality is not the active blocker on the representative exact-dense shelf
  - it remains an open issue for larger over-threshold CPR cases

## Active Conclusions
- Current-head hard shelves are best described as reservoir-row nonlinear problems with partial controller improvements already landed.
- The water shelf has a measurable win from hotspot-aware cooldown and broader growth suppression.
- The shipped gas shelf has better identity, damping activation, and classification, but still needs a stronger policy lever to reduce retries.
- Cross-outer-step dt-cap carryover was tested twice, regressed the water guard both times, and has been reverted; keep the gas observation as evidence that stronger shared memory may help, but not in that form.
- Coarse-pressure-solver quality should remain open only for over-threshold CPR cases; the bounded `22x22x1` vs `23x23x1` pair now isolates that penalty cleanly enough to treat it as the active linear-backend track.

## Recommended Next Tests - 2026-04-08
- Re-read against `docs/OPM_FLOW_MINIMAL_MAPPING.md` keeps the same two OPM lessons active on current head:
  - broader nonlinear stabilization is the right follow-up for the representative shipped shelves
  - explicit CPRW-style well-aware coarse pressure is the right follow-up only for over-threshold coarse systems
- Best next test order:
  1. Run the over-threshold CPR probe as the active linear-backend track first, anchored on the bounded `22x22x1` vs `23x23x1` comparison so coarse-stage quality can be measured separately from the shared nonlinear shelf.
  2. Keep the shared reservoir-row controller refinement second: the next shared-policy slice should still avoid hard dt-cap carryover across outer-step boundaries.
- Why this is the right split now:
  - both water and gas are still materially behind mature FIM behavior, so a worthwhile next controller slice should be allowed to help both if the mechanism is genuinely shared
  - the recent wins already came from shared hotspot/cooldown policy, not from fluid-specific physics rewrites
  - current diagnostics still separate the two main open mechanisms cleanly: representative water/gas shelves are dominated by reservoir-row nonlinear behavior, while only the `>512` coarse-row path points back to OPM-style CPRW work
- Concrete acceptance criteria for the shared controller test:
  - shipped `gas-rate --grid 10x10x3 --steps 6 --dt 0.25` should improve beyond the current reverted baseline (`0/3/0`, then `0/2/0`, `0/2/0`, `0/0/0`, `0/2/0`, `0/2/0`), not just wall clock
  - `hotspot_newton_caps` should stay active on steps `2-6`; losing that signal would mean the new policy is bypassing the intended controller path
  - `water-pressure --grid 12x12x3 --steps 1 --dt 1` should stay at or better than `129` accepted substeps with no retry-class regression and should remain in the reverted runtime class rather than reintroducing the carryover-regression regimes (`120294.2` / `22175.0` or worse, and especially not the failed refinement `137259.1` / `26023.0`)
- Concrete acceptance criteria for the over-threshold CPR track:
  - use the bounded pair together: `water-pressure --grid 22x22x1 --steps 1 --dt 0.25` as the exact-dense control and `water-pressure --grid 23x23x1 --steps 1 --dt 0.25` as the over-threshold probe
  - measure coarse-stage quality separately from nonlinear shelf behavior: coarse rows, solver kind, average and last reduction ratio, fallback frequency, and whether the step stays on the iterative backend long enough to matter
  - require any linear-backend slice to improve the over-threshold runtime class or fallback burden without worsening the bounded shelf counts (`substeps=8`, `retries=0/3/0`) before promoting it into a broader CPRW implementation slice
- Tests that should not be the next slice from this review:
  - do not reopen generic line-search or Appleyard work; the old OPM-gap note there is stale on current head
  - do not reopen broad well-Schur experiments for the representative shelves; current diagnostics no longer show a well/perforation-dominated blocker on those cases
  - do not generalize exact-dense water-shelf results into a closure for large-case CPR quality

## Next Questions
1. Gas shelf: can repeated gas-region failures retain stronger memory without carrying a hard dt cap across outer steps?
2. Gas shelf: is the next lever a softer first-regrowth throttle after a repeated hotspot, or is a broader memory key still needed beyond the current injector-region grouping?
3. Over-threshold CPR: what is the narrowest change that keeps the `23x23x1` iterative path alive longer or reduces fallback burden, while preserving the same bounded shelf shape as the `22x22x1` control?

## Validation Shortlist
- Water shelf summary:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json`
- Gas shelf outer replay:
  - `node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic outer --no-json`
- Over-threshold coarse probe:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 --diagnostic step --no-json | rg -m 8 "cpr=\[|FIM retry summary|FIM step done|Newton: dt="`
- Exact-dense threshold control:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 22x22x1 --steps 1 --dt 0.25 --diagnostic step --no-json | rg -m 8 "cpr=\[|FIM retry summary|FIM step done|Newton: dt="`
