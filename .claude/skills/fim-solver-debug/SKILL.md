---
name: fim-solver-debug
description: Debug or improve FIM (fully implicit) solver convergence, timestep fragmentation, Newton failures, or retry ladders in the Rust core. Use for any work in src/lib/ressim/src/fim/ that changes solver behavior, and for reproducing/diagnosing convergence baselines with the wasm diagnostic runner.
---

# FIM Solver Debugging & Convergence Work

FIM ships in selected public scenarios; its convergence research track remains developer-driven
(`docs/FIM_DEFERRED_BACKLOG.md`). FIM convergence is the hardest, most history-laden area of this
project. The graveyard of reverted experiments is large; the process below exists because ad-hoc
tuning repeatedly produced false wins.

## Oracle validity comes before a verdict

Every convergence experiment has two independently fallible parts: the solver mechanism under
test and the measurement/oracle used to judge it. Validate the oracle before interpreting a
negative result.

For live/direct or backend/backend comparisons, record the same full-system quantities on both
paths:

- `rhs_norm` (the initial linear residual norm);
- returned `solution` finite status;
- `final_residual_norm = ||rhs - J dx||` on the **full** system;
- `reduction = final_residual_norm / max(rhs_norm, 1e-30)`;
- reservoir-row and well-row residual partitions after any Schur recovery;
- correction-vector difference when two backends are claimed to agree or disagree.

Do not derive a universal quantity such as reduction from an optional backend-specific failure
payload. A wrapper that changes `converged` after checking a recovered full system must also
publish full-system norms; forwarding a reduced solver's absent diagnostics is not sufficient.
If any required quantity is absent or incomparable, stop and label the result `INCONCLUSIVE —
oracle defect`. Repair the report contract or add a diagnostic replay before changing physics,
Newton policy, wells, or timestep control.

Verdict meanings for convergence work:

- `PROMOTED`: the scoped mechanism passed its declared target, controls, physics, and clean-tree
  replay gates.
- `REFUTED`: a valid oracle directly measured the hypothesis as false or irrelevant.
- `REVERTED`: implementation removed because a valid control/promotion gate regressed; this does
  not necessarily refute the underlying mechanism.
- `INCONCLUSIVE`: the probe was incomplete, the oracle/reporting was invalid, or another defect
  prevented measurement of the stated hypothesis.
- `DIAGNOSTIC`: read-only or inert instrumentation result.

Never collapse `REVERTED` or `INCONCLUSIVE` into `REFUTED`. In particular, a promising live
trajectory is not refuted by a direct path that aborts with `reduction=n/a`; first compare the
actual corrections and full-system residual reductions.

## OPM parity requires a coherent semantic scope

Before implementing an OPM-alignment probe, write a dependency table covering the applicable
source lifecycle: stored primary state, update/chop, accumulation state, endpoint-clipped
properties, phase presence and `adaptPrimaryVariables`, well primary variables/equations, and
linear acceptance. Mark each item `matched`, `intentionally held constant`, or `missing`.

"One mechanism per commit" means one falsifiable causal question, not that an intrinsically
coupled OPM mechanism must be split into misleading fragments. A narrow probe may prove a local
effect, but it cannot refute the complete OPM mechanism when required lifecycle pieces are still
missing. AD/legacy/FD parity proves internal differentiation consistency only; it is not an OPM
equation or trajectory oracle.

## Read history BEFORE proposing a fix

Many plausible levers were already tried and **reverted**. Check these, in order, before designing anything:

1. `docs/FIM_STATUS.md` — current state, baselines with replay commands, open gaps. Then `docs/FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md` and the active row in the registry below.
2. `docs/FIM_EXPERIMENT_REGISTRY.md` — fast searchable index of promoted, reverted, refuted, diagnostic, and open FIM levers. **Search by mechanism name and by file, not just by target case** — the `FIM-NEWTON-007`→`FIM-DAMP-004` episode cost 3 live-test cycles because the "loosen the inflection chop" axis was searched by case, not by mechanism. If an equivalent experiment is already listed, do **not** repeat it unless the row's `Retry only if` condition is satisfied.
3. `docs/FIM_OPM_ALIGNMENT_STRATEGY_2026-04-26.md` + `.archive/docs/FIM_OPM_GAP_ANALYSIS_SPE1.md` — the standing 95%-track-OPM policy, Bundle A/B/C sequencing, and the FIM-vs-OPM gap decomposition with current triage. Any proposed change should be locatable on this map.
4. The active FIM GitHub Issues linked from `ROADMAP.md` — scope and acceptance criteria only;
   experiment detail belongs in the registry/worklog.
5. `docs/FIM_CONVERGENCE_WORKLOG.md` — active hypotheses and traces (Phase 9 onward — component-isolation lab, Phase 10's OPM `cprw` bundle, Phase 11's well-Schur-elimination/OSC-DETECT work, `FIM-DAMP-004`).
6. `.archive/docs/FIM_CONVERGENCE_ARCHIVE_2026-04-08_to_2026-07-03.md` — water/gas shelf investigations, Phase 5 AD-assembler cutover, Phase 6, Phase 7 (all 5 sub-phases), Phase 8, Hypothesis C. `.archive/docs/FIM_CONVERGENCE_ARCHIVE_2026-03_to_2026-04-06.md`, `.archive/docs/FIM_HISTORY_2026-03.md` — older still.

### Levers already tried — the checklist

One line each. The full write-up of every entry (the numbers, why it failed, what is kept in the
tree) is in `lever-history.md` beside this file. **Read that file before touching Newton
acceptance, the retry ladder, hotspot memory, linear-system scaling, well elimination, the
oscillation detector or the inflection chop.** The IDs are the registry's and are never
renumbered.

Reverted lever classes — do not re-try without new evidence:

- Widening Newton stagnation acceptance above tolerance: mid-loop (`FIM-NEWTON-004`) or
  post-loop (`FIM-NEWTON-005`).
- Softer retry factors near tolerance.
- Post-cooldown hotspot regrowth caps.
- Letting no-op accepts decay hotspot memory.
- Accepted-site-aware carryover persistence.
- Blanket per-row infinity-norm scaling of the linear system before the CPR/ILU0 solve.

The `FIM-LINEAR-008` bundle (OPM `cprw` recipe: tolerance, budget, block-ILU0) and its six
reconciliation attempts:

- `FIM-NEWTON-005` — refuted: post-loop near-converged acceptance.
- `FIM-LINEAR-009` — refuted as the primary cause: per-family linear stopping criterion. The
  infrastructure is kept, inert.
- `FIM-LINEAR-010` — promoted: Schur-eliminate the well rows every Newton iteration.
- `FIM-NEWTON-006` — promoted: the oscillation detector covers the well families.
- `FIM-NEWTON-007` — refuted: relaxing the fw-inflection chop's degenerate case.
- `FIM-DAMP-004` — promoted: inflection chop `k = 1.25`, found by re-sweeping `k`.

The rule the last two taught: search the registry for the mechanism by name before live-testing
a fix, not only for the target case.

## Architecture orientation

- `fim/newton.rs` (~3.4k lines) — damped Newton, Appleyard-style damping, hotspot streak tracking, direct-backend bypass logic.
- `fim/timestep.rs` (~4.3k lines) — outer step / substep / retry ladder controller, hotspot-repeat cooldown memory, plateau-replay bookkeeping, gas outer-step carryover.
- `fim/assembly_ad.rs` + `*_ad.rs`, on the dual-number type in `ad.rs` at the crate root — **the live assembly path** (AD-based). `fim/assembly.rs` is the legacy assembler, kept as the bit-parity reference. The alias is at the top of `timestep.rs`/`newton.rs`: `use crate::fim::assembly_ad::assemble_fim_system_ad as assemble_fim_system;`
- Parity gates live in `fim/assembly_ad.rs` tests (bit-identical residual + Jacobian occupancy vs legacy). Run them after touching assembly, properties, flux, flash, or wells:
  ```bash
  cargo test --manifest-path src/lib/ressim/Cargo.toml assembly_ad -- --nocapture
  ```
- Linear solvers: `solvers/faer_sparse_lu.rs` (direct), `solvers/bicgstab.rs`; FIM-specific wiring under `fim/linear/`.
- **Linear routing is the same on every target.** Small systems (≤ 512 rows) try sparse LU, then
  dense LU as a backup, then iterative CPR; `setFimDirectBackend` swaps the direct order. Do not
  reintroduce a `cfg(target_arch)` into solver routing. Native and wasm FIM agree bit for bit on
  the parity matrix since #62 routed engine math through `crate::math` (the `libm` crate on every
  target; glibc's `pow` is off by an ulp on some 257-knot relperm knots, and FIM turned that into
  1.1e-4 bar), and to ~1e-13 since FIM-DIRECT-001; before it they substepped differently (18 vs 4) because
  roundoff of one sign in an inactive unknown made the Jacobian singular. A direct solve never
  returns exactly 0 for an unknown that should not move, so **never give a primary a slope that
  depends on which side of a clamp its roundoff lands**
  (`docs/FIM_CROSS_TARGET_DIVERGENCE_2026-09-22.md` §9).
- The older convergence baselines in `docs/FIM_STATUS.md` are **wasm** measurements on the former
  dense route; re-measure before citing one against a native count.

## The canonical diagnostic loop

Always rebuild wasm first — the runner executes the same wasm the browser uses:

```bash
bash scripts/build-wasm.sh
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json
```

- Presets: `water-pressure`, `water-rate`, `gas-pressure`, `gas-rate`, `sweep-areal`, `wf-capillary`. `--list` prints the current set.
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

### The bounded matrix is not enough on its own

Every command above runs **1-6 report steps**, which is short enough to hide substep
fragmentation: a case can look clean at `--steps 1` and still fragment badly once the controller
has a trajectory to accumulate on. Pair the bounded matrix with a long horizon before concluding
anything about convergence:

```bash
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 20 --dt 1    --diagnostic outer --no-json
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate       --grid 10x10x3 --steps 24 --dt 0.25 --diagnostic outer --no-json
```

The quantity to read is **substeps / report steps**. Measured 2026-09-15 on `6be6d08`:

| case | steps | substeps | ratio | retries l/n/o | lin+pc share |
|---|---|---|---|---|---|
| water-pressure 20x20x3 dt .25 | 20 | 20 | 1.00 | 0/0/0 | 54% |
| water-pressure 23x23x1 dt .25 | 20 | 22 | 1.10 | 0/1/0 | 50% |
| gas-rate 10x10x3 dt .25 | 24 | 27 | 1.12 | 0/1/0 | — |
| water-pressure 12x12x3 dt 1 | 20 | 23 | 1.15 | 0/0/0 | — |
| water-pressure 22x22x1 dt .25 | 20 | 23 | 1.15 | 0/1/0 | 50% |

Ratios of 1.00-1.15 are OPM-class, and **no measured case stalls**. Linear solve plus
preconditioner build is 50-54% of FIM wall-clock, which is the expected profile for a
fully-implicit simulator rather than something to fix. Before proposing convergence work, re-run
the table above — if it still reads like this, the work is not justified by measurement. Full
evidence: [issue #23](https://github.com/sergeyfarin/ressim/issues/23), `docs/FIM_STATUS.md`
2026-09-15 banner.

## Baseline & promotion discipline (non-negotiable)

From the evidence rules in `AGENTS.md`:

1. Before experimenting: run the control matrix on a **clean committed tree**, record commit hash, exact commands, and verbatim summary lines.
2. Results from dirty trees or partially reverted states are **provisional** — say so explicitly.
3. To promote a change: rerun the matrix on the final post-cleanup tree, not an intermediate experiment commit.
4. When replacing a documented baseline, state which baseline was superseded and why.
5. Log the experiment (promoted or reverted, with numbers) in
   `docs/FIM_EXPERIMENT_REGISTRY.md` and `docs/FIM_CONVERGENCE_WORKLOG.md`; update the GitHub Issue
   with the verdict and links. Reverted results are valuable—record them so the next session does
   not retry the same lever.

## Working method for a convergence slice

1. Reproduce the target shelf/ladder on a clean tree; identify the *real* blocker (`accepts` split + `--diagnostic step` trace), not the bookkeeping.
2. State separately: hypothesis under test, oracle, expected confirming observation, expected
   refuting observation, and named coupled mechanisms held constant or missing.
3. Validate the oracle on the exact decision point. For backend comparisons, require the
   backend-neutral linear quantities above before allowing a physics verdict.
4. Form one bounded causal slice. Prefer narrow guards (failure family + iteration count + regime
   + dt floor) over global tuning, but keep source-required coupled semantics together and list
   why they cannot be evaluated independently.
5. Implement with a focused Rust unit test for the new mechanism (see existing
   `gas_outer_step_trial_cap` tests in `fim/timestep.rs` for the pattern).
6. Rebuild wasm, rerun target + full control matrix.
7. Classify the outcome using the verdict definitions above. A moved control can require a
   revert, but call the hypothesis refuted only if the measurement actually isolates it.
8. Add or update the corresponding row in `docs/FIM_EXPERIMENT_REGISTRY.md` before committing,
   including the verdict, oracle validity, omitted coupled semantics, and retry condition.
9. Run FIM locked baseline + parity gates (`ressim-validation` skill) before committing.

## Reference target

OPM Flow solves comparable cases at ~2.5 Newton iterations/step with zero timestep cuts. Reference decks for side-by-side comparison are in `opm/reference-decks/` with `scripts/opm-ressim-compare.sh`; the small-direct scorecard is `scripts/validate-cross-solver.sh`. See the `opm-reference-pipeline` skill.
