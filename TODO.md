# ResSim TODO

Active tracker — **open items only**. Reprioritized and pruned 2026-07-24: the user-facing
frontend/scenario/validation work now leads; FIM convergence is a parked dev-only maintenance track
(the 2026-07-24 re-baseline in `docs/SOLVER_COMPARISON_SUMMARY.md` shows heavy water at 4 substeps,
gas ~2x Flow, SPE1 OPM-class — no open *correctness* convergence defect on the shipped default).

- **Completed history:** `.archive/docs/TODO_HISTORY_2026-07-24.md` (full prior 1,474-line tracker,
  incl. all Wave 0–4 and FIM experiment narrative) and `.archive/docs/DELIVERED_WORK_2026_Q1.md`.
- **Future/backlog:** `ROADMAP.md` — do not duplicate future work here.
- **FIM provenance:** `docs/FIM_STATUS.md`, `docs/FIM_EXPERIMENT_REGISTRY.md` (search by mechanism
  before any convergence change), `docs/FIM_CONVERGENCE_WORKLOG.md`.

Keep this file short and action-oriented. Long narratives go to the worklog/registry, not here.

---

## Priority 1 — Frontend & scenario (user-facing critical path)

- [x] **Depletion catalogue mixes incompatible analytical contracts (2026-07-29).** Replaced the
  distributed-slab/single-exponential Fetkovich comparison with a finite-domain flow-regime
  reference; turn the Dietz case into a constant-rate PSS productivity measurement; rebase the
  N·c_t ambiguity exhibit on an exact lumped-tank contract; retain layered decline as exact tank
  superposition. Keep solution-gas drive as a separate black-oil/FIM mechanism and revalidate its
  gas evolution and committed OPM comparison rather than applying the oil-only depletion model.
  Added headless WASM gates for the finite-slab rate and Dietz late-time flowing BHP contracts;
  the curated FIM gate confirms solution-gas liberation, increasing free gas/GOR, phase closure,
  material balance, and the committed Flow 2026.04 acceptance bands.

- [x] **Layered depletion reference matched the wrong physical model (2026-07-29).** Replaced the
  repeated numerical runs behind the Arps-b picker with a fixed-total-PI layer-contrast experiment.
  Each layer is now a noncommunicating tank, and the quantitative reference sums its own PI/storage
  exponential and volume-averages layer pressure. Added a WASM replay gate across 3:1, 20:1, and
  100:1 contrast; corrected the decline citation from Fetkovich's 1971 aquifer paper to SPE-4629
  (1980).
- [x] **3D camera fit adapts to model shape (2026-07-29).** Replaced the bounding-sphere camera
  distance and its large fixed multiplier with a projected bounding-box fit. Long, thin 1D grids
  now fill the canvas more closely, while XZ/vertical grids use their actual projected height and
  retain a consistent safety margin.
- [x] **Stable pressure scale shared by 3D and spatial profile (2026-07-29).** Pressure now uses a
  pre-run physical envelope derived consistently from initial reservoir pressure and active well
  controls (pressure setpoints, or BHP limits for rate control). The scale no longer depends on the
  snapshots received so far, and the profile shares the 3D range while manual legend edits remain
  available.
- [x] **Buckley–Leverett spatial front used bulk rather than pore velocity (2026-07-29).**
  Corrected the saturation-profile reference overlay to divide injected volume by porosity and to
  integrate the pressure-controlled injection history through the selected replay time. Replaced
  the old piston-step drawing with the complete BL rarefaction/shock profile so the analytical
  curve remains visible and physical after breakthrough. The rate, water-cut, and recovery overlays
  already used injected pore volume and were unaffected.
- [x] **Spatial-profile geometry and replay correctness (2026-07-29).** Removed the redundant
  legend; hid path controls for 1D grids; made ordinary XY profiles pass through the producer and
  corner-pattern profiles follow the injector-to-producer diagonal; defaulted layered horizontal
  profiles to a K-column average with an explicit layer override. Completed-run replay now selects
  the indexed history grid/wells instead of pairing the indexed time with the final snapshot.
- [x] **Well-Test drawdown presentation (2026-07-29).** Restored the scenario's intentionally
  absent 3D default because rate-controlled skin variants have the same reservoir pressure field;
  made flowing BHP the sole expanded and explicitly visible chart, demoted constant oil rate to a
  collapsed control check, and made the grid-resolution study render its unchanged analytical
  solution once rather than as three coincident per-result curves. The explicit visibility matters
  because dedicated BHP panels are hidden in the shared fallback presentation.
- [x] **Positive reservoir-rate targets overridden by zero surface-rate sentinels (2026-07-29).**
  Legacy-generated explicit well schedules serialized the UI's unset `targetSurfaceRate = 0` and
  the well controller correctly gave it precedence over `targetRate`. Rate-controlled catalog
  wells therefore ran at zero rate (the Well-Test BHP stayed at its 300-bar initial pressure).
  Generated schedules now omit non-positive surface targets so the intended reservoir target is
  used; genuinely zero-rate schedules remain expressible through `targetRate: 0`.
- [x] **Spatial-view controls and profile alignment (2026-07-28).** Moved the shared property
  selector and legend range onto one compact row below the 3D canvas, followed by the live/scenario
  result selector and timestep scrubber; made the profile heading reflect the selected property;
  reduced the 3D canvas height by 25%. Scenario definitions now explicitly default the shared 3D
  and profile selector to water saturation for waterflood/sweep cases, pressure for oil-only
  depletion and pressure-transient cases, and gas saturation for gas/black-oil cases.

### SPE1 reference data (2026-07-24)
- [x] **SPE1 published oil-rate/BHP overlays were wrong.** The 4-point "Brontosaurus" samples showed
  oil rate ≈ flat to 1826 d (3155.9 Sm³/d); the real SPE1 Case 1 producer hits its 1000 psia BHP
  floor at ~1000 d and declines (1758.5 Sm³/d at 1826 d, 883.7 at 3650 d). Replaced with a monthly
  series from `flow 2026.04` on `OPM/opm-common/tests/SPE1CASE1.DATA` (WELLDIMS raised to 4 so the
  RFT wells load; `flow SPE1CASE1.DATA --output-dir=.`). ResSim's own decline onset (~950 d) was
  correct all along. `ECLIPSE_GOR` verified against the same run (≤0.3 %); `ECLIPSE_PRESSURE` tracks
  block (1,1,1) pressure, ~25 bar below it — labelled "Avg Pressure", worth renaming.
- [x] **(MAJOR) The generated OPM decks in `tools/opm_flow/opm_flow_tool/cases.py` were malformed.**
  `COMPDAT` put the wellbore radius in item 8 (connection transmissibility factor) instead of item 9
  (wellbore *diameter*), choking every connection by ~2 orders of magnitude — SPE1 ran at FOPR ≈ 24
  Sm³/d with both wells BHP-pinned from day 1, and `wf_bl1d` at FOPR ≈ 1e-4 Sm³/d (which was the
  cause of its long-standing "degenerate reference" caveat, now closed). Fixed by defaulting items
  7-8 and passing the diameter in item 9; SPE1 additionally got real depths (`TOPS` 2537.46 m with
  matching `EQUIL`/`RSVD`/`WELSPECS` datums), `EQLDIMS`, and `DRSDT 0`. Both artifacts regenerated
  with `flow 2026.04`. SPE1 now matches the canonical `SPE1CASE1.DATA` run to ~4 % on FOPR with the
  same ~day-1000 decline onset; `wf_bl1d` shows a proper BL front breaking through at ~14.5 d.

### Wave 4 follow-ups
- [x] **E5 log-time history divider (2026-07-28).** A scenario time boundary now maps to
  `Math.log10(boundary)` on the log-time presentation axis (positive boundaries only). Originally
  demonstrated by `dep_nct`; that case was withdrawn on 2026-07-30 because a correctly positioned
  divider still did not provide the required reserves/identifiability result view.
- [ ] **(MINOR, E7 cosmetic) 3D card hidden for pre-run scenarios leaves `xl:grid-cols-2` with one
  child** — empty right half on xl. Consider full-width chart when `isPrerunScenario`. Verify in
  `pnpm run dev`.
- [ ] **(MINOR, E1) `permMode: 'field'` single-run path not wired.** Flows only via the sweep path;
  `parameterStore.fieldPermX/Y/Z` default `[]` with no UI and `applyResolvedParams` doesn't map field
  arrays, so a single-run field-perm init silently falls back to uniform. Close with the first
  consuming scenario (Tavassoli/SPE10/Egg).
- [ ] **(MINOR) Wave-1 parser/artifact gaps:** (a) `_build_series` doesn't verify RSM unit strings vs
  `case.units` (TIME assumed days); (b) multi-page RSM merge success path has no real-data test;
  (c) `deckHash`/`flowVersion` stamped at build-artifacts time, not run time; (d) `find_summary_file`
  takes the first `*.RSM` glob match.

### Large-grid IMPES runtime (headless catalog sweep, 2026-07-24)
Measured all 130 catalog cases headless in Node against the committed wasm
(`node tmp/time-all.mjs`, harness not committed). Full catalog ≈ 1000 s; three scenarios own 97 % of it:

| Scenario (all variants) | Total | Worst single case |
|---|---|---|
| `sweep_combined` | 458 s | `interaction_favorable_layered` 21×21×5, 69 s |
| `spe1_gas_injection` | 268 s | `grid/grid_20` 20×20×3, 162 s |
| `sweep_areal` | 241 s | `grid_resolution/grid_high` 48×48×1, 124 s |

- [x] **SPE1 `grid_20` slowness diagnosed.** Not a frontend or worker problem — the worker's
  snapshot path is already incremental (`historyInterval = ceil(steps/25)`, `getRateHistorySince`
  deltas). Cost is engine-side and CFL-bound: 4089 accepted substeps × 1200 cells for 4000 days.
  Forcing `delta_t_days: 2.5 / steps: 1600` in the variant patch is **not** what makes it slow —
  running the same 4000 days at the base `dt = 30` costs the same wall time (172 s vs 162 s),
  because the internal adaptive loop subdivides to the same CFL limit either way.
- [x] **Repeated symbolic LU factorization removed** (`solvers/faer_sparse_lu.rs`). `sp_lu()` redid
  the fill-reducing ordering on every solve although the IMPES pressure pattern is fixed for a run.
  Now cached per sparsity pattern; numeric factorization still runs every solve, so trajectories are
  unchanged (substep/solve/retry counts bit-identical before/after on all three cases below).
  Measured: `sweep_combined` 55.6 → 48.4 s, `sweep_areal/grid_high` 49.4 → 40.1 s,
  `spe1/grid_20` 26.1 → 25.0 s (equal step caps, sequential, no CPU contention).
- [x] **(MAJOR) Large-grid pressure solves now use a size-aware iterative strategy.** Grids with
  at least 512 pressure rows use warm-started BiCGSTAB with scalar ILU(0); smaller systems retain
  sparse LU, and any failed iterative solve falls back to LU before IMPES cuts the timestep. The
  stopping test is RHS-relative like the LU residual check, so a good previous-pressure warm start
  can terminate immediately instead of being required to reduce its already-small residual by
  another `1e-7`. On clean commit `86e467c`, the committed replay
  `node scripts/fim-wasm-diagnostic.mjs --preset water-rate --grid 48x48x1 --steps 100 --dt 2
  --solver impes --diagnostic summary --no-json` took 3.35 s; the final dirty-tree replay took
  2.08 s (provisional 1.61x wall-clock speedup) with identical printed rates and pressure range.
  A provisional exact `spe1/grid_20` 4000-day replay completed its 1600-step loop in 62.8 s versus
  the prior 162 s catalog-sweep measurement (~2.6x), but its temporary timing harness was not kept,
  so treat that number as directional rather than a replayable baseline. Direct-vs-iterative tests
  cover a 576-row nonsymmetric Cartesian pressure system at `1e-10` relative residual.
- [x] **`spe1 delta_t` sensitivity rebuilt on measurement (2026-07-24).** The original item assumed
  an IMPES CFL-resubdividing outer loop; since b88ee28 the scenario is FIM (`fimEnabled: true`), and
  every rung measures **1.00–1.02 accepted substeps per outer step** — the outer Δt *is* the implicit
  solve, so this was never outer-loop overhead. Two real defects were found instead: `delta_t_5`
  patched only `delta_t_days` and inherited `steps: 120`, so it covered **600 days against the other
  rungs' 4000**; and the base-params comment claimed 4000 days when `120 × 30 = 3600`. The ladder is
  now 30 (base) / 5 / 2.5 / 1.25 days at a uniform 3600-day window, and `delta_t_0_25` is dropped.
  Measured headless on this tree by driving `sim.worker.ts`'s own message loop against
  `buildScenarioRunSpecs('spe1_gas_injection', 'delta_t', …)` — i.e. the exact catalog config the
  app runs — with `ReservoirSimulator.prototype.step` wrapped to accumulate
  `getLastFimStepStats().accepted_substeps`. 300 cells, FIM, sequential, no CPU contention. The
  harness was a throwaway vitest file and was **not kept**, so these are directional numbers, not a
  replayable baseline:

  | Δt (d) | steps | wall | substeps/step | end avg P (bar) | end GOR | end oil rate |
  |---|---|---|---|---|---|---|
  | 30 | 120 | 4.47 s | 1.02 | 256.2818 | 3847.25 | 878.824 |
  | 5 | 720 | 17.49 s | 1.00 | 256.1038 | 3857.94 | 876.630 |
  | 2.5 | 1440 | 32.61 s | 1.00 | 256.0868 | 3859.02 | 876.412 |
  | 1.25 | 2880 | 69.03 s | 1.00 | 256.0810 | 3859.14 | 876.368 |
  | 0.25 (dropped) | 14400 | 342.02 s | 1.00 | 256.0768 | 3856.82 | 876.612 |

  Cost is linear in step count (~23 ms/solve). The dropped rung costs 5× the rest of the ladder
  combined and moves end-state pressure by 0.002 % / GOR by 0.06 % versus Δt = 1.25 — it taught
  nothing the 1.25 rung does not. Provisional: measured on the dirty tree that became this commit,
  not re-replayed after commit.
- [x] **`sim.worker.ts` per-step rate-history marshalling removed.** Added
  `getLatestRatePoint()` (`frontend.rs`) returning the last `TimePointRates` or `null`;
  `peekLatestRatePoint()` now calls it instead of `getRateHistorySince(lastRateHistoryLen)`.
  Equivalence probed over 25 steps on a 10×1×1 waterflood: identical to both
  `last(getRateHistory())` and the old `getRateHistorySince` tail, and `null` before any rates
  exist (throwaway node probe, not kept). Measured probe cost: 373.8 → 2.5 µs/call at a 128-point
  tail (152×), 1277.2 → 2.4 µs/call at a 640-point tail (523×). Worth ~0.5 s on a 2880-step run —
  small next to the solver, as the original item said, but now zero.
- [x] **`pnpm test` was running the entire suite twice (2026-07-24).** Agent worktrees under
  `.claude/worktrees/` are full checkouts, and vitest's default `include` swept them up: 84 test
  files / 136 s instead of 42 / 22 s, with both copies of the heavy scenario tests competing for CPU.
  That is how it surfaced — `wf_tornado.test.ts` in the worktree copy hit the 30 s timeout during
  `validate:product` while the same test passed in isolation. `vitest.config.ts` now excludes
  `.claude/worktrees/**` and `tmp/**`. Any past "flaky timeout" in a scenario test is suspect for
  this cause.
- [ ] **(MINOR) `spe1 grid/grid_20` covers a different time window than its siblings.** It patches
  `delta_t_days: 2.5, steps: 1600` → 4000 days, while the base and the other grid rungs run
  `120 × 30 = 3600`. Same defect class as the `delta_t_5` one fixed on 2026-07-24 (a variant patching
  Δt without re-deriving `steps`). A scan of every scenario for `steps × Δt` drift within a
  dimension also flags `sweep_areal` (variants at 325 d and 1250 d against a 625 d base) and
  `wf_bl1d` (75 d against a 50 d base). Some of those may be deliberate — a coarser rung can need
  longer to reach breakthrough, and a termination policy can end the run early regardless — so each
  needs a judgement call, not a blanket fix. A contract test asserting the invariant per dimension
  (with an explicit opt-out flag) would stop the accidental cases recurring.
- [ ] **(MINOR, style)** `navigationStore`/`runtimeStore` import benchmark types via the
  `benchmarkCases` stub re-export instead of `scenario/referenceTypes` directly — trivial cleanup
  when next touched.

### Chart / catalog architecture
- [ ] **Chart consolidation is on the product critical path.** Scenario-library Tiers 5–6 can't land
  cleanly on the multi-generation chart stack without growing `buildChartData.ts` (forbidden by the
  frontend-architecture skill). Schedule a scoped ROADMAP P3.1 / COMPARISON_TOOLBOX Phase B pass
  before more chart features.
- [ ] **Pending-overlay case colors are position-derived, not identity-derived.** In
  `buildChartData.ts` the dashed overlay for a still-running variant takes
  `getReferenceComparisonCaseColor(orderedResults.length + i)`, while its cases-selector chip takes
  the variant's declaration index from `previewVariantParams`. Those agree only because sweeps
  complete in declaration order; any out-of-order completion recolors the curve away from its chip.
  Found while adding the ROADMAP 1.3 color-index-stability tests (2026-07-25) — the new tests pin
  the in-order behavior, so the fix is to key both paths off the declaration map.
- [ ] **The 2026-03-07 UI audit was never converted to backlog and is partially stale** (predates the
  scenario-first migration). Re-verify its 14 findings against the current UI before any UX pass;
  keep only survivors.

### Scenario library (Tier 5–7)

- [x] **Scenario-library taxonomy and scenario-ownership audit (2026-07-28).** Removed the redundant
  `wf_bl1d_opm` picker entry (the retained `wf_bl1d` uses its analytical reference) and withdrew
  `wf_tornado` until an interaction-specific plot/workflow exists. Renamed the retained cases,
  introduced six explicit future-facing catalog groups plus an orthogonal scenario role, rendered
  real group headings, and moved picker summaries and solver policy/rationale into every scenario.
  Removed the live gas-case key branch and duplicated scenario-to-chart adapter; added
  `scenarioAgnosticArchitecture.test.ts` to reject future canonical-key branching. Ownership and
  admission rules: `docs/SCENARIO_CATALOG_ARCHITECTURE.md`.

Case IDs below are `docs/CASE_LIBRARY_ROADMAP.md` Tier 7 IDs (`T7.n`) — that doc holds the
rationale and references; this list holds only the action and its blocker.

- [ ] **5.1 "Matched history, different reserves"** (N·c_t ambiguity) — withdrawn from the active
  catalog on 2026-07-30. Do not restore the scenario until a purpose-built
  reserves/identifiability result view can show OOIP, c_t, N·c_t, cumulative oil, recovery factor,
  and remaining/abandonment reserves side by side. Re-entry also requires one product-consistent
  OOIP definition and simulator-history tests that quantify pressure/rate/cumulative mismatch on
  linear and logarithmic plots. Full rationale and admission gate:
  `docs/CASE_LIBRARY_ROADMAP.md` Tier 5.1.
- [x] **5.2 kv/kh × density-contrast experiment** — implemented and measured, then withdrawn from
  the active catalog on 2026-07-28 because the product had no tornado/interaction-specific plot.
  Re-admission requires a distinct interaction workflow; the second candidate pair (capillary ×
  layer contrast) was never built → T7.16.
- [x] **5.3 "Two fluid models, one calibration point"** (correlation vs lab-report PVT) — shipped as
  `dep_pvt.ts`. Still no OPM deck for this case (below).
- [x] **E7 pre-run scenario class** — shipped as `capabilities.runMode: 'prerun-artifacts'`
  (`scenarios.ts`) and retained for future real multi-run exhibits. Its plumbing demonstrator
  `wf_bl1d_opm` was removed from the user catalog on 2026-07-28 because a reference source alone
  does not justify a second scenario. The multi-artifact fan/ensemble half is E8 and remains open.
- [x] **E5 history/forecast divider** — shipped (`resolveHistoryDivider`), including log-time
  mapping as of 2026-07-28.

Open, no engine gap (cheapest):
- [x] **T7.4 capillary waterflood — DONE 2026-07-24.** `scenarios/wf_capillary.ts` is the first
  scenario in the catalog with `capillaryEnabled: true`, closing the "shipped physics no user can
  see" gap. Entry-pressure ladder against a fixed BL overlay + a "physics or truncation error?"
  dimension. Key measurement: at `wf_bl1d`'s 400 bar drawdown an 8 bar entry pressure moves front
  width by only ~3 %; at a representative 40 bar drawdown the ladder spans 0.081 → 0.289 PVI of
  front width. Claims guarded by `wf_capillary.test.ts`. Full table in `CASE_LIBRARY_ROADMAP.md`
  Tier 7 delivery record.
- [ ] **T7.4 remainder: gravity-capillary transition zone** (hydrostatic P_c = Δρgh profile,
  Leverett J). **Display blocker cleared 2026-07-25**: `visualization/SpatialProfileChart.svelte`
  profiles any grid property along K against true depth (per-layer `cellDzPerLayer`, not layer
  index), which is the saturation-vs-depth view this case needed. What remains is the case itself —
  gravity-capillary equilibrium initialisation and the Leverett J scaling — plus deciding whether
  the analytical P_c(h) curve is drawn as a profile overlay (the chart currently overlays only the
  BL flood front, along I).
- [ ] **T7.11 grid-orientation effect — ATTEMPTED AND REFUTED 2026-07-24; reclassified.** A
  single-well-pair construction (same grid, wells moved edge-to-edge vs corner-to-corner, crossed
  with mobility ratio) was built, measured and discarded. Comparing at equal days is invalid under
  BHP control (36 % spread from throughput alone); balanced rate control fixes that but is too slow
  to ship; and comparing at equal PVI controlled for breakthrough gave a *larger* gap at the
  favorable mobility ratio (35 %) than the adverse one (21 %) — the opposite of the grid-orientation
  signature. Moving one well pair changes pattern geometry far more than grid alignment. **Needs
  E11 (multi-well patterns) for the real Yanosik-McCracken construction; do not retry with a single
  well pair.** Full record in `CASE_LIBRARY_ROADMAP.md` Tier 7.
- [ ] **T7.12 numerical vs physical dispersion** framing on the existing `wf_bl1d` grid ladder.
- [x] **T7.13 IMPES-vs-FIM formulation comparison — DONE 2026-07-28.** Removed the generic injected
  solver sensitivity from all catalog cases and added `solver_fim_impes` under Other. The dedicated
  24-cell rate-controlled waterflood uses 5-day report steps so the formulation is visible: the
  configuration probe ended at 100 days near 315.4 bar / 0.429 oil rate for FIM versus 299.9 bar /
  0.322 for IMPES, with no solver warning. This is labelled a numerical exhibit, not an analytical
  correctness oracle.
- [ ] **T7.14 joint relperm-endpoint uncertainty**, **T7.15 pattern density**.
- [x] **T7.18 endpoints × V_DP — DONE 2026-07-24.** `endpoints_vs_geology` dimension on
  `sweep_vertical` + `sweep_vertical.test.ts`. Drafted as an amplification case by analogy with
  `wf_tornado`, then rewritten when measurement showed the opposite: the two mechanisms *mask* each
  other (individual penalties sum to -0.4935 of mobile oil, combined is -0.3684, measured at
  0.625 PVI). The library's two interaction cases now deliberately have opposite sign.
- [ ] **T7.5 Koval correction** — honest reference for the high-M `wf_bl1d` rungs.

Open, needs a new analytical module:
- [x] **T7.1 well test — DONE 2026-07-24 (E10 closed).** `src/lib/analytical/wellTest.ts` + 37 tests
  (E1 verified to 1e-12 relative against independently computed 40-digit values; both inverse
  problems verified by round trip over skin ∈ [-3, 12]; all constants derived from
  `DARCY_METRIC_FACTOR`), plus full wiring: `'well-test'` as a first-class `AnalyticalMethod`, param
  adapters, `buildWellTestReference`, a `buildChartData` branch, a `well_test` log-time layout, and
  the `dep_welltest` scenario. Not piggybacked on `depletion` — see ROADMAP Priority 2.1.
  Measured (`dep_welltest.test.ts`): interpreting the simulated drawdown returns k = 8.674 / 9.118 /
  9.209 mD at 40 / 20 / 10 m cells against a true 10 mD, and apparent skin −1.00 / −0.67 / −0.60
  against a true 0 — a real near-well grid bias, converging with refinement, documented in the
  scenario rather than tuned away. Recovered k is identical to three decimals across the skin
  ladder, so the slope/offset separation holds exactly.
- [ ] **T7.2 dry-gas p/z + gas-cap blowdown** — extends `materialBalance.ts`, which already carries
  `m`/`driveIndex_gasCap` with no scenario exercising them. Water-drive variant needs E9.
  Now the cheapest remaining analytical case, since T7.1 closed.

Open, blocked on an enabler:
- [ ] **E1 single-run field perm** (see Wave 4 follow-ups) → unblocks **T7.9 Tavassoli**,
  **T7.6/T7.7 SPE10**, Egg.
- [ ] **E2: declarative time-based well schedule** in scenario params, worker-driven (wasm APIs exist;
  `sim.worker.ts` applies schedules only at create) → unblocks WAG (5.5) and **T7.8 SPE9**.
- [ ] **E8: ensemble / fan-curve chart primitive** (P10/P50/P90 band) → **T7.19**, and therefore all
  combined-uncertainty cases plus Tier 6.1/6.6. Lands in `buildChartData.ts` as a new sequential
  section (permitted by the frontend-architecture skill — it forbids inlining analytical-method
  physics, not adding a new concern) plus band-fill support in `ChartSubPanel` and the curve types.
  Not started 2026-07-24: it is a large, visually-verified change, and the chart-consolidation item
  above should be scoped first so the band does not land on the older of two coexisting paths.
- [ ] **E9: aquifer boundary model** → T7.2 water-drive, **T7.3**, T7.17, live PUNQ-S3 (5.6).
- [ ] **E11: multi-well patterns** — drive the worker's existing `payload.wells` array from scenario
  params (no scenario populates it today) → T7.11 done properly, SPE9, pattern density (T7.15).
- [ ] **Engine gaps deferred:** relperm hysteresis (E4, WAG), per-well injected fluid (E3), inactive
  cells (E6, blocks live PUNQ-S3).
- [ ] **Tier-6 pre-run exhibits** (E7 done; need data curation only, except where noted): 6.5 SPE11
  inter-simulator spread (no simulation at all — cheapest), 6.3/6.4 SPE5 WAG + hysteresis;
  6.1 PUNQ-S3 ensemble and 6.6 Egg additionally need E8; pilot `flowexp_comp` compositional for 6.2.
  Record dataset licenses/provenance before bundling artifacts.
- [ ] **OPM decks still missing** for `dep_pvt` (5.3), `gas_injection`, and `gas_drive`. The summary
  parser itself is done and both committed artifacts are `status: "parsed"`.

## Priority 2 — Validation & correctness
- [x] **(MAJOR) `scripts/validate-solver-coverage.sh` could report success without running a gate.**
  Closed 2026-07-24 — **the originally-reported root cause did not reproduce.** The script has had
  `set -euo pipefail` since `dce20c1`, and an injected `E0308` in a `#[cfg(test)]` fixture already
  made it exit 101, so "exits 0 when the crate fails to compile" is not a property of the committed
  script (most likely the 2026-07-24 observation came from invoking it through a pipe, where the
  observed status is the last command's, not the script's). The *real* silent-pass hole, confirmed
  by measurement: `cargo test <filter>` exits **0 when the filter matches nothing**
  (`0 passed; ... 510 filtered out`), so any renamed / deleted / `cfg`-ed-out test would turn its
  gate line into a no-op that still reported success. Fixed by (a) building the test target once up
  front so a compile break is reported as a build failure before any bucket runs, and (b) requiring
  every filter to prove it executed ≥ 1 test, parsed from the `test result:` lines, with a
  `gate ok: '<filter>' ran N test(s)` line per gate. Re-verified against all three injected failure
  modes (zero-match filter → exit 1; compile error → exit 1; failing assertion → exit 101) and on
  the clean tree: `bash scripts/validate-solver-coverage.sh all` → exit 0, 24/24 filters live,
  32 tests, 2m22s. No filter was already dead.
- [x] **Black-oil validation gates closed (2026-07-24, ROADMAP 1.1).** Quantitative SPE1 acceptance
  criteria (`src/lib/ressim/src/tests/spe1_acceptance.rs`) vs the `flow 2026.04` SPE1CASE1 reference:
  pressure 3 % / oil rate 8 % / GOR 12 % / plateau 0.5 % / MB drift 1 %; worst measured on `0cfead9`
  1.73 % / 3.33 % / 4.39 %. Grid-convergence checks for pressure, Rs, Bo and liberated gas
  (`.../tests/physics/depletion_grid_convergence.rs`). Safeguards documented for users in
  `docs/BLACK_OIL_VALIDATION.md`. Fast gates wired into `scripts/validate-solver-coverage.sh`
  (`fim` and `impes` buckets); the long replays are `--ignored --release`.
- [ ] **FIM and IMPES disagree on the black-oil depletion column** (~10 % on average liberated gas,
  0.6 bar on average pressure; `docs/BLACK_OIL_VALIDATION.md` section 2). Each converges cleanly under
  grid refinement, so this is a solver/timestep question. Dev-only priority, but it is the one
  black-oil result that two shipped paths do not agree on.
- [x] **`docs/DOCUMENTATION_INDEX.md` still says "FIM is dev-only; public scenarios ship IMPES."**
  Gas/three-phase scenarios (incl. `spe1_gas_injection`) have defaulted to FIM since `b88ee28`.
  Reconcile the doc with the shipped solver policy.
  Done 2026-07-24: rewrote the "FIM — current truth" preamble in `DOCUMENTATION_INDEX.md`, replaced
  the "Product Boundary" section of `FIM_DEFERRED_BACKLOG.md` with the then-shipped solver policy, and updated the
  `FIM_STATUS.md` current-state line. Historical mentions in `FIM_CONVERGENCE_WORKLOG.md`, the
  experiment registry, and dated review docs were left as provenance.
- [x] **Define three-phase `experimental` exit criteria** + acceptance tests for gas-injection and
  gas-drive (breakthrough timing, Sg evolution, phase-closure diagnostics).
  Done 2026-07-25: five exit criteria defined and met in `docs/THREE_PHASE_VALIDATION.md`;
  `gas_drive` upgraded to real black-oil and graded against a new OPM Flow reference deck;
  gas-front and per-phase-closure gates added in `three_phase_acceptance.rs` and wired into the
  `fim` bucket. `experimental` removed from README, implementation notes, case-library roadmap
  and the `gas_drive` scenario copy.
- [x] **Reconcile three-phase docs with implemented state:** explicit gas MB reporting,
  oil-phase diagnostic limits.
  Done 2026-07-25: `material_balance_error_oil_m3` is a *direct* diagnostic, not a residual —
  the old "oil is residual" wording was wrong and is corrected in
  `docs/THREE_PHASE_VALIDATION.md` §4, the implementation notes and README. Gas-oil capillary
  sign and `s_org` wording were already consistent with `capillary.rs` / `relperm.rs`.
- [ ] **Explain the +4 % cumulative-oil bias vs OPM Flow on `gas_drive`.** Inside the 8 %
  acceptance band and monotone (+1.1 % at 10 d to +4.3 % at 600 d) while pressure and GOR
  agreement *improve* with time, so it looks like an early-time displacement-efficiency
  difference locked into the integral rather than an accumulating drift. Replay:
  `three_phase_acceptance_error_replay`.
- [ ] **`gas_injection` has no OPM Flow reference of its own.** Covered indirectly by SPE1 (same
  mechanism, graded) and by the gas-front criteria, but a dedicated deck would close the last
  three-phase scenario without a direct numerical reference. The pipeline now has a working
  three-phase deck template (`GAS_DRIVE` in `tools/opm_flow/opm_flow_tool/cases.py`).
- [ ] **SPE1:** add regression tests for scenario wiring / published-reference panel placement /
  `cellDzPerLayer` + per-layer completion payload; re-verify the comparison source/metric mapping
  (Case 1 vs 2, avg vs field pressure, producing GOR). Rate-target tuning is done — the engine is
  within 3.3 % on oil rate and 4.4 % on GOR at 10×10×3 (`docs/BLACK_OIL_VALIDATION.md` §1).
- [ ] **SPE1 breakthrough sharpens with areal refinement.** Measured 2026-07-24: at 20×20×3 the
  producing-GOR error peaks at 32.8 % at 730 d and oil rate at 6.6 % at 1095 d, while *late*-time
  agreement is better than the coarse grid (0.12 % pressure, 1.0 % GOR at 3650 d). So the older
  "finer grid moves away from reference" note is really a breakthrough-timing/front-sharpness effect,
  not a whole-run degradation; material balance closes on both grids. Well/transport-model question.
  Replay: `spe1_areal_refinement_reference_error_replay`.
- [ ] **Revisit the ignored Buckley-Leverett refined-grid regression** as a potential solver/timestep
  issue, not just a slow-test classification.
- [ ] **Comparison-model tests:** preview mode, depletion per-variant analytical overlays, color-index
  stability.
- [ ] **Chart x-axis endpoints** (cumulative/time modes): prepend zero anchors, snap shared range/ticks
  to round values (no `70.00000000006`-style residues).
- [x] **Analytical-method integrity (ROADMAP 2.1) — DONE 2026-07-25, all 4 steps.** The
  `analyticalMethodRegistry.ts` routing table; `'sweep'` as a first-class analytical method (which
  deleted the suppress-then-strip machinery); `ScenarioCapabilities` as a discriminated union
  derived from `ANALYTICAL_OUTPUT_CONTRACTS`; and one declared `referenceSources` list replacing the
  three reference-source mechanisms. Full record in `ROADMAP.md` §2.1.
- [x] **Sweep-method framework (ROADMAP 2.2) — DONE 2026-07-25.** `sweepMethods` on the sweep
  capabilities arm + `analytical/sweepMethods.ts` for the prose; `Scenario.analyticalOptions`
  deleted; `sweep_vertical` gains the toggle, `sweep_areal` deliberately does not (the two
  correlations are bit-identical at areal geometry — measured, pinned by test); `sweep_areal`
  documented as a quarter five-spot with symmetry-plane boundaries. Record in `ROADMAP.md` §2.2.
- [x] **`SwProfileChart` decision — RESOLVED 2026-07-25: rebuilt in the 3D group.** It was dormant
  because it was mis-filed: every run-results chart shows one number per report step across a run,
  while this shows one number per cell at one instant, so it had no timestep to follow. Replaced by
  `visualization/SpatialProfileChart.svelte` + the pure `spatialProfileModel.ts`, mounted under the
  3D view, reading the 3D view's selected snapshot and property selector. Now covers pressure and
  all three saturations (ternary draws all three), profiles along I/J/K against distance in metres,
  and sources its BL front overlay from `analytical/fractionalFlow.ts` instead of a private copy.
- [ ] **Spatial profile follow-ups.** (a) The BL flood-front overlay is the only reference curve it
  draws, and only along I for water saturation — a depth profile has no analytical overlay yet
  (see T7.4). (b) The profile line selection is component-local state; it resets on remount, and a
  scenario may want to declare a default line rather than inheriting `producerJ` / top layer.
- [ ] **Analytical slot-context asymmetries, preserved not endorsed.** Building the method registry
  surfaced two reference curves that appear in some overlay contexts but not others, with no stated
  reason. Both were kept exactly as they were so the registry commit stayed a consolidation, and both
  are pinned by `analyticalMethodRegistry.test.ts` so a change has to be deliberate:
  - `gas-oil-bl` draws a `cum-oil-reference` in the `shared` context only — never per-result, pending
    or preview — while `depletion` draws its cumulative in all four.
  - `well-test` draws its `oil-rate-reference` per-result and in preview but not for still-pending
    variants, while its `producer-bhp-reference` is drawn in all three.
  - `buckley-leverett` computes a cumulative-oil reference in `buildBuckleyLeverettReference()` that
    no context ever draws. The `waterflood` layout asked for `cum-oil-reference` and got nothing;
    that dead key was removed 2026-07-25 when the positive layout validator landed, but the
    unreachable computation is still there.
  Decide whether each is a deliberate teaching choice or an oversight, then widen or document.

## Priority 3 — FIM solver (dev-only, parked maintenance track)

FIM is out of the user path (IMPES ships). Do not chase small deltas; big OPM-architecture gaps
matter more. Search `docs/FIM_EXPERIMENT_REGISTRY.md` by mechanism before any change.

- [x] **Promote corrected flexible GMRES recurrence (2026-07-24).** The shipped oil pressure-
  depletion FIM case exposed false convergence in the historical fixed-left recurrence with
  input-dependent CPR: report step 24 fragmented into 543 accepts/398 linear retries. Correct
  right-preconditioned FGMRES gives one substep/zero retries across all 160 report steps; the old
  path remains explicit diagnostic A/B only. This is not a claim of Flow linear-stack parity.

- [x] **2 hotspot-cooldown timestep tests — FIXED 2026-07-24 (stale tests, not a bug).**
  `changing_hotspot_resets_extra_growth_cooldown_budget` and
  `repeated_same_hotspot_extends_growth_cooldown_budget` asserted a pre-`89065164` (2026-04-08)
  clean-success budget of `2`; that commit deliberately made a repeated same-site hotspot *extend*
  the budget (`extra_clean_successes_for_repeated_hotspot`, `2..=3 => 1`) and added a sibling test
  endorsing it, but left these two un-updated. Corrected the two expectations `2 → 3`.
- [ ] **`legacy_resv_failed_direct_fallback_...` — DIAGNOSED + disabled 2026-07-24 (stale fixture,
  not a bug); revive later.** Verified the runtime: the direct RESV solve now returns
  `converged=true` / finite / `used_fallback=false`, the timestep still reports `!converged`, and
  `accepted_state` stays finite — the safety guarantee holds; physics/assembly drift since 2026-07-19
  (WATER-019..028 + singular-Jacobian handling) just made the `scoped_resv_sim` system solve finitely,
  so the fixture no longer reaches the non-finite branch. The reject→fallback mechanism is covered
  deterministically by `fim::linear::mod::tests::failed_forced_direct_solve_falls_back_once_and_reports_fallback`.
  Marked `#[ignore]` with a full comment. To revive the timestep-level orchestration check, build a
  fixture that deterministically forces a non-finite correction (or inject one) instead of relying on
  a physical case staying singular.
- [x] **Relperm-endpoint singularity routing — DONE 2026-07-28.** The user-visible
  `wf_capillary` FIM comparison made the formerly low-priority backstop pathological: its first
  0.25-day step took 15 substeps / 12 linear retries / 3.66 s. The forced dense solve correctly
  rejected the endpoint-singular Jacobian, but fallback routing then discarded requested CPR and
  used plain GMRES/ILU0. Preserving `FgmresCpr` on fallback gives 1 substep / 0 retries / 0.61 s;
  the full 500-day run completes with one substep per outer step. SWOF/relperm physics is unchanged.
  Exact replay: `node scripts/fim-wasm-diagnostic.mjs --preset wf-capillary --diagnostic summary
  --no-json`. Experiment `FIM-LINEAR-014`; analysis updated in
  `docs/FIM_RELPERM_ENDPOINT_SINGULARITY_ANALYSIS.md`.
- [ ] **ResSim over-predicts oil ~8–10% vs Flow** on the quarter-day controls — consistent across
  grids, points at a systematic property/well difference, not the solver. Needs a proper cumulative
  (`FOPT`) comparison before attribution.
- [ ] **Newton production-seam refactor** (bounded): extract damping/chop + convergence/acceptance
  diagnostics while keeping `run_fim_timestep()` as orchestration. Do not alter solver behavior or
  combine with physics work.
- [ ] **Bundle Y OPM parity (paused, low priority):** G1 heavy-case raw-Newton oscillation (Y1c),
  Y2b active-bound AD derivative scope, then a G4/G5 structural bundle → controller parity → stack
  promotion. AMG ("Bundle C") and variable substitution stay deferred (scale-up items). Owned by
  `docs/FIM_STATUS.md` + `docs/FIM_OPM_PARITY_PLAN.md`; only pursue if the re-baseline priorities shift.

## Reference notes to keep
- `sweep_ladder` intentionally shares analytical overlays despite the patched viscosity — teaching
  choice, not a bug.
- Three-phase IMPES accumulation uses `get_c_o_effective()` (includes dissolved-gas compressibility
  `(Bg/Bo)·dRs/dp`, dominant below bubble point); two-phase mode still uses `get_c_o()`.
- Water and gas cumulative MB errors are reported explicitly in three-phase mode; oil is the residual
  phase in diagnostics.

## Housekeeping
- [x] **Scenario picker density and hierarchy — DONE 2026-07-28.** Scenario families now render
  as compact wrapping group cards, and the input surface has explicit
  Scenario Selection, Scenario Description, and Sensitivity Selections sections. Fixed-reference
  and analytical-applicability notices are omitted from the picker; actionable validation and
  runtime warning policy remains a separate channel so current-input problems are not mistaken for
  catalog prose. Solver badges are also omitted from catalog buttons and retained in the description.
- [x] **No parity test between the tabulated-relperm value and derivative paths.** Found during the
  2026-07-24 dead-code review. `RockFluidProps::corey_table_derivatives` (analytic segment slopes)
  and `corey_table_generic` (what production differentiates via AD) are two independent
  implementations of the same piecewise-linear law, and nothing asserted they agree.
  Closed 2026-07-24 by `relperm::endpoint_derivative_tests::corey_table_derivatives_match_ad_derivative_of_the_table`:
  it compares the analytic slopes against `Ad<1>` duals of `corey_table_generic` over two Corey
  parameter sets, `points ∈ {2, 3, 5, 21, 101}`, and saturations covering both clamped tails, the
  knots themselves and mid-segment points, plus a value-path check that the AD instantiation agrees
  with `corey_table`. Verified to fail on an injected 1e-4 relative perturbation of the analytic
  slope.
- [ ] **The analytic well-sensitivity family in `fim/wells.rs` is entirely `#[cfg(test)]`.**
  `local_phase_sensitivity` and its ~600 lines of dependent analytic well/perforation blocks exist
  only as the oracle the AD well blocks are checked against (production has been on `assembly_ad`
  since the Phase 5 cutover). That is a legitimate role, but it is undocumented at the module level
  and the code reads as if it were live. Worth a module-level comment saying so; not worth deleting
  while it is the only independent check on the AD well Jacobian.
- [ ] **`.claude/settings.json` allowlist is stale** — `/home/reken/...` absolute paths and one-off
  experiment commands from old sessions. Needs a manual prune (agent-initiated permission edits are
  blocked by policy — user action).
