# Y2d6 — Flow 2026.04 Linear-Lifecycle Design

Status: **DESIGN COMPLETE; IMPLEMENTATION NOT STARTED (2026-07-15)**

This document defines the smallest valid next experiment after Y2d5. It is deliberately more
restrictive than “try BiCGSTAB”: the exact Flow reference uses one coupled operator/preconditioner
lifecycle, and several of its pieces change the meaning of the others. A partial port is an
`INCONCLUSIVE` diagnostic, never evidence against the complete Flow lifecycle.

## 1. Source and runtime pins

The executable reference is the installed Ubuntu package
`libopm-simulators-bin 2026.04-1~noble`. The preserved gas-rate `CASE.DBG` is the authority for
the options selected by the actual run. The exact matching source tag is:

- repository: `OPM/opm-simulators`;
- tag: `release/2026.04/final`;
- commit: `b82f21dba405286c4c4446614dd3bf9cdebf7a2c` (the annotated tag object is
  `21daa7b56d1081d97a084613ca289867e2c0e70b`);
- DUNE-ISTL: `2.11.0`, matching the installed Flow package dependency.

Do not source-pin this work to the repository's newer `master` commit. Later source is useful for
orientation but is not the binary oracle.

Primary source locations at the pinned tag:

| Contract | Source |
| --- | --- |
| selected CPRW property tree | `setupPropertyTree.cpp::setupCPRW` |
| outer solver construction | `FlexibleSolver_impl.hpp::initSolver` |
| true-IMPES weights | `getQuasiImpesWeights.hpp::getTrueImpesWeights` and `ISTLSolver.hpp::getWeightsCalculator` |
| CPRW well transfer | `PressureBhpTransferPolicy.hpp` |
| two-level ordering | `twolevelmethodcpr.hh::TwoLevelMethodCpr::apply` |
| fine smoother | `StandardPreconditioners_serial.hpp` and `ParallelOverlappingILU0_impl.hpp` |
| coarse loop | `PressureSolverPolicy.hpp`, DUNE `LoopSolver`, and DUNE AMG |
| update/rebuild lifecycle | `OwningTwoLevelPreconditioner.hpp` and `ISTLSolver.hpp::prepareFlexibleSolver` |

## 2. Exact selected lifecycle

### 2.1 Outer BiCGSTAB

The exact property tree selects:

```text
solver=bicgstab
maxiter=20
tol=0.005
preconditioner.type=cprw
```

In sequential Flow, `FlexibleSolver` supplies `Dune::SeqScalarProduct`; its norm is the raw vector
Euclidean two-norm. DUNE 2.11 BiCGSTAB computes `r=b-Ax`, records `||r0||2`, and converges only
when `||r||2 < 0.005 ||r0||2` (or the absolute DUNE floor `1e-30`). It tests after both the
`alpha` half-step and the `omega` full step. The preconditioner output is cleared before each
application, so the method is right-preconditioned: `y=M^-1 p`, followed by `v=A y`.

The DUNE loop counts half-steps internally (`0.5`, `1.0`, ...), while `maxiter=20` permits twenty
complete alpha/omega pairs. The Y2d6 oracle must record both completed pairs and preconditioner
applications. It must not reinterpret `20` as ResSim's current GMRES restart length, and it must
not promote it to `30`.

Flow's relaxed post-check (`relaxed_linear_solver_reduction`, normally `1e-2`) is a separate
failure-handling layer. The primary oracle first reports the DUNE `0.005` verdict and raw norm;
any relaxed acceptance is a second explicit field, never silently folded into convergence.

### 2.2 True-IMPES restriction

For each reservoir cell, Flow computes the local storage derivatives at the current Newton state,
forms the transposed local storage block, scales its pressure column by `50e5`, solves

```text
storage_block_transpose * weight = pressure_unit_vector
```

and divides that weight vector by its largest absolute component. These are storage-derivative
weights, not an inverse or heuristic derived from the assembled diagonal Jacobian block.

The weights are recomputed whenever the CPR preconditioner is updated. Flow updates weights,
fine ILU factors, and coarse matrix values between linear solves; AMG hierarchy recreation may be
less frequent. Inside one BiCGSTAB solve, all weights, factors, and hierarchy objects are fixed.
That within-solve fixedness is the contract BiCGSTAB needs.

The current 13 captures contain the full Jacobian, RHS, layout, and equation scaling, but not the
unscaled local storage derivative blocks. Therefore they cannot reconstruct true-IMPES weights.
They remain valid matrix/RHS controls, but a literal Y2d6 oracle requires a versioned companion
payload containing either:

1. the exact normalized true-IMPES weight vector plus enough raw storage-block data to verify it;
   or
2. the raw local storage blocks, pressure-variable index, cell volume, timestep, and pressure
   scaling needed to reproduce the source formula.

Using ResSim's existing quasi-IMPES weights and calling the result “true-IMPES” is forbidden.

### 2.3 Well operator split and CPRW

This is the most important newly-pinned dependency.

Flow's outer operator includes the effect of eliminated standard-well equations, but
`paroverilu0` is constructed from the reservoir matrix returned by `getmat()`. The fine smoother
does not factor the already Schur-modified reservoir operator. CPRW separately adds well pressure
equations to the scalar coarse matrix through `addWellPressureEquations(...)`.

ResSim currently performs `well_schur::eliminate_wells` first and passes the resulting reduced
matrix to the whole CPR build. Consequently both its block-ILU0 fine smoother and its pressure
matrix see the Schur contribution through one already-modified matrix. This is not the Flow
operator/preconditioner split.

The coherent captured-system implementation must retain these objects separately:

```text
A_outer(x) = J_rr x - J_rw (J_ww^-1 (J_wr x))
M_fine     = paroverilu0(J_rr)
A_pressure = restrict(J_rr, true_impes) + explicit_reduced_well_pressure_contribution
recover dx_w only after dx_r is accepted
```

Before real captures, a synthetic block system must prove that `A_outer` equals explicit Schur
application and that the coarse well term is added exactly once. Factoring the explicit Schur
matrix in the fine smoother, omitting the coarse well term, or adding it twice invalidates the
oracle.

### 2.4 Fine `paroverilu0`

The exact property tree selects fill level `0`, traditional `ILU` (not modified ILU), relaxation
`1.0`, with `pre_smooth=0` and `post_smooth=1` at the CPR level. The factorization is block ILU0
over Flow's reservoir block matrix. Application is one lower triangular block sweep followed by
one upper triangular block sweep and inverse diagonal-block multiplication. With relaxation one,
there is no extra damping.

ResSim's `FimBlockIlu0Factors` is structurally close, but equivalence is not assumed. Gate it on a
fixed block matrix by comparing:

- occupied block sparsity retained by ILU0;
- lower/upper factors and singular-block policy;
- one application to at least three independent RHS vectors;
- the fact that it factors `J_rr`, not the explicit Schur matrix.

### 2.5 CPR ordering and one-loop AMG

The selected two-level application is:

1. zero pre-smoothing;
2. restrict the untouched fine residual with true-IMPES weights;
3. apply the scalar pressure coarse solver once;
4. prolong only the pressure correction (plus the CPRW well-pressure semantics);
5. compute the corrected fine residual and apply one post `paroverilu0` sweep.

The coarse solver is DUNE `LoopSolver(maxiter=1,tol=0.1)` with an AMG preconditioner. One loop
iteration clears its correction, applies AMG once, updates the pressure solution, and returns.
The `0.1` value can classify that one application; it does not cause a variable number of AMG
cycles. AMG itself uses the pinned property tree (`iterations=1`, `pre_smooth=1`,
`post_smooth=1`, `coarsenTarget=1200`, `alpha=1/3`, `relaxation=1`, and the remaining
`setupDuneAMG` defaults).

For a fixed assembled linear solve, this is a deterministic fixed linear application. In
contrast, ResSim's current over-threshold pressure correction runs inner BiCGSTAB until
`1e-6 * ||rhs||` or 50 iterations. The number and polynomial of inner steps depend on its input,
which caused Y2d3's recurrence violation and is incompatible with an ordinary outer BiCGSTAB
parity test.

Y2d6 is not authorization for a general AMG library. Implement only the scalar aggregation/V-cycle
surface required by the captured oracle, behind `#[cfg(test)]`, with deterministic aggregation
and source-pinned constants. If that cannot be done without a broad solver project, stop and mark
the implementation `BLOCKED`, not partially complete.

## 3. Matched, held, and missing map

| Item | Flow 2026.04 | ResSim current | Y2d6 disposition |
| --- | --- | --- | --- |
| outer method | right-preconditioned BiCGSTAB | fixed-left GMRES or default-off true FGMRES | **missing**; add test-only exact recurrence |
| outer norm | raw sequential two-norm | raw full norm exists, plus family gates | **partly matched**; oracle verdict uses raw norm first |
| budget | 20 BiCGSTAB pairs | configured 20 promoted to GMRES restart 30 | **missing**; hard 20, no promotion |
| restriction | storage-based true-IMPES | diagonal-block quasi-IMPES | **missing capture data and implementation** |
| well semantics | matrix-free outer Schur effect; well term added to CPRW coarse matrix | explicit Schur before all CPR stages | **missing operator split** |
| fine smoother | block paroverilu0 on reservoir `J_rr`, fill 0, relaxation 1 | block ILU0 on reduced Schur matrix | **algorithm close, input wrong** |
| CPR order | pressure correction then one fine post-sweep | same high-level order | **matched shape; re-gate values** |
| coarse application | one loop + one AMG application | dense inverse or tolerance-terminated BiCGSTAB+ILU0 | **missing** |
| update frequency | values/weights/factors updated per solve; hierarchy may be reused | CPR rebuilt per solve | **held initially**; one captured solve has fixed setup |
| equation scaling | Flow's native equation formulation | explicit ResSim family scaling | **intentional representation difference**; preserve and report both scaled and recovered full norms |
| fallback | Flow throws unless its relaxed reduction rule applies | multiple direct/fallback paths | **disabled in oracle**; direct solve is comparison only |
| block layout | reservoir cell blocks with well operator outside fine matrix | three cell unknowns plus explicit well tail before Schur | **map explicitly**; no scalar flattening of ILU blocks |

## 4. Coupled 13-capture oracle contract

Implementation status (2026-07-15): Gates D6a and D6b are complete. Capture v3 and its dedicated native
first-system trigger preserve the raw local storage blocks and all four reservoir/well matrix
partitions. Parser validation recomputes the pinned weights and reconstructs full `J` bit-for-bit.
Proof artifacts pass for bounded `22x22x1` (`1456` rows, 484 cells, four well rows) and exact gas
`10x10x3` (`904` rows, 300 cells, four well rows). All seven component identities also pass on
both. Their pressure systems are below `coarsenTarget=1200`, so the source-complete bounded AMG
surface is one direct coarse level, not a new aggregation implementation. Gate D6c is now the only
authorized slice.

### Gate D6a — payload sufficiency

Extend or cleanly regenerate the existing eight bounded and five gas artifacts. Each item must
carry:

- the present full `J`, RHS, layout, and equation scaling;
- raw reservoir/well block partitions sufficient for matrix-free Schur application;
- true-IMPES storage inputs and the resulting normalized weights;
- the exact Y2d6 source/config fingerprint.

Round-trip tests must reject missing or mismatched companion data. Old captures remain usable for
Y2d0-Y2d5 but are not valid Y2d6 inputs.

### Gate D6b — component identities

Pass all of these before a corpus result is printed:

1. true-IMPES recomputation matches captured weights cell by cell;
2. matrix-free outer application matches explicit Schur application;
3. coarse CPRW well contribution is present exactly once;
4. paroverilu0 applies a fixed linear map on `J_rr`;
5. one-loop AMG repeats bit-for-bit for the same RHS and satisfies linearity checks within floating
   tolerance;
6. the full CPR application follows coarse-then-post-smooth order and is fixed within one solve;
7. outer reported norm equals independent `||rhs-A_outer dx||2`.

### Gate D6c — corpus comparison

Run the coherent stack on all 13 artifacts with zero initial correction, tolerance `0.005`, and
twenty complete BiCGSTAB pairs. For every capture report:

- strict and relaxed verdicts separately;
- alpha/omega half-step, full-pair, and preconditioner-application counts;
- RHS, final full, reservoir, and recovered-well norms;
- finite status and direct-solution delta;
- current production, true-FGMRES, and Flow-lifecycle results side by side.

The minimum continuation gate is bounded `8/8`, gas `5/5`, no lost current pass, valid partitions,
and no result requiring more than Flow's twenty-pair budget. This is necessary, not sufficient,
for a live experiment. A miss is `REFUTED` only if every D6b identity passed; otherwise it is
`INCONCLUSIVE`.

### Gate D6d — only after captured success

Authorize a default-off live path only after D6a-c pass. First run the exact gas and heavy-water
references, then the Y2 water controls and Legacy guards. Do not change Newton acceptance,
timestep control, primary-variable lifecycle, or default routing in the same commit.

## 5. IMPES applicability audit

| FIM finding/fix | IMPES applicability | Action |
| --- | --- | --- |
| Bundle X connected-cell producer fractions | shared well/control physics | already unconditional and therefore already benefits IMPES; retain shared regressions |
| Y2 active-bound derivatives and `Sg`/`Rs` primary switching | no Newton primary-variable/Jacobian lifecycle in explicit IMPES transport | no port; conserved gas-inventory split remains the owning IMPES mechanism |
| Y2d0 backend-neutral full residual | IMPES direct LU already recomputes `rhs-Ax`; fallback selection also recomputes it | no numerical change; targeted solver tests remain the oracle |
| Y2d3/Y2d4 variable CPR versus fixed Krylov contract | IMPES default is direct LU; fallback BiCGSTAB uses fixed diagonal Jacobi | no CPR/FGMRES change |
| Y2d6 well/operator split | IMPES solves a pressure-only assembled system with wells already represented in diagonal/RHS and has no coupled well unknown tail | no port; applying FIM CPRW would change the IMPES equation system |
| iteration-accounting audit | IMPES fallback BiCGSTAB recognized convergence at a loop boundary after incrementing the next iteration | corrected in `solvers/bicgstab.rs`; solution unchanged, completed-iteration reporting fixed |

The IMPES audit found no masked convergence fix analogous to the FIM lifecycle work. Its one real
shared observation is diagnostic: fallback iteration counts must mean completed corrections. A
focused nonsymmetric-system regression now proves one completed correction is reported as one and
independently verifies the returned raw residual.

## 6. Prescriptive handoff

Gates D6a and D6b were completed on 2026-07-15. The next implementing agent must do only Gate D6c:

1. regenerate or extend exactly eight bounded and five gas artifacts in capture v3 format;
2. run all seven D6b identities before each solve and stop that artifact as `INCONCLUSIVE` if any
   identity fails;
3. add the exact DUNE 2.11 BiCGSTAB recurrence with raw sequential two-norm, zero initial update,
   strict `0.005` reduction, and at most twenty complete alpha/omega pairs;
4. report half-step/full-pair/preconditioner counts, full/reservoir/recovered-well residuals,
   finite status, direct delta, and current-production/true-FGMRES/Flow-stack results side by side;
5. require bounded `8/8`, gas `5/5`, and no loss of an existing pass before considering D6d.

Do not add a live option or change production dispatch during D6c. Do not extend the one-level
coarse oracle into a general AMG project; these captures are below Flow's coarsening threshold.
