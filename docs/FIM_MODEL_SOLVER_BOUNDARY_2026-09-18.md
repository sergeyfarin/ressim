# FIM model/solver boundary contract

Date: 2026-09-18. This closes the design-only part of F7 in
`FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md`. It describes the smallest boundary that a second
fluid model may consume. It does not claim that the black-oil Newton loop already implements
these interfaces or that compositional behavior has been reviewed.

## Current reusable surface

| Surface | Current implementation | Reuse contract | Remaining coupling |
|---|---|---|---|
| Linear row layout | `fim/linear/mod.rs::FimLinearBlockLayout` | Cell blocks followed by typed well-BHP and perforation tail ranges | Cell block width and pressure-primary index remain supplied as black-oil constants |
| Linear result | `FimLinearSolveReport` plus recovered full-system norms/partitions | Return a full correction and backend-neutral full-system diagnostics | Some diagnostic-only paths still assume black-oil equation-family names |
| Linear solve | `solve_linearized_system` and optional well Schur elimination | Consume matrix, RHS, scaling and layout; no fluid-property calls | CPR pressure restriction needs a model-provided pressure row/column map for a different cell layout |
| Nonlinear convergence helpers | `fim/newton/convergence.rs` | Pure residual/update norm and material-change helpers where inputs are explicit | Main Newton orchestration owns black-oil state and equation families directly |
| Retry/controller | `fim/timestep.rs` | Retry only an uncommitted attempt; advance time/reporting only after commit | Orchestration directly constructs `FimState`, invokes black-oil Newton and writes black-oil reports |

The named layout is a real reusable seam. It is not the whole model/solver boundary.

## Required adapter contract

The implementation may use traits, generic functions or concrete adapter calls. These semantic
operations are the contract; their order is fixed.

### Model-owned state and assembly

```text
snapshot(simulator) -> PreviousState
initial_iterate(previous) -> TrialState
assemble(previous, trial, dt, residual_only) -> AssemblyResult
apply_trial_update(trial, correction, update_policy) -> TrialState
evaluate_accepted(previous, trial, dt) -> AcceptedEvaluation
commit(simulator, accepted_state)
report(simulator, previous_inventory, accepted_state, accepted_dt)
```

`AssemblyResult` supplies residual, optional Jacobian, equation scaling, variable scaling and a
linear layout. The model owns primary variables, phase classification, flash/cache state,
admissibility and component inventories. The solver must not infer these from `index % 3` or from
black-oil saturation fields.

`AcceptedEvaluation` contains the exact state eligible for commit and the residual/convergence
quantities re-evaluated for that state. The solver may not mutate it after acceptance. A model
whose phase classification changes at acceptance must reassemble after that classification.

### Solver-owned lifecycle

For each attempted dt the solver:

1. asks the model for an initial trial state;
2. assembles and solves corrections against that trial state;
3. asks the model to apply and validate each trial update;
4. asks the model to evaluate the final accepted candidate;
5. returns either an immutable accepted evaluation or a rejected-attempt report;
6. commits and reports only the accepted evaluation;
7. advances time and accepted-step controller memory only after commit.

A rejected attempt may update diagnostic and retry-controller memory explicitly designated as
nonphysical. It may not change reservoir state, well state, component/source ledgers, accepted
history, time, or model caches read by the next attempt. Test fault injection must act at the
attempt boundary and remain absent from production builds.

### Snapshot, commit and rollback

Rollback is structural: attempts operate on owned trial state. The pre-attempt simulator is not
an undo log. The snapshot contract includes all model state that can affect a later residual:

- reservoir primaries and derived/flash cache state;
- physical-well unknowns and per-connection unknowns;
- model-specific phase labels or stability decisions;
- accepted component/source ledgers and complete published history;
- time and accepted-step counters.

Controller-only retry memory is separate and must be named in the attempt report. This prevents
a compositional flash cache or stability result from leaking merely because it is not one of the
black-oil arrays checked by an older rollback test.

## Black-oil adapter mapping

| Contract operation | Black-oil owner |
|---|---|
| Snapshot / initial iterate | `FimState::from_simulator` |
| Assembly | `assemble_fim_system_ad` (`assembly.rs` remains the parity oracle) |
| Trial update/admissibility | `apply_newton_update_frozen`, damping and flash helpers in `newton.rs` / `state.rs` |
| Accepted evaluation | accepted-state convergence path in `newton.rs`; must return the exact state evaluated |
| Commit | `FimState::write_back_to_simulator` |
| Inventory/report | `total_*_inventory_sc` and `record_fim_step_report` |
| Retry and time | `step_internal_fim_impl` |

The adapter must preserve the current operation order and both `Legacy` and `OpmAligned`
policies. Pressure/Sw/Rs damping, black-oil phase switching and black-oil equation scaling stay
inside this adapter.

## Admission tests for an extracted boundary

Before moving orchestration behind this contract:

1. compare residuals, Jacobian occupancy, corrections, accepted physical state and trace counts
   before/after extraction on held black-oil fixtures;
2. run both nonlinear flavors, a three-phase transition and nonzero well sources;
3. prove a deliberately corrupted commit is detected after reading state back from the simulator;
4. inject a rejected outer attempt and compare with a clean replay of its accepted-dt sequence,
   including time, full history, wells, inventories and cumulative sources;
5. retain AD/legacy assembly parity, curated FIM/shared gates and the WASM control matrix.

The compositional implementation may introduce its own adapter after its state, flash,
component equations and well unknowns exist. This document does not accept that adapter or
resolve C12; those require review on the compositional branch.
