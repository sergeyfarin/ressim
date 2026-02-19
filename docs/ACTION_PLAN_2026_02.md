# ResSim Comprehensive Review & Action Plan

_Generated: 2026-02-19_

## Executive Summary

Full codebase review covering Rust physics engine, WebWorker communication, Svelte UI, 3D visualization, and analytical benchmarks. Issues are prioritized from **P0** (critical bugs) down to **P4** (nice-to-have enhancements).

---

## P0 — Critical Bugs

### 1. Stop Button Mostly Does Not Work

**Root cause**: The stop button sends a `{ type: 'stop' }` message to the worker, which sets `stopRequested = true`. However, the worker only checks `stopRequested` **between simulation steps** and **at yield intervals**. For large grids (e.g. 49×49×1 = 2,401 cells), a single `simulator.step()` call can take hundreds of milliseconds to seconds because it internally sub-steps via `step_internal()` with up to `MAX_ATTEMPTS = 10` IMPES cycles. During that entire Rust/WASM call, the JS event loop is blocked and the `onmessage` handler that sets `stopRequested` cannot execute.

**Fix**:
- Reduce `chunkYieldInterval` from 5 to 1 for the first batch, or dynamically based on measured step time, so that `await setTimeout(0)` yields between every step allowing stop messages to be processed.
- Alternatively, move the stop-check into a micro-yield pattern: after each `simulator.step()`, immediately yield (`await setTimeout(0)`) so `self.onmessage` can run before the next step. The current architecture already yields, but only every `chunkYieldInterval` steps (default 5). When each step takes 200ms+, the worker becomes unresponsive for 1+ seconds.
- Consider adding a `SharedArrayBuffer` / `Atomics` flag that the main thread can set and the Rust loop can poll via a WASM-exported callback. This is a longer-term solution but guarantees sub-step cancellation.

**Files**: `src/lib/sim.worker.ts` (lines 310-360), `src/App.svelte` (`runSimulationBatch`)

### 2. Pre-loaded Cases Cannot Be Continued (Run After Pre-run Load)

**Root cause**: When a pre-loaded case finishes loading, it sets `runCompleted = true` and `preRunContinuationAvailable = true`, but the simulator is **not initialized** — no `create` message has been sent to the worker, so `simulator` is `null` inside the worker. When the user clicks "Run", `ensurePreRunContinuationReady()` is called, which attempts to hydrate. The hydration flow sends `hydratePreRun` to the worker, which calls `configureSimulator(createPayload)` + runs N steps to replay the saved state. However, several issues exist:

1. **Hydration replays the pre-run steps from scratch** — for large cases (e.g. 240 steps on 96 cells), this takes noticeable time during which the UI shows "Preparing continuation…" but the user may get confused or click Run again.
2. **After hydration completes**, the `hydrated` message sets `preRunHydrated = true` and resolves the promise. Then `runSimulationBatch` proceeds to send a `run` message. But the history/rateHistory from the pre-run JSON are in a different format than what the live worker produces — the pre-run `rateHistory` entries lack some fields (`total_production_liquid_reservoir`, `total_injection_reservoir`, `material_balance_error_m3`). When new live data appends, the chart may have inconsistent x-axis data.
3. **If hydration fails or is cancelled**, the state variables get tangled: `workerRunning` stays true, the promise never resolves, and the UI freezes in a half-state.

**Fix**:
- Add a dedicated "Continue" button or a clear status when continuation requires hydration.
- Validate hydration completion before enabling Run.
- Add timeout protection for hydration promises.
- Ensure rate history schemas are consistent between pre-run JSON and live data.
- Consider serializing full simulator state as a WASM `serde` blob in the pre-run JSON to allow instant restoration without replaying steps.

**Files**: `src/App.svelte` (lines 502-570), `src/lib/sim.worker.ts` (lines 190-250)

---

## P1 — Logic & Physics Issues

### 3. Transmissibility Uses Average Total Mobility (Non-standard)

In `step.rs` line ~184, the transmissibility function uses arithmetic average of total mobility:
```rust
let mob_avg = (self.total_mobility(c1) + self.total_mobility(c2)) / 2.0;
```

Standard reservoir simulation practice uses **upstream weighting** for mobility in the transmissibility calculation. The current approach averages the mobilities across neighbor cells, which can lead to non-physical flow at sharp saturation fronts and explains why the Buckley-Leverett benchmark tolerance is 25-30% (quite loose).

**Fix**: Apply upstream weighting for phase mobilities in the transmissibility calculation, or separate the transmissibility into geometry (T_geometry = 8.527e-5 * k_h * A / L) and mobility terms. Then use upstream mobility for each phase separately in the flux calculation. This is partially already done in `calculate_fluxes()` where upwind fractional flow is used for saturation transport, but the pressure equation still uses averaged total mobility.

**Impact**: Improved BL benchmark accuracy, tighter tolerance targets.

### 4. Pressure Equation Uses Lagged Mobility (IMPES Splitting Error)

The IMPES (IMplicit Pressure, Explicit Saturation) approach used is correct in principle, but the transmissibilities in the pressure equation (line ~420 in `step.rs`) use saturations from the **previous** timestep, which is standard IMPES. However, the stability factor `stable_dt_factor` scales both pressure and saturation changes linearly, which is only first-order accurate. When `stable_dt_factor < 1.0`, the code scales `delta_water_m3` linearly by `dt_ratio`, but the pressure solution `p_new` was computed for the full `remaining_dt` — only the saturation volumes are scaled. This means pressure is over-predicted when sub-stepping occurs.

**Fix**: When sub-stepping is needed (`stable_dt_factor < 1.0`), re-solve the pressure equation with the reduced timestep rather than linearly scaling volumes. This would mean calling `calculate_fluxes(actual_dt)` in a retry loop. The current approach trades accuracy for performance.

### 5. Material Balance Error Calculation Has a Conceptual Issue

In `step.rs` lines ~604-615, the material balance error is computed as:
```rust
let net_added_m3 = (total_injection - total_prod_liquid) * dt_days;
let actual_change_m3: f64 = delta_water_m3.iter().sum();
let mb_error = (net_added_m3 - actual_change_m3).abs();
```

This compares **water** volume changes (`delta_water_m3`) against **total liquid** (oil + water) net flow. The well rates use surface volumes (divided by `b_o`/`b_w`), while `delta_water_m3` is reservoir-condition water volume. These are not apples-to-apples.

**Fix**: Track material balance in reservoir conditions consistently: compare total reservoir injection minus total reservoir production against total in-place change (both water and oil pore volumes). Or use surface volumes throughout. The current mismatch means the material balance error metric isn't reliable.

### 6. Compressibility Term Missing Porosity Factor  

In `step.rs` line ~380, total compressibility is:
```rust
let c_t = cell.porosity * (self.pvt.c_o * cell.sat_oil + self.pvt.c_w * cell.sat_water)
    + self.rock_compressibility;
```

The accumulation term becomes `Vp * c_t / dt` where `Vp = dx*dy*dz*porosity`. This effectively double-counts porosity for fluid compressibility (once in `Vp` and once in `c_t`). The standard formulation is:

`c_t = c_o * S_o + c_w * S_w + c_f` (no porosity multiplier on fluid terms)
`Accumulation = Vp * c_t / dt`

Where `c_f = c_rock / porosity` if rock compressibility is defined as bulk volume compressibility, or `c_f = c_rock` if it's pore volume compressibility.

**Fix**: Remove the `cell.porosity` multiplier from the `c_t` calculation. Verify whether `rock_compressibility` represents pore volume compressibility (common in reservoir simulation) or bulk compressibility.

**Impact**: Affects pressure response magnitude. With the current code, fluid compressibility effects are approximately `0.2×` what they should be (since typical porosity is 0.2).

### 7. Capillary Pressure at Connate Water Returns 1000 bar

In `capillary.rs`, when `s_eff <= 0.0`, the function returns `1000.0` bar. This is an arbitrary cap that could cause numerical instability if cells near connate water develop enormous pressure discontinuities. A more physically reasonable approach would be to use the analytical limit of the Brooks-Corey curve with a realistic maximum (e.g., 10-50 bar for typical reservoir rock).

**Fix**: Set the cap based on the entry pressure and lambda: e.g., `max_pc = p_entry * 20.0` or similar geologically motivated limit. Return the cap at 500 bar maximum is already in place for the calculated value, but the `s_eff <= 0.0` branch returns a potentially different value.

---

## P2 — UI / UX Issues

### 8. Inputs Are Read-Only for Pre-loaded Cases

When a pre-loaded case is selected, `InputsTab` receives `readOnly={!isCustomMode && activeCase !== ''}`. This prevents users from tweaking parameters of an existing case to explore "what-if" scenarios. The only way to modify inputs is to switch to Custom mode, losing the pre-run data context.

**Fix**: Allow editing inputs for pre-loaded cases, and automatically transition to a "Custom Sub-Case" (which the code already partially supports via `CUSTOM_SUBCASE_BY_CATEGORY`) when any parameter changes.

### 9. No Progress Indicator During Simulation

While `workerRunning` is used to disable buttons, there's no progress bar or step counter showing how many steps have completed out of the requested total. Users have no idea if the simulation is 10% or 90% done.

**Fix**: Track `completedSteps` vs `totalSteps` in the worker messages. The worker already sends periodic `state` messages with `stepIndex`. Display a progress bar in `RunControls`.

### 10. History Playback Uses Fixed Speed Without Duration Awareness

The playback timer interval is `1000 / playSpeed` ms. For runs with 240 steps, playback at speed 2 takes 120 seconds. There's no way to jump to a specific time or see the total duration.

**Fix**: Add a time slider in addition to the step slider. Show current time/total time. Allow speed adjustment up to 10x or more.

### 11. No Export/Download of Results

Users cannot save simulation results (rate history, grid states) for external analysis. This significantly limits the tool's usefulness for engineers.

**Fix**: Add CSV/JSON download buttons for rate history, grid state snapshots, and analytical comparison data.  

### 12. Welge Fractional Flow Diagram Is Commented Out

The entire Welge f(Sw) diagram in `FractionalFlow.svelte` template section is commented out (lines 310-339). This is a valuable diagnostic tool for waterflood cases.

**Fix**: Un-comment and display the Welge diagram, perhaps as a collapsible section or tab.

### 13. Missing Input Validation Feedback Visibility

Validation errors exist (`validationErrors`) but are passed to `InputsTab` without clear visual indication on which specific input field has errors. Users have to scroll through all inputs to find the problem.

**Fix**: Highlight errored inputs with red borders and show error messages inline below each field.

---

## P3 — Performance & Code Quality

### 14. History Stores Full Grid State Per Snapshot (Memory Intensive)

Each history entry stores the complete `grid[]` array. For a 20×20×10 grid (4,000 cells), with up to 300 history entries, that's ~1.2M cell objects in memory. Larger grids will cause OOM.

**Fix**: Options include:
- Store only changed cells (delta compression)
- Reduce snapshot frequency for large grids
- Store only aggregate metrics in history; re-derive from full state on demand
- Use `Float32Array` for grid data instead of JS objects

### 15. 3D View Rebuilds Instanced Grid on Every Grid State Update

The `$:` reactive block triggers `buildInstancedGrid()` which recreates all instanced mesh transforms. For large grids, this is expensive. Consider updating only colors via `setColorAt()` without rebuilding geometry.

### 16. PCG Solver Allocates Vectors Every Iteration

In `solver.rs`, the PCG solver allocates new `DVector`s for `z_new`, `p`, `r_new` at every iteration. Using pre-allocated workspace vectors would reduce allocation pressure.

### 17. `configureSimulator` Uses Dynamic Method Checks  

In `sim.worker.ts`, every configuration call uses `typeof setCellDimensions === 'function'` checks because of potential API version mismatches. This defensive coding adds complexity. If the WASM API is stable, these checks should be removed.

### 18. Chart.js Registers All Registerables

Both `RateChart.svelte` and `FractionalFlow.svelte` call `Chart.register(...registerables)` which imports every Chart.js module. Register only the needed components to reduce bundle size.

### 19. `sat_oil` Field Is Redundant

`sat_oil = 1.0 - sat_water` is maintained separately in every grid cell. This is error-prone and wastes memory/bandwidth. Derive it from `sat_water` when needed.

---

## P4 — Feature Enhancements

### 20. Support Multiple Well Patterns

Currently only two wells (1 injector + 1 producer) are supported. Add support for:
- 5-spot patterns
- Line drives
- Custom multi-well placement with drag-and-drop on the 3D grid

### 21. Add Grid Refinement / Corner-Point Grid Support

The current Cartesian grid with uniform cell sizes is limiting. Allow non-uniform cell sizes (dx varies by i) or simple corner-point geometry for more realistic reservoir models.

### 22. Add Time-Varying Well Controls (Schedule)

Allow BHP or rate changes at specified times (workover schedule). This would enable modeling well shut-ins, rate ramp-ups, and infill drilling scenarios.

### 23. Add Phase Relative Permeability Curve Visualization

Show interactive kr/Sw and Pc/Sw curves based on current SCAL parameters (Corey exponents, endpoint saturations) alongside the simulation. This would help users understand the physics they're configuring.

### 24. Add Water Cut vs. Time Chart

Currently the rate chart shows oil rate and water production separately. Add an explicit water cut (fw) vs. time curve, which is the most common diagnostic in waterflood operations.

### 25. Support Saving & Loading Custom Scenarios

Allow users to save their custom parameter configurations as JSON files and reload them later. Currently only pre-defined cases exist.

### 26. Add Comparison Mode (Overlay Multiple Runs)

Allow users to run a scenario, save the result, change parameters, run again, and overlay both results on the same chart. Essential for sensitivity analysis.

### 27. Add Sensitivity Tornado Chart

Run a base case with parameter variations (±10%, ±25%) for key parameters (permeability, viscosity, kv/kh ratio, etc.) and display a tornado chart of recovery factor sensitivity.

### 28. Improved 3D View: Cross-Section Slicing

Add the ability to slice the 3D grid with cutting planes (X-Z, Y-Z cross-sections) to see internal saturation/pressure distributions.

### 29. Add Summary Statistics Panel

Show real-time summary: OOIP, pore volume, recovery factor, average pressure, average Sw, water cut, VRR, cumulative production/injection volumes.

### 30. Add Undo/Redo for Parameter Changes

Track parameter change history to allow reverting to previous states.

---

## Implementation Priority Order

| Priority | Issue | Effort | Impact |
|----------|-------|--------|--------|
| P0-1 | Stop button fix | Small | High — critical usability |
| P0-2 | Pre-run case continuation fix | Medium | High — core workflow broken |
| P1-6 | Compressibility double-counting | Small | High — physics correctness |
| P1-5 | Material balance error fix | Small | Medium — diagnostic accuracy |
| P1-7 | Capillary pressure cap at Sw=Swc | Small | Medium — numerical stability |
| P2-9 | Progress indicator | Small | High — UX quality of life |
| P2-12 | Un-comment Welge diagram | Trivial | Medium — educational value |
| P2-11 | Results export/download | Small | High — practical value |
| P1-3 | Upstream mobility weighting | Medium | High — physics accuracy |
| P2-8 | Allow editing pre-loaded cases | Small | Medium — UX flexibility |
| P1-4 | Re-solve pressure on sub-step | Medium | Medium — accuracy |
| P2-10 | Improved playback controls | Small | Medium — UX |
| P2-13 | Inline validation feedback | Small | Medium — UX |
| P3-14 | History memory optimization | Medium | Medium — scalability |
| P3-19 | Remove redundant sat_oil | Small | Low — code quality |
| P3-17 | Remove dynamic method checks | Small | Low — code quality |
| P4-24 | Water cut chart | Small | Medium — operational value |
| P4-29 | Summary statistics panel | Small | Medium — practical value |
| P4-23 | SCAL curve visualization | Medium | Medium — educational value |
| P4-25 | Save/load custom scenarios | Small | Medium — practical value |
| P4-20 | Multiple well patterns | Large | High — feature expansion |
| P4-22 | Well schedule support | Medium | High — realism |
| P4-26 | Run comparison mode | Medium | High — engineering workflow |
| P4-28 | Cross-section slicing | Medium | Medium — visualization |
| P4-27 | Sensitivity tornado | Large | High — decision support |
| P4-21 | Non-uniform grid support | Large | Medium — realism |
| P3-15 | 3D view color-only updates | Medium | Medium — performance |
| P3-16 | PCG solver allocation reuse | Small | Low — performance |
| P3-18 | Selective Chart.js imports | Small | Low — bundle size |
| P2-30 | Undo/redo | Medium | Low — convenience |

---

## Recommended First Sprint (1-2 weeks)

1. **Fix stop button** — reduce `chunkYieldInterval` to 1, add dynamic yield based on step duration
2. **Fix pre-run continuation** — add timeout/error handling, validate hydration state machine 
3. **Fix compressibility double-counting** — remove `cell.porosity` multiplier from `c_t`
4. **Fix material balance error** — use consistent reservoir/surface volume basis
5. **Add simulation progress indicator** — show step X/N in RunControls
6. **Un-comment Welge diagram** — trivial win for waterflood cases
7. **Add results download** — CSV export for rate history

## Recommended Second Sprint (2-3 weeks)

8. **Upstream mobility weighting** — improve physics accuracy
9. **Allow editing pre-loaded case inputs** — smoother UX
10. **Water cut chart** — key engineering diagnostic
11. **Save/load custom scenarios** — practical workflow support
12. **Summary statistics panel** — OOIP, RF, avg pressure, VRR
