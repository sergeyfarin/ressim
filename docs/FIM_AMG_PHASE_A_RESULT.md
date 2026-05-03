# Phase A — port OPM-exact Quasi-IMPES weights and AMG defaults

**Branch:** `experiment/fim-amg-scirs2`
**Date:** 2026-05-03
**Status:** Phase A measured; OPM-exact restriction REGRESSES case 3
physics by -1.7% off converged reference. Not promotable. Reveals a
structural mismatch with ressim's unscaled Jacobian.

## What was ported

### OPM Quasi-IMPES weights (`getQuasiImpesWeights.hpp`)

Replaced ressim's existing pressure-restriction weights with the exact
formula from `opm-simulators/opm/simulators/linalg/getQuasiImpesWeights.hpp`:

```
bweights = (block^T)^-1 · e_p          (e_p = unit vector at pressure col)
bweights /= |max(bweights)|             (normalize)
```

Equivalently: `bweights = (block^-1)[0, :]` followed by max-abs
normalization. This is what OPM calls "Quasi-IMPES" (Wallis-style true
IMPES with local exact inversion).

Implemented in `build_pressure_transfer_weights` in
`src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs`. Test updated to
assert the OPM formula on a worked-out 3×3 example.

### OPM Dune-AMG defaults (`setupPropertyTree.cpp::setupDuneAMG`)

Replaced scirs2's `AMGOptions::default()` with OPM's exact AMG params:

| param | scirs2 default | OPM default | reason |
|-------|---------------:|------------:|--------|
| max_levels | 10 | **15** | deeper hierarchy |
| theta (alpha) | 0.25 | **0.333** | strength threshold |
| max_coarse_size | 50 | **1200** | OPM coarsens to ≥1200, then direct-solves |
| smoother | GaussSeidel | (OPM uses ILU0; not in scirs2) | kept GS as closest |
| pre/post_smooth | 1/1 | 1/1 | match |
| cycle | V | V | match |
| interpolation | Classical | (OPM defaults) | match |

Note: with `max_coarse_size = 1200`, AMG never builds a hierarchy on
ressim's 4-case shortlist (case 2 has n=1200, smaller cases have fewer)
— OPM's AMG falls through to direct-solve at the finest level.

## Measured result on 4-case shortlist

| case | master | Phase A (OPM-exact) | verdict |
|------|-------:|-------------------:|---------|
| 1 (medium-water 1 step) | 4 / ~5,500   | 4 / 4,845   (-12%) | win |
| 2 (medium-water 6 step) | 30 / ~50,000 | **32 / 40,254** (subs +7%, lin_ms -20%) | partial win |
| 3 (heavy-water 12x12x3) | 32 / ~4,300  | **128 / 21,099** (subs +300%, lin_ms +391%) | severe regression |
| 4 (gas-rate 10x10x3)    | 28 / ~1,800  | 28 / 8,132 (lin_ms +351%) | regression |
| Case 3 fine-dt FOPT     | 3826 (OPM)   | **3760.72** (-1.7% vs OPM ref) | **physics drift** |

## Why OPM-exact didn't work

The OPM Quasi-IMPES weights formula
`bweights = (block^-1)[0, :]` produces weights that depend on the
relative magnitudes of pressure-derivative vs saturation-derivative
entries in the local 3×3 block. The formula's intent is to cancel the
saturation-derivative entries when forming the coarse pressure
equation.

The catch: **OPM applies these weights to a scaled Jacobian**
(equation rows divided by `pv_over_dt / B_phase`, etc.). In OPM, every
cell-component-mass row has natural magnitude O(1) by the time the
weights are computed. ressim does NOT apply the existing
`EquationScaling` to the Jacobian before passing it to the linear
solver — the audit's Finding 2, refuted as a standalone fix in the
2026-04-19 LINSCALE Stage 1 sweep.

Consequence: in ressim, the local 3×3 block has entries dominated by
`d_water_d_p ≈ pv/(Bw·dt) ≈ 80` while saturation-derivative entries
are O(1). The "exact" `(block^-1)[0, :]` weights compensate for this
scale, not the underlying physics. The resulting coarse equation has
the wrong basis — it weights the saturation cancellation by ~80×
where OPM's same formula on a scaled block would weight it by ~1×.

This is a **structural mismatch**, not a tunable parameter. To make
OPM Quasi-IMPES work in ressim, we'd need:
1. Apply `EquationScaling` to the Jacobian before passing to the linear
   solver (refuted as a standalone fix; needs to be paired with the
   restriction change).
2. OR rewrite ressim's accumulation Jacobian assembly to natively
   produce O(1) rows.

Both are larger structural changes than Phase A targeted.

## Why case 3 fine-dt physics drifted

Case 3 fine-dt FOPT went 3826 (OPM converged ref) → 3760.72 with
Phase A's restriction. That's -1.7% off OPM's converged answer — a
correctness regression, not just performance.

The case-3 dt=1 attempt also regressed substeps 32→128 with FOPT
3191 vs OPM 3826 (-17% off). Both numbers are worse than the bundle B
variant in commit `db06fba` (FOPT 3827 fine-dt, FOPT 3837 dt=1).

This means **the OPM-exact weights, applied to ressim's unscaled
Jacobian, are giving Newton an actively-wrong update direction** that
sometimes converges to a wrong basin (case 3 physics drift).

## Verdict

**Phase A REFUTED.** Don't promote. The "copy OPM exactly" hypothesis
turns out to require copying multiple coupled pieces (restriction
weights AND equation scaling AND Jacobian assembly conventions) — not
just the restriction.

Returning to master baseline at commit `48da61b`. Branch
`experiment/fim-amg-scirs2` carries the Phase A attempt for the
record.

## What this teaches us

1. **OPM ports can't be partial.** The restriction weights only make
   sense in the context of OPM's equation-scaling pipeline. The
   bisection memo's finding that "case-3/4 cost is structural" is
   confirmed and sharpened: it's specifically a scaling-pipeline
   structural mismatch, not just a coarse-solver-spectrum issue.

2. **The 2026-04-19 LINSCALE refutation matters more than I credited.**
   That experiment showed `D_r·J·D_c` row+column scaling alone causes
   case 2 to stall and case 3 to regress. The conclusion at the time
   was "scaling alone won't help with current ILU0 chain" — which
   was correct. But the deeper truth is that the restriction operator
   and the equation scaling are paired pieces; you can't use OPM's
   restriction without OPM's scaling, and you can't usefully apply
   OPM's scaling to the current restriction.

3. **Path forward: either commit to the full OPM port (scaling +
   restriction + AMG + ILU0-smoother, all together as a multi-week
   project) OR pivot to mechanisms that don't depend on equation
   scaling.** The audit's Fix 5 (line search) and the wide-analysis
   doc's Schur-complement well elimination are both
   scaling-independent levers worth considering before another full-
   stack OPM port attempt.

## Files of record

- This branch's commit (next).
- `worklog/phase-a/` — A/B run logs (git-ignored).
- `docs/FIM_AMG_INTEGRATION_PLAN.md` (master 48da61b) — the original
  Phase A plan; this doc supersedes the optimistic "copy OPM exactly"
  framing.
- `docs/FIM_AMG_SPIKE_RESULT.md` (this branch) — Phase 2 spike
  context, where AMG-as-coarse-solver was wired and partially
  measured.
