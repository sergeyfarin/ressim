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
