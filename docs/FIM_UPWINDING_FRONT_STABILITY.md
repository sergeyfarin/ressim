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
