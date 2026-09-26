---
name: ressim-validation
description: Choose and run the correct ResSim validation gates for a change (frontend, Rust engine, IMPES, FIM). Use before claiming any change is done, when tests are failing, or when unsure which test suite covers a change. Critical - full cargo test is NOT a valid gate here.
---

# ResSim Validation Workflow

ResSim has several validation surfaces with different costs and different owners. Running the wrong one either wastes an hour or (worse) passes while missing the real regression.

## Hard rules

1. **Never use full `cargo test` as a pass/fail gate.** FIM/SPE1 diagnostic tests can hang or dominate runtime (`docs/FIM_DEFERRED_BACKLOG.md`). Use the targeted buckets below.
2. **Never change benchmark tolerances** (Buckley-Leverett tests, parity gates) without explicit written justification in the same commit.
3. A change is not "done" until its GitHub Issue is updated or closed with the implementing commit
   or pull request, and the relevant gate below is green. If the work has no issue and discovers
   actionable follow-up, open one rather than adding a checkbox to `TODO.md`.
4. **Separate code validation from hypothesis validation.** Green unit/parity/control tests prove
   their stated contracts only. They do not establish OPM parity or validate an experimental
   verdict unless the measured observable is itself an OPM or backend-neutral oracle.
5. A cross-backend FIM result is not a valid pass/fail gate until both reports contain comparable
   initial/RHS norm, final full-system residual norm, reduction, and finite-solution status. If
   one path reports `n/a`, reduced-system-only data, or a backend-specific failure payload, record
   the experiment as `INCONCLUSIVE` and repair the measurement contract first.

## Two tiers

The vitest suite is not uniform. 14 files under `src/lib/catalog/scenarios/` drive full WASM
simulations and dominate its cost; the other ~54 files are quick. Running the scenario
simulations after a chart or store edit buys nothing — a UI change cannot move a simulation
trajectory.

| Tier | Command | Cost | When |
|---|---|---|---|
| 1 — inner loop | `pnpm run validate` | ~60 s | Every iteration on Svelte/TS. Excludes the scenario sims (`test:fast`). |
| 2 — pre-commit | `pnpm run validate:product` | ~4 min | Before commit/push, and for any catalog, worker-payload or Rust change. Full vitest + IMPES bucket. |
| 3 — engine | `pnpm run validate:full` | ~5 min | Rust shared/solver changes. Tier 2 + `validate-solver-coverage.sh all`. |

Re-measured 2026-09-15 on `6be6d08`. The scenario tier is **~116 s**, not the ~440 s recorded
through 2026-08 — the FIM convergence work of 2026-07 (`efde1f4`, Bundle X, the WATER series)
removed the substep fragmentation that dominated it. The full vitest suite is **~115 s**.

Worth knowing before optimizing: the slowest single test is `dep_pss` at ~72 s CPU, and that
scenario is `fimEnabled: false`. It runs 7 variants x 400 report steps at ~25 ms/step — that is
volume, not solver convergence. Of the eight slowest scenario tests only `wf_capillary` and
`dep_gas_pz` are FIM at all.

`pnpm test` remains the **full** suite. The fast subset is always spelled explicitly
(`pnpm run test:fast`), so a run that skipped the scenario sims is visible at the call site
rather than hidden behind a default.

## Decision table — what changed → what to run

| Change touches | Run |
|---|---|
| Svelte/TS only (UI, charts, stores, workers) — while iterating | `pnpm run validate` (tier 1) |
| Same, before committing | `pnpm run validate:product` (tier 2) |
| Anything shipped to users (frontend + product Rust path) | `pnpm run validate:product` |
| Rust shared code (`relperm.rs`, `pvt.rs`, `mobility.rs`, `capillary.rs`, `well*.rs`, `step.rs`, `reporting.rs`, `frontend.rs`) | `bash scripts/validate-solver-coverage.sh all` + BL benchmarks |
| IMPES only (`src/lib/ressim/src/impes/`) | `bash scripts/validate-solver-coverage.sh impes` then `shared` |
| FIM only (`src/lib/ressim/src/fim/`) | FIM locked baseline (below) + `bash scripts/validate-solver-coverage.sh fim` then `shared`; for solver *behavior* changes also run the wasm control matrix (see `fim-solver-debug` skill) |
| FIM linear-report/oracle code | Focused report-contract tests for every affected backend and well-Schur wrapping + targeted replay of the captured system; then the normal FIM-only gates above |
| Analytical modules (`src/lib/analytical/`) | `pnpm test` (analytical + contract tests) |
| Scenario catalog (`src/lib/catalog/`) | `pnpm test` then `pnpm run typecheck` |
| WASM API surface (`frontend.rs`, `lib.rs`, worker payloads) | `bash scripts/build-wasm.sh` + `pnpm run validate:product` |
| Physics, PVT, wells, either timestep controller, or the small-system linear route — anything that can move an answer | the rows above **plus** `bash scripts/validate-cross-solver.sh` (every solver vs OPM Flow, scorecard ratchet; needs `flow`) **plus** `bash scripts/benchmarks.sh check` (every number on `docs/BENCHMARKS.md`, ~2–3 min) |
| Engine payload boundary (`api.rs`, `frontend.rs`) or `crates/ressim-py` | the rows above **plus** `bash scripts/validate-native-binding.sh` (engine-math lint, then native vs browser bindings on a case matrix, bit-identical since #62) |
| Compositional engine (`src/lib/ressim/src/compositional/`) | `bash scripts/validate-compositional.sh thermo`; `native` or `all` when OPM headers / `flowexp_comp` are available |
| Styling, Tailwind config, `index.html`, Vite/build config | `pnpm run build` + `pnpm run test:deployed` against `pnpm run preview` — the only check that catches an unstyled page (a Tailwind content-glob miss raises no error anywhere) |

## Command reference

Frontend (from repo root, always pnpm — never npm):

```bash
pnpm run typecheck        # tsc --noEmit                          ~11 s
pnpm run lint             # eslint, zero warnings allowed         ~15 s
pnpm run check:cycles     # runtime import-cycle gate             ~3 s
pnpm run test:fast        # vitest minus catalog/scenarios/**     ~16 s
pnpm run test:scenarios   # only the WASM scenario simulations    ~116 s
pnpm test                 # vitest run — the full suite           ~115 s
pnpm run validate         # typecheck + lint + cycles + test:fast + build
pnpm run validate:product # same but full vitest + Rust IMPES bucket
pnpm run validate:full    # validate:product + solver coverage `all`
```

Timings are wall-clock on a single-core box; multi-core machines and CI recover some of the
scenario tier through vitest file parallelism.

Rust buckets (grouped, curated, safe to run — no hanging tests):

```bash
bash scripts/validate-solver-coverage.sh shared   # both-solver parity contracts   17 gates,  ~11 s
bash scripts/validate-solver-coverage.sh impes    # IMPES-owned tests               4 gates,   ~3 s
bash scripts/validate-solver-coverage.sh fim      # FIM-owned fast tests           17 gates,  ~58 s
bash scripts/validate-solver-coverage.sh all      #                                38 gates,  ~72 s
```

Times are warm-cache wall clock on a single-core box, excluding the initial `cargo test --no-run`
compile. They are still small enough that there is no reason to run a narrower bucket than the
decision table calls for.

The FIM bucket roughly doubled during the F0–F8 repair series, from 14 gates / ~26 s to 17 / ~58 s.
Three filters were added because their modules were in no bucket at all (`fim::linear::`,
`fim::flow_resv::`, and the `fim_repair_` lifecycle group), and one of them dominates the cost:
`fim_repair_` runs ~31 s, almost entirely
`fim_repair_multi_completion_gravity_stays_physical_on_both_solvers`, which steps a 30x1x20
gravity section for 12 days across four solver/completion configurations. That is the price of
covering the multi-completion geometry on both backends (#10); if the bucket ever needs to get
back under ~30 s, that test is the thing to shorten, not the filters to drop.

The script builds the test target first (a compile break fails as a build error before
any bucket runs) and prints a `gate ok: '<filter>' ran N test(s)` line per filter. A
filter that matches **no** tests is a hard failure — `cargo test <filter>` exits 0 when
nothing matches, so without that check a renamed or deleted test would silently turn its
gate line into a no-op that still reported success. If you rename a test, update the
filter in the script.

A filter that matches **only `#[ignore]`d tests** is also a hard failure (#13): the count
is of *passed* tests, not matched ones, so a release-only replay cannot masquerade as a
gate that ran. `gate ok` lines report ignored tests separately when both are present.

FIM locked day-to-day baseline (exact commands from `docs/FIM_STATUS.md`):

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas -- --nocapture
```

Black-oil acceptance replays (`#[ignore]`d — run explicitly, always `--release`; criteria and
recorded baselines in `docs/BLACK_OIL_VALIDATION.md`):

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml spe1_full_horizon_matches_published_reference -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml spe1_areal_refinement_reference_error_replay -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml physics_depletion_grid_convergence_fim -- --ignored --nocapture
```

The fast counterparts (`spe1_first_year_matches_published_reference`,
`physics_depletion_grid_convergence_impes`) already run inside the `fim` / `impes` buckets above.

Buckley-Leverett physics benchmarks (validated tolerances — the core scientific gate):

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml benchmark_buckley -- --nocapture
```

Targeted single test (preferred while iterating):

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml <test_name_substring> -- --nocapture
```

WASM rebuild (required before any `scripts/fim-wasm-diagnostic.mjs` run and before `pnpm run dev` picks up Rust changes):

```bash
bash scripts/build-wasm.sh
```

Cross-solver gate — every ResSim solver against OPM Flow and against each other (~20 s with
Flow cached; skips with a note when `flow` is not installed):

```bash
bash scripts/validate-cross-solver.sh                   # check against the committed scorecard
bash scripts/validate-cross-solver.sh --update          # re-baseline: on a COMMITTED tree only
bash scripts/validate-cross-solver.sh --markdown        # README-ready tables, then check
bash scripts/validate-cross-solver.sh --refine 0.025 --case ow-2d-12x12   # time-refined referee
```

It runs FIM (sparse and dense LU) and IMPES natively on the eight small-direct decks
(`opm/reference-decks/small-direct/`), runs Flow on the same decks, and fails when an accuracy
metric against Flow (worst and final cell Δp/ΔSw/ΔSg, cumulatives) or a work metric (substeps,
Newton) regresses past its band, when a run raises a new solver warning, or when sparse and dense
stop agreeing. It prints improvements past the band too: re-baseline them deliberately with
`--update` on a committed tree and commit the scorecard with the change that earned it, so the
scorecard's provenance names that commit. **Do not hand-copy small-direct numbers into a doc**;
use `--markdown`.

Benchmark records — `docs/BENCHMARKS.md` is generated, never typed (#54):

```bash
bash scripts/benchmarks.sh check                 # measure every section, compare with docs/benchmarks/benchmarks.json
bash scripts/benchmarks.sh check --tier fast     # without Flow / flowexp_comp / JutulDarcy (what CI has)
bash scripts/benchmarks.sh update [--only spe1]  # re-record and re-render: on a COMMITTED tree only
bash scripts/benchmarks.sh render --check        # page and README summary in sync with records (no runs)
```

The full tier runs JutulDarcy (`tools/jutul`, needs `juliaup add 1.12`; ~4 min of it is Julia
compiling on first use of each model type) as a second simulator on the same decks.

`check` fails when a banded error grows by more than a tenth of its band or leaves it, or when a
recorded measurement disappears (a renamed test, a broken producer). Every other change is listed:
record it with `update` in a follow-up commit on the committed tree, so the section's stamp names
the commit that moved it. To add a benchmark, record it from its test with
`tests/bench_record.rs` (or append to `$RESSIM_BENCH_OUT/records.jsonl` from a script), then give
its section a renderer in `tools/benchmarks/benchmarks.py`.

The page's **Signals** and **Coverage** blocks are what should move priorities: same-model gaps to
a reference that solves the same discrete model, criteria near their band, loose bands, and
scenarios with no numerical reference. `python3 tools/benchmarks/benchmarks.py signals` prints them
plus stale sections (`--strict` exits 1 when anything needs attention). A gap stops being flagged
only through an entry in `docs/benchmarks/explained.json`: `tracked` with an open issue, or
`explained` with the mechanism written where `ref` points. The Coverage grid reads the scenario
catalog, so adding a scenario, a Flow artifact case or a refinement dimension needs
`benchmarks.sh render` (CI's `render --check` fails otherwise).

Agreement with Flow at the same time step is not accuracy: FIM and Flow share an implicit scheme
and its time-step error. When FIM and IMPES disagree, `--refine` is the referee (see the
small-direct README's "Referee" section, where refinement shows IMPES converging to Flow).

## Interpreting failures

- **Vitest contract failures after a catalog change** usually mean the scenario metadata violated a real contract (e.g. a sensitivity variant claims `affectsAnalytical: true` but doesn't perturb the analytical result). Fix the metadata, not the test — the test is the spec.
- **`*_on_both_solvers` test failures** mean IMPES and FIM public behavior diverged. Do not weaken the contract; find which solver changed.
- **Bit-parity gate failures in `fim/assembly_ad.rs`** mean the AD assembly and the legacy assembly no longer agree. See the `engine-physics-change` skill — physics helpers often have both a legacy and an AD implementation that must be changed together.
- If a test fails on a clean tree before your change, record that first (it is a pre-existing
  failure, not yours) and open or update the corresponding GitHub Issue.
- AD/legacy/finite-difference agreement means ResSim differentiated its own residual consistently;
  it does **not** prove that the residual, primary variables, bounds, or well formulation match
  OPM. Require a sourced OPM semantic or trajectory comparison for an OPM-parity claim.
- A direct solver is not automatically a truth oracle. Check `||J dx - rhs|| / ||rhs||` on the
  same full system and compare the returned correction before interpreting different
  `converged` flags.

## CI reality check

`.github/workflows/pr-tests.yml` is the source of truth. As of 2026-09-25 it runs, in order:
`pnpm install`, an explicit `scripts/build-wasm.sh`, lint, `check:cycles`, typecheck,
`validate-solver-coverage.sh all`, the Buckley-Leverett benchmarks,
`validate-compositional.sh thermo`, `validate-native-binding.sh`,
`benchmarks.sh check --tier fast` (benchmark records and the generated page), the OPM artifact pipeline
pytest (`tools/opm_flow`), the full vitest suite via `pnpm run test:coverage`, `pnpm run build`,
and a Playwright smoke test of the built bundle (`test:deployed` against `vite preview`).

Not in PR CI — run them yourself when a change touches what they measure:

- The `#[ignore]`d release replays (`spe1_full_horizon_matches_published_reference`,
  `spe1_areal_refinement_reference_error_replay`, `physics_depletion_grid_convergence_fim`).
- The wasm control matrix (`scripts/fim-wasm-diagnostic.mjs`); see the `fim-solver-debug` skill.
- `scripts/validate-cross-solver.sh`: CI has no OPM Flow. It is the gate for any change that can
  move an answer.
- `validate-compositional.sh` modes beyond `thermo` (they need OPM headers or `flowexp_comp`).

The explicit WASM build matters: `src/lib/ressim/pkg/` is **generated, not committed** (#30), so
that step is what produces the bindings every frontend simulation test loads. It is belt and
braces — `pretypecheck` / `pretest*` / `prebuild` in `package.json` run `scripts/build-wasm.sh`
too — but keeping it explicit puts a Rust build failure in its own CI step.

Locally the same hooks mean you never have to remember the rebuild: `pnpm run typecheck`, any
`pnpm run test*`, `pnpm run dev` and `pnpm run build` all build the bindings first. The script
skips the work (~25 s of wasm-bindgen + wasm-opt that `wasm-pack` re-runs even when cargo has
nothing to recompile) when `pkg/simulator_bg.wasm` is already newer than `src/lib/ressim/src/`,
both `Cargo.toml`s, `Cargo.lock` and the script itself. Force a rebuild with
`RESSIM_FORCE_WASM_BUILD=1 bash scripts/build-wasm.sh`.

Before 2026-09-14 (#13) CI ran only the IMPES bucket, no lint, no cycle check, no build and no
Buckley-Leverett gate. If you are reading an older run, do not assume it covered FIM.
