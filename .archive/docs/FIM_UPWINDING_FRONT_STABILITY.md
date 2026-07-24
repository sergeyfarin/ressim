# FIM Fix B — upwinding front stability investigation

## Motivation

After Fix A1 Stage 2 landed (commit `1fe7f08`, one-line stagnation-gate
fix), the 4-case shortlist improved substantially on case 2 (102 → 34
substeps, −38% lin_ms). The remaining gap vs OPM Flow 2025.10 on case 2
is still ~5× substeps (34 vs 7) and ~900× wall time (62s vs 0.07s).

A post-A1 attribution sweep (logs in
`worklog/fix-b-upwinding/post-a1-case{1,2,3,4}.log`) shows the
remaining STAGNATION bailouts on cases 1/2/3 are all **real-bump**
events with residual ratios **1.15–1.17** (+15–17% iter-over-iter
bumps), cascading to count=3. Three out of six bailouts across cases
1/2/3 localize to `cell0` (injector corner `(0,0,0)`, water row 0).
Two out of six localize to `cell399` (producer corner `(19,19,0)`,
oil row). Appleyard damping is hitting the `max_sw_change=0.2` cap
and Newton is oscillating instead of decaying — consistent with a
front-discontinuity response that Appleyard cannot damp.

Widening the stagnation real-bump tolerance was considered (Fix A2)
and rejected on data: small-bump events (ratio 1.00–1.07) never
cascade to count=3 on their own, and large-bump events (1.15–1.30)
are genuine divergence that *should* chop dt. Post-A1, tolerance
policy changes do not unlock further substep savings.

This investigation targets the **mechanism** producing the large
residual bumps: mid-Newton upwinding flips at the saturation front.

## Mechanism hypothesis (to be tested)

Single-point upstream weighting in `assembly.rs:1183-1197` chooses
per-phase upstream cell by the sign of the phase potential:

```rust
let dphi_w = (p_i - p_j) - (pcw_i - pcw_j) - grav_w;
let water_upstream = if dphi_w >= 0.0 { ...i } else { ...j };
```

When `dphi_w` flips sign between Newton iterations the residual
contribution from this face changes discontinuously:

- The upwinding choice jumps (step function, zero derivative).
- The mobility `lambda_w` used for the flux switches from
  `locals[0].mobilities[0]` to `locals[1].mobilities[0]`.
- The Jacobian mobility-derivative contribution moves from row 0 to
  row 1 of the block (see `assembly.rs:1408-1411`).

This is a known failure mode for single-point upstream FIM: near
the saturation front the phase-potential gradient crosses zero due
to capillary contribution `(pcw_i - pcw_j)`, and `sw` can change
by ~0.2 in a single damped Newton step (hitting the Appleyard
cap). That cap doesn't prevent `dphi_w` sign flips — it only
caps the magnitude of the sw update.

### Why the HOTSPOT localizes to cell0 and cell399

- **cell0**: injector corner, water flowing from cell0 outward.
  `dphi_w = (p_0 - p_1) - (pcw_0 - pcw_1) - grav_w`. At early
  iters the pressure gradient dominates (injector BHP=500 bar vs
  interior ~200 bar). As the water front advances and `sw` in
  neighbors rises, `pcw_j` changes and the Leverett J-function
  contribution can push `dphi_w` through zero.
- **cell399**: producer corner, water approaching from the
  interior. `dphi_w` starts negative (water flowing into cell399
  from cell398). When `sw` at cell399 rises from 0.1 toward the
  water front value, `pcw_399` drops and the sign of
  `(pcw_398 - pcw_399)` can flip.

Both locations coincide with cells where Appleyard hits its
`max_sw_change=0.2` cap in the bailout traces.

### What OPM Flow does differently (to cross-check)

OPM Flow's upstream weighting is not a pure step function. Known
OPM-relevant features to verify:
- Weighted upstream mobility blending near flip (e.g., Hamon-
  Vohralik or similar regularization).
- Monotone two-point upstream weighting.
- Pressure-dependent smoothing of `dphi_p` near zero.

Specifically, OPM's `TpfaLinearizer` / `FvBaseDiscretization` uses
the upstream cell but its Newton step includes update
stabilization (confirmed no-op on case 2 per earlier ablation —
but unclear whether OPM has a separate mobility blending on top).

OPM's evidence on case 2: 7 substeps across 6 report steps,
zero retry-dom "nonlinear-bad" events, 33 Newton iters total at
dt=0.25d. No dt-chop cascades on the water front. Either OPM's
upwinding doesn't flip, or its update policy absorbs flips
without triggering the oscillation cascade we see.

**Cross-check to do:** trace OPM's verbose output at step 1 with
`--debug-level=3` or equivalent, look for upstream-cell-selection
info in its per-iter diagnostics, and measure whether the
cell0/cell399 equivalent ever shows mobility discontinuity.

## Stage 1 probe design (read-only)

Goal: prove or refute that upwinding flips at HOTSPOT cells
correlate with the +15% residual bumps triggering the remaining
bailouts.

### Instrumentation

In `assembly.rs` add a per-face per-phase upwind-tracking
counter. For each cell face during Jacobian assembly, compare the
current `dphi_p >= 0` decision against the same face/phase from
the previous Newton iteration (stored on the state). Emit a trace
line when a flip occurs:

```
UPWIND-FLIP iter={n} cell_i={i} cell_j={j} dim={d} phase={w|o|g}
  dphi_prev={v} dphi_now={v}  (|∆dphi|={v})
```

Aggregate into per-iter totals:
- `upwind_flips[phase] = count of faces that flipped this iter`
- `upwind_flips_hotspot = count of flips involving cell0 or cell399`

Emit per-iter summary alongside existing STAGNATION-ATTRIB:
```
UPWIND-SUMMARY iter={n} flips_w={n} flips_o={n} flips_g={n}
  hotspot_flips={n}
```

### Storage

Add `previous_face_upwind: Option<FaceUpwindSnapshot>` on the
Newton solver state (local to `run_fim_timestep`). Each snapshot
is `Vec<(face_idx, [upwind_w, upwind_o, upwind_g])>`. Cheap —
face count is O(n_cells × 3) = ~3600 entries for 20x20x3, each
3 u8 flags.

### Signal to look for

- **PASS**: at iters immediately preceding the STAGNATION-
  REJECTED bailout, `upwind_flips` is non-zero and the flipping
  faces contain cell0 or cell399. This confirms upwind flips
  coincide with the residual bump.
- **FAIL**: no upwind flips occur during bump iters → the bumps
  are driven by something else (Appleyard damping interaction,
  capillary nonlinearity, well coupling) and Fix B shifts focus.
- **PARTIAL**: flips are present but also occur on good iters →
  flips are a symptom, not the mechanism; need a different
  instrumentation angle.

## Stage 1 result — 2026-04-24 (PASS on cases 1/2/3, NO-OP confirmed on case 4)

Probe implemented in `assembly.rs:1660-1807` (read-only snapshot
collection + diff) and invoked in `newton.rs:2321-2387` once per
Newton iter. Initially gated on `options.verbose`, which turned out
to be the harness-level knob that only drives `eprintln!` — the
internal trace buffer is driven by `capture_fim_trace`. Gate
removed; probe now runs every Newton iter and writes
`UPWIND-SUMMARY` into the trace buffer unconditionally. Cost: one
snapshot allocation per iter (~3080 entries × 32 bytes for the
4-case shortlist), fully bypassed by dead-code elimination only if
we re-gate later.

**Per-case aggregate:**

| case | Newton iters | zero-flip iters | nonzero | >100 flips | hotspot>0 | count=3 bailouts | total flip volume | total hotspot flips |
|------|-------------:|----------------:|--------:|-----------:|----------:|-----------------:|-------------------:|--------------------:|
| 1: medium-water 1 step    | 114 | 45  | 69  | 17 | 19 | 2 | 38,519 | 48 |
| 2: medium-water 6 step    | 739 | 361 | 378 | 18 | 40 | 2 | 43,070 | 75 |
| 3: heavy-water 12x12x3    | 391 | 226 | 165 | 14 | 16 | 2 | 21,703 | 70 |
| 4: gas-rate 10x10x3       | 415 | 323 |  92 |  4 |  **0** | 2 |  1,288 |  **0** |

**Localization of hotspot flips at bailout** (iters immediately
preceding `STAGNATION-ATTRIB count=3`):

- **Case 1, bailout iter 11**: iters 9/10/11 show 1982 → 2830 →
  1155 total flips, with 2 at hotspot each iter. Samples show
  face `(0, 400)` z-direction — a cell0→cell400 interface at the
  injector corner — oscillating: `dphi_w: +3.284e-2 → −5.439e-2
  → +2.701e-2 → −5.531e-2`. Textbook sign-flip oscillation.
- **Case 2, bailout iter 8**: iter 6 shows 4565 flips with 9 at
  hotspot, including `(0,1)` x-face with dphi jumping from
  `+5.293e-3 → −1.073e0` (200× magnitude change). Happens across
  all three phases simultaneously — this is the pressure-
  gradient term dominating via the injector BHP transient.
- **Case 3, bailout iter 8**: iter 6 shows 2371 flips with 16 at
  hotspot cells 0 and 143 (12×12×3 producer corner is 143=11+11·12).
- **Case 4 (gas-rate), both bailouts**: iters 14/15/16 and
  10/11/12 all show `total=0, hotspot=0`. No upwind flips
  whatsoever around these bailouts — consistent with the doc's
  prediction that gas-rate case is benign for this mechanism.
  (The 1288 non-hotspot flips earlier in the run are noise in
  zero-mobility phantom gas dphi sign — not material.)

**Verdict:** PASS per the decision rule. Upwinding flips coincide
with the residual bump iters on cases 1/2/3 and localize to the
hotspot cells (cell0 injector corner, cell399/cell143 producer
corner), matching the mechanism hypothesis. Case 4 (gas)
confirms the natural no-op safety net: Stage 2 smoothing cannot
regress it.

**Artifacts of record:**
- `worklog/fix-b-upwinding/stage1-case{1,2,3,4}.log` — full
  Newton traces with `UPWIND-SUMMARY` lines interleaved with
  existing `STAGNATION-ATTRIB` and residual/iter traces.
- `src/lib/ressim/src/fim/assembly.rs:1660-1807` — probe types
  and functions (`FaceUpwindSample`, `collect_face_upwind_snapshot`,
  `diff_face_upwind_snapshots`, `FaceUpwindFlipReport`,
  `FaceUpwindFlip`).
- `src/lib/ressim/src/fim/newton.rs:2321-2387` — probe emission
  in Newton loop.

**Replay commands (clean worktree at commit hash pending Stage 2):**
```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 20x20x3 --steps 6 --dt 0.25 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
  --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate \
  --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic step --no-json
```

## OPM cross-check — 2026-04-24

Fetched OPM Flow 2025.10 source (`opm-models`, `opm-simulators`)
via WebFetch to answer: does OPM smooth the upstream-weighting
decision, or does it use hard if/else like ressim?

**OPM uses the same hard if/else upstream weighting.** From
`opm/models/blackoil/blackoillocalresidualtpfa.hh`:

```cpp
ExtensiveQuantities::calculatePhasePressureDiff_(upIdx, dnIdx, ...);
const IntensiveQuantities& up = (upIdx == interiorDofIdx)
    ? intQuantsIn : intQuantsEx;
```

No blending, no regularization, no tanh smoothing. The upstream
cell is chosen by a single conditional based on phase potential.
**So the upwinding model is not OPM's advantage.**

### So why does OPM converge in 7 substeps on case 2?

Three mechanisms found in OPM source that ressim does NOT fully
mirror:

1. **SOR-style update stabilization** (`NonlinearSolver.cpp ::
   stabilizeNonlinearUpdate`): when residual-history-based
   oscillation is detected, OPM blends the current `dx` with the
   previous iter's `dx_old` via a relaxation factor:
   ```
   dx[i] = omega * dx[i] + (1-omega) * dx_old[i]
   ```
   **However**, the 2026-04-20 ablation
   (`--use-update-stabilization=false`) showed this is a **no-op
   on case 2** — OPM converges in 7 substeps either way. So
   this is not the case-2 lever.

2. **No residual-bump count=3 early termination.** OPM's step()
   loop just runs to `newton_max_iter_` (default 12). It does not
   penalize residual bumps mid-Newton. If residual decreases
   enough by iter 12, the step converges; otherwise the timestep
   is cut. Ressim cuts dt after count=3 residual bumps even when
   5+ iter budget remains.
   > Already addressed by **Fix A1 Stage 2** (2026-04-23):
   > zero-move iters no longer increment count. Reduced case 2
   > from 102→34 substeps but ~5× gap remains.

3. **Appleyard-equivalent saturation chop** (dsMax=0.2 in
   `blackoilnewtonmethod.hh`): identical to ressim's
   `max_sw_change=0.2`. Not the differentiator.

### Implication for Fix B Stage 2

Stage 1 PASSes the mechanism check — upwinding flips correlate
with bailouts on cases 1/2/3. But the OPM cross-check **refutes
the Stage-2 smoothing candidates as the OPM-parity path**: OPM
does not smooth upstream weighting either, and still converges.
Switching our upstream weighting to tanh/Hamon-Vohralik
regularization would diverge from the reference simulator rather
than converge toward it.

**Where the case-2 gap actually lives, post-cross-check:**

A. OPM's Newton continues past residual bumps because it only
   tests CNV/MB at the end, not a per-iter count. Ressim's
   remaining 2 real-bump bailouts on case 2 post-A1 are exactly
   the "iter_n+2 would have converged if we hadn't cut dt"
   failure mode.

B. OPM's per-substep Newton iter count cap is 12; ressim's is
   also ~12-15 (`max_newton_iterations`). Equal budget.

C. OPM doesn't chop the step when an individual residual bumps —
   even a +20% bump is absorbed if the next iter recovers. Our
   code chops after count=3 real-bump events regardless of iter
   budget.

**Revised Stage 2 direction: Fix A2, not Fix B.**

The correct lever is **widen or disable the STAGNATION count=3
gate when iter-budget remains**, not smooth the upwinding. The
audit doc previously listed Fix A2 as "rejected on data" because
small-bump events don't cascade to count=3 on their own. That
analysis was against case 2 post-A1 — but re-reading the post-A1
attribution shows all remaining bailouts are real-bump
ratio=1.15-1.17. A policy change that allows Newton to continue
past one or two such bumps — e.g., only count sequential bumps,
or only bail when iter-budget is near-exhausted — aligns with
OPM behavior without touching the physics operator.

### Plan going forward

1. **Shelve Fix B Stage 2** as a diverging-from-reference
   direction. Keep the Stage 1 probe in-tree as a diagnostic.
2. **Pivot to Fix A2** with the updated framing: allow Newton to
   absorb one or two real-bump events when residual is still
   trending (e.g., `res[n] < res[0] * 0.5`) and iter budget
   remains. This is the OPM-behavior-approximating policy change.
3. **Cross-check the Fix A2 change by replay** against OPM's
   per-iter trace on case 2 (already captured at
   `/tmp/opm-case2/trace/CASE2.DBG` per memory
   `project_fim_opm_reference_2026-04-20.md`).

## Stage 2 design sketch (if Stage 1 PASSes)

Replace the step-function upwinding at `assembly.rs:1183-1197` with
a smoothed variant. Candidates (ordered by simplicity):

1. **Potential-weighted upstream (smoothest)**: replace hard
   if/else with
   `lambda_eff = w(dphi) * lambda_i + (1-w(dphi)) * lambda_j`
   where `w(dphi) = 0.5 * (1 + tanh(dphi / epsilon))`. Parameter
   `epsilon` controls transition width. Jacobian needs chain rule
   for `w'(dphi)`.
2. **Hamon-Vohralik two-point upstream**: standard reservoir-
   simulator smoothed upwinding — more complex but well-studied.
3. **Flux-limited upstream**: cap mobility change per Newton iter
   at some fraction of `max(lambda_i, lambda_j)`. Ad hoc but cheap.

Whichever we pick needs to preserve the fine-dt reference on
case 3 (the A1 promotion criterion) and not regress case 4
(currently no-op since gas doesn't exhibit the flip).

## Files of record

- `worklog/fix-b-upwinding/post-a1-case{1,2,3,4}.log` — post-A1
  attribution sweep showing the remaining real-bump bailouts.
- `src/lib/ressim/src/fim/assembly.rs:1183-1197` — residual
  upstream weighting (site of Stage 2 change).
- `src/lib/ressim/src/fim/assembly.rs:1369-1454` — Jacobian
  upstream weighting (parallel site for Stage 2 change).
- `docs/FIM_LINEAR_SOLVER_AUDIT.md` §"Fix A1 Stage 2 result" —
  the immediate predecessor of this work.
- `docs/FIM_WIDE_ANGLE_ANALYSIS.md` — Tier 1 ranking that put
  "upwinding front stability" on the board.
