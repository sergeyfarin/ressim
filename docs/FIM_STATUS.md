# FIM Status

This is the consolidated current-state summary for the Rust FIM solver.

Use this file for:

- current implementation state
- current blockers and parity gaps
- canonical validation and diagnostic entry points

Do not use this file as a detailed experiment log. Active reproductions and temporary hypotheses belong in `docs/FIM_CONVERGENCE_WORKLOG.md`.

## Current State

- FIM remains under active convergence and parity work.
- The immediate goal is to stabilize the working surface before another solver-tuning pass.
- Cleanup is in progress so new convergence edits are judged against one consistent baseline instead of mixed tracker notes, debug probes, and stale artifacts.

## Validated Fixes Kept In Baseline

- `DRSDT = 0` handling now reaches the actual gas-inventory split path instead of only the regime label.
- FIM Newton no longer accepts ordinary-tolerance iteration-0 no-op states unless the unchanged state is effectively exact.
- A focused PVT regression covers excess dissolved gas flashing to free gas under a base-`Rs` cap.

## Known Open Gaps

- Coupled FIM convergence and timestep fragmentation are still unresolved on harder 2D and 3D cases.
- The remaining SPE1/FIM gas-path issue is not reduced to a stable canonical regression yet.
- Test and diagnostic surfaces are not yet fully classified into regression, diagnostic, and obsolete buckets.
- Some debug probes still live outside dedicated diagnostic files.

## Canonical Sources

- Active tracker: `TODO.md`
- Active investigation log: `docs/FIM_CONVERGENCE_WORKLOG.md`
- Architecture target: `docs/FIM_MIGRATION_PLAN.md`
- Cleanup sequence: `docs/FIM_CLEANUP_PLAN.md`
- Historical March 2026 solver notes preserved from the live tracker: `docs/FIM_HISTORY_2026-03.md`

## Current Validation Surface

Short regression checks worth keeping near the day-to-day baseline:

- `src/lib/ressim/src/tests/pvt_properties.rs`
  - `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`
- `src/lib/ressim/src/lib.rs`
  - `spe1_fim_first_steps_converge_without_stall`
  - `spe1_fim_gas_injection_creates_free_gas`

Diagnostic entry points for deeper convergence work:

- native-only ignored diagnostics in `src/lib/ressim/src/tests/fim_debug.rs`
- verbose stepping helper `step_fim_verbose()` in `src/lib/ressim/src/step.rs`

## Current Working Rules

- Keep `TODO.md` short and action-oriented.
- Put active reproductions, traces, and next hypotheses in `docs/FIM_CONVERGENCE_WORKLOG.md`.
- Keep `docs/FIM_MIGRATION_PLAN.md` focused on the intended end-state architecture, not current debugging status.
- Do not promote a toy or unstable repro into the canonical regression set until it is reliable enough to gate edits.