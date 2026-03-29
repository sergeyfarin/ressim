# FIM Phase 2 Execution Plan

Purpose: turn Phase 2 of the cleanup into a controlled migration instead of a one-pass file shuffle.

This document now reflects the approved policy direction and is ready to drive execution.

## Why Phase 2 Needs Its Own Plan

The original cleanup plan described Phase 2 as a test classification step, but the current workspace shows a broader problem:

- some FIM smoke tests still live in `src/lib/ressim/src/lib.rs`
- `src/lib/ressim/src/tests/spe1_fim.rs` exists but is currently only a stub and is not the active home of the SPE1/FIM regressions
- `src/lib/ressim/src/tests/fim_spe1_bug.rs` is a one-off ad hoc repro, not a clean maintained regression
- the native debug harness in `src/lib/ressim/src/tests/fim_debug.rs` is large and scenario-heavy
- the helper scripts mix real entry points with scratch files and partially broken experiments
- the desired diagnostic policy has shifted toward wasm-first investigation, with native retained only when it adds real value

Because of that, Phase 2 should be executed as a staged migration with explicit review gates.

## Phase 2 Goals

- classify every FIM-related test or probe into one of:
  - production regression
  - ignored diagnostic
  - obsolete probe
- remove tests and debug probes from `src/lib/ressim/src/lib.rs`
- move maintained FIM coverage into dedicated test files
- define a wasm-first diagnostic workflow, with native diagnostics treated as secondary tools
- reduce diagnostic verbosity so traces are useful rather than overwhelming
- classify helper scripts as maintained workflow, temporary scratch, or obsolete

## Current Surface Snapshot

### Rust tests and probes

- `src/lib/ressim/src/lib.rs`
  - active FIM smoke regressions currently live here:
    - `spe1_fim_first_steps_converge_without_stall`
    - `spe1_fim_gas_injection_creates_free_gas`
    - `spe1_fim_coarse_grid_reaches_producer_gas_breakthrough`
  - ignored debug probes also live here:
    - `debug_low_kv_injector_balance_probe`
    - `debug_spe1_producer_breakthrough_probe`
    - `debug_spe1_producer_late_time_probe`
- `src/lib/ressim/src/tests/fim_debug.rs`
  - real native-only ignored diagnostic harness
  - currently holds many scenario builders and many scenario-specific entry points
  - verbose output is emitted through `step_fim_verbose()` and `eprintln!`
- `src/lib/ressim/src/tests/fim_spe1_bug.rs`
  - ad hoc one-off repro using direct prints
  - likely obsolete or in need of conversion into a maintained diagnostic
- `src/lib/ressim/src/tests/spe1_fim.rs`
  - currently only a stub, not the real regression home yet

### Helper scripts and scratch files

- maintained-looking scripts:
  - `test-native.sh`
  - `test-wasm.sh`
- likely scratch or broken exploratory files:
  - `test-wasm-spe1-short.sh`
  - `test-wasm-spe1.js`
  - `test_import.mjs`
  - possibly `test.sh`, depending on whether it is still a maintained wasm repro path or just a manual snippet

### Frontend scenario entry points

- relevant long-running wasm scenarios exist in `src/lib/catalog/scenarios/`
- especially relevant but expensive:
  - `src/lib/catalog/scenarios/spe1_gas_injection.ts`
  - `src/lib/catalog/scenarios/sweep_combined.ts`

These should inform the diagnostic policy, but they should not be the first-line day-to-day convergence repros if they are too slow.

## Approved Policy Decisions

### 1. Diagnostic priority

Approved default:

- wasm is the default diagnostic target
- native should not remain part of the normal workflow unless it proves unique value that wasm cannot provide

Execution implication:

- Phase 2 must not only classify tests and scripts
- it must also provide a wasm diagnostic path that can expose useful solver diagnostics during investigation

Rationale:

- production behavior matters most in wasm
- native-only inputs and harnesses have drifted and can send investigation in the wrong direction
- if wasm can expose the needed diagnostics, native no longer justifies its maintenance cost

### 2. End-state for crate-root tests

Proposed default:

- no FIM-specific regression or debug probe remains in `src/lib/ressim/src/lib.rs`
- `lib.rs` keeps only shared helpers and explicit module declarations

### 3. End-state for dedicated files

Proposed default:

- `src/lib/ressim/src/tests/spe1_fim.rs`
  - becomes the maintained home for stable SPE1/FIM regressions
- `src/lib/ressim/src/tests/fim_debug.rs`
  - remains the native-only diagnostic harness, but with reduced verbosity and clearer purpose
- `src/lib/ressim/src/tests/fim_spe1_bug.rs`
  - either deleted or replaced by a documented diagnostic file only if it still adds unique signal

### 4. End-state for scripts

Approved default:

- keep one canonical wasm diagnostic script with switches for scenario, grid, settings, and diagnostic level
- do not keep a native script unless it proves unique value after the wasm diagnostic path exists
- remove broken or scratch workflow files rather than preserving them as undocumented history

Desired script behavior:

- easy to run from repository root
- support multiple scenarios or repro presets
- allow switching diagnostic verbosity without forcing per-iteration spam by default
- make it possible to inspect different grids, settings, and solver behaviors from one entry point

## Proposed Execution Sequence

### Slice 1: Inventory and decision table

Create a classification table covering:

- each Rust FIM test/probe
- each helper script
- each scenario intended for diagnostics

For each item, record:

- current role
- target bucket
- keep/move/delete decision
- target destination if moved
- whether it is wasm-first, native-only, or cross-target

Deliverable:

- a checked-in classification table in `docs/FIM_STATUS.md` or a dedicated appendix doc
- explicit marking of which workflows are wasm-default versus candidates for deletion

### Slice 2: Extract stable FIM regressions from `lib.rs`

Move the maintained SPE1/FIM smoke tests out of `src/lib/ressim/src/lib.rs` into a dedicated test module.

Expected target:

- `src/lib/ressim/src/tests/spe1_fim.rs`

Precondition:

- shared builders such as `make_spe1_like_base_sim()` must be accessible from the new test module without worsening module coupling

Review gate:

- if the helper layout fights the move, do a helper extraction first rather than leaving the tests in `lib.rs`

### Slice 3: Remove or relocate crate-root debug probes

Handle the ignored probes in `src/lib/ressim/src/lib.rs` one by one:

- move to a dedicated diagnostics file if still useful
- otherwise delete

Rule:

- no debug-only probe remains in `lib.rs`

### Slice 4: Rationalize `fim_debug.rs`

Keep only the parts that still add real value, and reduce noise:

- document intended use
- group scenarios more clearly
- cut needless always-on verbosity where possible
- preserve only the scenario entry points that still add diagnostic value
- merge repetitive probes where behavior can be switched by parameters instead of duplicated test bodies

Potential follow-up split if needed:

- separate scenario builders from scenario entry points
- separate waterflood, gas, and SPE1 diagnostics into smaller files

### Slice 5: Classify helper scripts and replace them with one canonical wasm entry point

Audit and classify:

- `test-native.sh`
- `test-wasm.sh`
- `test-wasm-spe1-short.sh`
- `test-wasm-spe1.js`
- `test_import.mjs`
- `test.sh`

Likely outcomes:

- keep and document the maintained wasm workflow
- delete broken scratch files immediately
- keep no native entry point unless wasm cannot cover the same diagnostic need

### Slice 6: Document the default workflow

Write down one default workflow for day-to-day FIM debugging:

- short regression run
- primary wasm diagnostic entry point
- when native is not needed and when an exception would have to be justified
- how to capture traces without drowning in output

Target location:

- `docs/FIM_STATUS.md`

## Initial Classification Direction

This is the approved starting direction for execution, subject only to item-by-item confirmation during the migration.

| Item | Current state | Proposed bucket | Proposed action |
| --- | --- | --- | --- |
| `src/lib/ressim/src/lib.rs` `spe1_fim_first_steps_converge_without_stall` | real regression in wrong file | production regression | move to `src/lib/ressim/src/tests/spe1_fim.rs` |
| `src/lib/ressim/src/lib.rs` `spe1_fim_gas_injection_creates_free_gas` | real regression in wrong file | production regression | move to `src/lib/ressim/src/tests/spe1_fim.rs` |
| `src/lib/ressim/src/lib.rs` `spe1_fim_coarse_grid_reaches_producer_gas_breakthrough` | real regression candidate but not short on 2D/3D-style follow-up workloads | production regression | move to `src/lib/ressim/src/tests/spe1_fim.rs`, but classify outside the short default set |
| `src/lib/ressim/src/lib.rs` debug probes | ignored probes in crate root | ignored diagnostic or obsolete probe | move or delete individually |
| `src/lib/ressim/src/tests/fim_debug.rs` | active native harness | ignored diagnostic or migration source | trim, consolidate, and keep only if wasm does not fully replace it |
| `src/lib/ressim/src/tests/fim_spe1_bug.rs` | ad hoc print-heavy repro | obsolete probe unless unique value proven | likely delete |
| `src/lib/ressim/src/tests/spe1_fim.rs` | disconnected stub | n/a | convert into the real regression module |
| `test-native.sh` | maintained native runner | candidate for deletion | keep only if unique value survives wasm diagnostic upgrade |
| `test-wasm.sh` | maintained wasm runner | canonical diagnostic workflow | keep, expand, and promote |
| `test-wasm-spe1-short.sh` | broken scratch writer | obsolete probe | delete |
| `test-wasm-spe1.js` | scratch note file | obsolete probe | delete |
| `test_import.mjs` | import probe | obsolete probe unless still needed | likely delete |
| `test.sh` | manual wasm snippet | candidate for consolidation | merge into canonical wasm entry point or delete |

## Execution Priorities

1. Build the classification table and mark current owners, destinations, and deletion candidates.
2. Design the canonical wasm diagnostic path so diagnostics are available without relying on native.
3. Delete clearly broken scratch files as early as possible.
4. Move stable FIM regressions and remaining useful diagnostics out of `lib.rs`.
5. Trim or remove native-only infrastructure once wasm coverage is sufficient.

## Confirmed Decisions

1. wasm is the default diagnostic target
2. delete repeated crate-root probes unless they clearly add unique value
3. only genuinely short cases belong in the short default regression set; 2D and 3D breakthrough-style cases do not
4. prefer one canonical wasm script with switches over parallel native and wasm workflows
5. delete clearly broken scratch files

## Execution Gate

Do not start the large move/delete pass until the wasm diagnostic path and classification table are defined clearly enough that cleanup reduces confusion instead of recreating it.