# Phase-label hysteresis for the SPE5 crossings (#116): design note

**Status: the tie-break proposed in sections 2–4 is REFUTED by its own gate (M0, section 0), and
is not to be implemented.** The note was written for review before any code, as the PR #128 review
asked, and the PR #129 review asked for a stronger M0 before anything else. That M0 is section 0.
Sections 1–7 are kept as written for the record. Their measurements came from traces of different
Newton iterates (release builds of `1fb3dcdb` plus then-uncommitted test drivers). Where section 0
corrects them, section 0 holds.

## 0. M0: identical-input measurement (2026-11-08)

**Provenance.** Commit `93b97f79`, clean tree, release build. The inputs are the five committed
crossing seeds (`opm/compositional/spe5/stall_*.json`; provenance in that folder's README), each at
the last failing `dt` of its trace, with the default flash (no refinement).

```bash
SPE5_TRACE_OUT=DIR cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_m0_branch_comparison -- --ignored --nocapture          # ~2 min
SPE5_TRACE_OUT=DIR SPE5_M0_POINTS=DIR/m0_gibbs_points.json cargo test --release \
    --manifest-path src/lib/ressim/Cargo.toml --lib comp_spe5_m0_trial_sums -- --ignored --nocapture
uv run --no-project --with mpmath python -I tools/jutul/spe5_m0_gibbs_mp.py \
    DIR/m0_gibbs_points.json DIR/m0_gibbs_mp.json                     # 50 digits
```

A first run on the uncommitted tree gave byte-identical output.

**What was measured.**
- **Branch comparison.** At fixed primaries in every cell, fixed well BHPs and a fixed previous
  inventory, the residual is assembled with the standard flash and again with the flipping cells
  forced onto the competing branch (a test-only hook, so accumulation, both sides of every face and
  the wells all see the same state).
- **Decomposition.** The difference is split into inventory and face outflow. The identity
  `ΔR = Δn + dt·Δoutflow` closes to ≤ 1.8e-12 mol in every case, and the well rates do not change
  (no flipping cell has a well connection).
- **Switch sweep.** Along the straight path between the cycle's two iterates (all primaries and
  BHPs), the label switch is bracketed by bisection to adjacent floats. The standard residual is
  then compared at distances 1e-1 … 1e-15 either side.
- **Gibbs energy and stability.** At every evaluated flipping-cell state: g_split − g_feed in Float64
  and in 50 digits, on the cubic root actually used, and the stability test's trial sums.

**1. A finite jump between differently labelled inputs.** Two measurements, kept separate:
- **Fixed inputs (branch comparison).** At the same primaries, BHPs and previous inventory, forcing
  only the flipping cells' branch changes the residual by the amounts in the last three columns
  below.
- **Varying inputs (switch sweep).** Bisection ends on adjacent floating-point inputs whose
  standard labels differ, and the scaled gap there is the "Scaled jump" column. Away from the
  bracket the label decision chatters, so the sweep's other pairs do not show a constant gap:
  - Pairs whose two ends carry different labels show a gap of the same size per switching cell.
  - Pairs whose two ends carry the same label show a much smaller one, for example 3.8e-8 at
    ε = 1e-10 in `cap5` (both TwoPhase) and 2.6e-10 at ε = 1e-4 in `memory`.
  - So the measured claim is a finite gap between differently labelled adjacent inputs, with
    same-label samples in between. It is not a gap constant over the interval, nor a one-sided
    limit.

  Cells not touching the flipping cells change by less than 1e-12 at every distance, and by 0 at
  the smallest.

At the bracket (ε = 1e-15, adjacent inputs, different labels), with the fixed-input comparison at
its two-phase end:

| Seed (`dt`, d) | Switching cell | Scaled jump | Δn per component (rel.) | dt·Δoutflow, C1 (mol) | Δn, C1 (mol) |
|---|---|---|---|---|---|
| `stall_6556_cap5` (9.77e-3) | 25 | 8.41e-5 | 3.4e-7 | 1.27e4 | 51.7 |
| `stall_6556_cap5_att12` (1.22e-3) | 25 | 1.08e-5 | 3.4e-7 | 1.59e3 | 52.0 |
| `stall_6432_cap05` (9.77e-4) | 16 | 1.52e-6 | 2.9e-11 | 282 | 5.4e-3 |
| `stall_6432_cap05_att12` (1.22e-4) | 16 | 1.90e-7 | 2.9e-11 | 35.3 | 5.4e-3 |
| `stall_6612_memory` (4.88e-6) | 39 | 8.91e-7 | 5.9e-7 | 53.6 | 106 |

- **The flux part scales exactly with `dt`.** 8× smaller `dt` gives 8× less (1.27e4 → 1.59e3; 282 →
  35.3).
- **The inventory part does not scale with `dt`.** It is 3.4e-7 to 5.9e-7 of each component's
  inventory at the near-critical cells, and 2.9e-11 at cell 16.
- **At the memory seed the inventory part dominates**, which is why that trace's residual stopped
  halving with `dt`.
- **The faces.** Upwind choices do not change on any face that carries flow; only vertical faces
  with zero vapour rate change their vapour upstream. On the flipping cell's outflow faces, the
  component flux changes by 60% (cells 25/31), 10% (cell 16) and 80–90% (cells 33/39).
- **Mobility, kr/μ in 1/cP, at the cycle's iterate:**

  | Cell | Single phase | Two-phase |
  |---|---|---|
  | 25 | 2.58 (liquid) | 0.90 (vapour only; the liquid is immobile) |
  | 33 | 5.92 (liquid) | 0.50 liquid + 0.67 vapour |
  | 16 | 5.97 (liquid) | 5.34 (vapour) |

**2. The near-critical switch is the stability test's margin, not a phase boundary.**

| Cells | Standard label either side | 50-digit split on both sides | g_split − g_feed, 50 digits | Float64 | Deciding trial sum − 1 |
|---|---|---|---|---|---|
| 25/31 | TwoPhase / SingleLiquid | β 0.729–0.731, Σ ln²K 1.45e-4 | −1.62e-13 to −1.64e-13 RT | −1.59e-13 to −1.65e-13 | liquid trial 0.998e-12 to 1.001e-12 |
| 33/39 | TwoPhase / SingleLiquid | β 0.4766, Σ ln²K 2.0e-4 | −4.71e-13 RT | −4.69e-13 to −4.72e-13 | vapour trial 0.999e-12 to 1.002e-12 |
| 16 | TwoPhase / SingleLiquid | β = 1 − 2.8e-9, Σ ln²K 0.791 | −1.4e-21 RT | ±1e-15 (noise; sign varies) | liquid trial 0.998e-12 to 1.001e-12 |

Every point has one real cubic root, so the root choice is not involved.

- **At the near-critical cells (25/31 and 33/39), the "single-phase" side is not single-phase.** A
  converged, physical, non-trivial split exists there, with β of 0.48–0.73, and its Gibbs energy is
  **lower** than the feed's.
- **Float64 resolves that difference.** It agrees with 50 digits to about 3e-15 RT, against a gap of
  1.6e-13 to 4.7e-13 RT.
- **The switch is the threshold crossing.** The label changes exactly where the deciding trial sum
  crosses `1 + TRIAL_SUM_TOLERANCE` (1e-12, `fluid/stability.rs`), a margin chosen for #93 so that
  ordinary bubble-point switches happen at β ≈ 1e-8. A split with β ≈ 0.5 between nearly identical
  phases has a tangent-plane signal of the same order as that margin. So the margin cuts a real
  split at finite β, and S_g, mobility and inventory jump there.
- **The label chatters inside a band.** Within the bracket, the standard label is not monotone along
  the path: the trial sum sits within ±1e-15 of the threshold.
- **This corrects section 1.** There, the near-critical labels were called thermodynamically hard to
  tell apart. At these states they are not: the split is the lower state, and Float64 shows it.

**3. Cell 16 is a genuine dew point with a mislabelled single phase.** The switch is at
β = 1 − 2.8e-9, the Gibbs difference is zero to 1e-21 RT, and the inventory is continuous
(2.9e-11). The jump is flux only. On the single side, Li's rule labels the vapour-like feed liquid,
so the cell switches from vapour to liquid relative permeability and viscosity. This is the identity
question of section 6, now measured. It is separate from the near-critical mechanism.

**Verdict on this design (Gate 0): REFUTED.**
- **The acceptance contract cannot be met.** Inventory equivalence fails by five orders of magnitude
  (3.4e-7 to 5.9e-7, against the proposed 1e-12).
- **Its premise is false.** The two labels are not Float64-indistinguishable. Holding the
  single-phase label would hold the wrong state, and holding the two-phase label is what a correct
  phase decision would return anyway.
- **The P1 review finding stands for any variant.** A contract that preserves the original
  equations cannot accept a held-label flux.

The mechanism is now measured rather than inferred, and it points away from the solver.

**What the measurement points to instead (each not approved, and each needs its own design and
evaluation, because it changes the standard flash and so answers everywhere):**
- **N1, the near-critical phase decision.** Inside the stability test's margin band, decide by the
  split itself: return a converged, physical, non-trivial split with g_split < g_feed (seeded from
  the trial K) instead of "stable".
  - That moves the label switch towards where the split actually collapses.
  - Not yet measured: where that collapse is for these cells. If it is at the critical point
    (K → 1 at finite β), the trivial-solution tolerance (Σ ln²K < 1e-5) cuts the split again at
    finite β, and the immiscible curves still jump there (section 5). Measuring that, as M1, comes
    before N1.
- **N2, single-phase identity at the dew point.** Identify the single phase consistently with the
  collapsing split (β → 1 means vapour) rather than by Li's T_pc rule there. The rule must be
  deterministic and reproducible from cold inputs and checkpoints, so it cannot depend on the
  previous iterate. A single cubic root by itself does not identify liquid versus vapour (every M0
  point has one real root).
- **M1 comes first (PR #129 review).** Continue the physical split along the relevant input paths
  in higher precision through its actual collapse and through the trivial-solution cutoff,
  tracking β, Σ ln²K, g_split − g_feed, density and inventory, and component flux. That
  establishes whether N1 removes the jump or only moves it. Lowering a threshold alone is not a
  demonstrated crossing fix, and neither N1 nor N2 is approved by M0.
  **M1 is done and points away from N1 alone:** 25/31 and 16 collapse at the dew point and leave
  only the single-phase identity jump (N2); 33/39 are cut by the trivial cutoff at β = 0.3929,
  still a real split, so N1 moves the jump rather than removing it. See
  [SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md](SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md).
- **Acceptance stays the original equations:** the standard-state residual, wells included, at
  `1e-8`. N1 and N2 change the standard flash itself, so no held-label flux needs accepting. This is
  option 1 of the P1 review finding.

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

**Answers after the PR #129 review and M0:**
1. **No new equivalence band was chosen.** `GIBBS_ENERGY_TOLERANCE` stays what it is: the #122
   guard's value. At the near-critical states, Float64 g_split − g_feed matches the 50-digit value
   to about 3e-15 RT, and the gap (1.6e-13 to 4.7e-13 RT) is real. At cell 16 the gap (1e-21 RT) is
   below Float64's resolution and is not a usable decision variable.
2. **The original equations are kept.** Any candidate must converge the standard-state residual,
   wells included, at `1e-8`. Held-label flux acceptance is withdrawn.
3. **Inventory bound: withdrawn with the design.** The measured inventory differences between the
   labels are 3.4e-7 to 5.9e-7 per component at the near-critical cells and 2.9e-11 at cell 16.
4. **Cell 16 was measured in M0** and is classified separately, as an identity defect at a genuine
   dew point (section 0, item 3).
