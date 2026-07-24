# Solver Test Coverage Plan

This document turns the solver-layout and ownership split into an explicit coverage program with
exit criteria. The goal is not "more tests" in the abstract. The goal is to ensure that every
meaningful behavior is owned by exactly one of three test buckets:

- shared logic and public contracts under `src/lib/ressim/src/tests/`
- FIM-owned solver-local behavior under `src/lib/ressim/src/fim/tests/` and FIM-adjacent white-box
  files
- IMPES-owned solver-local behavior under `src/lib/ressim/src/impes/tests/`

"Full coverage" for this codebase means every high-value behavior below has a default test home,
at least one fast gate, and clear ownership. It does not mean every scenario must match across
solvers on exact rates or substep traces.

## Coverage Rules

1. Shared tests own public contracts that should survive solver refactors.
2. FIM tests own coupled-implicit, Newton, assembly, and FIM-only convergence/diagnostic behavior.
3. IMPES tests own explicit transport, adaptive retry/substep, and IMPES-only near-well or
   pressure-state behavior.
4. Cross-solver parity only checks stable public outcomes:
   - finite public reports
   - final-time completion
   - sign conventions
   - monotone closed-system trends when guaranteed by the scenario
   - bounded accounting envelopes
   - public limit/control flags
5. Exact rate parity, identical history length, and identical retry/substep structure are not exit
   criteria for shared parity.

## Required Shared Coverage

### Runtime and API

Required fast gates:

- public stepping completes requested time
- public rate/BHP control state is exposed correctly
- public limit fractions stay valid
- public inventory/material-balance fields stay finite
- mixed-control and shared-block cases remain finite

Status:

- in place in `runtime_api.rs`

### Properties and API Surface

Required fast gates:

- geometry setters and validation
- PVT/property interpolation behavior
- three-phase public configuration and state behavior
- well-control public API behavior

Status:

- in place in `geometry_api.rs`, `pvt_properties.rs`, `three_phase.rs`, `well_controls.rs`

### Physics Families

Required fast gates per family:

- depletion oil: closed-system monotonicity, public reporting, timestep sanity
- depletion gas: closed-system monotonicity, bounded accounting, public reporting
- depletion liberation: undersaturated constancy plus phase-transition public contract
- waterflood: front direction, bounded accounting, public reporting
- gas flood: inventory/accounting, bounded state, public reporting
- gas cap: gravity/hydrostatic behavior plus at least one two-solver parity benchmark
- wells/sources: source totals, rate-target honoring, public report sign conventions
- geometry/anisotropy: directional outcome changes and bounded state

Exit criterion:

- each family has at least one default two-solver public-contract test where solver parity is
  meaningful, plus family-local tests where parity is not the right surface

## Required FIM Coverage

### Algebra and Newton

Required fast gates:

- Jacobian finite-difference checks
- accumulation and flux conservation checks
- well connection law oracles
- Newton acceptance / residual / material-balance sanity on local cells

Status:

- mostly in place across `fim/assembly_tests.rs`, `fim/assembly.rs`, and `fim/tests/depletion.rs`

### FIM-Owned Scenario Behavior

Required fast gates:

- SPE1 smoke progression
- coupled well behavior and shared-BHP consistency
- depletion refinement and analytical probes as opt-in diagnostics
- any known FIM-only benchmark or convergence shelf reproduced in an owned location

Exit criterion:

- every ignored FIM diagnostic has a corresponding default fast correctness gate that protects the
  same physics surface

## Required IMPES Coverage

### IMPES Timestep and Retry Path

Required fast gates:

- adaptive retry/substep loop exercised directly
- pressure-state guard stays physical after retries
- explicit transport path updates saturations and reporting sanely

Status:

- in place in `impes/tests/timestep.rs` and `impes/tests/transport.rs`

### IMPES Ownership Checklist

Use this checklist when deciding whether an IMPES-related test belongs under `src/tests/` or under
`src/impes/tests/`.

Keep a test in the shared bucket when it checks any of the following through public behavior:

- public step completion semantics
- public report fields, sign conventions, and limit fractions
- stable two-solver parity for bounded accounting, monotonicity, or final-time completion
- scenario-scale physics outcomes that should survive replacing the timestep implementation

Move or keep a test in the IMPES-owned bucket when it needs any of the following:

- explicit retry or substep counts
- direct dependence on the adaptive timestep loop
- direct dependence on pressure-state recovery or pressure-physicality guards
- direct dependence on `update_saturations_and_pressure()` behavior or IMPES-specific near-well
   reporting mechanics

Current audited IMPES-owned obligations:

- `transport.rs`: explicit transport/reporting sanity that depends on the IMPES update path
- `timestep.rs`: retry/substep and pressure-state guard behavior

Exit criterion:

- each IMPES-owned test maps to one private IMPES implementation obligation not already covered by
   shared runtime or shared physics tests

## Remaining High-Priority Gaps

1. Keep the shared/FIM/IMPES ownership audit current as new tests are added.
2. Keep the scripted grouped validation workflow current as new fast gates are added.
3. Revisit diagnostic-only probes only when their known parity or analytical/model-alignment gaps
   become active work.

## Execution Order

1. Keep the ownership checklist and diagnostic matrix current.
2. Keep the scripted grouped validation workflow current.
3. Revisit diagnostic-only probes only when their known gaps become active work.

The default-gate to ignored-diagnostic mapping and grouped validation commands now live in
`docs/SOLVER_DIAGNOSTIC_COVERAGE_MATRIX.md`.

The grouped validation workflow is now routinized through `scripts/validate-solver-coverage.sh`
and the `.vscode/tasks.json` coverage tasks.

## Definition Of Done

This coverage plan is complete when all of the following are true:

- each shared physics family has at least one meaningful two-solver public-contract test
- each solver-local bucket has only tests that truly need private solver internals
- every ignored diagnostic has an identified fast sibling or is explicitly marked exploratory only
- `TODO.md` and `docs/SOLVER_TEST_OWNERSHIP_INVENTORY.md` both point to the current coverage plan
- grouped validation exists for shared, FIM-owned, and IMPES-owned buckets