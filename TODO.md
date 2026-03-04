# TODO — ResSim Remaining Work Items

Consolidated from docs, reviews, action plans, and source code review.  
Items are grouped by priority.

---

## High Priority — Physics & Correctness

- [x] **Upstream mobility weighting in pressure equation** — `transmissibility_upstream()` in `step.rs` now uses upstream weighting based on potential difference. Confirmed in code review.
- [x] **Re-solve pressure on IMPES sub-step** — when `stable_dt_factor < 1.0`, pressure is now re-solved with the reduced dt via a second `calculate_fluxes(actual_dt)` call instead of reusing the pressure from the full `remaining_dt`.
- [x] **Material-balance error uses inconsistent volume basis** — Fixed to compute true cumulative missing material by accurately comparing effective mass changes after saturation bounds clamping strictly in reservoir volume conditions.
- [x] **Capillary pressure cap at `S_w ≤ S_wc`** — Replaced the arbitrary 1000 bar cap with a scaled heuristic bound (`20 * p_entry`), resolving the non-physical "capillary sponge" artifact where suction overpowered gravity.
- [ ] **Leverett J-Function scaling for Capillary Pressure** — Long term enhancement. Applying the J-Function allows $P_c$ to bound naturally based on local permeability and porosity, dynamically tightening the cap for variable rock properties and modeling real rock capillary transitions more rigorously than constant heuristics.
- [x] **Relative permeability endpoint scaling** — `k_rw_max` and `k_ro_max` multipliers natively supported and routed natively up into the front-end configuration inputs and Analytical plot lines `FractionalFlow.svelte`.
- [x] **Gravity head uses total density, not phase-density split** — gravity should act per-phase via phase potentials. Acceptable for weak gravity but incorrect for strong density contrast.
  - *Note: Phase-potential formulation significantly improved numeric stability for high-flow test cases over IMPES total velocity.*
- [x] **`maxRecoverable` naming** — `DepletionAnalytical.svelte` names `q0 × tau` as `maxRecoverable`; this is total expelled volume from compressible depletion, not recoverable oil. Rename to `totalExpelledVolume` or similar.
- [x] **`loadState` is broken** — `lib.rs:662-694`: `_grid_state` parameter is accepted but completely ignored; pressure/saturation arrays are never restored from it. Contains dead code (`if false {…}`) and a `TODO impl restore` comment. State hydration is incomplete.
- [x] **`so_new` redundantly clamped** — `step.rs:721` computes `so_new = (1.0 - sw_new).clamp(0.0, 1.0)` but `sw_new` is already clamped to `[s_wc, 1-s_or]`, so `so_new` is inherently in `[s_or, 1-s_wc]`. The `clamp` is misleading (suggests it could be out of range).
- [x] **`grid.rs` is empty** — the file exists and is imported via `mod grid` but contains no code (1 byte). Either populate it (e.g., move `GridCell` equivalent, `pore_volume_m3`, `idx`) or remove the empty module.

---

## High Priority — Frontend & UX

- [x] **Reactive clamping anti-pattern** — ~20 reactive statements aggressively clamp inputs while user types (e.g., deleting makes `0.` → forces `0.1`). Move validation to `buildCreatePayload` or `onBlur`. (Resolved via Svelte 5 store transition)
- [x] **Dual config-changed watchers** — two reactive blocks overlap in detecting parameter changes, causing redundant reinitializations. Consolidate into a single `checkConfigDiff()`. (Resolved via Svelte 5 store transition)
- [ ] **CSV/JSON export of results** — no way to export rate history, grid state snapshots, or saturation profiles for external analysis. Add download buttons via Blob API.
- [x] **Inline validation error highlighting** — validation errors exist but aren't shown inline on the offending input fields. Highlight with red borders and inline messages.
- [x] **History memory optimization** — each snapshot stores the full grid array. For large grids (e.g., 20×20×10 = 4000 cells, 300 steps), this consumes significant memory. Consider delta compression or reduced snapshot frequency.Reduced snapshots were implemented but not consistently used, missing in parts of the code across preloaded cases generation, UX, etc.
- [x] **Worker silently ignores `add_well` errors** — `sim.worker.ts:191-196` calls `simulator.add_well()` which returns `Result` but the worker doesn't check the return value. If grid indices or well params are invalid, the well is silently not added.

---

## Medium Priority — Simulation Improvements

- [ ] **Per-cell porosity variation** — porosity is already user-editable but uniform. Add per-layer or per-cell porosity input (uniform/per-layer modes, similar to permeability).
- [ ] **Per-cell initial water saturation** — enable water-oil contact / transition zone initialization with per-layer `S_w₀`.
- [ ] **Aquifer boundary conditions** — Carter-Tracy or Fetkovich aquifer influx model. Currently all boundaries are no-flow.
- [ ] **Acceptance tests for worker snapshots** — end-to-end parity tests between worker state snapshots and direct simulator state.
- [ ] **Expand physics regression benchmarks** — add at least one 3D gravity-drainage case and one heterogeneous 2D case beyond current 1D BL-only benchmarks.
- [ ] **Grid-convergence study preset** — auto-run same scenario at nx = 12, 24, 48, 96; overlay breakthrough PVI and recovery curves.
- [ ] **Benchmark trend tracking across commits** — baseline drift dashboard or CI report comparing benchmark values over time.
- [ ] **Calibrate depletion analytical model** — validate against published radial-flow references for center-producer depletion cases.
- [ ] **Full Dietz shape-factor table** — add output for additional boundary geometries and anisotropic grids beyond current 1D slab and 2D center cases.

---

## Medium Priority — Frontend Improvements

- [ ] **Cross-section / slice viewer** — add i/j/k slice selector to 3D view to inspect interior cells.
- [ ] **Sync x-axis range across chart panels** — zoom one panel → zoom all (requires Chart.js plugin coordination).
- [ ] **SwProfileChart legend consistency** — make legend pills match ChartSubPanel style.
- [ ] **Make SwProfile as 3D viz sub-card** — it shows spatial information similar to the 3D view.
- [ ] **Structured scenario export/import (JSON)** — allow saving/loading custom parameter configurations.
- [ ] **A/B run comparison** — store previous run's rate history, overlay as dashed series.
- [ ] **Relative error (%) curve in charts** — add percentage error between simulator and analytical.
- [ ] **Responsive layout on narrow screens** — 3D canvas + rate chart don't adapt gracefully on mobile.
- [ ] **Well schedule support** — time-varying BHP or rate changes (workover schedule).

---

## Low Priority — Code Quality & DevOps

- [ ] **CI pipeline for tests** — GitHub Actions: `cargo test` + `npm test` + `npm run build` on push/PR. Regenerate `benchmark-results.json` and compare.
- [ ] **Worker `typeof` guards → typed WASM interface** — `configureSimulator()` in `sim.worker.ts` uses 12+ `typeof X === 'function'` guards with `/** @type {any} */` casts. Generate proper TS bindings from `wasm-bindgen` or define a typed wrapper.
- [ ] **Frontend unit tests** — expand Vitest coverage for `FractionalFlow` analytical, `validateInputs()` edge cases. Currently only `buildCreatePayload`, `caseCatalog`, and `chart-helpers` have tests.
- [ ] **Selective Chart.js imports** — both `RateChart.svelte` and `FractionalFlow.svelte` register all registerables. Register only needed components to reduce bundle.
- [ ] **PCG solver allocation reuse** — `solver.rs:38,70` allocates new `DVector`s (`z`, `z_new`) per iteration. Pre-allocate workspace vectors outside the loop.
- [ ] **Remove redundant `sat_oil` array** — `sat_oil = 1.0 - sat_water` is maintained separately in every cell across `sat_oil: Vec<f64>`. Derive on access or via a helper to halve memory and eliminate sync risk.
- [ ] **3D view color-only updates** — `buildInstancedGrid()` in `3dview.svelte` recreates all mesh transforms on every state update. Update only colors via `setColorAt()` when grid geometry hasn't changed.
- [ ] **Pressure-equation neighbor allocation** — `step.rs:429` allocates a new `Vec<(usize,char,usize)>` for each cell's neighbors on every timestep. Use a fixed-size stack array `[(usize,char,usize); 6]` instead.
- [ ] **Sparse matrix rebuild every sub-step** — `calculate_fluxes()` in `step.rs` rebuilds the full sparse matrix (triplets → CSR) on every sub-step call. For fixed-topology grids, the sparsity pattern could be pre-built, updating only values.
- [ ] **Remove stale `tmp_heatpump.svelte`** — 135K file in root, appears unrelated to this project.
- [ ] **Clean up root-level temp scripts** — `fix_frontend_soa.mjs`, `fix_grid_cells_step.mjs`, `fix_grid_cells_tests.mjs`, `refactor_soa.mjs`, `test_hydrate.mjs`, `test_hydration_empty.mjs`, `test_hydration_payload.mjs`, `test_hydration_worker.mjs` are one-off migration/debug scripts. Delete if no longer needed.
- [ ] **Clean up root-level `.resolved` files** — `frontend_reactivity_review.md.resolved`, `task.md.resolved`, `task2.md.resolved`, `walkthrough.md.resolved` are resolved work artifacts. Delete.

---

## Long-Horizon / Nice-to-Have

- [ ] **Three-phase flow (oil/water/gas)** — phased API + UI rollout.
- [ ] **Horizontal / deviated well model** — generalized Peaceman PI.
- [ ] **Non-uniform cell sizes** — variable dx/dy/dz for local grid refinement.
- [ ] **Capillary hysteresis** — two-curve (drainage/imbibition) model.
- [ ] **Per-cell capillary pressure** — spatially varying P_entry and lambda.
- [ ] **Uncertainty/sensitivity batch runner** — seeded permeability ensembles.
- [ ] **Comparative visualization mode** — side-by-side scenarios / delta maps.
- [ ] **Report export** — plots + key metrics snapshot as PDF or HTML.
- [ ] **Sensitivity tornado chart** — parameter sensitivity analysis on RF.
- [ ] **Undo/redo for parameter changes**.
- [ ] **Multi-well patterns** — 5-spot, line-drive, custom placement.
- [ ] **Fetkovich type-curve overlay** — standard decline curve templates.
- [ ] **Areal sweep efficiency chart** — 2D waterflood diagnostic.
- [ ] **Phase relative permeability curve visualization** — interactive kr/Sw and Pc/Sw curves.
- [ ] **Summary statistics panel** — live OOIP, pore volume, RF, avg pressure, avg Sw, water cut, VRR.
- [ ] **Deterministic worker cancellation checkpoints** — persisted partial-state checkpoint metadata.
- [ ] **Multi-chart synchronized zoom/pan** — Chart.js plugin coordination.
