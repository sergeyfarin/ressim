# Comparison Toolbox Review — Findings and Forward Plan

Date: 2026-07-01
Status: current planning document. Supersedes no prior doc; consolidates findings that were scattered across `PLAN.md`, `docs/REFACTOR_PLAN.md`, `docs/OPM_FLOW_MINIMAL_MAPPING.md`, `docs/2.md`, `docs/20260426.md`, `ROADMAP.md`, and `TODO.md` into one place, and proposes what to do next.

Scope of this review: frontend architecture, comparison/chart logic, scenario catalog, OPM Flow integration, and documentation hygiene. Per instruction, this review does **not** propose changes to the Rust/WASM physics core (`src/lib/ressim/src/`) itself, and does not touch FIM solver internals. Where FIM is mentioned, it is only in the context of product boundary and documentation load, not solver algorithm.

No code was changed while preparing this document. `node_modules` is not installed in this session, so I did not re-run `npm run typecheck` / `npm test` / `cargo test` — all "current state" claims below come from reading source files, existing docs, and git history, not from a freshly reproduced baseline. Re-run the validation suite before treating any of this as a tested checkpoint.

---

## 1. Executive summary

The frontend has quietly outgrown its own documentation. `PLAN.md` describes a scenario-first rewrite as blocked mid-flight by a sandbox bug; `docs/REFACTOR_PLAN.md` describes an 8-phase refactor with "Phase 8 ← NOW" and a last-updated date of 2026-04-07. Neither is accurate today. The most recent commit (`4384fdf`, 2026-05-20, "migrate to scenario-first model") shows the scenario-first rewrite substantially landed: `ScenarioChart.svelte`, `scenarioChartModel.ts`, `src/lib/scenario/runModel.ts`, and `spatialViewModel.ts` all exist and are wired in. `App.svelte` is 286 lines, not the ~820 lines both planning docs describe.

What has **not** landed is the legacy-layer removal both plans call for: `benchmarkCases.ts`, `caseCatalog.ts`, `ReferenceComparisonChart.svelte` (588 lines), and `buildChartData.ts` (1,614 lines) are all still present and still doing the real work — `ScenarioChart.svelte` is a 126-line wrapper that, for every predefined scenario, delegates straight to `ReferenceComparisonChart`. So there are now **three** generations of chart-building code coexisting: the legacy benchmark-family path, the "Phase 4" `buildChartData.ts` orchestrator, and the new scenario-first shell. This is the single biggest architectural liability right now — not because any one file is broken, but because a change to comparison behavior has to be reasoned about across all three layers at once.

Separately, and this is the concrete, high-leverage finding for the "compare against OPM Flow for gas" goal: **the OPM Flow integration is real but incomplete at exactly one step.** `tools/opm_flow/` is a working Python CLI (`generate-deck`, `run-flow`, `build-artifacts`) that can drive the actual `flow` binary. The repo root even contains `SPE1CASE1.DBG`, a genuine OPM Flow debug log from a real local run on 2026-04-06 — proof the pipeline has been exercised end-to-end at least once. But `build_artifact()` in `tools/opm_flow/opm_flow_tool/artifacts.py` never parses `flow`'s summary output; it unconditionally writes `"series": []` and `"status": "deck-ready"`. Both committed artifacts (`wf_bl1d.json`, `spe1_gas_injection.json`) are stuck at that stub status. There is no OPM case defined at all for `gas_injection` or `gas_drive` (the two three-phase, non-black-oil gas scenarios) — only the black-oil `spe1_gas_injection` and the waterflood `wf_bl1d` have decks. **This is the one missing link between "we have OPM tooling" and "gas scenarios show a real OPM comparison."**

Recommended path, in priority order:

1. Finish the OPM summary parser (Python side only) so `spe1_gas_injection` gets a real, non-stub OPM comparison. This is scoped, testable without touching Rust/WASM or FIM, and directly answers "interim solution: compare gas scenarios against precalculated OPM Flow."
2. Add OPM decks/cases for `gas_injection` (three-phase, no black-oil PVT — closest thing OPM has is still a black-oil deck with `DISGAS`/`VAPOIL` off, or an oil-water-gas immiscible-style deck) so all three gas-flavored scenarios have a ground truth that isn't "wait for FIM."
3. Retire the legacy benchmark layer (`benchmarkCases.ts`, `caseCatalog.ts`, the `ReferenceComparisonChart`/`buildChartData.ts` duo) in favor of the scenario-first path that already exists, closing the architecture gap documented (but not finished) in `docs/REFACTOR_PLAN.md` Phase 8 and `ROADMAP.md` Priority 3.1.
4. Once OPM ground truth exists for gas cases, redefine "gas scenario acceptance" against OPM rather than against FIM. This turns FIM convergence work into something you can pick up later without it gating product readiness — which is already the stated intent in `docs/FIM_DEFERRED_BACKLOG.md`, just not yet backed by real comparison data.
5. Consolidate documentation: the FIM working-note pile (19 of 36 files in `docs/`) is disproportionate to FIM's current product priority, and `TODO.md` (666 lines) has drifted from its own stated "keep it short" policy.

Details, evidence, and a phased plan follow.

---

## 2. Findings

### 2.1 Frontend architecture is ahead of its own documentation

| Claim in `PLAN.md` / `docs/REFACTOR_PLAN.md` | Actual state (checked 2026-07-01) |
|---|---|
| Scenario-first rewrite blocked by a sandbox write bug; only `src/lib/scenario/` exists as an orphan module | `src/lib/scenario/runModel.ts` + test now wired into `ScenarioChart.svelte`, `scenarioChartModel.ts`, `navigationStore.svelte.ts`, `runtimeStore.svelte.ts` as of commit `4384fdf` (2026-05-20) |
| `App.svelte` ~820 lines, "still a god file" | 286 lines |
| `simulationStore.svelte.ts` — 1,995-line god object | File is now 24 lines; split into `parameterStore.svelte.ts` (647), `runtimeStore.svelte.ts` (910), `navigationStore.svelte.ts` (849) as REFACTOR_PLAN Phase 6 proposed |
| "Phase 8 ← NOW" (benchmark consolidation) | Not done: `benchmarkCases.ts` and `caseCatalog.ts` are both still present and still imported |
| `RateChart.svelte` 1,419 lines, builds curves inline | 230 lines; `buildRateChartData.ts` (634 lines) now does the building, matching Phase 5 |
| `3dview.svelte` monolith with FOV bug, `Array.isArray` bug, duplicated type | `spatialViewModel.ts` + `spatialViewModel.test.ts` now exist; did not re-audit whether every named bug is fixed — worth a focused pass, see §3 |

Net: the team (or a prior agent session) executed most of `docs/REFACTOR_PLAN.md` without updating the plan doc to say so, and separately started (and got further with) the `PLAN.md` scenario-first rewrite without ever reconciling the two documents. Treat both `PLAN.md` and `docs/REFACTOR_PLAN.md` as **historical** now — they describe a state that no longer exists. Neither is listed as "historical" in `docs/DOCUMENTATION_INDEX.md` today; both should be, or archived outright once this review is read.

### 2.2 The chart/comparison layer has three generations coexisting

Current file sizes (relevant subset):

```
src/lib/charts/ScenarioChart.svelte          126 lines   (new, scenario-first shell)
src/lib/charts/scenarioChartModel.ts          86 lines   (new)
src/lib/charts/ReferenceComparisonChart.svelte 588 lines  (Phase-4-era, still the actual renderer)
src/lib/charts/buildChartData.ts             1,614 lines (Phase-4-era orchestrator; still called by ReferenceComparisonChart)
src/lib/charts/buildRateChartData.ts           634 lines (Phase-5-era, live-run path)
src/lib/catalog/benchmarkCases.ts              ~18.7 KB  (legacy, still imported)
src/lib/catalog/caseCatalog.ts                 ~11.2 KB  (legacy, still imported)
```

`ScenarioChart.svelte` decides, per render, whether to show `ReferenceComparisonChart` (predefined scenario, comparison mode) or `RateChart` (custom mode / live mode) — this is the right shape. But `ReferenceComparisonChart` still gets its data from `buildChartData.ts`, a 1,614-line file that (per its own header comment) does BL, depletion, gas, sweep, published-reference, and preview handling all in one place — exactly the "does too much" file `docs/REFACTOR_PLAN.md` set out to eliminate in its own Phase 4/8. It was refactored once (from 2,560 lines down from `referenceComparisonModel.ts`) but not finished.

Practical consequence: adding a new reference-source type (OPM-precomputed curves) means threading it through `buildChartData.ts`'s panel assembly, not just through the new scenario-first types. The plumbing for `opm-flow-precomputed` as a `ReferenceSourceType` already exists (`opmFlowArtifacts.ts`, referenced in `buildChartData.ts` and `scenarioChartModel.ts`) — so this is wired, just resting on the older, harder-to-maintain file.

### 2.3 OPM Flow integration: real tooling, one missing step

Concrete chain of evidence:

- `tools/opm_flow/opm_flow_tool/cases.py` defines two `OpmCase` decks: `WF_BL1D` (waterflood, oil-water) and `SPE1_GAS_INJECTION` (black-oil, oil-water-gas, matches the SPE1 scenario's PVT/SCAL/geometry).
- `tools/opm_flow/opm_flow_tool/artifacts.py::build_artifact()` writes the artifact JSON but hardcodes `"series": []` and `"status": "deck-ready"` unconditionally — it never reads `flow`'s output directory (`tmp/opm-flow-runs/<case>/`, gitignored) to extract summary vectors (FOPR, FWPR, FGIR, WBHP, WGOR, etc., all already listed in `supported_curves`).
- `src/lib/catalog/opm-flow-results/wf_bl1d.json` and `spe1_gas_injection.json` are both committed with `"status": "deck-ready"` and empty `series` — i.e., no real OPM numbers have ever reached the frontend, despite the frontend having full type support for them (`OpmFlowArtifact`, `getOpmFlowPublishedReferenceSeries()` in `opmFlowArtifacts.ts`).
- Root-level `SPE1CASE1.DBG` (642 KB, plus a stray `SPE1CASE1.DBG:Zone.Identifier`) is a genuine `flow` run log from 2026-04-06 (`Flow Version = 2025.10`), proving `flow` has been run successfully at least once, locally, outside the `tools/opm_flow` pipeline's expected output directory. This file is not gitignored (`tmp/opm-flow-runs/` is, but this file sits at repo root) and isn't consumed by anything — it is a stray artifact from manual investigation, not part of the product path.
- `docs/2.md` (an uncategorized analysis note, not in `docs/DOCUMENTATION_INDEX.md`) is FIM-specific commentary that independently corroborates: OPM's SPE1 solve is ~2.5 Newton iterations/step with zero cut timesteps, vs. ResSim FIM's much rougher path — but this is about the **FIM solver**, not about IMPES or about the comparison pipeline. It is useful evidence for "why FIM parity is hard," not evidence that blocks IMPES-vs-OPM comparison work.

**Why this matters for your ask specifically:** the artifact-status field already models exactly the workflow you want (`deck-ready` → `flow-run` → `parsed` → `error`), and the frontend already knows how to render `opm-flow-precomputed` series once `status === 'parsed'` (see `opmFlowArtifacts.ts::getOpmFlowPublishedReferenceSeries`, which explicitly filters on `artifact.status !== 'parsed'`). The only missing piece is: **a summary-file parser that turns `flow`'s output (`.SMSPEC`/`.UNSMRY`, or the simpler `.RSM`/CSV summary if you export one) into `OpmFlowArtifactSeries[]`, and marks the artifact `parsed`.** That's a self-contained Python change (probably using `resdata` or `opm` Python bindings, or a minimal hand-rolled `.RSM` reader if you want zero new dependencies) plus wiring it into `build_artifact()`. It does not touch Rust, WASM, or FIM at all.

Constraint worth flagging: I don't have `flow` in this sandbox, so I can't build or test the parser end-to-end here — that step needs to happen on a machine (yours) where `flow` is installed, exactly like the run that produced `SPE1CASE1.DBG`.

### 2.4 Scenario/validation status by family — confirms the user's read is right

- **Waterflood, sweep, depletion (two-phase, no gas):** `wf_bl1d`, `sweep_areal`, `sweep_vertical`, `sweep_combined`, `dep_pss`, `dep_decline`, `dep_arps` — all have analytical references (BL/Welge, Craig, Dykstra-Parsons, Stiles, Dietz, Fetkovich, Arps, Havlena-Odeh) and are described in `README.md` as validated against those. Nothing here suggests IMPES itself is the blocker for these families — this matches "should be good to go with IMPES."
- **Gas (three-phase, no black-oil PVT):** `gas_injection` (analytical: gas-oil BL, has a real overlay) and `gas_drive` (`analyticalMethod: 'none'` — the scenario's own description says "qualitatively useful but not quantitatively accurate ... without Rs(P) tracking"). Neither has an OPM case defined in `tools/opm_flow/opm_flow_tool/cases.py`.
- **Black-oil (`spe1_gas_injection`):** has published Eclipse Case 1 reference overlays (digitized, `analyticalMethod: 'digitized-reference'`) and an OPM artifact hook, but the OPM artifact is the stub described in §2.3, and `README.md` itself says "quantitative acceptance criteria remain deferred."
- **Three-phase generally:** `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` is explicit that there is "no comparative-solution benchmark suite" and "oil-phase closure is still indirect." This is the IMPES three-phase path, independent of the FIM convergence problems — worth keeping these separate in your head: IMPES three-phase probably *runs fine*, it just has never been checked against an independent reference other than the gas-oil BL analytical (which only covers `gas_injection`, not `gas_drive` or SPE1).

Bottom line: the "gas scenarios need OPM comparison" instinct is exactly right, and it applies to all three gas-flavored scenarios (`gas_injection`, `gas_drive`, `spe1_gas_injection`), not just SPE1.

### 2.5 Documentation sprawl

`docs/` has 36 files. By rough category:

- **FIM-specific working notes/plans/audits:** 19 files (`FIM_BYPASS_AUDIT`, `FIM_CHOP_WIDEN_EXPERIMENT`, `FIM_CLEANUP_PLAN`, `FIM_CONVERGENCE_ARCHIVE_2026-03_to_2026-04-06`, `FIM_CONVERGENCE_IMPROVEMENTS`, `FIM_CONVERGENCE_WORKLOG`, `FIM_CPR_IMPROVEMENT_PLAN`, `FIM_DEFERRED_BACKLOG`, `FIM_HISTORY_2026-03`, `FIM_JACOBIAN_REUSE_INVESTIGATION`, `FIM_LINEAR_SOLVER_AUDIT`, `FIM_MIGRATION_PLAN`, `FIM_PHASE2_EXECUTION_PLAN`, `FIM_PHYSICS_TEST_PLAN`, `FIM_SLICE_A_EXTRAPOLATION`, `FIM_STATUS`, `FIM_TEST_CLASSIFICATION`, `FIM_TEST_COMPLETENESS_REVIEW`, `FIM_UPWINDING_FRONT_STABILITY`, `FIM_WIDE_ANGLE_ANALYSIS`).
- **Uncategorized / orphaned:** `docs/2.md` and `docs/20260426.md` — both look like pasted chat-analysis output, neither appears anywhere in `docs/DOCUMENTATION_INDEX.md`'s authoritative or historical tables. `docs/20260426.md` is specifically the OPM-vs-ResSim tracking-philosophy note referenced in commit `2227b28`.
- **Everything else:** ~15 files covering architecture, benchmarks, units, three-phase, solver layout/testing, SPE1.

Given `docs/FIM_DEFERRED_BACKLOG.md`'s own stated boundary — "FIM remains accessible only through explicit developer tests... do not treat FIM as part of product readiness" — having more than half the documentation directory be active FIM tuning logs is a mismatch between stated priority and actual maintenance burden. This isn't wrong (FIM work clearly happened and needs a paper trail), but it should not all sit at the top level of `docs/`, competing for attention with the docs that actually describe the shipping product.

`TODO.md` (666 lines) has the same problem at a finer grain: `FIM_STATUS.md` itself says "Keep `TODO.md` short and action-oriented" and "Put active reproductions, traces, and next hypotheses in `docs/FIM_CONVERGENCE_WORKLOG.md`," but the current `TODO.md` "Now" section contains dozens of paragraph-length, timestamped FIM micro-experiment entries (e.g. the 2026-04-11/04-12 hotspot-damping narrative reproduced in this review's research) that are exactly the kind of content that policy says belongs in the worklog file instead.

Root-level clutter, unrelated to the documentation set but worth naming: `SPE1CASE1.DBG` + `SPE1CASE1.DBG:Zone.Identifier` (stray OPM debug output, see §2.3), `image.png` (91 KB, no reference found anywhere in code or docs — likely a stray screenshot), and `ccopy.sh` (a personal Claude Code conversation-search shell helper, unrelated to the reservoir simulator product). None of these are gitignored. I did not delete anything — this is a hygiene note for you to action, not something to do silently.

### 2.6 What's genuinely solid and shouldn't be touched

- Two-phase analytical validation (BL/Welge, Craig, Dykstra-Parsons, Stiles, Dietz, Fetkovich, Arps, Havlena-Odeh) — mature, documented, has regression tests.
- The scenario contract (`Scenario`, `ScenarioCapabilities`, `analyticalDef`, `liveChartPanels`) is a good abstraction and is now mostly self-describing per scenario, which is exactly the target shape both planning docs wanted.
- `panelDefs.ts` / `curveStylePolicy.ts` — single source of truth for chart panel and styling constants, already extracted (Phase 3), and correctly still in use by both old and new chart paths.
- The `ReferenceSourceType` union (`'analytical' | 'published-reference' | 'opm-flow-precomputed' | 'simulation'`) is exactly the right shape for what you're asking for — it just needs real data behind the OPM branch.
- FIM/IMPES separation at the Rust layer (`src/lib/ressim/src/impes/`, `src/lib/ressim/src/fim/`) and the "FIM is dev-only, IMPES is product" boundary are already correctly enforced in scenario defaults (`fimEnabled: false` everywhere) and in the UI (no FIM toggle exposed). Nothing here needs to change for this plan.

---

## 3. Gap analysis (priority-ordered)

### P0 — Blocks the stated goal directly

1. **OPM summary parsing is unimplemented.** `build_artifact()` never reads `flow` output. This is the single gap between "tooling exists" and "gas scenarios show real OPM comparison."
2. **No OPM case for `gas_injection` or `gas_drive`.** Only `wf_bl1d` and `spe1_gas_injection` have decks. The two non-black-oil gas scenarios have no ground truth beyond the (partially-applicable, `gas_drive`'s is explicitly "not quantitatively accurate") analytical overlay.
3. **`docs/DOCUMENTATION_INDEX.md` doesn't reflect reality.** `PLAN.md` and `docs/REFACTOR_PLAN.md` are listed nowhere in it (in fact `PLAN.md` isn't mentioned in the index at all, and `docs/REFACTOR_PLAN.md` isn't either), so a new contributor has no signal that both are stale. `docs/2.md` and `docs/20260426.md` are similarly invisible to the index.

### P1 — Architecture debt that will make P0 and future gas work harder

4. **Three-generation chart layer.** `benchmarkCases.ts` / `caseCatalog.ts` / `ReferenceComparisonChart.svelte` + `buildChartData.ts` (legacy-but-live) vs. `ScenarioChart.svelte` / `scenarioChartModel.ts` (new-but-thin). Every future comparison feature (including finishing OPM wiring) has to be reasoned about against both layers simultaneously right now.
5. **`buildChartData.ts` at 1,614 lines** is still the de facto owner of "how does a reference-source curve get from data to a chart panel," including the OPM branch. Finishing OPM parsing without addressing this means bolting more logic onto the file the project has twice tried to shrink.
6. **No automated scenario/artifact integrity check.** Nothing currently fails a test if a scenario references an OPM artifact key that doesn't exist, or if an OPM artifact's `scenarioKey` points at a scenario that no longer exists. (`docs/ARCHITECTURE_NOTES.md` calls for exactly this kind of scenario validation but it isn't scoped to OPM artifacts specifically.) Once real OPM data starts flowing, a silently-wrong artifact link becomes a real risk of an incorrect chart, not just a missing one.
7. **`3dview.svelte` named bugs** (FOV-in-degrees conversion, `Array.isArray` on a non-array `GridState`, duplicated `VisualReservoirMetrics` type, `k ?? k` well indexing) were flagged in `PLAN.md`'s review findings. `spatialViewModel.ts` now exists, suggesting partial extraction happened, but I did not re-verify whether each named bug is actually fixed post-refactor — worth a short, scoped verification pass since it's cheap and these are concrete, testable claims.

### P2 — Validation/comparison-coverage gaps once OPM data exists

8. **No quantitative acceptance criteria** for any gas or black-oil scenario. Once OPM series are real, you'll want an explicit tolerance policy analogous to `docs/P4_TWO_PHASE_BENCHMARKS.md` for BL — e.g. "FOPR/FGPT within X% of OPM at breakthrough and at end of run," not just "curves overlaid, eyeball it."
9. **Oil-phase material balance is still only a residual quantity**, not an explicit reported diagnostic, per `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md`. This weakens any OPM-vs-ResSim comparison that wants to check conservation independently of the production curves.
10. **Tabular SCAL for SPE1** is still Corey-approximated rather than using the actual SPE1 `SWOF`/`SGOF` tables (visible directly in `tools/opm_flow/opm_flow_tool/cases.py` — the OPM deck already has the real tabulated curves that the ResSim scenario doesn't use yet). This alone could explain part of any ResSim-vs-OPM mismatch once the comparison exists, independent of solver quality.

### P3 — Product breadth (only after the above)

11. **Scenario coverage gaps** — see §4 below for concrete candidates.
12. **FIM documentation load** disproportionate to current FIM priority (§2.5) — not urgent, but worth a cleanup pass so `docs/` reflects what's actually product-critical.

---

## 4. New scenario ideas (not currently covered)

Ranked by how well they fit the existing analytical-vs-simulation-vs-OPM comparison framing, since that's the toolbox's core value:

1. **Gas cap primary depletion / blowdown** (already on `ROADMAP.md` 5.1) — has clean analytical references (p/z material balance, already partially present for gas diagnostics) and is a natural OPM case (SPE1-style deck already close). Highest-fit next scenario.
2. **Aquifer-supported depletion** — Fetkovich or Carter-Tracy aquifer analytical models are classic reservoir-engineering teaching content and pair naturally with the existing Dietz/Fetkovich decline scenarios (`dep_pss`, `dep_decline`). OPM supports `AQUFETP`/`AQUCT` directly, so this is comparison-friendly.
3. **WAG (water-alternating-gas) injection** — natural extension of `gas_injection`; no clean closed-form analytical exists, so this would be a "simulation + OPM" scenario (no analytical overlay), similar in spirit to `gas_drive` today but with an actual OPM ground truth instead of "none."
4. **Five-spot / line-drive pattern flood simulated directly** (not just via Craig's correlation) — you already have `sweep_areal` using Craig's analytical correlation; a directly-simulated quarter-five-spot compared against both Craig's correlation *and* OPM would let users see where the analytical correlation's assumptions (confined pattern, unit mobility ratio) break down, which is a genuinely useful teaching contrast.
5. **Waterflood with unfavorable mobility ratio / viscous fingering sensitivity** — you have mobility-ratio sensitivity in `gas_injection` already; an equivalent water-oil version stress-tests Buckley-Leverett's assumption of a stable front, which is pedagogically valuable and cheap to add (parameter sensitivity on an existing scenario family, not a new physics path).
6. **Simple well test / pressure transient analysis** (drawdown or buildup, Horner plot) — classic reservoir-engineering analytical (radial diffusivity solution) that nothing in the current catalog covers; would need a single-well radial-flow-style scenario and a Horner/MDH analytical module, but no new Rust physics (existing pressure solver + well model should already produce a usable transient if run at fine enough timesteps near the well).
7. **Non-uniform/refined grid near wellbore (local grid refinement or simply finer near-well spacing)** — connects directly to the existing grid-refinement sensitivity pattern (`gas_injection`'s `grid` dimension) and would demonstrate why well-block pressure needs a Peaceman correction, reinforcing rather than replacing existing content.

Lower priority / bigger lift, listed for completeness since you asked to think broadly: CO2 injection/storage (compositional or simplified black-oil proxy), polymer/surfactant EOR (Koval/Todd-Longstaff mixing), horizontal wells, compositional PVT. These are all reasonable multi-month engineering-content additions, but per `ROADMAP.md`'s own ordering principle ("add new physics only after existing interpretation and diagnostics are trustworthy"), they belong after the OPM/gas validation work above, not before it.

---

## 5. Recommended phased plan

**Phase A — OPM artifact pipeline completion (Python only, no Rust/WASM/FIM touched)**
- Implement summary parsing in `tools/opm_flow/opm_flow_tool/artifacts.py` (read `flow`'s `.UNSMRY`/`.SMSPEC` or exported `.RSM`, populate `OpmFlowArtifactSeries[]`, set `status: 'parsed'`).
- Requires running this on a machine with `flow` installed (not available in this sandbox) — this is the same step that already produced `SPE1CASE1.DBG` once, so the workflow is proven, just not automated into the artifact JSON yet.
- Regenerate `wf_bl1d.json` and `spe1_gas_injection.json` as real, parsed artifacts.
- Add OPM cases for `gas_injection` and `gas_drive` in `cases.py` (both can likely reuse a black-oil-style deck skeleton close to the SPE1 one, with `DISGAS`/`VAPOIL` tuned to match what each scenario actually models).
- Add a scenario/artifact integrity test (ties into gap #6): every scenario with an OPM hook must resolve to an artifact with matching `scenarioKey`, and CI should fail loudly if an artifact regresses from `parsed` back to `deck-ready`.

**Phase B — Chart/scenario architecture consolidation**
- Finish `ROADMAP.md` Priority 3.1 / `docs/REFACTOR_PLAN.md` Phase 8: remove `benchmarkCases.ts`, `caseCatalog.ts`, and fold `ReferenceComparisonChart.svelte` + `buildChartData.ts` into the scenario-first path so `ScenarioChart.svelte` is the single real entrypoint, not a thin wrapper around the old one.
- This should happen before or alongside Phase A's artifact-integrity test, since that test is much easier to write once there's one chart-building path instead of two.

**Phase C — Gas/black-oil acceptance criteria against OPM**
- Once Phase A produces real OPM series, define explicit tolerance bands (analogous to `docs/P4_TWO_PHASE_BENCHMARKS.md`) for `gas_injection`, `gas_drive`, and `spe1_gas_injection` against OPM rather than against FIM.
- This directly operationalizes `docs/FIM_DEFERRED_BACKLOG.md`'s existing stated intent ("FIM/OPM side-by-side diagnostic reproduction... after the current pressure-first path is re-baselined") but reframes it as an IMPES-vs-OPM check, which is achievable now, instead of waiting on FIM convergence.
- Swap SPE1's Corey-approximated SCAL for the real tabulated `SWOF`/`SGOF` (already sitting unused in `cases.py`) to remove that confound before judging IMPES-vs-OPM fit.

**Phase D — Documentation consolidation**
- Move the 19 FIM-specific docs under a `docs/fim/` subdirectory (or equivalent grouping) so `docs/` at top level reflects product priority; update `docs/DOCUMENTATION_INDEX.md` accordingly.
- Mark `PLAN.md` and `docs/REFACTOR_PLAN.md` as historical (or delete if fully superseded) once Phase B closes out the items they still list as open.
- Fold `docs/2.md` and `docs/20260426.md` into `docs/FIM_CONVERGENCE_WORKLOG.md` or the OPM mapping doc, since their content is real and useful but currently undiscoverable.
- Prune `TODO.md`'s "Now" section back to the short, action-oriented format `FIM_STATUS.md` already asks for; move the timestamped micro-experiment narrative into `docs/FIM_CONVERGENCE_WORKLOG.md` where it belongs per stated policy.

**Phase E — New scenario breadth**
- Only after A–D: pick up gas-cap depletion/blowdown first (best analytical + OPM fit), then aquifer support, per §4.

---

## 6. Open questions for you

1. **Is `flow` (OPM Flow) available on your machine right now**, the way it was for the 2026-04-06 run that produced `SPE1CASE1.DBG`? Phase A's parser can be written and unit-tested against a fixture file here, but generating fresh real artifacts needs to happen where `flow` actually runs.
2. **Sequencing preference: Phase A before Phase B, or the reverse?** I've proposed OPM-completion first since it's the most direct answer to your stated goal and is fully decoupled from the chart-architecture cleanup, but if you'd rather stop the architectural bleeding before adding more data sources through it, B-then-A is defensible too.
3. **Which new scenarios from §4 actually interest you?** I ranked by comparison-framework fit, not by what you'll find most useful day to day — happy to reprioritize once I know which reservoir-engineering topics you care about covering next.
4. **Documentation reorg (Phase D): comfortable with archiving/moving `PLAN.md`, `docs/REFACTOR_PLAN.md`, and the FIM doc pile, or would you rather I leave doc structure untouched and only add new docs going forward?** This review doesn't move anything yet — just flags it.
