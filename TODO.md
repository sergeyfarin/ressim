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
- [ ] **(MAJOR) The linear solver strategy is the real large-grid bottleneck.** `LinearSolverKind::DEFAULT`
  is `FaerSparseLu` — a *direct* sparse LU rebuilt from scratch every substep. It is 73–80 % of runtime
  on the ≥2000-cell cases and its cost grows superlinearly with cell count, so every future large case
  inherits this. Two supporting observations: (a) `calculate_fluxes` assembles `diag_inv` and passes
  `initial_guess` for an iterative solve, then the LU path **discards both** — the warm start from the
  previous pressure is free and unused; (b) a probe forcing `BiCgStab` cut `spe1/grid_20` linear time
  13.9 → 5.4 s, but needed ~119 Jacobi-preconditioned iterations per solve and was *slower* on the
  300-cell base grid, so a blanket switch is not the answer. Wanted: a size-aware choice plus a
  stronger preconditioner (ILU0/CPR) and warm starting. Note the per-solve `sprs → faer` triplet
  rebuild + `to_col_major()` is also redone every substep against a fixed pattern and could reuse a
  cached structure, refilling values only.
- [ ] **(MINOR) `spe1 delta_t/delta_t_0_25` runs 16 000 outer steps for 4000 days** (61 s, 300 cells,
  ~1 substep per step). Given the adaptive loop already controls CFL, this variant mostly measures
  outer-loop overhead; confirm it still teaches what it claims to.
- [ ] **(MINOR) `sim.worker.ts` calls `peekLatestRatePoint()` every step** for the termination policy,
  which marshals up to `historyInterval` rate-history entries across the wasm boundary each time
  though only the last point is used. Cheap next to the solver, but pure waste on long runs.
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
