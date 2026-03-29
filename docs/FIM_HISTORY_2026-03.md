# FIM History Archive 2026-03

This file preserves the March 2026 FIM implementation and debugging narrative that previously lived inline in `TODO.md`.

It is intentionally archival.

- Use `docs/FIM_STATUS.md` for the current consolidated state.
- Use `docs/FIM_CONVERGENCE_WORKLOG.md` for active investigations.
- Use `docs/FIM_MIGRATION_PLAN.md` for the target architecture.

## Preserved Context

### Implemented Foundation

- FIM state, flash, assembly, Newton, linear-solver, and well-topology slices are in place under `src/lib/ressim/src/fim/`.
- The public step path can execute the FIM branch and write accepted state back into the normal reporting pipeline.
- The current implementation includes explicit physical-well grouping, explicit perforation records, coupled well/control equations, and an iterative linear backend with a pressure-first coarse stage.

### Preserved Solver Findings

- `DRSDT = 0` semantics were initially enforced only in the regime label path; the actual gas-inventory split path also needed the same base-`Rs` cap.
- Accepted no-op iteration-0 entry states were allowing tiny cutback steps to advance time without meaningful state change; that acceptance path was tightened.
- The remaining hard cases are dominated by coupled nonlinear behavior rather than an obvious missing source term or a simple linear-solver failure.

### Preserved Validation Notes

- Focused regression coverage now includes a `DRSDT = 0` gas-split regression in `src/lib/ressim/src/tests/pvt_properties.rs`.
- Current SPE1/FIM smoke coverage exists in the crate-root Rust tests.
- Native-only convergence probes live in `src/lib/ressim/src/tests/fim_debug.rs` and remain useful, but they need explicit classification before more solver work.

### Preserved Open Questions

- The coupled SPE1/FIM gas-path issue still lacks a clean, stable repro suitable for the short regression baseline.
- Native-only diagnostics, crate-root ignored probes, and stale FIM test files still need classification and cleanup.
- Repository-root experimental patch artifacts and broad dead-code suppressions were identified as cleanup targets before more tuning.

## Why This Was Archived

The old `TODO.md` entry had become a running history log. That made it hard to tell:

- what is still open
- what is already validated
- where the current source of truth lives

The detailed narrative is preserved here so the live tracker can stay short without losing the useful March 2026 context.