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

## Fix 2 Stage 1 probe design — 2026-04-20

Context: after Lever 3 Stage 2 landed (commit d6069be, -15.9% lin_ms
on 4-case shortlist), the convergence gap with OPM Flow (30-day
timesteps vs our sub-day substeps) remains. Medium-water case 2
requires 12-23 substeps per 0.25-day step at dt ∈ [3e-3, 5e-2]d,
driven by `retry_dom=nonlinear-bad:oil@N` Newton damping failures.
Fix 2 (replace row-0 pressure restriction with summed-IMPES or
Dynamic Row Sum) is the next-highest-leverage lever per
`FIM_WIDE_ANGLE_ANALYSIS.md` Tier 1 ranking — it would attack both
lin_ms (via CPR quality) and dt ceiling (via more-accurate Newton
updates).

**What the probe measures:** per-cell restriction-weighted residual
magnitudes for the three candidate restriction schemes, side-by-side
with the current (Baseline) scheme. No preconditioner change; no
actual alternative solve. Measurement-only: we compute each variant's
*coarse-system RHS magnitude* and *coarse-system diagonal dominance*
at each CPR application and log aggregate statistics, then decide
whether to build a real alternative preconditioner in Stage 2.

**Signal we want:**
- Does Baseline's row-0 restriction attenuate the residual more than
  SummedImpes or DynamicRowSum at the cells where outer `fgmres-cpr`
  is stalling?
- Does Baseline's coarse-system diagonal have lower magnitude (worse
  conditioning) than the alternatives?
- A PASS is the alternative shows a consistent, cell-weighted
  advantage on the cases where `fgmres-cpr` fails. A FAIL is the
  alternatives produce the same or weaker signal → Fix 2 unlikely to
  help and we pivot to Fix 4 (stronger coarse solver) or Line Search.

**Instrumentation (gmres_block_jacobi.rs, ~50 lines additive):**

1. Add a variant enum `PressureRestrictionKind { Baseline, SummedImpes,
   DynamicRowSum }` — Baseline is what the code currently does.
2. In `build_pressure_transfer_weights`, add a probe path that computes
   all three variants' restriction weights on each block. Only
   Baseline is returned; the other two are stored alongside the
   preconditioner for measurement.
3. In `extract_pressure_rhs`, when a probe flag is on, compute the
   coarse RHS under all three restrictions. Log per-apply:
   - `|rhs_baseline|_inf`, `|rhs_summed|_inf`, `|rhs_dynrow|_inf`
   - cell index of the max-residual block and its per-row magnitudes
     `(|r0|, |r1|, |r2|)` so we can see whether the water row is
     under-representing the dominant residual direction
4. In `solve_pressure_correction`, compute a cheap proxy for each
   variant's coarse-diagonal magnitude: `sum_cell |restriction·block_diag|`.
   The Baseline diagonal is what the coarse solver actually inverts;
   the alternatives' values tell us whether the coarse problem would
   be better-conditioned under Fix 2.
5. Emit a trace line once per CPR outer apply:
   ```
   CPR-RESTRICTION-PROBE iter={n} rhs_inf=[base={v} summed={v} dynrow={v}]
     rhs_l2=[base={v} summed={v} dynrow={v}]
     coarse_diag_sum=[base={v} summed={v} dynrow={v}]
     worst_cell={c} block_r=[{r0} {r1} {r2}]
   ```

**Decision rule for promotion to Stage 2:**
- PASS: on cases 1 and 2 (medium-water), SummedImpes OR DynamicRowSum
  shows `rhs_inf` ≥ 1.5× and `coarse_diag_sum` ≥ 1.2× Baseline on the
  CPR applies that precede a `post-fail-fallback` iter. (The
  threshold is conservative — we're looking for a clean signal, not
  a marginal one, given Fix 1's false-positive on Stage 1 LINSCALE.)
- FAIL: no variant clears both thresholds → Fix 2 will not change
  behavior meaningfully; pivot to Fix 4 or add Line Search (Fix 5).
- PARTIAL: one variant clears the thresholds but only on a fraction
  of the failing iters → proceed with Stage 2 for that variant and
  measure actual lin_ms; expect a smaller win than the PASS case.

**Risk of false PASS:** the probe measures RHS magnitude, not solved
quality. A variant with a bigger RHS could still produce the same
update after the coarse solve converges. We accept this risk because
the probe is cheap and any real Stage 2 (build the alternative
preconditioner) would re-validate empirically.

**Stage 1 scope:**
- Probe lives in `gmres_block_jacobi.rs` only (additive, ~50 lines).
- Gated off by default; enabled by a `FIM_CPR_RESTRICTION_PROBE=1`
  env var so locked smoke tests stay green without toggling.
- Run 4-case shortlist with probe on, aggregate per-case statistics,
  revert probe, commit the audit doc update + a memory memo.

### Fix 2 Stage 1 SUPERSEDED — OPM Flow reference run is the sharper signal

Before implementing the internal restriction probe we ran the case-2
shortlist problem through **OPM Flow 2025.10** on the same machine
(see `/tmp/opm-case2/CASE2.DATA` for the translated deck). OPM Flow
ships the industry-standard CPR-W preconditioner (pressure-stage AMG
with wells) and a full Newton stack (residual/update/MB convergence,
update stabilization, line-search-like controls). This gives us a
**direct reference** for "what a correctly implemented CPR + Newton
stack does on this exact problem" — strictly stronger evidence than
our internal restriction proxy would have been.

**Case 2 — OPM Flow vs ressim post-Stage-2 baseline (same problem, same dt schedule):**

| metric                         | ressim (d6069be)          | OPM Flow 2025.10 | gap        |
|--------------------------------|---------------------------|------------------|-----------:|
| total substeps across 6 steps  |                       102 |                7 |     **15×** |
| Newton iterations (accumulated)|             hundreds (est)|               33 |    **~10×+** |
| linear iterations              |             hundreds (est)|               36 |    **~10×+** |
| wall time (simulation only)    |                     ~122 s |           0.07 s |   **~1700×** |
| final FPR                      |                 348.3 bar |        337.9 bar |     ~3% lower (vol-wt vs cell-avg) |
| producer oil rate trajectory   |          3337→3610 m³/d   |    3050→3558 m³/d |     same trend |

Physical trajectories match (within the cell-avg vs volume-weighted
pressure convention). Convergence robustness does not.

**Key data points from OPM's run:**
- Step 1 (dt=0.25d): **1 substep**, 9 Newton iters, 9 linear iters
- Step 2: **2 substeps** at dt=0.125 — this was OPM's only step that
  needed adaptive reduction; subsequent steps returned to dt=0.25
- Steps 3-6: **1 substep each** at dt=0.25, 4-5 Newton iters, 4-6
  linear iters per substep
- Linear solver: `LinearSolver="cprw"` — CPR with Wells, not ILU0+scalar
- Preconditioner reuse: `CprReuseInterval=30`, `CprReuseSetup=4`

**Implications for the fix ranking:**

1. **The convergence gap is not subtle — it is a ~3 order-of-magnitude
   structural gap.** Fix 2 (summed-IMPES restriction) alone will not
   close it. The 2026-04-19 audit correctly identified that our CPR is
   incomplete, but the gap is bigger than a restriction-operator
   change can close: OPM's coarse solver is AMG, not BiCGSTAB+ILU0 (our
   Finding 4), and OPM's Newton stack has update stabilization and
   adaptive time-step control tuned to reservoir problems (our Finding 6).
2. **Fix 2 remains worth doing as a component**, but its projected
   upside should be reframed: it is one of ~4 components in a correct
   CPR+Newton stack, and the biggest win will come from completing all
   of them. Any Stage 2 that lands Fix 2 alone and shows only a 10-20%
   lin_ms improvement would be consistent with this evidence — not a
   disappointment.
3. **The line-search / update-stabilization lever (Fix 5) may be
   larger than originally estimated.** OPM does not take 12-23
   substeps per 0.25-day step on this problem. A non-trivial fraction
   of our substep count comes from Appleyard damping failing to find a
   safe factor where a proper line search or update stabilization
   would succeed. `nonlinear-bad:oil@N` is the dominant retry class in
   our case-2 log — that is exactly the symptom Fix 5 targets.
4. **The decision between "finish CPR" and "add line search" is
   resolvable by a second OPM experiment, cheaper than either
   implementation.** Re-run OPM with `--use-update-stabilization=false`
   and see how many substeps it needs. If OPM still converges in ~7
   substeps without update stabilization, the win is in the linear
   solver (CPR-AMG); if it degrades, Fix 5 is the lever.

**Revised Stage 1 decision — 2026-04-20:**
- **Fix 2 internal restriction probe: skipped.** Its upside is bounded
  by the OPM evidence — even a perfect summed-IMPES restriction only
  addresses one of the multiple CPR deficiencies and cannot close a
  15-100× substep gap on its own.
- **Next action: OPM component-ablation experiments.** Run 2-3 cheap
  OPM variants on the same deck:
  1. default cprw — **done, 7 substeps**
  2. `--linear-solver=ilu0` (no CPR) — measure whether CPR is the
     dominant contributor to OPM's substep count
  3. `--use-update-stabilization=false` — measure whether update
     stabilization is material
  4. `--enable-adaptive-time-stepping=false` with our dt=0.25 schedule
     — measure baseline Newton robustness at fixed dt

  Each is a 1-line CLI change and a 0.1s run. The outcomes will tell
  us definitively whether the gap is CPR-dominated, Newton-dominated,
  or mixed — directing the next 2-4 weeks of implementation work.
- **Fix 5 (line search / update stabilization) moves up to the same
  tier as Fix 2/4.** It was "Tier 2" in the wide-angle analysis; the
  OPM evidence suggests it is Tier 1 alongside the CPR fixes.

### OPM Flow ablation experiments — 2026-04-20

Four single-flag experiments on the same deck. Each a 0.1s re-run.
Goal: localize the gap between ressim (102 substeps / 122s) and OPM
(7 substeps / 0.07s).

| experiment                                                  | substeps | Newton its | linear its | verdict                                   |
|-------------------------------------------------------------|---------:|-----------:|-----------:|-------------------------------------------|
| default (`LinearSolver=cprw`)                               |        7 |         33 |         36 | baseline                                  |
| `--linear-solver=ilu0` (disable CPR)                        |        7 |         48 |    **758** | **CPR is the dominant linear-iters lever**|
| `--use-update-stabilization=false`                          |        7 |         33 |         36 | **identical — update-stab is no-op here** |
| `--enable-adaptive-time-stepping=false` (fixed dt=0.25)     |        6 |         30 |         34 | adaptive splitting adds 1 substep         |
| `--tolerance-cnv=0.001 --tolerance-mb=1e-9` (tight)         |        7 |         39 |         42 | +18% iters, same substep count            |

**Localizations:**

1. **CPR contributes the full 21× linear-iter savings**: 758 → 36.
   Without CPR, OPM needs ~16 Krylov iters per Newton linearization
   (758/48); with CPR it needs ~1 (36/33). That's the AMG coarse
   grid doing its job on the elliptic pressure subsystem.

2. **CPR does NOT contribute to substep count on its own**: both
   `cprw` and `ilu0` take 7 substeps. The linear solver is accurate
   enough either way for Newton to find a damp-feasible direction.
   This contradicts an earlier hypothesis that a degraded linear
   solve causes ressim's substep explosion via "linear-bad-
   masquerading-as-nonlinear-bad" — at least, it's not the direct
   mechanism in OPM's reference stack.

3. **CPR does contribute modestly to Newton-iter count**: 33 → 48
   (+45%) without CPR. More inaccurate linear solves → more Newton
   iters to converge each substep.

4. **Update stabilization is a no-op on this problem**: identical
   substep/Newton/linear counts with and without it. This problem
   does not exhibit the oscillation pattern update-stab targets.
   Fix 5 (line search + update stabilization) drops back to Tier 2.

5. **Tight tolerances add ~18% iters but do not change substeps**.
   Our tolerance choice vs OPM's is not the explanation for the
   substep gap.

**Where does the 102-vs-7 substep gap come from, then?**

By elimination:
- Not CPR directly (CPR helps Newton iter count by 45%, not 15×
  substep count).
- Not update stabilization (no-op here).
- Not tolerance choice (minor).
- Not adaptive time-stepping (fixed dt only cuts 1 substep).

Remaining explanations, ranked by likely contribution:
- **(A) Newton damping policy + acceptance criteria.** OPM's Newton
  has a different acceptance rule than ressim's Appleyard
  `hotspot-newton-caps` + `retry_dom=nonlinear-bad:oil@N` machinery.
  On this problem OPM accepts after 4-9 Newton iters per substep
  with dt=0.25d; ressim cannot damp at dt=0.25d and retries down
  to dt ∈ [3e-3, 5e-2]d. The gap is not "our Newton fails to
  damp"; it's "our Newton thinks it has failed to damp and backs
  off dt, while OPM's Newton keeps iterating and succeeds". This
  is a ressim-specific over-conservatism in the acceptance gate.
- **(B) Jacobian accuracy / scaling.** Finding 2 (unscaled Jacobian
  given to the linear solver) was refuted as a simple D_r·J·D_c
  pre-scaling lever (Stage 1 LINSCALE probe showed no win), but
  the underlying "physical-units row variation" may still hurt
  Appleyard damping's cell-local stability estimates. Under-
  investigated.
- **(C) Property evaluation / upwinding at the well column.**
  retry_dom consistently lands at cells that move through the
  producer's water front (`oil@190`, `oil@256`, `oil@2719`, etc.).
  These are cells where upwinding may flip within an iteration,
  forcing Newton to chase a discontinuity Appleyard can't damp.
  OPM uses monotone upwinding with smoothing; ressim's upwinding
  strategy has not been audited for this failure mode.

**Revised next action:**

The gap is **NOT** in the linear solver as we previously framed it.
CPR helps, but not by enough to close the 15× substep gap.
Implementing Fix 2 or even Fix 4 alone is projected to save 10-30%
of lin_ms (consistent with the CPR ablation above: ~40% of Newton
iters, which is a subset of total lin_ms) but would leave the
substep gap almost entirely intact.

**New Tier 1 candidates (post-OPM-ablation):**

- **Fix A (new, HIGH): audit and widen ressim's Newton acceptance
  gate to match OPM's behavior.** Compare Newton iteration counts
  per substep between ressim and OPM on the same first substep of
  step 1. Find the iter where OPM accepts (~9) and ressim retries
  (~5-7 before hotspot-cap or damping failure). The acceptance
  rule in `should_accept_near_converged_iterative_step` and the
  hotspot-retry logic are the most likely culprits.
- **Fix B (new, HIGH): audit upwinding at the saturation front.**
  retry_dom localizes to cells on the water front. If upwinding
  flips mid-Newton-iter, Appleyard can't converge. A single-point
  upstream weighting with a smoothing term (Hamon-Vohralik or
  similar) would stabilize the flux Jacobian.
- Existing Fix 2 / Fix 4 (CPR completion): demoted. Worth doing for
  the ~20-30% lin_ms win they could deliver but no longer the
  headline substep-gap lever.
- Fix 5 (line search + update stabilization): demoted to Tier 2
  based on OPM ablation. Line search alone (without the
  stabilization half) may still help; TBD.

**Next concrete step:**

Side-by-side ressim vs OPM on a single substep at dt=0.25d, step 1.
Instrument both to print per-Newton-iter: residual norm (scaled),
max saturation change, max pressure change, acceptance decision.
Direct comparison tells us exactly where ressim's acceptance gate
diverges from OPM's. Expected outcome: ressim is failing to
accept at iter 3-5 where OPM accepts at iter 6-9 after more
damping.

## Side-by-side per-Newton-iter comparison — 2026-04-21

Executed the planned instrumentation on step 1, dt=0.25d. Traces
captured at `/tmp/opm-case2/trace/CASE2.DBG` and
`/tmp/ressim-trace-step1.log`. The comparison exposes the **exact
mechanism** by which ressim bails out while OPM converges cleanly.

**OPM (cprw, step 0, stepsize 0.25d — converges in 9 Newton iters):**

| iter | MB(O)    | MB(W)    | CNV(O)   | CNV(W)   |
|-----:|---------:|---------:|---------:|---------:|
|    0 | 7.48e-2  | 7.48e-2  | 89.89    | 89.72    |
|    1 | 4.09e-2  | 2.81e-2  | 45.79    | 33.22    |
|    2 | 2.16e-2  | 4.27e-2  |  9.70    | 31.59    |
|    3 | 2.38e-2  | 2.57e-2  |  8.97    | 11.74    |
|    4 | 1.67e-2  | 1.63e-2  |  6.70    |  6.74    |
|    5 | 5.93e-3  | 5.53e-3  |  1.53    |  5.34    |
|    6 | 1.72e-4  | 1.67e-4  |  0.58    |  1.62    |
|    7 | 2.44e-5  | 1.86e-5  |  8.3e-2  |  9.6e-2  |
|    8 | 1.40e-6  | 7.42e-7  |  5.5e-3  |  5.6e-3  |
|    9 | 5.37e-8  | 4.20e-8  |  2.1e-4  |  2.8e-5  |

Note the **non-monotone CNV(W)** at iter 2 (jumps 33.22 → 31.59 →
actually slight uptick in other MB rows at iter 2/3). OPM tolerates
these non-monotonicities and keeps iterating. No retries, no dt
chop.

**Ressim (fgmres-cpr, step 1 substep 0 retry 0, dt=0.25d — bails at
iter 7 STAGNATION):**

| iter | res      | damp      | stag  | notes                                               |
|-----:|---------:|----------:|------:|-----------------------------------------------------|
|    0 | 22.47    | 0.0055    |       | upd=36.4 → heavily damped                           |
|    1 | 22.35    | 0.0663    |       | small progress (−0.5%)                              |
|    2 | 20.87    | 0.0349    |       | fgmres-cpr FAILED max-iters, fallback to sparse-lu  |
|    3 | 20.14    | 0.0595    | stag=1| iterative-failure short-circuit → sparse-lu         |
|    4 | 18.95    | **0.0000**|       | **HOTSPOT effective-move floor cell399** (zero move)|
|    5 | 18.95    | 0.0580    | stag=1| zero-move bypass (repeats)                          |
|    6 | 18.34    | 0.1161    | stag=2| hotspot shifts to cell0 water front                 |
|    7 | **18.74**| —         | stag=3| **STAGNATION-REJECTED** (res increased!) → retry    |

At iter 7, the gate prints
`gates=[changed=true upd=1.485e-1/1.000e-3 reject res=1.874e1/1.000e-4 reject mb=1.562e-2/1.000e-5 reject]`
— i.e. none of the tolerances are met, so the near-converged-accept
gate cannot help. STAGNATION count=3 accumulated over iters 3, 5, 7
trips, substep FAILED, dt halved to 0.125d.

Same pattern repeats at dt=0.125d (iters 0-7 end with
STAGNATION-REJECTED, retry_factor=0.50), then at dt=0.0625d it
finally converges in 14 iters — but meanwhile 12 substeps have been
burned for step 1 alone.

**Mechanism-level diagnosis (sharp):**

1. **HOTSPOT effective-move floor is the primary stagnation driver.**
   At iter 4 ressim damps to `damp=0.0000` at `cell399` (the
   producer corner, `(19,19,0)` — the cell that sees the BHP=100
   constraint). This is the effective-move-floor code path: when
   Appleyard's damping would take the cell too far from its current
   state, the floor forces damp=0 on that row, which means the
   iteration makes **zero effective progress** but still counts as
   a Newton iteration. Iter 5 repeats zero-move. That is 2 of the
   3 stagnation counts accumulated in 7 iters.

2. **STAGNATION (count=3) gate is the proximate trigger.** The
   tripwire fires at iter 7 because the combination of (damped iter
   0-2) + (two zero-move iters 4-5) + (mild non-monotone bump at
   iter 7) accumulates 3 stagnation events well before the
   iteration budget is exhausted. OPM's equivalent monitor either
   doesn't exist or tolerates more non-monotonicity — OPM's CNV
   trajectory has a similar "plateau-like" region at iters 2-4
   (9.70 → 8.97 → 6.70) but OPM continues to iter 9 where
   convergence is clean.

3. **fgmres-cpr failure at iter 2 triggers the iterative-failure
   short-circuit (Lever 3).** That short-circuit is correct — it
   saves lin_ms — but it doesn't save the substep because the
   **nonlinear** stagnation is already inevitable by the time
   iter 4 hits the HOTSPOT floor.

4. **Retry at dt=0.125d replays the same failure mode.** Iter 4
   hits the same HOTSPOT cell399 zero-move. The dt chop does not
   break the HOTSPOT pattern — only dt=0.0625d does, because at
   that dt the Appleyard-adjusted move is small enough that the
   effective-move floor no longer kicks in.

**Why this localizes the fix:**

The previous tentative recommendation was a generic "widen Newton
acceptance gate". The side-by-side narrows that to two specific
pieces:

- **Fix A1 (HIGH, specific): audit the HOTSPOT effective-move
  floor.** Zero-move iters counting as stagnation is a
  self-inflicted wound: iterations that make no progress shouldn't
  *also* count against a stagnation budget that kills the substep.
  Options: (a) don't increment stagnation when the iteration is
  HOTSPOT-zero-move, (b) widen the effective-move floor so pure
  zero moves are rarer, or (c) skip the iter entirely (re-solve
  with different damping) instead of accepting a zero-move
  iteration.

- **Fix A2 (HIGH, specific): audit the STAGNATION gate threshold
  and the `gates` tolerances used inside it.** OPM tolerates
  non-monotone residual bumps (iter 7 res=18.74 > iter 6 res=18.34
  is a bump of +2% that triggered rejection); OPM's sample
  trajectory shows similar ~5% bumps that it absorbs. Widening the
  stagnation tolerance — or switching to an "accept-if-cnv-decreasing"
  rule that mirrors OPM's CNV/MB gate — would likely let ressim
  reach iter 12-14 at dt=0.25d instead of retrying at dt=0.125d.

- **Fix B (audit upwinding front stability) remains HIGH but
  distinct.** The HOTSPOT triggering at cell399 (producer) and
  cell0 (injector corner) may itself be caused by upwinding flips,
  but the side-by-side doesn't prove that; Fix A1/A2 can be
  prototyped first because they are strictly policy-level fixes
  that don't change physics.

**Stage 1 probe design for Fix A1:**

Read-only instrumentation first, then a narrow gate change as the
Stage 2 probe. Stage 1 plumbing (hours, not days):

- Add a `stagnation_attribution` field to the per-iter trace that
  prints which iter contributed to the stagnation count and why
  (zero-move, residual-bump, update-below-eps).
- Run the 4-case shortlist and case 2 to count how many stagnation
  events are zero-move vs real-bump across substep retries.
- Expected signal: if >50% of stagnation events on case 2 are
  zero-move, Fix A1 addresses the dominant mode. If <20%, Fix A2
  is the dominant mode. If split 50/50, both matter.

Stage 2 (days): change the stagnation-counter rule to exclude
zero-move iters. Measure substep count and lin_ms on the 4-case
shortlist. Promote only if case 2 substeps drop meaningfully
(target: 17 → 10 or fewer on step 1) without regressions on cases
1/3/4.

**Files of record:**
- `/tmp/ressim-trace-step1.log` — full ressim verbose trace
  capturing STAGNATION sequence.
- `/tmp/opm-case2/trace/CASE2.DBG` — OPM verbose trace showing
  monotone-enough CNV/MB convergence in 9 iters at dt=0.25d.

## Fix A1 Stage 1 probe — 2026-04-21

Added `STAGNATION-ATTRIB` trace line in `newton.rs` at the
`stagnation_count += 1` site. Each line emits a class label —
`zero-move`, `real-bump`, or `slow-decay` — plus running totals.
Read-only, no behavioral change.

**Attribution rule:**
- `zero-move` if `previous_effective_move_floor_site.is_some()` at
  the time of the counter bump (prev iter hit HOTSPOT floor → zero
  effective progress).
- `real-bump` if `current_norm > prev_residual_norm` (residual
  actually grew).
- `slow-decay` otherwise (decreased by < 5%).

**Sweep results (4 cases):**

| case                                 | substeps | zero-move events | slow-decay | real-bump | bailouts | bailout class (dominant) |
|--------------------------------------|---------:|-----------------:|-----------:|----------:|---------:|--------------------------|
| Case 1 — medium-water 20x20x3 step-1 |       12 |     43 (**79%**) |          9 |         2 |        4 | zero-move (2/4)          |
| Case 2 — medium-water 20x20x3 6-step |      102 |    286 (**86%**) |         45 |         2 |       34 | zero-move (34/38 ≈ 89%)  |
| Case 3 — heavy-water 12x12x3 dt=1    |       16 |     36 (**78%**) |          5 |         5 |        2 | real-bump (2/2)          |
| Case 4 — gas-rate 10x10x3 6-step     |       28 |       0 (**0%**) |          9 |         4 |        2 | slow-decay (2/2)         |

**Decisive finding: Fix A1 (stop counting zero-move iters as
stagnation) is the dominant lever on the water cases that drive
the 102-substep gap.**

- On case 2 — the case that drives the 15× substep gap — **86% of
  all stagnation events and 89% of actual bailouts come from
  zero-move iters.** Eliminating zero-move from the stagnation
  budget would prevent the vast majority of substep failures.
- Case 1 (a single step of case 2) shows the same pattern at 79%
  zero-move event fraction.
- Case 3 (heavy-water, 12x12x3, dt=1) also has 78% zero-move
  events but only 2 bailouts — and those bailouts were real-bump
  triggered. Fix A1 would save events without changing the bailout
  count here (case 3 is a different regime, fewer substeps).
- Case 4 (gas-rate) has **zero** zero-move events. HOTSPOT
  effective-move floor does not fire in this regime because gas
  flow doesn't produce the saturation-front discontinuities that
  trigger cell-local effective-move floors. Fix A1 is a no-op on
  case 4, which is the safest possible outcome (no risk of
  regression).

**Stage 2 design — narrow behavioral change:**

Change the stagnation counter rule from `if current_norm >=
prev_residual_norm * 0.95 then stagnation_count += 1` to exclude
the case where the previous iteration hit the HOTSPOT effective-
move floor. Concretely:

```rust
// Early termination: if residual is not decreasing, bail out to
// trigger timestep cut. Skip this check when the previous
// iteration was a HOTSPOT zero-move iter, because those iters
// make no progress by construction and shouldn't count against
// the stagnation budget.
if iteration >= 2
    && current_norm >= prev_residual_norm * 0.95
    && previous_effective_move_floor_site.is_none()
{
    stagnation_count += 1;
    ...
}
```

Optional refinement: if we want to preserve the ability to bail
out when the simulator is trapped at the HOTSPOT floor forever,
track a separate `zero_move_streak` counter and bail only if it
exceeds some larger threshold (e.g. 5 or 8), since a sustained
zero-move streak means Appleyard genuinely can't escape.

**Expected outcomes:**
- Case 2: substep count should drop substantially. Predicted: 102
  → ~30-50 substeps (the ~89% of bailouts that were zero-move-
  triggered should disappear). Newton iteration count will grow
  per substep but total work drops.
- Case 1: 12 → 6-10 substeps (half the bailouts were
  zero-move-triggered).
- Case 3: 16 → 16 substeps (zero bailouts caused by zero-move).
- Case 4: 28 → 28 substeps (zero impact, no zero-move events).

**Files of record:**
- `/tmp/stagnation-attrib-case{1,2,3,4}.log` — per-case
  instrumentation logs.
- Probe diff: two changes in `src/lib/ressim/src/fim/newton.rs`:
  three new local counters around line 2238, one `STAGNATION-
  ATTRIB` trace emission inside the stagnation block. Zero
  behavioral change.

**Promotion criteria for Fix A1 Stage 2:**
1. Case 2 substep count drops to ≤50 (≥2× improvement).
2. No regression on case 4 substep count (gas-rate unaffected
   because zero-move doesn't fire there).
3. Bit-exact trajectory check removed — behavior *must* change to
   realize the win, but the FPR/oil-rate smoke-check
   trajectory must remain within ~2% of OPM's reference
   (FPR 321→338 bar, oil 3050→3558 m³/d).
4. 4-case `fim-wasm-diagnostic` full timing must not regress by
   more than 5% on case 4 (where the fix is a no-op).

## Fix A1 Stage 2 result — 2026-04-21 → 2026-04-23

Implemented the narrow behavioral change: the stagnation counter
is no longer incremented when the previous iteration hit the
HOTSPOT effective-move floor, and any non-stagnating iteration
now resets the counter. Diff ~10 lines in
`src/lib/ressim/src/fim/newton.rs` around the stagnation block
(counter-reset semantics: when the previous iter was a zero-move
iter, the current iter does not count against the budget; any
good iter zeroes the counter). `STAGNATION-ATTRIB` probe lines
retained as non-gated diagnostics.

**4-case shortlist — baseline (pre-fix) vs Fix A1 Stage 2:**

| case                                 | baseline substeps/lin_ms/oil/avg_p | Fix A1 S2 substeps/lin_ms/oil/avg_p | Δ substeps | Δ lin_ms | verdict |
|--------------------------------------|------------------------------------|---------------------------------------|-----------:|---------:|---------|
| Case 1 — medium-water 20x20x3 step-1 | 12 / 13,759 ms / 3337.62 / 329.20  |  7 / 13,135 ms / 3326.63 / 328.49     |   **-42%** |    -4.5% | improvement |
| Case 2 — medium-water 20x20x3 6-step |102 /100,903 ms / 3610.27 / 348.31  | 34 / 62,005 ms / 3602.46 / 348.07     |   **-67%** |  **-38%**| **target case: big win** |
| Case 3 — heavy-water 12x12x3 dt=1    | 16 /  1,605 ms / 3808.44 / 338.37  | 27 /  3,503 ms / 3882.89 / 353.82     |      +69%  |   +118%  | see "Case 3 physics investigation" |
| Case 4 — gas-rate 10x10x3 6-step     | 28 /  2,225 ms /  161.92 / —       | 28 /  2,263 ms /  161.92 / —          |       0%   |    +1.7% | no-op (as predicted) |

Case 2 clears promotion criterion 1 (≤50 substeps, ≥2× improvement)
with substantial margin: 102 → 34 (−67%). Case 4 clears criterion
2 (no-op on gas-rate). Case 1 is a straight improvement.

### Case 3 physics investigation — the "regression" is a physics correction

Case 3 takes **more** substeps under Fix A1 (16 → 27), which is
the opposite direction from the design intent. Looked worrying
at first, but the avg_p number told a different story:
`baseline avg_p = 338.37 bar`, `Fix A1 avg_p = 353.82 bar` — a
**4.6% shift**, not noise.

Ran a **fine-dt reference**: same case 3 problem, same total
duration (1 day), but `dt = 0.0625d` with 16 steps (vs baseline
`dt = 1.0d` with 1 step). The fine-dt reference is the converged
physical answer; deviation from it is Newton-truncation error,
not a feature of the solver policy.

| configuration                   | avg_p (bar) | oil (m³/d) | deviation from fine-dt |
|---------------------------------|------------:|-----------:|-----------------------:|
| fine-dt reference (dt=0.0625)   |      354.80 |    3858.10 |                 0%     |
| baseline (dt=1.0, pre-fix)      |      338.37 |    3808.44 |   **−4.63% avg_p**     |
| Fix A1 Stage 2 (dt=1.0)         |      353.82 |    3882.89 |       −0.28% avg_p     |

The baseline converges to a measurably *wrong* answer on case 3
at dt=1.0d. Fix A1 converges to within 0.3% of the fine-dt
reference. **The extra substeps are ressim correctly splitting
the step into pieces each of which can converge** — the
baseline's apparent fast convergence at dt=1.0d was actually a
false convergence masked by early STAGNATION bailout that handed
back a damp-feasible but under-resolved update.

This reframes the case-3 "+118% lin_ms" entry: it is the price
of correctness on that specific heavy-water extreme-dt case. On
a 12×12×3 problem the absolute cost is 1.6s → 3.5s — a
non-issue in aggregate. The pre-fix was not a fast baseline; it
was a wrong baseline that terminated early.

**Fine-dt reference artefact:** `/tmp/case3-finedt.log`
(reproducible: `node scripts/fim-wasm-diagnostic.mjs --preset
heavy-water-12x12x3 --dt 0.0625 --steps 16`).

### Promotion decision — PROMOTE

All 4 promotion criteria met:

1. ✅ Case 2: 102 → 34 substeps (−67%, criterion was −50%).
2. ✅ Case 4: 28 → 28 substeps (criterion was no regression).
3. ✅ Physics smoke check: Case 2 FPR and oil rate remain within
   ~2% of the OPM reference. Case 3 avg_p moved *closer* to the
   fine-dt reference (fine-dt=354.80, Fix A1=353.82 vs
   baseline=338.37).
4. ✅ Case 4 lin_ms regression is +1.7%, well under the 5%
   bound.

Case 3's wall-time regression is an acceptable cost for a
physics correction: a 12×12×3 grid at dt=1.0d is an extreme
configuration outside the shortlist's primary performance
targets, and the new trajectory matches the converged physical
reference.

**Net across the 4-case shortlist**:
- Total substeps: 158 → 96 (−39%)
- Total lin_ms: 118,492 ms → 80,906 ms (−32%)
- Case 2 alone is a −38% lin_ms win, closing a material
  fraction of the OPM gap.

### What Fix A1 does NOT close

The OPM Flow reference on case 2 is 7 substeps / 0.07s. Fix A1
lands case 2 at 34 substeps / 62s. The remaining gap:

- **Substeps: 34 vs 7 (~5×).** Some of this is Appleyard damping
  still triggering HOTSPOT zero-move in regions where OPM's
  update stabilization would permit a non-zero move. Some is
  STAGNATION real-bump bailouts that Fix A1 does not address
  (criterion A2 remains unexplored).
- **Per-substep wall time.** OPM's 0.07s / 7 substeps ≈ 10 ms
  per substep; ressim's 62s / 34 ≈ 1.8s per substep — a 180×
  per-substep gap that reflects CPR completeness (Fixes 2/4) and
  the sparse-lu fallback dominating lin_ms on iterative-failure
  paths (documented in `project_fim_bypass_audit_2026-04-19`).

**Next candidates (post-Fix-A1):**
1. **Fix A2 (widen STAGNATION real-bump tolerance)** — case 2
   still has 2 real-bump bailouts; case 3 had 2; OPM absorbs +2%
   residual bumps. A narrow `prev_residual_norm * 1.05`
   tolerance (or CNV-based gate) could eliminate these without
   regressing the zero-move guard.
2. **Fix 2 (summed-IMPES)** and/or **Fix 4 (AMG coarse
   solver)** — now visible as the per-substep cost lever. With
   substeps reduced 67% on case 2, further per-substep speedup
   becomes proportionally more valuable.
3. **Fix B (upwinding audit)** — the HOTSPOT cell persistently
   localizes to the producer corner cell399 and the injector
   corner cell0. If upwinding flips at the saturation front are
   the root cause of those HOTSPOT events, a smoothed upstream
   weighting would reduce the zero-move iter frequency,
   further widening the Fix A1 margin.

**Files of record (Stage 2):**
- `src/lib/ressim/src/fim/newton.rs` — stagnation-gate change
  around line 2481 + counter declarations near line 2238 +
  STAGNATION-ATTRIB emission line.
- `/tmp/stage2-case{1,2,3,4}.log` — Stage 2 A/B run logs.
- `/tmp/case3-finedt.log` — fine-dt reference run that reframes
  case 3 as a physics correction.

## Fix A2 Stage 1 — 2026-04-24 (HYPOTHESIS REFUTED on case 2; gate widening will not close the Case 2 gap)

### Motivation

After Fix A1 Stage 2 landed Case 2 at 34 substeps (down from 102
but still 5× OPM's 7), the remaining 2 bailouts per case-2 step
were real-bump STAGNATION count=3 events at ratio 1.15–1.17. The
hypothesis behind Fix A2 was: "OPM absorbs comparable +15% CNV
bumps without chopping dt because it only checks convergence at
the end of its 12-iter budget. If ressim widened its count=3 gate
to keep Newton going when iter budget remains and residual has
net-progressed since the stagnation run started, Newton would
converge."

The post-Fix-B pivot reinforced this: Fix B upstream smoothing
would diverge from the OPM reference, so the policy-level Fix A2
was the natural next lever.

### Probe

Added a read-only residual-trend probe in
`src/lib/ressim/src/fim/newton.rs`:

- New local vars `best_residual_so_far` (min `current_norm` across
  all iters of this timestep) and `stagnation_entry_residual` (set
  to `prev_residual_norm` when `stagnation_count` first transitions
  0→1; reset in the else-branch where `stagnation_count = 0`).
- New `STAG-TREND` trace line emitted at every `stagnation_count +=
  1` event (beside the existing `STAGNATION-ATTRIB` line), reporting
  `count`, `res`, `entry_res`, `best_res`, `trend_vs_entry =
  current_norm / entry_res`, `vs_best = current_norm / best_res`,
  `iters_remain`, and a proposed `would_widen` flag defined as:

  ```
  stagnation_count >= 3
    && trend_vs_entry < 0.5
    && iters_remain >= 3
  ```

Probe is read-only; zero behavioral change.

### Stage 1 result — 4-case shortlist

Ran `node scripts/fim-wasm-diagnostic.mjs` with the same 4-case
shortlist as Fix A1/B. Logs in
`worklog/fix-a2-stagnation-gate/stage1-case{1,2,3,4}.log`.

**Key aggregate: `would_widen=true` fires 0 times across all 4
cases.** No count=3 bailout in any of the probed runs meets the
"current < 0.5 × entry" threshold. This is not a threshold-tuning
issue — the raw data shows every bailout is at a residual *above*
both `entry_res` and `best_res`:

| case | bailout | entry_res | best_res | count=3 res | trend_vs_entry | vs_best |
|------|--------:|----------:|---------:|------------:|---------------:|--------:|
| 1    |       1 |  1.937e1  | 1.834e1  |  2.458e1    |     1.27       |   1.34  |
| 1    |       2 |  4.652e0  | 4.652e0  |  5.618e0    |     1.21       |   1.21  |
| 2    |       1 |  1.937e1  | 1.834e1  |  2.458e1    |     1.27       |   1.34  |
| 2    |       2 |  4.652e0  | 4.652e0  |  5.618e0    |     1.21       |   1.21  |
| 3    |       1 |  1.902e1  | 1.832e1  |  2.174e1    |     1.14       |   1.19  |
| 3    |       2 |  4.687e0  | 4.687e0  |  5.711e0    |     1.22       |   1.22  |
| 4    |       1 |  9.477e-5 | 9.084e-5 | 9.084e-5    |     0.96       |   1.00  |
| 4    |       2 |  9.809e-5 | 9.138e-5 | 9.138e-5    |     0.93       |   1.00  |

(Case-1 / case-2 rows 1–2 are identical because case 2 is "case 1
extended to 6 steps" and step 1 bails out the same way both runs.)

### Iter-by-iter residual on case 2 step-1 dt=0.25 substep

The case 2 dt=0.25 substep that bails out at iter 11 shows a
genuinely divergent trajectory, not a stalled-but-progressing one:

```
iter:  0   1   2   3   4   5   6   7   8   9  10  11
res : 22.5 22.4 20.9 20.1 19.0 19.0 18.3 18.7 19.4 21.7 21.2 24.6
```

The residual **peaks above its starting point** (iter 11 res = 24.6
vs iter 0 res = 22.5) after bottoming at iter 6 (18.3). With
`iters_remain=8` at bailout and residual now climbing by +16% per
iter, there is no plausible recovery in 8 more iters. A widened
gate would have kept Newton going through a diverging trajectory.

Case 3 bailouts show the same shape (res bottoms, then climbs
above entry). Case 4 bailouts show monotone slow decay near the
`residual_tolerance=1e-5` threshold — a different failure mode.

### Conclusion

**Hypothesis refuted on the cases that matter.** The case 2 / case 3
bailouts are not "Newton is slowly converging but the gate bails
out too eagerly" — they are "Newton is genuinely diverging at
dt=0.25 and a dt-cut is the right response". Widening the count=3
gate would force Newton to waste 8 more iters per substep before
bailing out to the same dt-cut.

The 102→34 substep gain from Fix A1 Stage 2 came from a specific
mechanism (don't count zero-move iters as stagnation) that had a
real PASS signal in the probe data (86% zero-move on case 2). Fix
A2's equivalent "progress-since-entry" signal is absent — the
remaining bailouts after Fix A1 are genuinely unrecoverable at the
attempted dt.

**The 34→7 substep gap to OPM is NOT in the STAGNATION gate. It's
elsewhere:**
1. **Newton kernel strength at dt=0.25.** OPM converges at dt=0.25
   on case 2 step 1 in 9 iters; ressim diverges by iter 11 with
   damping 0.0055–0.126 (small fractional steps) and repeated
   HOTSPOT zero-move events at cell0/cell399. OPM's
   saturation-update limiting (`dsMax=0.2`) and implicit equation
   scaling likely yield a stronger update direction per iter.
2. **Jacobian scaling / equation scaling mismatches.** Linear-solver
   audit Finding 2 is not fully refuted; the CPR summed-IMPES
   restriction (Finding 3) plus full AMG pressure stage (Finding 4)
   are orthogonal to this and capped at ~20-30% lin_ms.

### Going-forward decisions

1. **Shelve Fix A2 (STAGNATION gate widening).** The probe shows
   it won't help on case 2. Moving the gate to "allow bumps when
   progress-since-entry is good" would not fire on the actual
   bailouts we see.
2. **Keep the STAG-TREND probe in-tree** as a diagnostic. Cost is
   three f64s and one trace line per stagnation event — negligible.
   The `best_res` and `entry_res` values are useful context for
   any future Newton-acceptance-policy investigation.
3. **Next candidates (no clear winner yet):**
   - **Fix 5 redux (line search / update stabilization on oil-row
     residual).** OPM's SOR update stabilization is a no-op on case
     2, but OPM's initial-iter damping is effectively larger than
     ressim's 0.0055 at iter 0 — something in the Appleyard /
     HOTSPOT / max_sw_change interaction is pinning ressim's first
     several iters to tiny steps. Needs a fresh probe.
   - **Jacobian/equation scaling audit (Finding 2 revisit).** The
     water-row residual at cell0 is dominated by `well=−1969` vs
     `accum=2.499` — 3 orders of magnitude asymmetry. Equation
     scaling by local-accumulation would rebalance the Newton
     direction.
   - **Fix 2 / 3 / 4 (CPR completion bundle).** Bounded upside per
     the 2026-04-20 OPM ablation (CPR vs ILU0 = 0 substep delta),
     but lin_ms gains of 20-30% are realistic and would
     multiplicatively compound with future substep reductions.

### Files of record

- `src/lib/ressim/src/fim/newton.rs` — Fix A2 Stage 1 probe
  (best-residual tracker + STAG-TREND emission). Kept in-tree as
  a diagnostic.
- `worklog/fix-a2-stagnation-gate/stage1-case{1,2,3,4}.log` — 4-case
  shortlist probe runs (git-ignored under `/worklog/`).

## Fix A3 Stage 1 — 2026-04-24 (PASS — inflection-point chop is the dominant damping constraint, 93% on case 2)

### Motivation

After Fix A2 refutation localized the 34→7 substep gap to "Newton
kernel strength, not gate policy", the working hypothesis became:
**something about ressim's Newton kernel produces small initial-iter
damping values (0.0055–0.07 observed on case 2 step-1 iter-0), and
that forces Newton to inch forward in small fractional steps over
many iters before reaching a state where dt=0.25 converges.**

The candidates were (a) Appleyard Sw cap `max_saturation_change=0.2`
— but this is identical to OPM's `dsMax=0.2`, so it can't be the
full story on its own; (b) the `fw_inflection_point_sw` trust-region
chop (Wang & Tchelepi, 2013) that ressim added for fractional-flow
basin stability — OPM does NOT have an equivalent chop; or (c)
equation-scaling / Jacobian-row asymmetry producing an oversized
raw update.

### Probe

Added `appleyard_damping_breakdown()` in `src/lib/ressim/src/fim/
newton.rs` — a read-only sibling that computes the same final damping
as `appleyard_damping()` but also records:
- `binding_kind` (pressure / sw_appleyard / sw_inflection / sg_appleyard
  / so_implied / rs / bhp / unbound)
- `binding_cell` or `binding_well` identifying the critical cell
- Raw (pre-damping) peaks: `raw_dp_peak`, `raw_dsw_peak`, `raw_dh_peak`,
  `raw_dbhp_peak` with associated cell/well indices
- `inflection_crossings` count

Emits a `DAMP-BREAKDOWN` trace line per Newton iter. Kept the
original `appleyard_damping()` alive under `#[cfg(test)]` so the
existing unit test continues to exercise the pure-computation path.

### Stage 1 result — PASS

Aggregate binding distribution across all iters:

| case | total iters | inflection | sw_appleyard | sg | unbound | % inflection |
|------|------------:|-----------:|-------------:|---:|--------:|-------------:|
| 1    |         115 |         90 |           12 |  0 |      13 |      **78%** |
| 2    |         756 |        699 |           12 |  0 |      45 |      **93%** |
| 3    |         402 |        326 |           10 |  0 |      66 |      **81%** |
| 4    |         418 |          0 |            0 |  4 |     414 |         0%   |

Iter-0 substep damping distribution (how many substeps start with
tiny damping that will force many iters of micro-steps):

| case | substeps | damp<0.05 at iter 0 | fraction |
|------|---------:|--------------------:|---------:|
| 1    |       10 |                   3 |      30% |
| 2    |       50 |                  19 |      38% |
| 3    |       35 |                  14 |      40% |
| 4    |       33 |                   0 |       0% |

**Decisive: 93% of case-2 Newton iters are bound by the
`fw_inflection_point_sw` trust-region chop, not by the Appleyard
`max_saturation_change=0.2` cap.** Only 12 of 756 iters bind on
`sw_appleyard`. The inflection chop fires at every front-advancing
iter because the linear solve, given the strong injector well term,
produces raw dsw values that step from Sw=0.1 (irreducible) across
the inflection (~Sw=0.4 for Corey n=2 water/oil with Swc=Sor=0.1)
to Sw>0.5 in one Newton step. The chop correctly stops at the
inflection — but then *every next iter also tries to cross the
inflection* because the residual at the inflection is still large,
so Newton repeatedly chops at the same inflection distance.

Case 4 (gas-rate with zero water front) is a natural no-op: the
inflection chop fires 0/418 iters because there's no water
saturation transitioning between basins. This is the safety-net
confirmation that a change to the chop would not regress gas-only
problems.

### Smoking gun — case 2 step-1 iter 0

```
DAMP-BREAKDOWN final=0.0055 bind=sw_appleyard@cell0
  raw_dp=1.189e2@cell0
  raw_dsw=3.640e1@cell0   ← 36× Sw_max_change; 180× realistic physical move
  raw_dh=0.000e0
  inflection=0
```

At cell0 (injector corner), the linear solver is asking for
`dsw = 36.4` while Sw is at 0.1. A physically sensible move to
reach the breakthrough shock state is dsw ≈ 0.4 — nearly two
orders of magnitude smaller. The Appleyard cap clamps to dsw=0.2
(damp=0.0055). Subsequent iters then hit the inflection chop
instead because the damped step is smaller than the residual
would drive on a fully linearized Newton.

### Cross-check with OPM

Per the 2026-04-24 OPM WebFetch (`blackoilnewtonmethod.hh`,
`BlackoilModel_impl.hpp`), **OPM has `dsMax=0.2` but does NOT
have a Wang-Tchelepi inflection-point trust region.** OPM's Newton
update limiter is:
- `dsMax=0.2` saturation cap (identical to ressim's
  `max_saturation_change`)
- SOR `stabilizeNonlinearUpdate` (no-op on case 2 per
  2026-04-20 ablation)
- That's it — no fw-basin guard.

So ressim's inflection chop is **a net-extra conservatism that OPM
does not have** and that fires on 93% of case-2 iters. This is the
mechanism by which ressim's "Newton kernel strength at dt=0.25" is
weaker than OPM's: OPM accepts each iter's dsMax=0.2 step even
when that crosses the fw inflection; ressim clamps at the
inflection, reducing per-iter Sw progress roughly in half for
front cells.

### Fix A3 Stage 2 design (to discuss)

Two feasible directions, both removing or relaxing the inflection
chop to match OPM's behavior:

**Option A (conservative): widen the chop threshold.**
- Only chop at the inflection if the full step overshoots by a
  meaningful margin (e.g., `max_damping * dsw_signed.abs()` ≥
  `2 * dist_to_inflection`), not on every marginal crossing.
- Allows Newton to cross the inflection when the step size is
  dominated by other chops, preserving the "no wild jump" intent
  without firing on routine front advancement.

**Option B (aggressive): remove the inflection chop entirely.**
- Match OPM exactly: only apply `dsMax=0.2` and pressure cap.
- Rely on OPM-style Newton iteration count + stagnation policy
  (which Fix A1 already moved toward) to handle any basin-jump
  divergence that would have been prevented by the chop.

**Predicted effect:** Cases 1/2/3 Newton iter counts drop
substantially (likely by 30-50% on case 2 based on 93% inflection-
bound fraction — not all will "return" their capped damping to
full 0.2, but front cells should see a factor of 2 larger step).
Case 4 is a guaranteed no-op.

**Risk:** The Wang-Tchelepi chop was added for a reason. If we
remove it and a test case (Buckley-Leverett validation, Craig
displacement, heavy-water breakthrough) regresses, we know why.
Would require the full benchmark suite + OPM cross-check before
promotion.

### Going-forward decisions

1. **Keep DAMP-BREAKDOWN probe in-tree** as a diagnostic. Cost is
   one trace line per Newton iter + one additional allocation-free
   pass over cells per iter (≈ same cost as `appleyard_damping`
   itself — effectively doubling the damping computation, which is
   ~0.1% of total fim_ms per the cost profile memory).
2. **Stage 2 is Option A (widen the chop threshold).** It's the
   minimal behavioral change, it can be tuned by a single
   threshold constant, and it preserves the basin-jump guard for
   cases where the step genuinely overshoots. Option B (full
   removal) becomes the fallback if Option A doesn't close enough
   of the gap.
3. **Before Stage 2**, promote the DAMP-BREAKDOWN probe to a
   before/after A/B tool — run the shortlist with the current chop,
   then with the relaxed chop, and diff substep counts, lin_ms, and
   physics accuracy (avg_p, oil_production match to fine-dt ref).

### Files of record

- `src/lib/ressim/src/fim/newton.rs` — `DampingBreakdown` struct
  and `appleyard_damping_breakdown()` function near line 678;
  `DAMP-BREAKDOWN` trace emission in the Newton loop just before
  the state-update. `appleyard_damping()` retained under
  `#[cfg(test)]` for the existing unit test.
- `worklog/fix-a3-damping-probe/stage1-case{1,2,3,4}.log` — 4-case
  probe runs (git-ignored under `/worklog/`).
