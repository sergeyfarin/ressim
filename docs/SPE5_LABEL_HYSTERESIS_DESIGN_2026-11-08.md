# Phase-label hysteresis for the SPE5 crossings (#116): design note

**Status: pre-implementation, not approved.** Written for review before any code, as the PR #128
review asked. Acceptance stays at `1e-8`. Everything described here would be **off by default**,
behind a generic compositional solver option, and evaluated as an experiment. All measurements
quoted are **provisional** (release builds of `1fb3dcdb` plus then-uncommitted test drivers; the
traces are in `docs/SPE5_VALIDATION.md`, "The crossings with the refinement and with timestep
memory").

## 1. What the traces show

Every failing call traced after the flash refinement is a two-cycle in which a cell's phase label
alternates on every Newton iteration. The labelled cell in the two iterates of the cycle:

| Trace (failing `dt`) | Cells | Two-phase iterate: β, Σ ln²K, S_g | Other iterate | Δp between iterates |
|---|---|---|---|---|
| 5 d cap, day 6555.996 (9.8e-3 d) | 25, 31 | 0.679, 2.4e-4, 0.317 | single phase, labelled L | 4.3e-3 bar |
| 0.5 d cap, day 6432.697 (9.8e-4 d) | 16 | 0.9999, 0.79, 0.550 | single phase, labelled L | 7.4e-4 bar |
| Memory run, day 6612.821 (0.02 d) | 33, 39 | 0.513, 0.139, 0.285 | single phase, labelled L | 0.35 bar |
| Memory run, day 6612.821 (4.9e-6 d) | 33, 39 | 0.484, 3.9e-4, 0.269 | single phase, labelled L | 4.7e-4 bar |

Two different situations are in this table.

**Near-critical (cells 25/31 and 33/39).** The two-phase iterate has β near one half and nearly
identical phases (Σ ln²K of 2e-4 to 4e-4). A split with β ≈ 0.5 can leave the two-phase region
only through the critical region, where the phases merge, so the label changes while S_g jumps by
about 0.3 with almost no change in p or z. Thermodynamically the two labels are hard to tell
apart: at the month-218 seed of signature A, g_split − g_feed was −8.4e-12 RT, inside the `1e-10`
Gibbs margin, and neither ResSim's nor MultiComponentFlash's stability test settled the label.

**Dew side, single-phase identity (cell 16).** Here the split is not near-critical
(Σ ln²K = 0.79) and is almost all vapour (β = 0.9999). The neighbouring single-phase state is
labelled **liquid** by Li's pseudo-critical-temperature rule (`fluid::flash::single_phase`). The
jump is between a vapour-like split and a single phase identified as liquid.

**Why Newton cannot converge.** The residual is not just non-smooth at the label change; it is
discontinuous. With the case's immiscible Stone II curves (SWOF/SGOF; the JutulDarcy reference uses
the same tables and OPM's Stone II form), oil and gas relative permeability differ even when the
two phases are identical. At cell 25's saturations, an estimate from the case tables gives a
hydrocarbon relative-permeability sum of 0.118 for the liquid label (S_w 0.53, S_g 0) and 0.041 for
the two-phase label (S_g 0.317): a factor of about 3 in the cell's mobility for the same fluid.
The flux residual therefore jumps by an amount proportional to `dt`:
- in the capped traces the cycle's residual halves with each halving of `dt`, and the solve
  converges once the step is small enough that the cell stays on one side;
- in the memory trace the residual stops halving below about 1e-5 d (1.19e-6 at 9.8e-6 d,
  8.9e-7 at 4.9e-6 d), so there is also a part that does not scale with `dt`, presumably an
  inventory difference between the labels (to be measured, M0).

No tolerance below the jump is reachable at a step that crosses it, which is why every crossing so
far either takes ever smaller steps or fails. JutulDarcy crosses the same window by accepting
ministeps at its own, looser criteria (48–153 failed ministeps in the reappearance months).

## 2. What the experiment changes

One thing: **during a single Newton solve, a cell keeps its current label while the competing
label is thermodynamically indistinguishable from it at Float64.** It is a deterministic tie-break
among states the flash cannot separate, not a relaxation of equilibrium or of acceptance. It
targets the near-critical case. The cell-16 case is a single-phase identity question and is
treated separately (section 6).

### 2.1 Branch selection

- **Held label `h_c`**, one per cell, local to one Newton attempt. It is initialised from the
  standard flash of the attempt's starting state (the accepted state). It is never stored in the
  state, a checkpoint or the next attempt.
- At each iterate, the standard flash gives label `F_c`. If `F_c = h_c`, nothing changes.
- If they differ, the held branch is evaluated at the same `(p, z)`:
  - **`h_c` two-phase, `F_c` single phase.** Solve the two-phase equations without the stability
    test, seeded with the K of the cell's last two-phase iterate. Keep `h_c` only if all of these
    hold:
    - converged to the existing `EQUILIBRIUM_TOLERANCE`;
    - 0 < β < 1 (no negative flash);
    - non-trivial (Σ ln²K ≥ `TRIVIAL_SOLUTION_TOLERANCE`);
    - g_split − g_feed ≤ +`GIBBS_ENERGY_TOLERANCE` (1e-10 RT, the #122 margin).
  - **`h_c` single phase, `F_c` two-phase.** Keep `h_c` only if F's split has
    g_split − g_feed ≥ −`GIBBS_ENERGY_TOLERANCE`, i.e. the split is not lower than the feed beyond
    round-off.
  - **Single phase both ways, L versus V.** Out of scope; the standard label is taken (section 6).
- Otherwise `h_c ← F_c`, and the switch is recorded.

**What is kept by this rule:** a converged, physical, non-trivial split of the same equations, or a
single phase the flash's own split does not beat by more than its round-off margin. It never keeps
a split the unmodified flash would reject as unconverged, unphysical or trivial, or one that raises
the Gibbs energy beyond the margin.

The margin was chosen for #122 as a round-off allowance, not as an equivalence threshold. Using it
here is a choice M0 must support with measurements (section 4).

### 2.2 Properties and derivatives

- **One state per cell per iterate.** Accumulation, both sides of every face's flux, and every well
  connection must see the same `FlashState`. Today each of `accumulation.rs`, `flux.rs` and
  `wells.rs` calls `flash(…, None)` for itself, which is consistent only because the flash is
  deterministic. The experiment computes each cell's state once per iterate, applies the held
  decision, and passes it to all three.
  - That refactor comes first and on its own. With the option off it must leave every answer
    **bit-identical** (compositional bucket, SPE3 replays, native-binding matrix).
- **Properties** are evaluated from the held `FlashState` as from any other: densities, viscosities,
  saturations and relative permeabilities follow its phases.
- **Derivatives** use the existing `flash_derivatives` on the held state. It branches on the
  state's `phase_state` and does not re-run the stability test, so a held two-phase state gets
  implicit differentiation of its own split. If the flash refinement is on, it applies to held
  splits as well, and the derivative pass differentiates the returned split, as it does today.
- **Check.** The Taylor test (`spe5_failure_b_tests::taylor`) is repeated at held iterates, so the
  Jacobian is verified against the residual it actually describes.

### 2.3 Acceptance, restart and records

- **Acceptance is unchanged:** `1e-8`, every well row, the existing admissibility checks.
- **Held labels at acceptance.** At the converged iterate, every cell whose held label differs from
  the standard flash's label must also pass two checks:
  - **thermodynamic equivalence:** |g_held − g_standard| ≤ `GIBBS_ENERGY_TOLERANCE`;
  - **inventory equivalence:** the cell's component moles computed from the held and the standard
    state differ by at most `1e-12` of the cell's inventory per component.

  Otherwise the attempt is **rejected**, and the existing halving applies.
- **The committed state is the standard one.** The state, the checkpoint and the next step see only
  `(p, z)` and the standard flash, so restarts and checkpoints are unchanged, with no schema change.
  The inventory-equivalence check bounds the mass that switching labels at the step boundary can
  book.
- **What remains different, and is recorded rather than hidden.** The step's fluxes were computed
  with the held label. From the next step on, the standard label's mobilities apply. Near the
  critical point both labels are valid within Float64, and with these immiscible curves they give
  different fluxes. This is the choice the review asked to see stated. It is a property of the model
  (section 5), and the experiment measures it rather than conceals it.
- **Per accepted step**, the step report records:
  - the number of cells held at acceptance;
  - the largest |Δg|;
  - the largest inventory difference;
  - the largest held-versus-standard total mobility ratio;
  - the residual of the accepted iterate re-evaluated with standard labels (a diagnostic, not a
    gate).

  A run keeps cumulative sums of the held inventory differences. They are kept apart from the
  well cumulatives and never added to them.
- **Within an attempt**, switches per cell are counted. A cell that switches more than twice in one
  attempt is reported: that would be a cycle the rule did not break.

## 3. What it does not do

- No change to `1e-8`, to the equilibrium tolerance, or to the Gibbs margin's value.
- No label memory across attempts, steps or checkpoints.
- No negative flash: a held split stays physical (0 < β < 1).
- No change to single-phase identity (L versus V), which is section 6.
- No change to the relative-permeability model (section 5).

## 4. Evaluation

**M0: measure before writing the option.** This is test-only, extends `comp_spe5_trace_seed_ladder`
and changes no engine behaviour. For every flipping pair in the five captured states (the 5 d and
0.5 d failures, the two 12-attempt failures, the memory failure), and at both iterates of each pair:
- g_split − g_feed for both labels;
- whether the seeded two-phase solve converges at the single-phase iterate, with its β and Σ ln²K;
- the inventory difference between the labels;
- the cell's hydrocarbon total mobility under each label, from the engine's own relperm.

**Gate 0.**
- **Proceed to E1 only if** the near-critical flips (cells 25/31, 33/39) meet the hold conditions of
  2.1 and 2.3 at both iterates.
- **If they do not, the rule as written cannot act.** The verdict for this design is then
  **REFUTED** before any engine change, and the note is revised.
- **Cell 16** is classified by the same measurements.

**E1: the option, off by default.** It is a generic `NewtonOptions` field, with no case or
scenario names. Before anything else:
- the shared-state refactor of 2.2, bit-identical with the option off;
- `validate-compositional.sh thermo`;
- the compositional bucket;
- the SPE3 replays;
- `validate-native-binding.sh`.

**E2: the captured states.** The five failure states, one Newton solve per halving, as in the
existing traces. Record converged `dt`, iterations, the held-at-acceptance records and the Taylor
test at a held iterate.

**E3: the replays.**
- From scratch at report steps, 5 d and 0.5 d caps, to month 240.
- For each: conservation (signed balance), the held ledgers, failed attempts, Newton work and wall
  time.
- Compared with JutulDarcy at matched caps through `spe5_compare.py --timestep-convergence`.
- Neither 0.5 d run is time-converged, and agreement with Jutul is not independent accuracy (see
  `SPE5_VALIDATION.md`).

**Verdicts** follow the `fim-solver-debug` rules:
- **INCONCLUSIVE ≠ REFUTED.** A replay that still fails at a crossing, with held cells recorded, is
  REFUTED for that crossing. A replay stopped by its wall budget is INCONCLUSIVE.
- **Nothing is proposed for default use from this experiment.** A positive E3 is evidence for a
  separate approval discussion. Only that discussion could change the default, and only with:
  - the cross-solver and benchmark gates;
  - the #98 shut-in probe;
  - unchanged controls.

## 5. The physical alternative, and why it is not this experiment

A flux jump between labels of an identical fluid comes from using immiscible curves where the
phases become miscible. Compositional simulators commonly handle this with near-critical relative
permeability that depends on interfacial tension: as the phases converge, the curves are
interpolated towards straight lines (Coats, 1980), so the flux is continuous through the critical
region. That would remove the discontinuity at its source. It also changes the SPE5 model that
ResSim and the JutulDarcy reference both run, so the comparison would need the same treatment in
the reference. It is recorded here as the principled route if the tie-break is refuted, not as
part of this experiment.

## 6. Single-phase identity (cell 16)

Li's rule labels a single phase by its pseudo-critical temperature. On the dew side, a vapour-like
feed next to an almost-all-vapour split (β = 0.9999) is labelled liquid, so its relative
permeability switches curves where the split collapses. The tie-break of 2.1 does not address
this: the split there is not near-critical, and its Gibbs advantage is unlikely to be within the
round-off margin (M0 will say).

Possible remedies change the label of single-phase cells, and so answers everywhere, including
SPE3. For example, identity by continuity with the collapsing split (β → 1 means vapour), or by the
cubic root's branch. Each would be a separate experiment with its own evaluation. It is listed so
that E3's result at the 0.5 d crossing is not misread: if cell 16 still fails with the tie-break,
that is this question, not a refutation of the tie-break.

## 7. Open questions for review

1. **The margin.** Is `GIBBS_ENERGY_TOLERANCE` (1e-10 RT) the right equivalence threshold, or should
   M0 set one from the measured Δg at the flips, with its own justification?
2. **Flux at acceptance.** Is it acceptable that an accepted step's flux used the held label while
   the committed state's standard label gives a different mobility? The alternative is to require
   the standard-label residual to meet `1e-8` too. That would reject exactly the steps of interest,
   which is why it is proposed only as a recorded diagnostic.
3. **Inventory bound.** Is `1e-12` per component per cell the right bound for inventory
   equivalence, given that the run's worst signed balance so far is 6.1e-9 of inventory?
4. **Scope.** Should the cell-16 identity question be measured in M0 now, as proposed, or left
   until the near-critical tie-break has a verdict?
