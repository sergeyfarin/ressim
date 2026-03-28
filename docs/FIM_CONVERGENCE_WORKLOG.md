# FIM Convergence Worklog

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