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
- [ ] **`docs/DOCUMENTATION_INDEX.md` still says "FIM is dev-only; public scenarios ship IMPES."**
  Gas/three-phase scenarios (incl. `spe1_gas_injection`) have defaulted to FIM since `b88ee28`.
  Reconcile the doc with the shipped solver policy.
- [ ] **Define three-phase `experimental` exit criteria** + acceptance tests for gas-injection and
  gas-drive (breakthrough timing, Sg evolution, phase-closure diagnostics).
- [ ] **Reconcile three-phase docs with implemented state:** gas-oil capillary sign, `s_org`, explicit
  gas MB reporting, oil-phase diagnostic limits.
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
- [ ] **Analytical-method integrity:** enforce method semantics at the scenario type level (sweep can't
  inherit BL primary curves); generalize the `sweep_combined` toggle into a reusable sweep-method
  framework; document `sweep_areal` as quarter-five-spot with no-flow outer boundaries; decide whether
  `SwProfileChart` is restored or removed.

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
