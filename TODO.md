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

### Wave 4 follow-ups
- [ ] **(MAJOR, E5) History/forecast divider never shows on `dep_nct` by default.** The `fetkovich`
  layout opens on `xAxisMode: 'logTime'` but `resolveHistoryDivider` only matches `axis: 'time'`.
  On `logTime` the chart plots `log10(time)` on a linear scale, so the divider must draw at
  `Math.log10(boundary)` (guard `boundary > 0`) — extend `resolveHistoryDivider` to treat `logTime`
  as time-family with a transformed boundary.
- [ ] **(MINOR, E5) Divider only exists on the comparison-chart path** (`ReferenceComparisonChart`);
  the live single-run `RateChart`/`UniversalChart` path doesn't thread `historyWindow`. OK by design;
  record so a future live-panel use doesn't assume it exists there.
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
- [ ] **(MINOR) `wf_tornado` runtime:** 4×800 steps of a 40×1×10 IMPES grid ≈ 4–5 s each headless —
  visual check the sweep UX stays responsive.
- [ ] **(MINOR, style)** `navigationStore`/`runtimeStore` import benchmark types via the
  `benchmarkCases` stub re-export instead of `scenario/referenceTypes` directly — trivial cleanup
  when next touched.

### Chart / catalog architecture
- [ ] **Chart consolidation is on the product critical path.** Scenario-library Tiers 5–6 can't land
  cleanly on the multi-generation chart stack without growing `buildChartData.ts` (forbidden by the
  frontend-architecture skill). Schedule a scoped ROADMAP P3.1 / COMPARISON_TOOLBOX Phase B pass
  before more chart features.
- [ ] **The 2026-03-07 UI audit was never converted to backlog and is partially stale** (predates the
  scenario-first migration). Re-verify its 14 findings against the current UI before any UX pass;
  keep only survivors.

### Scenario library (Tier 5–6)
- [ ] **5.1 "Matched history, different reserves"** (N·c_t ambiguity) — no engine gap, extends `dep_*`.
- [ ] **5.2 "The tornado plot lies"** (kv/kh × density-contrast) — no engine gap.
- [ ] **5.3 "Two fluid models, one calibration point"** (correlation vs tabular PVT) — no engine gap;
  no OPM deck for this case yet.
- [ ] **E2: declarative time-based well schedule** in scenario params, worker-driven (wasm APIs exist;
  `sim.worker.ts` applies schedules only at create). Unblocks WAG (5.5) and SPE9.
- [ ] **Engine gaps deferred:** relperm hysteresis (E4, WAG), per-well injected fluid (E3), inactive
  cells (E6, blocks live PUNQ-S3).
- [ ] **Tier-6 pre-run exhibits** (need only E7 + data curation): 6.5 SPE11 inter-simulator spread,
  6.1 PUNQ-S3 ensemble, 6.3/6.4 SPE5 WAG + hysteresis; pilot `flowexp_comp` compositional for 6.2;
  record dataset licenses/provenance before bundling artifacts.

## Priority 2 — Validation & correctness
- [ ] **Define three-phase `experimental` exit criteria** + acceptance tests for gas-injection and
  gas-drive (breakthrough timing, Sg evolution, phase-closure diagnostics).
- [ ] **Reconcile three-phase docs with implemented state:** gas-oil capillary sign, `s_org`, explicit
  gas MB reporting, oil-phase diagnostic limits.
- [ ] **SPE1:** add regression tests for scenario wiring / published-reference panel placement /
  `cellDzPerLayer` + per-layer completion payload; tune rate targets vs Eclipse reference (exact match
  needs tabular SCAL, now in place); re-verify the comparison source/metric mapping (Case 1 vs 2, avg
  vs field pressure, producing GOR). Note the post-breakthrough GOR still rises too sharply and finer
  grid moves *away* from reference — a well/transport-model question, not a solver-stability one.
- [ ] **Revisit the ignored Buckley-Leverett refined-grid regression** as a potential solver/timestep
  issue, not just a slow-test classification.
- [ ] **Comparison-model tests:** preview mode, depletion per-variant analytical overlays, color-index
  stability.
- [ ] **Chart x-axis endpoints** (cumulative/time modes): prepend zero anchors, snap shared range/ticks
  to round values (no `70.00000000006`-style residues).
- [ ] **Analytical-method integrity:** enforce method semantics at the scenario type level (sweep can't
  inherit BL primary curves); generalize the `sweep_combined` toggle into a reusable sweep-method
  framework; document `sweep_areal` as quarter-five-spot with no-flow outer boundaries; decide whether
  `SwProfileChart` is restored or removed.

## Priority 3 — FIM solver (dev-only, parked maintenance track)

FIM is out of the user path (IMPES ships). Do not chase small deltas; big OPM-architecture gaps
matter more. Search `docs/FIM_EXPERIMENT_REGISTRY.md` by mechanism before any change.

- [ ] **2 failing timestep tests** (verified red 2026-07-24, pre-existing, dev-only):
  `fim::timestep::tests::changing_hotspot_resets_extra_growth_cooldown_budget` and
  `repeated_same_hotspot_extends_growth_cooldown_budget`. (The other two of the old "3–4 failures"
  group — the closed-system/rate_history contract tests — are now green, fixed by `f95e075`.)
- [ ] **Relperm-endpoint singularity (re-scoped 2026-07-24).** The `linear-bad` backstop on small
  well-dominated cases (`22x22x1`, `23x23x1`, `sweep-areal`). **Recommendation: do not do the
  relperm-tail regularization (Option B); prefer proactive iterative routing (Option A) if ever
  prioritized.** Full analysis + why: `docs/FIM_RELPERM_ENDPOINT_SINGULARITY_ANALYSIS.md`. Keep the
  load-bearing fallback and its `fim/linear/mod.rs` comment meanwhile.
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
- [ ] **`.claude/settings.json` allowlist is stale** — `/home/reken/...` absolute paths and one-off
  experiment commands from old sessions. Needs a manual prune (agent-initiated permission edits are
  blocked by policy — user action).
