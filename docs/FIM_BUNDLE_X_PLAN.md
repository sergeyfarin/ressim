# FIM Bundle X: Well-Update Ordering (the H1 structural fix)

Status: PLANNED (2026-07-12). Registry: `FIM-BUNDLE-X` (OPEN).
Prerequisite evidence: `docs/FIM_DIAG_003_PLAN.md` (closed 2026-07-12, D0-D5) — H1 confirmed by
three independent methods, H2/H3 refuted. This bundle is the "scoped fix bundle for the
confirmed mechanism" that D5 named as the next unit of work. `FIM-BUNDLE-W`'s retry condition
("retry only after FIM-DIAG-003 ... and, if fixable, a fix — re-run W4's §5 gate with both in
place") is satisfied by this plan's X3.

## 1. The structural defect, stated precisely

Both ResSim (`OpmAligned`+`nested_well_solve`) and OPM perform the same three operations per
Newton iteration — (a) enforce well-equation consistency at fixed reservoir state, (b) linearize
and solve the coupled system, (c) apply an update — but in different orders, and the order is
the defect:

**OPM** (pinned `062cb1998`, verified this session):
1. `assembleWellEq` → `prepareWellBeforeAssembling` → `iterateWellEquations`
   (`WellInterface_impl.hpp:973`, `:1032`, `:1066`): the converged inner well solve runs
   **before** the well equations are linearized, gated by
   `shouldRunInnerWellIterations(max_niter_inner_well_iter_)` — default
   `MaxNewtonIterationsWithInnerWellIterations=99`, i.e. every iteration. The linearization is
   therefore built at a well-consistent point.
2. Coupled linear solve (wells Schur-eliminated).
3. `recoverWellSolutionAndUpdateWellState` (`StandardWell_impl.hpp:1489-1500`):
   `linSys_.recoverSolutionWell(x, xw)` back-substitutes the reservoir update through the well
   block — the well update **consistent with the coupled linearization** — then
   `updateWellState` applies it (with OPM's own chopping). **Nothing re-solves the wells after
   application within the iteration.**

**ResSim + `nested_well_solve`** (`fim/state.rs:408-457`):
1. Assemble (wells at whatever state the previous iteration's projection left — usually
   consistent, since the projection ran last iteration).
2. Coupled linear solve (wells are primary unknowns with their own rows).
3. `apply_raw_update` applies cells **and** wells, then `WellStateUpdateMode::NestedSolve`
   **re-solves the wells against the new reservoir state, overwriting the coupled `dq`/`dbhp`**
   (`state.rs:445-453`).

Step 3's override is the invariant-point generator `FIM-DIAG-003` measured: the recorded
signature is `raw_dq ≈ +0.58` proposed by the coupled solve every iteration, nested contribution
`≈ −0.58` (near-exact negative), `q` stable to 8 decimals, whole state frozen at 4 significant
figures for 18 iterations, frozen MB living at the producer's perforation cell (D1: 91% cell
143, 9% cell 130). The coupled solve zeroes the well-cell mass-balance rows *through* the
`dq` source term; the override discards `dq` after the cell moves were already computed against
it; the promised source change never materializes; the residual returns to the same value; the
map repeats bit-identically. OPM's ordering makes this fixed point structurally impossible: the
applied well update IS the coupled one, and the pre-assembly inner solve is (at a
first-order-consistent point) at most a second-order cleanup, not a veto.

## 2. The honest open question X0 must answer before the fix is coded

If the linear solution satisfied the linearized `rate_consistency` row exactly, the nested
solve's correction should be **second-order small** — the row ties `dq` to
`WI·λ·(dp_cell − dbhp)` plus mobility-derivative terms, and the nested solve re-solves the same
equation nonlinearly at the updated state. Yet D1's forced-direct run (exact `SparseLuDebug`
solves, all rows satisfied to machine precision) still froze (MB `2.3e-7`, same cells). So
somewhere between "linear solution returned" and "nested solve runs", first-order consistency
is broken. The suspects, in application order (`newton.rs` → `state.rs`):

1. the OSC-DETECT relaxation scalar / Appleyard damping composition (uniform scaling should be
   consistency-preserving — verify, don't assume);
2. `opm_per_cell_chopped_update` (per-cell saturation/pressure chop + BHP chop — a chop that
   binds on any well-cell entry breaks the promised `dp_cell` while `dq` was computed against
   the unchopped value... but note under `NestedSolve` the applied `dq` is then overridden
   anyway; what matters is which stage first diverges the *cell* part);
3. `enforce_cell_bounds` (`state.rs:435-437`) — clamps at the well cell;
4. `enforce_control_bounds` (`state.rs:438`) — BHP/rate clamps (recorded `raw_dbhp = 0.0`
   exactly, so BHP clamping is unlikely to be active, but the producer sits at its BHP limit —
   verify);
5. the nested solve itself (if 1-4 are all inert, the disagreement is inside the
   `rate_consistency` linearization's cell-state couplings — a Jacobian-fidelity question that
   would redirect this bundle).

X0 instruments exactly this: one windowed capped run logging, at each iteration of a frozen
substep, the well-relevant components (`dp`/`dsw` at cells 143/130, `dq`, `dbhp`) of the update
vector at each stage: raw solution → after relaxation/damping → after chop → after cell bounds
→ after control bounds → after nested solve. Whichever stage first changes the cell-143 block
or the `dq` entry by more than second order is the mechanism. No fix is coded until this is
measured (`FIM-DIAG-002` discipline; the D1-D5 evidence localizes the defect to this pipeline
but not to the stage).

## 3. Checkpoints

### X0 — stage-by-stage first-order-consistency forensics (~70s run + analysis)

Window-gated trace line (same `FIM_TRACE_SUBSTEP_START` machinery as D1) dumping the stages
above. Native-only, no wasm surface, no-op when the trace window is inactive. Gates: trace off
⇒ control matrix bit-identical + locked smoke 3/3.

### X1 — cheapest structural probe: pure coupled well update (never live-tested)

`WellStateUpdateMode::None` is currently test-only (`state.rs:387`) — the matrix cell "trust
the coupled system's well update, no post-application override at all" has **never been run
live** (Legacy always had `Relax`; Bundle W replaced `Relax` with `NestedSolve`; nobody ran
`None`). This is also the closest analog of OPM's step 3 for a system that carries well
unknowns un-eliminated: when the coupled system is solved with well rows in it, the returned
well entries ARE the back-substituted values `recoverSolutionWell` would produce.

One flag (`FIM_WELL_UPDATE_NONE` env on the native driver, or a third
`well_update_mode` value threaded like `nested_well_solve`), one ~70s capped heavy run
(`OpmAligned`, `FIM_TRACE_SUBSTEP_START≈980`, `FIM_MAX_SUBSTEPS=1000`), plus the D0
binding-criterion trace:
- Freeze breaks (MB drops below `1e-7`, `advanced_dt` materially past `0.92`) ⇒ the override
  alone is the whole defect; X2 becomes primarily an OPM-fidelity refinement (the pre-assembly
  inner solve for robustness on harder cases), not the fix itself.
- Freeze persists ⇒ combined with X0's stage attribution, the defect is upstream of the
  override (chop/damping/bounds or `rate_consistency` Jacobian fidelity) — re-plan within this
  bundle before touching ordering.
- Wells drift/diverge early in the run (the risk `Relax` was originally added for) ⇒ X2's
  pre-assembly inner solve is load-bearing, not optional; proceed to X2 directly.

`all_wells_converged` (the W3 outer acceptance gate) stays active in every variant — acceptance
never widens (`FIM-NEWTON-005` lesson).

### X2 — the OPM ordering, flag-gated

Move the well-consistency enforcement to OPM's position:
1. At the top of each Newton iteration, **before assembly** (the
   `prepareWellBeforeAssembling` analog): run `solve_wells_locally` against the current
   reservoir state. Also once at substep entry (OPM's `SolveWelleqInitially=1`).
2. After the linear solve: apply the coupled update's well components as returned (mode `None`
   semantics + `enforce_control_bounds`). The `NestedSolve` post-update override is **removed**
   from the applied path (the mode stays in the enum for the W1 agreement tests).

Flag-gated (`nested_well_solve` gains a mode or a sibling flag — decide at implementation
against wasm-surface minimalism), default off, Legacy path bit-identical. Unit test for the
ordering (well solve mutates state before assembly sees it); W1 agreement tests unchanged.

Gates: flag off ⇒ control matrix bit-identical + locked smoke 3/3 + `assembly_ad` parity 10/10
+ wasm build green.

### X3 — evaluation, then the stack gate

1. Capped heavy run (X1's economics): binding-margin + substeps-to-`t=0.9` vs the D1 baseline
   (`advanced_dt=0.9168`, MB frozen `1.41e-7`).
2. If the capped run clears: full uncapped §5 heavy run vs the stack baseline
   (`accepted_substeps=18,015` @ `c916c87`) — target the original `≤35`-substep class.
3. Verification oracle: the D3 deck (`opm/reference-decks/water-heavy-step1/`) INFOITER
   trajectory — ResSim's per-iteration MB should now transit the `1e-7`-`2e-7` zone the way
   OPM's does (one clean step, not 18 frozen ones).
4. Stack-level promotion decision (the original Bundle N §5 gate: heavy `≤35`-class + fine-dt
   FOPT + control matrix + bounded cases not worse than Legacy). This is where
   `OpmAligned`+well-ordering either promotes as a stack or the program re-plans honestly.

### Fallback (own future bundle, only if X0-X2 refute the ordering thesis)

OPM's well primary-variable structure itself: per-well `WQTotal`/fractions/`bhp` unknowns with
connection rates *derived* at assembly (no per-perforation `q` unknowns, no `rate_consistency`
rows). Large — a rewrite of `wells_ad.rs`/`assembly_ad.rs` well blocks. Not attempted while a
small ordering fix remains unfalsified.

## 4. Cost estimate

X0 ~1-2h (trace + one capped run + analysis); X1 ~1h (flag + capped run); X2 ~3-5h
(restructure + tests + gates); X3 ~1h capped, +~25min full heavy run, +gates if promoting.
Everything except X3's full run fits capped-run economics.

## 5. Documentation consequences

- Worklog entry per checkpoint, numbers verbatim; registry `FIM-BUNDLE-X` updated with the
  verdict either way.
- If X0 finds a chop/damping/bounds stage breaking first-order consistency: that finding gets
  its own registry row (it affects every case, not just the heavy one).
- If X3 promotes: `docs/FIM_STATUS.md` gap #4 closes; `FIM-BUNDLE-W`'s row gains its "re-run
  with both in place" resolution; the stack baseline `18,015` is superseded (say so explicitly,
  with the replay command).
