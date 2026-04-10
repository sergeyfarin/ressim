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
  - the parked Phase 1 CPR fine-smoother change is not the cause of the current gas replay shape: rebuilding wasm and replaying the same shipped case with the default CPR smoother temporarily forced back to block-Jacobi produced the same `0/3/0`, then repeated `0/2/0` shelf, including step 4 at `0/2/0`

### Coarse pressure solver
- The old coarse-solver note was too broad.
- Representative hard 3D shelf (`12x12x3`) still uses the exact-dense coarse path and is not limited by coarse-stage reduction quality:
  - `cpr=[rows=434 solver=dense apps=12 avg_rr=1.148e-13 last_rr=3.754e-14]`
- Over-threshold CPR cases are still open on current head:
  - bounded `23x23x1` probe now shows the new coarse backend in live use: `cpr=[rows=531 solver=bicgstab smoother=ilu0 ...]`
  - current coarse-stage reduction quality is materially better than the old ILU defect-correction baseline: first retry rung reached `avg_rr=2.551e-6`, `last_rr=4.597e-5`; later retry rungs stayed in the `1e-5` to `1e-3` band
  - despite that improvement, the same probe still hits repeated linear failure and dense-LU fallback on the full `1591`-row system after the first CPR-backed iteration on each retry ladder
  - bounded follow-up experiment now isolates the post-coarse smoother specifically on the same over-threshold path: `cpr=[rows=531 solver=bicgstab smoother=ilu0/post-bj ...]`
  - this does not remove fallback yet, but it changes the failure shape materially: the rejected CPR solves remain `reason=max-iters`, while `prec_res` drops from the earlier `1e8-1e9` class into `1e0-1e3` and `cand_res/final_res` drops from `1e3-1e4` into `1e-1-1e-3`
  - at `dt=0.03125`, some Newton iterations now converge on the CPR path directly before later fallback resumes (`linear_iters=4`, `26`, `45`, `50` on successive accepted Newton iterations), which is the first clear sign that the instability is in the post-coarse global smoother/Krylov interaction rather than in coarse pressure reduction itself
  - bounded Krylov-plateau follow-up is now tighter: one real GMRES issue was that full restart cycles could discard candidate progress unless convergence or breakdown happened inside the restart. After fixing that and replaying `23x23x1`, fallback burden dropped from `46` rejected CPR solves / `56` dense-LU uses to `28` rejected CPR solves / `36` dense-LU uses without changing the bounded shelf shape (`substeps=8`, `retries=0/3/0`)
  - second bounded Krylov-tail follow-up is now also in: when the Givens residual estimate drops below tolerance but the true candidate residual still disagrees and Krylov space can still grow, the solver now keeps the current restart alive instead of restarting immediately. Replaying `23x23x1` with that fix cut fallback burden again from `28` rejected CPR solves / `36` dense-LU uses to `15` rejected CPR solves / `21` dense-LU uses, still with the same bounded shelf shape (`substeps=8`, `retries=0/3/0`)
  - third bounded Krylov-tail follow-up is now in too: tiny-residual tails that have already reached the post-improvement asymptote now stop early as `restart-stagnation` instead of spending all five restart windows proving that later candidates are only worse. Replaying `23x23x1` with that bounded termination tweak cut fallback burden again from `15` rejected CPR solves / `21` dense-LU uses to `14` rejected CPR solves / `20` dense-LU uses, still with the same bounded shelf shape (`substeps=8`, `retries=0/3/0`)
  - new per-restart failure traces show the remaining pattern more narrowly now: many former failures no longer die at the old false-estimate boundary. The remaining set splits into two smaller families: a tiny-residual asymptotic tail where the first 2–3 restarts improve by orders of magnitude before later restart candidates stall or worsen, and a smaller hard-state family where restart 1 never helps (`upd=n` from the first cycle) and all five restart windows are effectively spent proving that
  - the new termination traces sharpen that split further: several former tiny-tail `max-iters` failures now stop at restart 3 or 4 with `reason=restart-stagnation` after `68-103` total iterations, while the hard-state family is still unchanged and remains full-budget `max-iters` with `upd=n` from restart 1
  - bounded dead-state detection is now in for that remaining hard-state family: when restart 1 consumes the full window, never improves the iterate (`upd=n`), and is still clearly far from the tiny-tail regime, the solver now exits immediately as `reason=dead-state` instead of spending all five restart windows. On the same `23x23x1` replay this did not change the headline fallback count (`14` rejected CPR solves / `20` dense-LU uses), but it did collapse the hard-state failures from `150` iterations / `155` CPR applications down to `30` iterations / `31` CPR applications each
  - Newton-side dead-state bypass is now in too: after one explicit `reason=dead-state` failure, later Newton iterations in the same substep stop retrying CPR and go straight to the direct backend. On the same `23x23x1` replay this cut rejected CPR solves again from `14` to `9`, while dense-LU uses stayed flat at `20`. In other words, the repeated rediscovery cost is now gone; the remaining cost is the direct fallback itself
  - first direct-fallback cleanup is now in on wasm: the dense fallback no longer clones the dense matrix before LU and no longer uses a dense residual multiply just to validate the solve. Replaying `23x23x1` after that cleanup kept the same bounded behavior (`9` rejected CPR solves / `20` dense-LU uses, `substeps=8`, `retries=0/3/0`), which is the expected outcome for a semantics-preserving cost cut rather than an algorithmic fallback reduction
  - bounded large-row direct-fallback A/B is now also in on wasm. Temporarily forcing the same `23x23x1` bounded replay back to dense LU preserved the earlier behavior class (`9` rejected CPR solves / `20` dense-LU uses, same `substeps=8`, `retries=0/3/0`) but took about `10866.6 ms` outer / `10764.0 ms` linear. Restoring sparse LU for large-row direct fallback changed the same replay only slightly on the control counts (`11` rejected CPR solves / `22` sparse-LU uses, still `substeps=8`, `retries=0/3/0`) while cutting runtime to about `1326.5 ms` outer / `1219.0 ms` linear. Current interpretation: sparse LU should stay as the large-row wasm direct fallback even though the iterative path still needs one more fallback-avoidance slice, because dense LU is an order-of-magnitude slower on the same remaining hard states
  - bounded sparse-LU refinement follow-up is now also in. Adding a short iterative-refinement loop to the large-row sparse direct backend did not change the bounded control counts on `23x23x1` (`11` rejected CPR solves / `22` sparse-LU uses / `5` dead-state bypasses, same `substeps=8`, `retries=0/3/0`), but it did improve runtime again to about `1289.6 ms` outer / `1182.0 ms` linear. Current interpretation: this is worth keeping as a direct-path efficiency improvement, but it does not move the active diagnosis. The remaining leverage is still fallback incidence, not another direct-backend micro-optimization by itself
  - bounded repeated-`restart-stagnation` bypass is now also in at the Newton layer. When the same substep hits two consecutive iterative failures with `reason=restart-stagnation`, later Newton iterations now bypass CPR and go straight to the row-selected direct backend for the remainder of that substep. On the same `23x23x1` bounded replay this new bypass fired twice, cut rejected CPR solves from `11` down to `9`, kept sparse direct fallback uses flat at `22`, preserved the same bounded shelf (`substeps=8`, `retries=0/3/0`), and improved runtime again to about `1174.6 ms` outer / `1074.0 ms` linear. Current interpretation: repeated restart-stagnation rediscovery is now as bounded as the earlier dead-state family; the remaining active cost is the `22` direct solves themselves
  - bounded zero-move fallback bypass is now also in and is the new current head on this replay. If a fallback-backed Newton iteration produces only an effectively zero state move, using the existing effective-move floor (`<5e-3 bar`, `<5e-5` saturation), the next Newton iteration now bypasses CPR and goes straight to the row-selected direct backend instead of rerunning the same iterative solve on the same unchanged state. Two confirming `23x23x1` replays land on the same control counts: `6` rejected CPR solves, `19` sparse direct fallbacks, `5` dead-state bypasses, and `2` zero-move bypasses, with the same bounded shelf (`substeps=8`, `retries=0/3/0`). The runtime class remains in the same improved band (`outer_ms≈1188-1257`, `lin_ms≈1091-1152`). Current interpretation: same-substep CPR rediscovery is now bounded one step further; the remaining active cost is the remaining `19` direct solves themselves
  - bounded near-converged iterative accept is now also in and is the new current head. If the CPR solve lands in a small-residual `restart-stagnation` or `max-iters` tail but is still close enough to tolerance (`outer_res <= 16x tol`, candidate residual no worse than `8x` the current iterate, and at least one restart improved the iterate), Newton now accepts that iterative step instead of paying for a direct fallback. Two confirming `23x23x1` replays agree on the new control counts: `6` rejected CPR solves, `18` sparse direct fallbacks, `5` dead-state bypasses, `2` zero-move bypasses, and `1` near-converged iterative accept, with the same bounded shelf (`substeps=8`, `retries=0/3/0`). Runtime remains in the same improved band (`outer_ms≈1209-1231`, `lin_ms≈1106-1121`). Current interpretation: the remaining active cost is now the remaining `18` direct solves themselves, not small-residual linear tails that were already good enough for Newton
- Isolated threshold comparison:
  - exact-dense control `22x22x1` stays below the coarse threshold (`rows=486 solver=dense`) and still lands on the same bounded shelf shape: `substeps=8`, `retries=0/3/0`, `retry_dom=nonlinear-bad:oil@1450`, `outer_ms=18580.2`, `retry_ms=1229.0`
  - over-threshold `23x23x1` crosses the coarse threshold (`rows=531 solver=bicgstab`) but keeps the same bounded shelf counts: `substeps=8`, `retries=0/3/0`, `retry_dom=nonlinear-bad:oil@1585`
  - the current code-backed threshold is explicit: coarse rows `<= 512` use exact-dense inversion and `> 512` switches to BiCGSTAB on the coarse system
- Current linear-track interpretation:
  - this bounded pair still isolates a real backend penalty: the over-threshold case does not add more retries or accepted substeps, but it still spends materially more time in linear work before converging the same shelf
  - Phase 2 improved coarse-stage quality itself, but that alone was not enough to change bounded step behavior because Newton still falls back to `dense-lu` on the full `1591`-row system almost immediately after the first CPR-backed iteration on each retry ladder
  - the bounded post-coarse smoother experiment adds a second, narrower result: once the post-coarse pass is switched to block-Jacobi, the iterative solve gets much closer to usable before fallback and occasionally survives whole Newton iterations on the CPR path
  - the restart-boundary fix adds a third result: a real portion of the old plateau was bookkeeping loss, not just bad preconditioning
  - the false-estimate continuation fix adds a fourth result: another real portion of the old plateau was premature restart, not just weak preconditioning or bad bookkeeping
  - the tiny-tail termination tweak adds a fifth result: another real portion of the old plateau was just wasted proof work after the asymptote had already been reached
  - that means the next over-threshold slice should stay inside Phase 2 but narrow further again: the wasted-iteration part, repeated-rediscovery part, and the first direct-path cleanup are now all in, so the remaining question is whether to make the direct fallback itself materially cheaper again via reusable workspace/factorization plumbing, or to reduce the number of direct fallbacks algorithmically
  - the dense-vs-sparse direct-fallback A/B plus the sparse-refinement, repeated-restart-stagnation, zero-move bypass, and near-converged iterative-accept follow-ups narrow that again on wasm: the direct backend itself is now cheap enough to keep sparse LU on the hot path for large systems, same-substep CPR rediscovery is bounded more tightly again, and one class of small-residual linear tails no longer pays for direct fallback. The higher-value next slice is now to reduce how often the remaining hard states need any direct fallback at all
- Current interpretation:
  - coarse-pressure-solver quality is not the active blocker on the representative exact-dense shelf
  - it remains an open issue for larger over-threshold CPR cases, but the problem is now narrower: coarse reduction quality improved, while full-system fallback burden did not
  - Phase 1 should therefore be treated as implemented but parked pending later promotion; Phase 2 is implemented and test-green, but still not promoted as a runtime/convergence win until the over-threshold iterative path survives longer

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
  - require any next linear-backend slice to improve the over-threshold fallback burden or runtime class without worsening the bounded shelf counts (`substeps=8`, `retries=0/3/0`) before promoting it into a broader CPRW implementation slice
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
