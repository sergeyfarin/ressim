# FIM Convergence Worklog

This file is the active investigation log for live FIM convergence work.

Use `docs/FIM_STATUS.md` for the current consolidated solver status.
Use this worklog only for active observations, reproductions, traces, and next hypotheses while an issue is still live.

This file is the working document for the March 2026 FIM convergence investigation.
Keep active observations, reproductions, diagnostics, and next hypotheses here until the issue is resolved.

## Validation Update - 2026-04-05 resumed after cleanup, material-change fix for well/perf-only Newton updates

- Cleanup and baseline status before resuming convergence work:
  - removed the FIM blanket dead-code suppression and cleaned the newly exposed FIM-only dead helpers by either deleting them or narrowing them to `#[cfg(test)]`
  - locked short baseline remained green after cleanup (`drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`, `spe1_fim_first_steps_converge_without_stall`, `spe1_fim_gas_injection_creates_free_gas`)
- First resumed wasm diagnostic on current head exposed a much worse hard-case day-1 shelf than the March notes:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json`
  - observed outcome before the fix: `outer_ms ≈ 381094.5`, `history += 50643`, `warning=none`
  - step trace showed repeated accepted microsteps with effectively zero cell-state change and a dominant `water` hotspot at `row=0`, `cell0=(0,0,0)` rather than the previously documented boundary cell `143`
  - representative failing retries reported `invalid bounded Appleyard candidate` even when the linear solve was healthy and the update norm was small
- Root cause found in `src/lib/ressim/src/fim/newton.rs`:
  - `iterate_has_material_change(...)` only compared cell unknowns and ignored `well_bhp` plus `perforation_rates_m3_day`
  - consequence: Newton candidates that changed only well/perforation unknowns were treated as unchanged
  - that polluted three acceptance/control paths:
    - candidate validity during damping
    - update-based convergence gating
    - the iteration-0 residual-entry guard exactness check
  - on the hard water shelf this let the solver fall into a pathological loop of tiny accepted outer substeps with no meaningful recognized state progress
- Change made:
  - extended `iterate_has_material_change(...)` to include well-BHP and perforation-rate deltas
  - added focused regression coverage via `iterate_has_material_change_detects_well_and_perforation_updates`
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml entry_guard_does_not_accept_unchanged_previous_state -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml iterate_has_material_change_detects_well_and_perforation_updates -- --nocapture` passed
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - healthy reference rerun:
    - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 24x1x1 --steps 1 --dt 1 --diagnostic summary --no-json`
    - outcome: `outer_ms ≈ 23.6`, `history += 5`, `warning=none`
  - canonical hard-case rerun after the fix:
    - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json`
    - outcome: `outer_ms ≈ 97585.1`, `history += 139`, `warning=none`

Interpretation:

- The catastrophic `50643`-history-point day-1 run was not just stale worklog drift; it exposed a real acceptance/accounting bug in the Newton material-change detector.
- Fixing well/perforation-only material-change recognition restored the hard day-1 waterflood shelf to essentially the earlier March regime (`~137-140` history points) without regressing the healthy 1D reference.
- The previously documented outer-step oscillation shelf is therefore still the active convergence target after cleanup. The `50643`-substep collapse was a separate bug layered on top of that shelf, not a replacement diagnosis.

## Validation Update - 2026-04-05 residual-aware growth clamp for near-tolerance accepted steps

- Motivation from the restored day-1 shelf:
  - after the material-change fix, the canonical hard water repro returned to the old oscillatory regime rather than the `50643`-history collapse
  - captured day-1 step trace on `wf_p_12x12x3` again showed the same pattern at boundary cell `143` / water row `429`:
    - repeated accepted steps at the same hotspot with residuals rising through roughly `3.1e-6`, `5.2e-6`, `8.4e-6`, `9.7e-6`
    - controller regrowth stayed at `1.25x` because Newton iteration count was still low and state changes were still small
    - the next growth step crossed back over tolerance (`~1.36e-5` to `~1.46e-5`) and stagnated after a few Newton iterations before retrying at about half-step size
- First attempted policy change did not help enough:
  - a stronger hotspot-memory policy with a remembered failed-`dt` guard band was tested first
  - result on `wf_p_12x12x3` day 1: `history += 147`, worse than the restored `139` baseline, so that version was not kept
- Change kept instead:
  - `src/lib/ressim/src/fim/timestep.rs` now computes the accepted-step growth factor with an additional residual-based clamp
  - if an accepted step already lands near the residual tolerance, the next growth factor is reduced even when Newton iterations are still low and the pressure/saturation changes are still small
  - this directly targets the observed `accept-near-tolerance -> immediate regrow -> fail` staircase without adding broader hotspot-memory side effects
  - added focused coverage via `residual_near_tolerance_throttles_growth_factor`
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml cooldown -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml hotspot -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml residual_near_tolerance_throttles_growth_factor -- --nocapture` passed
  - locked Rust baseline reran green:
    - `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`
    - `spe1_fim_first_steps_converge_without_stall`
    - `spe1_fim_gas_injection_creates_free_gas`
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - healthy reference rerun:
    - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 24x1x1 --steps 1 --dt 1 --diagnostic summary --no-json`
    - outcome: `outer_ms ≈ 23.4`, `history += 6`, `warning=none`
  - canonical hard-case rerun:
    - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json`
    - previous restored baseline: `outer_ms ≈ 97585.1`, `history += 139`
    - new outcome: `outer_ms ≈ 95357.8`, `history += 133`, `warning=none`

Interpretation:

- The residual-aware growth clamp is a real improvement on the restored day-1 hard shelf: it reduced the canonical `wf_p_12x12x3` day-1 history count from `139` to `133` without breaking the locked Rust baseline.
- The simple 1D waterflood reference stayed healthy, with only a small shift from `history += 5` to `history += 6`, which is acceptable for now given the harder-case gain.
- The remaining blocker is still the same front-local outer-step oscillation at boundary cell `143`; the next slice should continue to target later shelf windows (`day 2-3`) and/or further reduce unnecessary regrowth from accepted near-tolerance states instead of reopening linear/CPR work.

## Validation Update - 2026-04-05 structured outer-step diagnostics and tracked SPE1 internal-step budget

- Change made:
  - added a structured outer-step diagnostic record `FimStepStats` in `src/lib/ressim/src/reporting.rs`
  - the FIM timestep driver in `src/lib/ressim/src/fim/timestep.rs` now stores, per outer step:
    - accepted substep count
    - retry split (`linear-bad`, `nonlinear-bad`, `mixed`)
    - accepted `dt` range
    - last accepted-step growth limiter (`max-growth`, `residual-margin`, etc.)
    - last retry hotspot family/row
  - exposed the stats to wasm via `getLastFimStepStats()` and `getFimStepStatsHistory()` in `src/lib/ressim/src/frontend.rs`
  - updated `scripts/fim-wasm-diagnostic.mjs` summary output to print the new structured metrics directly instead of requiring trace-only interpretation
  - upgraded `src/lib/ressim/src/fim/tests/spe1.rs` so the early SPE1 smoke now asserts a tracked internal-step budget instead of only checking `last_solver_warning`
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml residual_near_tolerance_throttles_growth_factor -- --nocapture` passed
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - canonical hard-case summary now prints the structured metrics directly:
    - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json`
    - observed outcome: `substeps=133`, `retries=3/14/0`, `dt=[5.968e-4,6.250e-2]`, `growth=max-growth`, `retry_dom=nonlinear-bad:water@429`
- Measured SPE1 baseline captured by the new budgeted smoke:
  - the first 1-day SPE1 step currently uses `16` accepted substeps with `2` nonlinear retries
  - the tracked-current smoke budget is therefore intentionally looser than the long-term target for now:
    - `<=20` accepted substeps
    - `<=2` nonlinear retries
    - no accepted `dt < 5e-3 d`

Interpretation:

- This does not solve the simple-case fragmentation problem yet, but it removes a real workflow gap: the solver now exposes structured outer-step evidence instead of forcing every diagnosis through raw trace parsing.
- SPE1 is now explicitly tracked as a convergence-quality budgeted case, not only a no-warning smoke. That gives the next slices a stable target that can be tightened as real fixes land.
- The canonical hard water repro remains dominated by the same nonlinear water hotspot, and the new summary surface now makes that visible in one line without replaying the full step trace.

## Validation Update - 2026-04-05 day-2 hotspot-site growth cap using the new outer-step stats

- Motivation from the new outer-step scan:
  - the stats-backed bounded scan showed the real expensive shelf had moved to day 2, not day 3
  - `wf_p_12x12x3` day 2 measured `218` accepted substeps with `4/27/0` retry split, while day 3 completed in a single full-day substep
  - replay of the saved day-2 checkpoint showed the same producer-corner cell site repeating, but current hotspot memory was not accumulating because the dominant retry alternated between:
    - water row `429`
    - oil row `430`
    - same cell item `143`
  - because hotspot identity was keyed to exact family/row, the controller treated those as different hotspots and returned to `growth=max-growth` after cooldown release
- Change made in `src/lib/ressim/src/fim/timestep.rs`:
  - broadened hotspot memory from exact row/family equality to site-level matching for cell residual families (`water`, `oil`, `gas`) using the same cell item index
  - added a narrow growth-policy hook so repeated hotspot sites no longer regrow at `max-growth`; repeated site memory now caps the accepted-step growth decision to no regrowth (`1.0x`) and reports limiter `hotspot-repeat`
  - added focused tests for:
    - alternating water/oil rows on the same cell counting as one hotspot site
    - repeated hotspot site memory capping the growth decision
- Validation:
  - focused timestep regressions passed:
    - `hotspot`
    - `cooldown`
    - `residual_near_tolerance_throttles_growth_factor`
  - locked Rust baseline reran green:
    - `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`
    - `spe1_fim_first_steps_converge_without_stall`
    - `spe1_fim_gas_injection_creates_free_gas`
  - day-2 targeted replay improved:
    - before: `substeps=218`, `retries=4/27/0`
    - after: `substeps=211`, `retries=4/24/0`
  - day-1 canonical summary moved slightly the wrong way:
    - previous baseline: `substeps=133`, `retries=3/14/0`
    - after this slice: `substeps=136`, `retries=3/13/0`

Interpretation:

- The new stats were useful: they exposed that the day-2 shelf was a repeated site-level oscillation that the previous exact-row hotspot memory could not see.
- The site-level hotspot cap is directionally correct and does reduce the targeted day-2 shelf, but only modestly.
- This is not yet the fundamental fix. The remaining evidence still points to a harder nonlinear/front-local issue rather than a controller-only problem, because even with better site-level memory the solver still revisits the same producer-corner cell shelf.
- The next slice should keep the new site-level memory but move up one level of leverage: use the same day-2 checkpoint to target the repeated near-converged producer-corner Newton state itself rather than only further tightening outer-step regrowth.

## Validation Update - 2026-04-05 rejected Newton-side day-2 experiments

- Two direct Newton-side experiments were tested against the saved day-2 checkpoint and then reverted after validation:
  - broad residual-shortcut tightening that reserved residual-only acceptance for the initial iterate
  - a restored bounded-candidate line-search / guard-equivalent acceptance path
- Observed outcome of the first experiment:
  - targeted day-2 replay regressed catastrophically from the tracked shelf to `1540` accepted substeps with `4/216/0` retries
  - dominant retries moved off the producer-corner water/oil shelf and into a later perforation-dominated path, so the change was clearly not the right local mechanism
- Observed outcome of the second experiment:
  - the tracked SPE1 smoke failed its current budget because day 1 dropped to `min_dt=3.93e-3 d` (< `5e-3 d`)
  - the day-2 checkpoint replay slowed enough that it was terminated before completion, so it was not worth keeping for further tuning
- Validation after revert:
  - restored wasm package rebuilt successfully via `bash ./scripts/build-wasm.sh`
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture` passed again on the reverted baseline

Interpretation:

- The remaining day-2 shelf is not improved by broad residual-shortcut tightening; that path destroys too much useful near-converged Newton acceptance.
- Reintroducing the older bounded-candidate / guard-equivalent machinery wholesale is also too blunt on the current codebase; it regresses the tracked SPE1 budget before proving any day-2 gain.
- The next Newton slice should stay narrower than both rejected experiments, most likely focusing on the tiny-damping / effectively zero-length candidate path at the producer-corner hotspot rather than rewriting general Newton acceptance again.

## Validation Update - 2026-04-05 checkpoint-scoped effective-move diagnostics at cell 143

- Change made in `src/lib/ressim/src/fim/newton.rs`:
  - added a checkpoint-oriented Newton trace hook that fires only when the dominant residual hotspot is cell `143` and the damped local move falls below the effective printed resolution of the trace (`<5e-3 bar`, `<5e-5 sat`)
  - the new line reports:
    - hotspot family/row at cell `143`
    - local damped `dP`, `dSw`, `dSo`, `dSg`
    - attached perforation context for that cell using the existing perforation diagnostics
  - focused unit coverage added for the effective-move threshold and local cell delta helpers
- Validation:
  - focused Rust tests passed:
    - `move_is_below_effective_trace_threshold_detects_rounds_to_zero`
    - `local_cell_move_deltas_tracks_pressure_and_phase_changes`
    - `entry_guard_does_not_accept_unchanged_previous_state`
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - saved day-2 checkpoint replay with full step diagnostics still preserved the tracked behavior:
    - summary rerun stayed at `substeps=211`, `retries=4/24/0`
  - the new trace lines now show repeated producer-corner tiny-move states such as:
    - `HOTSPOT effective-move floor cell143 row=430 damp=0.0000 local_dP=0.00000 ... attached_perfs=[perf1->well1 inj=false ...]`
    - `HOTSPOT effective-move floor cell143 row=429 damp=1.0000 local_dP=0.00000 ... attached_perfs=[perf1->well1 inj=false ...]`
    - occasional nonzero but still sub-threshold pressure moves like `local_dP≈6.8e-4` to `1.5e-3 bar` with all phase moves still below the trace-effective saturation threshold

Interpretation:

- This confirms the remaining day-2 shelf is repeatedly revisiting an effectively zero-length local move at the producer-corner hotspot, not a broad injector-producer coupled ambiguity.
- For the current cell-`143` shelf, producer-side perforation context is sufficient: every emitted hotspot-effective-move line attached only `perf1->well1 inj=false`, with no injector perforation attached to that cell.
- Injector diagnostics remain useful at the outer-step level for global pressure-support questions, but they are not the missing local diagnostic for the current producer-corner Newton shelf.
- The next Newton-state fix should therefore target the producer-corner tiny-move path directly, likely by treating these sub-threshold local updates as a distinct stagnation mode rather than by broad acceptance-policy rewrites.

## Validation Update - 2026-04-06 native producer-hotspot matcher landed, rebuilt wasm checkpoint still at 246

- Change made in `src/lib/ressim/src/fim/newton.rs`:
  - added a narrow producer-hotspot matcher that remembers a producer-only effective-zero move on a boundary producer cell and can classify the following same-cell stagnation as an immediate nonlinear retry
  - kept the scope intentionally narrow:
    - only phase-family hotspot rows (`water`, `oil`, `gas`)
    - only boundary cells on at least two boundary planes
    - only cells whose attached perforations are all producer perforations
  - added focused unit coverage for:
    - producer-boundary qualification
    - bail/no-bail behavior on same-cell vs different-cell follow-up stagnation
- Native validation:
  - `cargo test producer_hotspot_stagnation -- --nocapture` passed
  - `cargo test spe1_fim_first_steps_converge_without_stall -- --nocapture` passed
- Rebuilt wasm validation on current head:
  - rebuilt via `bash ./scripts/build-wasm.sh`
  - replayed the saved checkpoint with:
    - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --checkpoint-in /tmp/fim-scan-wf12-stats/step-0001.json --diagnostic summary --no-json`
  - current rebuilt-head outcome: `substeps=246`, `retries=0/29/0`, `dt=[4.039e-4,5.758e-3]`, `growth=max-growth`
  - filtered step replay shows repeated `HOTSPOT effective-move floor` lines at cell `143`, but no emitted `PRODUCER-HOTSPOT STAGNATION` lines on the rebuilt current head
- Interpretation:
  - the new native matcher is mechanically correct and regression-safe, but it is not yet the fundamental fix for the rebuilt wasm shelf on current head
  - the current rebuilt checkpoint no longer matches the older `211`-substep replay signature exactly; in the late window it often resolves in 2-3 Newton iterations with producer/perf-dominated convergence instead of tripping the specific same-cell stagnation sequence required by the new bailout
  - the immediate follow-up should now be to explain why the rebuilt current head sits at `246` accepted substeps on the same saved checkpoint before attempting another Newton acceptance rewrite or another well-aware CPR coarse-space experiment

## Scope

- Problem class: native FIM convergence and timestep fragmentation that appears mainly on 2D and 3D cases.
- Main repro case: `gas_10x10x3`.
- Related symptom: similar fragmentation also appears on water cases, especially 3D pressure-controlled waterfloods.
- Additional scope note from the March 2026 depletion review: current FIM work is not only about timestep convergence. The active FIM path also has correctness/parity gaps on simpler depletion-style cases where convergence warnings may stay quiet while rates and pressures still differ materially from the validated IMPES path.
- Non-goal: treating this as a pure tolerance-tuning problem before the nonlinear source is localized.

## Correctness Follow-up - 2026-03-31 depletion review

- Cross-check outcome:
  - direct depletion probes on `dep_pss` and `dep_decline` showed that FIM can produce materially different oil-rate and pressure responses from IMPES even when both runs complete without solver warnings
  - the gap is much larger than the expected analytical-vs-simulation model mismatch on these cases; it is a solver-path correctness issue, not just a chart/reference issue
  - this is consistent with the already-ignored mixed-control parity regression in `src/lib/ressim/src/lib.rs`
- Interpretation:
  - the current FIM backlog must explicitly track correctness/parity in addition to convergence and runtime
  - “no retry spiral” is not sufficient evidence that a FIM timestep path is acceptable for benchmark-style cases
- Suggested follow-up tests for the FIM workstream:
  - add a depletion parity smoke test for a simple single-producer closed system: compare FIM vs IMPES on total oil production, average pressure, and first-step rate for a bounded depletion case like `dep_pss`
  - add a late-time depletion-tail parity test for the Fetkovich-style slab: compare FIM vs IMPES after the early transient window, not only at the first accepted point
  - promote the existing ignored mixed-control parity benchmark into the canonical “correctness” bucket for FIM work instead of treating it as a side diagnostic
  - add a benchmark-payload routing guard so literature depletion seeds cannot silently fall back to FIM when the intended validated path is IMPES
  - when investigating future FIM fixes, record both convergence counters and a small correctness summary (`q_o`, `p_avg`, control mode / BHP-limit fraction) on the same repro step

## Reproduction Summary

### Stable reference

- `wf_p_24x1x1` is healthy and converges without the severe retry spiral seen in larger coupled cases.

### Failing signatures

- `gas_24x1x1` already shows timestep fragmentation, so the issue is not exclusive to 3D gas, but it worsens with stronger coupling.
- `gas_10x10x3` repeatedly cuts back to about `1.22e-4 d`.
- `wf_p_12x12x3` shows frequent iterative-solver fallback and heavy retry fragmentation.

### Dominant Newton pattern in `gas_10x10x3`

- After a tiny retry step is accepted, the next doubled trial starts with a very small residual at iteration 0.
- Newton iteration 1 then jumps immediately to a plateau around:
  - scaled residual `res ≈ 0.36–0.43`
  - scaled update `upd ≈ 0.20`
- The solve then stagnates and triggers another cutback.

Interpretation: the problematic behavior is not simply “hard first residual on the new step”. It is a repeatable post-acceptance nonlinear plateau after the first update.

## Isolation Matrix

### Gravity toggle

- Diagnostic case: `fim_debug_gas_10x10x3_no_gravity`
- Result: still collapses, but accepted floor improves from about `1.22e-4 d` to about `2.44e-4 d`.
- Conclusion: gravity aggravates the problem but does not explain the collapse.

### Pressure-controlled wells

- Diagnostic case: `fim_debug_gas_10x10x3_pressure`
- Result: materially worse than the baseline rate-controlled case, with accepted dt dropping below `1e-5 d` in captured runs.
- Conclusion: rate control is not the primary culprit; pressure control does not remove the pathology.

### Near-zero capillary

- Diagnostic case: `fim_debug_gas_10x10x3_no_capillary`
- Implementation note: capillary lambda cannot be zero, so this case uses `lambda = 1e-6`.
- Result:
  - linear solve becomes non-finite at Newton iteration 0
  - fallback solve returns zero update
  - all damped candidates fail finite/bounds checks
  - acceptance only occurs at very small dt around `9.77e-4 d`
- Conclusion: capillary is acting as important regularization. Removing it exposes a worse conditioning or bounds-handling issue instead of curing the plateau.

## Jacobian Diagnostic Findings

- Original test: `full_system_jacobian_matches_fd_for_rate_controlled_waterflood()`
- Current status: diagnostic-only.
- Reason: whole-system central finite difference across the coupled residual is too expensive to use routinely.
- Even a sampled-column version still ran for more than 60 seconds.

### Likely reasons the whole-system FD path is slow

- It recomputes the full residual for every perturbed unknown.
- It traverses non-smooth logic:
  - regime hysteresis
  - flash/clamp transitions
  - well-control switching behavior
  - mobility and bounds enforcement
- Producer-control neighborhood sampling adds extra repeated local work.

Conclusion: full-system central FD is useful as a deep diagnostic, but not as the main day-to-day localization tool.

## Current Diagnosis

- This does not currently look like a simple tolerance problem.
- The stronger evidence points to a real nonlinear/conditioning issue in the coupled FIM equations.
- The observed hierarchy is:
  - gravity is secondary
  - rate control is not the primary cause
  - capillary is stabilizing an otherwise more pathological solve

The most likely next target is the equation family that causes the jump from a tiny iteration-0 residual to the iteration-1 plateau.

## External Solver Comparison - 2026-03-29

Cross-checks against OPM, MRST, JutulDarcy, DuMux, MOOSE, openDARTS, and the SPE overview article did not point to a hidden timestep trick that would plausibly solve the current pathology by itself.

### Main recurring ideas

- OPM, MRST, JutulDarcy, and DuMux all keep timestep adaptation logically separate from Newton line-search damping.
- DuMux, MRST, JutulDarcy, and MOOSE all expose stronger Newton globalization than the current local binary accept-or-cut path here:
  - backtracking line search
  - relaxation/dampening
  - bounded or chopped updates
  - iteration-count-aware timestep selection
- JutulDarcy and OPM both expose stronger CPR variants for well-coupled systems than the current reservoir-only reduced pressure path here.
- DuMux reduces residual/Jacobian drift risk by routing assembly through a more unified assembler interface and optionally numeric differentiation / partial reassembly.
- openDARTS is useful mainly as an example of explicit iteration accounting, engine/operator separation, and strong diagnostic surfacing of Newton and linear iteration counts, but the public docs were not detailed enough to extract a concrete nonlinear fix beyond that.

### Implication for the current `wf_p_12x12x3` hotspot

- The previous recommendation still holds: do not spend the next slice on more timestep heuristics.
- The most credible next fixes are still:
  1. residual/Jacobian consistency corrections
  2. stronger Newton globalization / bounded updates
  3. better well-aware CPR after the nonlinear path is less fragile

### Prioritized next plan

1. Fix the identified water-gravity mismatch between residual and exact Jacobian face terms and validate on the hard wasm repro.
2. If fragmentation remains, replace the current residual-improved acceptance rule with a stronger sufficient-decrease backtracking rule.
3. Add explicit update limits for pressure and saturation in the Newton candidate path instead of relying only on reject/retry.
4. Revisit the CPR reduction so wells and tail variables influence the pressure stage for large coupled systems.
5. Only after those steps, retune timestep growth and cutback targets using iteration-count feedback.

## Validation Update - 2026-03-29 water-gravity Jacobian patch

- Change made: the exact interface Jacobian now includes the missing water gravity term in `dphi_w`, matching the residual-side water potential expression.
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml zero_residual_scaffold_converges_in_one_newton_step -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture` passed
  - wasm rebuild succeeded
  - hard repro `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json` still fragmented badly
- New observed hard-repro outcome:
  - `FIM step done: 2063 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=6 nonlinear-bad=2057 mixed=0`
  - repeated failed doubled steps now reported `dom=oil` instead of the earlier dominant `dom=water`

Interpretation:

- The patch fixed a real consistency gap, but it was not the main driver of the micro-substep spiral.
- The dominant nonlinear bottleneck moved rather than disappearing, which makes the next best target the Newton acceptance/globalization path rather than another small flux-term correction.

## Validation Update - 2026-03-29 stronger Newton sufficient decrease

- Change made: Newton candidate acceptance now requires an Armijo-like sufficient decrease instead of accepting any finite residual improvement, while outer retry-factor policy remains separate from damping.
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml sufficient_decrease_rule_requires_more_than_any_improvement` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml failure_classification_marks_fallback_path_as_linear_bad` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall` passed
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - hard repro `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json` still fragments, but materially less than the previous baseline
- New observed hard-repro outcome:
  - `FIM step done: 1491 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=7 nonlinear-bad=1483 mixed=0`
  - repeated failed doubled steps are again dominated by `dom=water`, with failure traces now reporting both `cand_res` and the sufficient-decrease `cand_target`

Interpretation:

- The stronger acceptance rule is the first change in this sequence that materially reduced the micro-substep spiral on the hard wasm case.
- The remaining failure pattern is still overwhelmingly nonlinear and still concentrated in the same near-converged doubled-step retry cycle, so the next best lever remains bounded update controls rather than timestep heuristics.

## Validation Update - 2026-03-29 bounded Newton pressure/saturation updates

- Change made: the Newton damping path now uses the same `200 bar / 0.2 sat` trust-radius targets as the outer timestep driver, limits implied oil-saturation movement in saturated cells (`So = 1 - Sw - Sg`), and rejects damped candidates that exceed explicit pressure/saturation bounds before residual acceptance.
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml appleyard_damping_limits_combined_oil_saturation_change` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml candidate_update_bounds_include_oil_saturation_change` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml sufficient_decrease_rule_requires_more_than_any_improvement` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall` passed
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - hard repro `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json` still fragments and regressed slightly versus the sufficient-decrease-only baseline
- New observed hard-repro outcome:
  - `FIM step done: 1570 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=6 nonlinear-bad=1564 mixed=0`
  - the repeated failed doubled-step loop now reports `cand_dP=0.00` and `cand_dS=0.0000` alongside the residual trace, indicating the bounded-update controls are not what is blocking acceptance in that near-converged retry cycle

Interpretation:

- The bounded-update controls are implemented correctly and cover a real gap: the old chop could limit `ΔSw` and `ΔSg` individually without limiting the implied `ΔSo`.
- On the current hard waterflood repro, however, the remaining micro-substep loop is not being driven by large pressure or saturation moves. The near-failure candidates are already effectively zero-change states, so the next lever should move away from tighter state bounds and toward the residual/convergence logic around those near-no-op doubled-step retries.

## Validation Update - 2026-03-29 guard-band equivalent material-candidate acceptance

- Change made: inside the Newton damping loop, materially changed candidates are now allowed through when the iterate is already inside the residual guard band and the candidate residual is only numerically equivalent to the current residual. This is intentionally narrower than loosening unchanged-state acceptance: unchanged iterates are still blocked by the no-op guards.
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml entry_guard_does_not_accept_unchanged_previous_state` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml guard_band_equivalent_residual_allows_numerically_equal_candidate` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml guard_band_equivalent_residual_rejects_candidate_outside_guard_band` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_coarse_grid_reaches_producer_gas_breakthrough` passed
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - hard repro `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json` improved dramatically
- New observed hard-repro outcome:
  - `FIM step done: 138 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=6 nonlinear-bad=128 mixed=0`
  - the trace now shows repeated `[guard-equiv]` hits on the previously problematic doubled-step retries, followed by a second Newton iteration that converges instead of cutting the timestep immediately

Interpretation:

- This directly addresses the remaining micro-substep spiral without reopening the old no-op acceptance bug: unchanged states are still rejected, but materially changed candidates no longer get bounced just because their residual is worse only at roundoff level.
- The improvement is large enough that the next priority should stay on the linear/preconditioning side rather than further convergence-guard tweaking unless a new regression appears.

## Validation Update - 2026-03-29 CPR tail prolongation into well/scalar unknowns

- Change made: the CPR pressure correction now prolongates directly into the scalar tail block using the same local Schur data already used to build the coarse pressure system, so well-BHP and perforation-rate unknowns receive an explicit coarse-stage update instead of depending entirely on the post-pressure smoother.
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml pressure_projection_updates_tail_unknowns_from_coarse_correction` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml pressure_rhs_accounts_for_tail_schur_coupling` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml cpr_report_exposes_coarse_diagnostics` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_coarse_grid_reaches_producer_gas_breakthrough` passed
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - hard repro `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json` remained effectively unchanged
- New observed hard-repro outcome:
  - `FIM step done: 138 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=6 nonlinear-bad=128 mixed=0`

Interpretation:

- The explicit tail prolongation is worth keeping because it makes the CPR stage more internally consistent and more well-aware without harming the validated gas/water-front propagation tests.
- On the current hard waterflood repro it does not materially change the retry shelf, which suggests the remaining bottleneck is not simply “the coarse pressure stage fails to move tail unknowns.” The next productive slice should target coarse-system quality or outer-step policy rather than this specific prolongation path.

## Validation Update - 2026-03-31 reviewed head and current hard-case baseline

- Commit review summary:
  - `325e19b` is the latest materially relevant FIM commit.
  - `d58c35b` is mostly frontend/runtime plumbing and is not materially relevant to the current FIM convergence issue.
- Relevant solver changes in `325e19b`:
  - `src/lib/ressim/src/fim/newton.rs` now clips water-saturation moves against the water fractional-flow inflection point inside `appleyard_damping(...)`.
  - `src/lib/ressim/src/step.rs` now uses a more conservative OPM-like growth rule (`MAX_GROWTH = 1.25`, `MIN_GROWTH = 0.75`, target Newton iterations `8`) and includes gas saturation in the accepted-step growth estimate.
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_coarse_grid_reaches_producer_gas_breakthrough -- --nocapture` passed
  - wasm rebuild succeeded
  - hard repro `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json` improved again
- New observed hard-repro outcome:
  - `FIM step done: 130 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=4 nonlinear-bad=30 mixed=0`

Interpretation:

- The reviewed head is a net improvement over the earlier `138`-substep baseline.
- The cheap static water-inflection clip is useful, but it is not the full adaptive interface-local trust-region idea from the recent OnePetro article. It is better viewed as a safe local guard than as the end-state nonlinear strategy.

## Validation Update - 2026-03-31 checkpoint scan/replay workflow for mid-run shelves

- Change made: the canonical wasm diagnostic path now supports a cheap `outer` scan plus checkpoint save/load, so long scenarios can be scanned for expensive windows and only the interesting outer steps need full Newton replay.
- Validation:
  - water checkpoint/restart was validated on `water-pressure --grid 5x5x3`
  - gas checkpoint/restart was validated on `gas-rate --grid 4x1x1`
  - bounded long-window scans then ran on:
    - `water-pressure --grid 5x5x3 --steps 8 --dt 0.25 --diagnostic outer`
    - `water-pressure --grid 12x12x3 --steps 6 --dt 1 --diagnostic outer`
    - `gas-rate --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic outer`
  - targeted replays then ran from saved checkpoints on:

## Validation Update - 2026-03-31 used_fallback semantic audit and corrected canonical shelf replay

- Change made:
  - audited `used_fallback` end-to-end across Newton retry classification and linear solver reporting
  - fixed `src/lib/ressim/src/fim/linear/mod.rs` so normal requested-backend dispatch no longer marks `used_fallback=true`
  - fixed `src/lib/ressim/src/fim/newton.rs` so only Newton's real retry-time fallback path sets `linear_report.used_fallback = true`
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml default_fim_solver_uses_iterative_fallback_before_sparse_lu -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml large_default_fim_system_still_uses_iterative_backend -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml failure_classification_ -- --nocapture` passed
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - canonical wasm replays reran on:
    - `water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json`
    - `water-pressure --grid 10x10x3 --steps 1 --dt 1 --diagnostic step --no-json`
- Corrected observed outcomes after the semantic fix:
  - `wf_p_12x12x3`: `FIM step done: 137 substeps, advanced 1.000000 of 1.000000 days`
  - `wf_p_12x12x3`: retry counts are `nonlinear-bad=6`, `linear-bad=0`, `mixed=0`
  - `wf_p_10x10x3`: `FIM step done: 563 substeps, advanced 1.000000 of 1.000000 days`
  - `wf_p_10x10x3`: retry counts are `nonlinear-bad=6`, `linear-bad=0`, `mixed=0`
  - failed retries now show strong CPR reduction without any bogus fallback signal; representative failure traces remain damping failures on water-dominated hotspot rows with CPR reduction ratios around `1e-13` to `1e-14`

Interpretation:

- The earlier replay result that suggested a `linear-bad` majority on the water shelves was a reporting artifact caused by overloaded `used_fallback` semantics.
- With corrected semantics, the canonical shelves are again cleanly dominated by nonlinear retry failures, and the linear/CPR path no longer looks like the next bottleneck on these cases.
- The next productive slice should move back to nonlinear/timestep policy at the hotspot shelf, specifically a stronger repeated-hotspot failure-memory controller rather than more generic CPR tuning.

## Implementation Update - 2026-03-31 hotspot-aware timestep memory

- Change made:
  - extended `src/lib/ressim/src/step.rs` so timestep cooldown now tracks the dominant nonlinear retry hotspot (`family`, `row`, `item`) instead of treating every retry identically
  - repeated nonlinear failures on the same hotspot can now lengthen the regrowth hold budget, while linear-bad failures explicitly do not seed hotspot memory
  - hotspot memory now decays separately from the immediate cooldown cap after clean accepted steps without retry
  - added focused unit coverage for:
    - repeated same-hotspot extension
    - hotspot reset on a different failure site
    - linear failures not seeding hotspot memory
    - hotspot-memory decay after clean steps
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml hotspot -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml cooldown -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture` passed earlier in the same implementation pass
  - wasm rebuild succeeded via `bash ./scripts/build-wasm.sh`
  - canonical wasm replay reran on `water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json`
- Observed canonical outcome after tuning the first hotspot-memory pass:
  - `FIM step done: 140 substeps, advanced 1.000000 of 1.000000 days`
  - retry counts remain `nonlinear-bad=6`, `linear-bad=0`, `mixed=0`
  - the tuned replay mostly reported `repeats=1`, so the stronger repeated-hotspot branch did not materially engage on the canonical day-1 shelf

Interpretation:

- The control-path plumbing is now in place: timestep policy can react to dominant retry hotspots instead of only a generic retry/no-retry signal.
- On the current day-1 canonical hard shelf, however, the first tuned policy is effectively neutral to slightly conservative: it does not reduce the number of nonlinear retries and does not yet beat the earlier `137-138` substep baseline.
- The next tuning slice should focus on when hotspot memory is allowed to accumulate across regrow-fail cycles, because the current tuned pass is still not engaging the repeated-hotspot branch enough on the main repro.
    - `wf_p_5x5x3` around day `0.75 -> 1.00`
    - `wf_p_12x12x3` around day `1.00 -> 2.00`
    - `gas_10x10x3` around day `1.00 -> 1.25`

### Scan findings

- `wf_p_5x5x3`
  - day `0.25`: `~1.20 s`, `history+=409`
  - day `0.50`: `~1.82 s`, `history+=625`
  - day `0.75`: `~2.52 s`, `history+=779`
  - day `1.00`: `~1.18 s`, `history+=340`
  - days `1.25-2.00`: only `~4-8 ms`, `history+=1`
- `wf_p_12x12x3`
  - day `1.00`: `~69.4 s`, `history+=137`
  - day `2.00`: `~79.3 s`, `history+=1219`
  - day `3.00`: `~77.8 s`, `history+=927`
  - day `4.00`: `~15.0 s`, `history+=28`
  - days `5.00-6.00`: `~1.0-1.4 s`, `history+=1`
- `gas_10x10x3`
  - days `0.25-1.50`: stays in the `~1.0-2.3 s` range with only `3-11` history points per outer step and no warnings

Interpretation:

- The hard water shelf is not purely a day-`1` phenomenon; the worst replay target is later, at days `2-3`.
- The small-grid water case is front-loaded. Once the front passes the early difficult window, the remaining outer steps become nearly trivial.
- The representative gas case does have later nonlinear stress, but it does not currently show the same catastrophic outer-step fragmentation as the hard water case.

### Replay findings

- `wf_p_5x5x3` replay from day `0.75 -> 1.00`
  - still uses `dense-lu` at `229` rows
  - no warning or hard failure signature appeared in the replayed window
  - the expensive step is dominated by hundreds of tiny accepted substeps with low residuals and low per-substep work, not by repeated catastrophic retries
- `gas_10x10x3` replay from day `1.00 -> 1.25`
  - stays on `fgmres-cpr` at `904` rows with healthy CPR residuals
  - one initial `nonlinear-bad` gas-dominated failure occurs on the full `0.25 d` attempt, but the retry ladder resolves quickly (`0.125`, `0.09375`, `0.03125`) and the step finishes with `linear-bad=0`, `nonlinear-bad=1`
  - this points more toward a localized nonlinear shelf than to iterative linear failure
- `wf_p_12x12x3` replay from day `1.00 -> 2.00`
  - stays on `fgmres-cpr` at `1300` rows
  - repeated failed doubled steps are still dominated by a water-family hotspot at boundary cell `143` (`(11,11,0)`, row `429`)
  - the characteristic trace is: very small residual, `[guard-equiv]`, then stagnation after several nearly identical damped candidates, then retry to a slightly smaller `dt`
  - the replayed step ends with `linear-bad=3`, `nonlinear-bad=242`, `mixed=0`

Interpretation:

- The remaining hard `12x12x3` shelf is now localized as a true front-local nonlinear staircase rather than a hidden CPR breakdown. Linear solves remain healthy enough to keep advancing, but Newton keeps revisiting nearly equivalent tiny-step water-front states on the same boundary hotspot.
- The next nonlinear slice should therefore target this oscillatory near-front acceptance pattern directly, while continuing to use the checkpoint workflow to sample later windows instead of assuming the main shelf starts at day `0`.

## Current dominant retry pattern - 2026-03-31

- The old inner-Newton no-op acceptance bug is no longer the main driver of the retry shelf.
- The remaining deterministic pattern on the hard waterflood repro is:
  1. a reduced retry timestep is accepted in 1-2 Newton iterations
  2. the controller immediately regrows by about `1.25x`
  3. the next trial lands in the same hotspot, stagnates after a few Newton iterations, and fails
  4. the timestep is cut back and the cycle repeats

Interpretation:

- The next highest-value convergence change is now outer-step policy, not another generic Newton acceptance tweak.
- The specific gap is lack of failure memory or cooldown in timestep growth. Once a `dt` has just failed at a given state/front location, the controller should not immediately regrow back into the same failing regime after one easy retry acceptance.

## Bounded scaling investigation - 2026-03-31

User priority for this slice was broader than the hard repro:

- reduce substeps on cases that should not require heavy fragmentation
- understand why moderate grids like `10x10x3` become impractically slow
- keep all larger-grid probing bounded and safe

### Healthy bounded reference

- Command:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 24x1x1 --steps 1 --dt 1 --diagnostic step --no-json`
- Observed outcome:
  - `FIM step done: 5 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=1 nonlinear-bad=0 mixed=0`

Interpretation:

- The current solver can still advance a simple 1D waterflood day in a small number of substeps.
- That rules out a blanket statement like “the current growth policy always fragments 1-day steps.”

### Bounded 3D moderate-grid trace

- Command used for a bounded probe:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 10x10x3 --steps 1 --dt 1 --diagnostic step --no-json`
- Observed tail behavior during the bounded capture:
  - `n_cells = 300`
  - repeated accept/regrow/fail ladders at tiny `dt`
  - failures classified as `linear-bad`, but with the same near-stagnant local residual pattern already seen on the hard waterflood case
  - the bounded capture was still deep into hundreds of substeps at very small `dt`

Interpretation:

- The remaining 3D substep explosion is not just a runtime issue caused by a slower linear solve.
- Even at a moderate `300` cells, the 3D case is still trapped in the same local retry/regrow loop, so there is still a real nonlinear/front-local convergence problem to address.

## Linear-backend threshold diagnosis - 2026-03-31

- Confirmed in `src/lib/ressim/src/fim/linear/mod.rs`:
  - native direct threshold: `512` coupled rows
  - wasm direct threshold during the first trace pass: `1024` coupled rows
- Coupled rows are roughly:
  - `3 * n_cells + n_wells + n_perforations`

Implications:

- `10x10x3` with two wells already sits around the wasm direct-threshold neighborhood and above the native direct-threshold neighborhood.
- That creates a real backend cliff for runtime/scaling independent of the nonlinear retry problem.
- For user-reported `5x5x3 -> 10x10x3` pain, the likely picture is mixed:
  - nonlinear/front-local failure memory is still missing, so the solver takes far too many substeps
  - backend dispatch also crosses into a more expensive regime earlier than the grid size alone would suggest

## Current recommendation order - 2026-03-31

1. Add failure-memory-aware timestep growth cooldown in `step.rs` so recently failed timesteps are not immediately regrown.
2. Expose row count and backend-used information in the canonical wasm diagnostic output so the threshold cliff can be measured directly on moderate grids.
3. Revisit the moderate-system linear/coarse solve strategy after the cooldown slice, because near-threshold dispatch is likely part of the runtime cliff even when convergence improves.
4. If stronger nonlinear controls are still needed after that, prefer an oscillation-triggered localized trust-region approach over another globally stricter damping rule.

## Validation Update - 2026-03-31 repeated-hotspot guard-equiv suppression experiment

- Change made: added a narrow Newton-side experiment in `src/lib/ressim/src/fim/newton.rs` that suppressed guard-band-equivalent acceptance when the same residual hotspot repeated under stagnation with tiny candidate state changes.
- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml repeated_same_hotspot_guard_equiv_is_suppressed_when_stagnating -- --nocapture` passed while the experiment was present
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_ -- --nocapture` passed (`3 passed`)
  - checkpoint replay of the canonical hard shelf ran via `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --checkpoint-in /tmp/fim-scan-wf12/step-0001.json --steps 1 --dt 1 --diagnostic step --no-json`
- Observed hard-replay outcome with the experiment enabled:
  - `FIM step done: 1219 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=3 nonlinear-bad=242 mixed=0`
  - the trace still recycled through the same boundary-water hotspot at `row=429`, `cell143=(11,11,0)` and still accepted repeated `[guard-equiv]` steps into the same micro-substep shelf

Interpretation:

- This hook did not move the actual hard-case metric at all. The shelf remained identical to the established replay baseline, so repeated-hotspot guard-equiv suppression is not the missing control.
- Because the experiment added solver complexity without measurable benefit, it was removed after validation rather than left in place behind dead logic.
- The next nonlinear slice should target a different mechanism than guard-equiv suppression, most likely outer-step memory/trust behavior or a more explicit localized oscillation detector that changes timestep policy rather than only candidate acceptance.

## Implementation Update - 2026-03-31 cooldown plus canonical backend diagnostics

- Code changes:
  - `src/lib/ressim/src/step.rs`
    - added a small failure-memory-aware growth cooldown state
    - accepted retry steps now freeze regrowth at the proven-safe accepted `dt`
    - the cap is released only after two clean accepted steps without another retry
    - step-level accept/fail trace lines now include the actual linear backend used plus row count and active direct threshold
  - `src/lib/ressim/src/fim/linear/mod.rs`
    - added backend labels plus a helper for the active direct-solve row threshold
  - `src/lib/ressim/src/fim/newton.rs`
    - Newton trace now prints `n_perfs`, total coupled rows, requested linear backend, active direct threshold, and the actual backend used on each linear solve

- Validation:
  - `cargo fmt --manifest-path src/lib/ressim/Cargo.toml` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml retry_acceptance_freezes_growth_until_clean_success_budget_is_spent -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml cooldown_clamp_never_exceeds_remaining_dt -- --nocapture` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture` passed
  - `bash ./scripts/build-wasm.sh` succeeded

### Healthy 1D diagnostic after the trace upgrade

- `wf_p_24x1x1` now clearly reports:
  - `n_rows = 76`
  - `req_lin = fgmres-cpr`
  - `used = dense-lu`
  - `direct_thr = 1024`
- It still completes the 1-day step in `5` substeps with `linear-bad=1 nonlinear-bad=0 mixed=0`.

Interpretation:

- The canonical wasm trace now makes it explicit when a case is safely under the wasm direct threshold.
- This is the reference pattern for a healthy small system.

### Hard 3D diagnostic after the trace upgrade

- `wf_p_12x12x3` now clearly reports:
  - `n_rows = 1300`
  - `req_lin = fgmres-cpr`
  - `used = fgmres-cpr`
  - `direct_thr = 1024`
  - explicit cooldown lines such as:
    - `trial_dt=0.000663 (retry=0) [cooldown-clamped from 0.000829 cooldown_cap=0.000663 clean_left=2]`
- Latest rerun retry summary:
  - `linear-bad=4 nonlinear-bad=21 mixed=0`

Interpretation:

- The new trace confirms the hard 3D case is not a near-threshold direct-solve case on wasm; it is already firmly in the iterative CPR path.
- That means the new diagnostic is already useful for design decisions: for this case, improving near-threshold direct/iterative dispatch alone will not eliminate the retry shelf.
- The cooldown is doing its intended job. The trace now shows the controller holding the accepted retry `dt` for two clean solves before allowing regrowth again, instead of immediately stepping back into the last failed `dt`.

## Updated recommendation order - 2026-03-31

1. Use the new canonical trace on moderate-grid cases near the threshold cliff (`5x5x3`, `10x10x3`, similar waterfloods) to determine whether they are still using direct solve or already paying CPR cost.
2. Improve the moderate-system linear/coarse solve path using that evidence, rather than guessing a new threshold or backend dispatch policy.
3. Revisit cooldown tuning only after step 2 if the current two-clean-step hold proves too conservative or too weak.
4. If nonlinear controls are still needed after that, prefer localized oscillation-triggered trust-region behavior over another globally stricter damping rule.

## Implementation Update - 2026-03-31 retry-failure classification audit

- Code change:
  - `src/lib/ressim/src/fim/newton.rs`

- What changed:
  - retry classification no longer treats every direct backend (`dense-lu` / `sparse-lu`) as if it were a fallback path; it now keys fallback detection off `report.used_fallback`
  - clean converged direct solves are now classified as `nonlinear-bad`, which is the intended meaning when the linear stage has already been resolved accurately
  - `fgmres-cpr` retries now need a materially strong coarse-pressure signal before being labeled `nonlinear-bad`
    - current rule: `coarse_applications > 0`, `avg_rr <= 0.25`, and `last_rr <= 0.5`
  - weaker CPR behavior is now classified as `mixed` instead of being collapsed into the old single-threshold split
  - retry trace suffix now includes both `cpr_avg_rr` and `cpr_last_rr`

- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml failure_classification_ -- --nocapture` passed
  - focused regressions now cover:
    - clean strong-CPR path -> `nonlinear-bad`
    - clean direct backend path -> `nonlinear-bad`
    - explicit fallback path -> `linear-bad`
    - weak CPR path -> `mixed`

Interpretation:

- The old classifier had a real defect: on small/direct systems it could mislabel a clean direct linear solve as a linear fallback problem just because the backend label was direct.
- The new split is still intentionally simple, but it is materially more trustworthy for the next diagnostic slice:
  - direct-solve cases are no longer polluted by false `linear-bad` labels
  - CPR cases with only mediocre pressure-stage reduction will no longer be over-read as purely nonlinear shelves
- The next replay pass should use the updated retry labels to decide whether the remaining hard shelves are still predominantly nonlinear after this audit or whether some windows are actually landing in `mixed` territory and should redirect effort back toward CPR quality.

## Validation Update - 2026-03-31 replay after retry-classification audit

- Validation:
  - rebuilt wasm via `bash ./scripts/build-wasm.sh`
  - replayed canonical water shelves:
    - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json`
    - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 10x10x3 --steps 1 --dt 1 --diagnostic step --no-json`

### Updated retry summaries

- `wf_p_12x12x3`
  - `FIM step done: 156 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=17 nonlinear-bad=2 mixed=0`
- `wf_p_10x10x3`
  - `FIM step done: 740 substeps, advanced 1.000000 of 1.000000 days`
  - `FIM retry summary: linear-bad=91 nonlinear-bad=2 mixed=0`

### Trace-level observations

- Both cases still fail on the same producer-corner water rows (`cell143` on `12x12x3`, `cell99` on `10x10x3`) during the retry/regrow shelf.
- CPR reduction remains numerically strong on the failing retries:
  - representative `12x12x3` failed retry: `cpr_avg_rr ≈ 1e-13`, `cpr_last_rr ≈ 1e-14`
  - representative `10x10x3` failed retry: `cpr_avg_rr ≈ 1e-13`, `cpr_last_rr ≈ 1e-14`
- Despite that, the failure suffix now reports `fallback=true` on the failed retries, which is what flips those events into `linear-bad`.
- The same failing retries still show the old structural pattern:
  - tiny candidate state changes (`cand_dP ≈ 0`, `cand_dS ≈ 1e-4`)
  - no sufficiently decreasing candidate at the regrown `dt`
  - immediate acceptance again after cutback and cooldown clamp

Interpretation:

- The classification audit changed the headline diagnosis: the remaining shelves are no longer reading as predominantly `nonlinear-bad`.
- But the new traces also exposed a likely next audit target: the meaning of `used_fallback` / `fallback=true` on these retries needs to be checked carefully, because the same trace line still shows `used=fgmres-cpr` with extremely strong CPR reduction.
- So the next highest-value slice is not another timestep tweak yet. It is to audit the linear-report semantics around fallback and failed retry classification:
  - determine whether these retries are genuinely hitting the direct fallback path and still failing afterward
  - or whether `used_fallback` is being latched more broadly than intended on the failing Newton path
- After that audit, the path should split cleanly:
  - if fallback is real, move next toward CPR / linear-backend quality
  - if fallback semantics are overstated, return to hotspot-aware timestep memory with better confidence

## Implementation Update - 2026-03-31 wasm threshold alignment for moderate grids

- Code change:
  - `src/lib/ressim/src/fim/linear/mod.rs`
    - aligned the wasm direct-solve row threshold with the native threshold at `512`
    - factored the target-aware direct-dispatch choice into a small helper so the handoff is explicit and testable
    - added regressions covering the wasm handoff above `512` rows and the explicit sparse-LU override case

- Validation:
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml wasm_target_` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml large_default_fim_system_still_uses_iterative_backend` passed
  - `cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall` passed
  - `bash ./scripts/build-wasm.sh` succeeded

### Moderate-grid canonical wasm comparison after the threshold change

- `wf_p_5x5x3`
  - coupled rows: `229`
  - backend before: `dense-lu`
  - backend after: `dense-lu`
  - bounded 1-day wasm diagnostic:
    - before: `outer_ms = 6751.7`, `history += 2159`
    - after: `outer_ms = 6172.5`, `history += 2159`
- `wf_p_10x10x3`
  - coupled rows: `904`
  - backend before: `dense-lu`
  - backend after: `fgmres-cpr`
  - bounded 1-day wasm diagnostic:
    - before: `outer_ms = 102085.9`, `history += 755`
    - after: `outer_ms = 17167.1`, `history += 563`
  - representative updated Newton header / linear trace:
    - `n_rows = 904`
    - `req_lin = fgmres-cpr`
    - `used = fgmres-cpr`
    - `direct_thr = 512`
    - `linear_iters = 10..12` with CPR diagnostics instead of the previous `dense-lu` fallback path

Interpretation:

- The canonical moderate-grid trace confirmed the original diagnosis: the old wasm `1024` direct threshold was too high and was keeping `10x10x3` on the wrong backend.
- Lowering the wasm threshold to `512` removes that moderate-grid backend cliff without perturbing genuinely small direct-solve cases like `5x5x3`.
- The remaining `5x5x3` pain is not a backend-dispatch issue. It is still dominated by the same local nonlinear/front retry ladder and therefore points back to timestep/globalization behavior rather than linear-dispatch policy.

### Hard-case sanity check after the threshold change

- `wf_p_12x12x3` remains on CPR as expected:
  - `n_rows = 1300`
  - `used = fgmres-cpr`
  - `direct_thr = 512`
  - bounded 1-day wasm diagnostic remained in the same general regime with `outer_ms = 69380.2` and `history += 137`

Updated next focus:

1. Revisit the remaining small-grid and hard-3D nonlinear/front-local retry shelves now that the wasm moderate-grid backend handoff is corrected.
2. Only revisit the iterative near-threshold backend details again if new evidence shows CPR coarse solves or restart policy are now the dominant runtime cost.
3. Revisit cooldown tuning after that if the two-clean-step hold is clearly leaving performance on the table.

## Added Diagnostics

### Native debug scenarios

- `fim_debug_gas_10x10x3_no_gravity`
- `fim_debug_gas_10x10x3_no_capillary`
- `fim_debug_gas_10x10x3_pressure`

Location: `src/lib/ressim/src/tests/fim_debug.rs`

### Assembly-side FD helpers

- full-system FD fixture helper
- selected-column FD assertion helper

Location: `src/lib/ressim/src/fim/assembly.rs`

## Next Diagnostic Slice

- Add row-family residual diagnostics inside the Newton trace.
- Goal: report which residual family dominates each failing iteration, rather than only printing the overall scaled infinity norm.
- Expected initial families:
  - cell water rows
  - cell oil-component rows
  - cell gas-component rows
  - well-constraint rows
  - perforation-flow rows

This should tell us whether the plateau is driven mainly by cell transport/accumulation, well control, or perforation coupling.

## Row-Family Diagnostic Notes

### First live trace from `gas_10x10x3`

- Early failing Newton iterations are usually dominated by the gas-component cell residual.
- The dominant row repeatedly appeared at `row = 2`, which corresponds to the gas-component equation of cell 0.
- In practical terms, the first plateau is currently pointing at the injector-side gas equation, not the well-constraint rows.
- On some deeper retries and smaller timesteps, the dominant family shifts to the first perforation-flow row (`perf0`, `row = 902` in the 300-cell/2-well case).

### Working interpretation

- The initial failure mode looks more like injector-side gas transport/accumulation coupling than a primary well-control FB residual problem.
- Perforation coupling becomes competitive later in the cutback spiral, so the instability may be propagating from the gas balance into the perforation equation rather than starting in the well-control row itself.
- This makes the next useful slice narrower: inspect the injector-cell gas residual terms and the first perforation equation together on the first bad doubled retry.

## Waterflood Diagnostic Notes

### Frontend `sweep_areal` analog

- Added a native debug case that matches the frontend baseline more closely:
  - `21×21×1`
  - pressure-controlled injector/producer
  - no gravity
  - no capillary effect in practice (`capillaryPEntry = 0`)
  - external `dt = 5 d`
- The native harness now prints per-outer-step summaries with oil rate, water rate, watercut, average pressure, and BHP-limited fractions.

### Observed slowdown timeline in `sweep_areal`

- Through about `90 d`, the case is cheap:
  - mostly `1` accepted substep per outer `5 d` step
  - watercut remains `0`
- At `95 d`, the first nonzero produced water appears:
  - `wc ≈ 0.0001`
  - substeps jump to `10`
- At `100 d`, near visible breakthrough onset:
  - `wc ≈ 0.0147`
  - substeps jump to `297`
- After that spike, the solver returns to `1` accepted outer-step substep even as watercut rises rapidly:
  - `105 d`: `wc ≈ 0.135`
  - `110 d`: `wc ≈ 0.268`
  - `125 d`: `wc ≈ 0.496`
  - `250 d`: `wc ≈ 0.883`

Conclusion: the dramatic slowdown is tightly localized around the first water breakthrough window, not the whole post-breakthrough period.

### Short 2D breakthrough diagnostic (`wf_bt_12x12x1`)

- This case already starts inside the hard part of the transient:
  - at `1 d`, `wc ≈ 0.433`
  - outer step 1 requires `5` accepted substeps
- In the failing retries of that first outer step, the dominant residual family evolves in a repeatable sequence:
  - initial dominance: cell water/oil rows at injector/producer corners
  - then dominance shifts hard to the perforation-flow row (`perf0`, row `434`)
  - later stalled retries move back toward producer-side water rows
- Several retries show the perforation row becoming much larger than the cell rows, for example:
  - `perf ≈ 45`, then `249`, then `3682` on successive stalled attempts during the first outer step

### Interpretation

- This is not a gas-only problem.
- Water breakthrough produces its own localized nonlinear difficulty.
- The water-side slowdown has a different signature from the gas-injection plateau:
  - gas case: often starts as injector-cell gas balance dominance
  - water breakthrough case: perforation-flow residual becomes the dominant row during the hard retry window
- The shared theme is still coupled well/cell nonlinearity at breakthrough, but the dominant equation family depends on the physics regime.

## Perforation Detail Trace

The Newton trace now emits an extra detail block whenever a perforation-flow row is the dominant residual family.

### What it reports

- perforation index and physical-well index
- injector/producer flag
- perforation unknown rate `q`
- clipped connection rate `conn`
- raw connection rate `raw = WI * mobility * drawdown`
- `WI`, total connection mobility, drawdown, cell pressure, and BHP
- when applicable, parent-well target rate, current total well rate from unknowns, and the current BHP/rate slacks

### Water breakthrough finding

- In the hard `wf_bt_12x12x1` retries, the injector perforation row (`perf0`) becomes dominant.
- The important pattern is not just that the perforation row is large; it is why:
  - the unknown perforation rate `q` often remains far smaller in magnitude than the connection law wants
  - examples from the trace show `q` in the range of roughly `-40` to `-1200 m3/d`, while the connection law simultaneously wants `-7e3` to `-2.6e4 m3/d`
- That creates very large residuals in the perforation row even when the cell residuals are already much smaller.

### Gas-case finding

- In the gas case, perforation dominance appears later than the injector gas-row dominance, but when it does appear the same structural mismatch is visible.
- In the milder gas retries, the perforation unknown is held at the control target (`q ≈ -500`) while the connection law wants only about `-300`, leaving a persistent perforation residual around `0.35–0.45` in scaled units.
- In the pathological gas retries, the same row can blow up catastrophically because the connection law goes non-physical:
  - examples show `p_cell → 0`, `bhp → 400–900`, and raw connection rates of order `-6e5` to `-1.6e6 m3/d`

### Working systematic interpretation

- This strengthens the case that there is a shared systematic issue, not two unrelated bugs.
- The common failure mechanism is in the coupled well/perforation solve:
  - the well-control part of the system keeps the perforation unknowns close to a control-implied total rate,
  - while the perforation connection relation `q = WI * mobility * drawdown` can demand a materially different rate,
  - and near breakthrough that mismatch can become the dominant nonlinear bottleneck.
- Water and gas differ in which equation family becomes dominant first, but both can fall into the same well/perforation inconsistency once the solve gets into the hard regime.

## Frozen-Cell Well Consistency Check

The perforation detail trace now also reports a frozen-cell locally consistent well state for the parent well:

- `frozen_bhp`: the BHP that makes the current cell state locally consistent with the active well control
- `frozen_q`: the connection-law perforation rate at that frozen-cell-consistent BHP
- `dq = q - frozen_q`
- for rate-controlled wells, `frozen_well_rate` and whether the consistent state is already BHP-limited

### What this separates

- It distinguishes two different failure modes that previously looked similar in the row-family trace:
  - the Newton iterate can be far away from the local well/perforation manifold even with cell states frozen,
  - or it can already be close to that manifold and the remaining difficulty is the coupled cell/BHP interaction itself.

### Pressure-controlled water breakthrough

- In both `wf_bt_12x12x1` and `wf_bt_12x12x3`, the frozen-cell-consistent injector state is simply `bhp = 500 bar`, and the corresponding connection-law perforation rates are still very large in magnitude.
- Representative 1D/2D breakthrough traces show `frozen_q` of roughly `-7e3` to `-2.6e4 m3/d` while the actual perforation unknown remains between about `-1200` and `+600 m3/d`.
- Representative 3D traces show the same pattern on both injector and producer perforations:
  - injector examples: `q ≈ -3.2e2` or even positive while `frozen_q ≈ -1.6e4` to `-2.0e4`
  - producer examples: `q ≈ 0` while `frozen_q ≈ +3.4e3` to `+6.5e3`
- That means the difficult water-breakthrough retries are not just small complementarity or scaling defects; the current iterate is genuinely far from the local pressure-controlled well/perforation manifold.

### Gas case split

- The gas case now separates into two sub-regimes.
- Mild perforation-dominant plateau:
  - `frozen_bhp` differs from the iterate by only about `0.1–0.3 bar`
  - `frozen_q ≈ -500 m3/d`, matching the current perforation unknown almost exactly
  - the remaining perforation residual comes from a relatively small BHP mismatch that keeps the connection rate near `-300` instead of `-500`
- Catastrophic gas retries:
  - the iterate can move to `bhp = 300–900 bar` with `p_cell -> 0`, while the frozen-cell-consistent state still wants `bhp ≈ 0.28 bar` and `q ≈ -500 m3/d`
  - in that regime the Newton iterate is again far from the local well/perforation manifold, not just slightly misaligned with it

### Updated interpretation

- The diagnostic evidence is now strong enough to separate symptom from mechanism.
- Shared mechanism:
  - hard failures in pressure-controlled water breakthrough and catastrophic gas retries are large-distance-to-local-manifold failures in the explicit well unknowns
- Secondary mechanism:
  - the milder gas plateau is a smaller BHP-coupling/stagnation problem near an otherwise reasonable local well state
- This is enough evidence to move from pure diagnosis toward a fix aimed at keeping `(bhp, q_perf)` closer to the frozen-cell-consistent well manifold during Newton updates, instead of letting the explicit well unknowns wander far away and asking later iterations to recover.

## Diagnostic Infrastructure Notes

- `test-native.sh` now runs the native debug tests with exact Rust test names, so one scenario no longer accidentally executes substring-matched variants.
- The native debug runner now prints a compact per-outer-step summary after each accepted external step.

## Industry Comparison Check

Before changing the solver, the proposed direction was checked against public implementations and documentation for industry-style fully implicit simulators.

### OPM Flow / Eclipse-style facility model

- OPM Flow describes itself as a fully implicit black-oil simulator with industry-standard well and facility controls, including BHP, THP, and surface/reservoir rate constraints.
- The public OPM well-model code shows that wells are not treated as unconstrained auxiliary variables that are allowed to drift arbitrarily during the global Newton loop.
- Instead, the well block is maintained as an explicit coupled subsystem with its own equations and update controls:
  - the well equations are assembled as a separate local block (`StandardWellEquations`)
  - the global system applies the well Schur complement (`r = r - C D^-1 Rw`) and later recovers the well update from the local solve (`D^-1`)
  - well primary variables are updated with explicit step limits, including a relative BHP limit (`dbhp_max_rel_`) and limits on fraction changes
  - for operability, THP, and potential calculations, OPM repeatedly solves or iterates the well equations at fixed or guessed BHP values to obtain a locally consistent well state before using it
- This is consistent with the idea that the well unknowns should stay close to a locally consistent manifold. The mature approach is not "let the global Newton iterate throw BHP/rates far away and hope the next residual pull fixes it". It is closer to "preserve a robust local well solve and bounded well-variable updates inside the coupled FIM framework".

### MRST fully implicit AD models

- MRST’s AD core describes its black-oil solvers as fully implicit, industry-standard, and validated against commercial simulators.
- In the standard fully implicit black-oil path, MRST inserts well equations directly into the coupled AD system rather than treating wells as a late explicit correction.
- The public MRST well equations show the same structure as the standard black-oil formulation:
  - explicit well unknowns such as `bhp`, `qWs`, `qOs`, and `qGs`
  - connection equations of the form `q - WI * lambda * drawdown`
  - control closure equations for BHP/rate constraints
- MRST also contains older and experimental paths that solve local well equations separately (`solveLocalWellEqs`) or discuss explicit-well variants. Those exist as special/experimental handling, not as the preferred robust fully implicit production formulation.

### What this means for the proposed fix

- The proposed next step is directionally aligned with industry practice, with one important refinement.
- "Keep `(bhp, q_perf)` close to the frozen-cell-consistent local manifold" is the right intent.
- The industry-style version of that idea is usually one of these:
  - bounded Newton updates on well variables, often tighter than cell-variable updates
  - a local well solve / well-block recovery embedded in each nonlinear iteration
  - Schur-complement elimination of the well block from the reservoir solve, followed by recovery of a locally consistent well update
  - control switching / operability logic that reinitializes the well state from a locally consistent BHP-constrained or target-constrained solution when needed
- The less industry-aligned version would be an ad hoc projection that silently overwrites well unknowns in a way that breaks Newton consistency.
- So the evidence argues for moving to solution mode, but to do it in a structured way:
  - either reduce/eliminate the explicit perforation-rate drift through well-block recovery,
  - or apply explicit well-variable damping / trust-region limits tied to the local well solve,
  - rather than bolting on a purely residual-side correction.

### Practical conclusion

- Public OPM and MRST evidence both support the same conclusion:
  - robust FIM implementations keep the well subsystem tightly controlled and locally consistent during nonlinear iterations.
- That supports the current diagnosis and justifies making the next solver change in the well/perforation update path rather than continuing broad residual instrumentation.

## First Solution Slice: Local Well Trust Region

Implemented a first solver-side change in `fim/state.rs`:

- after each raw Newton trial update, the explicit well variables are relaxed toward a locally consistent single-well state derived from the current cell state
- BHP is pulled toward the locally consistent BHP with a bounded trust radius
- perforation rates are pulled toward the corresponding connection-law rates and sign-clamped by injector/producer role
- this is intentionally a bounded well-update step, not a full algebraic elimination of the well block

### Why this is the right first slice

- It matches the industry comparison better than letting explicit well unknowns drift arbitrarily:
  - mature simulators use bounded well updates and/or local well-block recovery
- It is much smaller in scope than a full Schur-complement or explicit well-block elimination refactor
- It directly targets the diagnosed failure mode: large distance between the iterate and the local well/perforation manifold

### Immediate effect on diagnostics

- `wf_bt_12x12x1`:
  - the hard retry window is no longer perforation-row dominated
  - dominant stalled rows are now back in the cell equations, with perforation residuals reduced to near-zero levels during the retries that previously blew up in `perf0`
- `wf_bt_12x12x3`:
  - same structural improvement as the 2D case
  - injector/producer perforation rows no longer dominate the failing window, except for already tiny near-converged traces where the perforation residual is numerically negligible
- `gas_10x10x3`:
  - the catastrophic perforation-manifold failures were suppressed
  - the dominant residual remains the injector-side gas cell row, and in later small-step regimes some retries now stall on the well-control row rather than the perforation row

### Updated interpretation after the first fix

- The first fix appears to have done what it was supposed to do:
  - remove the large explicit-well drift as the dominant nonlinear bottleneck
- The remaining bottleneck is now cleaner:
  - water breakthrough difficulty is back in the reservoir cell equations
  - gas still has a separate remaining issue, but it now looks more like cell/well-control stagnation than catastrophic well/perforation inconsistency
- This is good progress because the dominant failure mode is now closer to the actual physical/coupled stiffness rather than a bad well iterate.

## Next Newton Slice: Residual-First Entry Acceptance And Strict Backtracking

Implemented a second Newton-side cleanup in `fim/newton.rs`:

- if a retry enters Newton with residual already at or near tolerance, accept that iterate immediately instead of forcing another linear solve just to satisfy the update norm
- damped trial states are now required to reduce the residual before they are allowed to advance the iterate
- if no residual-reducing damped state exists and the current residual is already inside the small entry guard band, the solver accepts the current iterate rather than walking into a stagnation loop

### Why this was needed

- the previous damping path still accepted the best finite candidate even when every candidate made the residual worse
- that was exactly what the remaining gas tiny-step traces were doing: a nearly converged state at the doubled timestep was being kicked to a much worse state, after which the solve stagnated on the well row
- the water breakthrough traces showed a milder version of the same issue: after one good reduction, later damped candidates sometimes failed to improve further and the solve spent extra iterations sitting on an almost converged cell-row plateau

### Observed effect

- `gas_10x10x3` no longer escalates from a near-converged doubled-step entry state into the old `well row=900` stagnation shelf; those retries now fail fast on the first Newton iteration and immediately cut back
- the same gas case now accepts the smaller retry step in one iteration via the residual-entry guard (`res ≈ 1.446e-5`)
- `wf_bt_12x12x3` still shows a small doubled-step residual plateau in the cell rows, but the solver now rejects non-improving candidates instead of marching through repeated stagnant Newton iterations; perforation dominance remains absent

### Updated interpretation

- the remaining bottleneck is now less about bad Newton state acceptance and more about timestep growth repeatedly proposing a slightly-too-large external substep
- the gas case still has a small doubled-step entry residual (`~5.8e-5`) above the current guard band, so timestep growth oscillates between two nearby substep sizes instead of collapsing into the older multi-iteration well-row failure
- the water breakthrough case shows the same structural pattern in the reservoir rows: doubled steps can land on a low residual plateau just above tolerance, then cut back cleanly

### Practical status after this slice

- explicit well-manifold drift is no longer the dominant convergence failure
- non-improving Newton candidates are no longer allowed to worsen an already good iterate
- the next likely leverage point is timestep-growth policy or a slightly more explicit near-converged acceptance rule for doubled-step retries, not another broad well/perforation fix

## Timestep Controller Gap Analysis

### Comparison with OPM Flow / mature FIM simulators

A systematic comparison of the current timestep controller (`step.rs`) against OPM Flow and MRST identified five gaps between this implementation and mature FIM simulators. Three of these are in the outer timestep loop and can be fixed without touching the Newton solver itself.

### Gap 1: blind 2× growth — no Newton feedback

The current controller always doubles after a successful substep (`last_successful_dt * 2.0`). It has no awareness of how hard the Newton solve was.

OPM Flow uses a convergence-history-based estimator: if the solve used many iterations or heavy damping, growth is cautious (1.1×–1.3×); if the solve was easy (1–2 iterations), growth is aggressive (up to 2×). The typical formula is `growth = clamp(target_iters / actual_iters, 1.0, max_growth)`.

This is the primary cause of the breakthrough substep explosion documented in the waterflood diagnostic notes. The pattern is:
- a doubled step lands just above tolerance, fails
- halving succeeds, then immediately doubles into another failure
- this oscillation between two nearby dt values burns hundreds of substeps

### Gap 2: no post-step saturation/pressure change limiting

After an accepted FIM substep, the controller does not inspect the saturation or pressure change that actually occurred. The IMPES solver in this same codebase already computes `sat_factor`, `pressure_factor`, and `rate_factor` after each pressure solve to throttle the next trial dt.

OPM/Eclipse uses `DSMAXDT` (typically 0.1–0.2 max saturation change per step) and `DPMAX` (typically 200–300 bar) as post-step dt suggestions. If the accepted step produced a large saturation swing, the next trial is scaled down proportionally instead of blindly doubled.

At water breakthrough, saturation in the breakthrough cell can change by 0.3–0.5 in a single accepted step, but the controller has no awareness of this.

### Gap 3: Newton cutback factor is computed but ignored

The Newton solver computes a `cutback_factor` reflecting how badly the solve failed: stagnation returns 0.25, damping failure returns 0.5, max-iterations returns `accepted_damping * 0.5`. But the outer loop always cuts by exactly 0.5, ignoring this information.

When Newton stagnates early (suggesting 0.25), the outer loop only cuts by 0.5, requiring an extra failed retry. When the solver nearly converged (suggesting ~0.9), the outer loop over-cuts to 0.5.

### Remaining gaps (deferred)

- **ILU(0) pressure preconditioner instead of AMG**: degrades with grid size, causes linear solver failures that cascade into timestep cuts. Medium effort, deferred.
- **Full Jacobian reassembly during line search**: each damped candidate does a full `assemble_fim_system` instead of a cheaper residual-only evaluation. Multiplies cost per Newton iteration by 2–4×. Medium effort, deferred.

## Phase 1 Fix: Adaptive Timestep Controller

### Design

Three changes to `step_internal_fim_impl`, all in the outer substep loop:

1. **Newton-iteration-aware growth**: replace the fixed `2.0` growth factor with `clamp(TARGET_NEWTON_ITERS / actual_iters, 1.0, MAX_GROWTH)`. Target iterations = 6 (the "easy" midpoint of a max-20 loop). A step that converged in 1 iteration grows by 2×; a step that needed 12 iterations grows by 1.0× (no growth).

2. **Post-step saturation/pressure change limiting**: after accepting a step, compute max |ΔSw| and max |ΔP| between old and new state. If max |ΔSw| exceeds a target (0.2), scale the next suggested dt down by `target / actual_change`. Same for pressure with a target of 200 bar. This caps the growth suggestion independently of the Newton iteration count.

3. **Newton cutback factor**: on failure, use `trial_dt * report.cutback_factor` instead of `trial_dt * 0.5`. This lets the Newton solver communicate how aggressively to cut.

## Follow-Up Audit: SPE1 Gas Regression And 3D/Breakthrough Slowness

### SPE1 gas audit status

- Added a focused regression in `src/lib/ressim/src/lib.rs`: `spe1_fim_gas_injection_creates_free_gas`.
- Current result: the Rust-core FIM path does create free gas under SPE1 gas injection. The regression passes and verifies all of the following after 10 one-day steps:
  - injector-cell `Sg` becomes positive
  - average `Sg` increases from its initial value
  - total gas inventory increases
- That means the currently checked-in Rust FIM core does **not** reproduce a blanket “gas saturation stays flat” failure.
- The remaining user-visible symptom is therefore more likely one of these:
  - gas remains very localized near the injector for some time, so field-wide `Sg` changes are visually subtle
  - a frontend/runtime presentation issue makes the saturation field look static even though the core state changes
  - an earlier local branch had a transient regression that is no longer present in the current workspace

### Strong new clue on the remaining slowness

The case-size pattern now points very strongly at the linear solver backend, not just timestep policy.

Representative coupled-system sizes (`3 * n_cells + n_wells + n_perforations`):

- `wf_bt_48x1x1`: `148` rows → native direct solve
- `wf_bt_12x12x1`: `436` rows → native direct solve
- `gas_10x10x3`: `904` rows → iterative backend on native
- `spe1_10x10x3`: `904` rows → iterative backend on native
- `wf_bt_12x12x3`: `1300` rows → iterative backend
- `sweep_areal_21x21x1`: `1327` rows → iterative backend

This matches the observed symptom surprisingly well:

- 1D and modest 2D cases are fast even at breakthrough because they stay below the native direct-solve threshold (`512` rows)
- the first “slightly more complicated” cases fall off that cliff and switch to the current iterative `FgmresCpr` path
- the difficult breakthrough / 3D cases are exactly the ones that spend most of their time in that iterative path

### Comparison against OPM Flow

Public OPM guidance describes the default Flow linear path as BiCGSTAB with CPRW, where the pressure coarse system is normally handled by AMG and the full system uses an ILU-type fine smoother. OPM also exposes explicit tuning of linear reduction targets and maximum iterations, and documents that the linear solver failing to reduce the residual forces timestep cuts.

Our current implementation is materially weaker than that:

- the pressure coarse system is built from block-based restriction/prolongation plus a tiny in-house ILU(0), not AMG
- the pressure correction does only a fixed small number of defect-correction sweeps (`2`)
- the fine-stage preconditioner is block-Jacobi over the `3×3` cell blocks plus scalar diagonal scaling for the tail
- the pressure extractor explicitly drops all scalar-tail couplings (`if col_idx >= layout.scalar_tail_start { continue; }`), so well/BHP/perforation unknowns are **not** present in the coarse pressure system at all

That last point is important. OPM’s public CPRW examples explicitly mention `add_wells = true`; our extractor currently omits those couplings from the pressure coarse solve. In well-driven breakthrough problems, that omission is a plausible reason the iterative backend degrades exactly when well/cell coupling becomes dominant.

### Updated interpretation

- The adaptive timestep controller improvements were worth doing; they removed obvious outer-loop waste.
- But the remaining case-size-dependent slowdown is now best explained as a **linear-solver / preconditioner quality problem** interacting with breakthrough nonlinearity, not as a pure timestep heuristic issue.
- More precisely:
  - small cases are fast because they avoid the iterative backend entirely
  - large/breakthrough/well-driven cases are slow because they hit an iterative backend whose CPR stage is still much weaker than OPM-style CPRW/AMG and currently excludes well-tail couplings from the coarse pressure system

### Immediate implication

- Do not treat the remaining slowdown as evidence that the earlier well-manifold or Newton-acceptance fixes were wrong.
- The next high-leverage solver work is likely in the linear backend, especially coarse pressure construction / well inclusion, rather than another broad nonlinear rewrite.

## Follow-Up Audit: Iterative CPR Path And SPE1 Gas Pinning

### Research-backed CPR improvement targets

Comparison against public OPM Flow guidance and standard CPR practice points to four specific weaknesses in the current iterative backend:

1. **Wells are excluded from the coarse pressure system**
  - Current code in `fim/linear/gmres_block_jacobi.rs` drops every coupling whose column is in the scalar tail:
    - `if col_idx >= layout.scalar_tail_start { continue; }`
  - In this formulation the scalar tail contains well BHP and perforation-rate unknowns, so the extracted pressure system ignores well/facility couplings entirely.
  - Public OPM CPRW guidance explicitly exposes `add_wells = true`; in well-driven black-oil problems that is the expected production configuration.

2. **The coarse pressure solver is ILU(0)-class instead of AMG-class**
  - The current extracted pressure system is solved by a small in-house ILU-style factorization plus a fixed low number of defect-correction sweeps.
  - That is qualitatively weaker than the AMG-backed coarse solve described in OPM CPRW documentation and is expected to degrade with grid size / anisotropy.

3. **Pressure restriction/prolongation are heuristic local-block inverses, not explicit IMPES-style weights**
  - The current implementation derives restriction/prolongation from the first row / first column of each local `3×3` inverse.
  - This is serviceable as a placeholder, but it is not the same as a documented `true-IMPES` or `quasi-IMPES` pressure extractor.

4. **The coarse pressure correction budget is fixed and tiny**
  - `solve_pressure_correction(..., 2)` hardcodes two correction sweeps with no convergence-based stop.
  - That makes the CPR stage cheap, but also very easy to under-solve on the hard cases that most need it.

### Real solver bug found and fixed

While auditing the SPE1 slowdown, a separate nonlinear acceptance bug was confirmed in `fim/newton.rs`.

#### Symptom

- SPE1-like debug traces showed repeated accepted substeps with:
  - tiny dt (`~0.019531 d` in the captured run)
  - `upd = 0.000e0`
  - effectively zero state change
- Time advanced anyway, creating the exact user-visible pattern of gas appearing pinned near the injector while the simulation became extremely slow.

#### Root cause

Two guarded acceptance paths could accept an iterate that was still equal to the previously accepted state:

1. iteration-0 residual-entry guard
2. the "reject non-improving damped candidates but accept current residual inside guard band" fallback

There was also an update-based convergence path that could accept a zero-update Newton solve even though the residual was only inside the loose guard band, not actually converged in a physically meaningful sense.

#### Fix

- Added a material-change check in `fim/newton.rs` so these guarded acceptance paths only accept iterates that have genuinely moved away from the previous accepted state.
- Added focused regression coverage:
  - `entry_guard_does_not_accept_unchanged_previous_state`

### Measured impact on SPE1 debug trace

Before this fix, the first SPE1 outer step could collapse into a long sequence of no-op accepted substeps around `dt ≈ 0.019531 d`.

After the fix, the same filtered native trace improved materially:

- outer step 1 now cuts from `10 d` down to a real accepted step of `0.3125 d`, then grows back through `0.23`, `0.47`, `0.53`, `0.95` day-class substeps
- outer step 2 accepts `0.625`, `1.25`, `0.47`, `0.23`, `0.70`, `1.41`, `2.11` day-class substeps instead of getting trapped on a no-op micro-step shelf
- the case is still slower than it should be, but it is no longer dominated by advancing time on unchanged states

Interpretation:

- the user-reported SPE1 "gas is locked in the injector cell and the simulation is extremely slow" symptom was partly real physics/coupling difficulty and partly this acceptance bug
- the bug fix removes one artificial source of gas-plume pinning by ensuring accepted timesteps actually correspond to state advancement
- the remaining slowness is still primarily in the iterative CPR path, not in a literal zero-transport gas bug

### Current best-practice improvement order

1. **Include well / perforation couplings in the coarse pressure system**
  - highest-value CPR correction for this codebase
  - directly aligned with OPM-style `add_wells = true`
  - March 28 implementation note: a naive attempt to append scalar-tail unknowns directly into the current coarse system (identity restriction/prolongation for tail variables, direct row injection into the extracted coarse matrix) was tested and then reverted. It regressed SPE1 early-time behavior: the focused `spe1_fim_gas_injection_creates_free_gas` regression failed (`max_sg = 0` at 10 days) and the filtered native SPE1 trace fell back into repeated tiny accepted substeps. Conclusion: this item still needs to be done, but not as a simple identity-tail augmentation. The likely correct direction is a more Schur-consistent well-aware coarse system rather than bolting the raw scalar tail onto the current pressure extractor.
  - March 28 follow-up implementation: the current `fim/linear/gmres_block_jacobi.rs` now keeps a pressure-only coarse system but adds scalar-tail influence through an approximate Schur correction. The preconditioner builds a tail-block inverse, projects tail-to-pressure couplings, augments the coarse pressure operator with `-A_ct D^-1 A_tc`-style terms, and augments the coarse RHS with `-A_ct D^-1 r_t`.
  - Focused validation on the active branch:
    - `pressure_projection_updates_entire_local_block` passes
    - `pressure_rhs_accounts_for_tail_schur_coupling` passes
    - `large_default_fim_system_still_uses_iterative_backend` passes
    - `entry_guard_does_not_accept_unchanged_previous_state` passes
    - `spe1_fim_first_steps_converge_without_stall` passes
    - `spe1_fim_gas_injection_creates_free_gas` passes
  - Measured native SPE1 trace after the Schur-style change: correctness is better than the pre-fix no-op-acceptance state, but timestep fragmentation is still substantial. The first outer step still cuts from `10 d` to an accepted `0.3125 d`, and the second outer step still collapses as low as `0.09375 d` before recovering.
  - Current conclusion: step 1 is now implemented in a materially better form than the reverted naive tail augmentation, and it is worth keeping because it preserves focused SPE1 behavior and CPR unit coverage. It is not, by itself, enough to remove the practical SPE1 slowdown. The next leverage is still a stronger coarse pressure solve and better CPR diagnostics, not more scalar-tail bolting.

2. **Replace the current ILU(0)-class coarse solve with AMG or at least a materially stronger multilevel pressure solver**
  - this is the main expected lever for the 3D / breakthrough performance cliff
  - March 28 implementation: `fim/linear/gmres_block_jacobi.rs` now upgrades the extracted pressure stage from a fixed two-sweep ILU defect correction to a stronger hybrid solve. For moderate coarse systems (currently up to `512` pressure rows), the extracted pressure matrix is inverted once during preconditioner build and the CPR pressure stage uses that exact dense coarse solve. Larger coarse systems fall back to a residual-based ILU defect-correction loop with a higher iteration budget instead of the previous hardcoded two sweeps.
  - Focused validation on the active branch:
    - `pressure_projection_updates_entire_local_block` passes
    - `pressure_rhs_accounts_for_tail_schur_coupling` passes
    - `pressure_correction_uses_exact_dense_inverse_when_small` passes
    - `large_default_fim_system_still_uses_iterative_backend` passes
    - `spe1_fim_first_steps_converge_without_stall` passes
    - `spe1_fim_gas_injection_creates_free_gas` passes
  - Measured impact on a representative 3D breakthrough trace (`wf_bt_12x12x3`): the stronger coarse pressure solve does not materially remove timestep fragmentation by itself. The first outer step still cuts from `1 d` down to `0.015625 d`, later shelves around repeated `0.002768 d` accepted substeps, and then drops again to `0.001384 d`.
  - Current conclusion: this step is still worth keeping because it removes one obvious weakness in the CPR stage and, for the current case sizes, effectively rules out “the coarse pressure solve is just too under-solved” as the sole explanation. But the remaining slowdown is now pointing more strongly at coarse-system quality and outer-step policy than at raw coarse-solver strength alone.

3. **Replace the current inverse-first-row pressure extractor with an explicit IMPES-style or quasi-IMPES pressure weighting**
  - makes the coarse system more physically meaningful and more comparable to industry CPR practice
  - March 28 implementation: `fim/linear/gmres_block_jacobi.rs` now builds the per-cell pressure transfer weights from an explicit local Schur reduction of the cell block instead of reusing the first row and first column of the full `3×3` block inverse. The new restriction is `[1, -A_pt A_tt^-1]` and the prolongation is `[1; -A_tt^-1 A_tp]`, which is a more explicit quasi-IMPES-style pressure extractor for this natural-variable block layout.
  - Focused validation on the active branch:
    - `pressure_transfer_weights_follow_local_schur_elimination` passes
    - `pressure_projection_updates_entire_local_block` passes
    - `pressure_rhs_accounts_for_tail_schur_coupling` passes
    - `spe1_fim_first_steps_converge_without_stall` passes
    - `spe1_fim_gas_injection_creates_free_gas` passes
  - Measured impact on the representative 3D breakthrough trace: the qualitative retry pattern is still bad. The early outer-step cutback ladder is still `1 d -> 0.5 -> 0.25 -> 0.125 -> 0.0625 -> 0.015625`, and the later shelf around repeated `0.002768 d` retries still appears.
  - Current conclusion: this is a cleaner and more defensible coarse extractor than the old inverse-entry heuristic, and it belongs in the codebase. But it does not, by itself, remove the breakthrough retry shelf.

4. **Add CPR diagnostics before deeper tuning**
  - report linear iterations, final linear residual, whether the coarse solve is used, and possibly a pressure-coarse residual reduction metric
  - this will make future native and wasm traces far more informative without resorting to huge raw debug logs
  - March 28 implementation: `FimLinearSolveReport` now carries CPR diagnostics, and the verbose Newton trace prints them as `cpr=[rows=... solver=... apps=... avg_rr=... last_rr=...]` on iterative CPR iterations.
  - Focused validation on the active branch:
    - `cpr_report_exposes_coarse_diagnostics` passes
    - existing CPR unit and SPE1 regressions remain green
  - Measured result on `wf_bt_12x12x3`: the diagnostics are highly informative. On the hard early retries the extracted coarse system has `rows=432`, uses the exact dense coarse solve, and reports average coarse-stage reduction ratios on the order of `1e-14` to `1e-13`. In other words, the current coarse stage is already solving the extracted pressure system essentially to machine precision on this case.
  - Current conclusion: the remaining fragmentation on this representative case is not well explained by an under-solved coarse pressure stage anymore. The dominant failures now look like nonlinear damping / outer-step shelf behavior in the reservoir rows after a very effective coarse correction, so the next best leverage is probably outside raw coarse-solve strength.

  5. **Classify failed retries as linear-bad vs nonlinear-bad before pushing CPR further**
    - March 28 implementation: `fim/newton.rs` now classifies failed retries as `linear-bad`, `nonlinear-bad`, or `mixed` using the final dominant residual family together with the actual backend used by the last linear solve, and `step.rs` now prints a per-outer-step retry summary.
    - Focused validation on the active branch:
      - `failure_classification_marks_clean_cpr_failure_as_nonlinear_bad` passes
      - `failure_classification_marks_fallback_path_as_linear_bad` passes
      - `cpr_report_exposes_coarse_diagnostics` still passes
    - Measured result on the corrected `wf_bt_12x12x3` rerun: the first outer step reports `linear-bad=11`, `nonlinear-bad=83`, `mixed=0`.
    - Structure of that split:
      - the first few cutbacks from `1 d` down to `0.015625 d` include a short linear-bad phase
      - after that, the long accepted-substep shelf around `0.005536 d`, `0.002768 d`, and finally `0.001384 d` is overwhelmingly `nonlinear-bad`
      - the dominant family in that shelf is usually a reservoir row (`oil` first, then later many `water` labels), not a direct-solver fallback event
    - Current conclusion: on the representative hard 3D breakthrough case, the remaining retry explosion is now mostly a nonlinear / timestep-acceptance problem rather than a coarse-pressure under-solve problem. Additional CPR strengthening may still help the short early linear-bad window, but it is unlikely to remove the practical low-`dt` shelf by itself. The next highest-value work should target outer-step policy, near-stagnant retry acceptance/cutback rules, or a cheap residual-based nonlinear acceptance slice.

## Comprehensive FIM Review vs OPM / Open-Source Simulators (March 29)

Full code inspection of the FIM implementation, comparing against OPM Flow, MRST, and standard black-oil FIM practice. Focus on convergence pathology and the SPE1 gas stall bug.

### Architecture Comparison

| Aspect | This Implementation | OPM Flow |
|--------|-------------------|----------|
| Variable set | P, Sw, Sg/Rs per cell | P, Sw, Sg/Rs/Rv per cell |
| Well coupling | Full global system + post-update relaxation | Schur complement elimination |
| Phase transition | Frozen during Newton, hysteresis band post-Newton | Variable substitution, immediate at bubble point |
| Material balance | Not explicitly checked at convergence | MB + CNV dual criterion |
| Damping | Appleyard + residual line search | Appleyard chops only (tighter limits) |
| Linear solver | FGMRES-CPR with ILU/dense pressure coarse | BiCGSTAB-CPR with AMG pressure coarse |
| Timestep control | Newton-feedback adaptive growth | Similar + DSMAXDT/DPMAX limits |

The overall formulation (molar conservation for water/oil/gas components, upstream-weighted inter-cell flux, well perforation equations, Fischer-Burmeister complementarity for rate control) is structurally sound and comparable to industry practice. The differences are in secondary mechanisms, not in the core discretization.

### Verified Correct

The following areas were inspected in detail and found to be consistent:

- **Residual vs Jacobian gravity formulation**: `interface_density_barrier(rho_i, rho_j)` = `0.5 * (rho_i + rho_j)` and `gravity_head_bar(d_i, d_j, density)` = `density * g * dz * 1e-5`, which is algebraically identical to `gravity_half_coefficient * (rho_i + rho_j)` used in the Jacobian. No inconsistency.
- **Water gravity Jacobian omission is correct**: `get_rho_w(p)` returns a constant (ignores pressure), so water density has zero derivatives w.r.t. all state variables. Passing zero gravity derivatives for water is intentional, not a bug.
- **Accumulation Jacobian**: Pore volume pressure derivative `d_pv/dP = pv * cr` correctly differentiates `pv_ref * exp(cr * (P - P_prev))`. Previous-state inventory is constant w.r.t. Newton unknowns.
- **Dissolved gas flux**: `q_g_dissolved = q_o_sc * Rs_upstream` with Jacobian correctly including both `Rs * d(q_o)/d(var)` and `q_o * d(Rs)/d(var)` terms, with the latter applied only on the upstream side.
- **Oil FVF derivatives**: `d_bo_sat_d_p` for saturated (total derivative along bubble curve) vs `d_bo_d_p` at fixed Rs for undersaturated, plus `d_bo_d_rs` for undersaturated. Correct chain rule treatment.
- **Mobility Jacobian**: `local_flux_cell_sensitivity` and `phase_mobilities_for_state` use identical SCAL table functions and viscosity functions. Quotient-rule derivatives for `kr/mu` are correct.
- **Gas equation undersaturated regime**: `d_gas/d_h = d_oil/d_h * Rs + oil_inventory * 1` correctly captures that h = Rs in undersaturated regime, with Bo(Rs) dependency handled through the d_oil_d_h term.

### BUG FOUND: Material Balance Violation in Rs Clamping — Likely Gas Stall Root Cause

**Location**: `fim/state.rs`, `classify_regimes()`, lines 176–185.

```rust
HydrocarbonState::Undersaturated => {
    let rs_sm3_sm3 = cell.hydrocarbon_var.max(0.0);
    let rs_threshold = rs_sat * (1.0 + RS_OVERSHOOT_FRAC); // 1.01 * Rs_sat
    if rs_sm3_sm3 < rs_threshold {
        self.cells[idx].hydrocarbon_var = rs_sm3_sm3.min(rs_sat); // ← CLAMP
        continue;
    }
    // flash only happens if Rs > 1.01 * Rs_sat
}
```

#### Mechanism

When Newton converges with Rs between Rs_sat and 1.01 × Rs_sat, the `min(rs_sat)` clamp silently discards the excess dissolved gas. Over many small timesteps, this creates a one-way valve:

1. Gas flows into an undersaturated cell via inter-cell flux
2. Newton (with frozen regime = Undersaturated) increases Rs slightly above Rs_sat to accommodate the incoming gas
3. Post-Newton `classify_regimes` clamps Rs back to Rs_sat
4. The excess dissolved gas inventory is lost — no free gas is created, no mass is conserved
5. Next timestep starts from the damaged state — repeat

#### Why this matches the observed SPE1 gas stall

Near the gas front, cells receive small increments of gas per timestep. With adaptive timestepping producing small dt values near the front, each step may only push Rs slightly above Rs_sat — enough to get clamped but not enough to exceed the 1.01× threshold. The gas front stalls because downstream cells can never nucleate free gas. Gas appears to grow near the injector (where the flux is large enough to push Rs well past the 1% band) but then stops advancing.

#### How OPM handles this

OPM Flow uses **immediate variable substitution** at Rs = Rs_sat — no hysteresis band. The moment Rs reaches Rs_sat, the primary variable switches from Rs to Sg. Any excess dissolved gas is converted to free gas through the flash calculation. Material balance is preserved exactly.

MRST's fully implicit AD path similarly does not use a hysteresis band for undersaturated → saturated transitions. The standard formulation switches variables as soon as Rs ≥ Rs_sat(P).

#### Interaction with frozen-regime Newton

The frozen-regime strategy (keeping Saturated/Undersaturated fixed during Newton) is defensible and used in industry. But combined with the clamping bug, it creates a pathological interaction: Newton solves correctly for Rs > Rs_sat, then the post-convergence classification destroys the result by clamping. If the regime could switch within Newton (as in OPM's variable substitution), the solver would directly compute the correct Sg value and the post-classification would be a no-op.

#### Suggested fix directions

1. **Remove the hysteresis band for Undersaturated → Saturated** (match OPM): switch immediately when Rs ≥ Rs_sat. This is the simplest fix and the industry-standard approach.
2. **Flash instead of clamp**: when Rs is in the band (Rs_sat < Rs < 1.01 × Rs_sat), perform the flash calculation and convert excess to Sg anyway, instead of discarding. Keep the hysteresis only for Saturated → Undersaturated (where Sg dropping below 1e-4 is already handled correctly).
3. **Carry the Rs excess forward**: keep the cell undersaturated but do not clamp Rs to Rs_sat — let it sit slightly above Rs_sat. This preserves mass but may cause the frozen-regime Jacobian to see a slightly unphysical state.

Option 1 is recommended. The Saturated → Undersaturated hysteresis (Sg < 1e-4) is benign because it only delays removing negligible free gas. The Undersaturated → Saturated hysteresis is harmful because it prevents gas appearance and violates material balance.

#### Suggested diagnostic to confirm

Add a per-timestep check that computes total gas inventory before and after `classify_regimes`:

```rust
// Before classify_regimes:
let gas_inventory_before = sum over cells of (pv * Sg / Bg + pv * So * Rs / Bo);
// After classify_regimes:
let gas_inventory_after = sum over cells of (pv * Sg / Bg + pv * So * Rs / Bo);
if (gas_inventory_before - gas_inventory_after).abs() > 1e-6 {
    eprintln!("classify_regimes lost {:.6e} Sm3 gas", gas_inventory_before - gas_inventory_after);
}
```

If this fires on the SPE1 gas case, the diagnosis is confirmed.

### Issue: No Global Material Balance Convergence Check

**Location**: `fim/newton.rs`, convergence acceptance paths.

The Newton solver checks convergence via scaled infinity norms of the residual and update. This is equivalent to OPM's CNV (Component Normalized Volume) check — the maximum per-cell residual. However, OPM also requires a **MB (Mass Balance)** check: the sum of all residuals per component over the entire domain, normalized by total fluid volume.

The MB check catches cases where per-cell residuals are individually small but don't sum to zero — meaning the global conservation equations are not actually satisfied. This is particularly important when regime transitions or clamping operations modify the state after the Newton solve.

Adding a global mass balance check would provide a safety net against the Rs clamping bug and similar state-modification errors. If the sum of gas-component residuals is nonzero after acceptance, the step should be rejected regardless of per-cell convergence.

### Issue: Full Assembly During Line Search

**Location**: `fim/newton.rs`, lines 694–703 (damping loop).

Each damped trial candidate calls `assemble_fim_system`, which builds both the residual and the full Jacobian. Only the residual norm is needed for the line search acceptance decision. The Jacobian is discarded. This multiplies the cost of each Newton iteration by `(1 + number_of_damping_trials)`.

OPM Flow does not use a residual-based line search — it relies on Appleyard chops alone with tighter per-variable limits. The chops are cheaper because they don't require residual evaluation.

**Fix**: The `assemble_residual` function already exists as a separate path (used by the FD test helpers). Adding an `assemble_residual_only` option to `FimAssemblyOptions` that skips Jacobian construction would halve the cost of the line search without changing convergence behavior.

### Issue: Convergence Criteria Compared to OPM

OPM Flow uses two separate convergence criteria that must both be satisfied:

- **CNV** (per-cell): max over all cells of `|residual_i| / (pv_i / (dt * B_alpha))`. Similar to the current scaled infinity norm.
- **MB** (global): `|sum(residual_i)| / (total_fluid_volume / (dt * B_alpha))` per component. Catches global conservation failures.

The current implementation only has the CNV-equivalent check. Adding the MB check would be a small incremental change with disproportionate value for catching material balance problems.

### Remaining Convergence Bottleneck: Nonlinear Stagnation After Breakthrough

The worklog's earlier sections thoroughly document the retry failure classification as overwhelmingly `nonlinear-bad` on representative 3D cases. The dominant family is reservoir-row (oil, then water), not well/perforation after the trust-radius fix.

From the OPM comparison, two remaining gaps are likely contributors to this pattern:

1. **Tighter Appleyard chops without line search**: OPM typically limits ΔSw to ~0.2 and ΔP to ~200 bar per Newton iteration (compared to 0.5 and 500 bar here). The tighter limits reduce the need for line search entirely. The current generous limits combined with a residual-based line search can still accept states that are locally improving but globally far from convergence.

2. **Schur-complement well elimination**: By algebraically eliminating well unknowns from the reservoir system, the Newton update for cell variables is never contaminated by well-variable drift. The current relaxation approach is an approximation of this but does not fully prevent well-cell cross-contamination in the Newton direction.

### Recommended Next Steps (Priority Order)

1. **Fix Rs clamping material balance violation**: Remove the 1% hysteresis band for Undersaturated → Saturated, or flash instead of clamp. This is the most likely root cause of the gas stall.
2. **Add residual-only assembly for line search**: Skip Jacobian in damping loop candidates. Straightforward cost reduction.
3. **Add global mass balance convergence check**: Sum-of-residuals per component. Safety net against state-modification errors.
4. **Consider Schur complement for well block**: Eliminates well-variable drift as a convergence concern. Larger effort but aligned with industry practice.

## Session 2026-03-31: Inflection-Point Chop, OPM Growth Rates, and Architecture Review

### Changes implemented

Three changes were made in this session:

1. **OPM-aligned timestep growth policy** (`step.rs`):
   - `MAX_GROWTH` reduced from 2.0 → 1.25 (matches OPM `growth_rate`)
   - `MIN_GROWTH` = 0.75 added (matches OPM `decay_rate`): allows dt shrinkage after hard successful steps
   - `TARGET_NEWTON_ITERS` raised from 6.0 → 8.0 (matches OPM target)
   - Removed the floor at 1.0 in the growth clamp, so hard steps now actively shrink dt

2. **Gas saturation change tracking** (`step.rs`):
   - Growth limiter previously only tracked |ΔSw|; now also tracks |ΔSg| in saturated cells
   - Relevant for three-phase cases where the gas front was unconstrained

3. **Trust-region inflection-point chop** (`newton.rs`):
   - Added `fw_at_sw()` and `fw_inflection_point_sw()` helpers to compute the water fractional-flow inflection point via 16-point sampling of the fw(Sw) curve
   - Modified `appleyard_damping()` to prevent Sw from crossing the fw inflection point in a single Newton iteration (Wang & Tchelepi 2013 trust-region approach)
   - Also fixed the test call site (`appleyard_damping_limits_combined_oil_saturation_change`) to pass `&sim`

### Measured results on canonical wf_p_12x12x3 (1 day, wasm)

- Baseline entering session: **138 substeps, linear-bad=6, nonlinear-bad=128**
- After all three changes: **130 substeps, linear-bad=4, nonlinear-bad=30**
- Retry reduction: 77% improvement in nonlinear-bad

### Remaining retry pattern (fully characterized)

The remaining 30 nonlinear-bad retries follow a rigid 3-substep cycle in the post-breakthrough region (substeps ~87-129):

```
substep N:    ACCEPTED dt=D      iters=2  (guard-equiv)
substep N+1:  ACCEPTED dt=1.25D  iters=2  (guard-equiv)
substep N+2:  FAILED   dt=1.56D  iters=1  DAMPING FAILED → retry at 0.78D
substep N+2:  ACCEPTED dt=0.78D  iters=2
```

The failing step has `iters=1, res=~8e-6` (just 60% above tolerance `1e-5`) with `DAMPING FAILED: no sufficiently decreasing bounded candidate`. The hotspot is always `cell(11,11,0)` — the top-right corner boundary cell at `Sw≈0.10`, at the water breakthrough front. The inflection-point chop does not help here because the Newton direction is fundamentally non-descending at that dt, not crossing a basin boundary.

Interpretation: The front-local nonlinearity at the corner cell creates a minimum viable dt below which the step is easy (2 iters) and above which Newton can't descend. The 1.25× growth periodically crosses that threshold, and the 0.50 retry factor drops back below it. The system is stuck in a stable oscillation that no amount of Newton globalization can fully remove — only dt policy can.

### Correctness alert: 3 failing tests

Three pre-existing tests in the current HEAD are failing:
- `fim::state::tests::classify_regimes_switches_immediately_when_rs_exceeds_rs_sat` — FAILED
- `fim::state::tests::classify_regimes_preserves_gas_inventory_when_undersaturated_state_exceeds_rs_sat` — FAILED
- `fim::wells::tests::gas_injector_surface_pressure_derivatives_match_local_fd` — FAILED

The first two tests directly verify the Rs-switch correctness fix documented earlier in this worklog. Their failure means the Rs clamping fix is broken. This is the most likely explanation for FIM vs IMPES parity gaps on depletion cases (`dep_pss`, `dep_decline`) documented in the correctness review (lines 19-33 of this file).

**These failures must be investigated and fixed before further convergence tuning.**

### Architecture review: is an OPM rewrite viable?

The March 29 comprehensive review confirmed the Jacobian, residual, and physical model are correct. The gaps are in secondary mechanisms. Selective adoption of OPM patterns (in priority order):

| Approach | Effort | Expected value |
|---|---|---|
| Fix failing classify_regimes tests (Rs correctness) | Small | Accuracy |
| Add FIM vs IMPES parity test for dep_pss | Small | Correctness baseline |
| Appleyard-only globalization (no line search, tighter limits ΔSw≤0.1) | Medium | ~60% per-substep speedup |
| Hotspot-aware cooldown: freeze growth at last_failure_dt | Small | Fix 30 remaining nonlinear-bad |
| Variable substitution for Undersaturated→Saturated transition | Medium | Accuracy + gas convergence |
| Well Schur complement elimination | Large | Fundamental well coupling fix |

A full OPM rewrite is not recommended — the core formulation is verified correct and a rewrite would risk breaking the things that work. Selective component replacement is the right path.

### Priority order for next work

1. **Correctness first**: Investigate and fix the 3 failing tests, especially the classify_regimes failures
2. **Correctness baseline**: Add a dep_pss FIM vs IMPES parity smoke test
3. **Convergence**: Hotspot-aware cooldown policy (smaller scoped change)
4. **Convergence**: Replace line search with tighter Appleyard-only (OPM-style Newton globalization)
5. **Correctness + convergence**: Variable substitution for phase transitions