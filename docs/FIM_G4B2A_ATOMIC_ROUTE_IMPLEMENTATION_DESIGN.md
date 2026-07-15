# G4b2a — Flow RESV Atomic-Route Implementation Design

Status: **COMPLETE — CODE ROUTE SPECIFIED; EXECUTION BLOCK REMAINS (2026-07-15)**

This is the implementation contract for the next G4 code slice. It turns the G4b2 readiness
inventory into one default-off, one-perforation route. It is deliberately more prescriptive than
the G4a lifecycle design: an implementation is acceptable only if it follows the data model,
row/column contract, and gates below as a single change.

It does **not** authorize a live RESV convergence comparison. The pre-Newton execution block in
`fim/timestep.rs` remains until the atomic route and its non-live gates have landed. The first
live rung is a separate slice and is valid only if it has no retry and has the complete trace in
§9.

## 1. Decision record

### Scope

The route is exactly one enabled gas injector, one perforation, explicit schedule `RESV`, one
PVT/FIP region, no surface target, no explicit BHP constraint, and
`FIM_NESTED_WELL_SOLVE=0`. The native option stays default-off. All existing non-RESV wells and
all disabled-option executions must remain bit-identical.

The state has two well-tail unknowns for the selected physical well:

```text
x_tail = [ bhp (bar), u (Sm3/day) ]
```

where `u >= 0` is the positive gas surface-rate primary. It is not a signed reservoir connection
rate. The tail's count and offsets remain unchanged: BHPs precede one perforation primary per
perforation. The selected primary merely changes its physical type and units.

### Non-goals and held constants

- No q-coordinate nested well solve, multi-perforation allocation, BHP switching, or general
  RESV support. G4b3 owns any such extension.
- No change to Newton acceptance, controller/retry policy, damping, linear solver, primary-cell
  lifecycle, Schur algorithm, or tolerances.
- No change to IMPES, public/WASM API, schedule parsing, or shared production source helper.
  IMPES has no FIM well tail; its rate conversion is not evidence for this FIM route.
- No statement that Flow refreshes its report-step reference after each accepted FIM substep.
  G4b0's present capture/refresh behaviour remains an implementation lifetime to source before a
  retry-containing comparison. A retry makes that comparison **INCONCLUSIVE**, not refuted.

### Invariants

1. The existing `flow_resv_injector_residual()` is the only definition of the three G4 values.
   Assemblers, trace code, and tests must consume its returned bundle; they may not restate its
   equations.
2. `B_g,ref` and `Q_resv` are `f64` report-step constants. Current cell `B_g` and connection
   rate remain state-dependent in both residual and Jacobian.
3. The selected primary is never read as q, passed into q relaxation, passed into nested solve,
   or emitted as `q`. Code needing a reservoir rate must explicitly evaluate the connection law.
4. The historical route is selected iff no `FlowResvReportStepContext` is present. An invalid
   context/state/topology combination is a programmer error in focused tests and a guarded
   pre-Newton rejection in production; it must never fall back to BHP/q.

## 2. Exact data and construction boundary

### 2.1 State representation

Perform the internal mechanical migration from the ambiguous field
`perforation_rates_m3_day: Vec<f64>` to the following explicit representation. This migration is
intentional even though it touches test fixtures: retaining a q-named vector for a u value would
make the exact G4 error easy to reintroduce in reporting, scaling, or relaxation.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimPerforationPrimaryKind {
    ReservoirConnectionQ,
    FlowResvGasSurfaceU,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimPerforationPrimary {
    pub(crate) kind: FimPerforationPrimaryKind,
    pub(crate) value: f64,
}

pub(crate) struct FimState {
    pub(crate) cells: Vec<FimCellState>,
    pub(crate) well_bhp: Vec<f64>,
    pub(crate) perforation_primaries: Vec<FimPerforationPrimary>,
}
```

Add total accessors rather than open-coded `.value` reads:

```rust
fn perforation_primary(&self, perf_idx: usize) -> FimPerforationPrimary;
fn reservoir_connection_q(&self, perf_idx: usize) -> Option<f64>;
fn flow_resv_surface_u(&self, perf_idx: usize) -> Option<f64>;
fn perforation_primary_value(&self, perf_idx: usize) -> f64;
```

The unknown/equation offset functions retain their existing names and layout for this slice so
that `FimLinearBlockLayout` and well Schur remain dimension-compatible. Rename a local variable
from `q_col` to `primary_col` on all route-aware paths. Historical-only paths may use `q_col`
after obtaining `ReservoirConnectionQ` explicitly.

### 2.2 One resolver, no duplicated schedule interpretation

Add a small internal resolver in `fim/flow_resv.rs` (or a dedicated `fim/well_route.rs` if that
avoids a cyclic module dependency):

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FimWellRoute {
    Historical,
    FlowResvGasInjector(FlowResvReportStepContext),
}

fn fim_well_route(
    context: Option<FlowResvReportStepContext>,
    topology: &FimWellTopology,
    well_idx: usize,
) -> FimWellRoute;

fn flow_resv_context_for_perforation(
    context: Option<FlowResvReportStepContext>,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> Option<FlowResvReportStepContext>;
```

It must assert all three identity relationships when present: the physical well index, the sole
perforation index, and the topology's one-perf ownership. `physical_well_control()` remains the
historical control resolver; do not teach it a partial `RESV` meaning. The route resolver is the
only authority that selects `FlowResvGasInjector`.

### 2.3 Construction and first iterate

`FimState::from_simulator()` continues to construct only historical q primaries. After G4b0 has
captured the immutable context and before the first Newton assembly, call one explicit conversion:

```rust
fn initialize_flow_resv_gas_primary(
    &mut self,
    sim: &ReservoirSimulator,
    topology: &FimWellTopology,
    context: FlowResvReportStepContext,
) -> Result<(), String>;
```

For the selected perforation it must:

1. set `kind = FlowResvGasSurfaceU` and
   `u = Q_resv / B_g,ref` (strictly positive for the validated injector target);
2. initialize BHP by solving the local connection law at the report-start cell state for
   `q_res = -Q_resv`, not by treating RESV as the historical BHP target;
3. leave every other primary and BHP bit-identical to the historical constructor.

Extract a narrowly named local inversion helper if needed, e.g.
`solve_bhp_for_connection_rate(sim, reservoir_target_m3_day)`. It must not call
`physical_well_control()` for the selected well. Unit-test the initial identity
`-q_res/B_g,ref == u` and retain an error if the inversion is unavailable/non-finite.

The report-step context is copied unchanged into every `FimAssemblyOptions` / Newton attempt.
The existing accepted-state context refresh remains behind the same accepted-write boundary; it is
not a license to refresh on a rejected attempt.

## 3. Residual and Jacobian contract

For the selected gas perforation, evaluate the current local connection law
`q_res(p, Sw, hc, bhp)` and current `B_g(p, Sw, hc)` and call the G4b1 helper:

```text
c_s    = -q_res / B_g(current)                  [Sm3/day]
R_perf = c_s - u                                [Sm3/day]
R_ctrl = B_g,ref * u - Q_resv                   [rm3/day]
S_gas  = -c_s = q_res / B_g(current)            [Sm3/day]
```

Scatter it exactly as follows. `dt` is in days.

| Row | Raw value | Nonzero columns |
| --- | --- | --- |
| gas component at perforated cell | `+ dt * S_gas` | cell `p,Sw,hc`; selected `bhp`; **not** `u` |
| water/oil component at perforated cell | `0` from this route | none from this route |
| selected perforation row | `R_perf` | cell `p,Sw,hc`; selected `bhp`; selected `u=-1` |
| selected well-control row | `R_ctrl` | selected `u=B_g,ref`; no cell or BHP column |

There is no FB function or BHP-limit complementarity in the RESV control row. The selected BHP
column exists only through connection/source dependence. At a converged perforation the gas source
is `-u`, but it has **zero u derivative** in an arbitrary Newton evaluation; do not substitute
that converged identity into the source row.

### 3.1 Live AD path

Extend `FimAssemblyOptions` with:

```rust
pub(crate) flow_resv_context: Option<FlowResvReportStepContext>,
```

and thread it through every full, residual-only, diagnostics, and test assembly call. Default
construction uses `None`.

In `assembly_ad.rs`, create one route-specific local evaluator that seeds exactly the five local
variables `[p, sw, hc, bhp, u]`, derives current properties/connection with the existing generic
helpers, calls `flow_resv_injector_residual()`, and scatters every returned derivative into the
table above. It replaces all three historical actions for the selected perf/well together:

- `component_rate_coefficients_generic * q`,
- `q - connection_rate_generic`, and
- `well_constraint_residual_fb_generic` plus its gradient.

All nonselected perfs and wells continue through the existing loops. Do not put an `if RESV`
inside only the source loop or only the constraint loop; route at the top of each selected
perforation/well iteration so it cannot fall through to the historical action.

### 3.2 Legacy parity path

The test-only `assembly.rs` must get the same route in the same commit. It remains an independent
scatter/derivative oracle; it must not invoke the live AD local evaluator. Add an explicitly
named f64/manual local linearization value, for example:

```rust
struct FlowResvLocalLinearization {
    residual: FlowResvInjectorResidual<f64>,
    d_connection_sc_day_d_cell: [f64; 3],
    d_connection_sc_day_dbhp: f64,
}
```

where `d_connection_sc_day_d_*` is the analytic derivative of `-q_res/B_g(current)`. Obtain
`dq_res` from the existing connection derivative primitive and add/extract the matching analytic
current-`B_g` cell derivative so that

```text
d(-q/B_g) = -(dq * B_g - q * dB_g) / B_g^2 .
```

Do not approximate this with a finite difference and do not use `B_g,ref` in that derivative.
The legacy route calls the shared f64 residual helper for values and uses this independent manual
linearization only for its Jacobian scatter. This retains a meaningful AD/legacy matrix oracle.

## 4. Update, bounds, scaling, and linear compatibility

### 4.1 Update semantics

`WellStateUpdateMode` gains a route-aware mode, e.g. `FlowResv`, selected only when the context
is present. The selected update sequence is:

1. apply the damped/chopped raw update to BHP and u;
2. clamp only selected `u` to the physical injector floor `0.0`;
3. do not call q relaxation, q trust radius, `enforce_control_bounds`, or the q-coordinate
   nested solver for the selected well;
4. preserve existing behaviour for all other wells.

Do not add a new arbitrary u trust radius in this slice. The floor is a state domain boundary, not
a new convergence policy. FD gates must evaluate strictly away from it. Every debugging hash,
extrapolation, material-change comparison, update trace, and test fixture must use the typed
primary value so a u update is neither omitted nor labelled q.

### 4.2 Scaling

Add route-aware scale helpers rather than reuse the q/BHP assumptions:

```text
RESV control-row scale        = max(abs(Q_resv), 1)       [rm3/day]
RESV perforation-row scale    = max(abs(u), 1)            [Sm3/day]
RESV primary variable scale   = max(abs(u), 1)            [Sm3/day]
```

The existing gas-component scale remains `PV / (B_g(current) * dt)` and needs no new tolerance.
`EquationScaling`, `VariableScaling`, family diagnostics, and any inner-solve scale call must
select these values from the same route/context. The nested inner solve remains rejected before it
can observe a RESV state.

### 4.3 Schur layout

No elimination formula changes. The tail is still a two-by-two BHP/primary local block, now with
the RESV pattern:

```text
                bhp             u
R_ctrl            0           B_g,ref
R_perf         dR/dBHP          -1
```

and reservoir couplings from current connection/source terms. Add one targeted fixture that
solves the complete RESV matrix directly and through `well_schur`; recovered corrections must
match all entries to the existing exact-reformulation tolerance. This is a matrix equivalence
gate, not a new direct-solver convergence experiment.

## 5. Reporting and trace contract

Historical `perforation_component_rates_sc_day()` and public FIM-rate reporting are q-based.
They must not receive a u primary. Add a route-aware diagnostic/source view used by FIM-only trace
and reporting:

```text
primary_kind=flow_resv_gas_u
u_sm3_day
q_connection_rm3_day
c_s_sm3_day
bg_current_rm3_per_sm3
bg_reference_rm3_per_sm3
q_resv_target_rm3_day
r_perf_sm3_day
r_ctrl_rm3_day
gas_source_sm3_day
d_gas_source_d[p,sw,hc,bhp,u]
```

For the selected route `d_gas_source_du` must print zero. Retain old `WELLSOURCE` fields for
historical perfs, but do not print `q=<u>`. Evaluation 0 and 1 must include all fields above plus
the row/column identifiers, raw and scaled residual-family peaks, and the actual applied primary
update. `FimStepStats`/human report may show the RESV well as surface-rate controlled only after
it uses this view; public product reporting remains out of scope unless the same FIM helper is
already its sole source.

## 6. Atomic code sequence and commit boundary

One implementation commit may contain all items below; splitting any numbered pair into a live
experiment is prohibited.

1. Add typed perforation primaries and mechanical migration of every internal q read/write.
2. Add context-to-route resolver, construction conversion, and context pass-through in assembly
   options/Newton diagnostics.
3. Add AD and legacy selected-route residual/Jacobian/source/control scatter together.
4. Add route-aware update, floor, scaling, Schur fixture, trace/report source view, and the
   selected-route exclusions from q relaxation/nested solve.
5. Remove the G4b2 pre-Newton block **only in that same commit**, after all gates in §7 pass.

Keep existing unsupported-input rejection and the native default-off option. If any part cannot
be made atomic, retain the block and split only into further design or test-infrastructure work;
do not expose a partial route.

## 7. Mandatory non-live gates

All tests use the valid 1x1x1 one-perf gas/PVT/RESV fixture unless stated otherwise. They are
gates for ResSim's internal contract, not proof of Flow trajectory parity.

| Gate | Required observation |
| --- | --- |
| Default disabled parity | existing assembler/state tests and a direct before/after fixture produce identical historical state, residual, matrix occupancy, and trace fields |
| State initialization | selected `u=Q_resv/B_g,ref>0`; initialized BHP gives `-q_res/B_g,ref=u`; other primary types/values unchanged |
| Local values | G4b1's f64/AD/central-FD contract still passes at two non-reference current `B_g` values |
| Full AD/legacy parity | residual values, sparse occupancy, and every matrix value agree for the complete selected route—not merely the three local rows |
| Complete FD | central FD over `[p,Sw,hc,bhp,u]`, away from clamps, agrees for reservoir gas, perf, and control rows; verifies source-u zero and control-BHP zero |
| Route exclusion | selected state cannot call `physical_well_control`/FB control, q relax, q reporting helper, or nested solve; nested input remains rejected before assembly |
| Scaling | the three explicit scale values are used in the appropriate units; raw residuals are unchanged by scaling |
| Schur | full-direct and Schur-recovered corrections agree at established exact tolerance |
| Trace | evaluation 0/1 captures the fields in §5 and never calls u `q`; the source reflects current `B_g`, while control reflects `B_g,ref` |
| Safety | unsupported topology/control still blocks; the valid route can pass the former G4b2 block only after the preceding gates are green |

Run focused tests while implementing, then the prescribed FIM parity and locked smoke gates for
the atomic Rust change. Record any incomplete long-running bucket as **INCONCLUSIVE coverage**;
it cannot be upgraded to a convergence verdict.

## 8. First live slice after the atomic commit

Only after §7 is green may G4b2b run one capped exact `gas-rate-10x10x3` first report step with
one initial trial and no retry. The hypothesis is narrow: after update 1, the selected source is
computed with current `B_g` but the control remains tied to `B_g,ref`, removing the known
source/control/connection inconsistency. The oracle is complete evaluation-0/1 trace plus Flow's
mapped fields—not substep count alone.

Confirming observation: the trace has matching units/row semantics, `q_res/B_g(current)` in the
source, `B_g,ref*u-Q_resv` in control, and no historical FB/q update. Refuting observation: with
all fields comparable and no retry, the coherent route still has the same first source mismatch.
Missing fields, a route fall-through, active BHP behaviour, a clamped FD point, or any retry are
**INCONCLUSIVE** and return work to the relevant gate rather than authorizing G5 or acceptance
tuning.

## 9. Handoff checklist

Before editing, the implementing agent must state:

```text
Commit tested:
Hypothesis: the atomic route removes the historical q/BHP fall-through without changing
            any disabled/historical path.
Oracle: typed-state + AD/legacy/FD + Schur + evaluation-0/1 trace gates in §7.
Confirm: every selected row/column has the §3 contract and every historical path is unchanged.
Refute: not applicable to Flow convergence; an incomplete or non-comparable route is INCONCLUSIVE.
Held/missing: retry lifetime, multi-perf, BHP switching, nested u solve, IMPES, controller,
              acceptance, damping, linear lifecycle.
```

At closeout, record exact commands, commit hash, test counts, any incomplete coverage, the status
of the pre-Newton block, and **no live performance numbers**. The registry row must be updated
before the next live step.
