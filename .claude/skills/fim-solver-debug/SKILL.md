---
name: fim-solver-debug
description: Debug or improve FIM (fully implicit) solver convergence, timestep fragmentation, Newton failures, or retry ladders in the Rust core. Use for any work in src/lib/ressim/src/fim/ that changes solver behavior, and for reproducing/diagnosing convergence baselines with the wasm diagnostic runner.
---

# FIM Solver Debugging & Convergence Work

FIM is **dev-only** — public scenario runs use IMPES (`docs/FIM_DEFERRED_BACKLOG.md`). FIM convergence is the hardest, most history-laden area of this project. The graveyard of reverted experiments is large; the process below exists because ad-hoc tuning repeatedly produced false wins.

## Read history BEFORE proposing a fix

Many plausible levers were already tried and **reverted**. Check these, in order, before designing anything:

1. `docs/FIM_STATUS.md` — current state, locked baseline, open gaps.
2. `TODO.md` "Now" section — dated micro-experiment log (what was promoted vs reverted, with exact replay numbers).
3. `docs/FIM_CONVERGENCE_WORKLOG.md` — active hypotheses and traces.
4. `docs/FIM_CONVERGENCE_ARCHIVE_*.md`, `docs/FIM_HISTORY_2026-03.md` — older attempts.

Known-reverted lever classes (do not re-try without new evidence): widening Newton stagnation acceptance above tolerance; softer retry factors near tolerance (runtime clamps retry factors to ≤ 0.5 in `fim/timestep.rs`); post-cooldown hotspot regrowth caps; letting no-op accepts decay hotspot memory; accepted-site-aware carryover persistence (fixed 3-clean-step budget won); blanket per-row infinity-norm scaling of the linear system before the iterative CPR/ILU0 solve (`row_scaled_system` in `fim/linear/gmres_block_jacobi.rs` — regressed the heavy `12x12x3/dt=1` case 31→241 substeps; destroys physically-meaningful relative-magnitude information the preconditioner implicitly relies on — a narrower, equation-family-aware or quasi-IMPES-style weighting has not been tried).

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
6. Run FIM locked baseline + parity gates (`ressim-validation` skill) before committing.

## Reference target

OPM Flow solves comparable cases at ~2.5 Newton iterations/step with zero timestep cuts. Reference decks for side-by-side comparison live on branch `origin/fim-opm-continuation-plan` (`opm/reference-decks/`, `scripts/opm-ressim-compare.sh`). See the `opm-reference-pipeline` skill.
