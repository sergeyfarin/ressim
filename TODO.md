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

- [ ] **Wellbore hydrostatic datum missing on multi-layer completions (found 2026-08-01).** Every
  completion of a well is a separate `Well` row carrying the same `bhp` (`src/lib/ressim/src/well.rs`),
  and nothing corrects it for completion depth. With gravity enabled the reservoir pressure varies by
  ρ·g·H down the column (≈3 bar over 40 m), so a fully perforated BHP or rate-controlled well
  allocates flow towards the top of the section as a pure modelling artefact. It is negligible against
  a 300 bar drawdown and dominant in exactly the gravity-dominated regime where the drawdown is a
  bar or two. `wf_gravity` works around it by perforating one layer per well. The fix is an
  Eclipse-style datum depth per well plus a wellbore density, applied as
  `p_completion = p_bhp + ρ_wb·g·(depth_k − depth_datum)`; it only bites when `gravityEnabled`, so
  existing gravity-free scenarios and benchmarks cannot move.
- [ ] **IMPES reports an unphysical result with no warning when the saturation limiter is relaxed
  (found 2026-08-01).** `wf_numerics`'s `time_truncation` dimension ships this deliberately as
  teaching content, but it is also a product gap: with `max_sat_change_per_step` at 1.0 the 1D
  column recovers 0.936 of the oil in place when only 0.889 of it is mobile, and
  `getLastSolverWarning()` returns `''`. The run completes and the chart looks plausible. A cheap
  guard would be a post-step check that no cell's water saturation left `[s_wc, 1 - s_or]` by more
  than a tolerance, raised as a solver warning rather than an abort — the scenario's
  `dt_limiter_off` variant is a ready-made regression case, and `wf_numerics.test.ts` currently pins
  the *absence* of the warning, so that assertion flips when this is fixed.
- [ ] **Rock compressibility releases gas at the wrong pressure (found 2026-08-01, engine).**
  `fim/assembly.rs::pore_volume_at_state` and `fim/properties.rs::pore_volume_generic` compute
  `pv_ref * exp(c_f * (p - p_prev))` where `p_prev` is the **previous timestep's** pressure, not a
  fixed reference. Compaction therefore never accumulates: within a step `p ~ p_prev` so the pore
  volume stays at `pv_ref`, while the accumulation derivative injects `c_f * pv` every step. The
  same compaction energy is harvested repeatedly, and each release is converted to surface volume at
  that step's `B_g` rather than at abandonment `B_g`, so the error grows with both `c_f` and the
  depletion range. Measured on `dep_gas_pz` (400 -> 30 bar, c_f 5e-6 -> 5e-4 /bar), cumulative gas:
  ResSim 131.6 -> 145.7e6 Sm3 (+10.7 %), OPM Flow on the identical deck 129.9 -> 132.4e6 (+1.9 %),
  hand dry-gas material balance 131.2 -> 133.6e6 (+1.8 %). OPM and the hand balance agree; ResSim's
  compaction increment is ~5.8x too large. The base rung agrees with both to 1.3 %, so only the
  compaction term is wrong. **Fix:** reference the exponent to a stored initial/reference pressure
  rather than `p_prev`. The Jacobian is unchanged — `d/dp [pv_ref*exp(c(p-const))] = c*pv` either
  way — so this is a value fix, not a structural one. Pinned by
  `dep_gas_pz.test.ts` > "does not yet agree on how much gas compaction releases", which is written
  to fail once the fix lands. Existing scenarios use c_f 1e-6, where `exp(c*dp) ~ 1`, so they cannot
  move measurably; still needs the FIM + IMPES validation ladder and a wasm rebuild.
- [x] **`dep_gas_pz` OPM Flow decks (2026-08-01).** The scenario's claims are guarded against
  its own analytical reference and its own inventory closure, but there is no second simulator on the
  deck. A dry-gas OPM case is straightforward (PVDG from the same table, single producer on a BHP
  floor) and would let the pore-compressibility ladder be cross-checked, since ROCK compressibility is
  a first-class Eclipse keyword — which is exactly what it did: see the defect above. Two decks
  (`dep_gas_pz`, `dep_gas_pz_geopressured`, flow 2026.04, GAS+WATER, PVDG generated from the
  scenario's own table, Eclipse `ROCK`). Required a new `cumulative_gas_curve` field on `OpmCase`
  and a `cumulativeGasSm3` entry in the artifact x-axis map, because a p/z chart's x-axis is
  produced gas and `mapReferenceTimesToXAxis` previously dropped every reference series on it.
  `dep_pvt`, `gas_injection` and `gas_drive` still have no deck.
- [x] **`dep_gas_pz` and the p/z analytical method (2026-08-01).** Closes roadmap T7.2's dry-gas
  half. New `src/lib/analytical/gasMaterialBalance.ts` + a `'gas-material-balance'` analytical method
  through the registry, and a scenario measuring what bends the straight line: pore compressibility
  (reserves error +0.8 % to +10.5 % as c_f goes 5e-6 to 5e-4 /bar), compartmentalisation (+38.6 %)
  and abandonment pressure (+9.0 %). Replay
  `pnpm vitest run src/lib/catalog/scenarios/dep_gas_pz.test.ts src/lib/analytical/gasMaterialBalance.test.ts`.
- [x] **`p_z` was not p/z (fixed 2026-08-01).** `buildDerivedRunSeries` hard-coded `z = 1`, so the
  curve labelled "P/z" on every three-phase diagnostics panel was the average reservoir pressure. It
  now inverts z from the run's own `B_g` table, returns null when the case has no gas PVT or no
  `reservoirTemperature`, and lives on its own `pz` panel under the `p-over-z-` key family.
- [x] **Numerical-dispersion and crossflow scenarios (2026-08-01).** Two new cases, closing
  roadmap T7.12, T7.13, T7.16 and the Tier 1 "D-P with vertical communication" row.
  `wf_numerics` (1D, 500 m, six grids from 50 m to 1.25 m cells) measures first-order convergence
  onto an exact BL reference — breakthrough error 0.131 → 0.001 PVI, halving with cell size — plus
  timestep/limiter, dispersion-vs-rock-curve and IMPES-vs-FIM dimensions, with new OPM Flow decks at
  two resolutions bundled as reference curves. `sweep_crossflow` (48×1×5 layered section) varies the
  one parameter Dykstra-Parsons and Stiles cannot see: breakthrough moves 40 % across the k_v/k_h
  ladder while the analytical curve is provably fixed, the crossflow benefit reverses sign between
  M = 0.5 and M = 10, and capillary crossflow is inert without a path and super-additive with one.
  Replay: `pnpm vitest run src/lib/catalog/scenarios/wf_numerics.test.ts
  src/lib/catalog/scenarios/sweep_crossflow.test.ts`.
- [x] **Gravity-override scenario `wf_gravity` (2026-08-01).** New
  `buckley-leverett-displacement` case: a 30×1×20 vertical section, single-perforation wells at the
  base, rate-controlled injector. Gravity-off control recovers 0.699 of oil in place at 1 PVI against
  Buckley-Leverett's 0.715; gravity on at the base rate gives 0.586, and 0.383 in the
  gravity-dominated rung. Four dimensions (gravity number, density contrast, k_z × gravity,
  producer completion × gravity), all measured and guarded in `wf_gravity.test.ts`. Replay:
  `pnpm vitest run src/lib/catalog/scenarios/wf_gravity.test.ts`. Closes the gravity half of the
  "which Buckley-Leverett assumption breaks" family started by `wf_capillary`.
- [x] **Completion layers reach the live worker (2026-08-01).** `producerKLayers` / `injectorKLayers`
  were honoured by `buildCreatePayload` and the worker but never carried by `ParameterStore`, so only
  the benchmark-preset path (SPE1) could perforate a subset of layers; every live scenario perforated
  every layer. Added the two fields with a dedicated `parseCompletionLayers` (the existing
  `parseLayerValues` drops non-positive entries and would have deleted layer 0), wired through
  `applyParamValues`, `buildCorePayload`, `buildModelResetKey` and the parameter snapshot. Empty
  array keeps the previous all-layers behaviour.
- [x] **OPM Flow ground truth for `wf_gravity` (2026-08-01).** Deck added to
  `tools/opm_flow/opm_flow_tool/cases.py` reproducing the base case cell for cell (30x1x20, isotropic
  5 D, single-connection wells, 160 m3/day WCONINJE RATE, 210 x 2-day TSTEP). Run with flow 2026.04:
  OPM breaks through at 0.227 PVI and recovers 0.583 at 1 PVI against ResSim's 0.253 / 0.585 — 0.4 %
  apart on recovery, 12 % on breakthrough, both 18 % below Buckley-Leverett. Guarded by
  `wf_gravity.test.ts`. This is what makes the scenario's central claim checkable: the departure from
  BL is reproduced by an independent industrial simulator.
- [x] **Reference series render on injection-based x-axes (2026-08-01).** `PublishedReferenceSeries.data.x`
  is time in days and `appendPublishedReferenceSeries` passed it through unchanged, so an OPM overlay
  landed at the wrong x on a `pvi` or `cumInjection` axis — `wf_gravity` worked around it with a
  time-axis dimension override and a hidden-by-default reference. Fixed by having the artifact carry
  the reference run's own mapping: `tools/opm_flow` emits `xAxis: {timeDays, pvi,
  cumulativeInjectionM3, poreVolumeM3, cumulativeInjectionCurve}` from a declared cumulative
  reservoir-volume vector (FVIT) and the deck's pore volume, `resolveScenarioReferenceSeries` attaches
  it per series, and the new `mapReferenceTimesToXAxis` in `axisAdapters.ts` converts. Axes the
  reference run cannot pin (`tD`, `pvp`, `cumLiquid`, `cumGas`, and any injection axis for a series
  with no mapping) now **drop** the curve instead of misplacing it — a reference at the wrong x reads
  as disagreement with ground truth. The `wf_gravity` workaround is removed and its OPM curves are
  visible by default on the scenario's own PVI axis.

- [x] **Vertical-displacement scenario `wf_gravity_stability` (2026-08-01).** 1D 60-cell column,
  single perforations at each end, flooded upward (gravity-stable) or downward (unstable) at the same
  rate. Measured recovery at 1 PVI: 0.706 gravity off (BL 0.715), 0.792 upward, 0.540 downward; the
  rate ladder fans 0.737/0.792/0.839 up against 0.673/0.540/0.354 down as G goes 0.33 -> 3.3. Grid
  refinement moves the gravity answers by <0.03 while the gravity-free control converges onto BL, so
  the departure is a missing term and not truncation error. Replay:
  `pnpm vitest run src/lib/catalog/scenarios/wf_gravity_stability.test.ts`.
- [ ] **Gas-oil gravity case probed and parked (2026-08-01).** First-pass FIM probe of crestal vs
  basal gas injection in the same 30x1x20 section (mu_g 0.02, rho_g 200, q 160): gravity off / on with
  a basal injector gave RF 0.296 / 0.346 and crestal-with-gravity 0.336, with gas breakthrough at
  0.06-0.10 PVI in every case. The ordering is defensible (segregation away from a basal producer
  reduces gas cycling) but the rungs are close together and breakthrough is too early to read, so
  there is no crisp exhibit yet; each FIM run also costs 15-23 s headless. Needs a rate/density/
  completion campaign before it is worth a scenario — do not ship it on the numbers above.
- [x] **Chart line-style policy consolidated to three tiers (2026-08-01).** `curveStylePolicy.ts` is
  now the only module that writes a dash array: ResSim solid, analytical dashed `[7,4]`, every
  additional reference (published data or another simulator) dotted `[1,3]`, with runs inside a tier
  told apart by colour alone. Removes the per-metric sweep dashes and the `[4,4]`/`[2,4]` overlays
  that had accumulated across builders, and drops the solid-thin `reference-simulation` style — solid
  now means ResSim and nothing else. Guarded by `no-literal-border-dash.test.ts`, which fails on a
  literal `borderDash` outside the policy module. Average water saturation moved out of the watercut
  panel into its own `avg_water_sat` panel (one property per plot). Dead code removed:
  `SweepEfficiencyChart.svelte` and `buildRateChartData.ts`, with the scale configs still in use
  extracted to `scalePresetRegistry.ts`.
- [x] **Spatial-profile BL overlay only existed along I (2026-08-01).** `buildFloodFrontOverlay`
  returned null for any axis but `i` and computed its flow length from `nx * cellDx`, so
  `wf_gravity_stability` — whose flood runs down a 1 x 1 x 60 column — drew a simulated saturation
  profile with no analytical curve beside it, while the horizontal 1D cases drew both. The
  construction is now evaluated along whichever axis the wells are separated on, with the flow length
  and cross-section taken from that axis, and mirrored when the injector sits at the far end (the
  column is flooded upward from its base). Two guards came with it: the overlay is drawn *only* on
  the displacement axis — on `wf_gravity`'s section, where the flood runs along I, a BL curve down K
  would read as a prediction about the gravity tongue's vertical structure — and the per-scenario
  availability table is locked in `scenarioInputValidation.test.ts`. `wf_gravity` now opens its
  profile on I rather than K, so the reference is visible by default; at `nz > 1` the profile averages
  the column, which is the quantity BL predicts, and the tongue's vertical structure stays one
  dropdown away (axis K, or a single layer).
- [x] **Typography and description surfaces (2026-08-01).** Scenario descriptions rendered at 10px
  (`ui-microcopy`), which is a caption size, not a reading size, inside a block that also mixed a
  monospaced summary, three label weights, a chip and an info-tinted surface. Added `.ui-body-copy`
  (13px/1.6, foreground) for prose the reader is meant to read, raised `.ui-microcopy` 10 -> 11px and
  `.ui-chip` 10 -> 11px, and rebuilt the scenario block as one description paragraph plus a uniform
  Setup / Solver / Reference list. The sensitivity dimension's own description is now shown under the
  dimension selector — it was previously reachable only as a `title` tooltip on the variant chips, so
  each study's teaching point was invisible. Dense numeric tables and unit labels stay at 10px
  deliberately. Header title and subtitle rewritten to say what the app is and how to drive it, and
  `README.md` reordered to lead with capabilities and usage before internals.
- [x] **Vertical-column scenarios were blocked by the well-overlap rule (2026-08-01).**
  `wf_gravity_stability` is a 1 x 1 x 60 column, so both wells necessarily sit in the one column the
  grid has, and `validateInputs` refused to run it: "Injector and producer cannot share the same i/j
  location". The rule now compares *cells*, not columns — a shared i/j is an error only when the
  perforated layers also intersect (an absent completion list still means every layer, so areal cases
  behave exactly as before). `producerKLayers`/`injectorKLayers` are carried into
  `buildValidationInput` for that. Guarded catalog-wide by
  `src/lib/catalog/scenarioInputValidation.test.ts`, which pushes every scenario *and variant*
  through the product's own input gate and its well-payload builder — the scenario physics tests
  drive the wasm core directly and so could never have caught this.
- [x] **Analytical caveats were measured from empty builder toggles (2026-08-01).**
  `evaluateAnalyticalStatus` read `toggles.geo`/`toggles.well`, which the Scenario Builder sets and
  whose dimension catalog ships empty (`caseCatalog.ts`, `dimensions: []`). For every predefined
  scenario `toggles.geo` was `undefined`, so *all* waterflood cases carried "Geometry is not 1D" and
  "Wells are not end-to-end" — including `wf_bl1d`, a 96 x 1 x 1 slab with wells at cell 0 and cell 95 —
  and the depletion cases carried a well-position caveat even when centred. New `resolveGeometryFacts`
  measures 1D-ness, end-to-end placement and centring from the grid and completions, falling back to
  the toggles when a caller has no geometry (builder mode). The resulting per-scenario caveat table is
  locked in `scenarioInputValidation.test.ts`: `wf_bl1d`/`gas_injection`/`dep_decline`/`dep_pss`/
  `dep_welltest` now raise none, `wf_gravity` correctly raises not-1D plus gravity, and
  `wf_gravity_stability` raises gravity alone.
- [x] **Catalog taxonomy: the BL group is 1D in fact, not just in prose (2026-08-01).** `wf_gravity`
  shipped into `buckley-leverett-displacement` although its 30x1x20 section is two-dimensional, while
  the group's own description promised one-dimensional displacement. Group relabelled
  "1D Displacement — Buckley–Leverett" and its membership rule stated: a single flow path, so
  displacement efficiency is the whole answer and the fractional-flow solution can be judged
  pointwise. `wf_gravity` moved to `sweep-efficiency` — it measures contact, and its shortfall below
  BL *is* the vertical sweep term, the same quantity `sweep_vertical` measures for permeability
  contrast. Sweep group description widened to name gravity beside geometry and geology.
  `wf_gravity_stability` stays: a vertical column is still one flow path. The rule is now
  test-enforced (`scenarios.test.ts` checks every BL-group scenario and variant has at most one grid
  extent > 1), and the README inventory's group column was rebuilt — it still carried the
  "Gas-Dominated Recovery" group deleted on 2026-07-31.
- [ ] **Gravity-modified fractional flow as an honest reference for `wf_gravity` (opened 2026-08-01).**
  The scenario currently shows the viscous-only BL curve and measures the departure. Dake ch. 10 gives
  the gravity term in f_w explicitly, and adding it to `src/lib/analytical/fractionalFlow.ts` would let
  the reference follow the simulation in the segregated-flow (not tongued) limit — turning a
  "here is the error" case into a "here is the corrected theory" case. Needs a second overlay slot or a
  scenario-selected variant of the BL adapter.

- [x] **Catalog taxonomy: group by what the run can be checked against (2026-07-31).** `Gas Injection`
  sat in `gas-black-oil` although it carries a real analytical solution — `gas-oil-bl`, the same
  Buckley–Leverett/Welge fractional-flow construction as `wf_bl1d`, differing only in phase pair and
  in defaulting to FIM. Moved to `buckley-leverett-displacement` (group description widened to name
  both water–oil and gas–oil). That left `gas_drive` and `dep_pvt` — the catalog's only two
  `analyticalMethod: 'none'` scenarios — as the whole of `gas-black-oil`, so the group was replaced
  by `simulation-only` / "Simulation Only — No Analytical Reference", which states the property that
  actually distinguishes them: the simulation is the only curve on the chart. `spe1_gas_injection`
  deliberately stays in `validation-benchmarks` — it has digitized Eclipse *and* OPM Flow references,
  so it is the opposite of reference-less.
- [x] **SPE1 `kz_ratio` sensitivity removed (2026-07-31).** SPE1 does not specify kz, so varying
  k_v/k_h changed the physical case rather than its discretization, while every variant was still
  drawn against the published Case 1 reference curves — comparing a different reservoir to SPE1's
  answer. The remaining `grid` and `delta_t` dimensions hold the deck fixed and refine only the
  numerics, which is what a benchmark sensitivity is for.
- [x] **`runModel.test.ts` solver-variant assertions were stale (2026-07-31).** The two
  `solver_formulation` cases still enumerated the pre-`9657eee` two-variant list (`'IMPES'`, `'FIM'`)
  after the 5-day rungs and relabelling landed, so `pnpm test` was red on the committed tree
  independently of the catalog work. Updated both to the four-variant list.

- [x] **Sweep-efficiency panels had no numerical curve because the runs never asked for the metrics
  (2026-07-31).** `sweepConfig` was attached only in `RuntimeStore.buildCreatePayload` (the
  interactive path). Sensitivity and comparison runs go through
  `buildCreatePayloadForRun` → `buildBenchmarkCreatePayload` → `buildCreatePayloadFromState`, none
  of which set it, so the engine computed no sweep metrics for them, `rateHistory[i].sweep` was
  undefined, and `appendSimulationSweepCurves` pushed nothing. Every E_A / E_V / E_vol panel drew
  its analytical curve alone — which read as "the numerical value cannot be derived" when in fact
  the engine computes it. Extracted `buildSweepConfig` into `buildCreatePayload.ts`, called from
  both paths, and pinned by a `runModel.test.ts` case that asserts every sweep scenario's run
  payload carries a geometry-matched config and every non-sweep scenario's does not.

- [x] **Sweep layout showed the recovery factor twice (2026-07-31).** The shared `sweep` layout
  ordered `sweep_rf` ("Recovery Factor — Sweep Analysis", numerical + analytical) at the top and the
  generic `recovery` panel ("Recovery Factor", numerical only) further down — the same quantity with
  the reference stripped out. The generic panel is now hidden for this layout. In `sweep_combined`
  the E_A and E_V panels are collapsed by default: at `both` geometry the engine deliberately
  reports no `e_a`/`e_v`, because one simulation cannot separate areal from vertical contact, so
  those two remain analytical-only decomposition views.

- [x] **FIM-vs-IMPES was a standalone scenario that could not name a winner (2026-07-31).**
  `solver_fim_impes` declared `analyticalMethod: 'none'` and stated outright that no curve was
  promoted as the oracle, so it could only show that the two formulations differ. Folded into
  `wf_bl1d` as the `solver_formulation` dimension (IMPES/FIM × 0.25-day/5-day steps), where the
  Buckley-Leverett reference is independent of both grid and timestep and the four runs are judged
  against it. Measured over the 50-day flood: 0.97% apart in cumulative oil at 0.25-day steps;
  coarsening to 5-day steps costs IMPES 8.8% of its own fine-step recovery against FIM's 3.1%, so
  the formulations end 7.4% apart. The scenario's own test was ported rather than dropped. The
  emptied `Other` catalog group was removed.

- [x] **Depletion catalog split one continuum across two groups (2026-07-31).** `dep_welltest` sat
  in `pressure-transient` while `dep_pss`/`dep_decline`/`dep_arps` sat in `depletion-decline`,
  although the four are one well's pressure history in sequence. Merged into
  `flow-regimes-decline` ("Flow Regimes & Decline"), ordered transient → pseudo-steady →
  boundary-dominated → layered, with each scenario renamed to state its subject and carry its
  analytical method in brackets: Transient Radial Flow (Theis), Drainage Geometry & Productivity
  (Dietz), Boundary-Dominated Decline (Fetkovich), Layered Depletion (Arps). The now-empty
  `pressure-transient` group was removed; re-add it when buildup/Horner cases land.

- [x] **App.svelte passed 17 undeclared props to ScenarioChart (2026-07-31).** `rateHistory`,
  `analyticalMeta`, `rockProps`, `fluidProps`, the sweep group and others were left over from an
  earlier ScenarioChart API and had no matching `$props()` entries, so every one was silently
  discarded — one of the two long-standing `svelte-check` errors. Removed; the surviving wiring
  passes `selectedOutputProfile` whole to `ThreeDViewCard`. `svelte-check` is now clean at
  0 errors / 0 warnings (was 2 errors).

- [x] **Chart.js custom-plugin options were untyped (2026-07-31).** `options.plugins.historyDivider`
  failed `svelte-check` because Chart.js keys plugin options by a registry that a locally declared
  plugin has no entry in. Added the library's own `declare module` augmentation rather than casting
  the options object, which would have silenced real errors elsewhere in it.

- [x] **Chart y-axis log scaling was chart-wide, not per panel (2026-07-31).** One `logScale`
  `$state` in `ReferenceComparisonChart` and `UniversalChart` was bound into every `ChartSubPanel`,
  so toggling log on one panel changed all of them — panels that carry different properties over
  different dynamic ranges. Now keyed per panel like `panelExpanded`, with a per-panel
  `logScale` layout field that falls back to the chart-level flag, so every existing layout keeps
  its prior default. First consumer: `dep_pss`'s C_A panel, where the shipped geometries span
  30.8828 to 0.2318 and a linear axis from zero collapses the four least productive onto the
  baseline.

- [x] **`dep_pss` grid-refinement dimension illustrated nothing (2026-07-31).** Inferred `C_A` moved
  from +3.3% to +2.6% across 7×7 → 35×35 on the fixed 420 m square: a 0.7% span presented as a
  convergence study. The residual bias is not discretisation at all but the pressure dependence of
  the fluid properties over a depleting run, so refinement cannot remove it. Dimension deleted and
  replaced with `well_position`, which moves `C_A` 30.8828 → 12.9851 → 4.5132 on one fixed square.

- [x] **Line styles harmonized across every scenario chart (2026-08-01).** A review of all chart
  builders found the three tiers drifting apart one call site at a time: `[3,3]` on a simulation
  sweep curve, `[2,3]` on the MBE OOIP ratio, `[2,4]` on average saturation, `[6,4]` on the spatial
  front marker, three per-metric sweep dashes, and OPM Flow styled solid by `applyCurveTypeStyle`
  but dashed by `buildChartData` — two conflicting treatments of one source. Root cause: the
  composite style objects in `curveStylePolicy.ts` were exported and never imported, so every call
  site respelled `borderWidth`/`borderDash` by hand. The policy is now three tiers and nothing else
  — ResSim solid, analytical dashed `[7,4]`, any additional reference source (published data or
  another simulator) dotted `[1,3]`, sensitivity variants separated by colour alone. Average water
  saturation moved out of the water-cut panel into its own, since sharing a plot was what forced a
  fourth style. `no-literal-border-dash.test.ts` now fails any dash array written outside the
  policy module. Deleted with it: `buildRateChartData.ts` (no importer since the UniversalChart
  migration; carried 14 more style decisions, its scale presets moved to `scalePresetRegistry.ts`)
  and `SweepEfficiencyChart.svelte` (imported by nothing, own three-dash scheme).

- [x] **Dietz `C_A` simulation curves were drawn with the auxiliary dot-dash (2026-07-31).** Against
  the project convention (simulation solid, analytical dashed, dotted reserved for genuinely
  auxiliary series) the `pss-shape-factor-sim` series used `AUXILIARY_DASH` at reduced width. Now
  solid at `simBorderWidth`, matching every other simulation curve.

- [x] **`dep_pss` had no time-varying or falsifiable exhibit (2026-07-31).** The case plotted the
  productivity index and the inferred Dietz `C_A` — algebraic inverses of one another, both
  constant in time by construction — while its only real transient, the approach to
  pseudo-steady state, was masked by `analyticalPssStartDays: 1` (five times the measured
  0.15-0.2 day onset). Two of three sensitivity dimensions were degenerate on those panels:
  `production_rate` moved the productivity index by 0.07% (PI = q/dp with dp proportional to q),
  and `skin` moved inferred `C_A` by 1.8% because the inversion divides skin back out. Rebuilt
  around an equal-area drainage-geometry sweep (`C_A` 30.8828 / 21.8369 / 5.379 / 4.5132), a
  drawdown panel plotted from t=0, and a run that ends at the producer's BHP floor with the
  analytical reference terminating there rather than extrapolating past its own premise.

- [x] **Dietz `C_A` table held one unreproducible entry (2026-07-31).** `CA_SQUARE_CORNER = 0.5598`
  described a well in a corner cell. A corner well reflects onto itself in its two adjacent
  no-flow walls, so only a quarter of its sandface is open; separately the Peaceman well index
  assumes an interior block with full radial inflow. A measured run gives an effective `C_A` of
  ~1.6e-3 and a productivity index 63% below what 0.5598 predicts. Removed and replaced with
  tabulated rectangle and quadrant-well entries, each checked against the engine.

- [x] **Scenario routing tests inferred the solver from phase count (2026-07-30).** The run-model
  contract now follows each scenario's authoritative `solverPolicy`, allowing the two-phase
  capillary case to deliberately use FIM while ordinary sensitivities retain that default.

- [x] **Simulation legends repeated the numerical solver (2026-07-30).** Run labels now use the
  scenario and declared sensitivity variant without appending FIM/IMPES. Solver names remain in
  the dedicated FIM-vs-IMPES sensitivity because they are the variant names there.

- [x] **Analytical sweep-method toggle discarded numerical results (2026-07-30).** Switching
  between Dykstra-Parsons and Stiles now preserves completed simulation runs and rebuilds only the
  analytical overlays. Scenario, sensitivity-dimension, and variant changes retain their existing
  numerical-result invalidation because those selections can change simulator inputs.

- [x] **Layered composite decline used five areal lumped cells (2026-07-30).** Replaced the
  `1×1×5` tank stack with a `9×9×5` centered-well reservoir at the same physical dimensions.
  The numerical model now resolves areal pressure propagation and boundary arrival; its analytical
  overlay is explicitly late-time Dietz/Fetkovich layer superposition. Crossflow cases now use
  dimensionless `kv/kh` scales rather than geometry-dependent trace permeability labels.

- [x] **Chart panels enforce one physical property (2026-07-30).** Split depletion pressure,
  MBE OOIP ratio, and drive indices into dedicated panels; split Dietz PSS productivity and
  inferred shape factor into separate panels and axes. Catalog validation now rejects unknown
  curve classifications and any scenario/sensitivity panel that mixes semantic properties.

- [x] **Depletion comparisons expose their actual analytical observables and limits (2026-07-30).**
  Corrected the Dietz inflow constant from `exp(2γ)` to the standard
  `exp(γ)=1.781`, added numerical PI/effective-C_A curves and a centered-square grid-convergence
  gate, and made grid resolution the default Dietz comparison. Fetkovich grid variants now hold
  physical well PI fixed with compensating Peaceman skin and scale timestep with cell size; the
  gate verifies fine grid/timestep convergence against one fixed finite-slab reference. Layered
  depletion now includes an intentional vertical-crossflow sensitivity that departs from the
  shared noncommunicating-layer solution, making its validity limit visible rather than implicit.

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
- [x] **Spatial BL overlay default-axis regression (2026-07-31).** Restored I as the default
  profile axis when available, so diagonal well layouts show the analytical Buckley–Leverett
  saturation profile by default again; the injector-to-producer path remains selectable.
- [x] **Areal/combined sweep diagonal analytical profile (2026-07-31).** Sweep scenarios now
  default to the injector→producer profile. Their spatial reference maps Craig contacted-area
  progression onto that diagonal and evaluates Buckley–Leverett displacement inside it; combined
  layered profiles use Stiles permeability-weighted local PVI and follow the selected layer or
  column-average presentation. The UI identifies this Craig + BL construction explicitly.
- [x] **Sweep default-axis state transition (2026-07-31).** Keyed the spatial-profile component
  by reference geometry so entering areal or combined sweep resets stale component-local I/J/K
  state to the diagonal default, while result and sensitivity changes within that geometry keep
  the user's current profile selection.
- [x] **Scenario-owned spatial-profile path semantics (2026-07-31).** Added explicit preferred
  axes and path labels to every scenario that exposes a diagonal/path control. Depletion now calls
  its coordinate path `Diagonal`; injection cases retain `Injector → producer`; sweep scenarios
  continue to default to that path. Grid-derived axis availability, layer selection, and parameter-
  derived endpoints remain shared geometry behavior rather than scenario-key branches.
- [x] **Well-Test drawdown presentation (2026-07-30).** Restored the scenario's intentionally
  absent 3D default because rate-controlled skin variants have the same reservoir pressure field;
  made flowing BHP the sole expanded and explicitly visible chart and demoted constant oil rate to
  a collapsed control check. Removed the visually coincident grid-resolution sensitivity, retained
  the 2.4-day tail so closed-boundary arrival is visible, and fixed IMPES to publish rate-controlled
  BHP from the accepted end-of-step pressure instead of the beginning-of-step control state.
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
- [ ] **Drive-index curves ignore the case colour, so sensitivities collide (found 2026-08-01).**
  In `buildChartData.ts` the three material-balance drive indices take fixed colours
  (`#e67e22`/`#27ae60`/`#2980b9`) rather than the run's case colour, so with several variants
  visible every case draws the same three colours on top of each other. It is a two-dimensional
  problem (case x index) that one panel cannot encode by colour alone; adding a line style is
  ruled out by the styling policy, so the fix is one panel per index. Low urgency: the panel is
  hidden by default and is mostly read single-case.
- [ ] **The live waterflood `diagnostics` panel still mixes six properties (found 2026-08-01).**
  `waterfloodLivePanels.ts` puts average pressure, VRR, WOR, average saturation, water cut and MB
  error on one panel across `y`/`y1`/`y2` — the arrangement `validateSinglePropertyPanel` rejects
  for scenario layouts but does not check for live panel defs. No styling defect (every curve is
  correctly solid or dashed), so it was left alone during the 2026-08-01 style pass; splitting it
  changes the default live view of every waterflood scenario and wants its own decision.

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
  `dep_welltest.test.ts` interprets the pre-boundary numerical window and verifies the permeability
  ladder plus skin/slope separation. The visually coincident grid-resolution exhibit was removed;
  the shipped run instead continues to 2.4 days so departure from the infinite-reservoir reference
  makes closed-boundary arrival explicit.
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
- [x] **(MAJOR) IMPES oil-rate chatter at coarse report steps was two defects, one reporting and one
  physical (2026-07-31).** Reported as "IMPES needs very small steps for smooth curves, FIM does
  not". Measured on `wf_bl1d` (96 cells, 50 days, `dt_report` 0.25–2 d) by comparing the reported
  oil rate against stock-tank inventory depletion over the same interval.
  1. *Reporting*: `record_step_report` recomputed the produced phase split from the **updated**
     saturations, while `calculate_fluxes` had transported the split evaluated at the **beginning**
     of the substep. At a water-flooded producer those differ by a whole substep, so the reported
     rate was half an oscillation cycle out of phase with the oil actually produced — up to **66%**
     off over a report step, sign alternating. Fixed by capturing the split before the update
     (`producer_transport_phase_splits`) and reporting that.
  2. *Physics*: the producer cell is drained explicitly (`q·f_w(S^n)` for the whole substep), which
     rings unless `Δt·q·f'_w/V_p ≤ 1`. `max_sat_change_per_step` does not imply that criterion — the
     well cell moved ~0.045 saturation (under the 0.05 cap) while its water cut alternated between
     substeps. Added the missing well-cell throughput factor to `stable_dt_factor`
     (`impes/pressure.rs`) using `frac_flow_water_derivative` (`mobility.rs`).
  Zig-zag (mean |2nd difference| / mean reported rate, t > 20 d) before → after:
  `dt=2.0` 32.8% → 1.4%, `dt=1.0` 39.5% → 0.6%, `dt=0.5` 35.7% → 0.2%, `dt=0.25` 0.03% → 0.05%,
  at a cost of 12–20% more substeps (514→617 at `dt=2.0`). Cumulative reported oil also stopped
  depending on the report interval: 1349–1365 Sm³ spread → 1362.4–1362.5 Sm³. Pinned by
  `impes::tests::reporting::` (both tests fail on the parent commit at 41.6% / 32.8%). Gates:
  `validate-solver-coverage.sh all` 30/30, `benchmark_buckley` green (Case-B breakthrough
  rel_err 0.091 → 0.097, tolerance 0.30), `pnpm run validate:product` green.
  **Rejected alternative:** publishing one report-step point holding dt-weighted average rates
  smooths the chatter too, but the OPM Flow references are instantaneous rates at the report time
  — `three_phase_gas_drive_matches_opm_flow_reference` went to 69% error at t=10 on the steep
  early transient. ResSim must keep reporting instantaneous rates to stay comparable.
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

- [x] **Front sharpness: IMPES step-invariant, FIM smears with the step — VERIFIED CORRECT
  (2026-07-31).** Asked whether FIM's smoother front at coarse steps indicates a defect. It does
  not: it is the textbook numerical-diffusion signature of upwind advection, `D ∝ (1 − C)` explicit
  vs `D ∝ (1 + C)` implicit, with `C` the Courant number. Measured at t = 10 d, 96 cells, shock
  width = interpolated distance between the `S_w = 0.45` and `S_w = 0.15` crossings
  (`impes::tests::reporting::solver_front_sharpness_probe`, `--ignored --release`):

  | solver | report dt | max substep | max CFL | shock width |
  |---|---|---|---|---|
  | IMPES | 0.25 | 0.0428 | 0.16 | 1.94 cells |
  | IMPES | 5.0 | 0.0438 | 0.16 | 1.93 cells |
  | FIM | 5.0 | 1.0000 | 3.62 | 4.28 cells |
  | FIM | 0.25 | 0.1650 | 0.61 | 3.65 cells |
  | FIM | 0.05 | 0.0500 | 0.18 | 3.18 cells |
  | FIM | 0.02 | 0.0200 | 0.07 | 2.76 cells |

  IMPES is invariant because it is CFL-locked — it *cannot* take the 5-day step, so the report step
  is a sampling choice only (profiles agree to 4 decimals). FIM is unconditionally stable, so a
  coarse report step lets it reach `C = 3.6` and pay `(1 + C)` in diffusion. Forcing FIM to smaller
  steps sharpens it monotonically (4.28 → 2.76), i.e. it converges the right way, which is the
  evidence that the FIM discretization is sound rather than over-diffusive by construction. The
  residual gap at matched Courant number (FIM 2.76 at C = 0.07 vs IMPES 1.94 at C = 0.16) is the
  expected implicit-vs-explicit offset — `(1 + 0.07)/(1 − 0.16) = 1.27` predicted vs 1.42 observed —
  plus fully implicit end-of-step mobility upwinding. Against the analytical BL shock (a
  discontinuity, width 0) IMPES is the more accurate front here. This is the standard reason
  fully implicit simulators need small steps or higher-order/TVD transport for sharp fronts, and it
  is a genuinely good teaching exhibit for the `solver_formulation` sensitivity.

- [ ] **FIM is report-step sensitive on `wf_bl1d` where IMPES is not, via substep fragmentation
  (measured 2026-07-31, on the IMPES reporting/throughput fix above).** Surfaced by the user's new
  `solver_impes_coarse` / `solver_fim_coarse` variants. Refinement study, 50-day flood, 96 cells,
  `impes::tests::reporting::solver_timestep_refinement_probe` (`--ignored --release`, ~6 min):

  | solver | report dt | substeps | breakthrough PV | cum oil 50 d |
  |---|---|---|---|---|
  | IMPES | 5.0 → 0.25 | 613 → 704 | 0.5683 → 0.5689 | 1362.5 → 1362.4 |
  | FIM | 5.0 | 6316 | 0.5470 | 1350.6 |
  | FIM | 2.0 | 6183 | 0.5152 | 1355.9 |
  | FIM | 1.0 | 12031 | 0.5576 | 1361.9 |
  | FIM | 0.5 | 12728 | 0.5649 | 1363.1 |
  | FIM | 0.25 | 11899 | 0.5566 | 1364.1 |

  Cumulative oil converges monotonically (1350.6 → 1364.1, landing 0.12% from IMPES), so this is
  **not** a correctness defect — the residual is right and the answer converges. The mechanism is
  the timestep controller: FIM never actually takes the 5-day step, it fragments into ~630 substeps
  per report step, and the substep count is non-monotone in the requested step (6.3k / 6.2k / 12k /
  12.7k / 11.9k). Fine and coarse therefore descend different retry ladders rather than differing by
  clean time truncation, which is why the two FIM curves separate visibly while the two IMPES curves
  coincide. Same family as the open fragmentation track (`docs/FIM_STATUS.md`); the waterflood case
  is a cheaper reproducer than the gas cases.

- [ ] **`wf_bl1d` `solver_formulation` description is now factually wrong.** It claims "at 5-day
  steps IMPES loses 8.8% of its own fine-step recovery against FIM's 3.1%" and quotes 1347.7 vs
  1360.8 m³ at 0.25 days. Both were measured against the pre-fix reported rates. Post-fix: IMPES
  loses **0.0%** (1362.5 vs 1362.4), FIM loses 1.0% (1350.6 vs 1364.1), and the two agree to 0.12%
  at the fine step. The variant's teaching point has inverted — it now shows that IMPES is
  step-insensitive because its stability control picks the substeps, while FIM is the step-sensitive
  one. Note the pre-fix IMPES *solution* was already step-independent (breakthrough PV 0.5668–0.5676
  across the same sweep); only the reported rates scattered, so the old copy was describing a
  reporting artifact as a formulation property.

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
- [x] **Tarner–Tracy applicability review for Solution Gas Drive.** Evaluated and removed on
  2026-07-30: at the OPM pressure checkpoints the tank GOR starts near 176 versus OPM's 433 m³/m³,
  crosses later, and ends near 672 versus 520. ResSim itself stays within 6.1% of OPM and improves to
  0.12%, so the mismatch is the uniform-pressure/saturation tank assumption against localized BHP
  drawdown and initially mobile free gas—not a numerical-model failure. FIM sensitivities remain
  primary; OPM remains available but disabled by default.
- [x] **Log-time comparison charts left bundled OPM/published curves on raw time.** Fixed
  2026-07-30 by mapping static time-reference x values through `log10(time)` alongside numerical
  series, including reference-only previews; non-positive time points are omitted from the domain.
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
