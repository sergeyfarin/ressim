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

### 1.2 Three-phase validation

- Define the bar for leaving `experimental` status.
- Add gas-injection and gas-drive acceptance tests that check breakthrough timing, gas saturation evolution, and phase-closure diagnostics.
- Clarify what is and is not reported in current material-balance diagnostics: water and gas are explicit, oil is residual.

Why first:
- The code now contains more three-phase capability than the docs claim, but validation still lags implementation.

### 1.3 Regression coverage gaps

- Add missing comparison-model tests for preview-only cases, per-variant depletion analytics, and color-index stability.
- Add a regression guard for the duplicated undersaturated `c_o = 1e-5 /bar` assumption shared by `physics/pvt.ts` and `analytical/materialBalance.ts`.

## Priority 2: Analytical Method Integrity

### 2.1 Enforce one analytical method per scenario

- Promote sweep to a first-class analytical family in scenario capabilities instead of partially piggybacking on Buckley-Leverett semantics.
- Make invalid primary-rate and overlay combinations impossible at the type / config level, not just test-detected.
- Route benchmark disclosure and comparison metadata through the same analytical-method contract.

Why next:
- This removes a class of ambiguous chart and policy behavior before more analytical methods are added.

### 2.2 Finish the sweep-method framework

- Generalize the current `sweep_combined` Stiles / Dykstra-Parsons toggle so other sweep scenarios can opt into multiple analytical methods without custom wiring.
- Keep the semantics explicit: total recovery comparison can improve while decomposition panels remain teaching diagnostics.
- Document the `sweep_areal` quarter-five-spot interpretation so users do not mistake the outer no-flow boundaries for a gridding bug.

Why next:
- Stiles is the right direction for layered floods, but the current implementation is still scenario-specific rather than framework-level.

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

### 4.3 Export and persistence

- Add JSON export/import for scenarios and custom studies.
- Add CSV/JSON result export for sensitivity runs and benchmark summaries.

Why after architecture cleanup:
- Persistence and comparison UX are much easier to implement once the output-selection model is unified.

## Priority 5: Analytical Coverage And Physics Extensions After Validation

Case-level detail, references and blockers live in `docs/CASE_LIBRARY_ROADMAP.md` Tier 7
(stable IDs `T7.n`, enablers `E1`–`E10`). This section carries only the ordering rationale;
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
- Well-test analytical module: drawdown / buildup / Horner (**T7.1**, enabler **E10**). This is the
  largest missing pillar of classical reservoir engineering in the product. **The mathematics landed
  2026-07-24** (`src/lib/analytical/wellTest.ts`, 37 tests); the scenario wiring — union member,
  adapter, semilog chart layout — has not.
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
