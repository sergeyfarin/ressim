# FIM Wide-Angle Convergence Analysis (2026-04-19)

This doc steps back from the narrow-lever optimization trail (A/B/E
Newton-trial-policy attempts, Jacobian reuse, bypass audit) and asks a
broader question: **what do industry-standard black-oil FIM
implementations do that we might be missing, and where might there be
an actual bug versus "just slow"?**

Context for the reader: the last five direction attempts on this shelf
all ended in revert or mechanism refinement rather than a promoted
improvement:

- Direction A (Newton extrapolation, 3 rounds): reverted
- Direction B (dt-aware replay tolerance): reverted, no-op
- Direction E (water cross-step carryover): reverted, no-op
- Jacobian reuse (Stage 2): reverted, structural obstacle
- Bypass trigger audit: refuted its own 2026-04-18 framing —
  direct-bypass triggers are 2-3% of lin_ms; the real cost is
  `fgmres-cpr fails → sparse-lu rescues` at 68-75% of lin_ms

The bypass audit queued up Plan B (Lever 1 widen near-converged-accept
+ Lever 3 post-fail short-circuit). Before executing Plan B, this doc
asks: **is there a deeper reason fgmres-cpr is hitting max-iters so
often?** The answer may obviate Plan B entirely.

## Signals pointing at "bug / incomplete implementation" vs "just slow"

Raw evidence from the 2026-04-19 bypass audit trace on case 1 (medium
water 20x20x3 dt=0.25 step 1):

1. **Final residual 5 orders of magnitude above tolerance.** Example
   failure: `final_res=6.500e0 tol=2.102e-4 reason=max-iters`. Healthy
   iterative solvers either converge or diverge visibly; they do not
   get stuck at 30,000× tol while making small progress. This is
   either divergence masked as stagnation, or a near-singular linear
   operator that GMRES cannot make headway against.

2. **GMRES internal estimate diverges from recomputed candidate.**
   Same failure: `est_res=1.939e-1 cand_res=6.500e0`. The internal
   estimate is 30× smaller than the recomputed candidate. In a healthy
   CPR-GMRES these track. Divergence is a classic sign of either
   **indefinite / non-positive preconditioner** or **structurally
   wrong operator**.

3. **Restart progression is suspicious.** `r1 out=2.102e3 → r2 4.416e1
   → r3 2.273e1 → r4 1.479e1 → r5 8.533e0` — the outer norm drops
   ~50× in the first restart then stalls at ~1.5× per restart. A
   preconditioner that decays 50× on the first restart then stalls is
   doing the elliptic-like component well and the hyperbolic/front
   component not at all. That is literally the problem CPR-AMG is
   designed to solve, so if our "CPR" is not decoupling pressure, we
   are running raw ILU0 on a saddle-point black-oil operator and
   seeing exactly the failure pattern raw ILU0 produces on that
   operator.

4. **`retry_dom` is dominated by `water@0` — the Dirichlet injection
   boundary cell.** Every retry in the case 1 sample traces out to
   `cell0 = (0,0,0)`. A correctly-posed Dirichlet boundary should not
   dominate retries. Either the boundary condition is not truly
   Dirichlet (e.g. we're solving for a BHP constraint that gets
   clamped), or the perforation-through-boundary-cell coupling has
   an asymmetry in the Jacobian, or the boundary accumulation term is
   miscomputed.

5. **TODO.md item 5 on record: "CPR preconditioning (complete the
   FGMRES-CPR path): The current CPR path is incomplete — falls back
   to block-Jacobi + ILU0 for the pressure stage."** This is the
   user's own prior note. If accurate, it explains (1)-(3) directly:
   what we call `fgmres-cpr` is in fact GMRES-ILU0, and CPR is a label
   not a mechanism.

## Industry-practice checklist

Twelve angles from the canonical black-oil FIM literature and known
production simulators (ECLIPSE, OPM Flow, IX, CMG STARS/IMEX). For
each: what it is, whether we have it, whether it matters here.

### 1. CPR-AMG (pressure-stage AMG)
**What:** Constraint-Pressure-Residual decouples the full black-oil
system into an elliptic pressure system and a hyperbolic-ish
saturation system. The pressure stage is solved to tight tolerance
with AMG (algebraic multigrid) — mesh-independent convergence. The
saturation stage uses a cheap smoother on the full system. Standard
reference: Wallis 1983, Cao/Tchelepi 2005.

**Us:** CPR path is **incomplete** per TODO.md item 5. Falls back to
block-Jacobi + ILU0 for the pressure stage. No AMG.

**Does it matter here:** Almost certainly yes. Every symptom in
section 1 above is consistent with "we don't have CPR". Top candidate
for root cause.

### 2. True-IMPES or Quasi-IMPES decoupling operator (C)
**What:** The reduction to the pressure-only system requires a
decoupling operator `C`. True-IMPES forms `C` from accumulation-term
ratios; Quasi-IMPES is a cheaper approximation. "CPR" implementations
that omit this are CPR in name only — they produce a pressure system
that still couples to saturation and fails to get AMG's mesh
independence.

**Us:** Unknown, requires audit. Likely absent given (1) is absent.

**Does it matter here:** Only if (1) exists. If we're adding CPR from
scratch, add this as part of the same slice.

### 3. Proper block preconditioning
**What:** Black-oil has 3 equations per cell (water, oil, gas or
water, oil, Rs). Block-Jacobi at block size 3 (3×3 dense per-cell
inversion) is standard. Elementwise Jacobi is not.

**Us:** "block-Jacobi" is in the code name; whether it's 3×3 or
elementwise requires audit. Preconditioner build time is 123 ms /
19,401 ms = 0.6% — either it's fast because it's elementwise (cheap
but weak) or fast because block-size-3 is genuinely cheap (1202
3×3 inversions).

**Does it matter here:** Material only if it's elementwise. Check and
fix if wrong.

### 4. Variable scaling applied to the Jacobian, not just the update
**What:** Black-oil unknowns have very different scales: pressure
~10²-10³ bar, saturation ∈ [0,1], dissolved-gas ratio ~10-10² std
m³/rm³. Without scaling, the Jacobian is badly conditioned. Scaling
must be applied to the *system* being solved, not just to the update
norm for the convergence check.

**Us:** There is a `variable_scaling` field in assembly output; need
to verify whether it's used in the linear system or only in the
convergence-norm computation.

**Does it matter here:** If scaling is only cosmetic (norm-level), the
linear system is poorly conditioned and no preconditioner will save
it. High-leverage if present.

### 5. Equation scaling (mass-balance normalization)
**What:** Black-oil residuals in raw units mix kg/day, m³/day,
Pa·s/m; residual norms are physically meaningless unless equations are
normalized by pore volume, time step, or accumulation coefficient.
Some simulators multiply each equation by `dt/accum_coeff` — makes
the residual "unit-1" and makes CPR decoupling cleaner.

**Us:** Needs audit — `assemble_fim_system` writes the residual, but
whether the 3 equation families (water/oil/gas mass) have been
renormalized is unclear without reading.

**Does it matter here:** If absent, the iterative solver's tolerance
check is mixing units. Medium-leverage.

### 6. Line-search / trust-region Newton vs pure Appleyard damping
**What:** Appleyard damping is a per-cell 1D heuristic: "chop the
update so each cell's saturation stays in [0,1] and pressure doesn't
change by more than X%". It is not globally convergent — a Newton step
that points away from the solution is still taken, just shortened.
A proper line search (McCabe/Younis MP-LS, or trust region) checks
residual reduction before accepting and backtracks if it increased.

**Us:** Pure Appleyard damping, no residual-based line search. The
residual-based line search was explicitly removed (TODO item 3).

**Does it matter here:** Medium-high. Case 1 trace shows the residual
oscillating/stalling (2.24e1 → 2.23e1 → 2.08e1 → 2.01e1 → 1.89e1) while
Newton continues to take Appleyard-damped steps. Each of these
produces a failing fgmres-cpr (because the Jacobian around a stalled
residual is poorly conditioned). A line search that rejected those
steps would avoid the linear solves entirely.

### 7. ILU with fill-in (ILU(k), ILUT)
**What:** ILU(0) is the cheapest but weakest incomplete factorization
preconditioner. ILU(1), ILU(2), or ILUT with a threshold typically
give 3-5× better GMRES convergence at modest extra memory.

**Us:** ILU(0) only (gmres_block_jacobi.rs name suggests block-Jacobi
+ ILU0, confirming this).

**Does it matter here:** Only marginally if we add true CPR (pressure
AMG takes the load off ILU). If we stay without CPR, upgrading to
ILU(2) could be a 2-3× improvement on its own.

### 8. Block size awareness in CPR
**What:** CPR pressure-extraction assumes block size N (=3 for
black-oil). If the linear solver doesn't know the block size, it
cannot extract the pressure submatrix correctly.

**Us:** `FimLinearBlockLayout { cell_block_count, cell_block_size: 3,
well_bhp_count, perforation_tail_start }` is passed to solvers. Block
size is known. If we implement CPR, this is the API entry point.

### 9. Jacobian signs / gravity orientation / mobility upwinding
**What:** Classical black-oil bugs: gravity term sign reversal at
vertical faces, mobility upwinded based on potential rather than
pressure (or vice versa with wrong sign), accumulation term with wrong
derivative sign in a phase-transition cell.

**Us:** We have all of these features; their correctness requires
targeted audit. The fact that the benchmark suite (Buckley-Leverett,
Dietz, Craig/Dykstra-Parsons) passes is strong evidence against gross
sign errors — but subtle orientation bugs at the injection boundary
might still be present. Case 1 trace has `cell0` (injection boundary)
as the retry hotspot, which is a weak signal in this direction.

### 10. Residual-based convergence criterion (in addition to update-based)
**What:** FIM convergence requires BOTH `||update|| < tol_upd` AND
`||residual|| < tol_res`. If only update is checked, Newton may converge
to a non-solution (update is small because damping chopped it, not
because the iterate is at a root).

**Us:** Current code checks `final_update_inf_norm <= update_tolerance`
AND `current_norm <= residual_tolerance * ENTRY_RESIDUAL_GUARD_FACTOR`
AND `iterate_has_material_change`. Both criteria present; plus a
material-balance check. This looks correct.

### 11. Preconditioner freshness / reuse
**What:** Rebuilding ILU0 every solve is standard but wasteful if the
matrix structure is stable. Some codes rebuild every 3-5 calls.

**Us:** pc_ms = 123 ms / 19,401 ms = 0.6%. Not a lever here.

### 12. Mass-balance preconditioning / physics-based scaling
**What:** Some simulators scale each equation by `1/Δx³` (pore volume)
or by `PV_ratio` for each phase. Makes the elliptic pressure part
dominate in operator norm, which is what CPR wants.

**Us:** Unknown; needs audit. Typically set up in assembly.

## Synthesis — ranked by likely impact

**Tier 1 — likely bug or incomplete implementation, high leverage:**
- **Missing CPR pressure-stage decoupling (items 1+2).** TODO item 5
  says it's incomplete; trace symptoms (residual stuck 30000× tol,
  est vs cand divergence, restart progression) are all classic
  no-CPR-on-saddle-point patterns. Completing CPR likely collapses
  the 77% post-fail-fallback cost at the source.
- **Possibly absent Jacobian-level variable scaling (item 4).** If
  present but unused, the system is poorly conditioned and no
  preconditioner improvement helps.

**Tier 2 — medium leverage, worth checking but not expected root
cause:**
- Line search / trust-region Newton (item 6) — would remove the
  residual-stall failing-solve iters Plan B also targets.
- Equation scaling / mass-balance normalization (item 5) — enables
  CPR to work cleanly if added.
- Block-Jacobi block size verification (item 3) — cheap to audit.

**Tier 3 — unlikely to be the primary issue:**
- ILU fill-in upgrade (item 7) — only marginal without CPR.
- Jacobian sign audits (item 9) — benchmark suite passes, so gross
  errors are out; subtle boundary-orientation issues are possible but
  would have shown up in regression tests.
- Preconditioner freshness (item 11) — not on the lin_ms hot path.

**Tier 4 — already present and correct:**
- Residual-based convergence (item 10) — verified in code.

## Recommendation: pivot from Plan B to CPR completion audit

**Why Plan B is papering-over behavior:**
Lever 1 (widen accept gate) accepts more too-loose iterative results.
Lever 3 (post-fail short-circuit) skips the wasted iterative attempt.
Both reduce the *cost* of the `fgmres-cpr fails` path, but neither
fixes the underlying reason fgmres-cpr is failing. If the underlying
reason is "CPR is not actually implemented", fixing CPR eliminates
both the post-fail-fallback (~70% of lin_ms) AND the near-converged-
accept (~10%) categories simultaneously. It also likely reduces
substep retries classified as `nonlinear-bad:water@0`, which are
really `linear-bad-masquerading-as-nonlinear-bad` — a fallback
sparse-lu step that Newton immediately fails to damp because the
linear system was near-singular.

**Why Plan B remains the right secondary direction:**
Even with CPR completed, some iterative failures remain (genuine
saddle points, degenerate cells). Plan B's Lever 3 (post-fail
short-circuit) is still valuable as a generalized "don't retry what
just failed" policy. But its impact is then ~10-20% of lin_ms rather
than ~70%.

**Proposed next action: CPR completion audit (this session).**
Read-only audit:
1. `fim/linear/gmres_block_jacobi.rs` — full CPR code path end-to-end.
   Identify what exists, what's missing vs. Cao/Tchelepi textbook CPR.
2. `fim/linear/mod.rs` — backend dispatch, preconditioner setup,
   ILU0 block structure.
3. `fim/assembly/` and `fim/scaling.rs` — variable scaling usage;
   specifically whether `variable_scaling` modifies the Jacobian
   matrix itself or only the post-solve update norm.
4. Spot-check assembly around boundary cell `cell0` where every case 1
   retry is dominated — look for boundary/perforation coupling
   asymmetry.

Deliverable: `docs/FIM_LINEAR_SOLVER_AUDIT.md` with:
- Current state of each item 1-12 (present / absent / incomplete /
  buggy with file:line references)
- Root-cause diagnosis: which of the trace symptoms each
  finding explains
- Ranked fix proposals: smallest-first, with scope estimate and
  expected lin_ms impact
- Decision point: CPR completion vs Plan B vs something else

**What this audit will NOT do:**
- Not change any code. Pure read-only.
- Not run benchmarks. Benchmarks come after the fix design is
  concrete.
- Not speculate about fixes beyond what the code reveals. If the
  audit shows CPR is actually fine, the recommendation flips back to
  Plan B immediately.

## Potential outcomes

- **A. CPR is incomplete as TODO claims.** Highest probability. Next
  action: scope a CPR completion slice (likely 1-3 weeks of
  implementation given AMG integration, but a pressure-stage ILU0
  with proper True-IMPES decoupling is 2-3 days and captures most of
  the win). Plan B shelved until CPR done.
- **B. CPR is complete but something about the problem setup defeats
  it.** Medium probability. Next action: investigate the specific
  defeat mechanism — likely equation scaling or block structure.
- **C. CPR is fine, Jacobian has a subtle bug at boundary cells.**
  Lower probability given benchmark passes. Next action: targeted
  assembly audit around cell0 on case 1.
- **D. Everything in the linear stack is as-designed and the
  max-iters failures are genuine ill-conditioning of the problem
  (not the solver).** Lower probability. Next action: Plan B becomes
  the correct direction (it was never wrong — it just would have
  been papering over a solvable bug rather than a genuine limit).

Probability ranking: A > B > C > D. The audit distinguishes between
them.
