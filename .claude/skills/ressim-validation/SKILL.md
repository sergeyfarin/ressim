---
name: ressim-validation
description: Choose and run the correct ResSim validation gates for a change (frontend, Rust engine, IMPES, FIM). Use before claiming any change is done, when tests are failing, or when unsure which test suite covers a change. Critical - full cargo test is NOT a valid gate here.
---

# ResSim Validation Workflow

ResSim has several validation surfaces with different costs and different owners. Running the wrong one either wastes an hour or (worse) passes while missing the real regression.

## Hard rules

1. **Never use full `cargo test` as a pass/fail gate.** FIM/SPE1 diagnostic tests can hang or dominate runtime (`docs/FIM_DEFERRED_BACKLOG.md`). Use the targeted buckets below.
2. **Never change benchmark tolerances** (Buckley-Leverett tests, parity gates) without explicit written justification in the same commit.
3. A change is not "done" until `TODO.md` is updated (project TODO discipline) and the relevant gate below is green.
4. **Separate code validation from hypothesis validation.** Green unit/parity/control tests prove
   their stated contracts only. They do not establish OPM parity or validate an experimental
   verdict unless the measured observable is itself an OPM or backend-neutral oracle.
5. A cross-backend FIM result is not a valid pass/fail gate until both reports contain comparable
   initial/RHS norm, final full-system residual norm, reduction, and finite-solution status. If
   one path reports `n/a`, reduced-system-only data, or a backend-specific failure payload, record
   the experiment as `INCONCLUSIVE` and repair the measurement contract first.

## Decision table — what changed → what to run

| Change touches | Run |
|---|---|
| Svelte/TS only (UI, charts, catalog, stores, workers) | `pnpm run validate` |
| Anything shipped to users (frontend + product Rust path) | `pnpm run validate:product` |
| Rust shared code (`relperm.rs`, `pvt.rs`, `mobility.rs`, `capillary.rs`, `well*.rs`, `step.rs`, `reporting.rs`, `frontend.rs`) | `bash scripts/validate-solver-coverage.sh all` + BL benchmarks |
| IMPES only (`src/lib/ressim/src/impes/`) | `bash scripts/validate-solver-coverage.sh impes` then `shared` |
| FIM only (`src/lib/ressim/src/fim/`) | FIM locked baseline (below) + `bash scripts/validate-solver-coverage.sh fim` then `shared`; for solver *behavior* changes also run the wasm control matrix (see `fim-solver-debug` skill) |
| FIM linear-report/oracle code | Focused report-contract tests for every affected backend and well-Schur wrapping + targeted replay of the captured system; then the normal FIM-only gates above |
| Analytical modules (`src/lib/analytical/`) | `pnpm test` (analytical + contract tests) |
| Scenario catalog (`src/lib/catalog/`) | `pnpm test` then `pnpm run typecheck` |
| WASM API surface (`frontend.rs`, `lib.rs`, worker payloads) | `bash scripts/build-wasm.sh` + `pnpm run validate:product` |

## Command reference

Frontend (from repo root, always pnpm — never npm):

```bash
pnpm run typecheck        # tsc --noEmit
pnpm run lint             # eslint, zero warnings allowed
pnpm test                 # vitest run
pnpm run validate         # typecheck + lint + test + build
pnpm run validate:product # validate + Rust IMPES bucket
```

Rust buckets (grouped, curated, safe to run — no hanging tests):

```bash
bash scripts/validate-solver-coverage.sh shared   # both-solver parity contracts
bash scripts/validate-solver-coverage.sh impes    # IMPES-owned tests
bash scripts/validate-solver-coverage.sh fim      # FIM-owned fast tests
bash scripts/validate-solver-coverage.sh all
```

FIM locked day-to-day baseline (exact commands from `docs/FIM_STATUS.md`):

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_first_steps_converge_without_stall -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_fim_gas_injection_creates_free_gas -- --nocapture
```

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

## Interpreting failures

- **Vitest contract failures after a catalog change** usually mean the scenario metadata violated a real contract (e.g. a sensitivity variant claims `affectsAnalytical: true` but doesn't perturb the analytical result). Fix the metadata, not the test — the test is the spec.
- **`*_on_both_solvers` test failures** mean IMPES and FIM public behavior diverged. Do not weaken the contract; find which solver changed.
- **Bit-parity gate failures in `fim/assembly_ad.rs`** mean the AD assembly and the legacy assembly no longer agree. See the `engine-physics-change` skill — physics helpers often have both a legacy and an AD implementation that must be changed together.
- If a test fails on a clean tree before your change, record that first (it is a pre-existing failure, not yours) and note it in `TODO.md`.
- AD/legacy/finite-difference agreement means ResSim differentiated its own residual consistently;
  it does **not** prove that the residual, primary variables, bounds, or well formulation match
  OPM. Require a sourced OPM semantic or trajectory comparison for an OPM-parity claim.
- A direct solver is not automatically a truth oracle. Check `||J dx - rhs|| / ||rhs||` on the
  same full system and compare the returned correction before interpreting different
  `converged` flags.

## CI reality check

`.github/workflows/pr-tests.yml` runs only vitest + typecheck. **CI does not run any Rust test.** Local Rust validation is the only Rust gate — never assume CI caught an engine regression.
