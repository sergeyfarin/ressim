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

### Missing IMPES-Specific Obligation

Current gap:

- there is still no explicit checklist showing which shared public contracts are intentionally not
  duplicated in IMPES-owned tests because they already live above the solver boundary

Exit criterion:

- each IMPES-owned test should map to one private implementation obligation not already covered by
  shared runtime or shared physics tests

## Remaining High-Priority Gaps

1. Shared liberation-through-bubble-point public stepping parity on both solvers.
   Why: depletion oil and gas now have shared public-contract parity, but liberation still stops at
   undersaturated flash constancy in the shared suite while the stepping path is FIM-only.
2. Explicit coverage matrix for ignored diagnostics.
   Why: some FIM ignored probes already have default fast siblings, but the mapping is not yet
   written down in one place.
3. Shared geometry/anisotropy parity where both solvers should agree on directional outcomes or
   boundedness without matching exact rates.
4. Periodic grouped validation commands for each bucket so ownership regressions are caught as sets,
   not only as single tests.

## Execution Order

1. Close the shared liberation public-contract gap.
2. Record the default-gate to ignored-diagnostic mapping for FIM and IMPES obligations.
3. Add one shared geometry/anisotropy parity slice.
4. Add grouped validation commands to `TODO.md` and the ownership inventory.

## Definition Of Done

This coverage plan is complete when all of the following are true:

- each shared physics family has at least one meaningful two-solver public-contract test
- each solver-local bucket has only tests that truly need private solver internals
- every ignored diagnostic has an identified fast sibling or is explicitly marked exploratory only
- `TODO.md` and `docs/SOLVER_TEST_OWNERSHIP_INVENTORY.md` both point to the current coverage plan
- grouped validation exists for shared, FIM-owned, and IMPES-owned buckets