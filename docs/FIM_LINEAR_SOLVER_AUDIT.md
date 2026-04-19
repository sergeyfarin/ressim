# FIM linear solver audit — 2026-04-19

Read-only audit of the FIM linear stack, triggered by the wide-angle analysis
(`docs/FIM_WIDE_ANGLE_ANALYSIS.md`) pointing at "CPR is probably incomplete as
labeled" as the likely root cause behind the medium-water convergence pain that
five prior directions (A/B/E, Jacobian reuse, bypass trigger narrowing) all
failed to resolve.

No code was changed. This doc is the diagnosis + ranked fix candidates to take
into the next session.

## Scope of the audit

Files read end-to-end:
- `src/lib/ressim/src/fim/linear/mod.rs` (339 lines) — backend dispatch,
  threshold logic, layout struct.
- `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs` (2203 lines) — the
  file used for BOTH `FgmresCpr` and `GmresIlu0`. CPR preconditioner build,
  pressure-transfer weights, fine-smoother selection (block-Jacobi vs ILU(0)),
  fGMRES outer loop, restart-stagnation / dead-state / tiny-residual-tail
  exits, BiCGSTAB pressure coarse solver with pressure-level ILU(0).
- `src/lib/ressim/src/fim/scaling.rs` (148 lines) — `EquationScaling` and
  `VariableScaling` builders.
- Targeted reads in `src/lib/ressim/src/fim/newton.rs` (the call site at
  line 2681–2740 that actually hands the Jacobian to the linear backend, plus
  the `scaled_residual_inf_norm`, `scaled_update_inf_norm`, and
  `residual_family_diagnostics` users of the two scaling structs).
- `src/lib/ressim/src/fim/assembly.rs` lines 1–200 — confirmed the block
  layout: row `i*3 + {0,1,2}` = (water, oil, gas) component mass residual;
  col `i*3 + {0,1,2}` = (pressure_bar, sw, hydrocarbon_var).

## What the audit found

### Finding 1 (confirmed): "fgmres-cpr" and "gmres-ilu0" dispatch to the same function — with CPR on by default

In `linear/mod.rs` lines 215–219:

```rust
// CPR is still incomplete, but the default FIM path now uses a pressure-first
// two-stage iterative backend instead of falling straight back to sparse LU.
FimLinearSolverKind::FgmresCpr => {
    gmres_block_jacobi::solve(jacobian, rhs, options, layout, false)
}
```

Both `FgmresCpr` and `GmresIlu0` dispatch into `gmres_block_jacobi::solve`.
The branching happens inside `solve` (line 1584 onward):

```rust
let cpr_fine_smoother_kind = if options.kind == FimLinearSolverKind::FgmresCpr {
    CprFineSmootherKind::FullIlu0
} else {
    CprFineSmootherKind::BlockJacobi
};
```

and again in `solve_with_cpr_fine_smoother` at line 1181:

```rust
let use_pressure_correction = options.kind == FimLinearSolverKind::FgmresCpr;
```

So the **only** differences between `FgmresCpr` and `GmresIlu0` are:
1. `FgmresCpr` uses full-matrix ILU(0) as the fine smoother;
   `GmresIlu0` uses block-Jacobi with 3×3 cell blocks.
2. `FgmresCpr` prepends a pressure-correction stage before the fine smoother;
   `GmresIlu0` does not.

Both otherwise share the same outer-GMRES loop, restart policy, residual
estimation, and termination heuristics.

The "CPR is still incomplete" comment was committed in the current tree — it is
not stale. This matches the TODO item 5 claim and is the starting point for
the rest of the audit.

### Finding 2 (REFUTED BY STAGE 1 PROBE, 2026-04-19): row/column scaling applied before solve does NOT reduce lin_ms

**Summary (post-probe):** the "no row/col scaling" structural finding is
correct (the Jacobian IS handed to the backend unscaled), but the proposed
Fix 1 — applying `D_r * J * D_c` scaling — was measured on the 4-case
shortlist in all four modes (Off / RowOnly / ColOnly / RowCol) and produced
no clean speedup on any case. Several cases regressed or stalled.

See the "Stage 1 LINSCALE measurement" subsection below for the full table
and mechanism. The rest of this finding's original text (preserved below)
describes the structural observation accurately; the *implied remedy* was
wrong.

---

This was the **single most surprising finding of the audit** and was our
leading candidate for the linear-solve pain.

`newton.rs` line 2656–2682, the only production call site into the linear
backend:

```rust
let rhs = -&assembly.residual;
// ... bypass-dispatch block ...
let mut linear_report =
    solve_linearized_system(&assembly.jacobian, &rhs, &linear_options, block_layout);
```

The `assembly.residual` and `assembly.jacobian` are the raw physical values.
`EquationScaling` (rows) and `VariableScaling` (columns) exist and are
computed, but their exclusive uses are:
- `scaled_residual_inf_norm`, `residual_family_diagnostics`,
  `global_material_balance_diagnostics` — norm-only, never modifies the
  residual or Jacobian.
- `scaled_update_peak`, `scaled_update_inf_norm`,
  `scaled_applied_update_peak` — applied to the *solution vector* returned
  by the linear solver, never applied to the Jacobian columns before solve.

Consequence: the linear system the solver sees is **row-inhomogeneous** and
**column-inhomogeneous**:

- Rows: `water`, `oil_component`, `gas_component` scale factors come from
  `pv_over_dt / bw`, `pv_over_dt / bo`, `pv_over_dt / bg`. For a typical
  medium-water step-1 cell with `pv = 50 m³`, `dt = 0.25 day`,
  `bo ≈ 1.2`, `bg ≈ 0.005`, `bw ≈ 1.0`: the scales are
  ~200, ~167, ~40,000 respectively. **That's ~200× between water/oil and
  gas rows.**
- Columns: `pressure_bar` ≈ 250, `sw` = 1.0, `hydrocarbon_var`
  ≈ Rs ≈ 100 (saturated) or ≈ 1.0 (undersaturated). **~250× between pressure
  and sw columns.**
- Well / perforation rows: scale = 1.0 (FB-residual) or |BHP| (~350),
  `perforation_flow` scale = |rate| (highly variable, can be near 0 for
  marginal perforations).

Expected effect on the CPR preconditioner:

- **Pressure-transfer restriction** at `build_pressure_transfer_weights`
  (line 625). It builds the restriction by solving the local 2×2
  `transport_block` = `block[(row+1, col+1)]` (= the (sw, hc) × (sw, hc)
  subblock) and eliminating (sw, hc) from the water equation. The
  eliminated entries are `block[(0, inner+1)]` (water residual's derivative
  w.r.t. sw and hc). With unscaled rows and columns, **the magnitudes of
  these couplings are off by up to four orders of magnitude from what the
  CPR textbook assumes**. In practice this means:
  - The coarse-grid pressure operator `pressure_rows` has entries dominated
    by gas-row derivatives (which are ~200× larger than water-row
    derivatives).
  - The "prolongation" weights can be arbitrary sign because of this
    magnitude mismatch.
  - The ILU(0) built on top of the unscaled coarse system has large
    pivot-to-fill ratios, leading to inaccurate pressure corrections.
- **Fine-smoother ILU(0)** at `factorize_full_ilu0` (line 516): runs
  directly on the unscaled 3604-row Jacobian. ILU(0) stability is highly
  sensitive to diagonal dominance, which is destroyed by unequal row/col
  scales. In particular, well-constraint rows (scale 1) sitting in a matrix
  where neighboring cell rows have magnitude 10,000 will produce L/U factors
  with pivots jumping by 10,000× between adjacent rows — a well-known
  recipe for slow GMRES convergence and restart stagnation.
- **Stopping criteria**: the outer tolerance is
  `absolute_tolerance + relative_tolerance * rhs_norm` (line 1175). Because
  `rhs_norm = ||residual||_2` mixes units across rows,
  dropping `||residual||_2` by 10⁻⁷ can mean the water/oil rows have dropped
  far below tolerance while a single gas row or perforation row is still
  orders of magnitude out of balance. **That is the exact symptom medium-water
  traces show**: `outer_res/tol ≈ 30,000` at `max-iters`, residual-family
  diagnostic pointing at the gas row at a specific cell, fGMRES unable to
  reduce it further with the available Krylov dimension.

This is a textbook ill-scaled-linear-system symptom. No industry CPR
implementation leaves row/column scaling off. OPM Flow applies both; ECLIPSE's
"CPR+IMPES" uses the IMPES reduction which has implicit row-combining to
normalize the pressure equation; Stone CPR always operates on the
pressure-reduced (and therefore implicitly scaled) system.

#### Stage 0.5 SCALESPAN measurement (2026-04-19)

Before committing to a 16-run Stage 1 probe, a lightweight trace of
actual `EquationScaling` / `VariableScaling` magnitudes was added to the
Newton loop. On **medium-water 20x20x3 dt=0.25 step-1**:

| family         | min    | max    | ratio (max/min) |
|----------------|-------:|-------:|----------------:|
| eq.water       | 8.0e1  | 8.0e1  | 1.0×            |
| eq.oil         | 8.0e1  | 8.0e1  | 1.0×            |
| eq.gas         | 8.0e1  | 8.0e1  | 1.0×            |
| eq.well        | 1.0e2  | 5.0e2  | 5.0×            |
| eq.perf        | 7.2e3  | 7.2e3  | 1.0×            |
| var.p (iter 0) | 3.0e2  | 3.0e2  | 1.0×            |
| var.p (later)  | 1.8e2  | 4.3e2  | 2.4×            |
| var.sw         | 1.0e0  | 1.0e0  | 1.0×            |
| var.hc         | 1.0e0  | 1.0e0  | 1.0×            |
| var.bhp        | 1.0e2  | 5.0e2  | 5.0×            |
| var.rate       | 7.2e3  | 7.2e3  | 1.0×            |
| global row     | 8.0e1  | 8.0e1  | 1.0×            |

**This refuted the "row scales span ~200×" premise stated above.** On
medium-water the PVT formation-volume-factor ratios (`bw`, `bo`, `bg`)
are near unity with negligible free gas, so row scales are essentially
uniform within the cell-mass family. The ~40,000× gas-row magnitude
prediction was based on PVT values that don't apply here.

The actual magnitude spread is dominated by **columns**: perf_rate
(~7.2e3) vs sw/hc (~1.0) is a ~6,000× spread, and that is the axis most
likely to affect ILU(0) / coarse-solver behavior. This became the
directional prior for the Stage 1 probe: if any mode would win, column
scaling (or both) should be the winner.

#### Stage 1 LINSCALE probe (2026-04-19) — full 4-mode × 4-case sweep

Probe design: `LinscaleMode ∈ {Off, RowOnly, ColOnly, RowCol}` settable
via `--linscale-mode <off|row|col|both>`, defaults Off (bit-exact to
baseline). Wrapper `solve_linearized_system_with_linscale` builds D_r / D_c
from `EquationScaling` / `VariableScaling`, computes `J_scaled = D_r * J * D_c`
and `b_scaled = D_r * b`, solves, recovers `x = D_c * y`.

Headline numbers (lin_ms and substeps; `→ stall` flags trajectory
degeneracy, not speedup):

| case                                | Off (base) | RowOnly              | ColOnly            | RowCol              |
|-------------------------------------|-----------:|---------------------:|-------------------:|--------------------:|
| Case 1: medium-water 20x20x3 step 1 | 17,561 / 12 | 17,265 / 16 (−2%)   | 16,576 / 16 (−6%)  | 20,743 / 16 (+18%)  |
| Case 2: medium-water 20x20x3 6-step | 20,160 / 18 | **763 / 2 → stall** | 23,604 / 19 (+17%) | **1,026 / 2 → stall** |
| Case 3: heavy-water 12x12x3 dt=1    |  1,665 / 16 |  2,920 / 16 (+75%)  |  2,662 / 22 (+60%) |  8,921 / 96 (+436%, near-catastrophic) |
| Case 4: gas-rate 10x10x3 6-step     |    171 / 4  |    170 / 4 (neutral)|    906 / 4 (+430%) |    169 / 4 (neutral)|

Stall attribution: Case 2 Row and Both report `substeps=2`,
`accepts=1+4+2495`, `dt=[1.000e-4,1.000e-4]`, `growth=hotspot-repeat`,
`oil=3472.28` (vs Off `3610.27`). Simulator chopped to minimum dt and burned
~2495 micro-accept attempts at `dt=1e-4` without advancing the step. Apparent
−96% lin_ms is a degenerate stall at `dt_min`, not a real speedup.

Case 3 RowCol is clearly regressive: 96 substeps × `avg_p=359.89` vs Off's
16 substeps × `avg_p=338.37` — the scaling combination destabilized Newton
convergence and forced the controller into many small substeps.

**Why the signal disagrees with the SCALESPAN-informed prior:** the ColOnly
prediction was −50%-to-−80% on medium-water. Actual ColOnly result: **+17%
regression on Case 2**, −6% on Case 1 (within noise), +430% on Case 4. The
likely mechanisms:

1. **ILU(0) is not scale-invariant.** Rescaling `J → D_r J D_c` changes
   which entries are above the numerical drop threshold and changes fill
   ordering inside `factorize_full_ilu0`. A "better conditioned" matrix on
   paper can produce a strictly *worse* preconditioner if the particular
   sparsity pattern relies on relative-magnitude ordering. The current
   assembly produces matrices where ILU(0) was tuned (or at least,
   empirically converges) on the unscaled form.
2. **Newton convergence trajectory shifts.** Even when the scaled linear
   solve returns a numerically similar update, small perturbations in
   update direction interact with Appleyard damping and per-cell Newton
   caps in non-monotone ways. The scaling's effect on the trajectory can
   exceed its effect on per-iter lin_ms.
3. **The BiCGSTAB coarse solver** (Finding 4) inherits scaling from the
   full system. On heavy-water (`n_cells=432` is below the 512 threshold
   in native, but the WASM run uses the 512 dense-LU path, so the coarse
   solver chain is different between native and WASM — and the Stage 1
   sweep was WASM-only).
4. **Well-row and perforation-row magnitudes (`1e2`–`7e3`) are not the
   dominant numerical issue.** The SCALESPAN trace shows these rows are
   diagonally-dominant in their own right because the BHP/rate couplings
   are sparse in the coarse structure.

**Conclusion.** Fix 1 as designed (diagonal `D_r * J * D_c` pre-scaling)
does NOT work on this codebase's linear backend. The "no industry CPR
leaves scaling off" heuristic is true as a general statement, but is
load-bearing only in combination with a *different* coarse operator
(true-IMPES / summed-IMPES reduction) and a scale-aware ILU variant.
Adding just the pre-scaling on top of the current water-equation-as-
pressure restriction + block-Jacobi + ILU(0) chain doesn't help, and can
destabilize the Newton trajectory in Cases 2 and 3.

**Stage 1 probe reverted (2026-04-19)** via `git checkout` on
`fim/newton.rs`, `fim/linear/mod.rs`, `frontend.rs`, `lib.rs`, and
`scripts/fim-wasm-diagnostic.mjs`. WASM rebuilt from clean sources. No
source-tree changes remain.

**What remains valid from the original Finding 2:** (1) the structural
observation that `EquationScaling` and `VariableScaling` are computed and
discarded for norm-only use; (2) the specific `pv_over_dt / b*` formula at
`scaling.rs:45-47` producing uniform rows on low-gas cases; (3) that the
comment "CPR is still incomplete" at `linear/mod.rs:215` is not stale.

**What is now refuted:** the claim that *applying* diagonal row/col
scaling before the solve is the "single highest-impact next lever". It is
not, on this codebase.

**Raw sweep artefacts:** `/tmp/linscale/case{1-4}-{off,row,col,both}.log`
(not committed; reproducible from the `--linscale-mode` flag after
re-adding the probe). Aggregation script inline in the worklog.

### Finding 3 (confirmed): the "pressure equation" used by CPR is the raw water mass balance — not a combined or pressure-reduced equation

`build_pressure_transfer_weights` line 633: `restriction[0] = 1.0`. The
restriction matrix takes **row 0 of each cell block (the water residual)**
and subtracts a linear combination of rows 1 and 2 (oil, gas) to eliminate
(sw, hc) locally. This is a valid True-IMPES / Quasi-IMPES construction
**only when the water equation is a reasonable proxy for pressure at every
cell**. In medium-water runs the water equation is a decent proxy in the
aquifer region but NOT in the producer column where water cut is still low
and oil/gas dominate the accumulation term.

Industry-standard CPR uses one of:
- **Summed IMPES**: restriction = `[1, 1, 1]` (sum of mass balances),
  weighted by inverse formation volume factors — gives a proper pressure
  equation.
- **True-IMPES with adaptive row choice**: choose the dominant-accumulation
  row per cell.
- **Dynamic Row Sum**: scale each row by its formation volume factor before
  summing, so the combined equation is in "reservoir volume" units.

Using row 0 unconditionally is the simplest variant and is known to work
poorly in problems with large water-cut variation across the grid.

### Finding 4 (confirmed): pressure coarse solver falls to BiCGSTAB+ILU(0) at `n_cells > 512`

`invert_pressure_block` line 611 returns `None` if `n > 512`. For
`20x20x3 = 1200` + 2 well unknowns, the coarse system is ~1202 rows and the
coarse solver drops to `solve_pressure_with_bicgstab` with ILU(0)
preconditioning and `rel_tol = 1e-6`, `max_iters = 50` (line 14–15).

Industry practice for the pressure stage of CPR is **AMG** (algebraic
multigrid). BiCGSTAB+ILU(0) on a ~1200-row 3D Laplacian-like operator with
realistic permeability contrasts has the asymptotic convergence rate of any
Krylov method on a second-order elliptic operator: `~sqrt(condition_number)`
iterations. A well-conditioned reservoir pressure problem needs 15–30
iterations; an ill-conditioned one (which ours is, per Finding 2) needs
hundreds — capped at 50 here.

That 50-iteration cap is likely silent. The coarse solve returns whatever
residual it reached and the outer fGMRES continues with a degraded pressure
correction. The CPR reduction ratio (line 128–130 in
`PressureCorrectionAccumulator`) *should* expose this, but is surfaced only
as a diagnostic number (mean and last) — not used to gate the outer loop.

### Finding 5 (confirmed): `cell_block_size = 3` layout is correct, assembly matches

`newton.rs` line 2255–2260:
```rust
let block_layout = Some(FimLinearBlockLayout {
    cell_block_count: state.cells.len(),
    cell_block_size: 3,
    well_bhp_count: state.n_well_unknowns(),
    perforation_tail_start: state.n_cell_unknowns() + state.n_well_unknowns(),
});
```

Matches `assembly.rs` line 85–91 (`unknown_offset` / `equation_offset` both
`cell_idx * 3 + local`). No off-by-one or coupling-misattribution bug.

### Finding 6 (confirmed): no Line Search, no Trust Region — only Appleyard damping

Full-repo search for `line_search` / `trust_region` against the FIM newton
path yielded nothing. The Newton outer loop uses Appleyard saturation damping
(the standard reservoir-engineering damping), plus custom acceptance
heuristics (`should_accept_near_converged_iterative_step`, Newton-trial
tolerance relaxation). This is in line with reservoir-simulator practice
(ECLIPSE doesn't do line search either) but leaves the outer Newton reliant
on the linear solve being accurate. Given Finding 2, the linear solve is
*not* accurate on hotspot iters, which makes Appleyard damping take the
brunt of preventing divergence — and it can't. That's the `outer_res` stall
in medium-water.

### Finding 7 (not investigated): no Jacobian boundary-cell asymmetry spotted

Wide-angle outcome C was "Jacobian bug at boundaries". Not examined in this
audit beyond verifying the layout. `assembly.rs` lines 1–200 show the standard
structure (`add_exact_accumulation_jacobian`, `add_exact_flux_jacobian`,
well/perforation blocks) — no obvious smoking gun, but not ruled out. If
Finding 2 + 3 + 4 fixes turn out to be insufficient, boundary-cell assembly
around `cell0` should be the next place to look.

## Ranked fix candidates

Ordered by expected impact-per-risk. Each is a **Stage 0 proposal** — nothing
has been measured yet.

### Fix 1 (REFUTED BY STAGE 1, 2026-04-19 — DO NOT PROMOTE): apply row and column scaling to the Jacobian before the linear solve

**Stage 1 outcome:** tested via the LINSCALE probe on the 4-case shortlist
in all four modes. No clean win; Case 2 (medium-water 6-step) Row/Both
modes stalled at `dt_min`, Case 3 (heavy-water) regressed +60% to +436%,
Case 4 (gas-rate) ColOnly regressed +430%. Full measurement table in the
Finding 2 "Stage 1 LINSCALE probe" subsection above.

**Leave this Fix in the doc for historical / provenance reasons; do not
re-attempt without a different preconditioner chain** (summed-IMPES
reduction + scale-aware ILU). Simple `D_r J D_c` on the existing chain
does not help.

---

*Original plan preserved below for reference:*

This was the direct fix for Finding 2 and was expected to be a pure win
unless it changed the physics path (which, done correctly, it should not).

Plan:
1. After assembly, compute a diagonal row-scaling matrix `D_r` from
   `EquationScaling` (inverse of the scale per row) and column-scaling
   matrix `D_c` from `VariableScaling`.
2. Scale:
   `J_scaled = D_r * J * D_c`,
   `rhs_scaled = D_r * rhs`.
3. Solve `J_scaled * x_scaled = rhs_scaled`.
4. Recover the physical update: `x = D_c * x_scaled`.
5. Hand `x` to the Appleyard damping / state update as before.

Expected effect:
- All rows have unit scale, all columns have unit scale. ILU(0) pivots are
  order-1. GMRES residual norm is a meaningful convergence metric. CPR
  pressure-transfer weights become the textbook coupling magnitudes.
- The linear tolerance `rhs_norm * 1e-7` now corresponds to "each scaled row
  dropped by 1e-7", which is close to the Newton residual-tolerance convention.

Risks:
- Slightly more memory (2n floats for diagonal scales) and a O(nnz) multiply
  on assembly — negligible compared to LU.
- Well-constraint rows with scale = 1 should probably be left unscaled (the
  Fischer-Burmeister residual is already dimensionless and O(1)). Need to
  verify the scale logic doesn't introduce zeros.
- `perforation_flow` scale depends on `|rate|` which can be ~0 for marginal
  wells — need a floor (already present: `.max(1.0)`).

Validation:
- Stage 1 probe: add an optional `scale_before_solve: bool` flag gated to a
  diagnostic-only path. Run the 4-case shortlist. Measure
  `lin_ms`, `linear_iterations`, `max-iters-failures`, `outer_res` trajectory.
  Expected: 50–80% lin_ms reduction on medium-water. If actual reduction is
  only 10–20%, Finding 2 was wrong or Fix 3 is needed alongside.

### Fix 2 (HIGH): use summed-IMPES (or Dynamic Row Sum) for the CPR pressure equation

Addresses Finding 3. In `build_pressure_transfer_weights`, replace
`restriction[0] = 1.0; restriction[1] = ...; restriction[2] = ...;` with a
row-combining scheme:
- Simplest: `restriction = [1/bw, 1/bo, 1/bg]` (Dynamic Row Sum) at the time
  of Jacobian restriction.
- Alternative: adaptive row choice — pick the row with the largest
  `|diag|` and eliminate the other two.

This becomes much cleaner once Fix 1 is in (all rows are unit-scale so the
row-combining is just a sum).

Risk: small — this is pure preconditioner change, doesn't affect the solved
system. Could regress specific cases where water-only restriction happens to
work (unlikely but possible).

### Fix 3 (MEDIUM): gate the CPR pressure-coarse solver on achieved reduction

Addresses Finding 4. Currently `PressureCorrectionAccumulator` records the
coarse-solve reduction ratio but the outer fGMRES has no awareness of it.
Easy diagnostic win + possible convergence win:

1. After each coarse solve, if `reduction_ratio > 0.5` (i.e., coarse solve
   achieved less than a 2× reduction), skip the pressure-correction stage for
   the next few iters and use fine-smoother-only.
2. If `reduction_ratio > 0.9` repeatedly, trip an early exit to sparse-lu.

This is essentially a "make the solver aware when CPR is failing" knob.
Doesn't fix the underlying issue but short-circuits the wasted time more
cleanly than the current restart-stagnation / dead-state heuristics.

### Fix 4 (MEDIUM): promote the coarse solver from BiCGSTAB+ILU(0) to GMRES+ILU(k) or AMG

Addresses Finding 4 properly. AMG is a big project (would need a new crate or
a from-scratch implementation). ILU(k) for k=1 or k=2 is a 1–2 day job using
the existing factorization scaffolding — already structured to emit
(L,U,diag) tuples.

Risk: ILU(k) uses more memory and more flops per application; need to prove
the per-iter cost reduction justifies it.

### Fix 5 (LOW-MEDIUM): add a backtracking line search on the Newton update

Addresses Finding 6. Wide-angle analysis flagged this as a Tier 2 idea.
Orthogonal to the linear-solver fixes above and probably a 3-day project
(need a residual-reduction predicate that respects material-balance
constraints).

Risk: higher than Fixes 1–3 because it changes the Newton trajectory
directly. Only worth doing after the linear solve is no longer the
bottleneck.

### Fix 6 (DIAGNOSTIC): audit Jacobian assembly at boundary cell 0

Addresses Finding 7. Not a "fix" per se — a read-only check to verify no
boundary/perforation asymmetry in the Jacobian assembly when cell 0 hosts the
injector. If Fixes 1–3 close the convergence gap, Fix 6 can be dropped.

## Recommended ordering (UPDATED 2026-04-19 post-Stage-1)

**Fix 1 is refuted.** The recommended ordering changes as follows:

1. ~~**Fix 1 first**~~ **SKIPPED.** Stage 1 probe showed no mode wins
   cleanly; Case 2 stalled, Case 3 regressed. Simple `D_r J D_c` is not
   the lever.
2. **Fix 2 (summed-IMPES pressure restriction) as the new top candidate.**
   Changes the coarse-operator shape rather than rescaling the full system.
   Stage 1 probe: measure CPR `average_reduction_ratio` before/after. If
   `reduction_ratio` improves but fgmres-cpr still fails, Fix 2 alone is
   insufficient and Fix 4 (stronger coarse solver) is needed.
3. **Fix 3 (gate CPR on achieved reduction) as a parallel diagnostic win.**
   It surfaces CPR health cheaply and enables a cleaner post-fail
   short-circuit than the current restart-stagnation / dead-state heuristics.
   Orthogonal to Fix 2; can be stacked.
4. **Fix 4 (stronger coarse solver) is a bigger project** and should only
   be tackled if Fixes 2+3 leave meaningful lin_ms on the table.
5. **Plan B (Lever 1 widen near-converged-accept + Lever 3 post-fail
   short-circuit) from `project_fim_bypass_audit_2026-04-19`** is now
   rehabilitated as a near-term option. The bypass-audit showed
   post-fail-fallback is 68-75% of medium-water lin_ms; with Fix 1
   off the table, Plan B's cost-cap (~85% of medium-water lin_ms
   addressable) looks materially better than Fix 2's projected upside.

### Original ordering (preserved for provenance)

1. ~~**Fix 1 first**~~. ~~(a) the direct fix for the highest-confidence
   finding, (b) prerequisite to making Fixes 2/3/4 cleanly interpretable,
   and (c) likely to shift the 4-case shortlist enough that the remaining
   categories re-shuffle materially. Stage 1 probe only, no surgery until
   measurement confirms.~~
2. **Fix 2 if Fix 1 alone doesn't close the gap**. Small, low-risk
   follow-on.
3. **Fix 3 as a diagnostic win regardless** — it surfaces CPR health
   metrics that help diagnose whether any further surgery is warranted.
4. **Fix 4/5/6 only if Fixes 1–3 fail to reach the heavy-water / gas-rate
   shortlist targets.**

## What this audit did NOT do

- Did not modify any code.
- Did not re-measure lin_ms or any per-step timings — all numbers cited
  (88% sparse-lu, 68-75% post-fail-fallback, 3604 rows, etc.) are from the
  2026-04-18 cost profile and 2026-04-19 bypass audit, not re-verified here.
- Did not audit `assembly.rs` beyond the first 200 lines. The flux Jacobian,
  accumulation Jacobian, well constraint Jacobian, and perforation Jacobian
  were not read end-to-end. Finding 7 is therefore "not ruled out" rather
  than "ruled out".
- Did not verify the CPR implementation against a reference simulator's
  output (e.g., OPM Flow's CPR on the same problem). That would be the
  gold-standard validation and is reserved for Stage 2 of Fix 1.

## Sources used

- `src/lib/ressim/src/fim/linear/mod.rs` — full.
- `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs` — full (2203 lines).
- `src/lib/ressim/src/fim/scaling.rs` — full.
- `src/lib/ressim/src/fim/newton.rs` lines 1295–1520, 1640–1780, 2240–2750.
- `src/lib/ressim/src/fim/assembly.rs` lines 1–200.
- `docs/FIM_WIDE_ANGLE_ANALYSIS.md` (this session).
- `docs/FIM_BYPASS_AUDIT.md` (2026-04-19).
- `docs/FIM_CONVERGENCE_WORKLOG.md` (running).
- Memory: `project_fim_cost_profile_2026-04-18`,
  `project_fim_bypass_audit_2026-04-19`,
  `project_fim_jacobian_reuse_attempt`.
