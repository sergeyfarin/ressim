# ResSim Roadmap

This roadmap is future-facing. Completed work has been moved out of `TODO.md` into `.archive/docs/DELIVERED_WORK_2026_Q1.md` so the active plan stays readable.

## Prioritization Principles

The ordering below follows standard reservoir-engineering practice and the literature already referenced in the project.

1. Validate before expanding. Comparative-solution and benchmark evidence should lead black-oil and three-phase growth.
2. Keep analytical methods honest about assumptions. Buckley-Leverett, Craig, Dykstra-Parsons, Stiles, Dietz, Fetkovich, Arps, and Havlena-Odeh all have narrow validity ranges.
3. Reduce architectural duplication before adding new UI surfaces. The remaining benchmark layer and output-selection plumbing are still more expensive than they should be.
4. Add new physics only after the existing interpretation and diagnostics are trustworthy.

## Priority 1: Scientific Validation And Closure

### 1.1 Black-oil validation

Record: `docs/BLACK_OIL_VALIDATION.md` (acceptance criteria, measured baselines, replay commands, safeguards).

Done (2026-07-24):
- **Quantitative SPE1 acceptance criteria** against the `flow 2026.04` SPE1CASE1 reference, in the Rust engine rather than the frontend (`src/lib/ressim/src/tests/spe1_acceptance.rs`): field pressure 3 %, producer oil rate 8 %, producing GOR 12 %, plateau-hold 0.5 %, oil/gas material-balance drift 1 %, zero solver warnings. Worst measured errors on `0cfead9`: 1.73 % / 3.33 % / 4.39 %. Fast first-year gate runs by default in the `fim` validation bucket; the 10-year replay is an explicit `--ignored --release` run.
- **Grid-convergence checks** for pressure, Rs, Bo and liberated gas on a depletion column taken through the bubble point (`src/lib/ressim/src/tests/physics/depletion_grid_convergence.rs`): 5/10/20/40 cells, successive differences must contract by ≥ 0.8× and the two finest grids agree to 1 %. IMPES runs by default in the `impes` bucket; the FIM sweep is an explicit replay.
- **Black-oil safeguards documented** for users in `docs/BLACK_OIL_VALIDATION.md` section 3: the saturated-region `c_o` fallback and its bubble-point blend, the two-phase scalar-`c_o` choice, the `c_o` default asserted independently in the engine and the frontend, the residual-based oil material-balance diagnostic, redissolution off in SPE1, and tabular-vs-Corey SCAL.

Earlier progress (still current):
- Per-layer cell thickness (`dz` as `Vec<f64>`) and per-layer initial gas saturation are implemented in the Rust solver and wired through the TypeScript worker.
- Per-layer well completions (`producerKLayers`, `injectorKLayers`) allow single-layer wells as required by SPE1.
- SPE1 scenario is defined (`spe1_gas_injection`) with full PVT table, exact SWOF/SGOF tables, per-layer dz/perm, deck-intent surface-rate well control, and the Case 1 reference overlay.
- Published-reference overlay infrastructure (`publishedReferenceSeries`) is wired through the chart model with scatter markers.

Remaining:
- No SPE-style black-oil case beyond SPE1 (SPE9, volatile-oil style depletion) is covered.
- FIM and IMPES converge to measurably different answers on the same depletion column (~10 % on liberated gas); each is self-consistent under refinement, so this is a solver/timestep question, not a gridding one.
- SPE1 scenario-wiring regressions (published-reference panel placement, `cellDzPerLayer`, per-layer completion payloads) are still frontend-side gaps.

Why first:
- In the reservoir-simulation literature, black-oil extensions are only meaningful when the pressure equation, PVT coupling, and material-balance behavior are benchmarked against accepted reference problems.

### 1.2 Three-phase validation — DONE 2026-07-25

Record: `docs/THREE_PHASE_VALIDATION.md` (exit criteria, acceptance criteria, measured
baselines, replay commands, remaining envelope limits).

Done:
- **Exit criteria defined** as five conditions (`docs/THREE_PHASE_VALIDATION.md` §1) and met;
  the `experimental` label was removed from README, the implementation notes, the case-library
  roadmap and the `gas_drive` scenario copy.
- **Solution-gas-drive comparative solution.** `gas_drive` was upgraded from constant-PVT
  immiscible depletion to genuine black-oil (saturated start at the bubble point, Rs(P) from
  `generateBlackOilTable`), a matching OPM Flow deck was added
  (`tools/opm_flow/opm_flow_tool/cases.py::GAS_DRIVE`), and the parsed `flow 2026.04` series are
  committed as `src/lib/catalog/opm-flow-results/gas_drive.json`. Acceptance: pressure 3 %, GOR
  12 %, cumulative oil 8 %, oil rate 10 % while the rate is meaningful, oil/gas MB drift 1 %,
  zero solver warnings. Worst measured: 1.59 % / 6.08 % / 4.32 % / 4.61 %.
- **Gas-front acceptance tests** in `src/lib/ressim/src/tests/three_phase_acceptance.rs`:
  breakthrough timing inside a 2–8 day band and unchanged under timestep halving; gas saturation
  monotone in space and advancing monotonically in time; per-phase closure including oil.
  All wired into the `fim` bucket of `scripts/validate-solver-coverage.sh`.
- **Material-balance reporting clarified and corrected.** The old claim that oil is "residual"
  in the diagnostics was wrong: `material_balance_error_oil_m3` is direct. What is residual is
  the oil *saturation* in transport. Documented in `docs/THREE_PHASE_VALIDATION.md` §4.
- **Producing-GOR reporting bug fixed.** GOR was forced to 0 below an absolute 10 Sm³/day oil
  rate, blanking the diagnostic for most of a depleting gas-drive run. The floor is now a
  divide-by-zero guard.

Remaining (envelope limits, not validation debt):
- No three-phase analytical reference exists; grading is against numerical references.
- Vaporized oil (Rv) is not modelled, so wet-gas / gas-condensate is out of envelope.
- `gas_injection` has no OPM reference of its own (covered indirectly by SPE1).
- The +4 % cumulative-oil bias vs OPM on `gas_drive` is inside band but unexplained.

### 1.3 Regression coverage gaps — DONE 2026-07-25

Done:
- **Comparison-model tests** added to `src/lib/charts/referenceComparisonModel.test.ts` (42 tests,
  was 34): preview-only cases (declaration-order `colorIndex`, single-variant staying out of the
  cases selector and drawn neutral, `previewBaseParams` fallback), per-variant depletion analytics
  (one dashed reference per completed run vs. one shared `analytical-shared` curve, pending-variant
  overlays), and color-index stability (a pending variant holds its declared palette slot across
  0/1/2 completed runs, chip color equals its dashed-curve color and survives completion, palette
  wraparound at index 20).
- **`c_o` regression guard** in `src/lib/analytical/materialBalance.test.ts`: the numeric agreement
  test is now backed by a source-shape guard (exactly one definition of
  `DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR`, no re-introduced `c_o = 1e-5` literal in
  either `physics/pvt.ts` or `analytical/materialBalance.ts`) and a cross-language check that the
  Rust `FluidProperties::default_pvt()` `c_o` still equals the frontend constant.

Replay: `pnpm run typecheck && pnpm run lint && pnpm test` — 726 passed / 15 skipped, measured
2026-07-25 on the working tree that adds these tests (parent commit `e824198`).

## Priority 2: Analytical Method Integrity

### 2.1 Enforce one analytical method per scenario

**Step 1 of 4 — DONE 2026-07-25: analytical-method registry.**

`src/lib/charts/analyticalMethodRegistry.ts` is now the single routing table for the comparison
chart stack. Each `AnalyticalMethod` declares its curve slots (panel + curve key + label per
context), overlay builders, overlay-mode rule, native x-axis, panel presentation and disclosure
wording. `buildChartData.ts` walks those slots in one generic loop instead of a four-way branch
ladder repeated across four contexts, and `ReferenceComparisonChart.svelte`,
`benchmarkDisclosure.ts` and `axisAdapters.ts` read the registry instead of carrying their own
copies of the same branch. Adding an analytical method is now: write the overlay builder, add one
registry entry.

This also closed a real defect it exposed. The old branch ladder ended in a bare `else` that ran
the *depletion* overlay path, so `wf_tornado` and `wf_bl1d_opm` — both `analyticalMethod: 'none'` —
were rendering depletion recovery and cum-oil reference curves. Measured before/after and pinned by
`referenceComparisonModel.test.ts` → "emits no analytical reference curves for a scenario with
analyticalMethod none". Sweep scenarios were also silently building depletion overlays that
`suppressPrimaryAnalyticalPanels()` then stripped; the rendered result there is unchanged.

Replay: `pnpm run validate` — 737 passed / 15 skipped, measured 2026-07-25 on the working tree that
adds the registry (parent commit `e824198`).

**Step 2 of 4 — DONE 2026-07-25: sweep is a first-class analytical method.**

`'sweep'` joined the `AnalyticalMethod` union with a descriptor that declares no primary curve
slots, so the BL water-cut/recovery overlays are never built for a sweep scenario. That deleted the
whole build-then-hide mechanism: `suppressPrimaryAnalyticalOverlays` (the field and every threading
of it), `suppressesPrimaryAnalyticalOverlays()` / `hasPrimaryAnalyticalReferenceCurves()` (the
layout-substring inference), `suppressPrimaryAnalyticalPanels()` / `stripReferenceCurveKeys()` (the
strip pass), and the last `showSweepPanel === true` routing conditions.

`showSweepPanel` is no longer a declared capability — it is derived from
`analyticalMethod === 'sweep'` in `resolveCapabilities()`, so the live single-run path keeps its
existing call sites with one source of truth. `sweepGeometry` is now required on sweep and rejected
elsewhere. Which *simulation* curves a chart shows also became declared
(`AnalyticalMethodDescriptor.simulationCurveSet`) rather than another branch on the method name;
sweep and Buckley-Leverett share the water-cut set.

`validateScenarioChartLayout()` flipped from a negative check ("sweep must not name reference
curves") to a positive one ("a layout may only name reference curves its method actually emits").
That immediately found three dead references and they are fixed here:
- `wf_bl1d_opm` and `wf_tornado` (`analyticalMethod: 'none'`) shared the `waterflood` layout and
  asked for BL water-cut/recovery references — the layout-side residue of the step-1 fix. Both now
  patch those keys out.
- the shared `waterflood` layout asked for `cum-oil-reference`, which the BL method has never
  emitted in any overlay context. Dead key removed; the underlying asymmetry is logged in TODO.md.

Verification: the three sweep scenarios' rendered comparison models — every panel, curve key, label,
color, border, point count and endpoint values, on both the time and PVI axes — are byte-identical
before and after, checked by diffing a dumped model against the parent commit. That is the load-
bearing check, since the strip pass was already removing exactly what is now never built.

Replay: `pnpm run validate` — 741 passed / 15 skipped, measured 2026-07-25 (parent commit `37f7583`).

Remaining:
**Step 3 of 4 — DONE 2026-07-25: capabilities are a discriminated union.**

`ScenarioCapabilities` is now a union over `analyticalMethod`, with each arm's `primaryRateCurve`
narrowed to that method's `supportedRateCurves` and `sweepGeometry` required on `'sweep'` and typed
`never` elsewhere. The arms are *derived* from `ANALYTICAL_OUTPUT_CONTRACTS` via a mapped type
rather than restated, so the contract table is the single source of the rule at both compile time
and run time; the table switched from a type annotation to `as const satisfies` to keep its literal
tuples.

Three rules moved from runtime to compile time and their runtime checks are gone:
unsupported `primaryRateCurve`, missing `sweepGeometry` on sweep, `sweepGeometry` on a non-sweep
method. `validateScenarioCapabilities()` retains only the `runMode` / `default3DScalar` rule —
`runMode` is a second, independent discriminant and crossing it with `analyticalMethod` would
multiply the arms for one check. The three tests that used to construct invalid capabilities are now
`@ts-expect-error` assertions, so `pnpm run typecheck` fails if any of them ever starts compiling.

Verified by probe: `sweep`+geometry, BL+`water-cut`, depletion+`oil-rate`, none+`gas-cut` and
bare `well-test` all compile; BL+`oil-rate`, gas-oil+`water-cut`, depletion+geometry and bare
`sweep` are all rejected. Also fixed a stale test that claimed to cover "all AnalyticalMethod
values" while hardcoding 4 of 7 — it had silently stopped covering `well-test` and
`digitized-reference`.

Replay: `pnpm run validate` — 743 passed / 15 skipped, measured 2026-07-25 (parent commit `0acba9f`).

**Step 4 of 4 — DONE 2026-07-25: reference sources are declared.**

One `Scenario.referenceSources` list replaces three mechanisms that between them made "which
references does this chart show?" unanswerable from the scenario file:

- `publishedReferenceSeries` — a static series array;
- `opmFlowReferenceArtifactKeys` — an array that, despite the name, had **no effect at all on
  live-worker scenarios**; only `prerun-artifacts` scenarios read it;
- an implicit match of every bundled OPM artifact whose `scenarioKey` equalled the scenario's key.
  That invisible third path is how live scenarios actually got their OPM overlays. `wf_bl1d`
  declared `opmFlowReferenceArtifactKeys: ['wf_bl1d']` that did nothing, and received the same
  artifact anyway through a name match nothing in the file mentioned.

Now: `{ kind: 'published', series }` or `{ kind: 'opm-flow', artifactKeys, role?: 'overlay' |
'primary' }`, resolved in declaration order by `resolveScenarioReferenceSeries()`. Nothing is
matched implicitly — an artifact renders only where a scenario names it, which is now covered by a
test.

Verified: the resolved reference series for all 16 scenarios — source type, artifact key, panel,
label, curve key, axis, primary flag, point count and endpoints — diff byte-identical before and
after.

Replay: `pnpm run validate` — 746 passed / 15 skipped, measured 2026-07-25 (parent commit `83bcb5e`).

Priority 2.1 is complete. What it bought, end to end: adding an analytical method is one registry
entry instead of edits across four modules; sweep is a method rather than a flag plus a
build-then-strip pass; invalid capability combinations are compile errors; and every non-simulation
curve on a chart is traceable to a line in the scenario file. Three latent defects surfaced and
were fixed along the way (`'none'`-method scenarios rendering depletion overlays, dead layout
reference keys, a stale contract-coverage test).

Why next:
- This removes a class of ambiguous chart and policy behavior before more analytical methods are added.

### 2.2 Finish the sweep-method framework — DONE 2026-07-25

- **Sweep-method selection is declared, not hand-written.** `sweepMethods` on the sweep capabilities
  arm lists the selectable correlations, most-preferred first; the first entry is the default, so
  there is no `default: true` flag to keep in sync with ordering.
  `getScenarioAnalyticalOptions()` derives the picker's options from it and
  `src/lib/analytical/sweepMethods.ts` owns each correlation's label, claim and citation.
  `Scenario.analyticalOptions` is deleted — it existed only on `sweep_combined`, carrying that
  scenario's prose inline, which is why the toggle was available on exactly one case.
  `sweep_vertical` now offers the choice with no scenario-specific wiring.

- **Offered only where the choice is live, measured not assumed.** At `sweepGeometry: 'areal'`
  Stiles and Dykstra-Parsons are *numerically identical* — max |Δ| is exactly 0 on E_A, E_vol and RF
  across the whole PVI curve, because with one layer Stiles has nothing to order and both defer to
  the same Craig five-spot correlation. A toggle on `sweep_areal` would be a control that does
  nothing, so it declares no `sweepMethods`. Where layers exist the choice is large: max |Δ| ≈ 0.97
  on E_V and 0.037 on RF for `sweep_vertical`, 0.12 on E_vol and 0.055 on RF for `sweep_combined`.
  Both facts are pinned by `src/lib/analytical/sweepMethods.test.ts`.

- **Semantics stay explicit.** Every generated method summary ends with the decomposition caveat for
  the components that scenario shows ("E_A and E_V remain analytical diagnostic decomposition
  views"), so a better total-recovery comparison never reads as a promotion of the per-component
  panels to predictions. Summaries are geometry-aware for the same reason: a vertical-only scenario
  is not told its answer came from a Craig areal factor that is identically 1.

- **`sweep_areal` documented as a quarter five-spot.** Injector at (0,0), producer at (20,20) of a
  21×21 grid; the four outer boundaries are the pattern's symmetry planes, not a gridding artefact.
  Stated in the scenario description and beside the grid params, since a confined five-spot repeats
  and the quarter element is the standard unit Craig E_A applies to.

Replay: `pnpm run validate` — 757 passed / 15 skipped, measured 2026-07-25 (parent commit `50226a6`).

`SwProfileChart` — the last item bundled into this section — was resolved 2026-07-25 by rebuilding
it as `visualization/SpatialProfileChart.svelte` in the 3D group rather than the run-results charts.
It was dormant because it was mis-filed: run-results charts show one value per report step across a
run, this shows one value per cell at one instant, so it had no timestep to follow. It now reads the
3D view's selected snapshot and property selector, covers pressure as well as all three saturations,
and profiles along I/J/K in metres. That also cleared the display blocker on T7.4's
gravity-capillary transition zone, which needed a saturation-vs-depth view.

## Priority 3: Scenario And Benchmark Architecture Consolidation

### 3.1 Remove the remaining split brain — DONE 2026-07-17

The legacy benchmark-family system was archived and type ownership moved to
`src/lib/scenario/referenceTypes.ts`. Full record: `docs/FRONTEND_EXECUTION_PLAN_2026-07.md` Wave 3
and `.archive/docs/TODO_HISTORY_2026-07-24.md`.

### 3.2 Extract the output-selection view model

- Pull the active output payload selection out of `App.svelte` so charts, 3D view, and analytical helpers consume the same typed result.
- Use that refactor to simplify comparison-state wiring and reduce chart duplication.

Why this block matters:
- The largest remaining maintenance cost in the UI is architectural overlap, not missing widgets.

## Priority 4: Product Workflow And Data Portability

### 4.1 Per-scenario overrides

- Introduce scenario-preserving parameter overrides instead of forcing users into an all-or-nothing custom mode.
- Track per-field provenance and reset behavior.

### 4.2 Multi-case inspection

- Add multi-case 3D inspection and synchronized case selection across charts, summaries, and spatial views.
- Restore or explicitly retire the dormant saturation-profile path as part of the same output review.

### 4.3 History/forecast divider on non-linear time axes (E5, deferred from TODO 2026-07-24)

- `resolveHistoryDivider` (`src/lib/charts/historyDivider.ts`) only matches `axis: 'time'`, so the
  divider never draws on `dep_nct`, whose `fetkovich` layout opens on `xAxisMode: 'logTime'`.
- `logTime` plots `log10(time)` on a linear scale, so a fix means treating `logTime` as time-family
  with the boundary transformed to `Math.log10(boundary)` (guard `boundary > 0`).
- Deferred rather than done: it is not settled that the divider is the right treatment for a
  Fetkovich log-time plot at all. Decide the intended UX before touching the resolver.
- Related, recorded so it is not assumed away: the divider exists only on the comparison-chart path
  (`ReferenceComparisonChart`); the live single-run `RateChart`/`UniversalChart` path does not thread
  `historyWindow`. Fine by design today.

### 4.4 Export and persistence

- Add JSON export/import for scenarios and custom studies.
- Add CSV/JSON result export for sensitivity runs and benchmark summaries.

Why after architecture cleanup:
- Persistence and comparison UX are much easier to implement once the output-selection model is unified.

## Priority 5: Analytical Coverage And Physics Extensions After Validation

Case-level detail, references and blockers live in `docs/CASE_LIBRARY_ROADMAP.md` Tier 7
(stable IDs `T7.n`, enablers `E1`–`E11`). This section carries only the ordering rationale;
`TODO.md` carries the active checkboxes. Do not restate case detail in all three places.

The 2026-07-24 gap audit of the shipped library (14 scenarios, 4 analytical modules) produced two
structural findings that reorder this priority:

- **Capillarity is implemented, validated, and used by no scenario** (`capillaryEnabled: false` in
  all 14; gravity on in only 2). Exercising existing physics now outranks adding new physics.
- **No ensemble/fan chart primitive exists**, so no case can pose a P10/P50/P90 or
  "many models match, forecasts diverge" question. That is a chart-architecture gap (E8), not a
  physics gap, and it gates the most valuable remaining case content.

### 5.1 Exercise shipped physics and close analytical gaps (no engine change)

- ~~Capillary waterflood case (**T7.4**) — first scenario to turn capillarity on.~~ **Done
  2026-07-24** (`wf_capillary`). The gravity-capillary *transition-zone* half remains open and is
  blocked on a saturation-vs-depth profile chart, not on physics.
- ~~Well-test analytical module and scenario: drawdown / buildup / Horner (**T7.1**, enabler
  **E10**).~~ **Done 2026-07-24** — `wellTest.ts` (37 tests) plus `'well-test'` as a first-class
  analytical method and the `dep_welltest` scenario. This closed the largest missing pillar of
  classical reservoir engineering in the product. It also produced a result worth acting on: read as
  a measurement instrument, the simulator reports permeability ~9 % low and a slightly stimulated
  well that is not stimulated, converging with near-well refinement but still biased at 10 m cells.
- Grid orientation (**T7.11**) was attempted and refuted on 2026-07-24: a single injector-producer
  pair cannot separate grid alignment from pattern geometry. It now depends on multi-well pattern
  support (**E11**) and belongs under 5.4, not here.
- Numerical-vs-physical dispersion framing (**T7.12**) — partly delivered as the second dimension of
  `wf_capillary`; the `wf_bl1d` grid-ladder framing is still open.
- Dry-gas p/z material balance and gas-cap blowdown (**T7.2**). `materialBalance.ts` already carries
  the gas-cap ratio `m` and `driveIndex_gasCap`; no scenario exercises either.
- Koval correction for unfavorable-mobility floods (**T7.5**).

### 5.2 Uncertainty and decision content (needs the chart pass, not new physics)

- Ensemble / fan-curve chart primitive (**E8** → **T7.19**): P10/P50/P90 bands across live variants
  and across multiple pre-run artifacts. Gated behind the Priority 3 chart consolidation, since it
  lands on `buildChartData.ts`.
- Combined-uncertainty cases once E8 exists: capillary × layer contrast (**T7.16**), relperm
  endpoints × heterogeneity (**T7.18**), joint endpoint uncertainty (**T7.14**).
- Per-cell permeability on the single-run path (**E1**, half-wired today) → the Tavassoli
  "perfect match, wrong forecast" flagship (**T7.9**) and SPE10 Model 1 / layer subsets
  (**T7.6**, **T7.7**).

### 5.3 Vertical and areal sweep upgrades

- Kv/Kh-aware Warren-Root style blending between Dykstra-Parsons and perfect communication.
- Additional well-pattern correlations only after current sweep semantics are clean (Priority 2.2).

### 5.4 Longer-range reservoir-model features

- Aquifer models (**E9**) — the one large physics item with broad reach: it unlocks water-drive gas
  (**T7.2**), aquifer-strength × OOIP ambiguity (**T7.17**), and live PUNQ-S3.
- Well schedules (**E2**) — unlocks SPE9 (**T7.8**) and immiscible WAG.
- Multi-well patterns (**E11**) — the worker already honors a `payload.wells` array but no scenario
  drives it. Unlocks the real Yanosik-McCracken grid-orientation construction (**T7.11**), SPE9, and
  pattern-density studies (**T7.15**).
- Relperm hysteresis (**E4**), inactive cells (**E6**), per-well injected fluid (**E3**).
- Non-uniform grids and local refinement.
- Horizontal or deviated wells.

Why later:
- These features add breadth, but they should come after the simulator's current black-oil, gas, and
  analytical foundations are better validated — and, per the audit above, after the physics already
  in the engine is actually reachable from the case library.

## References Behind The Ordering

The roadmap direction is consistent with the classic references already used by the project and standard simulator-development practice:

- Buckley and Leverett, Welge: use analytical flood theory only where assumptions remain explicit.
- Craig; Dykstra and Parsons; Stiles: sweep methods are pattern- and communication-dependent, so method selection must stay explicit.
- Dietz; Fetkovich; Arps; Havlena and Odeh: depletion diagnostics are useful only when geometry, PVT, and drive assumptions are clear.
- SPE comparative-solution practice: benchmark the physics before claiming maturity for a simulator mode.

## Delivered Work

Recent delivered work lives in `.archive/docs/DELIVERED_WORK_2026_Q1.md`.
