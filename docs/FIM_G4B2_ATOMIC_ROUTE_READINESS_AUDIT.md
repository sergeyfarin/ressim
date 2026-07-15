# G4b2 — Flow RESV Atomic Route Readiness Audit

Status: **COMPLETE — ROUTING BLOCKED PENDING ATOMIC IMPLEMENTATION DESIGN (2026-07-15)**

This audit follows G4b0's representation/context and G4b1's local mathematical contract. It
does not authorize a partial assembler edit or a live comparison. Its purpose is to enumerate the
coupled paths that must change together for the one-perforation gas `RESV` injector.

## Finding that changed the safety boundary

`WellScheduleControl::Resv` is accepted by schedule validation, but current
`physical_well_control()` recognizes only explicit `rate`. Without a guard, an opt-in RESV run
would pass the captured context into Newton while the real well assembly silently selected the
historical BHP/q path. That result would not exercise the G4 equations and would be an invalid
Flow oracle.

G4b2 therefore adds a structural pre-Newton block. A valid scoped context now leaves time
unchanged and emits `captured but not executable until G4b2 atomic routing`. The regression test
uses the valid one-perf gas/PVT/RESV fixture, not an unsupported-input shortcut.

## Coupled route inventory

| Route | Current implementation | Required atomic RESV action | Gate before live use |
| --- | --- | --- | --- |
| Activation/context | `fim/timestep.rs` builds `FlowResvReportStepContext`; `FimNewtonOptions` carries it but assemblers cannot read it | Replace the block only when one immutable context is passed explicitly to both assembly paths and trace/report paths | Valid RESV flag must select no historical q/BHP row; unsupported forms still return before Newton |
| State/primary | `FimState::perforation_rates_m3_day` is one signed **reservoir** q per perforation; initialization solves BHP then initializes q from local connection | Add an explicit selected-perf primary kind and positive surface u initialization `Q_resv/B_g,ref`; do not reinterpret other q entries | Exact offsets/counts preserved; selected u starts positive, nonselected wells remain bit-identical |
| Control selection | `physical_well_control()` treats only `rate` as rate-controlled; `RESV` becomes BHP; `well_constraint_residual_fb_generic()` is FB BHP/rate switching | Add a scoped RESV control variant and an explicit inactive-BHP rate-only `R_ctrl=B_g,ref*u-Q_resv`, not a reused FB branch without equality proof | f64/AD/legacy equality, `dR_ctrl/du=B_g,ref`, zero BHP column, no implicit BHP activation |
| Perforation row | AD uses `q-connection_rate_generic`; legacy uses `q-connection_rate_for_bhp` | Route selected row through G4b1's `R_perf=-q_res/B_g(current)-u` in both paths | Current-FVF p/BHP columns and u column `-1` agree with AD/FD and legacy |
| Reservoir source | AD uses `component_rate_coefficients_generic * q`; legacy/reporting use `perforation_component_rates_sc_day(q/B_g)` | Route gas source through G4b1 `S=-c_s=q_res/B_g(current)`, retaining current property derivatives and no u column | AD/legacy/source helper/trace agree; source-u column zero; source p/BHP columns are negative connection columns |
| Jacobian/legacy oracle | Live AD is `assembly_ad.rs`; `assembly.rs` is the test-only bit-parity oracle | Change residual and every matching Jacobian scatter in both in the same commit | Complete RESV matrix value/occupancy parity and central FD, then existing assembly parity suite |
| Update/local solve | Raw update blindly increments q; `Relax` recomputes/clamps signed q; `NestedSolve` is q-coordinate FB local Newton | Update u as a positive primary with an explicit u floor/chop; skip q relax; keep nested mode rejected until G4b3 | Raw/global update contract; no selected u is written by q relaxation; nested request remains rejected |
| Scaling/acceptance | `well_constraint_scale` assumes FB or BHP; perf/variable scales use `|q|`; family diagnostics label generic well/perf rows | Specify scaled units for `R_ctrl` (rm3/day) and `R_perf` (Sm3/day), surface-u variable scale, and trace labels | Scaled residual families and convergence checks match raw rows; no changed acceptance policy |
| Linear tail | `FimLinearBlockLayout` and `well_schur` treat BHP + perforation tail generically | Keep tail dimensions/offsets, but prove the new 2x2 local block survives Schur elimination/recovery exactly | Direct full versus Schur correction equality for the RESV fixture |
| Diagnostics/reporting | `WELLSOURCE`, well detail, `FimStepStats`, and FIM rates call q-based helpers/names | Add `u`, `c_s`, `B_g,ref`, current `B_g`, control/perf residuals, source p/u columns; do not label u as q | Evaluation-0/1 trace is complete and source-comparable before any six-step claim |
| Retry/accepted refresh | Context helper has retry-copy/accepted-refresh unit tests; executable route is now blocked | Source Flow's retry lifetime separately; define when the reference refreshes relative to accepted FIM substeps/reports | First live rung must have no retry; any retry is `INCONCLUSIVE` for Flow comparison |
| IMPES/public API | IMPES has no FIM well tail; RESV has no public schedule setter | Keep unchanged | No IMPES claim or product API change in G4b2 |

## Required atomic implementation order

The next code slice must be one reviewable atomic route, not a sequence of partial experiments:

1. Add explicit selected-perf u primary/control metadata to `FimState` and `physical_well_control`.
2. Thread immutable context through `FimAssemblyOptions` into **both** AD and legacy assembly.
3. Replace control, perforation, source residuals and all corresponding Jacobian scatters together.
4. Replace q-only update/relax behavior for that selected primary; keep nested solve rejected.
5. Add scaling, Schur, reporting, and source-trace routes in the same change.
6. Gate with local value/AD/FD, full AD/legacy parity, local/global update, direct/Schur equality,
   and evaluation-0/1 trace completeness before the first capped no-retry rung.

No item above may be promoted alone. The existing G4b1 helper is the common residual primitive;
duplicating its equations in an assembler or a trace would recreate the source/connection split
that G4 was designed to remove.
