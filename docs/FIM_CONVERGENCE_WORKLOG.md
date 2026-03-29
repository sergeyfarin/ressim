# FIM Convergence Worklog

This file is the active investigation log for live FIM convergence work.

Use `docs/FIM_STATUS.md` for the current consolidated solver status.
Use this worklog only for active observations, reproductions, traces, and next hypotheses while an issue is still live.

This file is the working document for the March 2026 FIM convergence investigation.
Keep active observations, reproductions, diagnostics, and next hypotheses here until the issue is resolved.

## Scope

- Problem class: native FIM convergence and timestep fragmentation that appears mainly on 2D and 3D cases.
- Main repro case: `gas_10x10x3`.
- Related symptom: similar fragmentation also appears on water cases, especially 3D pressure-controlled waterfloods.
- Non-goal: treating this as a pure tolerance-tuning problem before the nonlinear source is localized.

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