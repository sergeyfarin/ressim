# FIM Test And Diagnostic Classification

This file is the checked-in Phase 2 inventory for FIM-related regressions, diagnostics, scripts, and scenario entry points.

It is the working classification table used to drive cleanup execution.

## Policy Baseline

- wasm is the default diagnostic target
- native should be kept only if it provides unique diagnostic value that wasm cannot expose
- no debug-only probe should remain in `src/lib/ressim/src/lib.rs`
- stable FIM regressions should live in dedicated test modules, not in `lib.rs`
- per-iteration and per-substep spam should not be the default diagnostic mode

## Rust Test And Probe Inventory

| Item | Current state | Bucket | Target action | Target runtime |
| --- | --- | --- | --- | --- |
| `src/lib/ressim/src/tests/pvt_properties.rs` `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas` | focused regression in correct file | production regression | keep | cross-target |
| `src/lib/ressim/src/fim/tests/spe1.rs` `spe1_fim_first_steps_converge_without_stall` | maintained FIM smoke regression in solver-local file | production regression | keep in `src/lib/ressim/src/fim/tests/spe1.rs` | cross-target |
| `src/lib/ressim/src/fim/tests/spe1.rs` `spe1_fim_gas_injection_creates_free_gas` | maintained FIM smoke regression in solver-local file | production regression | keep in `src/lib/ressim/src/fim/tests/spe1.rs` | cross-target |
| `src/lib/ressim/src/lib.rs` former crate-root debug probes | redundant ignored debug probes previously embedded in the crate root | obsolete probe | deleted during Phase 2 cleanup | removed from crate root |
| `src/lib/ressim/src/tests/fim_spe1_bug.rs` | ad hoc print-heavy repro | obsolete probe | deleted during Phase 2 cleanup | removed |
| `src/lib/ressim/src/fim/tests/spe1.rs` | solver-local regression module | production regression container | keep as the home for stable SPE1/FIM regressions | cross-target |

## Helper Script Inventory

| Item | Current state | Bucket | Target action | Notes |
| --- | --- | --- | --- | --- |
| `test-wasm.sh` | wrapper for the canonical wasm diagnostic runner | canonical diagnostic workflow | keep as the shell entry point over `scripts/fim-wasm-diagnostic.mjs` | default wasm-first path |
| `test.sh` | manual wasm snippet superseded by the canonical runner | obsolete probe | deleted during Phase 2 cleanup | replaced by `test-wasm.sh` and `scripts/fim-wasm-diagnostic.mjs` |
| `test-wasm-spe1-short.sh` | broken scratch script that writes an invalid Rust stub | obsolete probe | deleted in first Phase 2 pass | scratch artifact |
| `test-wasm-spe1.js` | scratch note file, not a maintained runner | obsolete probe | deleted in first Phase 2 pass | scratch artifact |
| `test_import.mjs` | import probe with no documented workflow role | obsolete probe | deleted in first Phase 2 pass | scratch artifact |

## Frontend Scenario Inventory

| Item | Current state | Bucket | Target action | Notes |
| --- | --- | --- | --- | --- |
| `src/lib/catalog/scenarios/spe1_gas_injection.ts` | valuable wasm-facing reference scenario | diagnostic scenario | keep, but do not use as a default quick repro because it is expensive | useful for parity, not for fast loops |
| `src/lib/catalog/scenarios/sweep_combined.ts` | useful wasm-facing scenario, but expensive | diagnostic scenario | keep, but do not use as a default quick repro because it is expensive | useful for broader workflow checks |

## Short Default Baseline Direction

The short default FIM baseline should contain only genuinely short checks.

Keep in the short default set:

- focused unit regressions such as the `DRSDT = 0` gas-split regression
- short SPE1/FIM smoke tests in `src/lib/ressim/src/fim/tests/spe1.rs`

Keep outside the short default set:

- 2D and 3D breakthrough-style regressions
- long scenario-driven wasm checks
- verbose diagnostics and exploratory probes

## Immediate Next Actions

1. keep extending the canonical wasm diagnostic runner instead of reintroducing target-specific probes
2. keep the short default baseline focused on true regressions rather than verbose diagnostics