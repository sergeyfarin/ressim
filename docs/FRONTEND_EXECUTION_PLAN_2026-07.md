# Frontend Execution Plan — Waves 0–3 (2026-07)

Date: 2026-07-16. Status: active execution plan. This operationalizes the reprioritization agreed 2026-07-07/16 (fixes → product → scoped refactor; UX polish deferred). It consolidates the actionable frontend sequence from `ROADMAP.md` P2–P4, `docs/COMPARISON_TOOLBOX_REVIEW_2026-07-01.md` §5, and `docs/CASE_LIBRARY_ROADMAP.md` Tiers 5–6 into one ordered plan. FIM/IMPES solver work is explicitly out of scope (separate, currently paused workstream — do not touch `src/lib/ressim/src/` internals from this plan).

## Wave 0 — Guardrails (DONE 2026-07-16)

CI Rust gate (`impes` bucket), 3D-view named-bug verification (all four already fixed), shared `c_o` constant + parity test, root/doc hygiene. Record: TODO.md "Discovered issues" section.

---

## Wave 1 — OPM artifact pipeline completion (the unlock)

**Goal:** real OPM Flow numbers reach the UI for `wf_bl1d` and `spe1_gas_injection`. Everything strategic (acceptance criteria, Tier 5 reference bands, all Tier 6 pre-run exhibits) queues behind this. Python + frontend-test work only.

**Current gap (verified in source):** `tools/opm_flow/opm_flow_tool/artifacts.py::build_artifact()` unconditionally writes `"series": [], "status": "deck-ready"`. The frontend renders OPM series only when `status === 'parsed'` (`opmFlowArtifacts.ts::getOpmFlowPublishedReferenceSeries`), so no OPM number has ever reached a chart.

### Steps

1. **W1.1 — Summary parser (agent-doable, no `flow` needed).**
   - Add `RUNSUM` + `SEPARATE` to both deck `SUMMARY` sections in `cases.py` so `flow` emits a text `.RSM` alongside binary output (output-only keywords; physics unchanged; deck hash changes — fine, artifacts are regenerated anyway).
   - New `tools/opm_flow/opm_flow_tool/summary.py`: hand-rolled `.RSM` reader → `{mnemonic: [(time_days, value)]}`. Zero new runtime deps (project rule). Read unit strings from the file itself — never assume (skill warning). Fallback decision if `.RSM` layout proves fragile across flow versions: take `resdata` as a dependency of this offline tools package only (never the app) — decide after W1.3's first real run.
   - `build_artifact()` gains a `run_root` lookup: if `tmp/opm-flow-runs/<case>/` contains a parseable summary, populate `series` (map mnemonics → the artifact's `panelKey`/`curveKey` conventions from `supported_curves`), set `status: 'parsed'`, record `flowVersion`; else keep today's `deck-ready` stub behavior.
   - Unit tests (`uv run pytest`, dev-dep only) against a committed fixture `.RSM` under `tools/opm_flow/tests/fixtures/`.
2. **W1.2 — CLI/docs**: `build-artifacts` picks up the run directory automatically; update `tools/opm_flow/README.md` and the `opm-reference-pipeline` skill's "Known gap" section when closed.
3. **W1.3 — Regenerate real artifacts (USER MACHINE — `flow` required, proven working there 2026-04-06).** Exact commands:
   ```bash
   uv run --directory tools/opm_flow python -m opm_flow_tool.cli run-flow wf_bl1d
   uv run --directory tools/opm_flow python -m opm_flow_tool.cli run-flow spe1_gas_injection
   uv run --directory tools/opm_flow python -m opm_flow_tool.cli build-artifacts all
   ```
   Commit the regenerated `src/lib/catalog/opm-flow-results/*.json` with flow version + deck hash provenance intact.
4. **W1.4 — Scenario↔artifact integrity test** (extend `src/lib/catalog/opmFlowArtifacts.test.ts`):
   - every scenario `opmFlowReferenceArtifactKeys` entry resolves to a bundled artifact whose `scenarioKey` matches;
   - every artifact's `scenarioKey` resolves to a registered scenario;
   - a `PARSED_BASELINE` list of case keys that must never regress from `parsed` back to `deck-ready` (starts empty; add `wf_bl1d`, `spe1_gas_injection` in the same commit as W1.3's artifacts — keeps CI green before the real run).
5. **W1.5 — Verify rendering**: unit test `getOpmFlowPublishedReferenceSeries` against a parsed fixture; visual check that both scenarios show dashed OPM curves in comparison charts.

**Exit criteria:** parser unit-tested against fixture; both artifacts committed with `status: 'parsed'` and non-empty series; integrity tests in CI; OPM curves visible in the app.
**Estimate:** 2–4 agent-days + one user session with `flow`.
**Out of scope (later waves):** decks for `gas_injection`/`gas_drive`, acceptance tolerance bands (Phase C), SPE1 tabular-SCAL swap.

---

## Wave 2 — Three insight scenarios (visible product wins; parallel-safe with Wave 1)

**Goal:** first "decision-insight" content in the showcase (roadmap Tier 5.1–5.3). All three are pure scenario-file work on existing panels — no engine change, no chart-architecture exposure. Follow the `add-scenario` skill checklist per case; README inventory row each.

### W2.1 — `dep_nct` "Matched history, different reserves" (Tier 5.1)

- Base: clone the `dep_decline`/`dep_arps` family (constant-PVT depletion, BHP producer, PSS-dominated run length).
- Dimension `nct_ambiguity`, 3 variants with equal `N·c_t` product but different splits: vary pore volume (porosity and/or dz) against compressibility (`c_o`/`rock_compressibility`) inversely. Same pressure/rate history, different RF = Np/N by construction. `affectsAnalytical: true` (tau changes inputs), `analyticalOverlayMode: 'per-result'`.
- Panels: pressure + recovery + existing MB diagnostics (Havlena-Odeh via `analyticalParamAdapters.ts` shows exactly the ambiguous quantity).
- Extra test: a targeted vitest asserting the variants' *analytical* pressure curves coincide (≤0.1% spread) while RF ceilings differ — cheap pure-TS check that locks in the teaching point.
- Honest-physics note: under BHP control the early transient depends weakly on c_t; run long enough that PSS dominates and state the residual mismatch in the scenario description.

### W2.2 — `wf_tornado` "The tornado plot lies" (Tier 5.2)

- New 2D vertical cross-section waterflood (e.g. `nx=50, ny=1, nz=10`, `permMode: 'perLayer'`, gravity on).
- Dimension `interaction`, 4 variants: base / kv↑ alone (`uniformPermZ` or per-layer kz) / density-contrast↑ alone (`rho_o`↓) / both. Single changes move RF little; the pair produces a Dietz gravity tongue and a large RF loss at fixed PVI.
- `affectsAnalytical: false` for kv variants — the 1D BL overlay is blind to kv, which is itself the lesson; `analyticalOverlayMode: 'shared'`. References: Dietz (1953); Shook, Li & Lake (1992).
- **Main risk: physics tuning.** The gravity number must be large enough to tongue within a browser-scale run. Budget a tuning pass (adjust rate/length/Δρ/kv range) before finalizing variant values; verify in the 3D view that the tongue is visible (`default3DScalar: 'saturation_water'`).

### W2.3 — `dep_pvt` "Two fluid models, one calibration point" (Tier 5.3)

- Black-oil blowdown through the bubble point, SPE1-style tabular input path (`pvtMode: 'black-oil'` + explicit `pvtTable` — already supported end-to-end via `buildCreatePayload.ts`).
- Variants ship two full `pvtTable`s that honor the same Pb and Rs(Pb): one from `generateBlackOilTable()` correlations, one perturbed "lab" table (different saturated-Bo curvature / undersaturated trend). Both are "calibrated" at the same point; GOR and RF diverge in blowdown.
- Analytical: keep the depletion overlay as context only or `none` — the reference here is the *contrast between variants*, stated honestly per the `gas_drive` precedent. OPM decks for both tables are the natural Wave-4/Phase-C follow-up (`cases.py` additions) once Wave 1 lands.

**Exit criteria per case:** registered in `scenarios.ts`; contract/lint/typecheck/test suites green; visual run confirms variants move in the physically expected direction; README row added.
**Estimate:** ~1–1.5 days each; W2.2 carries the tuning risk.

---

## Wave 3 — Scoped chart/catalog consolidation (enabler for Tier 5/6 chart features)

**Goal (narrow, not a rewrite):** new comparison features (history/forecast divider E5, pre-run class E7, ensemble charts) can land purely in the scenario-first path; `benchmarkCases.ts` and `caseCatalog.ts` are deleted. Explicit non-goals: no Chart.js re-architecture, no visual redesign, no `RateChart` live-path changes.

**Verified dependency map (2026-07-16), which dictates the order:**
- Type leakage: `BenchmarkFamily` type → `runModel.ts`, `scenarioChartModel.ts`, `referenceChartConfig.ts`, `buildChartData.ts`, `ReferenceComparisonChart.svelte`; `BenchmarkVariant` → `ReferenceExecutionCard.svelte`; reference/metric/criterion types → `scenario/runModel.ts`.
- Store coupling: `parameterStore` (`catalog`), `navigationStore` (`catalog` + `BenchmarkFamily`), `runtimeStore`, `phase2PresetContract` (`CaseMode`/`ToggleState`) all import from `caseCatalog.ts`.
- `caseLibrary.ts` and `caseCatalog.ts` both build on `benchmarkCases.ts`.

### Steps (each its own commit/PR, each gated on `pnpm run validate` + visual run)

1. **W3.1 — Type ownership move (pure types, zero behavior).** Relocate the type unions the new path actually needs (`BenchmarkReferenceDefinition`, `BenchmarkComparisonMetric`, `BenchmarkBreakthroughCriterion`, `BenchmarkXAxisKey`, panel/style keys) from `benchmarkCases.ts` into the scenario layer (`src/lib/scenario/referenceTypes.ts` or extend `runModel.ts`); `benchmarkCases.ts` temporarily re-exports. Kill the `BenchmarkFamily` type dependency in `scenarioChartModel.ts`/`referenceChartConfig.ts` by narrowing to what those files actually read.
2. **W3.2 — Store decoupling (highest regression risk).** Move what the three stores + `phase2PresetContract` still consume from `caseCatalog.ts` (facet catalog, `CaseMode`, `ToggleState`) into a slim scenario-first module; delete unused facet paths. The architecture tests (`modePanel*.test.ts`, `no-direct-chart-datasets-access.test.ts`, `ratechart-usage.test.ts`) are the spec — if a behavior isn't covered, add the test before moving it.
3. **W3.3 — Runtime data migration.** Fold what `benchmarkCases.ts` still provides at runtime (benchmark entries, per-family reference definitions, display defaults) into scenario definitions + `benchmark-case-data/` JSONs; repoint `caseLibrary.ts` at the scenario registry.
4. **W3.4 — Render-path ownership.** ~~Split `buildChartData.ts` along per-family seams~~ **RESCOPED 2026-07-17 after reading the file in full.** The premise was stale: the per-family physics (BL/depletion/gas-oil overlays, sweep panels, axis handling) is *already* extracted into `referenceOverlayBuilders.ts`/`sweepPanelBuilder.ts`/`axisAdapters.ts`/`analyticalParamAdapters.ts` — this must have landed between the 2026-07-01 comparison review and now, without the review being re-run. What's actually left in `buildChartData.ts`'s ~1,190-line `buildReferenceComparisonModel()` is one cohesive (if long) orchestration function — mode resolution → per-result derived series → per-analytical-method curve-assembly branches (BL/depletion/gas-oil-bl × shared/per-result/pending-preview) → sweep panels → published overlays — plus a lot of repetitive `CurveConfig` boilerplate per branch, not unextracted physics. Similarly `ScenarioChart.svelte`'s routing between `ReferenceComparisonChart` (multi-run) and `RateChart` (single live run) is an intentional split by data shape, not a broken delegation. **Completed instead:** repointed `buildChartData.ts` and `ReferenceComparisonChart.svelte`'s `BenchmarkFamily` type import from `../catalog/benchmarkCases` to `../scenario/referenceTypes` (W3.1's pattern) — the substantive, safe "ownership" fix, since it severs the last type-level tie to the archived legacy system. **Not done, and not recommended right now:** restructuring the 1,190-line orchestrator into smaller files. It's a legitimate DRY-ing opportunity (the repetitive per-analytical-method curve-assembly could share a helper), but it's discretionary polish with real regression risk (many subtle shared/per-result/pending-preview/sweep mode interactions, no visual verification available this session) — not the "resolve a split-brain architecture" risk this Wave was chartered to close. Revisit only if E5/E7 (Wave 4) actually need to touch this function; don't restructure it speculatively.
5. **W3.5 — Deletion + doc truth.** `benchmarkCases.ts`/its 5 case-data JSONs archived (not deleted) in W3.3 per user decision — see `.archive/README.md`. `caseCatalog.ts` and `ReferenceExecutionCard.svelte` are *not* fully orphaned (Custom Mode facet logic in the former is live; the latter's orphan status was already true and unrelated to this Wave) — leave both in place. Remaining doc-truth work: update the `frontend-architecture` skill's "three generations" table, `docs/DOCUMENTATION_INDEX.md`, and mark `ROADMAP.md` P3.1 done to reflect the above.

**Exit criteria (revised):** `grep -r "from '.*benchmarkCases'" src/` returns only the stub's own internal type re-export and files that need the *live* legacy-family functions (none remain, since the system is archived) — confirmed. All suites green (typecheck/lint/vitest). Visual parity spot-check deferred to a session with browser access, per the standing constraint this whole Wave operated under.
**Actual effort:** W3.1–W3.3 done in one extended session (2026-07-16/17); W3.4 rescoped down after re-reading the file rather than trusting the original 1,614-line assumption.

---

## Standing decision points

1. **`.RSM` vs `resdata`** — resolved at first real `flow` run (W1.3); prefer zero-dep `.RSM`.
2. **W2.2 tuning budget** — if the gravity tongue can't be made convincing at browser scale, demote the case to a kv/Pc-crossflow interaction pair (same lesson, different physics) rather than shipping a weak contrast.
3. **Wave 4 preview (not planned here):** E5 divider + E7 pre-run class + SPE11 exhibit; SPE1 tabular SCAL + acceptance bands vs OPM; then E1 per-cell perm → Tavassoli. Plan after Wave 3 lands.

---

## Wave 4 — Product capabilities (E5, E7, E1) — DONE 2026-07-19

Scoped from the preview above to the three capability features; **SPE11 deferred** (its value is the real published 18-simulator spread, which must be sourced/curated, not fabricated). SPE1 tabular SCAL / acceptance bands also not in this wave. Plan file: `~/.claude/plans/cryptic-honking-garden.md`. Full per-item records in `TODO.md`.

- **E5 — history/forecast divider.** Optional `Scenario.historyWindow`; inline Chart.js plugin (`historyDivider.ts`, no new dep) shades the history region + dashed divider, resolved against the active x-axis. Demonstrated on dep_nct (day-12 boundary).
- **E7 — pre-run artifact scenario class.** `capabilities.runMode: 'live-worker' | 'prerun-artifacts'`; pre-run scenarios skip the worker (guarded in runtimeStore via `nav.isPrerunScenario`), replace RunControls with a precomputed note, hide 3D, keep params read-only (already true for predefined scenarios), and render their bundled artifact as **primary** content (`getOpmFlowArtifactSeriesByKeys` + `primary` flag → solid curves). Demonstrator `wf_bl1d_opm`. Fan-*bands* deferred to SPE11.
- **E1 — per-cell permeability.** Additive `setPermeabilityField` wasm setter (no solver logic touched; wasm rebuilt + `pkg/` committed); `permMode: 'field'` + `fieldPermX/Y/Z` plumbed through payload/worker. Capability exposed; no consuming scenario yet.

Validation each item: typecheck + lint + vitest (654 pass) + `pnpm run build`; E1 also `validate-solver-coverage.sh impes` (green). **No live-browser check available all session — `pnpm run dev` spot-checks recommended** (E5 divider on dep_nct; E7 wf_bl1d_opm: no run controls, 3D off, solid OPM curves, "Pre-run" badge).
