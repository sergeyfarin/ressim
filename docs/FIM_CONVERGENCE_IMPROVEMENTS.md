# FIM Convergence Improvement Plan (2026-04-18)

This document is a forward-looking plan for the next FIM convergence work on
current head. It is grounded in:

- `docs/FIM_CONVERGENCE_WORKLOG.md` (active 2026-04-08..2026-04-12 findings)
- `docs/OPM_FLOW_MINIMAL_MAPPING.md` (minimal OPM Flow lessons)
- `docs/FIM_SLICE_A_EXTRAPOLATION.md` (Newton initial-guess extrapolation,
  attempted 2026-04-13, reverted — 2 rounds)
- `docs/FIM_STATUS.md` + current code in `src/lib/ressim/src/fim/`

It consciously does not repeat already-landed wins (hotspot memory,
local-region grouping, zero-move bypass, replay aggregation, gas-only
3-step carryover, producer-hotspot threshold 2, CPR Phase 1/2). The
purpose is only to identify the *next* candidate directions.

## Where we actually are

Current reproducible baseline on the documented validation shortlist:

| Case | substeps | retries | notes |
|------|----------|---------|-------|
| `water-pressure 12x12x3 dt=1`  | `16` real accepts (`15+4+8354` split after replay aggregation) | `0/9/0` | heavy guard, still dominated by `water@1020` hotspot-repeat tail |
| `water-pressure 20x20x3 dt=0.25` | `12` | `0/5/0` | medium water shelf, still retrying |
| `water-pressure 22x22x1 dt=0.25` | `4` | `0/2/0` | exact-dense threshold control |
| `water-pressure 23x23x1 dt=0.25` | `4` | `0/2/0` | over-threshold CPR probe |
| `gas-rate 10x10x3 steps=6 dt=0.25` | step1=`8`/`0/3/0`, steps2..6 `4`/`0/0/0` (after 3-step gas carryover) | total 5 retries | shipped gas shelf |
| `gas-rate 20x20x3 dt=0.25` | `4` | `0/2/0` | bounded gas control |

For context, OPM Flow finishes the corresponding first report step in about
`4` substeps on this class of Cartesian case. So the active gap is
*substep count and retry ladders*, not linear-solve correctness.

Current dominant failure mode across the remaining shelves:

1. Heavy water: one hotspot (`water@1020`, cell340) keeps producing
   bounded-Appleyard zero-move rejections on the large rung, forcing dt
   to collapse roughly 2 orders of magnitude into the accepted shelf.
2. Medium water `20x20x3`: `oil@190`, a 5-rung nonlinear retry ladder
   walking through a small front patch near `(3,3,0)`..`(3,4,0)`.
3. Gas step 1 of the shipped replay: still takes 3 retry rungs at
   startup even with the three-step carryover; steps 2..6 are now clean.

The common thread: these are *physical-front / regime-flip* failures, not
linear-solver failures. That shapes the recommended directions below.

## What has already been ruled out (do not re-attempt in the same form)

From the worklog + Slice A memory:

- Plain global Newton initial-guess extrapolation from last two clean
  states (OPM-style `extrapolateInitialGuess`), both alone and paired
  with replay-gate tolerance relaxation. Both rounds regressed net;
  the dominant cost was new `nonlinear-bad` retries near the gas-oil
  contact and the water hotspot. See
  `docs/FIM_SLICE_A_EXTRAPOLATION.md` + `project_fim_slice_a_attempt`.
- Cross-outer-step hard `dt` carryover for water (two variants,
  both reverted for water guard regression).
- Generic first-post-cooldown accepted-step regrowth cap on
  `20x20x3` water (`1.6x` and `2.0x` variants, both worsened the shelf).
- Stagnation acceptance widening for repeated non-gas hotspots
  (introduced longer plateau ladders).
- Retry-factor shaping below `0.5` inside timestep for repeated
  non-gas stagnation.
- Hotspot-memory early release on exact no-op accepts (reopened the
  retry ladder).
- Trace-ratio / post-improvement-plateau GMRES-tail variants for the
  exact-dense `22x22x1` regression (all reverted).
- Broad residual-shortcut tightening in Newton (saved day-2 replay
  regressed to `1540` substeps).

These should stay on the "rejected" list; a new direction is only a
duplicate of one of these if the promised mechanism is the same.

## Candidate directions, ranked

### A. Strict per-cell Newton extrapolation with smoothness gating  (top 1)

Why: The Slice A negative result was specifically against *global*
extrapolation — one shared scalar dt-ratio applied to every cell. The
failure mechanism was that a small number of cells near hotspots and
the gas-oil contact left the Newton basin, and those few cells cost
more than the replay savings. That is exactly the failure mode a
per-cell freeze is designed to suppress.

Minimal rule to try (not yet implemented):

1. Maintain last two clean-accepted full states (as Slice A did).
2. Before Newton, compute per-cell linear extrapolation of `p`, `Sw`,
   `Sg`, `Rs`.
3. For each cell, freeze extrapolation (fall back to last clean state)
   if *any* of:
   - `|dp|` too small or too large (below trace threshold or above a
     saturation-guard factor of last seen),
   - `|dSw|` or `|dSg|` would cross a fractional-flow inflection or a
     regime boundary (undersaturated↔saturated, Sg=0 boundary),
   - cell is inside a N-neighbour ring around the last accepted
     `hotspot_site` (use existing geometry-aware site grouping).
4. Apply the extrapolation only to the surviving cell set; all other
   cells start at the previous clean-accepted state.

Acceptance criteria (exactly the current validation shortlist):

- `12x12x3 water dt=1` heavy: must not regress `substeps`/`retries`/
  `outer_ms` material; wall-clock noise tolerated inside 1.1× band.
- `20x20x3 water dt=0.25` medium: improvement required (this is the
  target shelf); success = fewer than `12` real accepts or fewer than
  `5` retries without shelf-ladder explosion.
- `22x22x1` and `23x23x1` dt=0.25: must stay at `4`/`0/2/0`.
- `20x20x3 gas-rate dt=0.25`: must stay at `4`/`0/2/0`.
- Shipped gas `10x10x3 steps=6 dt=0.25`: retry pattern must stay at
  `0/3/0` then all-clean.

Stop rule: if the per-cell freeze does not remove the "extrapolated
iterate outside Newton basin near hotspot" class of failures that
sank Slice A round 2, revert and move to direction B.

Relevant files:

- `src/lib/ressim/src/fim/newton.rs` (Newton initial-iterate seeding)
- `src/lib/ressim/src/fim/timestep.rs` (outer loop owns last-two-state
  buffer for this)

### B. Dt-aware replay acceptance tolerance tied to update tolerance  (top 2)

Why: Current `iterate_has_material_change` replay predicate uses a
fixed epsilon (≈`1e-12`) plus a per-family band (`5e-3 bar`,
`5e-5 sat`). On the heavy `12x12x3` shelf, the combination already
gives most of the runtime savings, but it does not scale the band
with the currently-attempted `dt`. A dt-aware band would, for example,
allow more aggressive replay aggregation on the very small tail rungs
(`~4e-5 d`) while staying conservative at the larger accepted rungs.

Minimal rule:

- Scale the per-family replay tolerance linearly in `dt / dt_ref`,
  with a floor equal to the current fixed band and a cap well below
  the Newton convergence tolerance, so replay can never mask a real
  convergence near the accepted-step scale.
- Keep the replay *only* inside already-established cooldown or
  hotspot-plateau regimes (same gates already in `timestep.rs`).
- Do not touch the Newton convergence tolerance itself.

Acceptance: same shortlist as A, and specifically the heavy shelf
accepted-substep bookkeeping ledger (`substeps` field, not real
accepts) should collapse further without real-accept regression.

Risk: this is purely a bookkeeping cost cut; it will not reduce real
accepted solves or retries. That is fine — the heavy shelf is already
runtime-bound by ledger size, not real solves.

### C. Primary-variable switching / update hysteresis at the gas-oil contact  (top 3)

Why: `docs/OPM_FLOW_MINIMAL_MAPPING.md` item 4 (medium expected value)
and several worklog traces both point to regime flips at the gas-oil
contact. The current `FimState` has `hydrocarbon_state` but does not
have an explicit hysteresis rule on variable-set switching during
Newton. OPM treats the primary-variable switching as a first-class
source of oscillation and wraps it in explicit hysteresis.

Minimal rule:

- Inside Newton, if a cell flips hydrocarbon regime twice within the
  same substep, freeze the variable set for the remainder of that
  substep and accept a residual-only convergence on the frozen set.
- Apply only to cells currently inside the geometry-aware hotspot
  ring, to keep the change local.

Acceptance: same shortlist. Additionally, the `gas-rate` step-1
ladder should start shortening; today it still takes `0/3/0`.

Why this ranks third, not higher: it is riskier than A and B because
it changes the *state* a cell is in, not just how the solver *seeds*
that state. The ResSim Jacobian paths in `assembly.rs` assume the
variable set for the cell; freezing needs to be handled carefully.

Relevant files:

- `src/lib/ressim/src/fim/state.rs` (regime transitions)
- `src/lib/ressim/src/fim/newton.rs` (per-substep hysteresis gate)

### D. Over-threshold CPR path — local preconditioner augmentation on dead-state rows  (separate track)

The bounded `22x22x1` vs `23x23x1` pair still shows that once the
coarse solver changes from exact-dense to BiCGSTAB, the full system
falls back to sparse-LU 18–22 times per bounded substep set, even
after all the known bypass / near-converged-accept work.

**Refined (see Q2 below):** wire a tiny additive Schwarz patch as a
*local preconditioner augmentation*, keyed on the rows that trip
the `dead-state` classifier
([gmres_block_jacobi.rs:1117](src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs#L1117)).
The `dead-state` trigger condition
`preconditioned_residual > 4 × outer_residual` is precisely a
measurement that the current block-Jacobi preconditioner is the
weak link on those rows; augmenting it there attacks the mechanism
the classifier is already measuring. Do **not** wire this as a
post-failure smoother — the 2026-04-11 sparse-LU work already
reduced fallback cost; another post-failure cleanup has low value.

Acceptance: preserve `23x23x1` at `4`/`0/2/0`, reduce the direct-solve
count, and not regress `22x22x1`.

This track is independent of A/B/C — it affects the linear backend,
not the nonlinear controller.

### E. Step-0 trial policy generalized to medium water  (top 4)

Why: the committed gas-only cross-step carryover already shows that a
narrowly-scoped first-trial policy works when it is gated on the
correct regime signature (flat hotspot-repeat shelf). An equivalent
for medium water — but carefully gated, because two water carryover
variants already regressed — is the most obvious next slice once A
or B lands.

Minimal rule:

- Only inside a committed working baseline containing A or B.
- Seed the next outer step's first trial at the last accepted `dt`
  *only* if the previous step ended with a materially-changed clean
  accept *and* did not finish on `hotspot-repeat` (the reverse of the
  gas rule). This is explicitly not a hard dt-cap; the trial can grow
  on the first retry rung as usual.
- Expire after one clean regrowth step, not three.

Acceptance: `20x20x3 water dt=0.25` should improve; all other
controls should stay unchanged.

## Execution order (recommended, updated 2026-04-18 after Q1-Q3)

0. **Pre-work (diagnostic only).** Land the basin-escape proxy probe
   described in Q3. This is additive, non-controller-changing, and
   directly determines whether A or C is the right next slice. Cost
   estimate: one extra residual evaluation per clean-accepted outer
   step. No expected change to current validation shortlist numbers.
1. Read probe output on the heavy + medium water + shipped gas
   shelves. Decide the A/C order based on the cluster pattern:
   - tight cluster at hotspot ring / gas-oil contact → proceed to A.
   - diffuse risk → skip A, proceed to C.
2. Land D (local preconditioner augmentation on dead-state rows, per
   refined Q2 answer) in parallel with step 1. It does not depend on
   A/B/C. Validate with the `22x22x1` vs `23x23x1` pair only.
3. Attempt A (per-cell Newton extrapolation) *or* C (regime
   hysteresis), whichever the Q3 probe selected. Not both in the
   same slice.
4. If A (or C) is green, add B (dt-aware replay tolerance) as a cost
   cut on top.
5. Only after the chosen A/C path and B are settled, attempt E
   (generalized step-0 trial for medium water).

Do *not* attempt more than one of A, C, E in the same slice; they all
touch the Newton/timestep interaction surface and would make
attribution impossible.

## Non-negotiable guardrails (inherited from current worklog)

- Every promotion must rerun the validation shortlist on a committed
  revision, per `CLAUDE.md` baseline discipline. Do not promote on
  dirty worktree numbers.
- Locked Rust smoke gates remain:
  - `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`
  - `spe1_fim_first_steps_converge_without_stall`
  - `spe1_fim_gas_injection_creates_free_gas`
- Do not regress `22x22x1` from `4`/`0/2/0`; that control has already
  moved once silently between `12ae00a` and current head and caught us.
- Do not generalize exact-dense water-shelf results into large-case
  CPR quality claims.

## Open questions — answered 2026-04-18

### Q1. Is the `water@1020` zero-move Appleyard rejection on the large rung physical?

**Answer: no, not in the strict Newton-basin sense. It is a
bounded-Appleyard acceptance-policy rejection, not a physics failure.**

Evidence in current code:

- [src/lib/ressim/src/fim/newton.rs:2132-2148](src/lib/ressim/src/fim/newton.rs#L2132-L2148)
  defines `zero_move_appleyard_acceptance_allows`. The rule only
  accepts the unchanged state if the residual and material-balance
  diagnostics are already inside a very tight band
  (`residual_tolerance * 1e-3 * 2`, same for MB).
- [src/lib/ressim/src/fim/newton.rs:3032-3090](src/lib/ressim/src/fim/newton.rs#L3032-L3090)
  is the call site: when the Appleyard damping produces a
  materially-unchanged candidate, the accepted-state diagnostics are
  evaluated, and if they clear the tight band, the substep is
  accepted as converged. Otherwise it falls through to
  [src/lib/ressim/src/fim/newton.rs:3092-3108](src/lib/ressim/src/fim/newton.rs#L3092-L3108),
  which logs `DAMPING FAILED — invalid bounded Appleyard candidate`
  and returns a non-converged `FimStepReport`.

What that means concretely:

- The solver is *not* saying "the next Newton iterate would have
  blown up." It is saying "the candidate I computed was chopped to
  zero by bounded-Appleyard, and my accepted-state diagnostics at
  the unchanged state are *not* tight enough to call it converged."
  That is a policy decision about the acceptance band, not a
  statement about Newton basin geometry.
- The 2026-04-11 worklog entry (`ZERO-MOVE APPLEYARD ACCEPTED
  res=1.764e-8 mb=3.090e-9`) is direct evidence the unchanged state
  can clear a tight band under the current rule. When it does, the
  rung is accepted. When it does not, the rung is rejected — but the
  cell's local physics did not change between the two outcomes.

Implication for direction A:

- The relevant question is not whether per-cell extrapolation
  changes the *physics* of `water@1020`. It is whether per-cell
  extrapolation, applied *outside* the hotspot ring, changes the
  *global residual and MB diagnostics* enough that the same rung's
  zero-move candidate now falls inside or outside the tight band.
- The answer is almost certainly yes: the accepted-state
  diagnostics are global inf-norms, and an extrapolation that
  lowers the contribution from smooth cells will make the hotspot
  cell's contribution stand out more — it could go either way,
  depending on whether the extrapolation reduces or increases the
  global residual norm after damping.
- Net: direction A *might* move this rung, but the mechanism is
  indirect. Do not promise a heavy-shelf improvement from A in
  advance; measure it on the heavy guard replay before claiming it.

### Q2. Does direction D's Schwarz smoother just shift where fallback fires?

**Answer: on current evidence from the worklog site-classification,
the answer is "mostly yes, for the hard-state family; no, for the
tiny-tail family" — so the Schwarz candidate is *only* worth doing
if it targets the hard-state family specifically.**

Evidence:

- The 2026-04-11 site classification in the worklog (lines 97-108)
  splits the surviving fallback sites into six families. The
  dominant repeated one is substep 6 oil cell95
  (`row=286 item=95`): a `dead-state` fallback followed by five
  same-substep direct-bypass cleanup iterations.
- The `dead-state` classifier is defined at
  [src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs:1117-1135](src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs#L1117-L1135)
  with thresholds `DEAD_STATE_MIN_OUTER_FACTOR=1024.0`,
  `DEAD_STATE_MIN_ESTIMATE_FACTOR=16.0`,
  `DEAD_STATE_MIN_PRECONDITIONED_RATIO=4.0`. A `dead-state` trip
  means restart 1 never improved the iterate *and* the residual is
  still far from tolerance *and* the preconditioned residual is
  much larger than the outer residual.
- That last condition (`preconditioned_residual_norm >
  outer_residual_norm * 4`) is precisely a statement that the
  *current preconditioner* is the problem on this row, not the
  Krylov method. A locally stronger preconditioner (what direction
  D's additive Schwarz would be) is exactly what changes the
  `DEAD_STATE_MIN_PRECONDITIONED_RATIO` measurement.
- By contrast, the tiny-tail families (substep 0 oil cell48,
  substep 5 water cell96) end in `max-iters` with a small residual
  already. The near-converged iterative-accept path
  ([src/lib/ressim/src/fim/newton.rs:272](src/lib/ressim/src/fim/newton.rs#L272))
  is already catching the ones where the iterate is close enough;
  the remaining tiny-tail fallbacks are specifically the ones
  where the global preconditioner is still wrong locally. A
  Schwarz patch on those rows would *also* apply there.

Implication for direction D:

- If the Schwarz patch is wired in as a **local preconditioner
  refinement** (augment block-Jacobi on the rows flagged by the
  dead-state classifier), it attacks the mechanism the classifier
  is measuring, not just where fallback is reported. That is worth
  trying.
- If instead it were wired as a **post-failure smoother** (run it
  after `dead-state` trips), it would only reduce the cost of the
  fallback, not the fallback incidence. The worklog 2026-04-11
  direct-fallback work already shows sparse-LU is cheap enough;
  another post-failure cleanup has low expected value.

So direction D is worth doing *as local preconditioner
augmentation*, not as a post-failure smoother. Update the plan
accordingly.

### Q3. Is there an offline way to measure "cells that would leave the Newton basin under global extrapolation"?

**Answer: yes — there is a cheap measurement that does not require
a third Slice A run. It reuses data ResSim already computes.**

The idea:

1. On every clean-accepted substep, the solver already knows the
   last two clean-accepted states (Slice A's data need).
2. Also on every clean-accepted substep, the solver already knows
   the residual and Jacobian at the accepted state (final Newton
   iteration).
3. The "basin-escape proxy" is: evaluate the residual **at the
   globally-extrapolated candidate** (no Newton iteration, no
   linear solve). If the residual inf-norm at the extrapolated
   candidate is > K × (residual at `previous_state`) for some K,
   flag the cell contributing the max.
4. Because this uses only `evaluate_residual`, not a full Newton
   solve, it costs one residual evaluation per clean accept,
   which is already inside the main assembly cost envelope.

Concretely in the current tree:

- `FimStepReport.final_residual_inf_norm` and the residual family
  diagnostics are already recorded. The extrapolation vector is
  already cheap to form.
- The residual eval itself is
  [src/lib/ressim/src/fim/assembly.rs](src/lib/ressim/src/fim/assembly.rs);
  calling it on an alternate state is a well-defined operation.
- The offline probe lives behind the diagnostic summary path
  ([scripts/fim-wasm-diagnostic.mjs](scripts/fim-wasm-diagnostic.mjs))
  and reports a per-substep list of "basin-escape risk cells"
  with their residual amplification factor.

Why this short-circuits the A/B/C decision:

- If the probe shows that on the current heavy + medium water
  shelves, the risk cells cluster exactly at the gas-oil contact
  and the hotspot ring (as the Slice A verdict hypothesised), then
  direction A's freeze rule can use that cluster directly and we
  know in advance which cells to anchor. No third Slice A retry
  needed.
- If the probe shows a diffuse pattern, then per-cell freeze is
  not enough either, and we should skip A and go to C (regime
  hysteresis) first.

Cost estimate: one extra `evaluate_residual` per clean-accepted
outer step — << 1 Newton iteration's worth of compute; strictly
additive, does not change physics or acceptance.

**Recommended pre-work:** land this probe first, before attempting
A. It is low-risk (diagnostic only, no controller change), and its
output changes which of A or C is the right next slice.

## Step 0 probe results — 2026-04-18

The basin-escape proxy probe was landed on current head (uncommitted
working tree, built on top of `c653fab`). Instrumentation: fires once
per materially-changed, retry-free clean accept when two prior clean
states exist; evaluates the residual of the globally-extrapolated
state (linear, dt_ratio-weighted, regime inherited from curr, Sw
clamped to [0,1]) and reports the scaled per-family inf-norm plus the
top contributing cell. Trace lines have the form:

```
BASIN-ESCAPE PROBE dt_ratio=<r> res_prev=<X> res_extrap=<Y> amp=<Y/X>
  top=[<family>] site=cellN(ijk=i,j,k) fam=[water=...,oil=...,gas=...]
```

**Neutrality check.** Probe adds one `evaluate_residual` per clean
accept with no controller path touched. On the heavy water case, the
summary line with probe active matches the pre-probe baseline
exactly: `substeps=16, accepts=15+4+8354, retries=0/9/0,
dt=[4.167e-5,3.864e-2]`.

**Results across the validation shortlist (current head, probe on):**

| Case | Probes fired | amp range | amp ≥ 1? | Top family | Top-cell pattern |
|------|--------------|-----------|----------|------------|------------------|
| `water-pressure 12x12x3 dt=1` | 10 | 0.49–0.86 | 0/10 | water (all) | ijk=(1,1,0)→(5,5,0) — tight, tracks advancing water front |
| `water-pressure 20x20x3 dt=0.25` | 6 | 0.16–3.24 | **1/6** | water (all) | cell63 ijk=(3,3,0) twice, cell464 (4,3,1), cell883 (3,4,2), cell84 (4,4,0) — tight cluster near early front |
| `water-pressure 22x22x1 dt=0.25` | 1 | 1.57 | **1/1** | water | cell92 ijk=(4,4,0) — front |
| `water-pressure 23x23x1 dt=0.25` | 1 | 1.55 | **1/1** | water | cell96 ijk=(4,4,0) — front |
| `gas-rate 20x20x3 dt=0.25` | 1 | 0.996 | 0/1 | gas | cell0 ijk=(0,0,0) — injector perforation |
| `gas-rate 10x10x3 steps=6 dt=0.25` | 14 | 0.076–0.672 | 0/14 | gas/oil | cell0–cell33, clustered near (0..3, 0..3, 0) — injector neighbourhood |

**Interpretation.**

1. Every observed amp ≥ 1 event is water and lives on the advancing
   front, not on a diffuse set: `(3,3,0)` on medium, `(4,4,0)` on
   both `22x22x1` and `23x23x1`, `(1,1,0)`..`(5,5,0)` on heavy (all
   of which happen to fall below amp=1 on this preset, but the
   top-cell geometry is still a tight front ring).
2. Shipped gas: probe fires frequently but amp stays well below 1
   (max 0.672). Risk cells cluster at the injector neighbourhood.
   Extrapolation is near-harmless here; regime hysteresis (direction
   C) would be attacking a different failure mode than what the
   probe measures.
3. No diffuse-risk case was observed across the shortlist.

**Decision.** Proceed with direction A (strict per-cell Newton
extrapolation with smoothness gating), using the observed front-ring
cluster as the anchor for the freeze rule. Skip direction C for this
slice. The freeze list for A on current head should include:

- cells in the `3`-cell Chebyshev ring around the latest clean-accepted
  `hotspot_site`, and
- cells that would flip phase regime (inherited from curr state in the
  probe, but re-evaluated by A).

Do **not** proceed to a second attempt at global extrapolation; the
two amp > 1 water cases already show the Slice A failure mode is live
at exactly the fronts the probe selects.

**Raw traces preserved at** `/tmp/basin-escape/{heavy-water,medium-water,dense-water,over-dense,bounded-gas,shipped-gas}.log`
(not committed; rerunnable from the commands listed in
`docs/FIM_CONVERGENCE_WORKLOG.md` Validation Shortlist).

## Cross-references

- Worklog: `docs/FIM_CONVERGENCE_WORKLOG.md` (active findings, all
  recent negatives recorded).
- Archive through 2026-04-06: `docs/FIM_CONVERGENCE_ARCHIVE_2026-03_to_2026-04-06.md`.
- OPM minimal mapping: `docs/OPM_FLOW_MINIMAL_MAPPING.md`.
- Slice A (the reverted Newton-extrapolation experiment):
  `docs/FIM_SLICE_A_EXTRAPOLATION.md`.
- Status dashboard: `docs/FIM_STATUS.md`.
- CPR implementation plan: `docs/FIM_CPR_IMPROVEMENT_PLAN.md`.
