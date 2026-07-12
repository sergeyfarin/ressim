# FIM Bundle X: Well-Update Ordering / Producer-Fraction Fidelity (the H1 structural fix)

Status: X0 COMPLETE (2026-07-12) — **root-cause hypothesis revised**, see §1a. Registry:
`FIM-BUNDLE-X` (OPEN).
Prerequisite evidence: `docs/FIM_DIAG_003_PLAN.md` (closed 2026-07-12, D0-D5) — H1 confirmed by
three independent methods, H2/H3 refuted. This bundle is the "scoped fix bundle for the
confirmed mechanism" that D5 named as the next unit of work. `FIM-BUNDLE-W`'s retry condition
("retry only after FIM-DIAG-003 ... and, if fixable, a fix — re-run W4's §5 gate with both in
place") is satisfied by this plan's X3.

## 1a. X0 result: a different, more precise root cause than originally planned

X0 (`docs/FIM_CONVERGENCE_WORKLOG.md` "Bundle X checkpoint X0") measured, rather than assumed,
where first-order consistency breaks — and the answer eliminates §1/§2's original suspects
(relaxation, chop, bounds enforcement, `NestedSolve`'s ordering) as the *primary* mechanism and
identifies a different one:

**`producer_fractions_generic` (`fim/wells_ad.rs:52-81`) is fed a 3x3 areal-neighborhood window
around every producer perforation (`perforation_control_cells`, `fim/wells.rs:822-838` — the
single shared call site for both the coupled assembly, `assembly_ad.rs`, and the nested solve,
`wells_inner.rs`), not just the perforated cell itself.** Injectors already get the single-cell
treatment (`if perforation.injector { return vec![perforation.cell_index]; }`); only the
producer branch pulls in neighbors. At the heavy case's producer (cell 143, corner, pre-
breakthrough, `sw` pinned exactly at the connate floor `s_wc=0.1` with `krw=0` there exactly),
the 3x3 window includes near-front neighbors (cell 130 among them — D1's secondary binding cell)
whose slightly-elevated `sw` leaks nonzero water mobility into the *well's* aggregate
`water_fraction`. That manufactured water withdrawal is then debited entirely against cell 143's
own water balance — a residual (`water=+1.696e-3` raw) with no available fix: `dsw` is the only
lever with meaningful sensitivity there (`d/dsw≈±20` vs `d/dp≈1e-3`-`1e-6`, `d/dq≈1e-5`-`1e-7`)
and `dsw` is legitimately clamped at the connate floor every iteration
(`enforce_cell_bounds`, unconditional, before any `WellStateUpdateMode` branch — so this fires
identically under `Relax`/`NestedSolve`/`None`).

OPM's `WellInterface::getMobility` (`WellInterface_impl.hpp:2105-2143`, pinned `062cb1998`) uses
**only** the single connected cell's own mobility for every well, producer or injector, no
neighborhood — confirmed by reading the source, not inferred. OPM's equivalent producer would
compute `water_fraction=0` exactly and never manufacture this residual. This directly explains
D3's oracle finding (OPM transits the same MB magnitude cleanly).

**Consequence for this plan**: the well-update-ordering hypothesis (originally X1/X2) is
downgraded to secondary/contingent — real (the `dq` veto D1/W measured is genuine), but
downstream of, not the cause of, the frozen residual. The new primary candidate fix is narrower,
more surgical, and different in kind: restrict `perforation_control_cells`'s producer branch to
the single perforated cell. It is also **broader-reaching** than an ordering change — it alters
producer water-cut/GOR physics for every FIM case with a producer under any control mode, not
just the heavy case — so it needs the full control-matrix + locked-smoke + BL-benchmark gate
before any promotion decision, not just a capped heavy-case check. Origin note: the 3x3 design
traces to `d824f4f` ("Add producer control state management..."), part of the original pre-FIM,
pre-OPM-alignment simulator; `wells_ad.rs` faithfully mirrored it into the FIM/AD layer without
re-examining it against OPM. Not a deliberate OPM-motivated choice — an inherited, unexamined
divergence.

The checkpoints below (X1-X3) are retargeted accordingly. §2's original suspects (relaxation,
chop, bounds, `NestedSolve` ordering) remain correctly ruled out/measured by X0 and do not need
re-investigation.

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

### X1 — single-cell producer fraction, capped probe (retargeted 2026-07-12 per X0)

Change `perforation_control_cells` (`fim/wells.rs:822-838`) to return `vec![perforation.cell_index]`
for producers too, matching the injector branch (already single-cell) and OPM's `getMobility`
exactly. This is the one shared call site (`assembly_ad.rs`, `wells_inner.rs` both go through
`control_influence_cells` → `perforation_control_cells`) — a single, well-scoped change.

Flag-gate it first for the capped probe (env var on the native driver, e.g.
`FIM_SINGLE_CELL_PRODUCER_FRACTION`, or a `ReservoirSimulator` field threaded the same way as
`fim_force_direct_linear` — decide at implementation; native-diagnostic-only is enough for this
checkpoint, promotion to a permanent change happens at X3): one ~70s capped heavy run
(`OpmAligned`+`nested_well_solve`, `FIM_TRACE_SUBSTEP_START≈980`, `FIM_MAX_SUBSTEPS=1000`), plus
the D0 binding-criterion trace and the new `WELLJAC`/`WELLJAC-WATER` X0 trace:
- Freeze breaks (MB drops below `1e-7`, `advanced_dt` materially past `0.92`, `WELLJAC-WATER`'s
  `well=` term at cell 143 drops to ~0) ⇒ X0's diagnosis confirmed live, proceed to X3's full
  gate on this fix.
- Freeze persists but the `well=` term at cell 143 is now ~0 (confirms the fraction fix landed)
  ⇒ a second, still-hidden mechanism remains — re-open X0-style forensics rather than guessing.
- Freeze persists and `well=` is unchanged ⇒ the fix didn't take (wiring bug) or the hypothesis
  itself needs re-examination — do not proceed to X3 blind.

Also re-run the two bounded no-op cases the same way D1/D4 checked nested_well_solve
(`22x22x1`/`23x23x1` under `OpmAligned`) — this change touches every producer's phase split, so
even the capped probe should sanity-check it isn't silently breaking a currently-passing case.

### X2 — well-update-ordering (secondary/contingent, only if X1 does not fully resolve it)

The original §1/§2 ordering hypothesis, kept as a fallback: move the well-consistency
enforcement to OPM's position (pre-assembly inner solve, apply the coupled update's well
components as-returned, no post-update override). Only pursue this if X1 leaves a residual
freeze after the fraction fix — X0 already showed the `dq` veto is real but secondary, so it may
still matter as a second-order cleanup once the primary (fraction) defect is fixed. Design
unchanged from the original plan (see §1/§2 above for the OPM-vs-ResSim ordering comparison and
the `WellStateUpdateMode::None` probe), flag-gated, default off, Legacy path bit-identical.

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
4. **Full control-matrix + locked-smoke + BL-benchmark gate** (not just the heavy case) — the
   producer-fraction fix changes water-cut/GOR physics for every FIM producer, broader-reaching
   than a pure ordering change. Compare reported production numbers on at least one case with a
   producer that sees breakthrough mid-run (e.g. `water-medium-6step`), not just pre-breakthrough
   cases, to confirm the fix doesn't regress the *converged*, physically-correct answer once
   water genuinely should be co-produced.
5. Stack-level promotion decision (the original Bundle N §5 gate: heavy `≤35`-class + fine-dt
   FOPT + control matrix + bounded cases not worse than Legacy). This is where the fix either
   promotes (independent of `nested_well_solve`/`OpmAligned` — a `perforation_control_cells`
   fix is a physics-fidelity correction with its own merit even outside the heavy-case stack
   question) or the program re-plans honestly.

### Fallback (own future bundle, only if X1-X3 refute the fraction-fidelity thesis)

OPM's well primary-variable structure itself: per-well `WQTotal`/fractions/`bhp` unknowns with
connection rates *derived* at assembly (no per-perforation `q` unknowns, no `rate_consistency`
rows). Large — a rewrite of `wells_ad.rs`/`assembly_ad.rs` well blocks. Not attempted while a
small, precisely-located fraction fix remains unfalsified.

## 4. Cost estimate

X0 ~1-2h (trace + capped runs + analysis, done — WELLJAC/WELLJAC-WATER instrumentation plus the
OPM source read). X1 ~1-2h (flag + fix + capped probe + bounded no-op re-checks). X2 ~3-5h, only
if needed after X1. X3 ~1h capped, +~25min full heavy run, +full control-matrix/smoke/BL gate
(~30-60min) given the broader blast radius. Everything except X3's full runs fits capped-run
economics.

## 5. Documentation consequences

- Worklog entry per checkpoint, numbers verbatim; registry `FIM-BUNDLE-X` updated with the
  verdict either way.
- If X0 finds a chop/damping/bounds stage breaking first-order consistency: that finding gets
  its own registry row (it affects every case, not just the heavy one).
- If X3 promotes: `docs/FIM_STATUS.md` gap #4 closes; `FIM-BUNDLE-W`'s row gains its "re-run
  with both in place" resolution; the stack baseline `18,015` is superseded (say so explicitly,
  with the replay command).
