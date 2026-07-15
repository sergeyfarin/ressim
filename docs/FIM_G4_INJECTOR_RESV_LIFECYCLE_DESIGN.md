# G4a — Flow Injector-RESV Lifecycle Design

Status: **DESIGN COMPLETE; NO BEHAVIOUR CHANGE (2026-07-15)**

This is the only authorized design after Y2d8. It defines a narrow, default-off,
source-comparable probe for the exact `gas-rate-10x10x3` Flow case. It is not permission to
freeze one ResSim source coefficient, promote `nested_well_solve`, alter Newton acceptance, or
port Flow's general well model.

The causal question is: does representing a single-perforation gas `RESV` injector by Flow's
surface-rate primary, report-step regional conversion, connection equation, and source term
remove the evaluation-1 source mismatch while preserving a coherent residual/Jacobian?

Until every oracle in §6 passes, this work is **DIAGNOSTIC**, not a convergence fix. A better
substep count alone cannot promote it.

## 1. Source pin and measured oracle

The runtime oracle is Flow `2026.04` and the tracked
`opm/reference-decks/gas-rate-10x10x3/CASE.DATA` deck (`WCONINJE ... GAS RESV 500.0`). The
release/binary source pin is in `docs/FIM_Y2D6_FLOW_LINEAR_LIFECYCLE_DESIGN.md` §1; the local
OPM checkout is source orientation, not the runtime authority.

| Flow contract | Source | Required meaning |
| --- | --- | --- |
| Injector primary | `StandardWellPrimaryVariables::{update,getQs}` | Injector `WQTotal` is injected-phase **surface** rate; ordinary gas gives `getQs(gas)=WQTotal`. |
| RESV control | `WellAssemble::assembleControlEqInj` | `B_g,ref * WQTotal - Q_resv = 0`; the coefficient comes from `RateConverter::calcInjCoeff`. |
| Reference lifetime | `BlackoilWellModel::{beginReportStep,beginTimeStep}` and accepted-step refresh | `RateConverter::defineState` is created at report-step entry and refreshed after acceptance, not per Newton evaluation. |
| Connection | `StandardWell::computePerfRate` | Current-state standard-condition connection rate uses current local drawdown, total mobility, `invB`, and surface mixing. |
| Source | `StandardWell::assembleWellEq` / `BlackoilWellModel::addReservoirSourceTerms` | The same standard-condition connection rate is used in well equations and reservoir source with reservoir sign. |
| Rate update | `StandardWellPrimaryVariables::updateNewton` | The rate is updated as a surface unknown with injector sign restriction, not overwritten by a reservoir-q relaxation. |

At the deck's uniform initial regional state, `B_g,ref=0.0065 rm3/Sm3`. Flow's controlled source
is consequently `-500/0.0065=-76,923.076923 Sm3/day` at evaluations 0 and 1. Y2d8 observed
nested ResSim instead use local `B_g=0.005219627384` after update 1 and emit
`-95,792.278488 Sm3/day`: an excess `-18,869.201565 Sm3/day`, or `-4,717.300391` in the
0.25-day gas residual row. The comparison is valid because Flow's evaluation-1 well status is
converged; it does not assert every intermediate Flow well value is constant.

## 2. Current ResSim representation and the invalid shortcut

`FimState::perforation_rates_m3_day` is a signed local **reservoir** connection rate: injector
q is negative. `FimState::from_simulator` initializes it from `connection_rate_for_bhp`, and
`wells_ad::connection_rate_generic` supplies the same local-current law to the perforation row.
The gas source is `q/B_g(cell)` in both `wells::perforation_component_rates_sc_day` and
`wells_ad::component_rate_coefficients_generic`. Current RESV control aggregates `-q`; current
surface control aggregates `(-q)/B_g(cell)` through a Fischer--Burmeister BHP/rate switch.

Those current rows are internally consistent but not Flow:

```text
q_res - q_connection_res(p,bhp) = 0
Q_resv - (-q_res)                = 0
source_gas = q_res / B_g(cell)
```

Changing only `source_gas` to use a frozen FVF would leave the primary, connection row, control
row, and source in different units and with incompatible derivatives. It is an invalid oracle and
is prohibited.

## 3. Dependency table

| Dependency | Flow | Current ResSim | G4b single-perf probe | Status |
| --- | --- | --- | --- | --- |
| Rate unknown | Per-well positive surface `WQTotal` | Perforation negative reservoir q | New opt-in positive gas-surface `u`; never silently reinterpret the general q field | Matched only for one gas injector / one perf |
| BHP | Well BHP | Well BHP | Retain current BHP unknown/offset | Held |
| RESV row | `B_g,ref*u-Q_resv` | `Q_resv-(-q)` within FB | Frozen-reference control row; BHP limit must be inactive | Rate row matched; active switch held |
| Conversion lifetime | Regional hydrocarbon-PV weighted state, report-step lifetime | No such context | Immutable `FlowResvReference` before scoped report step | Matched only for no-cut, one-region probe |
| Accepted refresh | Refresh after accepted timestep | State rebuilt from simulator after accepted FIM substep | Refresh only after acceptance, never in Newton | Candidate, unit-tested |
| Retry lifetime | Must be sourced separately | Retries reuse `previous_state` | Retry makes initial live result INCONCLUSIVE | Missing by design |
| Connection | Current standard-condition connection rate | Current reservoir connection law | `c_s(p,bhp)-u=0`, where `c_s=-q_res/B_g(cell)` | Matched for pure gas |
| Source | Negative current standard-condition connection rate | `q/B_g(cell)` | `S_gas=-c_s=q_res/B_g(cell)` | Matched for pure gas |
| Aggregation | One `WQTotal`, possibly many perforations | One q per perforation | Exactly one open perforation required | Missing outside scope |
| Control switch | Flow active control mechanism | Smooth FB | Assert BHP branch inactive; retain FB only if its rate branch is exact | Held/missing |
| Update/local solve | Surface-rate update | q update + Relax/NestedSolve | Raw u update/sign floor; q-coordinate relax/nested solve incompatible | Missing / disabled |
| Linear layout | StandardWell block | Explicit BHP/perf tail | Same one BHP + one rate column; re-gate AD/legacy/Schur | Structurally held |
| Y2 primary lifecycle | Existing Flow lifecycle | Complete tagged lifecycle | Hold exactly as Y2d8 | Held |
| IMPES | No FIM well tail | Separate split | No code change | Not applicable |

Every matched row above is a package. The option may not select the new control/primary in
`assembly_ad` while legacy assembly, diagnostics, rate reporting, or local well code still use a
different connection/source formulation.

## 4. Exact scoped equations and signs

G4b applies only to an enabled, rate-controlled, single-perforation ordinary gas injector with
explicit `RESV`, one FIP/PVT region, no solvent/multisegment well, and inactive BHP limit. Let
`u>=0` be surface gas rate (Sm3/day), `q_res(p,bhp)<=0` the current reservoir connection rate
(rm3/day), `B_g(p,state)` current local FVF, `B_g,ref>0` frozen regional FVF, and `Q_resv>=0`
the scheduled target. Assemble:

```text
c_s    = -q_res(p,bhp) / B_g(p,state)
R_perf = c_s - u                         = 0
R_ctrl = B_g,ref * u - Q_resv             = 0
S_gas  = -c_s
```

For pure gas, Flow's injection branch gives standard-condition connection `cq_s=-q_res/B_g`:
`volumeRatio=B_g` and `cq_s=cqt_i/volumeRatio`. The source is this current connection rate with
reservoir sign, while the perforation equation enforces `cq_s=u`. Thus current local FVF
derivatives belong in **both** the connection row and source; the frozen `B_g,ref` belongs only
in the RESV control row. At a converged well, `c_s=u=Q_resv/B_g,ref`, which is why Y2d8's
evaluation-1 source comparison is valid. This is why a source-only freeze and a q field without
an explicit mode tag are both wrong.

## 5. Prescriptive G4b sequence

1. **G4b0: representation only.** Add explicit parsed `RESV` control kind and native-only,
   default-false `FIM_FLOW_RESV_INJECTOR=1`. Add immutable `FlowResvReference` to FIM
   report-step/attempt context, not a cell or mutable global. Calculate scoped regional
   hydrocarbon-PV-weighted reference. Unit-test constructor, accepted refresh, and retry rollback.
   Reject rather than approximate multi-perf, non-gas, surface-rate, BHP-active, multi-region,
   or `FIM_NESTED_WELL_SOLVE` requests. Do not route assembly.
2. **G4b1: shared residual contract.** Build one AD + f64 pure function for §4. Test it at two
   pressures where `B_g != B_g,ref`: `dS/dp` and `dR_perf/dp` retain the current
   connection/FVF derivative, `dS/du=0`, and `dR_ctrl/du=B_g,ref`. Do not route production
   assembly yet.
3. **G4b2: atomic FIM route.** Select the same mode in AD and legacy assembly, source helper,
   scaling, diagnostics, reporting, and state update. Initialize/update positive u and do not call
   q-coordinate Relax/NestedSolve for that injector. The FB rate branch may remain only after an
   equality test proves it supplies exact `R_ctrl`; otherwise use an explicit rate-only test row.
4. **G4b3: local/global coupling.** Re-derive a u-coordinate local solve before supporting nested
   mode. Prove its rows/columns equal global assembly (Bundle W invariant). Capture evaluation 0/1
   source/control/connection partitions before a six-step run.
5. **G4b4: live gates.** First run one capped no-retry rung; only then run six steps and compare
   cuts, applied updates versus Flow `7,5,4,3,4,3`, row partitions, and wall time. This remains a
   default-off behavior result, not promotion.

## 6. Mandatory oracle and commands

The one-cell pure-gas fixture uses `Q_resv=500`, `B_g,ref=0.0065`, and evaluation
`B_g(cell)=0.005219627384`. At `u=500/0.0065`, require:

| Quantity | Required result |
| --- | --- |
| control residual | zero to numerical tolerance |
| source | `-76,923.076923 Sm3/day`, not `-95,792.278488` |
| source pressure/BHP derivatives | equal negative current connection-rate derivatives |
| source u-column | zero before dt scaling |
| control u-column | `0.0065` |
| connection row | `-q_res/B_g(cell)-u` with nonzero local-property derivatives |
| AD/central FD and legacy/AD | agree away from a clamp |

With the option unset, the old default must remain bit-identical. Evaluation-1 trace must include
u, current `c_s`, `B_g,ref`, local `B_g`, control residual, connection residual, source, source
rate-column, and source pressure-column. It must show `c_s≈u` before comparing source to
`-76,923.076923`. Missing fields, non-comparable well status, active BHP constraint, or a retry
are **INCONCLUSIVE**, never a refutation.

These are G4b commands, not commands executed by G4a:

```text
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  fim::wells_ad::tests::flow_resv_injector -- --nocapture

FIM_TRACE_FILE=/tmp/ressim-g4b.trace FIM_TRACE_DT_BELOW=1 \
FIM_Y1J_GRID=10 FIM_Y1J_FLAVOR=opm FIM_Y1J_STEPS=1 \
FIM_Y2B_RAW_SATURATION=1 FIM_FLOW_RESV_INJECTOR=1 \
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  fim::timestep::phase5_repro::repro_gas_rate_10x10x3_y1j -- --ignored --nocapture --exact
```

The command must not set `FIM_NESTED_WELL_SOLVE` until G4b3 makes it u-coordinate compatible.

## 7. Scope, promotion, and IMPES

This design does not prove general multi-perf, multiphase, water/oil, solvent, group, or
BHP-limited Flow well parity. The one-perf restriction is structural: Flow has one WQTotal per
well, ResSim currently has one q-like unknown per perforation. Promotion needs a separate design
for aggregation, active switching, retry semantics, and the bounded/heavy matrix.

No IMPES change is authorized. The proposed change is FIM-tail representation. If G4b extracts a
genuinely shared injector conversion primitive, run an IMPES focused source/FD regression in that
same commit; do not infer an IMPES convergence benefit.

## 8. Closeout

Commit tested: `be6326c` (Y2d8 source audit). Oracle validity: **VALID** for one-perf evaluation-1
source comparison, **not yet a live behavior oracle**. Retry, aggregation, active control
switching, and u-coordinate nested solve remain unimplemented. Verdict: **DESIGN COMPLETE;
G4b0 authorized.**
