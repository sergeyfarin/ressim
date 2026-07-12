# FIM Convergence Worklog

This file is the active investigation log for live FIM convergence work.
Use `docs/FIM_STATUS.md` for the current consolidated solver status.
Use this worklog only for active observations, reproductions, traces, and next hypotheses while an issue is still live.

Historical narrative was trimmed out of this file twice:
- March 2026 tracker history from `TODO.md`: `docs/FIM_HISTORY_2026-03.md`
- Full live worklog snapshot through 2026-04-06: `docs/FIM_CONVERGENCE_ARCHIVE_2026-03_to_2026-04-06.md`
- Water/gas shelf investigations, Phase 5 AD-assembler cutover, Phase 6 (legacy Jacobian
  retirement), Phase 7 (OPM-style Newton globalization), Phase 8 (hotspot state
  characterization), and the Hypothesis C row-scaling attempt (2026-04-08 through 2026-07-03):
  `docs/FIM_CONVERGENCE_ARCHIVE_2026-04-08_to_2026-07-03.md`

## Active Scope
- Keep this file limited to current-head repros, latest measurements, and next solver questions.
- Treat resolved correctness hardening and old exploratory branches as archival unless they reopen on current head.
- Current active repro set:
  - hard water shelf: `water-pressure --grid 12x12x3 --steps 1 --dt 1`
  - shipped gas shelf: `gas-rate --grid 10x10x3 --steps 6 --dt 0.25`
  - over-threshold CPR probe: `water-pressure --grid 23x23x1 --steps 1 --dt 0.25`

### Phase 9 (revised 2026-07-04) — component-isolation lab built and validated

User reviewed `CODEX_FIM_DIALOGUE_03.07.2026.md` (an independent parallel investigation) and an uncommitted
experimental commit (`db3bdaf`, "Experiment - not completed"), and directed a structural fix: stop testing
linear-solver hypotheses by changing the live solver and replaying full simulations — that conflates linear-
solve quality with Newton-trajectory and timestep-controller feedback, which is exactly why the session's
own row-scaling attempt (Hypothesis C above) took a full wasm-rebuild-and-replay cycle to falsify. Directed
instead: build the ability to test components — matrix builder, matrix solver, CPR — separately. Plan file
Phase 9 rewritten accordingly (see `/home/coder/.claude/plans/graceful-splashing-micali.md`).

**Step 9.0 — rejected commit `db3bdaf` (revert `55d6dcf`).** That commit added an in-situ CPR-restriction-
variant probe gated on `options.verbose`; confirmed by direct code read that the only production caller
(`step_internal_fim_impl`, `timestep.rs:849`) hardwires `verbose=false`, so the probe could never fire through
the canonical wasm diagnostic runner — dead code from the moment it landed. It also measured only one
preconditioner *application* per variant, which cannot answer the only question that matters ("would a full
solve with variant X converge where the current row0-schur restriction dead-states?"). Reverted cleanly;
`cargo build` + `fim::linear` tests (29) green after. `FIM-LINEAR-005` registry row reset to `OPEN` with a
note pointing at the offline lab as the correct vehicle; the variant math (`CprPressureRestrictionKind`,
5 variants) remains recoverable from git history for Step 9.2 continuation.

**Step 9.1 — capture harness built.** New `fim/linear/capture.rs` (`#[cfg(not(target_arch = "wasm32"))]`,
std-only text format, no new dependencies): dumps `jacobian`/`rhs`/`layout`/failure-metadata for every failed
iterative linear solve to disk, gated on the `FIM_CAPTURE_DIR` env var. Hooked at the existing linear-failure
branch in `newton.rs` (same site as `FAIL-SITE-DETAIL`, Phase 8's Step 8.1 addition). Because the whole module
is `#[cfg(not(target_arch = "wasm32"))]`, it is not merely inert but **entirely absent** from wasm builds —
confirmed via clean `bash scripts/build-wasm.sh` and a heavy-case replay bit-identical to the Phase 8 baseline
(`31` substeps, `accepts=30+3+1678`, `retries=0/12/0`, identical `real_accept_rungs`/`retry_rungs` sequence).
Locked smoke set green (3/3, 381.04s). Refreshed the stale doc comment on `repro_water_pressure_12x12x3`
(described the pre-`ffd965a` non-terminating singularity, no longer true — the case completes at 31 substeps)
and repurposed it as the capture driver; added a sibling `repro_water_pressure_23x23x1` mirroring the bounded
control case. Captured two corpora: **54 systems** from the heavy case, **13 systems** from the bounded case
(both native `--release`, `--ignored`, a few seconds each).

**Step 9.2 — offline solver lab built, both validity gates passed on both corpora.** New
`fim/linear/solver_lab.rs` (`#[cfg(test)]`, `#[ignore]`d): loads a captured corpus and runs **full solves**
through the existing `solve_linearized_system` entry point with each backend (sparse-LU, `GmresIlu0`,
`FgmresCpr`) on identical input — a new hypothesis is now one enum arm and a few-seconds rerun, no wasm
rebuild, no simulation. Both plan-mandated validity gates (asserted, not just printed) passed on both corpora:

- **Stop-condition-1 gate (assembly-level sanity):** sparse-LU reference converged on **all 54 heavy-case and
  all 13 bounded-case systems**, residual ~1e-13 to 1e-16 relative — the captured systems are genuinely
  solvable; no evidence of an assembly-level problem at these states.
- **Stop-condition-2 gate (capture fidelity):** the current `FgmresCpr` (row0-schur) path failed to converge
  on **54/54 heavy-case and 13/13 bounded-case** systems offline — it reproduces its live failure with 100%
  fidelity on both corpora (well above the ≥50% bar), confirming the capture is complete enough to trust for
  comparison.

**First substantive finding, and an important nuance between the two corpora:**
- **Heavy case (54 systems): uniform signal.** Plain `GmresIlu0` (full ILU(0), no CPR pressure-correction
  stage at all) landed 1-4 orders of magnitude closer to converged than the current `FgmresCpr` on **every
  single one** of the 54 systems (e.g. capture 00037: ILU0 relative residual `3.24e-5` vs. CPR `1.00e0`;
  capture 00053: ILU0 `3.74e-7` vs. CPR `1.00e0`). On this case, the CPR pressure-correction stage is not
  merely insufficient — adding it on top of ILU0 makes convergence uniformly and substantially *worse*.
- **Bounded case (13 systems): mixed signal, not the same pattern.** Here CPR sometimes helps a lot (capture
  00000: ILU0 rel `1.93e-3` vs. CPR `6.82e-5`, ~28x better) and sometimes hurts a lot (capture 00010: ILU0 rel
  `2.34e-3` vs. CPR `4.58e-1`, ~200x worse) — no uniform direction. **Do not over-generalize the heavy case's
  "CPR is actively harmful" finding as universal** — it is state-dependent, and the lab has now demonstrated
  that cleanly across two independent corpora in seconds, something the prior live-replay-only workflow could
  not have surfaced this precisely.

**Status:** lab validated and ready for its intended use — testing the salvaged `CprPressureRestrictionKind`
variants (`sum-rows`, `diag-balanced-sum`, `dominant-diag-row`, `local-schur-balanced`) and an OPM-style
quasi-IMPES weighting (per-cell solve of `A_ii^T w = e_pressure` from the diagonal accumulation block,
normalized) as full-solve variants against both corpora, per Step 9.2's remaining scope. Not yet implemented;
presented to the user as the next increment before proceeding.

### Restriction-variant comparison — decisive, consistent result across both corpora

Salvaged the variant math from `db3bdaf`'s history (kept out of the live path — `solve()`, the only production
entry point, hardcodes `Row0Schur` explicitly, unchanged from current behavior) and parameterized
`build_pressure_transfer_weights`/`build_block_jacobi_preconditioner`/`solve_with_cpr_fine_smoother` with a
`CprPressureRestrictionKind` enum: `Row0Schur` (current), `SummedRows`, `DiagBalancedRows`,
`DominantDiagonalRow`, `LocalSchurBalanced` (all salvaged), plus a new `QuasiImpes` matching OPM's
`getQuasiImpesWeights.hpp` (`w = A_ii^{-1}.row(pressure_index)`, normalized — reuses the block inverse already
computed for the same cell). Added a lab-only `solve_with_restriction_kind` entry point (`#[cfg(test)]`,
never reachable from production) and a new `solver_lab_compare_restriction_variants` test. All 31 pre-existing
`fim::linear` tests pass unchanged; production build has zero new warnings.

**Full-solve comparison, both captured corpora (numbers are `converged/total`, median relative residual):**

| Variant | Heavy (54 systems) | Bounded (13 systems) |
|---|---|---|
| `row0-schur` (current, live) | `0/54`, median `1.00e0` | `0/13`, median `2.61e-3` |
| `sum-rows` | `50/54`, median `3.32e-8` | `12/13`, median `2.73e-8` |
| `diag-balanced-sum` | `8/54`, median `1.00e0` | `0/13`, median `1.04e-1` |
| `dominant-diag-row` | `39/54`, median `7.11e-6` | `12/13`, median `4.42e-8` |
| `local-schur-balanced` | `0/54`, median `1.00e0` | `0/13`, median `2.60e-3` |
| `quasi-impes` | `50/54`, median `3.64e-8` | `12/13`, median `6.55e-8` |

**Reading:** the current live restriction (`row0-schur`) never converges on either corpus — consistent with
everything found earlier this session. `local-schur-balanced` (row0-schur plus normalization) doesn't fix
this either — the problem isn't scaling of the row0-schur weights, it's the row0-schur construction itself.
Three variants dramatically outperform current on **both** independent corpora: `sum-rows` and `quasi-impes`
both converge on ~92-93% of systems on each corpus; `dominant-diag-row` is strong on bounded (12/13) but
weaker on heavy (39/54). `quasi-impes` is the principled choice — it's literally OPM's own production CPR
construction, not an ad-hoc heuristic, and it matches `sum-rows`'s convergence rate almost exactly on both
corpora while being derived from the actual physics (the diagonal accumulation block) rather than treating
all rows as interchangeable.

**This is now Step 9.3-eligible evidence** (the plan's promotion bar: "convincingly wins offline on the clear
majority of captured dead-state systems, solution accurate vs the direct reference," on both independent
corpora) — presented to the user for the promotion decision rather than promoted autonomously, since Step 9.3
still requires the live control-matrix gate and Newton-trajectory feedback is a real, separate risk (stop
condition 4) that this offline lab cannot rule out by itself.

### Step 9.3 live gate result — mixed, not a clean promote or revert

User approved proceeding to Step 9.3. Changed `solve()`'s hardcoded restriction from `Row0Schur` to `QuasiImpes`
(`gmres_block_jacobi.rs`, the only production entry point) — production build clean, zero new warnings, all 31
pre-existing `fim::linear` tests pass unchanged. Full control matrix + locked smoke on the rebuilt wasm:

| Case | Baseline (`row0-schur`) | After (`quasi-impes`) |
|---|---|---|
| water-pressure 20x20x3 | `8` substeps, `0/3/0` | `8` substeps, `0/3/0` (unchanged) |
| water-pressure 22x22x1 | `4` substeps, `0/2/0` | `4` substeps, `0/2/0` (unchanged) |
| water-pressure 23x23x1 | `4` substeps, `0/2/0` | `4` substeps, `0/2/0` (unchanged) |
| gas-rate 20x20x3 | `2` substeps, `0/1/0` | `2` substeps, `0/1/0` (unchanged) |
| gas-rate 10x10x3, 6 steps | `14` total substeps | `14` total substeps (unchanged) |
| **water-pressure 12x12x3 dt=1 (heavy)** | **`31` substeps, `0/12/0`, `26.5s`** | **`26` substeps, `0/13/0`, `78.4s`** |

Locked smoke set green (3/3, 445.81s). All 5 control-matrix cases bit-identical. The heavy case (the actual
target) is genuinely mixed: substeps improved `31→26` (16% fewer, the primary metric this whole effort has
tracked) but retries went up by 1 and wall-clock nearly tripled (`26.5s→78.4s`), driven by `pc_ms` jumping from
~6-13s to `66.2s` — `quasi-impes` converges far more often per Newton iteration than the old restriction, but
each convergent solve now runs many more GMRES iterations to reach the old, very tight `1e-7`-relative target,
instead of the old restriction's fast-fail-at-30-iterations-then-cheap-direct-solve pattern.

**Decision: neither promoted nor reverted.** Presented to the user as a genuine trade-off; user's response was
to reject further isolated-lever tuning and instead directed a systematic replication of OPM's whole recipe as
one bundle (Phase 10, below) — this is exactly the item-by-item testing trap the user flagged: a real
improvement (restriction choice) tested against the wrong tolerance philosophy produces a misleading verdict.
`solve()` is currently left on `QuasiImpes` (uncommitted) pending Phase 10's bundle re-test, which will
supersede this isolated result either way.

## Phase 10 (2026-07-04) — adopt OPM's CPR recipe as a bundle, not item by item

Full context, OPM's actual shipped defaults (researched from `OPM/opm-simulators/` source), ResSim's confirmed
current state, and the Rust-ecosystem AMG constraint are in
`/home/coder/.claude/plans/graceful-splashing-micali.md` "Phase 10." Summary: OPM's `cprw` default pairs a
**loose** linear tolerance (`0.005` relative reduction, `maxiter: 20` for the CPR path) with block-ILU0 and
TrueIMPES/QuasiIMPES weighting — tested and shipped as one matched set, not tuned as independent levers. AMG is
explicitly out of scope (no wasm32-compatible pure-Rust crate exists; hand-rolling is ~1500-2000 LOC and not
needed at current benchmark scale per the existing `docs/FIM_CPR_IMPROVEMENT_PLAN.md` finding).

### Step 10.0 — tolerance/budget translation, derived from real data (not pasted from OPM)

ResSim's linear solve always starts from `x_0 = 0` (confirmed: `solution = DVector::zeros(rhs.len())` at the
top of `solve_with_cpr_fine_smoother`, `gmres_block_jacobi.rs:1290`), so `r_0 = rhs` exactly on every solve.
This means OPM's relative-reduction target `||r_k||/||r_0|| <= 0.005` translates **exactly** (not
approximately) to ResSim's own absolute-residual check as `relative_tolerance = 5e-3` (with the old
`absolute_tolerance: 1e-10` term becoming vestigial — confirmed below, not assumed).

Pulled the actual `rhs_norm` values observed across both captured corpora (54 heavy-case systems, 13
bounded-case systems, via a fresh `solver_lab_compare_backends` rerun on each): range is **`5.2e-2` to
`2.5e3`** across both corpora combined. Substituting the extremes and a mid-range sample:

| `rhs_norm` | old tolerance (`1e-10 + 1e-7·rhs`) | new tolerance (`5e-3·rhs`) | new/old ratio |
|---|---|---|---|
| `5.202e-2` (smallest observed) | `5.302e-9` | `2.601e-4` | `~49,057x` |
| `2.079e-1` | `2.089e-8` | `1.040e-3` | `~49,761x` |
| `9.140e0` | `9.141e-7` | `4.570e-2` | `~49,994x` |
| `3.026e1` (largest observed in bounded corpus) | `3.026e-6` | `1.513e-1` | `~49,998x` |

The new tolerance is consistently **~50,000x looser** than the current one across the entire observed range —
this is not a marginal adjustment, it's a wholesale change in what "converged" means for a linear solve. The
old absolute floor (`1e-10`) is confirmed vestigial at every observed scale (at the smallest `rhs_norm` it
still contributes only ~2% of the old tolerance; at larger scales it's negligible) — dropping it (or keeping a
tiny floor purely to avoid a degenerate zero-tolerance edge case) matches OPM's own pure-relative criterion.

**`max_iterations`**: per the plan, OPM's `maxiter: 20` is the coarse-AMG-solve budget *inside* one CPR
preconditioner application — a different axis from ResSim's outer FGMRES iteration budget — so it is
deliberately **not** pasted as `150 → 20`. Step 10.3's offline lab will sweep `max_iterations ∈ {150, 50, 30,
20}` at the new `5e-3` tolerance and pick the smallest budget that holds the ~92-93% convergence rate already
established for `quasi-impes`, as a corpus-derived choice.

### Step 10.2 — block-ILU0 implemented

Added `FimBlockIlu0Factors`/`factorize_block_ilu0` alongside the existing scalar `FimIlu0Factors` in
`gmres_block_jacobi.rs`: standard block-IKJ ILU(0) over the natural `cell_block_size × cell_block_size`
reservoir-cell blocks (dense `nalgebra::DMatrix` arithmetic, reusing the `try_inverse()` pattern already
established by `cell_block_inverses`), with the well-BHP/perforation tail factorized independently as a
scalar sub-ILU(0) (Option A from the plan — no cross-block fill between cell region and tail, matching OPM's
own architecture of handling wells via a separate operator rather than folding them into the block-ILU0
reservoir smoother). New `CprFineSmootherKind::BlockIlu0` variant, explicit (not a silent redefinition of
`FullIlu0`) so the offline lab can A/B scalar-vs-block ILU0 independently. Two new unit tests: exact-solve on
an uncoupled block-diagonal system, and residual-reduction (not bit-equivalence — the correct gate per the
plan) on a coupled system with a scalar tail. All 33 pre-existing `fim::linear` tests pass unchanged;
production build has zero new warnings.

### Step 10.3 — offline bundle lab: decisive, consistent win on both corpora

Extended `solver_lab.rs` with `solver_lab_compare_bundle_tolerance_iterations`, testing 8 combinations of
`(relative_tolerance, max_iterations, smoother)` — all using `QuasiImpes` (already the live restriction) —
against both captured corpora via the new `solve_with_smoother_and_restriction` lab-only entry point:

**Heavy corpus (54 systems):**

| Row | Converged | Median iters | Mean iters |
|---|---|---|---|
| baseline: `tol=1e-7 iter=150 ilu0` (= today's live config) | `50/54` | `24` | `25.1` |
| `tol=5e-3 iter=150 ilu0` | `50/54` | `18` | `27.3` |
| `tol=5e-3 iter=50/30/20 ilu0` | `50/54` | `18` | `18.4-19.9` |
| **`tol=5e-3 iter=50/30/20 block-ilu0`** | **`54/54`** | **`4`** | **`4.4`** |

**Bounded corpus (13 systems):**

| Row | Converged | Median iters | Mean iters |
|---|---|---|---|
| baseline: `tol=1e-7 iter=150 ilu0` | `12/13` | `42` | `49.7` |
| `tol=5e-3 iter=150 ilu0` | `13/13` | `12` | `12.8` |
| `tol=5e-3 iter=50/30/20 ilu0` | `13/13` | `12` | `12.8` |
| **`tol=5e-3 iter=50/30/20 block-ilu0`** | **`13/13`** | **`5`** | **`5.2`** |

**Reading, consistent across both corpora:** loosening the tolerance alone is a real but modest win (closes
some of the convergence gap, cuts iterations by roughly 2x on heavy / 3.5x on bounded) — but reducing
`max_iterations` down to `20` costs **nothing** at either corpus once the tolerance is loose (identical
converged-count and median iterations at 150/50/30/20), confirming OPM's `maxiter: 20` budget is safe here,
derived from data rather than pasted. The decisive lever is **block-ILU0**: paired with the loose tolerance
it reaches **100% convergence on both corpora** (up from 50/54 and 12/13) with median iteration counts of just
**4 and 5** — a 5-6x reduction from baseline, and most systems now converge within a single 30-iteration
restart cycle. This is exactly the kind of result the bundle approach was meant to surface: block-ILU0's
value was invisible when only tolerance/restriction were being varied one at a time.

**Winning combination for Step 10.4 live promotion**: `relative_tolerance=5e-3`, `absolute_tolerance=1e-12`
(degenerate-case guard only), `max_iterations=20`, `restart=30` (unchanged), `CprFineSmootherKind::BlockIlu0`,
`CprPressureRestrictionKind::QuasiImpes` (already live). This clears Step 10.3's gate decisively — proceeding
to Step 10.4's live control-matrix promotion test.

### Step 10.4 — live gate: decisive REGRESSION, whole bundle REVERTED

Wired the offline-winning combination live: `FimLinearSolveOptions::default()` (`mod.rs`) changed to
`relative_tolerance=5e-3`, `absolute_tolerance=1e-12`, `max_iterations=20`; `solve()`'s CPR fine-smoother
selection (`gmres_block_jacobi.rs`) changed to `CprFineSmootherKind::BlockIlu0`. Production build clean, zero
new warnings; all 33 `fim::linear` tests pass (2 label-assertion tests updated to expect `"block-ilu0"`,
matching the intentional smoother change). Full control matrix on the rebuilt wasm:

| Case | Baseline (post-Step-9.3, `quasi-impes`+old tolerance+`ilu0`) | Phase 10 bundle |
|---|---|---|
| water-pressure 20x20x3 | `8` substeps, `0/3/0` | `8` substeps, `0/3/0` (unchanged) |
| water-pressure 22x22x1 | `4` substeps, `0/2/0` | `4` substeps, `0/2/0` (unchanged) |
| water-pressure 23x23x1 | `4` substeps, `0/2/0` | `4` substeps, `0/2/0` (unchanged) |
| gas-rate 20x20x3 | `2` substeps, `0/1/0` | `2` substeps, `0/1/0` (unchanged) |
| gas-rate 10x10x3, 6 steps | `14` total substeps | `14` total substeps (unchanged) |
| **water-pressure 12x12x3 dt=1 (heavy)** | **`26` substeps, `0/13/0`, `78.4s`** | **`59` substeps, `0/16/0`, `152.5s`** |

All 5 control-matrix cases bit-identical (substep/retry counts; timing noise only). The heavy case — the
one this whole recipe was built for — **regressed decisively**: substeps more than doubled relative to the
pre-quasi-impes original baseline (`31`) and grew further from the Step 9.3 state (`26→59`); retries worsened
(`0/13/0→0/16/0`); wall-clock nearly doubled again from the already-bad Step 9.3 number (`78.4s→152.5s`),
worse than every prior state recorded this session. Note this is a genuinely distinct decision point from
Step 10.3: the offline lab's promotion bar ("wins on the clear majority of both corpora") was met decisively
(100% convergence, 5-6x fewer iterations, both corpora) — this is not a case of the offline evidence being
weak or ambiguous; it is a case of the offline lab measuring the right thing (linear-solve quality on frozen
states) while missing something real about live Newton-trajectory feedback.

**Root cause, as far as this session can characterize it without further investigation:** the offline lab
tests linear solves on *frozen* captured Jacobian/RHS pairs — it cannot see how a *less accurate* linear
correction (satisfied at `5e-3` relative residual instead of `1e-7`) feeds back into the *next* Newton
iteration's trajectory. Apparently, in this codebase's Newton/timestep-controller interaction, accepting a
much less precise linear correction leads to Newton needing measurably *more* outer iterations and hotspot
re-visits to recover — even though each individual linear solve got dramatically cheaper. This is exactly the
"Newton-trajectory feedback is a real, separate risk the offline lab cannot rule out by itself" caveat flagged
in the Phase 9 plan before Step 9.3 was even attempted — now confirmed concretely, on a larger and more
decisive scale, by Phase 10's bundle.

**Reverted per the plan's explicit no-piecemeal-retry rule**: both `mod.rs`'s tolerance/budget defaults and
`gmres_block_jacobi.rs`'s smoother selection reverted together to their Step 9.3 state (`1e-7`/`1e-10`/`150`,
`FullIlu0`) in the same pass — not picked apart to isolate "was it the tolerance or the smoother." Two test
label assertions reverted to match. Rebuilt wasm; heavy-case replay confirmed exact restoration to the Step
9.3 state (`26` substeps, `accepts=25+4+2082`, `retries=0/13/0`, ~`70-78s`). **The validated `FimBlockIlu0Factors`/
`factorize_block_ilu0` implementation, its 2 unit tests, and the offline-lab bundle-comparison infrastructure
(`solver_lab_compare_bundle_tolerance_iterations`, `solve_with_smoother_and_restriction`) are kept** — they
remain correct, tested, unused-in-production capability for any future systematic attempt at this territory;
only the *live wiring* that caused the regression was reverted.

**Per the plan's discipline: do not now try picking the bundle apart on a hunch** (e.g. "keep block-ILU0,
revert only the tolerance" or vice versa) in this same session — that is precisely the piecemeal-retry pattern
this whole phase existed to avoid, and the offline/live divergence found here means any such probe would need
a *live* control-matrix cycle to mean anything (the offline lab has now been shown not to predict this
failure mode), which is exactly the expensive, noisy, one-lever-at-a-time loop Phase 10 was designed to
replace with something better. A legitimate next attempt would come from a fresh systematic analysis of *why*
the Newton trajectory needs more outer iterations under the looser tolerance specifically in this codebase —
not from retuning constants.

### Step 10.4 (reopened) — bundle re-applied per explicit user direction; Step 10.1 done with real evidence

User pushback on the Step 10.4 revert above: reverting the whole bundle after a single live regression,
without first executing the plan's own Step 10.1 (Newton-side reconciliation), was too quick — the offline
lab's win was decisive and the plan already called for finishing the Newton-side half before judging the
bundle. Re-applied both `mod.rs` (`relative_tolerance=5e-3`, `absolute_tolerance=1e-12`, `max_iterations=20`)
and `gmres_block_jacobi.rs` (`CprFineSmootherKind::BlockIlu0` for `FgmresCpr`) to their Step-10.4 state; fixed
the 2 stale test-label assertions back to `"block-ilu0"`. All 33 `fim::linear` tests pass.

**Step 10.1 measurement (real trace data, heavy case, `--diagnostic step`):** of 16 total retries, 9 are
`post-loop: NOT CONVERGED after 20 iterations`. 8 of those 9 are dominated by `perf@1299` (a single well
perforation equation) with a strikingly consistent pattern: `mb` 3-5 orders of magnitude inside its own
tolerance (`1e-5`), `upd` well inside `update_tolerance` (`1e-3`), but scaled `res` only `2-6x` over
`residual_tolerance` (`1e-5`) — e.g. `res=2.063e-5 mb=5.257e-9 upd=9.903e-5`. The 1 outlier (`oil@430`) is a
genuine large miss (`res=4.974e-3`, ~500x over), not a near-miss. Separately: `OSC-DETECT osc_phases=0
relax=1.00` fires on every logged iteration even while `STAGNATION-ATTRIB class=real-bump` shows the scaled
residual genuinely *increasing* between consecutive iterations — the Phase 7 oscillation detector (tuned for
alternating zig-zag residuals, `d1<0.2<d2` on a 3-iteration window) does not recognize this monotonic-growth
signature as oscillation, so it never engages extra relaxation here. Conclusion: the dominant live failure
mode under the loosened bundle is a **persistent near-miss at the well/perforation row**, not the mid-loop
stagnation-bailout or oscillation-relaxation machinery over-reacting — those mechanisms are largely inert at
this specific hotspot.

**Hypothesis tested and REFUTED by a real run (`FIM-NEWTON-005`):** added a bounded, post-loop-only
near-convergence acceptance (`should_accept_near_converged_newton_final_state`: accept the final iterate when
`max_newton_iterations` is exhausted, residual is within `10x` of `residual_tolerance`, and both `mb` and
`upd` already satisfy their own unrelaxed tolerances) — deliberately scoped narrower than, and distinct from,
the previously-reverted mid-loop stagnation-acceptance widening (`FIM-NEWTON-004`: that one gave up *early*
with budget remaining; this one only fires *after* the full budget is spent). 4 focused unit tests added,
all passed; `cargo build`/`cargo test --lib fim::newton` clean (52/52). Rebuilt wasm, ran the heavy case live:
the run did **not** finish in over 8 minutes of wall-clock (vs. `152.5s` for the bundle alone, `~26s`/`78.4s`
pre-Phase-10 baselines) and was killed. **This is decisive negative evidence, not a hunch**: accepting a
marginally under-converged perforation-rate constraint as "good enough" does not fix anything — the residual
well/perforation error carries forward as the next substep's initial condition and compounds, apparently
explosively, rather than being absorbed. This is exactly the failure mode the codebase's "known-reverted lever
class" entry for widening stagnation acceptance warns about, now confirmed to also apply to a differently-
scoped (post-loop, not mid-loop) variant of the same idea. **Reverted cleanly** (constant, function, live
wiring, 4 tests) back to the pre-hypothesis state; `fim::newton` back to 48/48 passing; wasm rebuilt to the
bundle-only state (matches the Step 10.4 numbers above: heavy case still `59` substeps / `152.5s`).

**Root-cause refinement (updates the Step 10.4 "root cause" note above):** the problem is not general
Newton-mechanism over-reaction to a noisier linear correction; it is specifically that `perf@1299`'s row is
persistently and repeatedly left a small-but-real multiple over tolerance by the global relative-residual
stopping criterion, and forcing acceptance of that state cascades forward rather than resolving. A single
scalar relative-residual criterion applied to the whole system (reservoir + well + perforation rows together)
can leave a numerically small subsystem systematically under-resolved even while the overall norm is
satisfied — plausible here because perforation-row magnitudes are small relative to the dominant
pressure/saturation rows, so a large *relative* error there barely moves the *global* norm. This is consistent
with why OPM keeps wells in a separate well-operator with its own convergence handling rather than folding
them into one global norm — an architectural distinction Phase 10's own context notes ResSim does NOT
currently have (wells are explicit unknowns, matching OPM, but they share one global stopping criterion with
the reservoir rows, which does not match OPM). Registered as `FIM-NEWTON-005` (REFUTED) and `FIM-LINEAR-008`
left `OPEN` (bundle still live, no longer reverted, no verdict yet) pending a decision on whether to pursue a
per-family/per-block linear stopping criterion (a real architectural change, not yet offline-lab-validated)
or a different reconciliation path.

### Step 10.1 follow-up — per-family linear convergence built and offline-lab-tested; WEAK support, root cause redirected to Newton damping

Designed and built the per-family/per-block linear stopping criterion floated above, systematically rather
than as another live guess: `EquationScaling::family_peaks`/`within_relative_reduction` (new,
`fim/scaling.rs`, reuses the same per-equation-family scale factors Newton already computes — no new scaling
scheme invented), threaded as an opt-in `equation_scaling: Option<&EquationScaling>` parameter through
`solve_linearized_system` → `gmres_block_jacobi::solve_with_cpr_fine_smoother`'s three convergence-decision
sites (top-of-loop, restart-completion, final post-loop fallback) via a `family_ok` closure — every family's
own scaled residual must clear its own relative-reduction target, not just the whole-system norm. `None`
everywhere except the live Newton call site (which now passes `Some(&assembly.equation_scaling)`) and the
new offline-lab test, so every pre-existing synthetic-matrix test is unaffected (34/34 `fim::linear` still
pass unchanged). New `fim::scaling` unit tests (3) and 6 threading edits, all green.

Extended the capture format to `fim-capture-v2` (adds the `EquationScaling` 5 arrays) so the offline lab
could actually test this on real systems, since the old v1 corpora couldn't carry it. Also found and fixed a
real gap in the capture *trigger*: the existing hook only fires when the linear solve itself fails, but under
the loosened Phase 10 tolerance the linear solve now almost always succeeds — only the outer Newton loop
fails (exhausts `max_newton_iterations`). Added a second, unconditional capture at the final Newton iteration
(`newton.rs`, gated on `iteration + 1 == options.max_newton_iterations`) so the lab has real near-miss systems
at all. Recaptured both corpora fresh against the live bundle: heavy case 35 systems, bounded case 3 systems
(both native, `--release`, `FIM_CAPTURE_DIR` set, `repro_water_pressure_12x12x3`/`_23x23x1`).

**Offline result (new `solver_lab_compare_family_aware_convergence` test, both corpora):**

| Corpus | Non-family-aware | Family-aware | Per-family overshoot |
|---|---|---|---|
| Heavy (35 systems) | converged 35/35, mean_iters 3.1 | converged 34/35, mean_iters 3.9 | only **1/35** systems shows overshoot > 1.0 (12.14x); the rest are ≤ 1.0 |
| Bounded (3 systems) | converged 0/3 | converged 0/3 (no change) | overshoot ratios in the billions — a measurement artifact, not a real signal (see below) |

`worst_family=perforation_flow` dominates the overshoot ranking on every system in both corpora (confirms
the earlier live-trace finding that this family is the tightest-constrained), but the *magnitude* of the
effect on the heavy corpus's captured final-iteration systems is small: 34/35 already satisfy their own
family target with the existing global criterion, and enforcing the stricter per-family check costs ~25% more
mean linear iterations for essentially no convergence-rate gain (35→34/35, i.e. it can occasionally make a
system *harder* within the same iteration budget). The bounded corpus's astronomical ratios are a division-
by-near-zero artifact: at a captured near-converged Newton iteration, the *raw RHS* perforation-row entry
(the ratio's denominator basis) can itself be tiny, so any non-trivial residual — even a perfectly reasonable
one — reports as billions-of-multiples over target; both configurations fail identically (0/3) regardless of
family-awareness, meaning these 3 systems are hard for an unrelated reason, not informative for this
hypothesis. **Verdict: the offline evidence does not support per-family linear-convergence swamping as the
dominant cause of the heavy case's near-miss retries.** Per the project's own "don't rebuild wasm on hope"
discipline (Step 10.5's stop-condition philosophy), this was NOT wired live and no control-matrix rerun was
attempted — the offline gate itself already says no.

**Redirected root-cause lead, found while investigating the above:** re-examined the same `DAMP-BREAKDOWN`
trace lines from the earlier heavy-case `--diagnostic step` capture (substep 22's second retry, the one that
exhausts all 20 Newton iterations at `perf@1299`). After the first 3 iterations, the applied damping factor
alternates **perfectly** between `1.0` and `0.5` for the remaining 17 iterations (`1.0, 0.5, 1.0, 0.5, ...`) —
a textbook two-step oscillation signature. Yet `OSC-DETECT osc_phases=0 relax=1.00` fires on every one of
these iterations: the Phase 7 oscillation detector's `family_is_oscillating` test (`d1 = |f0-f2|/f0 < 0.2 <
d2 = |f0-f1|/f0`) is evaluated on the *residual* history and evidently isn't tripping even though the
*damping factor* it's supposed to help moderate is visibly bouncing. This is a more concrete, better-
supported lead than the family-aware linear criterion: the mechanism that's supposed to catch exactly this
pattern appears to have a real gap between what it measures (residual ratios) and what's actually happening
(a damping-factor bounce), rather than the linear solver leaving a family under-resolved. **Not yet
implemented or further measured — flagged as the next Step 10.1 angle, pending the user's direction**, since
another live speculative change without full measurement first is exactly the pattern this phase exists to
avoid (see the `FIM-NEWTON-005` lesson above: a plausible-sounding mechanism doesn't earn a live promotion
without decisive offline/measured support).

**Kept as validated, tested, currently-inert infrastructure** (matches the project's `FIM-AD-002` precedent —
don't delete correct, tested capability just because this specific application didn't pan out):
`EquationScaling::family_peaks`/`within_relative_reduction`, the `family_ok` threading in
`gmres_block_jacobi.rs`, the `fim-capture-v2` format, and the new offline-lab test. All are opt-in
(`Option<&EquationScaling>` defaulting to `None`) and do not change production behavior when unused.

## Phase 11 (2026-07-04) — Well Schur elimination + OSC-DETECT scope fix (`FIM-LINEAR-010`, `FIM-NEWTON-006`)

User directive after the Step 10.1 follow-up: stop testing individual mechanisms in isolation; implement OPM's
architecture *consistently* — ResSim solving well-BHP/perforation-rate as ordinary global Newton unknowns
(rather than eliminating them via Schur complement every iteration, as OPM's `StandardWellEquations` does) is
itself an inconsistency with OPM that could be masking or interacting with other fixes. Full plan in
`/home/coder/.claude/plans/graceful-splashing-micali.md`, "Phase 11".

### Step 11.1-11.2 — Well Schur elimination built, proven exact, offline-decisive

New module `fim/linear/well_schur.rs`: eliminates well-BHP + perforation-rate rows from the linear system via
Schur complement before the iterative CPR/GMRES solve, recovering them exactly afterward — pure sparse/dense
linear algebra on the existing `FimLinearBlockLayout` row partition, no well-specific physics needed. Gated
behind `FimLinearSolveOptions::eliminate_wells` (opt-in, default `false` initially). **Correctness proven**:
a synthetic cell+well+perforation system solved via elimination matches a direct full-system solve to `1e-9`
(`well_elimination_matches_direct_full_system_solve`). **Offline lab** (`solver_lab_compare_well_elimination`,
new test) on the 35 real captured heavy-case systems: convergence `34/35 → 35/35`, mean linear iterations
`3.9 → 1.1` — a decisive win, passing this project's own offline-gate bar.

### Step 11.3 — Live gate: correct and faster per-solve, but does NOT fix the target oscillation

Promoted `eliminate_wells: true` as default, rebuilt wasm. Control matrix (5 non-target cases): bit-identical
substeps/retries, no regression. **Heavy case: regressed** (`59→160` substeps, `16→40` retries,
`hotspot_newton_caps` `20→58`, wall-clock `~65s→166s`), `retry_dom` still `nonlinear-bad:perf@1299`.

**Decisive diagnostic**: pulled the `perf@1299` residual trace after elimination — **the oscillation pattern is
unchanged** (`2.137e-5 ↔ 3.419e-5`, damping still alternating `1.0/0.5`, essentially the same numbers as before
elimination). This is the key insight: Schur elimination is an *exact* reformulation of the same linear system
— it doesn't change what Newton direction gets computed, only how efficiently. It cannot fix an oscillation
that lives in the nonlinear/damping layer, because that layer sees essentially the same direction either way.
**This rules out well-architecture-as-linear-system-structure as the oscillation's cause** — the well/perforation
oscillation is a genuine nonlinear phenomenon, not an artifact of solving wells jointly with reservoir cells.

### Step 11.3 (continued) — OSC-DETECT scope was the actual gap, now measured and fixed

Pulled the exact per-family peak values during the oscillation: `water` flat at `2.733e-7`, `oil_component`
wobbling ±8% (both deeply converged, no real oscillation), `perforation_flow` swinging `2.137e-5 ↔ 3.419e-5`
(±60%, driving the global residual peak). Applying OPM's own `d1 = |f0-f2|/f0 < 0.2 < d2 = |f0-f1|/f0` test by
hand to the perforation values: `d1 ≈ 0`, `d2 ≈ 0.6` — a textbook positive match. **The OPM-ported oscillation
detector's algorithm is correct; it was just never given the row family where this oscillation actually lives**
(`PerFamilyNorms`, Phase 7, scoped to `water`/`oil_component`/`gas_component` only, with an explicit "revisit
only with evidence" note — that evidence now exists). Separately confirmed the *active* damping mechanism at
this site was never the OPM-ported detector (`osc_phases=0` throughout, confirmed again after elimination) but
`nonlinear_history_stabilization_decision` (home-grown, site-keyed): its `repeated_site_streak` requires
`current_residual >= previous_residual * 0.98` to count as weak progress, and the clean 2-period bounce makes
every *other* iteration look like a genuine improvement, resetting the streak before its cap can escalate past
the first tier (`0.5`) — it never breaks the cycle.

**Fix**: widened `PerFamilyNorms`/`detect_oscillation` (`newton.rs`) to include `well_constraint` and
`perforation_flow` (both `Option<ResidualFamilyPeak>` in the existing `ResidualFamilyDiagnostics`, mapped to
`f64::INFINITY` when absent so "no wells present" never registers as oscillating). No new mechanism — folds
into the same `compose_damping` composition point Sub-phase 7.2 already established. 4 new/updated unit tests
(`detect_oscillation_flags_perforation_flow_two_step_relative_change`,
`detect_oscillation_ignores_missing_well_and_perforation_families`, plus the 2 existing tests updated for the
wider struct). `cargo test --lib fim::newton::` 50/50 pass.

**Live result**: heavy case `160→62` substeps, `40→20` retries, `hotspot_newton_caps` `58→16`, and — the actual
target signature — `retry_dom` shifted from `nonlinear-bad:perf@1299` to `nonlinear-bad:water@387`. Pulling
that new dominant retry's trace shows a **completely different failure mode**: `DAMPING FAILED — invalid
bounded Appleyard candidate` at `cell129` — a hard damping failure, not a residual oscillation. This confirms
the fix worked for its actual target (the `perf@1299` oscillation is gone as the bottleneck) and has uncovered
a separate, pre-existing issue underneath, out of scope for this phase. Control matrix (5 non-target cases):
bit-identical, no regression. Locked smoke (`spe1_fim_first_steps_converge_without_stall`,
`spe1_fim_gas_injection_creates_free_gas`, `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`):
3/3 pass.

**Net verdict**: heavy case still not back to the pre-Phase-10 baseline (`26` substeps) — genuine, verified
progress (`160→62`, and the specific targeted pathology resolved), not a full fix. Both changes kept live
(well elimination for its own offline-proven correctness/efficiency value and OPM-architecture consistency;
the OSC-DETECT widening for its now-decisive live evidence). `water@387`'s "invalid bounded Appleyard
candidate" failure is the next open item — a distinct, unrelated mechanism, not a continuation of this phase's
work.

### Step 11.4 (continued) — `water@387`'s Appleyard-inflection stall investigated; fix REFUTED after three variants

Direct diagnosis (`--diagnostic step` on the post-Phase-11 heavy case): the "invalid bounded Appleyard
candidate" failure at `cell129`/`water@387` traces to `appleyard_damping_breakdown`'s fw-inflection
trust-region chop (`bind=sw_inflection@cell115` in the trace — a *different* cell than the reported hotspot,
since the binding constraint is whichever cell's raw step is tightest). The chop formula
`chop = dist_to_inflection / |dsw_signed|` degenerates to exactly `0.0` when a cell's current water
saturation sits essentially at the fw inflection point (`dist ≈ 0`), and the existing "let marginal
crossings through" overshoot check (`FW_INFLECTION_OVERSHOOT_FACTOR * dist`) cannot rescue this, since a
threshold computed from a near-zero `dist` is itself near-zero and trivially exceeded by any real step. With
`damping = 0.0` exactly, `candidate_is_valid` fails (`damping > 0.0` check), and `zero_move_appleyard_
acceptance_allows` also rejects the state (its threshold is `residual_tolerance * 1e-3 * 2.0 = 2e-8`, far
below the observed `8.569e-6` residual) — Newton has no valid move in either direction and the substep fails
outright.

Three variants of a fix were built and live-tested, each targeting the same degenerate case differently:
1. **Additive margin** (`chop = (dist + max_saturation_change) / |dsw|`): heavy case `62→263` substeps,
   `retry_dom` reverted to `perf@1299`.
2. **Max-based floor** (`chop = dist.max(max_saturation_change) / |dsw|`, chosen to be more surgical — a
   floor only changes behavior when `dist` is already below the floor, unlike an unconditional additive
   margin): **bit-identical regression to variant 1** (`263` substeps, same `accepts`/`retries` breakdown) —
   `max_saturation_change` (`0.1`) turned out to be larger than `dist` on nearly every crossing in this
   problem, so the "floor" engaged almost universally, not just in the truly-degenerate case.
3. **Skip below a degenerate-range threshold** (reuse `fw_inflection_point_sw`'s own `MIN_RANGE = 1e-4`
   convention; skip the inflection chop entirely when `dist <= 1e-4`, leaving the ordinary unconditional
   `sw_appleyard` cap as the sole saturation-change bound at that state): heavy case `62→238` substeps,
   `retry_dom` again reverted to `perf@1299`.

All three variants passed their own focused unit tests (each correctly fixes the isolated degenerate case
they were tested against) and left the 5 non-target control-matrix cases unaffected — but each caused a
substantial regression specifically on the heavy case, and each regression brought back the *already-fixed*
`perf@1299` oscillation pattern rather than any new failure mode. This is a genuine, repeated negative
signal, not a single false start: the heavy case's Newton trajectory is evidently sensitive enough to this
exact site's damping value that essentially any change here perturbs the accept/retry path into re-visiting
a different, already-addressed problem, rather than net-improving anything. **Reverted cleanly** to the
original chop formula (all three variants collapse to the same reversion); heavy case reconfirmed back to
the known-good `62` substeps / `0/20/0` retries / `hotspot_newton_caps=16` / `retry_dom=water@387` state.
Recorded as `FIM-NEWTON-007` (REFUTED) — do not re-attempt a local chop-formula change at this exact site
without new evidence explaining *why* it is this sensitive (e.g. characterizing what specifically links
cell115's damping decision to cell1299's/perf1's oscillation many iterations and possibly substeps later).

### Task #37 — the sensitivity mechanism found and fully explained; no further local fix pursued

Traced substep 59's exact retry sequence (`--diagnostic step`, same post-Phase-11 heavy case). What actually
happens is precise and deterministic, not chaotic:

- Substep 59 starts (`iteration 0`, before any update) with residual `8.569e-6` — comfortably inside the
  *ordinary* Newton tolerance (`1e-5`) but not inside the much stricter "already converged, skip the update
  entirely" entry guard, which requires `residual <= residual_tolerance * NOOP_ENTRY_EXACT_FACTOR = 1e-5 * 1e-3
  = 1e-8` (`newton.rs:2555-2556`) when the state hasn't materially changed — which it hasn't, since iteration 0
  always starts from the unmodified previous state. `8.569e-6` is `~857x` too large for this gate, so Newton
  must attempt a real update.
- That update gets `cell115`'s fw-inflection trust-region chop applied (a *different* cell from the reported
  hotspot `cell129`, since the binding constraint is whichever cell's own raw step is tightest that iteration),
  which computes exactly `damping = 0.0` — legitimate protection, not a bug on its own (a cell genuinely at the
  fw inflection point has `dist ≈ 0`, so any real step "crosses" it and the chop that would land exactly at the
  boundary is itself ≈0).
- With `damping = 0.0`, the candidate is invalid, and the rescue path (`zero_move_appleyard_acceptance_allows`)
  is *also* gated at `residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR = 1e-5 * 1e-3 *
  2.0 = 2e-8` (`newton.rs:2241-2246`) — similarly far below `8.569e-6`. Newton has no valid move in either
  direction; the substep fails and the retry ladder halves `dt`.
- The residual scales down almost exactly **quadratically** with `dt` across the 5 retries this substep needed
  (`8.569e-6 → 2.142e-6 → 5.356e-7 → 1.339e-7 → 3.347e-8 → 8.368e-9`, each halving of `dt` roughly quartering
  the residual) until it finally crosses the `1e-8` entry-guard threshold and gets accepted — confirmed in the
  trace as a genuine, exact local plateau: every cell in the `cell129` neighborhood shows literally `dP=+0.00
  dSw=+0.0000` (no movement at all, at any precision shown). This is not a bug; it is the retry ladder correctly
  discovering that this region of the reservoir has reached local steady-state partway through the outer
  timestep, and mechanically shrinking `dt` until the residual is small enough to accept that as fact.

**Why the three `FIM-NEWTON-007` variants backfired, precisely**: `cell115`'s zero damping and the tight
entry/zero-move thresholds are each individually legitimate and — for the thresholds — already an explicit,
previously-litigated project decision (`FIM-NEWTON-004`: do not widen acceptance above tolerance). Both are
*single global scalars* applied to the *entire* Newton update vector, not per-cell values. Loosening either one
doesn't just let `cell115`/`cell129`'s own local plateau resolve faster — it lets the *whole* global Newton step
move further in the *same* iteration, including at whatever other site happens to be marginal that iteration
(here, `perf1`, whose oscillation `FIM-NEWTON-006` had just fixed). The "sensitivity" is not a special
`cell115`↔`perf1` relationship; it is the ordinary consequence of coupling every cell's allowed movement through
one shared scalar, which is an inherent property of scalar-damped Newton globalization generally (OPM's own
persistent relaxation scalar has the same global-coupling character) rather than something specific to this
codebase's implementation.

**No further local fix pursued at the time.** The two candidate levers (loosen the inflection chop; loosen the
entry/zero-move acceptance gates) are each already-explored territory with a clear negative verdict, and a fix
that actually addresses this without the same side effect would need per-cell or per-region damping/acceptance
criteria — a materially larger architectural change than a chop-formula tweak. Recorded as understood at the
time; `62` substeps stood as the interim heavy-case baseline (down from the pre-Phase-11 `160`, still short of
the pre-Phase-10 `26`) — **superseded by Task #38 below**, which found a materially better `k` value for the
*existing* `FW_INFLECTION_OVERSHOOT_FACTOR` mechanism rather than trying to change its formula.

### Task #38 — user pointed at prior art (`FIM-DAMP-002`/`003`); re-swept `k` under the current bundle, found a new stable point

User recollection, confirmed by a docs search: the "loosen the inflection chop" direction was already explored
in depth in April 2026, well before this session — `docs/FIM_LINEAR_SOLVER_AUDIT.md` "Fix A3" and
`docs/FIM_CHOP_WIDEN_EXPERIMENT.md`. Two directly relevant prior results:

- **`FIM-DAMP-002` (REVERTED)**: removing the inflection chop entirely — full alignment with OPM, which has no
  equivalent mechanism — was tried on a dedicated branch (`experiment/fim-no-inflection-chop`) and failed on
  *both* axes: substeps got worse (`27→162` on that era's case 3) and physics accuracy got worse (`FOPT
  3883→3019`, a genuine `-22%` loss vs. the converged fine-dt reference `3826`). The chop is doing real
  correctness work, not "OPM-inconsistent extra conservatism" — it compensates for ResSim's linear solver
  (no full AMG-CPR, unlike OPM) producing wilder raw Newton directions than OPM's.
- **`FIM-DAMP-003` (PROMOTED)**: the live `FW_INFLECTION_OVERSHOOT_FACTOR=1.2` came from a deliberate k-sweep
  (`k ∈ {1.0, 1.2, 1.5, 2.0, ∞}`) on that era's linear solver, showing a *monotonic* trend — more loosening,
  worse on both substeps and FOPT. `k=1.2` was the identified sweet spot, with an explicit retry condition:
  "retune only with k-sweep and fine-dt reference."

This session's `FIM-NEWTON-007` (three variants relaxing the chop's degenerate-`dist≈0` case) was, in
retrospect, more points along that same already-swept axis — the regression found there is a re-confirmation
of the April trend, not new information, and cross-referencing this prior art first would have saved three
live-test cycles. **The linear solver has changed substantially since the April sweep** (Phase 10's loosened
tolerance/budget/block-ILU0, Phase 11's well elimination) — `FIM-DAMP-003`'s own retry condition is satisfied,
so a fresh k-sweep under the *current* bundle is legitimate, not another attempt at the same refuted direction.

**Sweep result on the heavy case (`k`, substeps):**

| `k` | substeps | retries | `hotspot_newton_caps` | `retry_dom` |
|-----:|---------:|--------:|----------------------:|---|
| 1.0 | 248 | 51 | 105 | `perf@1299` |
| 1.1 | 32  | 13 | 8   | `water@1215` |
| 1.15 | 214 | 49 | 91  | `perf@1299` |
| 1.2 (April sweet spot, now stale) | 62 | 20 | 16 | `water@387` |
| **1.25 (chosen)** | **32** | **13** | **7** | `water@1215` |
| 1.3 | 32 | 13 | 7 | `water@1215` (identical to 1.25) |
| 1.5 | 204 | 48 | 95 | `perf@1299` |
| 2.0 | 134 | 34 | 51 | `water@819` |

**The `k`↔substep relationship is genuinely chaotic, not smooth** — `k=1.15` (214 substeps) sits *between* two
good values (`1.1`, `1.25`/`1.3`, all `32`), a hallmark of Newton-trajectory bifurcation (a retry/accept
decision flips discretely at some critical iteration depending on the exact `k`), not a tunable trend. This
means picking a value because it "looks good" in one measurement would be exactly the kind of trial-and-error
this project's discipline exists to avoid — the reason `k=1.25` is defensible is that `k=1.25` and `k=1.3`
produce **bit-identical** trajectories (same `accepts`/`retries`/`hotspot_newton_caps`/production numbers),
a genuine stable plateau, unlike the isolated single points at `1.1` or `1.2`.

**Promoted `k=1.25`** (middle of the demonstrated `[1.25, 1.3]` stable range). Full control matrix (5
non-target cases) bit-identical; locked smoke 3/3 (`spe1_fim_first_steps_converge_without_stall`,
`spe1_fim_gas_injection_creates_free_gas`, `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`).
Checked the new dominant retry site (`water@1215`/`cell405`) directly — same benign, already-understood
local-Sw-plateau retry-ladder mechanism from Task #37 (`DAMPING FAILED`, residual scaling quadratically with
`dt` across retries, genuine zero-movement plateau on acceptance), not a new failure mode, just occurring less
often (13 retries vs. 20) and at a different cell. Recorded as `FIM-DAMP-004`.

**Net**: heavy case now at `32` substeps — down from `160` at the Phase-11 low point, `62` at the interim
Task #37 baseline, and close to (though not exactly at) the pre-Phase-10 `26`. Production numbers (`oil`,
`inj`) stayed in the same ballpark across all tested `k` values (no gross physics breakdown at any point in
the sweep), though a proper fine-dt reference re-derivation under the current bundle (matching the April
methodology) has not been done and would be needed before treating `26` itself as the target to chase further.

### Task #38 (continued, 2026-07-06) — fine-dt FOPT reference: `k=1.25` has a real accuracy cost

Closed the gate this task's own writeup flagged as skipped: re-derived the April `FIM-DAMP-003` fine-dt
methodology (`docs/FIM_CHOP_WIDEN_EXPERIMENT.md` "case 3") under the current bundle, commit `43c6a1d` plus
the local `FW_INFLECTION_OVERSHOOT_FACTOR` edits described below (each rebuilt via `bash scripts/build-wasm.sh`
before its run):

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 16 --dt 0.0625 --diagnostic outer --no-json
```

| Configuration | fine-dt FOPT (`oil` @ step 16, `time=1.0000d`) | vs. OPM converged (`3826.12`) |
|---|---:|---:|
| April, old (tight-tolerance) bundle, `k=1.2` (`FIM-DAMP-003`, historical) | 3826.36 | +0.01% |
| **Current bundle, `k=1.2`** (isolation run, this task) | 3845.38 | +0.50% |
| **Current bundle, `k=1.25`** (`FIM-DAMP-004`, live) | 3883.47 | +1.50% |

Full 16-step trace for `k=1.25` (the live value): steps 1-11 climb smoothly (`oil` 3349→3902), step 12 turns
over (3902.25, a small decline begins), steps 13-14 continue declining under retry fragmentation
(`retry_dom` shifts to `nonlinear-bad:water@1215`), and steps 14-16 freeze bit-identically at `oil=3883.47`/
`inj=3872.87` — confirmed as the documented benign local-plateau replay mechanism (`accepts=1+5+1018`, the
`1018` being replayed hotspot-plateau bookkeeping per the `fim-solver-debug` skill's reading guide), not a
stall bug. Both `k=1.2` and `k=1.25` isolation runs (rebuilt from a temporarily-edited constant, then
restored to `1.25` and rebuilt again to confirm bit-identical reproduction — `3883.47` reproduced exactly)
show this same tail shape; only the final magnitude differs.

**Isolation result**: rerunning the identical fine-dt command at `k=1.2` under the *unchanged* current bundle
(same tolerance/budget/block-ILU0/well-elimination) gives `3845.38`, almost 2x closer to the OPM reference
than `k=1.25`'s `3883.47`. This cleanly separates two effects:

1. The Phase 10/11 bundle itself (not touched by this row) already costs ~0.5% FOPT drift vs. April's
   validated 0.01% match — a previously unquantified, unstated cost of the tolerance-loosening/block-ILU0/
   well-elimination changes, not attributable to `k` at all.
2. `k=1.25` specifically adds a further ~1.0 percentage point of drift on top of that (`0.50%→1.50%`) — a
   real, measured accuracy cost from letting more "marginal" fw-inflection crossings through unchopped, not
   a bug or measurement artifact.

**Conclusion**: the `62→32` substep win from `FIM-DAMP-004` is real, but it is **not accuracy-neutral**. The
promotion stands (registry verdict remains `PROMOTED`, updated with this caveat) because reverting outright
would only buy back partial accuracy (`k=1.2` here is still `0.5%` off, not `0.01%`) at the cost of doubling
substeps (`32→62`) — not an unambiguous win either way. This is a genuine, open trade-off, not a settled
question; flagged to the user rather than resolved unilaterally. Candidates for a real fix, not yet
attempted: (a) determine whether the bundle-level `0.5%` drift is itself fixable (tolerance-loosening
accuracy cost was never checked against a fine-dt reference when `FIM-LINEAR-008` was promoted — this may be
the bigger, more foundational gap); (b) a `k` value between `1.1` and `1.25` that hasn't been fine-dt-checked
(the chaotic `k`↔substep relationship means this isn't a simple bisection); (c) revisit whether the inflection
chop's role should shift once/if AMG (Bundle C) lands, per the existing `docs/FIM_OPM_ALIGNMENT_STRATEGY_2026-04-26.md`
guidance that per-cell damping and dropping the chop are deferred until after AMG.

## Validation Shortlist
- Water shelf summary:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json`
- Gas shelf outer replay:
  - `node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic outer --no-json`
- Over-threshold coarse probe:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 --diagnostic step --no-json | rg -m 8 "cpr=\[|FIM retry summary|FIM step done|Newton: dt="`
- Exact-dense threshold control:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 22x22x1 --steps 1 --dt 0.25 --diagnostic step --no-json | rg -m 8 "cpr=\[|FIM retry summary|FIM step done|Newton: dt="`
- Bounded control matrix additions (added 2026-07-02, part of the fim-solver-debug skill's routine gate set):
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json`
  - `node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json`

### Task #41 (2026-07-07) — Gap factor budget: heavy case, ResSim vs OPM Flow side-by-side

User directive: stop optimizing individual mechanisms; attribute the full 2-3-orders-of-magnitude
wall-clock gap to OPM Flow before designing the fix. Both sides measured on this machine, same
day, commit `468a103` (clean tree; wasm rebuilt from exactly this source).

**ResSim side** (exact command, verbatim summary):

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json
step=  1 | time=1.0000d | outer_ms=36705.8 | history+=32 | substeps=32 | accepts=31+4+1764 | retries=0/13/0 | avg_p=353.55 | oil=3893.94 | inj=3883.24 | gor=0.00 | dt=[4.003e-5,6.866e-2] | growth=hotspot-repeat | hotspot_newton_caps=7 | retry_dom=nonlinear-bad:water@1215 | fim_ms=36658.0 | lin_ms=34732.0 | pc_ms=32867.0 | retry_ms=11803.0
```

Counted from the step trace: **336 real Newton iterations** across **44 Newton solve attempts**
(31 real accepted substeps + 13 retry rungs), 12 linear-solver failures, per-Newton linear
iterations 1-4 (median 3). Wall-clock 36.9 s. A `--capillary false` rerun is bit-identical
(`oil=3893.94`, same trajectory), confirming the preset's effective Pc is nil here.

**OPM Flow side** (Flow 2025.10, `/usr/bin/flow`, default options): adapted the tracked parity
deck `origin/fim-opm-continuation-plan:opm/reference-decks/water-medium-step1/CASE.DATA` to the
heavy case (`DIMENS 12 12 3`, 432 cells, producer moved to 12 12, `TSTEP 1.0`, array counts
rescaled — pure `sed`, no physics edits). Verbatim result:

```
 Newton its=11, linearizations=12 (0.0sec), linear its= 13 (0.0sec)
Number of timesteps:             1
Simulation time:                 0.05 s
  Linear solve time:             0.03 s  (Linear setup: 0.03 s)
Overall Newton Iterations:      11   Overall Linear Iterations:      13   (Wasted: 0; 0.0%)
```

**Caveat (recorded, not hidden):** the deck is a *cost-class* reference, not a physics reference —
its FOPT (2609.5) does not match ResSim (3893.9); porosity/viscosity/relperm/Pc/wells/perm all
verified matching, so the residual parity gap is inherited from the branch's Phase-0 deck lineage
(whose own doc says numerics-parity tolerances were never stated). The April OPM converged FOPT
(3826.12) remains the physics reference. An 11-Newton/zero-cut/0.05 s cost class is robust to
this level of physics mismatch.

**The factor budget (36.9 s / 0.05 s = 738x), multiplicative decomposition:**

| Factor | OPM | ResSim | Ratio |
|---|---:|---:|---:|
| Newton iterations for the 1-day step | 11 | 336 | **30.5x** |
| Wall-clock per Newton iteration | 4.5 ms | 109 ms | **24x** |

`30.5 x 24 ≈ 730x` — the decomposition closes. Within each factor:

- **Newton-count factor (30.5x)** is nonlinear-layer architecture: 32 substeps + 13 retry rungs
  where OPM takes ONE step with zero cuts. Per solve attempt ResSim averages 7.6 iterations —
  the same order as OPM's 11 for the whole day. The multiplication comes from acceptance +
  timestep control, not from Newton being weak per-attempt. 32% of wall-clock (`retry_ms=11803`)
  is spent on discarded retry-rung work (OPM wasted: 0%).
- **Per-iteration cost factor (24x)** is almost entirely preconditioner build: `pc_ms=32867` =
  95% of `lin_ms` = **89% of total wall-clock**. ResSim rebuilds quasi-IMPES weights + block-ILU0
  + the O(n³) dense coarse inverse at every Newton iteration; OPM's default `--cpr-reuse-setup=4`
  reuses the CPR setup and fully recreates it only every 30 linear solves.

**OPM installed-binary defaults captured for the design work** (from `/usr/bin/flow --help-all`,
Flow 2025.10 — no memory-derived numbers): `tolerance-mb=1e-7` (relative to total mass in place,
relaxed `1e-6`), `tolerance-cnv=0.01` (LOCAL max saturation error, relaxed `1`),
`relaxed-max-pv-fraction=0.03` (3% of pore volume may violate CNV even during strict iterations),
`min-strict-cnv-iter=-1` (relaxed kicks in when the Newton budget is exhausted),
`tolerance-wells=1e-4`, `ds-max=0.2`, `dp-max-rel=0.3`, `newton-max-iterations=20` /
`newton-min-iterations=2`, `time-step-control=pid+newtoniteration` with
`target-newton-iterations=8`, growth `1.25`, decay `0.75`, restart factor `0.33`, max restarts
`10`; `linear-solver-reduction=0.01`, `linear-solver-max-iter=200`, `cpr-reuse-setup=4` /
`cpr-reuse-interval=30`.

Contrast with ResSim's acceptance: `scaled_residual_inf_norm` (`newton.rs:1425`) at
`residual_tolerance=1e-5` — the single worst cell/equation in the grid must individually pass
`1e-5`, roughly **1000x stricter locally than OPM's CNV `1e-2`**, with no relaxed tier, no
pore-volume exemption, and no volume-averaged criterion. The `water@1215` plateau ladders that
dominate the heavy case's retry burden are single cells sitting above `1e-5` that OPM's criteria
would have accepted many iterations earlier.

**Conclusion → design doc:** the gap splits cleanly into a nonlinear-architecture half (30x:
acceptance criteria + controller + global-scalar damping) and a per-iteration-cost half (24x:
preconditioner rebuild). Both are addressed as two coherent bundles in
`docs/FIM_BUNDLE_N_DESIGN.md` (this measurement is its motivating section). Per the standing
user directive, neither bundle is to be implemented or judged mechanism-by-mechanism against
current-architecture baselines.

### Task #43 (2026-07-07) — Bundle N step 0: port-fidelity pass against OPM source, done

Cloned `opm-simulators` at `release/2025.10/final` (commit `b8b2b9e`, the exact release of the
installed `/usr/bin/flow`) and extracted the verbatim formulas for every Bundle N item into
`docs/FIM_BUNDLE_N_DESIGN.md` §9: CNV/MB convergence (incl. the per-cell pore-volume
normalization, `B_avg` FVF weighting, and the 3%-PV relaxed-CNV rule that fires at ANY
iteration), per-cell `dsMax`/`dpMaxRel` chopping (incl. the implied `dSo = -(dSw+dSg)` term),
the `pid+newtoniteration` controller (`min(PID, iteration-target)` with damping factors
1.0/3.2 — NOT the 1.25/0.75 rates, which belong to the non-default simple controller; this
corrected a real error in the design doc's original sketch), substep failure/growth clamps
(0.33 restart factor, 3x/2x growth clamps), and linear-failure handling (`reduction ≤ 0.01`
accepted with a warning; no direct-solver fallback exists in OPM's path). Also verified
`convergence-monitoring` is default-off → excluded. Design doc updated in place; §9 is now the
implementation contract for checkpoints 1-5.

### Bundle N checkpoint 1 (2026-07-07) — inert CNV/MB measurement: criteria are NOT the direct waste; the damping stall is

Implemented the OPM CNV/MB convergence measures (design doc §9.1) as a read-only per-iteration
diagnostic in `fim/newton.rs` (`cnv_mb_from_parts` pure core + `cnv_mb_diagnostics` wrapper +
`CNV-MB` trace line; 2 focused unit tests incl. the 3%-PV-rule case). One unit adaptation,
recorded in code comments: ResSim's residual is already dt-integrated (surface m³), so OPM's
`* dt` factor is intentionally absent; also noted that ResSim's existing `build_equation_scaling`
is structurally `pv/(dt·B)` — i.e. the current scaled inf-norm is already a per-cell-B CNV times
dt, which explains task #37's "residual scales quadratically with dt" observation.

**Behavioral no-op gate (all passed, commit to follow this entry):** locked smoke 3/3; full
control matrix bit-identical (20x20x3 `8, 0/3/0`; 22x22x1 `4, 0/2/0`; 23x23x1 `4, 0/2/0`;
gas 20x20x3 `2, 0/1/0`; gas 10x10x3 x6 steady `2/step`); heavy case bit-identical
(`substeps=32 | accepts=31+4+1764 | retries=0/13/0`, `oil=3893.94`, `hotspot_newton_caps=7`).

**Measurement** (heavy case `--diagnostic step`, 44 Newton solve attempts, 358 traced
iterations):

- Iterations if OPM's criteria had decided acceptance on these same trajectories: **357 of 358 —
  no saving.** OPM's test would accept earlier in only a handful of blocks (9), later or never in
  the rest.
- **35 of 44 blocks never pass OPM's criteria at all** within their attempted iterations. Binding
  criterion: **MB(1e-7) alone in 32 blocks, both in 4, CNV alone in 0.** The "1000x looser local
  CNV" framing is measured to be a non-factor on ResSim's own trajectories — CNV is comfortably
  met whenever ResSim's trajectories settle.
- The signature (longest block, dt=0.037, 20 iterations): MB contracts `1.7e-2 → 2e-3` over 9
  iterations, drops to `1.1e-5`, then **stalls oscillating at ~2e-6 for 10 straight iterations**
  with CNV at `8e-4` (passing) and violating-PV at 0. Neither ResSim's nor OPM's criteria accept
  a stalled state; ResSim then dt-halves. Distribution across the 32 MB-blocked blocks: final MB
  ranges `1.3e-7` (1x over) to `1e-3` (10000x, blowing-up rungs), median `2.2e-6` (~22x over).

**Interpretation — this refutes the simple half of the design's §1 narrative and confirms the
bundle thesis:**

1. Porting OPM's *acceptance criteria* alone (N1) would fix nothing on this case and would
   likely regress: OPM's MB `1e-7` is effectively TIGHTER than ResSim's exit states (23 of 31
   ResSim-accepted substeps end at OPM-MB between `1.3e-7` and `~2e-6`). The criteria are not
   where the 30x Newton-count factor lives *given ResSim's current update dynamics*.
2. The waste lives in the *trajectory*: the damped-Newton stall at ~2e-6 (global damping scalar
   collapsing the update at local plateaus — task #37's mechanism) burns half of every capped
   block and forces the dt-halving ladder. OPM's per-cell chop (N2) is what lets its Newton walk
   through the same plateau to `1e-7` in 11 iterations flat.
3. Accuracy note: ResSim's accepted states sitting at `1-20x` of OPM's strict MB tolerance means
   current physics acceptance is roughly OPM-comparable — no accuracy scandal in either
   direction from the criteria themselves.

**Consequence for the build order** (design doc §6 updated): checkpoint 2 becomes N2 (per-cell
chopping) — the measured load-bearing item — with N1's acceptance flip moving after it. The
bundle's end-state gates are unchanged; this is a development-order change only, fully within
the "judge only at the end" principle.

Replay: `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1
--dt 1 --diagnostic step --no-json`, grep `CNV-MB`; analysis script inline in the session log.

### Bundle N checkpoint 2 (2026-07-07) — `OpmAligned` flag + N2 per-cell chopping live behind it

Implemented `FimNonlinearFlavor::{Legacy, OpmAligned}` on `FimNewtonOptions` (default `Legacy`)
and OPM's per-cell update chopping (`opm_per_cell_chopped_update`, design doc §9.2: per-cell
`satAlpha = dsMax/maxSatDelta` incl. the implied `dSo = -(dSw+dSg)`, Rs-meaning non-negativity
guard, `±0.3·p` relative pressure clamp; oscillation-relaxation scalar pre-multiplies the raw
update matching OPM's dampen-then-chop order). Under `OpmAligned` the Legacy update-limiting
layer (history stabilization + global Appleyard scalar + inflection chop +
`candidate_respects_update_bounds`) is bypassed; everything else (acceptance, entry guards,
retry ladder, controller) is still Legacy at this checkpoint. Plumbing:
`setFimOpmAlignedNonlinear` wasm setter, `--opm-aligned` runner flag. 4 focused unit tests
(implied-So chop, per-cell independence, relative dp clamp, Rs guard, relax-then-chop order).

**No-op gate (default Legacy), all passed:** locked smoke 3/3; control matrix + heavy case
bit-identical (heavy: `substeps=32 | accepts=31+4+1764 | retries=0/13/0`, `oil=3893.94`,
`hotspot_newton_caps=7`).

**Informational first `--opm-aligned` heavy-case run** (NOT a gate — intermediate bundle state
under Legacy acceptance/controller, judged only at bundle end per the design principle):

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --opm-aligned --diagnostic step --no-json
step= 1 | substeps=226 | accepts=224+5+1568 | retries=0/48/0 | oil=3804.38 | hotspot_newton_caps=132 | retry_dom=nonlinear-bad:perf@1299   (141.5 s)
```

End-to-end worse under Legacy gates, exactly as the bundle thesis predicts for a mismatched
intermediate — but the CNV-MB probe (checkpoint 1, still live) shows the *Newton quality
underneath* moved decisively in the right direction:

| CNV-MB verdict per Newton solve attempt | Legacy damping (44 blocks) | Per-cell chop (272 blocks) |
|---|---:|---:|
| OPM-acceptable mid-solve (strict/pv-relaxed) | 9 | 7 |
| OPM-acceptable at exhaustion (relaxed MB `1e-6` / CNV tier) | 12 | **251** |
| Truly failing under full OPM rules | 23 (52%) | **14 (5%)** |
| Median final MB in non-accepted blocks | `2.2e-6` | **`2.9e-7`** |

The MB stall that checkpoint 1 identified as the binding failure (damped Newton frozen at
`~2e-6`) is gone — per-cell chopping brings 95% of solve attempts to a state OPM's full
acceptance rules would take (vs 48% before). The 226-substep fragmentation is now almost
entirely the *Legacy acceptance layer* (inf-norm `1e-5` + entry guards) rejecting
OPM-acceptable states, plus the Legacy hotspot controller reacting to those rejections.
Checkpoints 3-4 (N1 acceptance incl. relaxed tiers + N5 linear handling, then N3 controller)
are precisely the harvest step. Caveat recorded: "acceptable at exhaustion" means ~20-iteration
substeps; the N3 controller's 8-iteration target should settle on fewer, larger substeps —
end metrics (§5) remain the only judgment.

### Bundle N checkpoint 3 (2026-07-07) — N1 acceptance criteria + N5 linear handling live behind `OpmAligned`

Implemented, under `OpmAligned` only: (N1) the per-iteration entry check now decides
acceptance purely via `opm_conv.would_accept` (design doc §9.1's CNV/MB, including the
final-iteration relaxed tiers via a new `relax_final_iteration: bool` param on
`cnv_mb_from_parts`/`cnv_mb_diagnostics`), gated on `iteration >= 1` (OPM's
`newton-min-iterations=2` translated to this loop's 0-indexing); (N5) the linear-solve
failure branch is replaced entirely — no direct-LU fallback ladder, no
dead-state/restart-stagnation/zero-move bypass bookkeeping; a solve is used as-is if
converged, accepted with a trace if it achieved OPM's relaxed reduction (`< 0.01` relative to
`rhs_norm`, both already present on `FimLinearFailureDiagnostics`), else the Newton iteration
fails immediately (returns, no rescue) exactly like OPM's `NumericalProblem` throw. 1 new unit
test (`cnv_mb_relax_final_iteration_applies_relaxed_tiers_unconditionally`).

**A real bug was found and fixed during this checkpoint's own gating, before any live-run
measurement was trusted** (worth recording since it nearly produced a false read): two
*additional* Legacy-only acceptance shortcuts inside the loop were not yet gated on
`opm_aligned` — a mid-iteration "raw update is already tiny" check (`update_tolerance` +
`ENTRY_RESIDUAL_GUARD_FACTOR`, no OPM analog) and the zero-move-Appleyard rescue on an invalid
candidate — plus the post-loop exhaustion check (which must use OPM's final-iteration-relaxed
CNV/MB, not Legacy's strict tolerances, since it genuinely corresponds to OPM's
`iteration == maxIter` case). Caught because a first small-case (`22x22x1`) test run showed
zero `OPM-CONVERGED`/`LINEAR-ACCEPT`/`LINEAR FAILED` trace lines fire across 37 iterations —
i.e. the new mechanisms were silently never reached, exactly the kind of "looks fine, isn't"
result the project's own baseline discipline exists to catch before trusting a number. Fixed
all three sites; re-ran the same small case and confirmed `OPM-CONVERGED` now fires exactly
once per accepted substep.

**No-op gate (default Legacy), all passed:** locked smoke 3/3; full control matrix + heavy
case bit-identical to the recorded baseline (heavy: `substeps=32 | accepts=31+4+1764 |
retries=0/13/0`, `oil=3893.94`, `hotspot_newton_caps=7`).

**Informational `--opm-aligned` runs (not gates — intermediate bundle state, N3 controller
still Legacy):**

- `22x22x1` (bounded control-matrix case, 484 cells, comparable size to the heavy target):
  `substeps=14 | retries=0/7/0 | hotspot_newton_caps=9`, 15.8s — vs Legacy's `substeps=4 |
  retries=0/2/0` at well under 1s. Worse, as expected for a mismatched intermediate: the
  Legacy retry ladder (hotspot-repeat cooldown, retry factor selection) was tuned against
  Legacy's own acceptance/damping behavior and doesn't yet know how to drive dt for the new
  per-cell-chopped, CNV/MB-accepted trajectory efficiently.
- Heavy case (`12x12x3`, the actual target): **timed out past 280s** (previously 141s under
  checkpoint 2's chop-only state). Not investigated further live — burning more wall-clock on
  a known-intermediate, known-mismatched state contradicts the bundle's own "judge only at the
  end" principle, and `MAX_SUBSTEPS=100_000` means this is a slow, not infinite, run (a
  genuine safety-valve ceiling, not evidence of a hang). The retry ladder is very likely
  driving dt down repeatedly against `water@1215`'s local plateau without an N3-shaped
  mechanism to recognize "OPM would already call this converged" and stop shrinking — exactly
  the gap checkpoint 4 (N3, the `pid+newtoniteration` controller) closes.

**Consequence for the build order:** checkpoint 4 (N3, timestep controller) is now the clear
next step — it is very likely load-bearing for making the heavy case tractable again, not an
optional polish item, since the Legacy retry ladder is now actively working against the
already-fixed N1/N2/N5 mechanics rather than neutral to them.

### Bundle N checkpoint 4 (2026-07-07) — N3 timestep controller live behind `OpmAligned`

Implemented, under `OpmAligned` only (design doc §9.3/9.4): OPM's `pid+newtoniteration`
controller (`opm_accepted_step_growth_decision` = `min(dt_pid, dt_iter)` then the two growth
ceilings — always `solver-max-growth=3.0`, further `solver-growth-factor=2.0` if this substep
needed any retries), backed by a new `opm_relative_change` (OPM's `BlackoilModel::relativeChange()`
— sum-of-squares of pressure/saturation deltas between consecutive ACCEPTED substep states,
normalized by the new state's own sum-of-squares, implied `So` included) feeding a 3-value
rolling error history reset each outer step. On retry, the flat `solver-restart-factor=0.33`
replaces both ResSim's failure-classified `retry_factor` and the repeated-hotspot acceleration
on top of it. The Legacy cooldown/gas-carryover trial-dt clamps and the "retry-hold" growth
override are skipped entirely for `OpmAligned` (no OPM analog); their bookkeeping keeps
running unconditionally (harmless — nothing reads it under `OpmAligned`). 9 new unit tests
(`opm_relative_change_*`, `opm_pid_dt_*`, `opm_iteration_count_dt_*`,
`opm_accepted_step_growth_decision_*`).

**Pre-existing, unrelated test failures found and ruled out before trusting the smoke gate**:
`fim::timestep::tests::changing_hotspot_resets_extra_growth_cooldown_budget`,
`repeated_same_hotspot_extends_growth_cooldown_budget`,
`fim_enabled_step_advances_time_and_records_history_for_closed_system` fail on a clean checkout
of commit `41d45f2` (checkpoint 3, before any of this checkpoint's edits) — confirmed via
`git stash` + rerun. Unrelated to Bundle N; logged in `TODO.md`, not investigated further here.

**No-op gate (default Legacy), all passed:** locked smoke 3/3; full control matrix + heavy case
bit-identical to baseline (heavy: `substeps=32 | accepts=31+4+1764 | retries=0/13/0`,
`oil=3893.94`, `hotspot_newton_caps=7`).

**Informational `--opm-aligned` run (not a gate — full end-metric evaluation is §5, after N4):**

`22x22x1` (same case tracked at checkpoints 2/3 for a consistent trend):

| Checkpoint | substeps | retries | attempts |
|---|---:|---:|---:|
| Legacy | 4 | 2 | 6 |
| 2 (chop only) | — (not separately run) | — | — |
| 3 (chop + N1 + N5) | 14 | 7 | 21 |
| 4 (+ N3 controller) | 20 | 12 | 32 |

**Honest finding: N3 alone made this small case worse, not better.** Growth-decision trace
breakdown across the 20 accepted substeps: 7 at the `3.0` ceiling (clean accepts), 11 at the
`2.0` post-retry ceiling (i.e. the large majority of substeps needed at least one retry before
accepting), 2 at smaller PID/iteration-bound values. The retry_dom stays pinned at the same
site (`oil@415`) throughout. The most likely explanation: this case's stubborn site needs a
*more aggressive* dt cut than OPM's flat `0.33` to get past — exactly what Legacy's
repeated-hotspot-acceleration (shrinking to `0.2` after repeats) was tuned to do, and which N3
deliberately does not replicate (OPM's own retry backoff really is failure-class- and
site-agnostic). This is a genuine, not-yet-resolved open question the design's checkpoint list
did not anticipate: N3's simplicity may trade away a real capability Legacy had for navigating
specific repeated-failure sites. Not chased further live (heavy case not re-attempted this
checkpoint either, for the same "don't chase intermediate mismatches" reason as checkpoint 3) —
recorded honestly rather than declared a win.

**Consequence for the build order**: proceed to N4 (mechanism deletion) as planned — deleting
the compensating mechanisms is what actually tests whether the *combined* bundle (not each
piece in isolation against a Legacy-shaped baseline) resolves the heavy case. If the full
bundle's end-metric evaluation (§5) shows the retry-navigation gap is real and material, the
recorded fallback is revisiting N3's retry-factor choice specifically (e.g., OPM's own
`solver-max-restarts`-scaled backoff, or accepting this as a genuine, small, permanent
trade-off versus OPM if the alternative — reintroducing site memory — reopens the
architectural inconsistency this whole bundle exists to remove).

### Bundle N checkpoint 5 (2026-07-07) — N4 mechanism deletion sweep: the forensic prediction holds

Deleted, under `OpmAligned` only, exactly the three mechanisms checkpoint 4's log forensics
identified as having no OPM analog and actively causing the observed regressions:

1. **`candidate_materially_changed` dropped from `candidate_is_valid`** (the dominant fix,
   confirmed by checkpoint 4's forensics: 11 of 12 failures on the tracked small case were
   this exact exit firing on a near-zero-move update). OPM has no "was the raw update
   materially small" validity check at all — a near-zero update is still a normal Newton step;
   the loop keeps iterating and the entry check (or post-loop exhaustion) decides.
2. **Residual-stagnation bailout** (`stagnation_count >= 3` → accept-or-bail, both Legacy-scaled)
   gated behind `!opm_aligned`. OPM never inspects residual *trend* mid-solve, only its
   absolute value against CNV/MB at each entry check.
3. **Preemptive direct-solve bypass ladder** (`any_preexisting_bypass`, gating dead-state/
   restart-stagnation/zero-move/repeated-zero-move flags) gated behind `!opm_aligned` as a
   whole, closing a latent gap checkpoint 3 didn't fully cover: `repeated_zero_move_direct_bypass`
   doesn't depend on `used_fallback` (unlike the other three, already inert for `opm_aligned`
   since `used_fallback` never becomes true under N5) and could still have forced a direct
   solve ahead of any FGMRES-CPR attempt, silently reintroducing exactly the "no direct-solve
   fallback in the loop" violation N5 (checkpoint 3) was built to remove.

`repeated_hotspot_streak` checked and confirmed already safe (only read inside the
already-`opm_aligned`-gated `nonlinear_history_stabilization_decision` call from checkpoint 3).

**No-op gate (default Legacy), all passed:** locked smoke 3/3; full control matrix + heavy case
bit-identical to baseline (heavy: `substeps=32 | accepts=31+4+1764 | retries=0/13/0`,
`oil=3893.94`, `hotspot_newton_caps=7`).

**Informational `--opm-aligned` runs — the forensic prediction confirmed directly:**

`22x22x1` (the case tracked since checkpoint 2, full trend):

| Checkpoint | substeps | retries | attempts | DAMPING FAILED |
|---|---:|---:|---:|---:|
| Legacy | 4 | 2 | 6 | n/a |
| 3 (chop + N1 + N5) | 14 | 7 | 21 | (not counted) |
| 4 (+ N3 controller) | 20 | 12 | 32 | 11 |
| **5 (+ N4 sweep)** | **12** | **1** | **13** | **0** |

`DAMPING FAILED` dropped to exactly 0 as predicted; `post-loop CONVERGED` still never fires
(0) but `post-loop NOT CONVERGED` is now only 1 (a single genuine end-of-budget failure,
`retry_dom` shifted from the stubborn `oil@415` to `water@0`) — the checkpoint-4 diagnosis was
correct and the fix resolved it directly, not incidentally. 13 attempts is close to Legacy's 6
and a completely different order of magnitude from checkpoint 4's 32.

**Two honest findings that are NOT yet resolved:**

- **Heavy case (`12x12x3`) still times out** — attempted at both `--diagnostic step` (280s) and
  the cheaper `--diagnostic summary` (400s), both killed with zero output produced. The heavy
  case's dominant site (`water@1215`, the genuine local-saturation-plateau from Task #37) is a
  different pathology class from the small case's `oil@415`/`water@0` sites; N4's fix (aimed at
  a spurious VALIDITY exit) does not obviously address a genuine physical plateau. Not
  chased further live this checkpoint — still consistent with "the real test is §5," but this
  is the first checkpoint where a *comparable* case (not just a differently-shaped small case)
  showed a real, measured win, so the heavy case's continued intractability is now a specific,
  named open risk rather than a generic "intermediate state" excuse.
- **A third case (`23x23x1`, 529 cells, part of the control matrix) surfaced a DIFFERENT
  failure mode**: `substeps=26 | retries=9/0/0` (all `linear-bad`, not `nonlinear-bad`) at
  `retry_dom=linear-bad:oil@361` — fast in wall-clock (3s, this case's linear systems are cheap)
  but structurally worse than Legacy's `4/0/2/0`. This is the first sign that N5's linear-failure
  handling (accept if reduction `<0.01`, else fail outright, no rescue) may itself be a material
  cost on some cases, distinct from the nonlinear-layer issues N1-N4 have been addressing. Not
  investigated further this checkpoint — flagged for the N4/end-metric evaluation stage.

**Consequence for the build order:** N4's sweep list (design doc, the remaining "site-keyed
history stabilization remnants"/"plateau-replay bookkeeping"/"retry-family classification as a
control input" items) is not yet fully exhausted — `history_stabilization` is already `None`
under `opm_aligned` (checkpoint 3) and plateau-replay is Legacy-only bookkeeping already unread,
but the newly-surfaced `linear-bad` finding on `23x23x1` suggests N5 itself may need a second
look before §5's end-metric evaluation, not just N1-N4's mechanisms. Recommend one more
targeted pass at the `linear-bad` finding before declaring N4 complete and moving to §5.

### Bundle N checkpoint 6 (2026-07-07) — N5 bug fix: wrong residual field, not a design gap

Investigated checkpoint 5's open item (`23x23x1`'s `linear-bad` retries) directly. Root cause:
N5's reduction check (`docs/FIM_BUNDLE_N_DESIGN.md` §9.5) used
`failure.outer_residual_norm / failure.rhs_norm`, but `outer_residual_norm` is computed at the
TOP of `gmres_block_jacobi.rs`'s restart loop from the last COMMITTED solution — on a solve
that never converges, this can stay pinned at the seed value (`rhs_norm`, i.e. the residual at
`x_0=0`) even when later restarts produced a materially better, already-*returned* candidate.
Confirmed directly from the `23x23x1` trace: every one of the 9 `LINEAR FAILED` lines reported
`reduction=1.000e0` regardless of wildly different residual magnitudes (`2.542e3` down to
`1.696e-4`) — a dead giveaway, not a coincidence. The correct quantity (matching Dune ISTL's own
`result.reduction`, computed from the solution actually returned, not an intermediate
diagnostic) is `FimLinearSolveReport::final_residual_norm`, which `gmres_block_jacobi.rs`
already sets correctly to the candidate's true residual at the max-iterations return site.
One-line fix: read `linear_report.final_residual_norm` instead of
`failure.outer_residual_norm`.

**No-op gate (default Legacy):** trivially preserved — this code path only executes under
`opm_aligned`; re-ran the full control matrix + heavy case + locked smoke 3/3 anyway (all
bit-identical to baseline) since the fix touched a function called from the shared linear
report handling.

**Informational re-runs, dramatic and directly attributable improvement:**

| Case | Before fix | After fix |
|---|---|---|
| `23x23x1` | `substeps=26, retries=9/0/0` (35 attempts), all `linear-bad`, `reduction=1.000e0` always | `substeps=12, retries=1/0/0` (13 attempts) — `LINEAR-ACCEPT relaxed reduction=9.928e-3` fires once (correctly, just under the `0.01` bar), one genuine `LINEAR FAILED` remains (`reduction=1.520e-2`) |
| `22x22x1` | `substeps=12, retries=0/1/0` | unchanged (its one remaining retry was already `nonlinear-bad`, untouched by this fix) |
| Heavy (`12x12x3`) | times out >280s | **still times out >280s** (tried again after the fix) — its dominant `water@1215` failure is `nonlinear-bad` (Task #37's genuine physical plateau), a different pathology class this fix does not touch |

**Consequence:** N5 is now correctly implemented per design, not just "inert by default and
plausible." Two of three tracked comparable-size cases (`22x22x1`, `23x23x1`) are now close to
or better than Legacy in attempt count. The heavy case remains the one unresolved item, and its
failure class (`nonlinear-bad`, not `linear-bad`) means this specific fix class is exhausted for
it — the next lead, if pursued, would need to look at the `nonlinear-bad` retry path itself
(effectively re-opening the N2/chop or N1/acceptance question specifically for that site,
which the design's own build order already treats as the terrain N1-N4 were meant to cover).
Recommend proceeding to the §5 end-metric evaluation next; further live probing of the heavy
case's `nonlinear-bad` failures without a fresh forensic angle risks repeating the same
"chase an intermediate state" pattern already flagged as unproductive at checkpoints 3-4.

### Bundle N §5 end-metric evaluation (2026-07-09) — heavy case FAILS decisively; root cause identified

Ran the heavy case (`water-pressure 12x12x3`, `dt=1`) natively in `--release` mode under
`OpmAligned` (new test `repro_water_pressure_12x12x3_opm_aligned`, added specifically because
the wasm diagnostic runner's I/O buffering made this case's timeout at checkpoints 3-6
inconclusive — the previous "times out at 280-400s" data points were never resolved to a real
number). Result, verbatim (`176m25s` wall-clock):

```
FIM step done: 18002 substeps, advanced 1.000000 of 1.000000 days
FIM retry summary: linear-bad=7 nonlinear-bad=2 mixed=0
```

**18,002 substeps** — vs Legacy's `32`, vs the §5 gate's `≤35` Newton-iteration target. This is
not a "close but not quite" result; it is 2-3 orders of magnitude over every efficiency gate in
§5. Per §5's own rule ("any gate failing → the bundle as a whole is reworked or reverted"),
**Bundle N as currently implemented does not promote.**

**Root cause identified — a specific, narrow architectural gap, not a diffuse problem across
N1-N5.** The trace tail shows the run entering a compounding dt-collapse: consecutive accepted
substeps with `max_dSat=0.0000 max_dP=0.00` (essentially zero physical change — the reservoir
has reached steady state, injector/producer balanced) but `iters=20` (hit the Newton cap) and
`growth=0.400` — repeating substep after substep. The detail trace pinpoints the cause:
`perf1 well1 ... bhp=100.000 frozen_bhp=100.000 ... dq=1.802e-1` — a producer pinned exactly at
its BHP limit, its rate-vs-BHP complementarity residual not fully resolved. This residual does
not shrink with dt (it is a discrete control-mode condition, not a smooth PDE term), so a
smaller substep does not make it converge faster — yet `opm_iteration_count_dt` (checkpoint 4)
applies its full penalty regardless: `its=20 > target=8 → dt_iter = dt/(1+(20-8)/8*1.0) = 0.4·dt`.
Because the SAME well-pinned state recurs across many consecutive substeps (nothing about it
changes as dt shrinks), the `0.4×` penalty compounds: `0.4^N → 0` rapidly, and no ordinary
substep is "lucky" enough to escape since the underlying cause is structural, not transient.

**Verified directly against the OPM source** (not assumed) that this is a genuine architecture
mismatch, not a formula bug: OPM's well equations are resolved via a **dedicated inner
iteration loop** — `WellInterface::iterateWellEquations` /
`StandardWell::iterateWellEqWithControl` / `iterateWellEqWithSwitching`
(`opm/simulators/wells/{WellInterface,StandardWell}.hpp`), called *within* a single outer
reservoir Newton iteration. OPM's `total_newton_iterations` (the exact quantity fed to
`computeTimeStepSize`, confirmed at `BlackoilModel_impl.hpp:270` and
`AdaptiveTimeStepping_impl.hpp:790` `getNumIterations_`) counts only the **outer reservoir**
iterations — well-control-switching cost is resolved inside that inner loop and is invisible
to the timestep controller. ResSim's FIM solver has no such two-level split: it is one flat
Newton loop over reservoir + well + perforation unknowns together (well-BHP/perforation-rate
rows are Schur-eliminated at the *linear* level per Phase 11, `FIM-LINEAR-010`, but not at the
*nonlinear* level — the outer Newton loop still iterates until the recovered well variables
also satisfy nonlinear convergence). Porting OPM's iteration-count growth formula literally,
using ResSim's own combined count, therefore punishes well-control-switching cost as if it were
"physics moving too fast" — a category error that OPM's own architecture never exposes because
the two costs are structurally separated there.

**This is exactly "Hypothesis A" from the original Phase 8/9 well-coupling investigation**
(`docs/FIM_CONVERGENCE_ARCHIVE_*`), independently re-derived here from a completely different
angle (a timestep-controller pathology instead of a linear-solver pathology) — a second,
convergent line of evidence that ResSim's flat well/reservoir coupling, not any single
mechanism tuning, is the real remaining architectural gap to OPM.

**What this does and does not indict:**
- N1 (acceptance), N2 (chopping), N4 (mechanism deletion), and N5 (once the checkpoint-6 bug
  was fixed) each showed real, measured, positive results in isolation on the `22x22x1` and
  `23x23x1` cases (checkpoints 5-6). This finding does not undo those.
- N3's specific formula — using the *combined* reservoir+well iteration count — is the
  identified defect. It is a narrow, well-understood gap, not a reason to distrust the whole
  bundle's design.
- The heavy case is disproportionately exposed to this gap because its geometry (a single
  producer/injector pair) reaches a well-pinned steady state partway through the 1-day step and
  stays there — exactly the condition that triggers the compounding shrink. Cases without a
  well hitting a hard control limit (most of the control matrix) would not exhibit this at all,
  which is consistent with `22x22x1`/`23x23x1` both showing genuine improvements.

**Recommendation:** do not promote Bundle N in its current form. Two concrete, scoped remediation
paths, in order of preference:
1. **Decouple the well/perforation iteration count from N3's growth formula** — track reservoir-
   cell Newton iterations separately from well/perforation-driven retries within the same
   substep, and feed only the reservoir-cell count to `opm_iteration_count_dt`. This is a
   bounded, well-scoped fix consistent with Bundle N's existing architecture (no new nonlinear
   well layer needed) and directly targets the identified mechanism.
2. **Build a genuine nested well-equation solve** (matching OPM's `iterateWellEquations`) —
   correct architecturally, but a materially larger undertaking, effectively a new sub-phase of
   its own; not scoped for this bundle.
Path 1 is recommended as the next step: cheap to try, directly targets the confirmed mechanism,
and testable on the same heavy-case repro without another 176-minute wait if a bounded substep
cap is added to the test for iteration purposes.

### Bundle N §5 follow-up (2026-07-09) — well-BHP update chop implemented, heavy-case rerun pending

Implemented the recommended fix from the §5 evaluation: OPM's well-BHP update clamp
(`--dbhp-max-rel`, default `1.0`), verified from `StandardWellPrimaryVariables.cpp::updateNewton`
at the pinned tag (not invented): `dBHP_limited = sign(dBHP) * min(|dBHP|, |bhp_current| *
dbhp_max_rel)`, then floored so the next BHP stays `>= 1 bar` (OPM's own
`bhp_lower_limit = 1 bar - 1 Pa`, simplified to `1.0` here). Added to `opm_per_cell_chopped_update`
alongside the existing per-cell reservoir chop. Perforation-rate deltas remain deliberately
unchopped — confirmed directly from the same OPM source that `WQTotal` (well rate) has no
magnitude clamp at all, only a post-hoc sign-consistency check (injector can't produce, producer
can't inject) — so adding a rate clamp would have been inventing a limit OPM itself doesn't have.

**Note on the originally-proposed "decouple well iteration count from N3's growth formula" fix**:
re-examined before implementing and found it would be a no-op. N1's acceptance check
(`opm_conv.would_accept`) already excludes well/perforation rows entirely (matching OPM's
`getReservoirConvergence`), and under `OpmAligned` it is the *only* path to acceptance
(checkpoint 5 removed every other exit) — so `report.newton_iterations` fed to N3's growth
formula already reflects "how long reservoir-only convergence took," by construction. There was
no well-iteration count mixed into it to decouple. The trace instead showed the reservoir's own
MB residual genuinely stalling (`stagnation_count=17`) while the well/perforation family
dominated the residual mix — consistent with an unchopped, oscillating well update perturbing
the coupled linear solve and dragging out the reservoir's own convergence, not "extra iterations
tacked on for the well's sake." This is why the BHP chop (not an iteration-count change) was
implemented instead.

**Gates so far:** 3 new unit tests (`opm_per_cell_chop_clamps_well_bhp_relative_when_increasing`,
`_well_bhp_within_cap_is_untouched`, `_well_bhp_floors_above_lower_limit`); locked smoke 3/3;
full control matrix + heavy case bit-identical under default Legacy (no-op preserved). `22x22x1`
and `23x23x1` under `--opm-aligned` are unchanged (`12/1` each, both previously used to validate
checkpoints 5-6) — expected, since neither case has a well pinned at its BHP limit.

**Heavy-case rerun**: in progress (native `--release`, no-trace variant, background). Given the
previous full run took 176 minutes, this is the actual test of whether the fix addresses the
identified mechanism — result to be appended once available, not assumed from the mechanism
analysis alone.

### Bundle N §5 follow-up — trace-overhead isolation result (2026-07-09)

The no-trace native repro (old/unfixed code, run concurrently with the BHP-chop fix's own
confirmation run below — the two competed for CPU, inflating this run's wall-clock to
`304m59s` real / `178m23s` user) confirms: **`accepted_substeps=18002`, exactly matching** the
original `step_with_diagnostics` run. Tracing overhead was not inflating the substep count —
the `18,002`-substep pathology is a genuine solver/controller behavior, not a diagnostics
artifact. User CPU time (`178m23s`) is also close to the original traced run's wall-clock
(`176m25s`), confirming the `fim_trace!` macro's unconditional `format!()`/detail-computation
calls (not the trace-string storage) account for most of the tracing-adjacent cost, as
suspected — but that cost is small next to the substep count itself, which is the real problem.

### Bundle N §5 follow-up — well-BHP chop fix REFUTED (2026-07-09)

Result, verbatim (native `--release`, no-trace, `298m13s` wall-clock running solo):

```
accepted_substeps=18002 advanced_dt=1.000000/1.000000 linear_bad=7 nonlinear_bad=2 mixed=0
min_dt=Some(1.0868125188689959e-7) max_dt=Some(0.1850314752) last_dt=Some(1.0868125188689959e-7)
```

**Identical to both prior runs — `accepted_substeps=18002`, and `min_dt`/`max_dt`/`last_dt`
match to the exact same floating-point bits.** The well-BHP chop had zero effect and, given the
bit-identical `min_dt`/`max_dt`, most likely never engaged at all: the well's raw per-iteration
BHP delta apparently never exceeded the `dbhp-max-rel=1.0` (100% of current BHP) cap in this
scenario. **The hypothesis that an unchopped, oscillating BHP update was perturbing the coupled
reservoir residual is REFUTED.** BHP itself is not the oscillating variable.

**Consequence — do not guess again.** Two well-reasoned, OPM-verified fixes have now been tried
(iteration-count decoupling, ruled out by code inspection before implementation; BHP chop, ruled
out empirically at a cost of ~5 hours of compute across two runs). Continuing to guess at a
third fix without better visibility into what is *actually* oscillating (candidates not yet
ruled out: the perforation-rate variable itself, deliberately left unchopped to match OPM's own
lack of a `WQTotal` clamp; or ResSim's own `relax_well_state_toward_local_consistency`
post-processing step — a RESSIM-SPECIFIC mechanism with no direct OPM counterpart, run after
every Newton update, which is a plausible oscillation source the OPM-fidelity review has not
yet examined) would repeat exactly the trial-and-error pattern flagged as unproductive earlier
in this session. Recorded as an open item requiring cheaper diagnostic tooling (e.g. a modified
native test that writes the full trace to a file for the specific late-time window, since the
pathology's substep count implies it is concentrated very close to the end of the simulated
day — `max_dt=0.185` days means the "healthy" phase likely covers the bulk of the day in a
handful of substeps, with virtually all 18,002 substeps spent crawling through a tiny residual
sliver of time) before attempting a further live fix.

### Bundle N disposition (2026-07-10) — parked; retrospective written

Consolidated retrospective + recommended sequencing written to `docs/FIM_BUNDLE_N_DESIGN.md`
§10 (what was established with evidence, disposition, and the P → diagnostic → W plan);
`docs/FIM_STATUS.md` updated with a Bundle N section and reprioritized "Known Open Gaps"
(Bundle P first, then the late-window diagnostic, then the nested well solve "Bundle W");
`TODO.md` FIM next steps refreshed to match. Bundle N's code stays behind the `OpmAligned`
flag, default `Legacy`, fully no-op gated — inert, not deleted (its pieces are the building
blocks the eventual OPM-shaped solver still needs) and not promoted (§5 failed).

### Coarse-factorization cost lever (2026-07-10) — offline decisive, live promoted, `FIM-LINEAR-011`

Follow-up to `FIM-BUNDLE-P`'s P0 (REFUTED for reuse). P0.1's build-cost breakdown was re-run
with `coarse_factorization_ms` split into `dense_inverse_ms`/`coarse_ilu0_ms` (they were
conflated in one timer) to confirm precisely which piece dominates: on the heavy corpus,
`dense_inverse_ms=48.8` vs `coarse_ilu0_ms=0.12` — **400x**, confirming the dense inverse
(`invert_pressure_block`'s `try_inverse()`) alone is the cost, not the ILU0 setup that runs
alongside it.

**Offline 3-way comparison** (new `coarse_factorization_lab_compare` in `gmres_block_jacobi.rs`,
new `solver_lab_coarse_factorization_comparison` test; recaptured both corpora identically —
185 bounded + 414 heavy systems, exact counts matching the original P0 run):

| | dense inverse | LU factorization | BiCGStab+ILU0 |
|---|---:|---:|---:|
| bounded (529 coarse rows) | 90.7ms | 20.5ms (4.4x cheaper) | 0.54ms (**168x cheaper**) |
| heavy (432 coarse rows) | 45.3ms | 10.6ms (4.3x cheaper) | 0.45ms (**101x cheaper**) |

LU reproduces the inverse's solution to `~1e-10` (machine precision, exact). BiCGStab —
already the production coarse-solve path above the 512-row threshold — converges on **every
one of 599 captured systems with zero failures**, residual reduction ratio median `~4e-7`, max
`~1e-6` (far tighter than Newton's own tolerance needs). BiCGStab strictly dominates LU here:
cheaper by another ~20-25x, and already proven production code (no new solver path).

**Live promotion**: `PRESSURE_DIRECT_SOLVE_ROW_THRESHOLD` lowered `512→300` (coarse rows =
cell count post-well-elimination: heavy=432, `22x22x1`=484, `23x23x1`=529 already above 512,
`20x20x3`x2=1200 already above; `gas-rate 10x10x3`=300 stays on dense, exactly at the new
threshold, untested at that size but trivially cheap regardless). Gates:

- `cargo test --lib -- fim::linear::`: 36/36 pass (0 changes needed — thresholds referenced
  symbolically everywhere).
- Locked smoke 3/3; Buckley-Leverett benchmarks 3/3.
- Full control matrix: **bit-identical on all 5 non-heavy cases**, including `22x22x1` (newly
  flipped from dense to BiCGStab) — `substeps=4 | accepts=4+0+0 | retries=0/2/0`, unchanged.
- Heavy case (`--dt 1`): wall-clock `36.9s → 6.8s` (**5.4x**). Substep count/trajectory DID
  shift (`32→52` substeps, `retries=0/13/0→0/8/7` — a new "mixed" retry classification appears
  for the first time), consistent with this system's already-established chaotic sensitivity to
  linear-solve perturbations (Task #37, the `k`-sweep) — the coarse solve is now approximate
  (`~4e-7` residual) rather than exact, and this system's Newton trajectory is known to bifurcate
  on changes this small.
- **Fine-dt FOPT** (`--steps 16 --dt 0.0625`, the physics gate that actually matters):
  `oil=3847.59` vs OPM's converged `3826.12` — **+0.56% drift, better than the currently-accepted
  bundle's own +1.50%** (`3883.47`, `k=1.25`'s accepted result). The coarse-solve swap did not
  cost accuracy; if anything the trajectory it lands on tracks OPM slightly more closely.

**Verdict: PROMOTED as `FIM-LINEAR-011`.** Net: dramatic, validated per-solve cost win (100-170x
on the coarse stage, 5.4x heavy-case wall-clock) with no physics-accuracy cost — the opposite of
`FIM-DAMP-004`'s trade-off. The heavy-case substep-count change is not itself a regression signal
(this system's substep counts have never been directly comparable across bundle configurations —
`docs/FIM_STATUS.md`'s own historical trajectory note says as much); the fine-dt FOPT check is
the metric that was actually at risk, and it improved.

**Not investigated (candidate follow-ups, not required for this promotion):** whether `k=1.25`
(tuned against the OLD dense-inverse coarse solver) is still the best value under this new
config — the "mixed" retry class appearing for the first time is worth a first-principles look
if a future k-resweep happens, but per the user's own 2026-07-06 guidance not to chase small
accuracy deltas, this is deferred, not urgent.

### Late-window trace diagnostic on the 18k pathology (2026-07-11)

Per `docs/FIM_BUNDLE_N_DESIGN.md` §10's recommended sequencing (Bundle P → this diagnostic →
Bundle W) and `TODO.md` "FIM next steps" #2: two fixes for the heavy-case `OpmAligned`
18,002-substep pathology were already honestly refuted (iteration-count decoupling — no-op by
inspection; verbatim `dbhp-max-rel` BHP chop — bit-identical 18,002 rerun). Two refuted fixes is
the guessing budget; this builds the cheap diagnostic visibility instead of a third blind guess.

**Instrumentation** (`src/lib/ressim/src/fim/trace_sink.rs`, new module; wiring in
`fim/timestep.rs`/`fim/newton.rs`): a native-only, env-gated file trace sink
(`FIM_TRACE_FILE`), mirroring `fim/linear/capture.rs`'s pattern. `FIM_TRACE_FILE` alone gets a
per-substep `LEDGER` line (BHP, perforation rates, iters, growth, dt) for the whole run, cheap
enough to run unconditionally. `FIM_TRACE_DT_BELOW`/`FIM_TRACE_SUBSTEP_START` narrow full
per-iteration tracing (every existing `fim_trace!` line, plus a new `WELLTRACE` line with
per-iteration well/perforation state) to just the collapse window. `FIM_MAX_SUBSTEPS` overrides
the hardcoded 100,000 cap so a windowed rerun can abort shortly after capturing the window.
All four are no-ops when unset — verified: full control matrix (all 6 commands) bit-identical,
locked smoke 3/3, `assembly_ad` parity 10/10, wasm build green (module compiles for wasm like
`capture.rs` but every call site is `#[cfg(not(target_arch = "wasm32"))]`, so it's dead code
there — confirmed harmless, same as the existing `capture.rs` precedent).

**Re-baseline** (commit `9554e9f` + this no-op-verified instrumentation on top; working tree
otherwise had only an unrelated pre-existing `docs/CASE_LIBRARY_ROADMAP.md` edit — treated as
non-provisional since the instrumentation's no-op behavior was independently proven via the
bit-identical control-matrix check above, not merely assumed):

```
FIM_TRACE_FILE=<path> cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  fim::timestep::phase5_repro::repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture --exact
→ accepted_substeps=17990 advanced_dt=1.000000/1.000000 linear_bad=7 nonlinear_bad=1 mixed=1
  min_dt=Some(1.032244883492794e-6) max_dt=Some(0.1850314752) last_dt=Some(3.5426427140716754e-6)
  wall-clock: 1288.708s (~21.5 min)
```

**The pathology persists post-`FIM-LINEAR-011`**: `17,990` substeps (vs. the prior `18,002`,
both catastrophically over the `≤35` gate) — the small shift is the same chaos-sensitivity to
linear-solve precision already documented for this case (cf. Legacy's `32→52` shift under the
same lever), not a meaningful change. Wall-clock dropped from the cleanest prior solo run's
`298m13s` (`17893s`) to `1288.7s` — **~13.9x faster**, confirming `FIM-LINEAR-011`'s own framing
("makes every future heavy-case experiment ~an order of magnitude cheaper") and making this kind
of diagnostic run tractable within a session for the first time.

**Windowed rerun** (`FIM_TRACE_SUBSTEP_START=25 FIM_MAX_SUBSTEPS=530`, same driver): completed
in `40.14s` (`accepted_substeps=530`, capped as expected). The run is bit-for-bit deterministic
across invocations — early substeps (dt, iters, bhp, q) match the uncapped re-baseline run
exactly at the same indices — so this window is representative of the full pathology, not an
artifact of a different trajectory.

**Finding — the oscillating/stuck variable, with a mechanism, not just a name:**

BHP is independently reconfirmed, more strongly than the earlier chop refutation, as not the
culprit: `raw_dbhp=[0.0, 0.0]` for both wells, **exactly** (not merely small) on every single
Newton iteration across every substep inspected — the well-constraint row is trivially satisfied
every iteration because BHP is pinned at its control limit (`bhp=[500.0, 100.0]` bit-identical
across the entire window).

The actual culprit is the **producer well's perforation rate**, and the mechanism is a
**persistent disagreement between the raw Newton correction and
`relax_well_state_toward_local_consistency`** ([state.rs:307](../src/lib/ressim/src/fim/state.rs:307)),
not a classical back-and-forth oscillation. Inspected 5 independent `iters=20` substeps (27, 30,
32, 33, 35) via the new `WELLTRACE` line — same signature every time:

- The raw (pre-relax) Newton correction to the producer's perforation rate (`raw_dq[1]`)
  settles into a **non-vanishing plateau** within the first few iterations and stays there
  through iteration 18+ (e.g. substep 27: settles at `≈+0.581 m³/day`; substep 30: `≈+0.405`;
  substep 32: `≈+0.641`; substep 33: `≈+0.278`; substep 35: `≈+0.381` — different per substep,
  constant *within* a substep).
- `relax_well_state_toward_local_consistency`'s contribution (`relax_dq_approx`, computed as
  `candidate − (state + damping·raw_update)`) tracks the **near-exact negative** of that same
  plateau every iteration (e.g. substep 27: `≈−0.581`), so the two nearly cancel — net movement
  per iteration is tiny, which is why `q` and the reservoir-side CNV/MB both *look* converged
  (flat) from iteration ~2 onward.
- Despite that, the `perforation_flow` residual family (`res_pf`) never drops to zero — it
  plateaus at a small-but-nonzero floor that scales linearly with the same `raw_dq[1]` plateau
  (ratio `≈8.63e-5`, consistent across all 5 substeps to 3 significant figures — i.e. `res_pf`
  and `raw_dq[1]` are literally measuring the same underlying disagreement).
- The injector well is unaffected throughout (`raw_dq[0]`/`relax_dq_approx[0]` both stay at
  `~1e-7`/`~1e-12`, negligible) — this is specific to the BHP-limited producer.

Mechanistic read: `relax_well_state_toward_local_consistency` recomputes its own "consistent"
perforation rate from the current candidate reservoir state each iteration
(`connection_rate_for_bhp`) and blends toward it — a rate formula independent of, and evidently
not agreeing with, the AD-linearized Jacobian's implicit perforation-flow equation, by a
persistent offset. Every iteration: the raw Newton step corrects toward *its* zero, relax
immediately pulls back toward a *different* implied value by nearly the same magnitude, and the
next iteration's Newton correction (computed against the post-relax state) reproduces the same
disagreement — a standoff, not a convergence, and not a 2-period oscillation either (`res_pf`'s
own iteration-to-iteration delta stays small throughout, which is precisely why `FIM-NEWTON-006`'s
OSC-DETECT widening — tuned to the classical `d1≈0, d2≥0.2` signature — doesn't and structurally
can't see this: the large raw/relax components cancel before they ever show up as a residual
swing). This is consistent with, and sharpens, the original §5 finding ("a well/perforation
residual that does NOT shrink with dt forces `iters=20`") — the missing piece is *why* it doesn't
shrink: not the perforation-rate unknown itself (which converges fine on its own, per the
raw-Newton-only trajectory), but its forced disagreement with `relax_well_state_toward_local_consistency`.

**Consequence for Bundle W**: the nested well-equation solve (`docs/FIM_STATUS.md` gap #3)
should replace `relax_well_state_toward_local_consistency` outright with a converged per-well
inner solve (matching OPM's `iterateWellEquations`), rather than trying to patch the relax
step's blend factor or trust radius — the diagnosis is a structural disagreement between two
independently-derived rate formulas, not a tuning parameter. `WELL_RATE_MANIFOLD_BLEND`/
`WELL_RATE_TRUST_RADIUS_*` in `state.rs` are the relax step's current constants, kept for
reference but not a promising tuning target given this finding.

**Scope**: read-only diagnostic, no solver-behavior change. Instrumentation kept in the tree,
gated off by default (verified no-op above). Raw ledger/window trace files are scratch output,
not committed.

### Bundle W plan written (2026-07-11)

Follow-up to the diagnostic above: full checkpointed implementation plan for the nested
well-equation solve written to `docs/FIM_BUNDLE_W_PLAN.md`; registry row `FIM-BUNDLE-W` (OPEN)
added. Key plan decisions, all downstream of `FIM-DIAG-002`'s finding: (1) the inner solve must
drive the same assembled well residual rows as the global assembly (W1's bit-match agreement
test encodes this), (2) the previously-refuted `dbhp-max-rel` chop is re-homed inside the inner
loop where OPM actually applies it, (3) the cheap WELLTRACE mechanism gate runs before the full
§5 re-run so a wrong design fails in minutes not hours, (4) Legacy adoption is explicitly
deferred to its own experiment (plan §7). Also corrected the "Hypothesis A" citation history in
the plan's evidence section: Phase 8's original FB-crossover hypothesis found no supporting
evidence — what survived was the well-source-dominance pattern plus Bundle N §5's controller
pathology, now sharpened by the diagnostic's standoff mechanism.

### Bundle W checkpoint W0: OPM source verification (2026-07-11, commit `2f0f284`)

Full findings live as an appendix in `docs/FIM_BUNDLE_W_PLAN.md` ("Appendix: W0 OPM source
verification"); this entry is the worklog-discipline summary with the numbers that matter for
later checkpoints. Verified against the pinned local checkout `OPM/opm-simulators`
(`062cb19986aa8f11cffc30351fd2fee355d0ccb4`, tag `interim_release/2024.12-4152-g062cb1998`).

**Correction to prior citations**: the reservoir Newton model class was renamed upstream from
`BlackoilModel`/`BlackoilModel_impl.hpp` (what earlier Bundle N docs cite) to
`NonlinearSystemBlackOilReservoir`/`NonlinearSystemBlackOilReservoir_impl.hpp`, between whatever
checkout Bundle N's session used and this one. All citations below are against the file names
actually present in this checkout — don't trust old file-name citations without re-verifying.

**Loop structure confirmed**: `WellInterface::iterateWellEquations`
(`WellInterface_impl.hpp:532`) is called from `prepareWellBeforeAssembling`
(`WellInterface_impl.hpp:1018`, call site `:1066`), itself invoked once per outer Newton
iteration from `BlackoilWellModel::assemble()`, **before** `assembleWellEqWithoutIteration(dt)`
(`BlackoilWellModel_impl.hpp:1186`) — wells converge first, then get linearized into the global
system without further iteration that same outer step. Gated (not literally unconditional) by
`shouldRunInnerWellIterations` (`NewtonIterationContext.hpp:95`): true while
`globalIteration_ < max_niter_inner_well_iter_` (`MaxNewtonIterationsWithInnerWellIterations` =
**99**, effectively always-on for realistic outer iteration counts).

**Inner loop body** (`iterateWellEqWithSwitching`, `StandardWell_impl.hpp:2458` — the default
path since `LocalWellSolveControlSwitching` defaults `true`): `do {...} while (it < max_iter)`,
`max_iter = MaxInnerIterWells` = **50**. Control-mode switching (rate↔BHP↔THP, open↔stop) is
checked via periodic discrete re-evaluation every 4 iterations
(`min_its_after_switch`, `StandardWell_impl.hpp:2482`) — structurally different from ResSim's
continuous Fischer-Burmeister complementarity row. Documented as a deliberate divergence, not
something Bundle W ports; ResSim's FB row stays as the existing assembled equation.

**Convergence test** (`StandardWellEval::getWellConvergence`, `StandardWellEval.cpp:156`): two
*separately*-toleranced checks, correcting the "tolerance-wells=1e-4" shorthand used loosely
elsewhere in these docs — that number is only exactly right for the flux/mass-balance rows and
for BHP-controlled wells specifically:
- flux rows: `ToleranceWells` = **1e-4** (relaxed to `RelaxedWellFlowTol` = **1e-3** after
  `StrictInnerIterWells` = **40** iterations); hard fail above `MaxResidualAllowed` = **1e7**.
- control-equation row (`checkConvergenceControlEq`, `WellConvergence.cpp:39`): tolerance
  depends on the well's *current active control mode* — `{rates: 1e3, grup: 1e4, bhp: 1e-4,
  thp: 1e-6}` (`StandardWellEval.cpp:211`), four orders of magnitude looser for rate control
  than for BHP control.
- a hard `WrongFlowDirection` sign-consistency check for pressure-controlled wells (not a
  tolerance) — producer flux must not be negative, injector flux must not be positive.

**`updateNewton` chop** (`StandardWellPrimaryVariables.cpp:262`, called every inner iteration):
BHP capped at `DbhpMaxRel` = **1.0** (100% relative), floored at `1 bar − 1 Pa`. **This is the
exact value the refuted Bundle N §5 BHP-chop follow-up ported verbatim** — confirms that fix was
tested in the wrong place (the outer Newton loop) rather than built with the wrong formula; its
correct home is the inner well loop this bundle builds. `WQTotal` (total well rate) has **no
magnitude clamp at all**, only a post-hoc sign floor — reconfirms the 2026-07-09 finding with a
fresh citation.

**Parametrization note** (informative, doesn't change the design): OPM's `StandardWell` uses a
*lumped* `[WQTotal, WFrac, GFrac, Bhp]` unknown set, not one rate unknown per perforation the
way ResSim's `FimState::perforation_rates_m3_day` does. Bundle W targets ResSim's own assembled
rows (plan §2), not OPM's exact variable choice — only the *structural* pattern (nested bounded
Newton, converged before global assembly, invisible to the outer iteration count) transfers.

**Failure policy**: non-convergent wells get `stopWell()` + `solveWellWithZeroRate(...)`
(`ShutUnsolvableWells` = **true** default) — OPM does not silently accept an under-converged
well state, it forces a well-defined degraded state. Flagged as an open design question for W2
(resolved there: keep the last iterate, report not-converged, let the outer retry ladder decide
— did not implement OPM's stop-the-well escalation, which is a larger behavior change than this
bundle's scope).

**One real correction to the standing Bundle N narrative**: the *iteration count* fed to the
timestep controller is confirmed outer-only (`getNumIterations_`,
`AdaptiveTimeStepping_impl.hpp:1186`, reads `total_newton_iterations` which increments once per
call to `nonlinearIterationNewton`, `NonlinearSystemBlackOilReservoir_impl.hpp:237` — wells'
internal iterations never separately increment it). But the outer **convergence check** is NOT
well-blind: `NonlinearSystemBlackOilReservoir::getConvergence`
(`NonlinearSystemBlackOilReservoir_impl.hpp:1008`) computes `report = getReservoirConvergence(...)`
then `report += wellModel().getWellConvergence(...)` — the aggregate outer report does include a
well term. In practice this rarely blocks anything extra (wells already converged via the inner
loop by the time this runs), but "N1's acceptance excludes well/perforation rows entirely"
(`docs/FIM_BUNDLE_N_DESIGN.md` §5.1) describes ResSim's `OpmAligned` simplification, not
literally OPM's structure — this is exactly the gap plan §5 step 3 proposes closing, now backed
by a precise citation instead of an inference.

### Bundle W checkpoint W1: local well system + agreement tests (2026-07-11, commit `7509289`)

New `fim/wells_inner.rs`: `assemble_well_local_system(sim, state, topology, well_idx) ->
FimWellLocalSystem` builds one physical well's local residual (`well_constraint` row, then one
`rate_consistency` row per perforation) and Jacobian w.r.t. `[bhp, q_perf...]`, with the
reservoir cell state held frozen (read as input, not solved-for). Built by calling the exact
same shared AD primitives `assembly_ad.rs`'s `add_well_residual_terms`/`add_well_jacobian_terms`
call for these rows (`well_constraint_residual_fb_generic`,
`well_constraint_bhp_column_and_fb_gradient`, `well_constraint_own_perforation_rate_jacobian`,
`connection_rate_generic`, `rate_consistency_cell_bhp_jacobian`, `producer_fractions_generic`)
— not a reimplementation. Two small `assembly_ad.rs` helpers (`well_cell_input`,
`well_control_generic`) promoted from private to `pub(crate)` for reuse; verified zero behavior
change via `assembly_ad` parity (10/10 unaffected).

**Agreement tests** (4, all passing) directly encode the design constraint from plan §2: for a
constructed state, the local system's rows/columns must exactly match the corresponding
rows/columns of a full `assemble_fim_system_ad` call.
- `local_system_matches_global_assembly_bhp_controlled` / `..._rate_controlled`: both control
  modes, two-well fixtures mirroring `assembly_ad.rs`'s own `two_phase_bhp_controlled_wells`/
  `two_phase_rate_controlled_wells` structural-parity fixtures.
- `local_system_matches_global_assembly_away_from_convergence`: perforation rates perturbed
  `+500` before comparing — deliberately not a near-zero-residual state, where a formula bug
  could hide behind both sides evaluating to ~0.
- A no-cross-coupling check: one perforation's `rate_consistency` row never touches another
  perforation's `q` column, matching the global assembler's `tri.add_triplet(perf_row, q_col,
  1.0)` (each row only ever writes its own `q_col`).

Residuals compare bit-identical (`assert_eq!`). Jacobian entries needed a `1e-12` tolerance, not
exact equality — one test failure at `local=-9.414691248821328e-16 global=0.0` before the fix
traced to the global sparse assembler's `add_if_nonzero` dropping `|value| <= 1e-14` as an
implicit zero, while the dense local Jacobian stores the raw computed value. Confirmed as a
sparse-storage convention difference, not a formula divergence, by inspecting `assembly_ad.rs`'s
`add_if_nonzero` directly before accepting the tolerance fix (not just widening the assertion
until it passed).

**Closed-form observation** (not exploited yet at W1, confirmed empirically at W2 below):
`connection_rate_generic` takes `(bhp, cell)` and does not depend on `q` at all — the
`rate_consistency` row's dependence on `q` is the trivial identity (`tri.add_triplet(perf_row,
q_col, 1.0)`). For a BHP-controlled well (`well_constraint = bhp − bhp_target`, no `q`
dependence either — exactly why `FIM-DIAG-002` measured `raw_dbhp` at exactly `0.0` every
iteration) with frozen reservoir cells, `q = connection_rate_generic(bhp_target, cell)` is a
one-shot closed-form evaluation, not an iterative fixed point. This reframes the `FIM-DIAG-002`
standoff: very likely an artifact of the *coupled global iterative linear solve*'s imprecision
on that specific row/unknown pair (fgmres-cpr/BiCGStab solving the FULL system approximately),
not genuine nonlinear difficulty in the isolated well subsystem.

Gates: `assembly_ad` parity 10/10, `fim::wells` 18/18, locked smoke 3/3, wasm build green (new
code unreachable in production until W3 wires it in — same "kept in the tree, no-op verified"
pattern as `trace_sink.rs`).

### Bundle W checkpoint W2: inner Newton loop (2026-07-11, commit `5765f28`)

`solve_well_locally`/`solve_wells_locally` (`fim/wells_inner.rs`): a bounded, chopped Newton
loop over W1's `assemble_well_local_system`, mutating `state.well_bhp[well_idx]`/
`state.perforation_rates_m3_day[perf_idx]` in place — same call shape as
`relax_well_state_toward_local_consistency`, so it is a drop-in replacement at that single call
site (`state.rs:424`) once W3 flips the flag. Defaults per W0's numbers:
`max_iterations = 50` (`MaxInnerIterWells`), `tolerance = 1e-4` (`ToleranceWells`),
`dbhp_max_rel = 1.0` (`DbhpMaxRel`).

**`chop_bhp_update`**: OPM's exact chop formula (`raw_delta_bhp.clamp(-cap, cap)` where
`cap = |bhp| * dbhp_max_rel`, then floored at `BHP_LOWER_LIMIT_BAR = 1.0 − 1e-5` bar) — the
formula the refuted Bundle N follow-up ported to the wrong (outer) loop; this is its correct
home. No magnitude clamp on `q`, matching OPM's `WQTotal` update exactly.

**Convergence check reuses the exact scaling formula the global assembly's own convergence test
uses** — a small refactor of `fim/scaling.rs` extracted `well_constraint_scale(bhp_bar,
control_slacks)` and `perforation_flow_scale(rate_m3_day)` out of `build_equation_scaling`'s
inline logic into standalone `pub(crate)` functions, with `build_equation_scaling` itself
updated to call them (zero-behavior-change refactor: verified via `fim::scaling` 3/3 and
`assembly_ad` parity 10/10 unaffected). `wells_inner.rs` calls these same two functions for its
own scaled-residual-peak convergence check, so "inner converged" and "outer sees zero" cannot
silently drift into two hand-matched copies of the same formula.

**`perforation_flow_direction_ok`**: OPM's `WrongFlowDirection` check (W0), scoped to
pressure-controlled (non-rate-controlled) wells, applied per-perforation since ResSim has no
single aggregate `WQTotal` the way OPM's lumped parametrization does. A state whose residual is
within tolerance but has the wrong sign correctly reports `converged: false`, not silently
accepted.

**Failure handling**: a singular local Jacobian (`.lu().solve()` returns `None`) or exhausted
iteration budget both keep the last iterate and report `converged: false` — no acceptance
widening, per the `FIM-NEWTON-005` lesson (don't paper over an inner failure by loosening what
counts as "close enough").

**10 tests, all passing — the closed-form observation from W1 confirmed empirically, not just
in theory**: `bhp_controlled_well_converges_from_perturbed_state` starts from perforation rates
perturbed `+800` m³/day away from consistency and converges in **exactly 1 iteration**, final
scaled residual peak `2.3e-16`/`8.5e-16` (verified via a temporary debug print, removed before
commit) — machine epsilon, exactly matching the "one Newton step lands on the closed-form
solution" prediction. `rate_controlled_well_converges_to_slack_feasible_state` (genuinely
nonlinear FB case, since the constraint row *does* depend on `q` there) converges and lands on a
feasible `(bhp, q)` per the well's own slack tolerances. `exhausted_budget_reports_not_converged_
without_panicking` uses a deliberate `max_iterations: 0` to exercise the give-up path
deterministically — standing in for the plan's "deliberately infeasible case" wording, since the
FB reformulation is specifically designed to avoid genuine physical infeasibility rather than
produce a clean pathological test fixture.

**Regression gate**: full `fim::` test suite, 277 passed / 3 failed / 20 ignored (197.53s). The
3 failures were confirmed byte-identical *by exact test name* to the pre-existing 2026-07-07
known failures recorded in `TODO.md` (`fim::timestep::tests::
changing_hotspot_resets_extra_growth_cooldown_budget`, `repeated_same_hotspot_extends_growth_
cooldown_budget`, `fim_enabled_step_advances_time_and_records_history_for_closed_system`) — not
a new regression, verified by rerunning the full suite with output saved to a file and grepping
the `failures:` block, not assumed from the count alone. `assembly_ad` parity 10/10, wasm build
green.

### Bundle W checkpoint W3: flag wiring + no-op gates (2026-07-11)

**`state.rs`**: `apply_raw_update`'s last parameter changed from `relax_well_state: bool` to a
new `WellStateUpdateMode` enum (`None`/`Relax`/`NestedSolve`), matched in a 3-way `match` at the
single call site where well post-processing happens (right after the raw Newton update and
`enforce_control_bounds`). `NestedSolve` calls `wells_inner::solve_wells_locally` then
`enforce_control_bounds` again, mirroring `Relax`'s existing shape exactly. The only other
caller (`apply_newton_update`, `#[cfg(test)]`) passes `None`, and one existing test
(`apply_newton_update_frozen_limits_well_overshoot_toward_local_consistency`, which specifically
tests the Legacy relax trust-radius behavior) was updated to pass `Relax` explicitly so it keeps
testing what it always tested.

**`newton.rs`**: `FimNewtonOptions` gained `nested_well_solve: bool` (default `false` in the
`Default` impl — the two existing literal-construction sites both already use
`..FimNewtonOptions::default()`, so neither needed updating). The single
`apply_newton_update_frozen` call site picks `NestedSolve` vs `Relax` based on the flag,
independent of `nonlinear_flavor` (plan §5: "an independent flag... evaluable under both
flavors" — confirmed live by the flag-on sanity check below, which ran `--nested-well-solve`
under *Legacy*, not just `OpmAligned`).

Plan §5 item 3 (the outer-criteria addition): the `converged_on_entry` computation's
`opm_aligned` branch gained a `wells_ok` term —
```rust
let wells_ok = !opm_aligned
    || !options.nested_well_solve
    || wells_inner::all_wells_converged(sim, &state, &topology, &FimWellInnerSolveOptions::default());
let converged_on_entry = if opm_aligned {
    iteration >= OPM_NEWTON_MIN_ITERATION_INDEX && opm_conv.would_accept && wells_ok
} else { ... };
```
`wells_ok` is trivially `true` whenever either flag is off, so this is provably a no-op unless
*both* `opm_aligned` and `nested_well_solve` are set — verified by the control-matrix bit-
identity gate below, not just by inspection. This was the only acceptance decision site in the
whole file that reads `opm_conv.would_accept` under `opm_aligned` (confirmed by grep before
writing the change — a single site, not the many-return-points shape some other Bundle N
mechanisms have had to thread through).

**`wells_inner.rs`**: refactored the per-iteration scaled-residual-peak + flow-direction check
out of `solve_well_locally`'s loop body into a standalone `well_convergence_status` helper
(takes an already-assembled `FimWellLocalSystem`, returns `{converged, scaled_residual_peak}`).
Two new public functions built on it: `well_is_converged` (one well, read-only — assembles
once, checks, returns) and `all_wells_converged` (all wells, `.all(...)`) — together these are
the `getWellConvergence` analog from W0 appendix G, a pure *check*, not a solve. Two new tests:
`well_is_converged_matches_solve_result_before_and_after` (asserts the read-only check agrees
with `solve_well_locally`'s own verdict, both before solving on a perturbed state and after) and
`all_wells_converged_requires_every_well` (converges only one of two wells, confirms the
aggregate check still fails). `fim::wells_inner` now 12/12.

**Diagnostic/API surface** (mirrors the existing `fim_opm_aligned_nonlinear`/
`setFimOpmAlignedNonlinear`/`--opm-aligned` triple exactly): `ReservoirSimulator.fim_nested_well_solve: bool`
field (`lib.rs`), initialized `false` in the constructor (`frontend.rs`), `setFimNestedWellSolve`
wasm setter, threaded into `newton_options.nested_well_solve` in `timestep.rs`'s
`step_internal_fim_impl`, `--nested-well-solve` CLI flag in `scripts/fim-wasm-diagnostic.mjs`
(help text + parsing + `sim.setFimNestedWellSolve(true)` call, alongside the existing
`--opm-aligned` wiring). The native `repro_water_pressure_12x12x3_opm_aligned_no_trace` driver
(the exact `FIM-DIAG-002` re-baseline vehicle) gained a `FIM_NESTED_WELL_SOLVE` env-var toggle
so it doubles as the W4 §5 re-run vehicle without a new test function.

**No-op gate** (flag off): full `fim::` suite `279 passed / 3 failed / 20 ignored` (192.93s) —
the 3 failures are the identical pre-existing 2026-07-07 names (confirmed by grepping the
`failures:` block, not inferred from count), `+2` from this checkpoint's own new tests vs W2's
277. Full 6-command control matrix, rebuilt wasm, bit-identical to documented baselines
including the heavy Legacy case (`substeps=52 accepts=51+3+2060 retries=0/8/7
retry_dom=nonlinear-bad:water@1215` — exact match). Wasm build green (`bash scripts/build-wasm.sh`,
only the pre-existing harmless `dim()`-never-used warning).

**Flag-on sanity check** (informational, not a W4 gate — just confirms the wiring is live code,
not dead): `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 22x22x1 --steps 1
--dt 0.25 --diagnostic summary --no-json --nested-well-solve` (Legacy flavor) lands on the same
coarse substep/retry counts as the flag-off baseline (`4 substeps, retries=0/2/0`) but with
visibly different per-substep Newton iteration counts (`n10,n9,n8,n6` vs the baseline's
`n13,n9,n7,n7`) — a genuinely different trajectory that happens to match on the coarse metric,
not a silent no-op. Adding `--opm-aligned` on top of `--nested-well-solve` on the same case
changes the outcome much more visibly: `24` substeps (vs `12` for `--opm-aligned` alone,
previously recorded), a new dominant retry class (`linear-bad:oil@1450`, previously
`nonlinear-bad`), and a much lower minimum dt (`2.813e-5` vs whatever `--opm-aligned` alone
reached). This confirms the new `wells_ok` outer-criteria gate is genuinely firing and changing
acceptance decisions under the combined flags — a real, substantial behavior change worth
investigating, deliberately **not evaluated here**: the plan's own W4 ordering runs the cheap
`FIM-DIAG-002` mechanism gate on the *heavy* case first, bounded cases only after that passes.
This `22x22x1` regression (relative to `--opm-aligned` alone) is exactly the kind of finding W4
step 3 ("bounded cases... watch whether the gap narrows") is designed to characterize honestly,
not something to explain away here.

### Bundle W checkpoint W4: evaluation — mechanism fixed, heavy-case gate failed for a different reason (2026-07-11)

**Step 1, mechanism check (PASSED).** Native `--release` capped run,
`FIM_NESTED_WELL_SOLVE=1 FIM_TRACE_FILE=<ledger> FIM_MAX_SUBSTEPS=1000` on
`repro_water_pressure_12x12x3_opm_aligned_no_trace`: `accepted_substeps=1000
advanced_dt=0.916822/1.000000 ... min_dt=6.309373552505448e-6 ... elapsed 67.617s`. Immediately
notable: the pathology's `min_dt` floor (`6.3e-6`) is roughly 6x less extreme than the
post-`FIM-LINEAR-011` baseline's `1.03e-6` and ~60x less extreme than the original
pre-`FIM-LINEAR-011` `1.09e-7` — a hint, confirmed below, that *something* did genuinely
improve even though the substep count did not.

Followed with a windowed rerun (`FIM_TRACE_SUBSTEP_START=980`, same cap) to get full
per-iteration `WELLTRACE` on a stuck (`iters=20`) substep (substep 997, deterministically
reproduced — the ledger's last 5 lines matched the uncapped-cap run bit-for-bit, confirming no
run-to-run nondeterminism). Inspected all 20 iterations in full:

- `res_wc=0.000000e0` every single iteration (well-constraint row trivially satisfied, as
  expected for a BHP-pinned well — matches `FIM-DIAG-002`'s finding that `raw_dbhp` is exactly
  `0.0`, now doubly confirmed).
- `res_pf` (perforation-flow row): `0.0` at iter 0, then `3.56e-12, 4.76e-16, 3.57e-16, 7.14e-16,
  ...` for the rest — **machine epsilon from iteration 1 onward**. Previously (`FIM-DIAG-002`,
  pre-Bundle-W) this same field floored at a non-vanishing `~5e-5`–`5.5e-5` and stayed there for
  all 20 iterations. **This is the standoff, gone.**
- `q_post` for the producer: `3821.179165213835` (iter 0) → `3821.1791704584202` (iter 1) →
  `3821.1791703504177` (iter 2) → ... → `3821.179169910965` (iter 19) — stable to **8 decimal
  places** from iteration 1 onward. The well variable itself converges almost immediately and
  stays put.
- `raw_dq[1]` (producer) is still a persistent, non-vanishing ~0.58 m³/day every iteration, and
  `relax_dq_approx[1]` (the WELLTRACE field's name predates Bundle W and still reads "relax" —
  it is arithmetically the same `post − (pre + raw)` decomposition, now attributing the *nested
  solve's* multi-step internal convergence work rather than the old relax blend) still tracks
  its near-exact negative. **This surface pattern looks unchanged from `FIM-DIAG-002`'s trace at
  first glance — the numbers that actually matter (`res_pf`, `q_post`'s stability) show the
  underlying meaning has completely changed**: before, this cancellation was two mechanisms
  fighting to a non-converging standoff; now it's the nested solve's own internal Newton
  iterations converging smoothly to a fixed point that the outer linear solve's raw proposal
  simply doesn't move away from. A reminder to read the *converged quantity*, not just the
  presence of a cancellation pattern, when judging whether a standoff is real.
- `cnv=[6.100e-5, 6.146e-5, 0.000e0]` (water, oil, gas): **frozen** — unchanged past the 4th
  significant digit across all 19 iterations shown (`6.100e-5`/`6.146e-5` at iter 1 through
  `6.100e-5`/`6.146e-5` at iter 18). `mb=[1.412e-7, 1.423e-7, 0.000e0]` similarly frozen. This is
  the reservoir-side CNV/MB entry criterion — completely unaffected by the well fix, and it is
  what keeps `would_accept=no` for 19 straight iterations before the final-iteration relaxed
  tier (`would_accept=pv-relaxed` at iteration 19) finally accepts.

**Verdict for step 1**: the mechanism check's literal pass condition ("`res_pf` drops below
tolerance within the inner-converged iterations instead of flooring") is unambiguously met.

**Step 2, full uncapped §5 re-run (FAILED).** Same command without the cap, background,
native `--release`:
```
FIM_NESTED_WELL_SOLVE=1 cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  fim::timestep::phase5_repro::repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture --exact
→ accepted_substeps=18015 advanced_dt=1.000000/1.000000 linear_bad=8 nonlinear_bad=1 mixed=3
  solver_ms=1220457.88 min_dt=1.0337753842559846e-6 max_dt=0.1850314752 last_dt=1.4780226131883012e-6
  elapsed 1235.482s (~20.6 min)
```
**`18,015` vs the `17,990`-substep `OpmAligned`-only baseline (commit `a362e29`) — a `25`-substep
difference, well within this case's already-documented chaos-sensitivity to small perturbations
(cf. `FIM-LINEAR-011`'s Legacy `32→52` shift), i.e. essentially unchanged.** Wall-clock `1235.5s`
vs `1288.7s` — also essentially the same. **Decisively fails the `≤35` gate**, exactly as
severely as before Bundle W.

Root cause, from the same ledger: only **12 retry events** across all 18,015 substeps (`8
linear-bad, 1 nonlinear-bad, 3 mixed`), all with `dominant=oil@430` — none well-related, and far
too few to explain 18k substeps on their own. The substep explosion is entirely the *accepted*-
substep dt cycle: `growth=0.400 limiter=opm-iter` (an `iters=20` substep, shrinking) alternating
with `growth=3.000 limiter=opm-max-growth` (an `iters=2` substep, recovering) — the exact same
alternation pattern `FIM-DIAG-002` originally found, but now driven by the reservoir CNV
plateau confirmed above, not the well standoff. The run's final accepted state at `t=1.000000`
(`q≈[-3628.19, 3627.10]`) matches the *original, pre-Bundle-W* baseline's own final `q` values
closely — the two runs converge to essentially the same physical endpoint via the same
pathological path, just for a different underlying reason at the per-iteration level.

This pattern — a small residual (`cnv≈6e-5`) that won't shrink further and only clears via a
relaxed final-iteration tier — is consistent with (though not proven identical to; different
cell, different controller mechanism) the phenomenon `docs/FIM_STATUS.md` already documents as
"understood and benign" for Legacy's own `water@1215` plateau: "a genuine local steady-state
region colliding with intentionally-strict entry/zero-move acceptance gates." Bundle W's scope
never included this — plan §5's "Explicitly NOT in Bundle W" already excludes any acceptance-
criteria change.

**Step 3, bounded cases (mixed).**
```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 \
  --diagnostic summary --no-json --opm-aligned --nested-well-solve
→ substeps=12 retries=1/0/0 retry_dom=linear-bad:oil@1585
```
**Identical** to `--opm-aligned` alone (same substep count, same retry count, same dominant
retry cell — differs only in wall-clock `outer_ms`, expected run-to-run timing noise). The
nested solve is a genuine no-op on this case: `23x23x1`'s own bottleneck (`linear-bad:oil@1585`)
was never well-related, so fixing wells here changes nothing, for or against.

`22x22x1` (first measured in W3's flag-on sanity check, re-cited here since it's directly
relevant to this evaluation): `24` substeps vs `12` for `--opm-aligned` alone — a real
regression. **Not root-caused with the same rigor as the heavy case** — plausible by analogy
(fixing wells could equally expose a reservoir-side plateau on this smaller grid) but not
confirmed by a dedicated windowed trace. Recorded honestly as an open, unresolved data point,
not assumed to match the heavy case's story just because the shape rhymes.

**Step 4, fine-dt FOPT: deliberately not run.** With the primary gate failing decisively and one
bounded case regressing, running the more expensive physics-accuracy check would not change the
disposition — mirrors Bundle N §5's own precedent (moved straight to root-cause analysis once
its gate failed decisively, rather than completing every remaining checklist item first). Revisit
only if a future fix for the reservoir-CNV plateau reopens the heavy-case gate.

**Step 5, control matrix**: already done and gated at W3 (flag-off bit-identity); nothing in W4
touched the flag-off path, so not repeated.

### Bundle W checkpoint W5: verdict — NOT PROMOTED, mechanism kept (2026-07-11)

Applying the original Bundle N `docs/FIM_BUNDLE_N_DESIGN.md` §5 promotion rule (end metrics
only, heavy-case substep/cut behavior in the `≤35` class or nothing) to the heavy case with W
in: **FAILS** (`18,015` substeps). This is the same disposition shape as Bundle N itself: the
targeted mechanism is real, independently validated three separate ways (W1's bit-exact
agreement tests against the global assembly, W2's empirical 1-iteration/machine-epsilon
convergence on synthetic perturbed states, and W4's windowed trace confirming the exact
diagnosed standoff is gone on the real heavy-case trajectory) — but insufficient alone, because
fixing it exposed a second, independent, previously-masked architecture gap (the reservoir CNV
plateau) that Bundle W was never scoped to address.

**Disposition**: `nested_well_solve` stays in the tree, default `false`, fully no-op verified
(W3's control-matrix bit-identity gate). Not deleted — it is validated, correct, and the
`getWellConvergence`-equivalent well-convergence checking it introduces is exactly the building
block a future combined fix would still need. `FIM-BUNDLE-N`'s own registry status
(REWORK REQUIRED) is unaffected — Bundle N is evaluated independently of this flag, which
defaults off, so Bundle N's own heavy-case number is unchanged by this work.

**New open item** (not Bundle W's to fix): the reservoir-side CNV plateau at near-steady-state
under `OpmAligned`'s entry criterion, now clearly exposed as the heavy case's dominant remaining
blocker. Recommend a fresh, dedicated diagnostic pass — reusing `FIM-DIAG-002`'s own
`WELLTRACE`/`LEDGER` tooling, which already incidentally captured this signature while
investigating wells — targeting `cnv`/`mb` per-iteration evolution and the exact conditions
under which the final-iteration relaxed tier is needed, before proposing any fix. The same
discipline that produced `FIM-DIAG-002` (evidence before a third guess) applies here by direct
extension: this is effectively a *first* look at a newly-exposed mechanism, not a well-trodden
one, so there is no guessing budget to spend carelessly.

### Week retrospective (2026-07-11): the heavy-case failure is a conjunction, and the chain is nearly exhausted

Prompted by a user review question: three consecutive NOT-PROMOTED verdicts (Bundle N, Bundle P,
Bundle W) under end-metric-only gating — if the heavy case fails on a *combination* of errors,
element-by-element evaluation can never promote anything and never "solves" the case. Analysis
of the week's own recorded traces, plus two fresh source checks, confirms the combination
structure directly and sharpens the remaining problem to a single measurable quantity.

**1. The conjunction is real and is directly visible in the traces already recorded.**
The `FIM-DIAG-002` window trace (substep 27, pre-Bundle-W) shows TWO criteria frozen above
tolerance simultaneously: the perforation-flow residual floored at `~5e-5` (the diagnosed
standoff) AND `mb=[1.858e-7, 4.668e-7]` frozen from iteration 2 onward — both > `1e-7`, both
immobile. Bundle W's W4 trace (substep 997) shows exactly ONE remaining: `res_pf` now at machine
epsilon, `mb=[1.412e-7, 1.423e-7]` still frozen. The end gate (substep count) is a step function
over a conjunction: it cannot move until the LAST frozen criterion clears, which is why fixing
the well standoff produced `17,990 → 18,015` (no change) despite being a genuine fix. The user's
critique is confirmed by the data — with the caveat that the *construction* side of the week was
already combination-aware (N1-N5 were deliberately built as one bundle; W was evaluated stacked
on N) — the gap is on the *measurement* side: end-metric gating is conjunction-blind, and the
NOT-PROMOTED bookkeeping makes cumulative progress read as serial failure.

**2. The hidden progression the substep count can't show.** Re-reading the week's numbers as a
sequence of binding-constraint margins instead of substep counts:
- pre-Bundle-N: MB stalls at `≈2e-6` (20x over the `1e-7` tolerance) — fixed by N2 per-cell chop
  (95% vs 48% OPM-acceptable attempts, checkpoint 2);
- post-N: perforation-flow residual floored at `≈5e-5` scaled (the `FIM-DIAG-002` standoff) —
  fixed by Bundle W (machine epsilon, W4);
- post-W: MB frozen at `1.41e-7` — **1.4x over tolerance**, the sole survivor.
The binding margin has tightened >100x across the week. The chain is nearly exhausted, which
argues for finishing the serial-peeling approach rather than abandoning it — but with the
measurement and bookkeeping changes below.

**3. The remaining bottleneck, sharpened from already-recorded data + two fresh checks.**
Fresh check 1 (OPM pinned source): `ToleranceMb=1e-7`, `ToleranceMbRelaxed=1e-6`,
`ToleranceCnv=1e-2`, relaxed MB applies at the final iteration only by default
(`MinStrictMbIter=-1`, `NonlinearSystemBlackOilReservoir_impl.hpp:751`) — our port's constants
and tier logic match exactly (`newton.rs:1794-1797`). Fresh check 2: on the stuck substeps,
CNV passes by **160x** (`6.1e-5` vs `1e-2`); **MB alone binds, at 1.41x over strict**, for 18
straight iterations until the final-iteration relaxed tier (`1e-6`) accepts — then N3 sees
`iters=20` and collapses dt. The entire 18k-substep catastrophe now rests on one number: why
does our MB freeze at `1.41e-7` when OPM solves this same case at ~2.5 iters/step (i.e., its MB
genuinely drops below `1e-7`, since its acceptance tiers are identical to ours)?

Frozen at 4 significant figures across 18 iterations means the state is an **invariant point of
the modified iteration map** (Newton step + per-cell chop + nested well solve + bounds), not
slow convergence. The trace shows the mechanism: each iteration the coupled linear solve
proposes `dq≈+0.58` for the producer (its way of zeroing the well-cell mass-balance rows via the
source term) and the nested well solve vetoes it back to the perforation-consistent value.
Three ranked hypotheses, each with a cheap decisive test:
- **H1 (displaced standoff / constrained fixed point)**: enforcing the perforation equation
  exactly displaces the same underlying inconsistency into the well-cell mass-balance rows —
  the `1.41e-7` MB *is* the old standoff wearing a different family label. Test: locate the
  peak-MB cell during the freeze (extend the CNV-MB trace line to name the binding cell/family
  — the FAIL-SITE-DETAIL machinery already exists); if it's the producer's perforation cell
  (cell 143) or its column, H1 confirmed. ~70s capped run.
- **H2 (linear-precision floor)**: the loose `5e-3` outer linear tolerance (`FIM-LINEAR-008`)
  caps achievable MB reduction near steady state. Test: force the direct linear backend on a
  capped heavy run — if the freeze breaks, H2. Also testable offline against captured frozen
  substeps (`FIM_CAPTURE_SEQUENCE_DIR` + solver lab, exact-vs-iterative post-step MB).
- **H3 (MB formula fidelity)**: our MB formula runs ~1.4x hot vs OPM's at the same state — a
  units/pore-volume/dt-factor discrepancy would explain everything, including the unexplained
  3x bounded-case gap (Bundle N §10 obs. 6: `12/1` vs Legacy `4/2` — if our MB reads hot,
  *every* `OpmAligned` case pays extra iterations chasing `1e-7`). Test: W0-style formula audit
  of `cnv_mb_diagnostics` against `NonlinearSystemBlackOilReservoir_impl.hpp`'s
  `getReservoirConvergence` (B_avg construction, pore-volume weighting, dt factor).

**4. The unexploited asset: OPM Flow is installed (`/usr/bin/flow`).** All week we compared
against OPM's *source* (static fidelity, W0-style) and its *end physics* (fine-dt FOPT) — never
its *per-iteration runtime trajectory* on the pathological window. A differential run — OPM Flow
on the heavy-case deck (the FOPT reference deck already exists per the opm-reference-pipeline)
with convergence logging, diffed against our ledger through the steady-state tail (t≈0.83-1.0) —
answers H2/H3 from the oracle side: what are OPM's actual MB values at the same simulated times,
and does it ever need its relaxed tier there? This should become a standing method, not a
one-off: trajectory-level differential testing is the direct form of "learn from OPM Flow".

**5. Combination coverage gaps found in the review** (now cheap post-`FIM-LINEAR-011`: capped
heavy runs are ~70s, full runs ~21 min — a factorial that would have cost 40+ hours a week ago
costs an afternoon now):
- **Legacy + W on the heavy case: never run** (only the `22x22x1` sanity check). Legacy's own
  heavy-case issues (the `perf@1299` mixed retries, the `water@1215` plateau ladders) are
  well-adjacent; if Legacy+W beats Legacy's current `52`, that is a *promotable Legacy-side win*
  independent of the whole `OpmAligned` question (own full gate per plan §7).
- OpmAligned+W ± forced-direct linear backend (the H2 test).
- OpmAligned+W with `min_strict_mb_iter` set positive — OPM's own shipped knob for "use relaxed
  MB after N iterations" — recorded as a *fallback only*: OPM's defaults solve this case without
  it, so reaching for the knob before understanding H1-H3 would be acceptance-widening in OPM
  clothing (`FIM-NEWTON-005` lesson applies).
- The `22x22x1` OpmAligned+W `12→24` regression and the `23x23x1` first-substep
  `linear-bad:oil@1585` — both uninvestigated, both cheap windowed traces.

**6. Bookkeeping reframe (the direct answer to "element by element you never promote"):**
declare the candidate stack (`OpmAligned` + `nested_well_solve`) a first-class tracked
configuration with its own registry identity and baseline (`18,015` @ this commit), measure
per-fix progress by **binding-constraint margin** (currently MB `1.41e-7` vs `1e-7`) and
substeps-to-t=0.9 on capped runs, and make the promotion decision once, for the stack, when the
chain is exhausted. Mid-chain mechanisms get "validated-in-stack" dispositions rather than
reading as failures. One porting nit to fold into the next verification pass: OPM's
`NewtonMinIterations` default is **2**; our `OPM_NEWTON_MIN_ITERATION_INDEX` is 1 — re-verify
the intended off-by-one semantics against `iterCtx.iteration()`.

**Recommended order**: (1) the FIM-DIAG-003 binding-cell trace + forced-direct capped run
(hours, discriminates H1/H2); (2) MB formula audit (hours, H3, also explains the bounded-case
3x if it hits); (3) OPM Flow differential trajectory on the heavy deck (a day, decisive from
the oracle side); (4) Legacy+W heavy case (minutes, possible independent win); (5) only then
decide whether the stack promotes or the approach changes.

### FIM-DIAG-003 checkpoint D0: instrumentation (2026-07-11)

Per `docs/FIM_DIAG_003_PLAN.md` D0. No-op gated; both additions are diagnostic-only and neither
touches the accept/retry decision.

**1. Binding-criterion trace.** `cnv_mb_from_parts` (`fim/newton.rs`) now also computes, per
component: `cnv_peak_cell[c]` (the cell realizing `max_coeff[c]`, tracked inline in the existing
loop) and `mb_peak_cell[c]` (the largest `|r_i,c|` among cells whose sign agrees with the summed
imbalance `r_sum[c]` — the cell(s) actually driving the MB error rather than one that cancels
against it). A new `binding: Option<BindingCriterion>` field names the single failing
criterion (CNV or MB, which component) with the largest `value/tolerance` overshoot ratio. The
`CNV-MB` trace line gained a trailing `binding=[...]` field, e.g.
`binding=[mb[water]=1.412e-07/1.000e-07 cell=143]`, or `binding=[none]` when `would_accept`.
`fim_trace!` always runs (writes to the sim's trace buffer unconditionally); this is pure
diagnostic output, no control-flow change.

**2. Forced-direct-linear switch.** New `fim_force_direct_linear: bool` field on
`ReservoirSimulator` (`lib.rs`, default `false`), set only via a `pub(crate)`
`set_fim_force_direct_linear` (added to the existing plain `impl ReservoirSimulator` in
`timestep.rs`, alongside `append_fim_trace_line` — no `#[wasm_bindgen]`, no wasm surface, same
as the pattern established for `fim_nested_well_solve`). When set, `step_internal_fim_impl`
forces `newton_options.linear.kind = FimLinearSolverKind::SparseLuDebug` — every Newton
iteration solves exactly via the direct backend instead of the default iterative CPR/GMRES
stack. Wired into the native repro driver
(`repro_water_pressure_12x12x3_opm_aligned_no_trace`, same env-gated pattern as
`FIM_NESTED_WELL_SOLVE`): `FIM_FORCE_DIRECT_LINEAR=1` sets the flag before `sim.step()`.

**Gate results** (clean tree, commit pending this checkpoint):
- `cargo build --manifest-path src/lib/ressim/Cargo.toml` (lib only): clean, no new warnings.
- `cargo build --manifest-path src/lib/ressim/Cargo.toml --tests`: clean (the `set_...` setter
  warning under lib-only build is expected — it's only called from the `#[cfg(test)]` repro
  driver).
- `cargo test assembly_ad`: 10/10 pass (parity gates untouched).
- Locked smoke: `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`,
  `spe1_fim_first_steps_converge_without_stall` (124.5s), `spe1_fim_gas_injection_creates_free_gas`
  (425.1s) — all pass.
- `bash scripts/build-wasm.sh`: succeeds (`release` profile, `wasm-opt` optimized).
- Control matrix (`fim-solver-debug` skill, all 6 commands): pending — the sandbox's Bash safety
  classifier went temporarily unavailable mid-session; will complete and record before D1.

### FIM-DIAG-003 checkpoint D2: H3 MB formula audit (2026-07-11)

Per `docs/FIM_DIAG_003_PLAN.md` D2. Static, line-by-line comparison of `cnv_mb_from_parts`
(`fim/newton.rs:1846-` — updated line numbers post-D0) against the pinned OPM checkout at
`OPM/opm-simulators` (verified `git log -1` = `062cb19986aa8f11cffc30351fd2fee355d0ccb4`,
`interim_release/2024.12-4152-g062cb1998`, clean tree — this IS the tag the design doc cites).

**Quantity-by-quantity comparison**, OPM source in
`opm/simulators/flow/NonlinearSystemBlackOilReservoir_impl.hpp`:

| Quantity | OPM (file:line) | ResSim (`fim/newton.rs`) | Verdict |
|---|---|---|---|
| `B_avg[c]` | `getMaxCoeff` (`:1114` etc.): `Σ 1/invB(phase)` per cell, then `/= global_nc_` in `localConvergenceData` (`:621-623`) — current-iterate pressure | `b_avg[c] = Σ fvf[c] / n_cells`, `fvf` from `state.cells[idx].pressure_bar` (current iterate) | **Matches** |
| `R_sum[c]` | `getMaxCoeff` (`:1117`): raw `modelResid[cell][compIdx]` accumulated, no PV weight | `r_sum[c] += residual[i*3+c]` raw | **Matches** |
| `maxCoeff[c]` (CNV) | `getMaxCoeff` (`:1118-1121`): `max_i(|R_i|/pvValue_i)`, `pvValue = referencePorosity(cell,t=0) * dofTotalVolume(cell)` — fixed reference PV | `max_coeff[c] = max_i(|r|/pv_i)`, `pv_i = sim.pore_volume_m3(i) = dx*dy*dz*porosity` (pure geometric reference, no rock-compressibility factor — confirmed via `grid.rs:17-19`, contrasted with the compressibility-adjusted PV the actual accumulation term uses, `properties.rs:136-145`) | **Matches** — both use fixed reference PV for the convergence check, deliberately different from the compressed PV inside the physics itself |
| per-cell max-over-components (CNV-PV-split) | `characteriseCnvPvSplit` (`:646-656`): `maxCnv = max_c(\|r_c\| * B_avg[c])` (inner_product with `max` combine, `multiplies` per-element — **not** a sum, re-checked after an initial misread) | `cell_max_cnv = cell_max_cnv.max(r.abs()*b_avg[c]/pv)` | **Matches** |
| `pv_sum` | `localConvergenceData` (`:607`): same `pvValue` accumulated across all cells | `pv_sum += pv` (same `pore_volumes_m3[i]`) | **Matches** |
| `CNV[c]` | `:827`: `B_avg[c] * dt * maxCoeff[c]` (`dt` explicit — OPM's residual is a **rate**, verified via `fvbaselocalresidual.hh:590-596`: storage `(V_new-V_old)*scvVolume/dt`, flux already a rate via `computeFlux`/`alpha`) | `cnv[c] = b_avg[c] * max_coeff[c]` (no `dt` — ResSim's residual is dt-integrated: accumulation is a raw `current-previous` volume difference with **no** `/dt`, verified `properties.rs::cell_accumulation_generic:199-203`; flux/well terms are `coefficients * q * dt_days`, verified `assembly_ad.rs:144,246,249`) | **Matches structurally** — `ResSim_residual ≡ dt_days * OPM_rate_residual` by construction, so the `dt` factor is legitimately absorbed; this is inherently a dimensionless ratio either way (the `dt` cancels within each system's own formula), not a source of a fixed cross-system scale error |
| `mb[c]` | `:828`: `\|B_avg[c]*R_sum[c]\| * dt / pvSum` | `mb[c] = \|b_avg[c]*r_sum[c]\| / pv_sum` | **Matches** (same dt-absorption argument) |
| `ToleranceMb` / `ToleranceMbRelaxed` / `ToleranceCnv` | `1e-7` / `1e-6` / `1e-2` (already verified week-retrospective) | same constants | **Matches** (re-confirmed) |
| `RelaxedMaxPvFraction` | `BlackoilModelParameters.hpp:50`: `0.03` | `OPM_RELAXED_MAX_PV_FRACTION = 0.03` | **Matches** |
| `min_strict_mb_iter_` relax-final-iteration gate | `:752-753`: `relax_final_iteration_mb = min_strict_mb_iter_ < 0 && iteration() == maxIter` | `relax_final_iteration` passed in as `is_final_newton_iteration = iteration+1 == max_newton_iterations` | **Matches** |

**No `~1.4x`-shaped formula bug found.** Every sub-quantity in the MB/CNV computation is a
faithful, source-verified port. **H3 (MB formula fidelity) is REFUTED** as an explanation for
the `1.41e-7` freeze — the formula itself is correct.

**Independent finding (not H3): confirmed off-by-one in `OPM_NEWTON_MIN_ITERATION_INDEX`.**
Traced the exact semantics of OPM's `iteration() >= minIter` gate:
`NewtonIterationContext::iteration()` (`NewtonIterationContext.hpp:52-55`) is 0-based and starts
at 0 each timestep (`resetForNewTimestep`, `:122-128`); `NonlinearSystemBlackOilReservoir::
nonlinearIteration` (`:203-231`) calls `initialLinearization` (assemble + convergence check
against the **current** `iteration()` value, **before** any update this pass) then, only after
that, `this->simulator_.problem().advanceIteration()` (`:229`) — so `iteration()` at each check
equals the number of Newton **updates already applied** in this timestep, exactly mirroring
ResSim's own `for iteration in 0..max_newton_iterations` loop structure (`newton.rs:2802` region:
assemble → check `converged_on_entry` → apply update only if not converged). `NewtonMinIterations`
default is **2** (`BlackoilModelParameters.hpp:163`), checked as `iteration() >= minIter`
(`NonlinearSystemBlackOilReservoir_impl.hpp:175`) — so OPM requires `iteration() >= 2`, i.e. at
least **2 Newton updates already applied** (3 total residual evaluations) before acceptance is
even possible. ResSim's `OPM_NEWTON_MIN_ITERATION_INDEX = 1` only requires `iteration >= 1` (1
update applied, 2 evaluations) — **one iteration too permissive**, confirmed by direct
correspondence between the two loop structures, not just parameter-name pattern-matching.

This is a real, independently-scoped correctness bug, but **does not explain the heavy-case
plateau**: the stuck substeps run ~18-20 Newton iterations, far past either gate value, so
`minIter=1` vs `minIter=2` is irrelevant once the substep is already this deep. It could plausibly
explain (or contribute to) the still-unexplained bounded-case 3x gap (`OpmAligned` 12/1 vs
Legacy 4/2 — Bundle N §10 obs. 6) for fast-converging cases where the gate is the actual limiter.
**Not fixed in this checkpoint** — flagged as a candidate follow-on fix, own registry row, own
control-matrix + locked-smoke gate per the project's promotion discipline (changes acceptance
behavior under `OpmAligned` everywhere, not just the heavy case). Decision on fix timing deferred
to D5.

**D2 verdict**: H3 refuted via source-cited static audit. No fix, no re-run triggered by this
checkpoint (the plan's "if a discrepancy is found, fix it" branch does not fire for H3 itself).
Effort: ~1.5h (audit) vs the ~2-4h estimate.

### FIM-DIAG-003 checkpoint D1: H1 CONFIRMED, H2 REFUTED (2026-07-12)

Per `docs/FIM_DIAG_003_PLAN.md` D1. Two windowed capped runs, native `--release`, commit
`a4fad1c` (D0+D2 checkpoint): `FIM_TRACE_SUBSTEP_START=980 FIM_MAX_SUBSTEPS=1000
FIM_NESTED_WELL_SOLVE=1 cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib
repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture`, with/without
`FIM_FORCE_DIRECT_LINEAR=1`, `FIM_TRACE_FILE` pointed at a scratch log.

**Run 1 (baseline, default `fgmres-cpr`)**: `accepted_substeps=1000 advanced_dt=0.916822/1.0
linear_bad=8 nonlinear_bad=1 mixed=3`, wall `75.85s`. Binding-cell census over the whole window
(238 `binding=[mb...]` lines): **198/218 (91%) at cell 143, 20/218 (9%) at cell 130** — zero
lines anywhere else. Cell 143 is `idx(11,11,0)` — the producer's own perforation cell (`nx-1,
ny-1, 0`, confirmed from the repro driver's `add_well` call). Cell 130 is `idx(10,10,0)`, the
producer's immediate diagonal neighbor in the same (only-completed) layer. **100% of the frozen
MB is concentrated at the well or its immediate neighborhood.**

**Run 2 (`FIM_FORCE_DIRECT_LINEAR=1`, every Newton iteration solved exactly via
`SparseLuDebug`)**: `accepted_substeps=1000 advanced_dt=0.922922/1.0 linear_bad=0
nonlinear_bad=13 mixed=0`, wall `171.58s` (2.3x slower, as expected — direct factorization vs
iterative CPR/GMRES, consistent with `FIM-LINEAR-011`'s own cost measurements). **The freeze does
not break**: binding-cell census (220 lines) is **180/200 (90%) at cell 143, 20/200 (10%) at
cell 130** — same cells, same ~91/9 split. MB magnitude at the frozen plateau is `2.301e-7`/
`2.331e-7` in this run's tail window vs `1.412e-7`/`1.423e-7` in the baseline's — **higher**, not
lower, with exact linear solves (different substep/trajectory state, not a strict single-point
comparison, but decisively not "drops below `1e-7`"). Progress metric: `advanced_dt` moved
`0.9168→0.9229`, a **0.6% improvement** — nowhere near the plan's decisive-test bar ("materially
improves").

**This run is also the D1 point-3 cross-check** (forced-direct + binding-cell trace together):
exact linear solves with the residual still parked at the well cells is the plan's own stated
signature for **pure H1**.

**Verdict**:
- **H1 (displaced standoff) CONFIRMED.** The frozen `1.41e-7`-class MB literally lives at the
  producer's perforation cell (and its immediate neighbor) in both runs, regardless of linear
  solve exactness — direct evidence that Bundle W's fix (which drove the perforation-flow
  residual itself to machine epsilon, W4) displaced the same underlying well/reservoir
  inconsistency into the well-cell mass-balance row rather than resolving it. This matches the
  mechanism already on record from the pre-D1 trace: the coupled linear solve proposes `dq≈+0.58`
  each iteration to zero those rows via the source term, and the nested well solve vetoes it back
  — an invariant point of the modified iteration map, now confirmed to be spatially exactly where
  H1 predicted.
- **H2 (linear-precision floor) REFUTED.** Forcing exact linear solves neither breaks the freeze
  nor moves it off the well cells nor materially improves progress — the `5e-3` outer linear
  tolerance is not the limiting factor near this plateau.
- H1 and H3 (refuted at D2) together leave H1 as the sole standing explanation. The fix direction
  is therefore **nonlinear/well-coupling**, not linear-tolerance policy and not a CNV/MB formula
  bug.

Effort: ~15 min setup + 76s + 172s run time + analysis, well under the ~1h estimate.

### FIM-DIAG-003 checkpoint D4: combination coverage (2026-07-12)

Per `docs/FIM_DIAG_003_PLAN.md` D4, both items, using `scripts/fim-wasm-diagnostic.mjs`
(`--opm-aligned`/`--nested-well-solve` flags), commit `e12b95d`.

**1. Legacy + `nested_well_solve` on the heavy case (never run before).** Raw summary line:
`substeps=8 accepts=7+4+14907 dt=[6.104e-5,3.125e-2]`, real accept rungs `s0@3.125e-2 →
s6@6.104e-5` (7 entries, dt collapsing monotonically over just 7 real Newton-solved steps).
**Read carefully, not at face value**: `substeps=8` looks dramatically better than Legacy's own
baseline `substeps=52` (`real=51,cooldown=3,hotspot_plateau=2060`, real accept rungs run
`s0@3.125e-2` through the high-30s/40s before the tail collapses) — but the `accepted_substeps`
ledger field collapses an entire hotspot-plateau replay block into ~1 history entry regardless of
its size (`51 real + 1 collapsed block ≈ 52`; `7 real + 1 collapsed block ≈ 8` — the arithmetic
matches exactly in both cases). The real signal is `real_accepted_substeps`: Legacy alone does
**51** genuine Newton-solved dt advances before the tail-end plateau; Legacy+`nested_well_solve`
does only **7** before permanently stalling at `dt=6.1e-5` and having the remaining ~99.8% of the
timestep auto-filled by cheap plateau-replay bookkeeping (14,907 replayed units, vs Legacy's own
2,060). **This is a regression, not a win** — `nested_well_solve` under Legacy causes a much
*earlier and more severe* stall (dt collapses to the plateau floor after 7 real attempts instead
of ~50), the opposite of the `docs/FIM_DIAG_003_PLAN.md` "possible independent win" framing
(condition explicitly required "physics intact," which this fails). This is exactly the
measurement trap the skill (`.claude/skills/fim-solver-debug/SKILL.md` "Reading the summary
line") and the week retrospective (§2, "the measurement was blind") both warn about — do not
read `accepted_substeps` alone as the gate metric when a plateau-replay block is present; check
`real_accepted_substeps` first. **Not a promotable win. No further action.**

**2. The `22x22x1` OpmAligned+`nested_well_solve` "`12→24` regression" and the `23x23x1`
`linear-bad:oil@1585` ride-along.** Re-derived at current HEAD (baseline discipline: "do not
trust expected counts written in old docs"):

| Case | OpmAligned alone | OpmAligned + `nested_well_solve` | Delta |
|---|---|---|---|
| `22x22x1` | `substeps=24`, `retry_dom=linear-bad:oil@1450`, `avg_p=319.00` | `substeps=24`, same `retry_dom`, same `avg_p` | **bit-identical** |
| `23x23x1` | `substeps=12`, `retry_dom=linear-bad:oil@1585`, `avg_p=317.18` | `substeps=12`, same `retry_dom`, same `avg_p` | **bit-identical** |

**Does not reproduce.** Both bounded cases are confirmed no-ops for `nested_well_solve` at the
current tree (matches every other field checked: `oil`, `inj`, `gor`, `dt` bounds). Note `23x23x1`
OpmAligned-alone's own number (`12`) matches the "`12`" the retrospective attributed to
`22x22x1` — most likely a mislabel/stale reading from an earlier commit, not a real regression
that has since been fixed (no intervening commit touched the `nested_well_solve`-off path). The
week-retrospective row is superseded: **no `22x22x1` regression exists on the current tree; both
bounded no-ops reconfirmed.**

**D4 verdict**: heavy Legacy+W = confirmed regression (not promotable); both bounded no-ops
reconfirmed clean, prior "regression" claim was stale/non-reproducible. No fixes triggered, no
new registry rows needed beyond this record (kept as this checkpoint's own evidence).

### FIM-DIAG-003 checkpoint D3: OPM Flow differential trajectory (2026-07-12)

Per `docs/FIM_DIAG_003_PLAN.md` D3, commit `c9d041e`. `/usr/bin/flow` (confirmed installed);
`origin/fim-opm-continuation-plan` has the deck harness but is stale relative to current master
(pre-dates most of `.claude/skills/`, several docs) — did **not** merge that branch; instead
recreated the specific deck adapted from its `water-medium-step1` template and tracked it fresh
on this branch as `opm/reference-decks/water-heavy-step1/CASE.DATA` (`DIMENS 12 12 3`, perms
2000/2000/200 mD matching `water-medium-step1` exactly, corner wells BHP 500/100, `TSTEP 1.0`).
One deliberate deviation from the template: `COMPDAT` well radius `0.1`, not the template's
`0.2` — verified against `frontend.rs::add_well` call sites that the actual repro driver
(`repro_water_pressure_12x12x3_opm_aligned_no_trace`) and the `fim-wasm-diagnostic.mjs` preset
both pass `well_radius=0.1`; the medium-step1 template's `0.2` does not match either and appears
to be its own pre-existing discrepancy (out of scope to fix here — noted for future cleanup).

Ran: `flow CASE.DATA --output-extra-convergence-info=steps,iterations` in a scratch working
directory (matching the harness's own copy-before-run pattern, keeping the tree clean).

**Result: OPM solves the entire `t=0→1.0` interval in ONE Newton solve, 11 iterations, ZERO
timestep cuts.** `CASE.INFOSTEP`: `Time=0 TStep=1 ... NewtIt=11 LinIt=14 Conv=1`. Total wall
time `0.03s`. This alone is a stark contrast with ResSim: Legacy needs 52 substeps (51 real),
Legacy+`nested_well_solve` collapses to 7 real substeps before stalling (D4), and
`OpmAligned`+`nested_well_solve` needs 18,015 substeps to cover the same interval.

**`CASE.INFOITER` per-iteration trajectory** (the decisive piece — answers the plan's D3
question #2 directly): at **iteration 10**, `MB_Oil=6.947e-7 MB_Water=1.970e-7` — the SAME
order of magnitude ResSim is frozen at (`1.41e-7`-`2.33e-7` across D1's runs), both still above
OPM's own strict `ToleranceMb=1e-7`. At **iteration 11** (the very next, and final, iteration),
`MB_Oil=1.088e-9 MB_Water=1.130e-8` — a clean **2-3 order-of-magnitude drop in a single Newton
step**, comfortably below tolerance (matching `CNV_Oil`/`CNV_Water`'s own `~250x` one-step drop,
the classic quadratic-convergence tail of a well-posed Newton iteration). `WellStatus=CONV` and
`PenaltyWellRes=0` at every iteration — no well-side distress recorded anywhere in the OPM
trajectory.

This directly answers the plan's own framing: **"is its MB at these states `~1e-8`, or
`~1.4e-7`-but-still-converging, or does it also touch its relaxed tier?"** — the answer is
**still-converging**: OPM transiently occupies the exact same residual-magnitude neighborhood
ResSim is stuck at, and takes one clean further Newton step through it. This is oracle-side proof
that `1e-7`-to-`2e-7` MB is not an inherent numerical floor for this physics/grid/well
configuration — it is a ResSim-specific structural stall, independently confirming H1 (D1) and
reinforcing H3's refutation (D2): a correctly-behaving Newton iteration passes through this exact
zone in one step, so there is nothing "hot" about the tolerance comparison itself, only about
ResSim's own iteration map having an invariant point there that OPM's doesn't.

`CASE.PRT` well-solver defaults (context, not a new lever): OPM's own well-equation inner solve
tolerance is `ToleranceWells=1e-4` — two orders looser than the reservoir `ToleranceMb=1e-7` — and
its own inner well iteration budget is `MaxInnerIterWells=50`/`StrictInnerIterWells=40`, far more
generous than ResSim's `nested_well_solve` inner solve. Consistent with (not new evidence beyond)
the already-recorded Bundle W design intent; not investigated further here, out of D3's scope.

**Not attempted in this checkpoint** (time-boxed per the user's "do D3 now, full plan" but the
single-shot result already being decisive): a matching multi-`TSTEP` OPM run replaying ResSim's
own accepted-dt sequence to compare dt-by-dt through the `t≈0.83-1.0` steady tail specifically.
The single-shot per-iteration trajectory already answers the load-bearing question (H1
independent oracle confirmation); the dt-sequence comparison would be corroborating, not
decisive, and is left as a candidate follow-on if a future fix needs finer trajectory-level
verification (this checkpoint doubles as the "adopt trajectory-level differential comparison as
a standing method" pilot per the week retrospective §4 — the method works and is cheap: one deck
+ one `flow` invocation + reading two small text files, seconds not hours).

**D3 verdict**: OPM oracle confirms H1 independently — a well-posed Newton iteration passes
cleanly through the exact MB magnitude ResSim is frozen at. No fix attempted (out of scope for a
diagnostic checkpoint); fix direction guidance for D5 is now well-triangulated from three
independent angles (D1's binding-cell census, D2's formula-fidelity audit, D3's oracle
trajectory): nonlinear/well-coupling, not linear tolerance, not CNV/MB formula fidelity, not an
inherent numerical floor at this residual magnitude.

### FIM-DIAG-003 checkpoint D5: verdict + `FIM-NEWTON-008` (2026-07-12)

Per `docs/FIM_DIAG_003_PLAN.md` D5, commit `08fe69b`.

**Verdict on H1/H2/H3**: the plan's three-way discrimination is complete and unanimous across
three independent methods:

- **H1 (displaced standoff into well-cell MB rows) — CONFIRMED.** D1's binding-cell census: 100%
  of the frozen-MB iterations bind at the producer's own perforation cell (91%) or its immediate
  neighbor (9%), in both the default and forced-exact-linear runs. D3's OPM oracle: a well-posed
  Newton step transits the exact same MB magnitude in one clean iteration with a 2-3
  order-of-magnitude drop, `WellStatus=CONV`/`PenaltyWellRes=0` throughout — proving the frozen
  magnitude is not intrinsically hard, only ResSim's own well-coupled iteration map has an
  invariant point there.
- **H2 (linear-precision floor) — REFUTED.** D1: forcing every Newton iteration through the
  exact `SparseLuDebug` backend neither breaks the freeze, moves it off the well cells, nor
  materially improves progress (`advanced_dt` `+0.6%` only).
- **H3 (MB formula fidelity) — REFUTED.** D2: line-by-line audit of `cnv_mb_from_parts` against
  the pinned OPM source found every sub-quantity a faithful, source-cited port; no `~1.4x`-shaped
  translation bug exists.

This is plan case **(a)**: "a scoped fix bundle for the confirmed mechanism." The mechanism is
now precisely located (the well-cell mass-balance rows, under `nested_well_solve`'s handling of
the coupled linear solve's `dq≈+0.58` proposal each iteration, vetoed back by the nested solve —
the exact signature already on record from before D1 even ran). **Designing and building that fix
bundle is out of scope for this diagnostic plan** (D0-D5 was explicitly instrumentation +
discrimination, "zero guessing budget spent on this mechanism" per the plan's own framing) — it
is the natural next unit of work, to be scoped as its own plan/bundle document with the same
checkpoint discipline as Bundles N/P/W, once opened.

**`min_strict_mb_iter` still explicitly out of scope**, now for a sharper reason than before: H1
is a genuine structural fixed point (the residual is bit-identical across 18 iterations, not
slowly converging), so relaxing WHEN the relaxed tolerance kicks in would not fix anything — it
would only widen acceptance around an unfixed defect, the exact `FIM-NEWTON-005` anti-pattern.

**Independent fix, PROMOTED this checkpoint (`FIM-NEWTON-008`)**: D2's off-by-one
(`OPM_NEWTON_MIN_ITERATION_INDEX` `1→2`) is small, well-understood, and orthogonal to H1 — fixed
and gated here rather than left open. Full control matrix (all 6 standard commands): **bit-identical**
(the constant is `OpmAligned`-only, so the Legacy/flag-off path is provably untouched — confirmed,
not just argued). `OpmAligned`-flavored re-runs of the three fast bounded cases:
`22x22x1`/`23x23x1` substep counts **unchanged**, `20x20x3` shows a small **expected increase**
(`15→16`, matching the fix's direction — stricter acceptance, closer to OPM's actual default,
which requires more not fewer forced iterations). Heavy-case pathology is unaffected by
construction (the floor only gates iterations 0-1; the heavy case fails at iteration ~18-20) — not
re-run for this fix, the code-path argument is exact. Locked smoke 3/3, `assembly_ad` parity
10/10. **This fix does NOT explain or close the bounded-case 3x gap** (`OpmAligned` `12/1` vs
Legacy `4/2`, Bundle N §10 obs. 6) — if anything it moves `OpmAligned` iteration counts up
slightly, the opposite direction from "explaining away the gap." That gap remains open and
unexplained; H3's refutation (D2) already ruled out the leading hypothesis for it (a hot MB
formula), so it is now itself a small standing question, not urgent enough to warrant its own
diagnostic plan yet.

**Stack promotion status: still OPEN.** The candidate stack (`OpmAligned` + `nested_well_solve`,
baseline `18,015` substeps @ `c916c87`) has not been re-evaluated end-to-end — that gate is the
original Bundle N §5 gate (heavy `≤35`-substep class + fine-dt FOPT + control matrix + bounded
cases not worse than Legacy) and stays closed until a fix for the now-precisely-located H1
mechanism exists and is evaluated. `FIM-DIAG-003` itself is **closed as a diagnostic** (D0-D5
complete, unanimous verdict, zero unresolved hypotheses) even though the underlying pathology is
not yet fixed — this is the correct disposition per the retrospective's own bookkeeping reframe
("mid-chain mechanisms get 'validated-in-stack' dispositions rather than reading as failures").

**Summary of this diagnostic's yield**: one independently promoted correctness fix
(`FIM-NEWTON-008`), one confirmed regression averted (`nested_well_solve` under Legacy, D4 — would
have been a false "win" without the `real_accepted_substeps` correction), one stale claim
retracted (the `22x22x1` "regression," D4), one refuted formula-fidelity concern closed (H3, D2),
one refuted linear-tolerance concern closed (H2, D1), and the mechanism precisely located for a
future fix bundle (H1, D1+D3) — all without a single guess spent on the mechanism itself before
the evidence was in hand.

## Bundle X (`docs/FIM_BUNDLE_X_PLAN.md`): well-update ordering / well-fraction fidelity

### Bundle X checkpoint X0: stage-by-stage forensics — a DIFFERENT root cause than planned (2026-07-12)

Per `docs/FIM_BUNDLE_X_PLAN.md` X0. Commit base `1fdd157`. Extended the D0 window instrumentation
with a new `WELLJAC` trace line (`fim/newton.rs`, window-gated, no-op when inactive) dumping,
per perforation, the `rate_consistency` row's own residual/diagonal plus its cell's water/oil/gas
row residuals and their `d/dq`, `d/dp`, `d/dsw` couplings — read directly from `assembly.jacobian`
via the same `CsMat::get(row, col)` accessor the W1 agreement test uses — plus a `WELLJAC-WATER`
line with the full face-by-face flux/accumulation/well-source breakdown
(`cell_equation_residual_breakdown`, the same helper `FAIL-SITE-DETAIL` uses). Windowed capped run
(`FIM_TRACE_SUBSTEP_START=980 FIM_MAX_SUBSTEPS=1000 FIM_NESTED_WELL_SOLVE=1`, same window as D1).

**Finding 1 — the planned question is answered, and it eliminates the planned suspects.**
`res_pf=0.000000e0` and `d(res_pf)/dq=1.000000e0` (a trivial, always-1 diagonal — the row is
definitional, `residual = q − f(p_cell, bhp, mobility)`) at every frozen iteration. A row with
zero residual and a unit diagonal contributes **zero** pull toward any `dq` change on its own —
the observed `raw_dq≈+0.70` is not coming from the well's own equation at all. Ruled out by
direct measurement, not inference: relaxation (`PERCELL-CHOP relax=1.00`, already on record),
chop (`sat_chopped_cells=0`, already on record; confirmed `opm_per_cell_chopped_update` never
touches perforation-rate entries by code inspection), `enforce_cell_bounds`/`enforce_control_bounds`
(confirmed by code inspection: neither function references `perforation_rates_m3_day` at all).

**Finding 2 — cell 143's own mass-balance residual is real and large relative to its only
effective lever.** Raw (unscaled) water/oil residuals at cell 143: `water=+1.696e-3`,
`oil=−1.721e-3` — genuinely nonzero (the `~1e-9`-scale figures seen earlier were
`EquationScaling`-normalized, not raw). Their Jacobian sensitivities: `d/dsw≈±20` (water `+20.00`,
oil `−20.04`, nearly exact opposites — a saturation move trades almost 1:1 between the two,
the expected mass-conservation identity), `d/dp≈2.7e-6` (water) `/5.55e-3` (oil), `d/dq≈4.5e-7`
(water) `/2.59e-5` (oil). **`dsw` is 3-4 orders of magnitude the dominant lever** — a fix via `dq`
alone would need `dq≈66` (vs the observed `0.70`); a fix via `dsw` alone needs only `≈-8.5e-5`.

**Finding 3 — `dsw` is legitimately clamped and cannot move.** `sw=0.100000` exactly, every
iteration — matching `s_wc=0.1` exactly (the repro's connate/irreducible water saturation, also
the reservoir's uniform initial condition — cell 143 is the producer, farthest corner from the
injector in a 12x12 grid, and genuinely has not yet seen breakthrough at `t≈0.92d` of `1.0d`).
`raw_dsw≈-7.6e-5` (the coupled linear solve's own proposed step) would push `sw` to `≈0.09992`,
**below** the connate floor — `enforce_cell_bounds`'s `cell.sw = cell.sw.clamp(sim.scal.s_wc, ...)`
clamps it straight back to exactly `0.1` every iteration, discarding the correction. This clamp
runs **unconditionally**, before the `WellStateUpdateMode` branch (`state.rs:435-438`) — i.e. it
would fire identically under `Relax`, `NestedSolve`, or `None`. **The `nested_well_solve`
ordering (the original Bundle X hypothesis) is not the primary mechanism** — the `dq` veto is
real (confirmed in D1/W's own traces) but it is downstream of, and secondary to, a saturation-bound
collision that has nothing to do with which well-update mode is active.

**Finding 4 — where the persistent residual actually comes from, and why it's structural, not
numerical noise.** `WELLJAC-WATER`: `accum≈-9.7e-9` (negligible — `sw` isn't moving, consistent
with Finding 3), `x-`/`y-` flux ≈`-1.7e-5` each (negligible — near-zero cross-cell water mobility,
as expected this close to the front), **`well=+1.730e-3`** — the well source term alone accounts
for essentially the entire residual (`total=1.695941e-3 ≈ well`). The producer's phase-split
(`producer_fractions_generic`, `fim/wells_ad.rs:52-81`) is **not** computed from the perforated
cell's own mobility alone — `perforation_control_cells` (`fim/wells.rs:822-838`, the single
shared call site for both the coupled assembly, `assembly_ad.rs:126/192/305`, and the nested
solve, `wells_inner.rs:82-92`) builds a **3x3 areal window** around every *producer* perforation
(injectors already get the single-cell treatment, `if perforation.injector { return
vec![perforation.cell_index]; }`) and sums mobility across it. Cell 143's own `krw` is exactly `0`
at `sw=0.1` (`SWOF` table endpoint), but its 3x3 neighborhood includes cell 130 (D1's secondary
binding cell, 9% of iterations) and other near-front neighbors with slightly elevated `sw` — their
nonzero water mobility leaks into the *well's* aggregate `water_fraction`, producing a small but
persistent nonzero water withdrawal that gets debited entirely against cell 143's own water
balance (the well source term is applied at the perforated cell, not distributed across the
neighborhood that supplied the mobility estimate). This is a structural mismatch, not noise — it
recurs identically every iteration because both the neighborhood-averaged fraction and cell 143's
own zero local mobility are themselves stable/converged; there is no lever in the system that can
zero it (Finding 2/3).

**Finding 5 — OPM does not do this.** Read `WellInterface<TypeTag>::getMobility`
(`WellInterface_impl.hpp:2105-2143`, pinned `062cb1998`): `mob[activeCompIdx] =
intQuants.mobility(phaseIdx)` at `intQuants = simulator.model().intensiveQuantities(cell_idx, 0)`,
where `cell_idx = this->well_cells_[local_perf_index]` — **the single connected cell only**, for
every well type (no producer/injector distinction, no neighborhood). `StandardWell::getMobility`
(`StandardWell_impl.hpp:706-756`) layers polymer/solvent/`WINJMULT` adjustments on top of that
same single-cell mobility — still no neighborhood averaging anywhere in the connection-rate path.
OPM's producer at the equivalent state would compute `water_fraction=0` **exactly** (its own
`krw=0` at the connate floor, nothing to blend in), withdrawing pure oil with no residual to
create. This directly explains D3's oracle finding (OPM transits the same MB magnitude cleanly in
one more iteration): OPM's local-only formulation never manufactures this residual in the first
place.

**Origin of the 3x3 design**: `git log -S "producer_control_state"` traces it to `d824f4f`
("Add producer control state management and enhance well control logic"), part of the original
pre-FIM, pre-OPM-alignment simulator design. `wells_ad.rs::producer_fractions_generic` is
documented as "a generic mirror of `wells::producer_control_state`'s fraction computation" — the
FIM/AD layer faithfully carried this convention forward without re-examining it against OPM. Not
a deliberate OPM-motivated design choice; an inherited, never-revisited divergence.

**Revised diagnosis for `FIM-BUNDLE-X`**: the well-update-ordering hypothesis (X1/X2 as originally
planned) is downgraded from primary to secondary/contingent. The primary, now-precisely-located
candidate fix is narrower and different in kind: **restrict `perforation_control_cells`'s producer
branch to the single perforated cell**, matching both the injector branch's existing behavior and
OPM's `getMobility` exactly. This is a single shared function (confirmed one call site pattern
across `assembly_ad.rs` and `wells_inner.rs`), scoped to the FIM engine only (`fim/wells.rs` is
separate from the legacy/IMPES `well_control.rs::producer_control_state_for_pressures`, confirmed
by grep — no legacy/public-simulator blast radius). Higher-leverage and more surgical than an
ordering change, but broader-reaching (it changes producer water-cut/GOR physics for every FIM
case with a producer, not just the heavy case) — needs the full control-matrix + locked-smoke +
BL-benchmark gate, not just a capped heavy-case check, before any promotion decision.

Plan doc (`docs/FIM_BUNDLE_X_PLAN.md`) updated to record this pivot; X1 retargeted from "pure
coupled well update" to "single-cell producer fraction," the original ordering probe kept as a
secondary/fallback item.

### Bundle X checkpoint X1: single-cell producer fraction — the fix, decisive result (2026-07-12)

Per `docs/FIM_BUNDLE_X_PLAN.md` X1 (retargeted per X0). Commit base `bb23c81`.

**Implementation**: `perforation_control_cells` (`fim/wells.rs:822`) gained a dev flag
(`fim_single_cell_producer_fraction`, default `false` = unchanged 3x3-window behavior). When set,
producer perforations get the same single-cell treatment injectors already have —
`vec![perforation.cell_index]` — matching OPM's `WellInterface::getMobility` exactly (X0
Finding 5). Threaded as a proper wasm-exposed dev flag (`setFimSingleCellProducerFraction`,
`frontend.rs`, mirroring `setFimNestedWellSolve`'s pattern) rather than native-only, since X1's
own gate needs the standard wasm control matrix on the bounded cases — and because this is a
strong enough candidate to be worth promoting all the way, not just probing. Native repro driver
gained the matching `FIM_SINGLE_CELL_PRODUCER_FRACTION` env var. One correction during
implementation: the native-only setter was initially given the same name as the wasm one,
which doesn't compile (duplicate inherent method across `impl` blocks) — resolved by keeping
only the wasm-exposed `pub` version, callable from the native test driver too.

**Heavy-case result — decisive.** Native repro driver
(`repro_water_pressure_12x12x3_opm_aligned_no_trace`), clean uncapped runs (no windowing, no
trace file, verified twice for reproducibility):

| Configuration | `accepted_substeps` | `advanced_dt` | wall time |
|---|---|---|---|
| `OpmAligned` + `nested_well_solve` (baseline, `c916c87`) | `18,015` | `1.0/1.0` | `1235.5s` |
| `OpmAligned` + `nested_well_solve` + `single_cell_producer_fraction` | **`16`** | `1.0/1.0` | **`3.05s`** |
| `OpmAligned` alone (no `nested_well_solve`) + `single_cell_producer_fraction` | **`16`** | `1.0/1.0` | **`2.95s`** |

**A ~1126x reduction in substep count, ~400x wall-clock speedup, and the fix works identically
with or without `nested_well_solve`** — confirming X0's finding that the well-update-ordering
mechanism was never the primary defect. `linear_bad=0 nonlinear_bad=1 mixed=0` in the fixed run
(vs baseline's `linear_bad=8 nonlinear_bad=1 mixed=5`-class retry pattern even within a
1000-substep capped window) — the fix doesn't just shorten the run, it makes it clean. `16`
substeps for a 1-day interval is in OPM's own efficiency class (OPM: 11 Newton iterations in a
*single* substep for the same interval, D3) — ResSim now needs a handful of substeps rather than
tens of thousands.

**Isolation and no-regression checks, all wasm-based (`fim-wasm-diagnostic.mjs`)**:
- Full 6-command standard control matrix, flag OFF: **bit-identical** to the recorded baseline on
  every field (`20x20x3`=`8`, `22x22x1`=`4`, `23x23x1`=`4`, `gas-rate 20x20x3`=`2`, `gas-rate
  10x10x3` 6-step outer = `4,2,2,2,2,2`, heavy `12x12x3`=`52`/`51+3+2060`) — the new code path is
  fully inert when the flag is unset, as expected (`perforation_control_cells`'s new branch is an
  `||` addition to the existing injector check, structurally cannot fire when the flag is false).
- `22x22x1`/`23x23x1` under `--opm-aligned`, flag ON vs OFF: **bit-identical**
  (`24`/`24` substeps, `12`/`12`, matching `avg_p`/`oil`/`inj`/`retry_dom` exactly) — the fix
  changes nothing on cases that don't hit this specific pre-breakthrough-corner-producer
  scenario, no regression risk apparent on the cases already exercised regularly.
- `assembly_ad` parity: 10/10.
- Locked smoke, 3/3: `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`,
  `spe1_fim_first_steps_converge_without_stall` (`218.5s`), `spe1_fim_gas_injection_creates_free_gas`
  (`432.5s`) — all pass with the code change in the tree (flag defaults off, but this exercises
  the changed `perforation_control_cells` function's injector/producer branch structure).

**Remaining gates, completed** (the sandbox environment was severely CPU-throttled during this
checkpoint — observed ~1:60 CPU-time-to-wall-clock ratio on the background `cargo test` process,
several hours wall-clock for the `fim` bucket alone — but all gates below did complete and pass):
- `bash scripts/validate-solver-coverage.sh fim`: **8/8 pass** — `fim::tests::spe1::` (the same 2
  tests already verified individually above, plus this run confirmed no others in that filter),
  `fim::tests::wells::` (3 tests, including `single_cell_producer_reporting_matches_local_source_state`
  — directly relevant to this change, passes), 3 `dep_pss_fim_*` depletion tests.
- `bash scripts/validate-solver-coverage.sh shared`: hit a **pre-existing, unrelated** failure
  partway through (`closed_system_public_step_keeps_same_water_inventory_on_both_solvers`, `assert_eq!
  (fim.2, 1)` — left `2`, right `1`; the script's `set -euo pipefail` stops at first failure).
  Verified pre-existing via `git stash` (reproduces identically on the clean `bb23c81` tree,
  before any Bundle X X1 changes) — and structurally cannot be caused by this change regardless:
  the test constructs a well-less closed system (`ReservoirSimulator::new(4,4,1,0.2)`, no
  `add_well` calls), so `perforation_control_cells` is never invoked (zero perforations). Ran the
  remaining 11 tests in the bucket individually past that point: **11/11 pass**
  (`simple_pressure_control_public_step_has_same_stable_contract_on_both_solvers`,
  `shared_block_multiwell_public_step_remains_finite_on_both_solvers`, 4
  `physics_depletion_*`/`physics_waterflood_*`/`physics_gas_flood_*` contract tests,
  `physics_gas_cap_vertical_column_fim_matches_impes_hydrostatic_benchmark`,
  `physics_wells_sources_gas_injection_surface_totals_match_target_on_both_solvers`, 2
  `physics_geometry_*` tests). The pre-existing failure itself is a new discovery worth a TODO
  entry (its symptom — an extra `rate_history` entry on the FIM path for a well-less closed
  system — closely resembles the already-known-and-tracked "3 pre-existing failures found
  2026-07-07" class in `TODO.md`, though not an exact name match; not investigated further here,
  out of scope for Bundle X).
- `benchmark_buckley`: **3/3 pass** (`benchmark_buckley_leverett_case_a_favorable_mobility` rel_err
  `0.041`, `case_b_more_adverse_mobility` rel_err `0.090`, `smaller_dt_improves_coarse_alignment`
  — all within the existing validated tolerances, untouched).

**X1 verdict: PROMOTABLE.** Every gate in the plan's X3 promotion checklist that doesn't require
the full uncapped heavy re-run (done above, `16` substeps) or the D3 oracle comparison (X3's own
remaining item) is green. This is the rare case of a single-function fix (`perforation_control_cells`,
one `||` condition added) resolving what had been, across the whole `FIM-BUNDLE-N`/`FIM-BUNDLE-W`/
`FIM-DIAG-002`/`FIM-DIAG-003` arc, an ~18,000-substep catastrophic failure — because the arc had
been chasing a downstream symptom (the well-cell MB freeze) of an upstream formula-fidelity gap
that had never been compared against OPM's actual source until D3/X0 did so directly.

### Bundle X checkpoint X3: D3 oracle re-comparison, generality checks, stack promotion (2026-07-12)

Per `docs/FIM_BUNDLE_X_PLAN.md` X3. Commit base `9bb4925`.

**D3 oracle re-comparison.** Full unwindowed `LEDGER` trace of the fixed run
(`OpmAligned`+`nested_well_solve`+`single_cell_producer_fraction`, `FIM_TRACE_SUBSTEP_START=0`,
no cap needed — only 16 substeps): dt schedule climbs cleanly from `0.0825` to `0.259` days,
reaching the exact `dt≈0.185`-class step the original D3 plan asked about
("does OPM hold `dt=0.185`-class steps at 2-3 iterations where we collapse?") — substep 12 covers
`t=0.313→0.498` at `dt=0.185031` in **7 iterations**, substep 13 covers `t=0.757→1.0`-ish at
`dt=0.259044` in **12 iterations**. `mb` values throughout: `1e-8`-`1e-10` range, comfortably
under strict tolerance — no plateau anywhere in the trace. Total Newton iterations across all 16
substeps: **168** (summing the `iters` column) vs OPM's **11** in its single one-substep solve
(`docs/FIM_CONVERGENCE_WORKLOG.md` "FIM-DIAG-003 checkpoint D3"). ResSim is now firmly in a
*functional* regime — clearing the `≤35`-substep gate by more than 2x — but still ~15x less
iteration-efficient than OPM per unit of simulated time, consistent with `OpmAligned`'s known,
separately-tracked per-iteration cost gap (Bundle P's own wall-clock attribution work) and not
something this fix was aimed at closing.

**Generality checks (not required by the plan, run because the finding was too clean not to
stress further):**

1. **Legacy flavor benefits too.** `water-pressure 12x12x3 --dt 1` under Legacy (no
   `--opm-aligned` at all) + `single_cell_producer_fraction`: **`52 → 25` substeps**. Smaller
   improvement than `OpmAligned`'s `18,015 → 16` (Legacy's Appleyard-damping retry ladder was
   already tolerating the old formula's residual, just at higher cost — `OpmAligned`'s strict
   CNV/MB gate could not), but a real, unconditional improvement — confirms the fix is a genuine
   physics-formula correction, not something that only matters under a specific Newton-loop
   flavor. Reported production numbers shift (`oil=3887.33 → 2900.00` at this snapshot) — expected
   and correct, not a regression: the old numbers included a manufactured water-fraction leak at
   the producer that this fix removes.
2. **`water-medium-6step` (`water-pressure 20x20x3 --steps 6 --dt 0.25`), a second,
   independently-discovered broken case.** Not part of the standard 6-command control matrix, but
   checked per the plan's own X3 item 4 ("a producer that sees breakthrough mid-run"). Baseline
   (flag off): steps 1-4 clean (`8/3/4/5` substeps), but **steps 5-6 exhibit the same
   plateau-replay-explosion pathology** as the heavy case (`accepts=3+5+1018`, then
   `accepts=1+5+2042` — `real_accepted_substeps` collapsing while `accepts` balloons via the same
   ledger-collapsing mechanism D4 diagnosed for `FIM-DIAG-003`). Reported `oil` **freezes** at
   `3560.89` identically across both stuck steps — itself evidence the run wasn't making genuine
   progress. With the fix: steps 5-6 resolve cleanly (`8`/`3` real substeps, `accepts=8+0+0`/
   `3+0+0`), and reported `oil` **continues climbing** (`3543.90 → 3578.05 → 3599.27`) instead of
   freezing. Steps 1-4 (pre-breakthrough, before the fix's mechanism is even exercised) are
   near-identical between the two runs (`oil` differs by `<0.1` at every step) — the fix changes
   nothing until the physics it corrects actually matters, exactly as expected.

**Stack-level Bundle N §5 promotion decision.** The original gate (heavy `≤35`-substep class +
fine-dt FOPT + control matrix + bounded cases not worse than Legacy) had TWO independent parts,
and this fix resolves only one of them cleanly:
- **Heavy-case substep class: PASSED, decisively** (`16 ≤ 35`, and `16` is also *better* than
  Legacy's own `52` — the fix doesn't just clear the bar, it makes `OpmAligned` the better choice
  on this specific case).
- **Bounded cases "not worse than Legacy": still open, unrelated to this fix.** Re-confirmed
  unchanged by `single_cell_producer_fraction` (bit-identical on `22x22x1`/`23x23x1` with the flag
  on or off): `OpmAligned` alone was already costlier than Legacy on the bounded cases *before*
  this fix (`docs/FIM_STATUS.md` "Bundle N" section, recorded at the time as "close on attempts,
  not yet better") and remains so — `20x20x3` `8→15`, `22x22x1` `4→24`, `23x23x1` `4→12`,
  `gas-rate 20x20x3` `2→459`. This is a pre-existing, separately-tracked characteristic of
  `OpmAligned`'s more conservative per-cell chopping / stricter CNV-MB acceptance vs Legacy's
  Appleyard-damping ladder — not something `FIM-BUNDLE-X` was ever scoped to fix, and this fix
  neither helps nor hurts it (structurally cannot, per X0's finding that the mechanism only fires
  near a pre-breakthrough producer's saturation-bound collision, which these bounded cases don't
  hit in a way that changes their substep count).

**Verdict, split into two independent decisions**:
1. **`single_cell_producer_fraction` itself: PROMOTABLE now, independent of the
   `OpmAligned`/`nested_well_solve` stack question.** It is a physics-fidelity bug fix (matches
   OPM's actual formula exactly), strictly improves every case tested under every flavor
   combination, and has zero observed regression. This is the kind of fix the "systemic steer"
   guidance favors (fix the OPM-inconsistent base, not another mechanism layered on top of it).
2. **The `OpmAligned`+`nested_well_solve` stack (the original Bundle N §5 question — should this
   *become the default*, replacing Legacy): still NOT closed.** The heavy-case blocker that
   stalled it across `FIM-BUNDLE-N`/`FIM-BUNDLE-W`/`FIM-DIAG-002`/`FIM-DIAG-003` is now removed,
   but the bounded-case cost tradeoff (never this bundle's target) remains as the standing
   obstacle to full stack promotion. That is a distinct, pre-existing question, appropriately
   left for its own future work rather than folded into this bundle's scope.

**Open product question, not a technical one: should `single_cell_producer_fraction` become
unconditional (delete the flag, always match OPM) rather than stay an opt-in dev flag?** It
changes reported production numbers on any FIM scenario where a producer's 3x3 neighborhood
differs from its own cell's saturation (i.e. once water is near but hasn't reached a producer) —
this is more physically correct, but is a default-behavior change for the public-facing FIM path
(FIM is currently dev-only per `docs/FIM_DEFERRED_BACKLOG.md`, which lowers the stakes, but
worth an explicit decision rather than a silent default flip). Deferred to the user.
