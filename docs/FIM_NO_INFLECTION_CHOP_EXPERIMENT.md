# Experiment: Remove Wang-Tchelepi fw inflection-point chop (Option B)

**Branch:** `experiment/fim-no-inflection-chop`
**Started:** 2026-04-25
**Status:** initial A/B captured; OPM case-3 reference run pending

## What the experiment does

Removes the per-cell saturation chop in `appleyard_damping_breakdown`
(formerly `appleyard_damping`) that prevented Newton from crossing
the water fractional-flow inflection point in a single iter. The
chop was added 2026-03-31 (commit `325e19b`) as a safety guard
during the FIM convergence push; the Fix A3 Stage 1 probe
(2026-04-24, master commit `260f61e`) showed it binds 93% of
case-2 Newton iters and OPM Flow's reference uses no such guard
(only `dsMax=0.2`, identical to ressim's `max_saturation_change`).

Helpers `fw_at_sw`, `fw_inflection_point_sw`, the
`inflection_crossings` counter, and the chop block itself are
deleted on this branch. `appleyard_damping_breakdown` no longer
needs the `&ReservoirSimulator` parameter.

## Initial 4-case A/B (vs master commit `260f61e`)

| case | substeps with→without | lin_ms with→without | avg_p with→without | oil with→without |
|------|----------------------:|--------------------:|-------------------:|-----------------:|
| 1: medium-water 1 step   | 7  → 8   | 13,920 → 4,602  | 328.49 → 329.44 | 3326.63 → 3340.82 |
| 2: medium-water 6 step   | 34 → 13  | 60,910 → 5,340  | 348.07 → 346.22 | 3602.46 → 3555.17 |
| 3: heavy-water 12x12x3   | 27 → 162 | 3,517 → 14,123  | 353.82 → 365.12 | 3882.89 → **3018.88** |
| 4: gas-rate 10x10x3      | 28 → 28  | 1,628 → 1,594   | 275.96 → 275.96 | 161.92 → 161.92  |

Logs preserved in `worklog/fix-a3-damping-probe/stage2-option-b/case{1,2,3,4}.log`
(git-ignored; on-disk for the duration of this experiment).

Fine-dt reference for case 2 (master `260f61e`, dt=0.03125 × 48):
**avg_p = 348.41, oil = 3609.73**.

Fine-dt reference for case 3 (master, dt=0.0625 × 16, recorded
in `project_fim_opm_reference_2026-04-20.md`):
**avg_p = 354.80, oil = 3858.10**.

## What changed and why

- **Cases 1, 2, 4: improvement or no-op.** Big lin_ms drops on
  cases 1/2 (−67%, −91%) at the cost of small physics drift
  (oil −1.5% on case 2 vs fine-dt). Case 4 is bit-exact (gas-only
  problem; chop never fired there).
- **Case 3: severe regression.** Substeps blow up 27→162 and oil
  drops 22% vs fine-dt reference. The dt-cut loop hits the
  minimum dt (`dt=[2.220e-16, 1.333e-1]` per the case-3 log).
  retry_dom = `nonlinear-bad:water@429`.

## The open question

**Is the case-3 oil drop the chop doing real correctness work, or
is the chop masking a different bug that ressim shares with OPM
only when the chop is absent?**

Two hypotheses:

1. **Chop is correctness-essential.** Without it, Newton at dt=1
   on heavy-water 12×12×3 jumps across the fw inflection on
   front-cell iters and lands in the wrong shock-speed basin.
   The state converges (substep accepts) but to a different
   saturation distribution than the physically correct one. If
   true, Option B is dead and we look at Option A (widened
   threshold) instead — or we accept that the chop is a permanent
   ressim addition that OPM doesn't need because OPM's CPR /
   summed-IMPES restriction produces less-oscillatory initial
   updates.

2. **Chop is masking another bug.** Removing it exposes a
   weakness in the dt-growth policy, the linear solve direction,
   or the equation scaling. Maybe at dt=1 ressim's Newton
   *should* take many substeps but the chop was tricking the
   stagnation/growth machinery into accepting a wrong answer in
   27 substeps. If true, the with-chop "baseline" oil=3882 was
   wrong all along and we have a bigger bug to find.

## Investigation plan

### Phase 1: OPM reference for case 3 (this branch)

Translate `water-pressure --grid 12x12x3 --steps 1 --dt 1` to an
OPM Flow deck (mirror the 2026-04-20 case-2 translation in
`project_fim_opm_reference_2026-04-20.md`). Run with default
`cprw` preconditioner. Record: substeps, Newton iters, linear
iters, wall time, final FPR (avg_p), final oil-production rate.

Expected outcome decides which hypothesis to follow:

- **OPM lands at oil ≈ 3858** (matches fine-dt ref): chop is doing
  real correctness work. Hypothesis 1 confirmed. Try Option A
  (widened threshold) or accept the chop as a permanent ressim
  addition.
- **OPM lands at oil ≈ 3018** (matches Option B without-chop): chop
  was masking wrong physics. Hypothesis 2 confirmed. The
  with-chop "baseline" was wrong; investigate why fine-dt also
  gives 3858 (does fine-dt also share the bug? does fine-dt with
  the chop disabled give 3018?).
- **OPM lands somewhere else entirely**: new diagnostic data to
  consider.

### Phase 2: localize the divergence (after Phase 1)

If Hypothesis 1: probe iter-by-iter on case 3 step 1. Capture
which face/cell first crosses the inflection without the chop, and
what saturation distribution diverges from the with-chop trajectory.
Focus on:
- Whether the divergence is at the front cells only, or at the
  injector/producer corners.
- Whether widening the chop threshold (Option A:
  `max_damping * |dsw_signed|` ≥ `2 × dist_to_inflection`) restores
  case 3 while keeping the case 2 lin_ms gain.

If Hypothesis 2: rerun the fine-dt reference on this branch
(without chop). If fine-dt also drops to 3018, the chop is
correctness-essential downstream too. If fine-dt without chop
still gives 3858, the chop is genuinely masking something only at
larger dt.

### Phase 3: decision

Promote Option B to master (rare best case), promote Option A,
keep the chop as-is and document why ressim diverges from OPM here,
or shelve and move to a different lever (line search, equation
scaling, CPR completion).

## How to reproduce on this branch

```bash
# 4-case shortlist
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 20x20x3 --steps 6 --dt 0.25 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate \
  --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic step --no-json

# Fine-dt reference for case 2 (without-chop on this branch)
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 20x20x3 --steps 48 --dt 0.03125 --diagnostic outer --no-json

# Fine-dt reference for case 3 (without-chop on this branch)
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 12x12x3 --steps 16 --dt 0.0625 --diagnostic outer --no-json
```

## How to abandon the experiment

```bash
git checkout master
git branch -D experiment/fim-no-inflection-chop
```

Master is untouched by this experiment; only the Stage 1 probes
landed there (commit `260f61e`).
