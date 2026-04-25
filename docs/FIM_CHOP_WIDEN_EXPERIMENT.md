# Fix A3 Option A — Widen the fw inflection-chop threshold

**Status:** PROMOTED to master 2026-04-25 with `FW_INFLECTION_OVERSHOOT_FACTOR = 1.2`.
**Originating branch:** `experiment/fim-chop-widen-threshold` (kept as record).

## What the experiment does

Changes the Wang-Tchelepi inflection-chop predicate from "fire on
any crossing" to "fire only when the proposed step would overshoot
the inflection point by `FW_INFLECTION_OVERSHOOT_FACTOR ×
dist_to_inflection`."

Single new constant `FW_INFLECTION_OVERSHOOT_FACTOR` in
`src/lib/ressim/src/fim/newton.rs`. Default 1.2 selected after a
small sweep (k ∈ {1.2, 1.5, 2.0}); k=1.2 preserves case-3
correctness (oil within 0.3% of converged reference) while
unlocking case-2 lin_ms gains.

Setting the constant to 1.0 exactly reproduces the original
Wang-Tchelepi 2013 chop (any crossing fires), so this is a strict
generalization.

## Result vs master with-chop k=1.0 baseline

| case | substeps with→k1.2 | lin_ms with→k1.2 | dt-run FOPT | fine-dt FOPT |
|------|-------------------:|-----------------:|------------:|-------------:|
| 1    | 7 → **4** (-43%)   | 13,920 → **5,667** (-59%) | 3326 → 3289 | n/a |
| 2    | 34 → **30** (-12%) | 60,910 → **46,620** (-23%) | 3602 → 3598 (-0.1%) | 3609.73 → **3609.73** (bit-exact) |
| 3    | 27 → **32** (+19%) | 3,517 → **3,687** (+5%) | 3883 → 3837 (-1.2%) | 3847 → **3826** (matches OPM converged 3826) |
| 4    | 28 → **28**        | 1,628 → 1,656 (+2%) | unchanged | n/a |

**Total: substeps 96 → 94 (-2%), lin_ms 79,975 → 57,630 (-28%).**

Notably, **case 3 fine-dt at k=1.2 (FOPT=3826) lands closer to
the OPM converged reference (3826) than master with-chop k=1.0
fine-dt (3847)** — a small but real correctness improvement on the
case where Option B catastrophically failed.

## k-value sweep on case 3 (the discriminator)

| k    | substeps | FOPT (dt=1) | inflection-bound iters / total |
|-----:|---------:|------------:|------------------------------:|
| 1.0  | 27       | 3883        | 326/402 (81%)                 |
| 1.2  | 32       | 3837        | 247/350 (71%)                 |
| 1.5  | 42       | 3793        | (not computed)                |
| 2.0  | 139      | 3191        | 182/1010 (18%)                |
| ∞    | 162      | 3019        | 0% (Option B branch)          |

Sweet spot: **k=1.2** preserves correctness and gains lin_ms.
k≥1.5 starts trending toward Option B's wrong basin.

## Validation

- All Rust tests pass (Buckley-Leverett A/B + smaller-dt
  refinement, Dietz PSS, SPE1 first-steps + gas-injection +
  breakthrough, Appleyard damping). 298 passing tests, 6
  pre-existing failures unchanged from master.
- Case 2 fine-dt (48 × dt=0.03125): FOPT bit-exact match to
  master with-chop fine-dt (3609.73).
- Case 3 fine-dt (16 × dt=0.0625): FOPT=3826, matches OPM
  dt=0.015625 converged ref (3826) within 0.01%.

## Why k=1.2 works

The damping breakdown probe shows that on case 2/3, ~93% / 81%
of Newton iters bind on `sw_inflection`. With k=1.0 (the
classical chop), every "marginal" crossing (where the proposed
step lands just past the inflection point) gets chopped down
hard. This is over-conservative: the basin-jump risk only
materializes when the step lands FAR past the inflection.

k=1.2 lets through crossings that overshoot by ≤20% — the
largest population of front-cell iters — while still chopping
the wild updates (the "deep" overshoots that Wang-Tchelepi
2013 was actually targeting). Case-3 binding rate drops from
81% to 71% (we still chop the high-stakes crossings).

## Why this didn't show up in Option B

Option B (k=∞, full removal) lost the basin-jump protection
entirely. Case 3 oil dropped from 3883 to 3019 because Newton
started landing in the wrong shock-speed basin on the deep
overshoots, integrating to a wrong saturation distribution.

Option A (k=1.2) only changes the firing threshold for
small-overshoot crossings, leaving the deep overshoots
chopped. The basin-jump guard is intact for the cases where it
matters.

## Validation evidence at promotion time

- cargo check + 298 Rust tests pass (Buckley A/B + dt-refinement,
  Dietz PSS, SPE1 first-steps + gas-injection + breakthrough,
  Appleyard damping). Same 6 pre-existing failures as the master
  baseline at commit `260f61e`; no new failures.
- Case 2 fine-dt (48 × dt=0.03125): FOPT bit-exact match to master
  with-chop fine-dt (3609.73). Same physics.
- Case 3 fine-dt (16 × dt=0.0625): FOPT=3826.36, matches OPM
  dt=0.015625 converged ref (3826.12) within 0.01%.

## How to re-verify

```bash
bash scripts/build-wasm.sh

# 4-case shortlist
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 20x20x3 --steps 6 --dt 0.25 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate \
  --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic step --no-json

# Fine-dt physics references
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 20x20x3 --steps 48 --dt 0.03125 --diagnostic outer --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 12x12x3 --steps 16 --dt 0.0625 --diagnostic outer --no-json
```

A/B logs from the original promotion runs are preserved in
`worklog/fix-a3-option-a/` (git-ignored under `/worklog/`).
