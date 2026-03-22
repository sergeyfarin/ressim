# ResSim — Roadmap

Phased plan to evolve ResSim from a validated 2-phase waterflood teaching tool into a full black-oil simulator with gas, volatile-oil, and compositional-aware analytics.

---

## Phase 1 — Consolidate & Fix (current foundations)

Goal: clean up tech debt, fix known correctness issues, finish incomplete work.

### Active Slice (2026-03-21)

- [x] Restore repository health for validation checks (`npm run typecheck`, `npm run lint`, `npm test`, and build-related Svelte diagnostics) after the recent sweep/chart changes.
- [x] Add a dedicated `npm run validate` bundle for sequential repo validation (`typecheck` → `lint` → `test` → `build`).

### 1A. Three-Phase Correctness Fixes

Confirmed bugs blocking gas scenarios from leaving "experimental" status.

- [x] **Gas-oil capillary pressure direction** — `capillary.rs`: P_cog now parameterised on S_o_eff using `s_org`; increases correctly with S_g.
- [x] **Stone II missing `S_org` parameter** — `relperm.rs`: `k_ro_gas` now uses `s_org` (residual oil to gas) as terminal saturation. `s_org` added to `RockFluidPropsThreePhase`, WASM API, TypeScript types, store, worker, and gas scenarios (default 0.20).
- [x] **3-phase material-balance diagnostic** — `step.rs`: gas accumulator (`actual_change_gas_m3`) added; gas injection/production tracked separately; `cumulative_mb_gas_error_m3` reported in `TimePointRates.material_balance_error_gas_m3`.

### 1B. Analytical Contract Gaps

- [x] **Dietz well-location sensitivity** — `computeShapeFactor()` now accepts `producerI`/`producerJ` and uses log-linear interpolation between tabulated C_A endpoints (center 30.8828, corner 0.5598). All three adapter call sites pass grid dims and producer position. `affectsAnalytical: true` for both shape-factor variants. Tests prove center/corner divergence in q0 and tau.
- [x] **Analytical adapter coverage tests** — contract test in `scenarios.test.ts` verifies every `affectsAnalytical: true` variant actually changes analytical output. Runs BL, sweep, and depletion fingerprints for all 6 analytical scenarios (50 test cases).
- [x] **Analytical method disclosure upkeep** — `dep_pss` summary already reads "for the active well location"; `dep_decline` (1D slab, no position sensitivity) is also correct. `ScenarioPicker.svelte` renders metadata directly — no alignment gap.

### Discoveries (from 1B work)

- **`sweep_ladder` affectsAnalytical semantic gap** — `sweep_combined / sweep_ladder` variants patch `mu_o` (which DOES change BL/sweep analytical output) yet are marked `affectsAnalytical: false`. This is intentional — the ladder dimension uses a shared analytical reference for pedagogical clarity while degradation is shown via simulation only. The contract test correctly skips the `false` direction for this reason. Document this pattern if more "intentionally-shared" dimensions are added.
- **Pre-existing test failures** — 7 UI/terminology tests (`modePanelFlows`, `outputTerminology`, `terminologyCopy`, `appThemeTypography`, `appStoreDomainWiring`, `ratechart-usage`) and 2 `referenceComparisonModel` tests fail on clean master. Not caused by 1B changes.
- **Dietz C_A log compression** — the 55× shape factor ratio (center vs corner) compresses to only ~30% PI difference through the `ln(A/C_A)` denominator. This limits the visual divergence between analytical curves; the scenario description should set appropriate expectations.

### 1C-caps. Scenario Capability Declarations (Phase 1 Refactor)

`ScenarioCapabilities` type added to `Scenario` — each scenario explicitly declares its analytical method, primary rate curve, sweep panel, x-axis, injector presence, 3D default, and three-phase gate. Consumer code reads capabilities instead of branching on `scenarioClass`/`domain`. All consumer-side branching now uses `analyticalMethod` (unified vocabulary); `caseMode` eliminated.

**Completed (2026-03-21):**
- [x] `ScenarioCapabilities` type with 9 behavioral fields replaces scattered conditional logic
- [x] All 8 scenarios split into per-scenario files under `src/lib/catalog/scenarios/` for easy comparison
- [x] `scenarios.ts` restructured as barrel file (types + chart presets + imports + lookup helpers)
- [x] `CUSTOM_MODE_CAPABILITIES` default for custom mode (no scenario object)
- [x] `simulationStore.svelte.ts`: `activeScenarioAsFamily` adapter, `CaseMode` resolution, sweep specs all use capabilities
- [x] `App.svelte`: `showSweepPanel`, `default3DProperty` use capabilities
- [x] `ScenarioPicker.svelte`: `requiresThreePhaseMode` replaces `domain !== 'gas' || activeMode === '3p'`
- [x] `referenceComparisonModel.ts`: sweep panel gate simplified to `family.showSweepPanel`
- [x] `ReferenceComparisonChart.svelte`: sweep panel gate and gas-oil-bl preview axis fixed
- [x] Sensitivity-level `analyticalOverlayMode` metadata added for all scenario-defined analytical studies, so preview/comparison overlay grouping is scenario-owned (`shared` vs `per-result`) instead of falling back to chart-side `auto` inference except for legacy benchmark-family flows

**Consumer-side branching — migrated to `analyticalMethod` (2026-03-21):**
- [x] All consumer types (`BenchmarkRunSpec`, `BenchmarkRunResult`, `BenchmarkReferencePolicy`) use `analyticalMethod: AnalyticalMethod` instead of `scenarioClass`
- [x] All branching in `benchmarkRunModel.ts`, `referenceComparisonModel.ts`, `referenceChartConfig.ts`, `benchmarkDisclosure.ts`, `ReferenceComparisonChart.svelte` uses `analyticalMethod`
- [x] `previewScenarioClass` prop/param renamed to `previewAnalyticalMethod` throughout
- [x] `phase2PresetContract.ts` `benchmarkScenarioClass` parameter type widened to `AnalyticalMethod`
- [x] `caseMode` eliminated from `ScenarioCapabilities` — derived from `analyticalMethod` at the single consumption site in `simulationStore`
- [x] `BenchmarkScenarioClass` type and `BenchmarkFamily.scenarioClass` field kept as deprecated — only used by the 5 benchmark family definitions and the adapter mapping in `simulationStore`

**Analytical Output Contracts & Validation (2026-03-21):**
- [x] `ANALYTICAL_OUTPUT_CONTRACTS` table — declares what each `AnalyticalMethod` produces, supported rate curves, native x-axis, default panel expansion, and tau availability
- [x] `resolveCapabilities()` — merges analytical method defaults with optional per-scenario overrides; returns fully-resolved `ResolvedCapabilities`
- [x] `validateScenarioCapabilities()` — catches configuration mistakes (e.g. requesting `water-cut` from a depletion method) at test time
- [x] Per-scenario capabilities simplified — `primaryRateCurve`, `analyticalNativeXAxis`, `hasTauDimensionlessTime` now optional (derived from analytical method unless overridden)
- [x] `simulationStore` adapter uses `resolveCapabilities()` for the `activeScenarioAsFamily` bridge
- [x] Dead `activeMode` parameter removed from `buildScenarioEditabilityPolicy`, `shouldAutoClearModifiedState`, `shouldAllowReferenceClone` (3 functions in `phase2PresetContract.ts` + all call sites)
- [x] 6 new validation tests in `scenarios.test.ts` covering contracts, resolution, overrides, and error detection
- [x] `CaseMode` confirmed as catalog-key only — all remaining branches are legitimate catalog/preset lookups (no behavioral migration needed)

**Discovered issues:**
- `gas_injection` scenario was silently falling through to `CaseMode: 'dep'` instead of `'3p'` — now CaseMode derived from analyticalMethod (gas-oil-bl → 'dep')
- `gas_drive` same issue — CaseMode derived as 'dep' (analyticalMethod: 'none')
- `ReferenceComparisonChart.svelte` preview x-axis effect was missing `'gas-oil-bl'` — gas-oil scenarios in preview mode weren't defaulting to PVI x-axis (fixed)

### 1C. Legacy Cleanup (Phase 1 Step 7 Remainder)

Eight legacy catalog/benchmark files still have active production dependencies. The blocker is that `ReferenceExecutionCard`, `benchmarkRunModel`, and the chart layer still import from old catalog files.

- [x] Audit whether `ReferenceExecutionCard` and `benchmarkRunModel` are superseded by the S1 sweep-run model
- [ ] Delete confirmed-dead files; update remaining imports; verify 0 TS errors and all tests pass

**Audit result (2026-03-20):** None of the 8 files are dead — all are actively load-bearing. The S1 scenario system coexists with (rather than replaces) the old benchmark layer:

- `benchmarkRunModel.ts` defines `BenchmarkRunSpec`/`BenchmarkRunResult` types shared by *both* old reference families and new scenario sweeps (the store synthesizes virtual families via `activeScenarioAsFamily`).
- `ReferenceExecutionCard.svelte` is still the only UI for traditional benchmark family selection (rendered in App.svelte when `activeReferenceFamily` is set).
- `benchmarkDisclosure.ts` provides parameter snapshots/variant summaries consumed by `ReferenceExecutionCard`.
- `benchmarkCases.ts` exports benchmark family definitions and types imported by 10+ files.
- `caseCatalog.ts` re-exports from benchmarkCases + caseLibrary + presetCases; provides `CaseMode`/`ToggleState` types used across UI and stores.
- `caseLibrary.ts`, `presetCases.ts`, `phase2PresetContract.ts` all imported by caseCatalog, stores, or UI components.

**To unblock deletion**, the following migration is needed:
1. Move shared types (`BenchmarkRunSpec`, `BenchmarkRunResult`, etc.) into a new `referenceRunModel.ts` (or into `scenarios.ts`)
2. Merge `ReferenceExecutionCard` functionality into `ScenarioPicker` or a unified sensitivity-selection component
3. Migrate remaining preset cases and benchmark families into `scenarios.ts` with equivalent sensitivity dimensions
4. Inline or relocate `caseCatalog.ts` type exports (`CaseMode`, `ToggleState`, `Dimension`, etc.) into appropriate modules

### 1D. Chart & Output Polish

- [x] **Sweep geometry masking bug** — `computeCombinedSweep()` now masks inapplicable component curves to unity before returning results, so vertical-only cases no longer expose Craig `E_A` curves and areal-only / uniform-layer cases no longer expose spurious `E_V` penalties.
- [x] **RateChart sweep analytics ignore geometry** — `RateChart.svelte` now receives `sweepGeometry` from `App.svelte` and passes it into `computeCombinedSweep(...)`, keeping the runtime sweep decomposition aligned with the geometry-aware RF calculation.
- [x] **Simulation sweep panels are not geometry-aware** — sweep panel visibility is now gated by inferred geometry in both the runtime chart and the comparison model, so vertical-only XZ cases hide areal panels and areal-only cases hide vertical panels.
- [x] **Runtime sweep threshold uses `s_wc` instead of `initialSaturation`** — `App.svelte` now derives the swept-cell threshold from `initialSaturation`, matching `referenceComparisonModel.ts`. Runtime and comparison sweep curves now use the same swept-cell criterion.
- [x] **Sweep panels were hard-wired to PVI x-axis** — runtime and comparison sweep charts now remap sweep simulation and analytical curves onto the selected x-axis for completed runs; comparison sweep panels also share the common x-range with the other subplots.
- [x] **Chart x-range truncation is now preset-driven** — x-axis range policy moved into `rateChart` layout config, so breakthrough cases keep tail clipping while sweep scenarios guarantee at least a 0–2.5 PVI view remapped onto the selected axis, even when runs stop earlier.
- [x] **Sweep geometry is now scenario-driven** — vertical, areal, and combined sweep panels/normalization come from scenario capabilities rather than per-variant input inference in either the runtime chart or the comparison model, so uniform variants no longer flip vertical/combined scenarios into areal mode.

- [x] Scenario sweep runtime controls keep `steps` tied to the run-controls input while allowing per-variant `Δt` defaults unless the timestep field is explicitly edited.
- [x] **Color stability when sweep results arrive out of order** — `orderResults()` now sorts by `previewVariantParams` declaration index; pending preview cases also use declaration-order color indices.
- [x] **Snapshot target reduced from ~50 to ~25 samples per run** — `defaultHistoryInterval`, benchmark run specs, and scenario sweep specs now all sample history at roughly 25 snapshots instead of 50, reducing worker/UI overhead and speeding up comparison runs.
- [x] **Combined sweep numerics now match the tuned vertical scenario** — `sweep_combined` now uses the same fine `Δt` / step budget as `sweep_vertical`, fixing the remaining early-time jump at the scenario level instead of distorting the shared simulation E_V metric.
- [x] **Worker now emits an actual t=0 snapshot before stepping** — the run pipeline posts a pre-step `state` message with `stepIndex = -1`, so sweep charts and playback no longer rely only on synthetic zero anchors when diagnosing early-time behavior.
- [x] **Sweep chart dedupe now preserves the true origin point** — if the first post-step sweep sample remaps to x=0, the runtime and comparison charts keep the origin anchor instead of overwriting it with the first nonzero sweep value.
- [x] **Combined-scenario component sweep charts are now analytical-only** — for `sweepGeometry='both'`, the runtime and comparison charts no longer calculate or plot simulated `E_A` / `E_V`. Combined decomposition stays analytical, while simulation contributes total outcomes only.
	- [x] Keep analytical `E_A`, `E_V`, and analytical `E_vol` visible for combined scenarios.
	- [x] Replace the combined simulation curve in the total panel with an explicitly named `Mobile Oil Recovered` metric derived from `1 - remaining_mobile_oil / initial_mobile_oil`, instead of pretending it is a decomposed simulated `E_vol`.
	- [ ] Follow up with stronger analytical layered accounting via Stiles so the combined analytical comparison improves without reviving subjective simulated `E_A` / `E_V` heuristics.
- [ ] **Single-variant preview uses neutral reference color** — inconsistent with multi-variant behavior. Low priority.
- [x] **Sweep panels now support preview and pending overlays** — comparison sweep charts render analytical-only preview panels before runs complete, keep pending variants visible during mid-sweep, and overlay solid simulation curves with dashed analytical references once results land.
- [x] **Sweep breakthrough overlay grouping is now scenario-driven** — sweep sensitivity dimensions now declare whether BL/gas-BL breakthrough references stay `shared` or remain `per-result` after runs complete, so mobility-sensitive sweep scenarios keep their multiple analytical curves while intentionally shared heterogeneity ladders stay collapsed to one.
- [x] **Completed PVI BL references no longer depend on runtime injection history** — `referenceComparisonModel.ts` now uses the pure analytical PVI path for completed BL / gas-oil BL overlays on the PVI axis, so sweep scenarios do not collapse analytical breakthrough curves to zero when runtime `total_injection` fields are missing or zero.
- [x] **`previewBaseParams` coupling is fragile** — removed redundant `!previewVariantParams?.length` guard from App.svelte; model builder already handles variant→base precedence internally.
- [ ] **Tests missing for `previewCases` and depletion per-variant** — add coverage for pure-preview mode, mid-sweep mode, depletion with `analyticalPerVariant=true`, and `colorIndex` offset correctness.

### 1E. Documentation Refresh

- [ ] Update BENCHMARK_MODE_GUIDE.md — references pre-S1 `ModePanel.svelte` and 4-layer navigation
- [ ] Resolve SwProfileChart status — either restore the card in `App.svelte` or remove the stale component and doc references
- [ ] Document capillary pressure cap (20 × P_entry) in user-facing physics notes, not just code comments
- [ ] Clarify `sweep_areal` geometry / boundary interpretation — the current injector-at-origin, producer-at-far-corner setup behaves as a quarter five-spot symmetry element with no-flow outer boundaries, so `Imax`/`Jmax` wall lag can be mistaken for an indexing bug. Add a note in scenario/docs or revise the setup if full-pattern visual intuition is desired.

---

## Phase 2 — Custom Mode Redesign & UX

Goal: make custom mode a deliberate power-user feature; improve input density.

### 2A. Custom Mode Redesign

Current custom mode is a catch-all that dumps 50+ raw parameter inputs with no context, no grouping intelligence, and no relationship to the predefined scenarios. It reads as legacy, not intentional.

**Focus: fully custom mode ground-up redesign.**

- [x] **Grouped parameter sections** — all sections redesigned with dense `<table>` layouts: Geometry (3×4 cells/size/total grid), Reservoir (initial conditions table + fluid PVT table + inline perm), Wells (compact 2-row table), Rel Perm (endpoints table + inline capillary + side-by-side SVG curves), Timestep (single-row table), Gas (combined rel perm + PVT in one table), Analytical (inline controls). FilterCard dimension toggles removed from custom mode panel.
- [x] **Preset starting points** — rock-type quick-pick chips (Sandstone, Carbonate, Shale/Tight, Heavy Oil) in custom mode header. Each applies domain-appropriate defaults for porosity, permeability, viscosity, saturation endpoints, capillary pressure. Defined in `reservoirPresets.ts`.
- [x] **Validation guidance** — proactive advisory warnings for low permeability (<0.1 mD), high mobility ratio (>50), large grid (>50k cells), very small timestep (<0.01 d). Wired through existing `ValidationWarning` system and `WarningPolicyPanel`.
- [x] **Custom-mode default well layout** — when well coordinates are implicit, the producer now defaults to the opposite `j` boundary (`ny - 1`) instead of sharing the injector edge. This avoids misleading 2D floods where the `Jmax` wall appears to lag due to the default corner-to-edge layout rather than an indexing bug.

**Postponed (revisit after custom mode lands):**

- **Clone-and-edit flow** — cloning scenario params into custom mode may be confusing. Revisit after per-scenario customisation is designed.
- **Per-scenario customisation** — allow parameter overrides *within* a predefined scenario without switching to full custom mode. Depends on the grouped layout being stable first.
- **Save/load custom configurations** — persist named custom scenarios in localStorage; export/import as JSON. Not needed initially.

### 2B. Compact Input Layout

- [x] Reduce default section padding and vertical spacing — all sections use `px-2.5 py-2` instead of `p-3`+`space-y-2`; panel wrapper uses `space-y-1 px-1 py-1.5`
- [x] Convert overly tall input groups into compact dense tables — replaced card grids, stacked label+input pairs with inline `<table>` rows
- [x] Tighten margins — Collapsible sections, table cells, checkbox labels all use minimal padding; summary text removed from section headers (data is in the tables)

### 2C. Theme Refresh

- [ ] Replace near-black dark / flat-white light surfaces with deliberate working themes
- [ ] Improve panel contrast, content focus, and data-first visual balance

### 2D. Unify Chart & Output Architecture

- [ ] Consolidate chart-shell header and expansion-state wiring across `RateChart` and `ReferenceComparisonChart`
- [ ] Finish shared panel/x-axis selection across both chart types
- [ ] Extract output selection view model from `App.svelte` — one typed helper returning the active run/result payload shared by charts, 3D, and analytical components

---

## Phase 3 — Gas Scenarios (Immiscible)

Goal: promote gas injection and solution gas drive from experimental to production-ready with analytical references. Still immiscible (constant PVT); no dissolved-gas tracking yet.

### 3A. Fix Physics (prerequisite — Phase 1A must land first)

All three-phase correctness fixes from Phase 1A are prerequisites.

### 3B. Gas-Oil Buckley-Leverett Analytical

- [x] Extend `fractionalFlow.ts` to support gas-oil displacement: `f_g(S_g)` using gas relative permeability (Corey) and gas/oil viscosity ratio
- [x] Welge tangent construction for gas-oil system — shock front saturation, breakthrough PVI
- [x] Wire as analytical overlay for `gas_injection` scenario
- [x] Add sensitivity dimensions to `gas_injection`: mobility ratio (μ_g/μ_o), S_gc, permeability, grid convergence
- [x] Validate: simulator gas breakthrough PVI vs analytical for favorable and adverse gas mobility

### 3C. Solution Gas Drive Scenario

- [x] Define `gas_drive` base params: initially undersaturated oil with gas saturation above S_gc, BHP below bubble point
- [x] Add sensitivity dimensions: initial gas saturation, oil viscosity, permeability
- [x] Note: without Rs(P) tracking (Phase 4), solution gas drive is simulated as immiscible depletion with free gas — qualitatively useful but not quantitatively accurate for below-bubble-point behavior

### 3D. Gas Material Balance Diagnostics

- [x] Add p/z diagnostic output panel — plot cumulative gas produced vs (P_i − P)/z for each timestep
- [x] Straight line = depleting gas reservoir; curvature = aquifer influx or phase change
- [x] Requires z-factor correlation (even simple c_g ≈ 1/P) — add as configurable option
- [x] High diagnostic value for validating gas simulation physics

---

## Phase 4 — Black-Oil PVT (Volatile Oil)

Goal: upgrade from immiscible constant-PVT to full black-oil model with pressure-dependent fluid properties. This is the largest physics extension and unlocks volatile oil, solution gas drive with dissolved gas, and gas cap behavior.

### 4A. PVT Correlations

- [x] **Rs(P) — solution gas-oil ratio**: Standing (1947) or Vazquez-Beggs (1980) correlation. Rs decreases as P drops below bubble point P_b; Rs = Rs_max above P_b.
- [x] **Bo(P) — oil formation volume factor**: Standing or Glaso (1980). Bo increases with Rs (swelling); above P_b, Bo decreases slightly with pressure (compression).
- [x] **Bg(P) — gas formation volume factor**: ideal gas law corrected by z-factor. Bg = (P_sc × z × T) / (P × T_sc).
- [x] **μ_o(P) — oil viscosity**: Beggs-Robinson (1975) or Vasquez-Beggs. Viscosity decreases with dissolved gas; increases sharply below P_b as gas liberates.
- [x] **μ_g(P) — gas viscosity**: Lee-Gonzalez-Eakin (1966) correlation. Varies with pressure and temperature.
- [x] **z-factor**: Standing-Katz chart or Hall-Yarborough (1973) correlation for real gas compressibility.
- [x] **PVT table structure in Rust**: `PvtTable` struct with interpolation; replace constant `c_o`, `c_g`, `mu_o`, `mu_g` with pressure-dependent lookups.
- [x] **Black-oil pressure solver stabilization** — table-derived `c_o` now falls back to the base positive compressibility when the saturated `B_o(P)` slope would otherwise make the simplified IMPES pressure accumulation negative. Fixes severe slowdowns / PCG non-convergence for high-bubble-point volatile-oil and condensate custom presets.
- [x] **UI for PVT**: bubble-point pressure input; API gravity and gas specific gravity for correlation-based PVT; option to input tabular PVT directly.
- [x] Extract `FluidPropertiesSection` out of `ReservoirSection.svelte` to declutter the inputs.
- [x] In Black-Oil PVT mode, add a graphical preview chart (utilizing Chart.js via `ChartSubPanel`) alongside the tabular data for clear visualization of PVT curves vs Pressure.
- [x] Phase 4D: Split monolithic reservoir presets into independent `ROCK_PRESETS` and `FLUID_PRESETS` for improved Custom Mode UI flexibility with Black-Oil testing.

### 4B. Bubble-Point Tracking & Phase Split

- [x] Track dissolved gas per cell: `Rs_cell(P_cell)` — gas comes out of solution when local pressure drops below P_b
- [x] Phase split logic: if `P < P_b`, compute `Rs(P)` and liberate excess gas → increase `S_g`
- [x] Secondary gas cap formation: cells that drop below P_b develop free gas saturation even if initially `S_g = 0`
- [x] Gas re-dissolution: if pressure rises above P_b (due to injection), gas dissolves back into oil
- [x] Modify accumulation term in `step.rs`: use time-dependent Bo, Bg when computing compressibility and phase volumes

### 4C. Updated Pressure Equation

- [x] Accumulation: `(V_p / dt) × [c_t(P) × S_phases + ...]` with pressure-dependent compressibility — `get_c_o(p)` and `get_c_g(p)` use PVT table interpolation with finite-difference approximation (dP = 1 bar)
- [x] Transmissibility: mobility uses pressure-dependent viscosity from PVT lookup — `get_mu_o(p)`, `get_mu_g(p)` called per-cell in `phase_mobilities()` and `phase_mobilities_3p()`
- [x] Well model: PI uses local PVT properties via `total_mobility(id)` / `total_mobility_3p(id)` updated before each pressure solve; producing GOR tracked as (free gas / Bg + dissolved gas Rs × oil SC) / oil SC and reported in `TimePointRates.producing_gor`; surface-volume conversion uses pressure-dependent Bo, Bg at well cell pressure

### 4D. Volatile Oil Analytical References

- [x] **Hyperbolic decline (Arps)** — Arps b-parameter (0–1) in `depletionAnalytical.ts`: exponential (b=0), hyperbolic (0<b<1), harmonic (b=1). New `dep_arps` scenario with 5-layer commingled reservoir and b/contrast sensitivities. `dep_decline` renamed to "Fetkovich Decline (Oil)".
- [ ] ~~**Fetkovich type-curve matching**~~ — _Long-term / deferred. Value unclear: the Arps b-parameter sensitivity already lets users visually match decline behaviour against the layered simulation. Full type-curve overlay (transient + decline stems with r_eD) requires constant-terminal-pressure radial flow solution (Ei/Bessel) — significant analytical work for marginal incremental insight in this context. Revisit if demand arises._
- [x] **Material-balance equation** — Havlena-Odeh (1963) in `materialBalance.ts`: computes F, E_o, E_g, E_fw, E_t, N_mbe and drive indices per timestep. Supports constant and black-oil PVT (Standing Bo/Rs, Hall-Yarborough z-factor). MBE OOIP ratio diagnostic curve added to depletion chart presets (secondary y-axis, default hidden). 6 unit tests covering volumetric OOIP, constant-PVT depletion, drive index normalization, gas cap ratio, black-oil PVT, and convergence.
- [x] **Drive-mechanism indicator** — three drive index curves (compaction, oil expansion, gas cap) added to depletion diagnostics panel from MBE computation. Fractional values summing to 1.0 on secondary y-axis. Grouped under "Drive Indices" legend section, default hidden.

### 4E. Gas Cap Scenarios

- [ ] **Primary gas cap** — initial free gas zone above oil column. Configure via gas-oil contact (GOC) depth and initial gas saturation profile. Analytical: Schilthuis (1936) material balance with gas cap ratio `m`.
- [ ] **Gas cap expansion** — as oil zone depletes, gas cap expands downward. Analytical: track gas-cap movement via material balance; compare vs simulation gas front position.
- [ ] **Secondary gas cap** — forms when undersaturated oil drops below bubble point. No classical analytical solution; compare simulation against material-balance-predicted cumulative gas liberation.
- [ ] **Gas cap blowdown** — producing from gas cap after oil zone depleted. Simple volumetric depletion with p/z analysis.

### Known Limitations & Design Notes

1. **Hardcoded undersaturated c_o = 1e-5 /bar** — `generateBlackOilTable()` in `physics/pvt.ts` and `evaluateBlackOilPvt()` in `materialBalance.ts` both use a fixed `c_o = 1e-5 /bar` for undersaturated Bo extrapolation (`Bo = Bo_pb × exp(−c_o × ΔP)`). This is the standard oilfield default but does not reflect the scenario's actual fluid compressibility. A correlation-based value (e.g. Vasquez-Beggs) could be substituted if more accuracy is needed above the bubble point. The two locations must stay in sync.

2. **Saturated-region c_o fallback in pressure solver** — In `lib.rs::get_c_o()`, below the bubble point Bo *increases* with pressure (more gas dissolves → oil swells), so the finite-difference derivative `(−1/Bo) dBo/dP` is negative. A negative accumulation term would destabilise the IMPES pressure solve (PCG non-convergence), so the code falls back to the base positive `c_o`. This is physically correct: the volumetric effect of gas dissolving/liberating is already handled by the phase-split Rs-tracking logic in `step.rs`, so omitting saturated-oil swelling from the pressure accumulation avoids double-counting.

### 4F. Validation

- [ ] **SPE comparative solution** benchmarks (SPE1, SPE3) for black-oil validation
- [ ] Grid-convergence study for volatile-oil depletion: verify Bo, Rs, Sg profiles converge with refinement
- [ ] Material-balance closure: cumulative production should match OOIP × recovery factor from analytical MB

---

## Phase 5 — Advanced Analytics & Sweep Methods

Goal: upgrade analytical reference methods beyond first-order approximations.

### 5A. Stiles Method (Recommended Next Sweep Upgrade)

- [ ] Implement Stiles (1949) layer-by-layer BL integration in `sweepEfficiency.ts` (~100 lines)
- [ ] Eliminates the local-PVI approximation (largest current sweep error)
- [ ] Sorts layers by permeability; applies independent BL displacement to each; sums oil production
- [ ] Exactly satisfies material balance by construction
- [ ] Keep current local-PVI method as fast baseline / teaching mode

### 5B. Warren-Root Vertical Sweep Upgrade

- [ ] Add Kv/Kh-aware blending between DP (zero cross-flow) and perfect communication (E_V → 1)
- [ ] Simple approach: `E_V_adjusted = E_V_DP × (1 − f(Kv/Kh)) + 1.0 × f(Kv/Kh)` where f is a monotonic blend function
- [ ] Resolves known mismatch where simulator (nonzero Kv) always exceeds DP analytical for vertical sweep

### 5C. Multi-Method Comparison Framework

- [ ] Selectable analytical sweep methods per scenario: local-PVI, Stiles, stream-tube
- [ ] Side-by-side comparison views: show how method choice changes RF, breakthrough timing, sweep penalty
- [ ] Compact flow-unit abstraction for representing random areal heterogeneity analytically

### 5D. Extended Pattern Correlations

- [ ] Craig tables for other well patterns: nine-spot, seven-spot, inverted five-spot, line drive
- [ ] User-selectable pattern geometry in sweep scenarios
- [ ] Line-drive: E_A = 1 in displacement direction, use BL directly

---

## Phase 6 — Multi-Case Inspection & Data I/O

Goal: make sensitivity studies inspectable beyond charts; enable data portability.

### 6A. Multi-Case 3D Inspection

- [ ] Case selection/switching for the 3D view across sensitivity runs
- [ ] Comparison awareness in saturation profile and compact summary cards
- [ ] Synchronized selected case across summary, chart, and 3D inspection

### 6B. Summary Statistics Panel

- [ ] OOIP, pore volume, recovery factor, average pressure/saturation, water cut, VRR
- [ ] Visible for both single runs and sensitivity sweeps
- [ ] Cross-case comparison table for sweep results

### 6C. Data Export/Import

- [ ] Structured scenario export/import (JSON)
- [ ] CSV/JSON export of simulation results and benchmark summaries
- [ ] Report export with plots and key metrics

### 6D. Cross-Section Viewer

- [ ] i/j/k slice viewer for property inspection in the 3D view
- [ ] Sw profile plot evolution and tighter 3D companion integration

---

## Phase 7 — Extended Physics & Well Models

Goal: advanced reservoir modeling features.

- [ ] Multi-well patterns (5-spot, line-drive, custom placements)
- [ ] Well schedule support (rate changes, shut-ins, re-completions over time)
- [ ] Aquifer boundary conditions (Carter-Tracy or Fetkovich aquifer model)
- [ ] Per-cell or per-layer porosity variation
- [ ] Per-cell initial water saturation / transition-zone initialization (J-function or specified)
- [ ] Non-uniform cell sizes and local grid refinement
- [ ] Horizontal or deviated well model with generalized Peaceman PI

---

## Backlog (Low Priority / Nice-to-Have)

- [ ] Benchmark trend tracking across CI runs
- [ ] Multi-chart synchronized zoom/pan
- [ ] Responsive/mobile chart and 3D layout improvements
- [ ] Phase relative permeability / capillary curve visualization (interactive kr/Pc editor)
- [ ] Undo/redo for parameter changes
- [ ] Per-cell capillary pressure variation and capillary hysteresis
- [ ] Leverett J-Function capillary scaling
- [ ] Grid-convergence study preset family
- [ ] A/B run comparison overlays
- [ ] Relative error (%) diagnostic curves
- [ ] Uncertainty and sensitivity batch runner beyond curated benchmark sensitivities
- [ ] Benchmark acceptance policy refresh — add tighter validation tier (~5%) alongside current 25–30% regression guards
- [ ] **Defer geometry source-of-truth redesign** — keep the current explicit `(nx, ny, nz) + (cellDx, cellDy, cellDz)` model for now. Revisit much later, only if custom-mode redesign or future non-uniform-grid work justifies the extra resolver/helper complexity.

---

## Known Constraints

- Three.js pinned at 0.183.2 — do not upgrade casually
- WASM requires `wasm32-unknown-unknown` target
- Worker ↔ UI communication: structured cloning only (no functions or class instances)
- Stop latency = one WASM step duration; for large grids (>30×30×20), single step may take >500 ms. Options: SharedArrayBuffer+Atomics, worker termination+recreate, or Rust-side callback hook.

---

## Completed

- **Mapped analytical overlays by run axis** (2026-03-20): Shared analytical waterflood/depletion overlays now remap per completed run on simulation-derived axes, preview/pending analytical curves stay hidden until mapping data exists, and the comparison chart shows an x-axis advisory when analytical overlays depend on run histories.
- **Live convergence warning updates** (2026-03-20): Convergence notices now appear on first hit, persist while later steps recover, update live with affected reference runs/counts, and still reset cleanly for a fresh non-reference run.
- **3D ternary blend + gas defaulting** (2026-03-20): Added a separate ternary saturation color-blend mode to `ThreeDView`, switched the ternary mixing from straight RGB to perceptual OKLab blending, fixed legend redraw immediately when switching both into and out of ternary mode, aligned gas saturation to the oil-style hue ramp with a `Swc`-anchored legend range, brightened the phase anchors, compacted the triangle legend, removed its border/padding/labels, and the app now defaults the 3D selector to gas saturation for gas-injection contexts.
- **3D gas saturation + waterflood default** (2026-03-20): `ThreeDView` now supports gas saturation as a selectable scalar, and the app auto-selects water saturation when the active 3D context switches into waterflood or sweep runs.
- **3D output provenance label** (2026-03-20): `ThreeDView` now renders the passed `sourceLabel`, removing the Svelte unused-export warning and keeping output provenance visible in the Spatial View header.
- **B1–B10** (2026-03-07): Benchmark modernization — family registry, explicit reference policy, sensitivity sweeps, benchmark-specific chart defaults, benchmark docs.
- **F1–F3** (2026-03): Unified Inputs/Run/Outputs shell; family-first navigation; case library; reference execution card; warning policy unified; case disclosure cards; compact Run Set selector; master-detail Results layout; IBM Plex Sans/Mono typography; semantic utility classes.
- **Store refactor** (2026-03-17): Converted `createSimulationStore()` to Svelte 5 class with `$state` fields. Fixed silent bug: 13 three-phase parameters declared as `$state` but never exposed in `parameterState` accessor.
- **Simplification Refactor Steps 1–6** (2026-03-17): `scenarios.ts` + `ScenarioPicker.svelte` replace `ModePanel.svelte` + 4-layer case-library navigation.
- **Run Controls UX** (2026-03-19): Stop button shows "Stopping…" immediately; `stopPending` state added.
- **Sweep Efficiency** (2026-03-19): Craig (1971) areal sweep, Dykstra-Parsons (1950) vertical sweep, volumetric product. `SweepEfficiencyChart.svelte`. Four new sweep scenarios.
- **S1 — Scenario/Sensitivity Architecture Redesign** (2026-03-19): Consolidated 18 scenarios → 8 canonical scenarios with multi-dimension sensitivity selection.
- **Analytical preview lifecycle** (2026-03-19): Multi-variant analytical preview before runs; mid-sweep color continuity; depletion per-variant analytical.
- **F10 — Simulation Sweep Efficiency** (2026-03-19): E_A_sim, E_V_sim, E_vol_sim computed from grid saturation. Analytical RF = E_vol × E_D_BL.
- **Chart Legend & Cases Selector Redesign** (2026-03-20): Dual-line indicator (analytical/simulation); sub-panel legends with "Simulation (N)" / "Analytical solution (N)".
