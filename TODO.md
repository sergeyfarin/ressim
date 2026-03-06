# TODO — ResSim Remaining Work Items

Consolidated from docs, reviews, action plans, and source code review.  
Items are grouped by priority.

## Product Direction (Locked: 2026-03-05)

- [x] **Adopt Option B** — Unified "Preset + Customize" surface is the primary product direction.
- [x] **No migration constraints** — greenfield product (no existing users, no legacy data).
- [x] **Analytical policy** — permissive analytical overlays with clearly visible warnings for approximate assumptions.
- [x] **Benchmark workflow** — benchmark presets remain curated references but must support one-click clone into custom mode.

---

## Authoritative Recovery Plan — Schema-Driven Composer (Interruption-Safe)

Single source of truth: this section is the authoritative tracker for the current frontend recovery/refactor path after the 2026-03-06 UI audit.

- [x] **R0.1 Audit current UI state** — confirmed the current `ModePanel` is an intermediate splice: it is hardcoded, passes `params: any`, and bypasses part of the Phase 2 preset/customize contract (`isModified`, grouped override/reset UX, shell-level provenance/override visibility).
- [x] **R0.2 Reset docs to an authoritative plan** — updated `TODO.md`, `docs/status.md`, `docs/PHASE2_PRESET_CUSTOMIZE_CONTRACT.md`, and `docs/FRONTEND_INPUT_SELECTION_REACTIVITY_REVIEW_2026-03-05.md` so interruption-safe resume state reflects the schema-driven direction rather than the older shell-only plan.
- [ ] **R1.1 Restore unified-panel preset/customize semantics (in progress)** — route manual field edits back through domain intent so `isModified`, `basePreset.source`, `parameterOverrides`, and benchmark clone provenance remain truthful in the live UI.
- [ ] **R1.2 Define typed schema for UI composition** — introduce typed definitions for parameters, controls, sections, presets/facets, simple patches, formatting metadata, and custom-entry affordances. Keep behavior/rules in TypeScript, not JSON strings.
- [ ] **R1.3 Define warning severity + surfacing policy** — separate `blocking validation`, `non-physical or contradictory`, `approximate/reference-model caveat`, and `advisory` states, with explicit UI surfaces for each.
- [ ] **R1.4 Migrate Geometry + Grid to schema renderer** — prove the approach on one vertical slice before replacing the whole panel.
- [ ] **R1.5 Add toggle-plus-custom pattern** — allow curated quick-select options (e.g. `nx = 12 | 24 | 48`) plus a `Custom` affordance that reveals a typed input or advanced parameter sub-panel.
- [ ] **R1.6 Migrate remaining sections to schema renderer** — reservoir, SCAL, wells, timestep, analytical, and benchmark surfaces.
- [ ] **R1.7 Remove obsolete shell-era UI leftovers** — retire `TopBar.svelte`, `InputsTab.svelte`, `PresetCustomizeShell.svelte`, and any stale App-side assumptions once schema-driven parity is reached.
- [ ] **R1.8 Regression + policy hardening** — add tests for modified-state transitions, clone provenance, override visibility/reset behavior, warning policy, and schema-driven rendering.

Recovery acceptance checklist:

- [ ] A manual field edit always transitions the current preset into truthful customized state.
- [ ] Quick presets and custom input can coexist in the same control group without ambiguous precedence.
- [ ] Control layout/labels/options come from typed schema/config, not hardcoded component wiring.
- [ ] Constraint rules remain deterministic and code-defined; no string-encoded logic is introduced in JSON.
- [ ] Warning surfaces are explicit: blocking errors stop run, non-reference states stay permissive with visible rationale.
- [ ] `TODO.md` and `docs/status.md` are sufficient to resume work after interruption without rereading old chat history.

Interruption resume protocol (mandatory):

- [ ] Keep this recovery section current before ending a work session.
- [ ] Mark only one active slice at a time by adding `(in progress)` in the item text.
- [ ] Append each completed slice outcome to `docs/status.md` with files touched, tests run, and the next active slice.
- [ ] If interrupted mid-slice, add a short `WIP` note in `docs/status.md` naming the file and pending edit.
- [ ] Do not start a new slice until `TODO.md` and `docs/status.md` are synchronized.

---

## Active Execution Plan — Phase 1 (Interruption-Safe)

Single source of truth: this section is the authoritative tracker for ongoing Phase 1 work.

- [x] **P1.1 Validation centralization** — store uses shared `validateInputs.ts` implementation.
- [x] **P1.2 Explicit model-reset diff domain** — model reset key narrowed to structural/model domain.
- [x] **P1.3 Store state domain split (API layer)** — introduce explicit `scenarioSelection`, `parameterState`, `runtimeState` in store return shape.
- [x] **P1.4 Compatibility shim window** — preserve current top-level store fields as temporary aliases to avoid big-bang breakage.
- [x] **P1.5 App migration** — move `App.svelte` consumers to domain objects with no behavior change.
- [x] **P1.6 UI consumer migration** — verified no additional direct store consumers beyond `App.svelte`; no extra migration edits required in this slice.
- [x] **P1.7 Domain-scoped dirty/reset behavior** — ensured model-reset key includes `reservoirPorosity`; runtime-only controls remain outside model-reset signature.
- [x] **P1.8 Remove shim fields** — removed temporary top-level compatibility aliases; store now exposes domain objects only.
- [x] **P1.9 Validation and regression pass** — completed typecheck + tests + diagnostics after domain migration and shim removal.
- [x] **P1.10 Docs and handoff update** — `docs/status.md` synchronized with completed slices, validations, and next-step context.

Phase 1 acceptance checklist:

- [x] Store exposes explicit domain objects and App uses them.
- [x] No silent behavior regression in run/init/step flow. Verified 2026-03-05 with targeted Rust tests: `cargo test adaptive_timestep_produces_multiple_substeps_for_strong_flow`, `cargo test pressure_resolve_on_substep_produces_physical_results`, `cargo test saturation_stays_within_physical_bounds`.
- [x] Validation gating and error visibility remain unchanged or improved. Verified 2026-03-05 via targeted frontend tests (`npm run test -- src/lib/validateInputs.test.ts src/lib/buildCreatePayload.test.ts`) and wiring checks in `src/App.svelte` + `src/lib/stores/simulationStore.svelte.ts` (run controls disable + explicit blocked-run runtime error).
- [x] Benchmark and non-benchmark case selection behavior remains correct. Verified 2026-03-05 via `npm run test -- src/lib/caseCatalog.test.ts`. Note: benchmark pre-run gate checks were later superseded by complete pre-run pipeline removal.
- [x] Temporary compatibility shims removed before Phase 1 close.

Interruption resume protocol (mandatory):

- [ ] Keep this Phase 1 section current before ending a work session.
- [ ] Mark only one active slice at a time by adding `(in progress)` in the item text.
- [ ] Append each completed slice outcome to `docs/status.md` with tests run and explicit next step.
- [ ] If interrupted mid-slice, add a short `WIP` note in `docs/status.md` with current file and pending edit.
- [ ] Do not start a new slice until TODO and status are synchronized.

---

## Active Execution Plan — Phase 2 (Interruption-Safe)

Single source of truth: this section is the authoritative tracker for ongoing Phase 2 work.

Historical note: this section records the earlier shell-oriented Phase 2 slices. It is no longer the authoritative forward plan after the 2026-03-06 UI audit. Use `Authoritative Recovery Plan — Schema-Driven Composer` above for current work.

- [x] **P2.1 UX contract + state schema freeze** — unified contract module and store schema fields landed (`basePreset`, `parameterOverrides`, `benchmarkProvenance`, `analyticalStatus`) with focused tests and docs (`src/lib/stores/phase2PresetContract.ts`, `src/lib/stores/phase2PresetContract.test.ts`, `docs/PHASE2_PRESET_CUSTOMIZE_CONTRACT.md`).
- [x] **P2.2 Preset composer shell UI** — hybrid flow polished: per-facet `Customize` + `Reset` controls are part of each facet group, active customize selection is highlighted, section-targeted focus/highlight is wired, customize sessions collapse via explicit `OK`, facet mapping is centralized in shared Phase 2 contract helpers, and generated-profile controls now include shell-level `show changed fields` and per-group quick actions.
- [x] **P2.3 Override tracking + changed-field UX** — shell-level per-group reset-to-preset and "show changed fields" are wired against deterministic base-preset diff, with dedicated contract-level regression tests for deterministic override ordering and grouped reset-plan behavior (including de-duplication and stale-key filtering).
- [x] **P2.4 Benchmark clone-to-custom flow** — added one-click `Clone to Custom` from benchmark presets in shell/top controls, wired provenance creation and display, and enforced immutable lineage per clone session by clearing provenance only on preset/mode changes.
- [x] **P2.5 Analytical eligibility evaluator** — analytical status now computes `reference | approximate | off` with explicit reason details and severity levels (`none | notice | warning | critical`) plus deterministic summary severity for UI policy.
- [x] **P2.6 Analytical status banner UX** — wired persistent, high-visibility approximation banner above analytics panels with expandable caveat details and per-reason severity badges/tooltips.
- [x] **P2.7 Store/App integration hardening** — moved clone/reset UI flows to domain APIs (`scenarioSelection`, `parameterState`), removed App-side transitional assembly logic, and tightened analytical banner contract consumption.
- [ ] **P2.8 Regression + policy tests (in progress)** — add tests for clone provenance, changed-fields diffing, analytical status modes, benchmark selection/run policy after pre-run removal, and validate the per-mode catalog schema migration.
- [x] **P2.9 Remove pre-run loading pipeline** — removed benchmark pre-run fetch/decompression/hydration path; benchmark selection now only applies case parameters and runs always start from fresh init.
- [ ] **P2.10 Docs + handoff update** — sync `docs/status.md`, capture decision rationale, and record residual follow-ups with verified commands/results.

Phase 2 acceptance checklist:

- [x] Unified Preset + Customize surface is primary path for depletion/waterflood/simulation and benchmark entry.
- [ ] Benchmark presets can be cloned into custom mode with traceable source metadata.
- [ ] Analytical overlay remains permissive, with persistent warning and explicit reasons when in approximate mode.
- [ ] No disabled facet remains selected after any interaction sequence.
- [x] No scenario interaction triggers pre-run fetch (pipeline removed).
- [ ] Run/step invalid-input behavior is explicit and never a silent no-op.
- [ ] App/store regression tests cover Phase 2 pathways (clone, overrides, analytical status, policy guards).

Interruption resume protocol (mandatory):

- [ ] Keep this Phase 2 section current before ending a work session.
- [ ] Mark only one active slice at a time by adding `(in progress)` in the item text.
- [ ] Append each completed slice outcome to `docs/status.md` with tests run and explicit next step.
- [ ] If interrupted mid-slice, add a short `WIP` note in `docs/status.md` with current file and pending edit.
- [ ] Do not start a new slice until TODO and status are synchronized.

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

- [x] **Implement Option B shell UI** — replaced TopBar + InputsTab with unified ModePanel: mode tabs + collapsible sections with nested dimension sub-selectors and expandable inline parameter panels. Removed customize-flow indirection from App.svelte.
- [ ] **Replace intermediate `ModePanel` with schema-driven composer** — current panel proves the unified layout direction but hardcodes sections/controls and bypasses part of the preset/customize contract.
- [ ] **Restore truthful customized-state UX in unified panel** — manual edits must drive `isModified`, grouped override visibility, and reset-to-preset semantics through the store contract.
- [ ] **Support quick-select + custom-entry controls** — curated toggles/selects should be able to reveal typed custom inputs or advanced parameter groups without splitting the mental model.
- [ ] **Define warning policy by severity** — use blocking validation for invalid runs, visible warnings for contradictory/non-physical inputs, and permissive caveats for approximate analytical/reference assumptions.
- [ ] **Benchmark clone flow** — add `Clone to Custom` action that copies all benchmark parameters and stores source benchmark provenance.
- [x] **Analytical status banner** — introduced `reference | approximate | off` status path with persistent, highly visible warning and expandable caveat details for approximate overlays.
- [x] **Remove pre-run loading pipeline** — benchmark/non-benchmark selections now only apply parameters; no prerun fetch/decompression/hydration path remains.
- [x] **Fix run validation gating** — pass `hasValidationErrors` into run controls and show explicit message when run is blocked by invalid inputs.
- [x] **Fix custom-subcase mode mapping** — normalize mode aliases (`dep/wf/sim` vs `depletion/waterflood/simulation`) so custom sub-case switching is reliable.
- [x] **Stabilize faceted constraints** — replace one-pass toggle auto-fix with iterative constraint stabilization.
- [x] **Preserve per-layer edits on Nz change** — resize layer arrays without wiping existing user values.
- [x] **Fix stale simulator reuse after case geometry changes** — 2D/3D runs could execute on previous 1D simulator instance; runs now force reinit when model is stale before sending run payload.
- [x] **Use single-source validation in store** — `simulationStore` now consumes `validateInputs.ts` instead of duplicating rules.
- [x] **Model-domain config diff tracking** — config diff signature now tracks model-reset domain explicitly via `buildModelResetKey`.

- [x] **Reactive clamping anti-pattern** — ~20 reactive statements aggressively clamp inputs while user types (e.g., deleting makes `0.` → forces `0.1`). Move validation to `buildCreatePayload` or `onBlur`. (Resolved via Svelte 5 store transition)
- [x] **Dual config-changed watchers** — two reactive blocks overlap in detecting parameter changes, causing redundant reinitializations. Consolidate into a single `checkConfigDiff()`. (Resolved via Svelte 5 store transition)
- [ ] **CSV/JSON export of results** — no way to export rate history, grid state snapshots, or saturation profiles for external analysis. Add download buttons via Blob API.
- [x] **Inline validation error highlighting** — validation errors exist but aren't shown inline on the offending input fields. Highlight with red borders and inline messages.
- [x] **History memory optimization** — each snapshot stores the full grid array. For large grids (e.g., 20×20×10 = 4000 cells, 300 steps), this consumes significant memory. Consider delta compression or reduced snapshot frequency.Reduced snapshots were implemented but not consistently used, missing in parts of the code across preloaded cases generation, UX, etc.
- [x] Worker silently ignores `add_well` errors — `sim.worker.ts:191-196` calls `simulator.add_well()` which returns `Result` but the worker doesn't check the return value. If grid indices or well params are invalid, the well is silently not added.
- [x] **FilterCard/ToggleGroup alignment** — unified `FilterCard` to use the more polished `ToggleGroup` internally, including adding a wrapping grid layout for elements > 3.
- [x] **Retire pre-run decompression path** — removed `.json.gz` pre-run loader and related DecompressionStream fallback from `simulationStore.svelte.ts`.
- [x] **End-to-end regression validation for benchmark pre-run data** — no longer applicable after pre-run pipeline removal.

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
- [ ] **Additional published benchmarks** — search for 2-phase immiscible verification cases in:
  - SPE Comparative Solution Projects (extract 2-phase subsets from SPE1/SPE5/SPE9)
  - Dake, "Fundamentals of Reservoir Engineering" — worked examples Ch. 5-10
  - Craft, Hawkins & Terry, "Applied Petroleum Reservoir Engineering" — depletion cases
  - Craig, "The Reservoir Engineering Aspects of Waterflooding" — areal sweep factors
  - MRST (MATLAB Reservoir Simulation Toolbox) verification tutorials
  - ECLIPSE/CMG published benchmarks (reservoir engineering textbook appendices)
  - ResearchGate papers tagged "reservoir simulation verification" or "immiscible displacement"
  - McEwen (1961) — single-phase slightly compressible radial flow analytical solution

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
- [x] **Frontend unit tests** — expand Vitest coverage for `FractionalFlow` analytical, `validateInputs()` edge cases. Currently only `buildCreatePayload`, `caseCatalog`, and `chart-helpers` have tests.
- [x] **App-store domain wiring regression tests** — added static wiring checks in `src/lib/appStoreDomainWiring.test.ts` to guard domain-object usage and prevent fallback to App-side transitional logic.
- [ ] **Model-reset domain key coverage tests** — add unit tests to lock model-reset signature field coverage (e.g., `reservoirPorosity`) and prevent omission regressions.
- [ ] **Restore lint toolchain availability** — `npm run lint` currently fails with `eslint: not found`; add/install ESLint dependency in dev setup or align lint script with available tooling.
- [ ] **Selective Chart.js imports** — both `RateChart.svelte` and `FractionalFlow.svelte` register all registerables. Register only needed components to reduce bundle.
- [ ] **PCG solver allocation reuse** — `solver.rs:38,70` allocates new `DVector`s (`z`, `z_new`) per iteration. Pre-allocate workspace vectors outside the loop.
- [ ] **Remove redundant `sat_oil` array** — `sat_oil = 1.0 - sat_water` is maintained separately in every cell across `sat_oil: Vec<f64>`. Derive on access or via a helper to halve memory and eliminate sync risk.
- [ ] **3D view color-only updates** — `buildInstancedGrid()` in `3dview.svelte` recreates all mesh transforms on every state update. Update only colors via `setColorAt()` when grid geometry hasn't changed.
- [ ] **Pressure-equation neighbor allocation** — `step.rs:429` allocates a new `Vec<(usize,char,usize)>` for each cell's neighbors on every timestep. Use a fixed-size stack array `[(usize,char,usize); 6]` instead.
- [ ] **Sparse matrix rebuild every sub-step** — `calculate_fluxes()` in `step.rs` rebuilds the full sparse matrix (triplets → CSR) on every sub-step call. For fixed-topology grids, the sparsity pattern could be pre-built, updating only values.
- [x] **Remove stale `tmp_heatpump.svelte`** — removed root-level legacy artifact on 2026-03-05.
- [x] **Clean up root-level temp scripts** — removed root-level one-off migration/debug scripts on 2026-03-05 (`fix_frontend_soa.mjs`, `fix_grid_cells_step.mjs`, `fix_grid_cells_tests.mjs`, `refactor_soa.mjs`, `test_hydrate.mjs`, `test_hydration_empty.mjs`, `test_hydration_payload.mjs`, `test_hydration_worker.mjs`).
- [x] **Clean up root-level `.resolved` files** — verified no root-level `.resolved` artifacts are present as of 2026-03-05.

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
