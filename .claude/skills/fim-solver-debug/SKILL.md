---
name: fim-solver-debug
description: Debug or improve FIM (fully implicit) solver convergence, timestep fragmentation, Newton failures, or retry ladders in the Rust core. Use for any work in src/lib/ressim/src/fim/ that changes solver behavior, and for reproducing/diagnosing convergence baselines with the wasm diagnostic runner.
---

# FIM Solver Debugging & Convergence Work

FIM is **dev-only** — public scenario runs use IMPES (`docs/FIM_DEFERRED_BACKLOG.md`). FIM convergence is the hardest, most history-laden area of this project. The graveyard of reverted experiments is large; the process below exists because ad-hoc tuning repeatedly produced false wins.

## Read history BEFORE proposing a fix

Many plausible levers were already tried and **reverted**. Check these, in order, before designing anything:

1. `docs/FIM_STATUS.md` — current state, locked baseline, open gaps.
2. `docs/FIM_EXPERIMENT_REGISTRY.md` — fast searchable index of promoted, reverted, refuted, diagnostic, and open FIM levers. Search by mechanism, file, target case, and failure family. If an equivalent experiment is already listed, do **not** repeat it unless the row's `Retry only if` condition is satisfied.
3. `TODO.md` "Now" section — active task tracker only; if it contains dated micro-experiment notes, treat them as candidates to move into the registry/worklog.
4. `docs/FIM_CONVERGENCE_WORKLOG.md` — active hypotheses and traces.
5. `docs/FIM_CONVERGENCE_ARCHIVE_*.md`, `docs/FIM_HISTORY_2026-03.md` — older attempts.

Known-reverted lever classes (do not re-try without new evidence): widening Newton stagnation acceptance above tolerance, in either its original mid-loop form (`FIM-NEWTON-004`, gives up early with budget remaining) or a post-loop "accept the max-iteration result if it's close enough" form (`FIM-NEWTON-005` — tried specifically to reconcile Phase 10's loosened linear tolerance with Newton's final-iteration near-misses at `perf@1299`; live heavy-case run did not finish in 8+ minutes because the accepted under-converged well/perforation state compounded into subsequent substeps rather than resolving — accepting "close enough" at the Newton level is not free just because it's scoped to only the exhausted-budget case); softer retry factors near tolerance (runtime clamps retry factors to ≤ 0.5 in `fim/timestep.rs`); post-cooldown hotspot regrowth caps; letting no-op accepts decay hotspot memory; accepted-site-aware carryover persistence (fixed 3-clean-step budget won); blanket per-row infinity-norm scaling of the linear system before the iterative CPR/ILU0 solve (`row_scaled_system` in `fim/linear/gmres_block_jacobi.rs` — regressed the heavy `12x12x3/dt=1` case 31→241 substeps; destroys physically-meaningful relative-magnitude information the preconditioner implicitly relies on — a narrower, equation-family-aware or quasi-IMPES-style weighting has not been tried).

**`FIM-LINEAR-008` (OPM `cprw` recipe bundle — tolerance/budget/block-ILU0) is currently live but unresolved, not reverted.** Offline lab win was decisive; live heavy-case regression (`26→59` substeps) is real. Four reconciliation attempts tried with real measurement, not guesswork — two refuted, two promoted:
- `FIM-NEWTON-005` (refuted): a bounded post-loop near-converged-final-state acceptance. Live run did not finish in 8+ minutes — accepting the marginally under-converged state compounds forward instead of resolving. Do not re-attempt any acceptance-widening fix at this site.
- `FIM-LINEAR-009` (refuted): a per-family/per-block linear stopping criterion (`EquationScaling::family_peaks`, opt-in via `equation_scaling` param through `gmres_block_jacobi.rs`) built and offline-lab-tested on 35 real captured near-miss systems. Only 1/35 showed a real per-family overshoot; the hypothesis that a swamped global norm is the dominant cause is not supported. The infrastructure (`fim/scaling.rs`'s `EquationFamilyPeaks`, `family_ok` threading, `fim-capture-v2` format) is kept as validated but currently-inert.
- `FIM-LINEAR-010` (promoted): Schur-eliminate well-BHP/perforation-rate rows from the linear system every Newton iteration (`fim/linear/well_schur.rs`, `eliminate_wells` option), matching OPM's `StandardWellEquations`. Proven exact (unit test, `1e-9` match vs direct solve); offline-decisive (34/35→35/35 converged, mean linear iters 3.9→1.1). Alone, live heavy case regressed further (`59→160` substeps) — but the `perf@1299` oscillation trace was byte-for-byte unchanged before/after, proving elimination doesn't touch the nonlinear oscillation (it's an exact reformulation of the same linear system, same Newton direction either way). Kept live for its own correctness/efficiency/architecture-consistency value even though it alone didn't fix the target.
- `FIM-NEWTON-006` (promoted): widened `PerFamilyNorms`/`detect_oscillation` (Phase 7's OPM-ported oscillation detector, `fim/newton.rs`) to include `well_constraint`/`perforation_flow` — previously excluded pending evidence; that evidence is `FIM-LINEAR-010`'s finding plus a direct measurement showing `perf@1299`'s residual matches OPM's own oscillation test (`d1≈0, d2≈0.6`) while being invisible to the detector purely due to family-scope exclusion. Combined with `FIM-LINEAR-010`: heavy case `160→62` substeps, `retry_dom` shifted away from `perf@1299` entirely (confirming the target oscillation is fixed) — but the case is still not back to the pre-Phase-10 `26` baseline; a new, unrelated failure (`water@387`, "invalid bounded Appleyard candidate" — a hard damping failure, not an oscillation) is now dominant. That is the next open item, not a continuation of this fix.

## Architecture orientation (as of 2026-07)

- `fim/newton.rs` (~5.2k lines) — damped Newton, Appleyard-style damping, hotspot streak tracking, direct-backend bypass logic.
- `fim/timestep.rs` (~2.5k lines) — outer step / substep / retry ladder controller, hotspot-repeat cooldown memory, plateau-replay bookkeeping, gas outer-step carryover.
- `fim/assembly_ad.rs` + `fim/ad.rs` + `*_ad.rs` — **the live assembly path** (AD-based). `fim/assembly.rs` is the legacy assembler, kept as the bit-parity reference. The alias is at the top of `timestep.rs`/`newton.rs`: `use crate::fim::assembly_ad::assemble_fim_system_ad as assemble_fim_system;`
- Parity gates live in `fim/assembly_ad.rs` tests (bit-identical residual + Jacobian occupancy vs legacy). Run them after touching assembly, properties, flux, flash, or wells:
  ```bash
  cargo test --manifest-path src/lib/ressim/Cargo.toml assembly_ad -- --nocapture
  ```
- Linear solvers: `solvers/faer_sparse_lu.rs` (direct), `solvers/bicgstab.rs`; FIM-specific wiring under `fim/linear/`.

## The canonical diagnostic loop

Always rebuild wasm first — the runner executes the same wasm the browser uses:

```bash
bash scripts/build-wasm.sh
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json
```

- Presets: `water-pressure`, `water-rate`, `gas-pressure`, `gas-rate`, `sweep-areal` (`--list`).
- `--diagnostic` granularity: `summary` → `outer` → `step` (per-Newton and retry traces).
- Checkpoints exist (`--checkpoint-*`) but **isolated checkpoint replays do not preserve cross-step carryover state** (e.g. gas outer-step trial caps). Validate cross-step behavior with full sequential `--diagnostic step` runs only.

### Reading the summary line

- `substeps=N` — total accepted-substep history (may include replayed bookkeeping).
- `accepts=A+B+C` — real Newton-solved accepts + replayed cooldown-held accepts + replayed hotspot-plateau accepts. **Optimize the real accepts and retries, not the replayed ledger.**
- `retries=X/Y/Z` — linear-bad / nonlinear-bad / mixed.
- `retry_dom=nonlinear-bad:water@1020` — dominant failure family, phase, flat cell index.
- `dt=[min,max]`, `growth=<limiter>` (e.g. `hotspot-repeat`, `newton-iters`, `max-growth`), `hotspot_newton_caps=N`.

## The bounded control matrix (non-regression gates)

Any solver-behavior change must leave these unchanged unless the change is explicitly about them. Exact commands matter — a past false regression came from running controls with the wrong `--dt`:

```bash
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 22x22x1 --steps 1 --dt 0.25 --diagnostic summary --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 --diagnostic summary --no-json
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate     --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate     --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic outer   --no-json
# heavy target (minutes-long historically; ~2s after plateau-replay optimizations):
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json
```

Do **not** trust expected counts written in old docs — re-derive them (baseline discipline below).

## Baseline & promotion discipline (non-negotiable)

From project instructions (`.github/copilot-instructions.md`):

1. Before experimenting: run the control matrix on a **clean committed tree**, record commit hash, exact commands, and verbatim summary lines.
2. Results from dirty trees or partially reverted states are **provisional** — say so explicitly.
3. To promote a change: rerun the matrix on the final post-cleanup tree, not an intermediate experiment commit.
4. When replacing a documented baseline, state which baseline was superseded and why.
5. Log the experiment (promoted or reverted, with numbers) in `TODO.md` / `docs/FIM_CONVERGENCE_WORKLOG.md`. Reverted results are valuable — record them so the next session doesn't retry the same lever.

## Working method for a convergence slice

1. Reproduce the target shelf/ladder on a clean tree; identify the *real* blocker (`accepts` split + `--diagnostic step` trace), not the bookkeeping.
2. Form one bounded hypothesis touching one controller mechanism. Prefer narrow guards (failure family + iteration count + regime + dt floor) over global tuning.
3. Implement with a focused Rust unit test for the new mechanism (see existing `gas_outer_step_trial_cap` tests in `fim/timestep.rs` for the pattern).
4. Rebuild wasm, rerun target + full control matrix.
5. Green and improved → promote with recorded numbers. Any control moved → revert and record the negative result.
6. Add or update the corresponding row in `docs/FIM_EXPERIMENT_REGISTRY.md` before committing, including the verdict and the condition for retrying if it failed.
7. Run FIM locked baseline + parity gates (`ressim-validation` skill) before committing.

## Reference target

OPM Flow solves comparable cases at ~2.5 Newton iterations/step with zero timestep cuts. Reference decks for side-by-side comparison live on branch `origin/fim-opm-continuation-plan` (`opm/reference-decks/`, `scripts/opm-ressim-compare.sh`). See the `opm-reference-pipeline` skill.
