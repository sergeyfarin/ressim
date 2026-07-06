# FIM vs OPM Gap Analysis (SPE1 Newton-efficiency decomposition)

> Provenance: rescued 2026-07-05 from the orphaned `docs/2.md` (a pasted session analysis,
> ~May 2026, previously unindexed and with literal `\n` escapes). Kept because its 6-item gap
> decomposition against OPM's SPE1 numbers is the clearest strategic map of what separates
> ResSim FIM from OPM's ~2.5 Newton iterations/step with zero cuts.

## Status triage as of 2026-07-05 (added at rescue time — read this first)

| # | Gap identified below | Status now |
|---|---|---|
| 1 | Rs clamping / hysteresis band discards gas | **CLOSED** — immediate regime switching landed; regression tests in `fim/state.rs` assert the old 1% band is gone |
| 2 | No global material-balance convergence check | **CLOSED** — Newton now gates on `material_balance_inf_norm` alongside the residual check (`fim/newton.rs`) |
| 3 | Full Jacobian assembly during line search | **CLOSED** — `assemble_residual_only: true` exists and is used at the residual-check sites |
| 4 | Hotspot-aware timestep growth (DSMAXDT-style) | **PARTIAL** — Newton-iteration-based growth control exists (`accepted_step_growth_decision`, `fim/timestep.rs`); OPM-style post-step dSat/dP-proportional limiting not implemented |
| 5 | Variable substitution (regime switch inside Newton) | **OPEN** — ResSim still freezes regimes during the Newton loop and reclassifies post-convergence |
| 6 | Well Schur complement elimination | **CLOSED** — `FIM-LINEAR-010` (`fim/linear/well_schur.rs`), promoted 2026-07-04 |

The largest remaining architectural gap vs. OPM is not in this list: the **AMG coarse solver for
CPR** (see `docs/FIM_OPM_ALIGNMENT_STRATEGY_2026-04-26.md` "Bundle C" and registry row
`FIM-LINEAR-006`). The linear stack is otherwise OPM-aligned as of Phase 10/11 (loose tolerance
`5e-3`/budget `20`, block-ILU0 smoother, quasi-IMPES restriction, well elimination).

---

## Why OPM Solves SPE1 in ~2.5 Newton Iterations/Step — and What's Missing

**OPM's numbers decoded:**
- 311 Newton iterations / 123 timesteps = **2.53 Newton/step average**
- 437 linear iterations / 311 Newton = **~1.4 linear iterations/Newton** (BiCGSTAB converges almost immediately)
- **0 wasted iterations** — zero timestep cuts, zero step failures

That last point is the most revealing: OPM never cuts a timestep on SPE1. Every attempted step succeeds. Our solver is still fragmenting SPE1 into multiple substeps and hitting the retry path.

---

### Root Causes, Priority Order

**1. Rs clamping material balance violation** ← most impactful, correctness issue

The worklog section "BUG FOUND" at line 1485 identifies this clearly. In `fim/state.rs`, `classify_regimes()` silently discards dissolved gas when `Rs_sat < Rs < 1.01 × Rs_sat`:

```rust
self.cells[idx].hydrocarbon_var = rs_sm3_sm3.min(rs_sat);  // ← CLAMP, no flash
```

OPM uses **immediate variable substitution** — the moment Rs ≥ Rs_sat, the primary variable switches from Rs to Sg and excess dissolved gas converts to free gas. No hysteresis band, no gas discarding. Our 1% band is a one-way valve that:
- makes the gas front stall near the injector
- puts Newton in a loop over physically damaged state
- causes the substep fragmentation seen on SPE1 gas cases

Two tests are still failing that directly assert this fix: `classify_regimes_switches_immediately_when_rs_exceeds_rs_sat` and `classify_regimes_preserves_gas_inventory_when_undersaturated_state_exceeds_rs_sat`. The fix (remove the hysteresis band for undersaturated→saturated) is documented, just not landed.

**2. No global material balance convergence check** ← correctness/false-convergence

OPM requires **CNV + MB** to declare convergence. CNV catches per-cell residuals. MB catches cases where individual cell residuals are small but don't sum to zero globally — meaning the global conservation equation isn't actually satisfied. We only have CNV-equivalent. After the Rs clamping bug modifies state post-Newton, a global MB check would reject those steps rather than accepting a non-conservative solution.

**3. Full Jacobian assembly during line search** ← 2-4× cost multiplier per Newton iteration

From the worklog at line 1559:

> Each damped trial candidate calls `assemble_fim_system`, which builds both the residual and the full Jacobian. Only the residual norm is needed for the line search acceptance decision. The Jacobian is discarded.

OPM doesn't use a residual-based line search at all — it relies on tighter Appleyard chops (ΔSw ≤ 0.1–0.2, ΔP ≤ 200–300 bar) that prevent divergence without needing to check residuals at intermediate damped states. Our limits (ΔSw ≤ 0.5, ΔP ≤ 500 bar) are too generous, forcing the line search to compensate.

Fix path: add a `residual-only` assembly mode (the infrastructure already exists for FD tests), and simultaneously tighten Appleyard limits to OPM-class values. This would eliminate the line search entirely.

**4. Well Schur complement vs relaxation approximation** ← well-cell coupling correctness

OPM algebraically eliminates the well block from the global Newton system before solving:
```
r_reservoir = r_res - C D^{-1} r_wells
update_wells = D^{-1} (r_wells - C^T update_res)
```

We have a post-update **relaxation** toward a locally-consistent well state. The worklog's local well trust region (March 2026) improved things materially but doesn't fully prevent well-variable drift across Newton iterations. The diagnostic evidence at line 1065–1093 confirms that hard water breakthrough failures are "large-distance-to-local-manifold failures in explicit well unknowns" — well unknowns wandering to physically impossible BHP/rate states.

Full Schur complement elimination is the right fix but large effort. The intermediate path (bounded well-variable updates tighter than Appleyard chops on cell variables) is a smaller step already partially done.

**5. Coarse pressure solver quality** ← linear solver scale-up

OPM uses AMG for the pressure coarse system. We use either dense exact inverse (good, but only below ~512 rows) or an ILU-style defect correction loop. The worklog confirmed at line 1439 that for representative 3D cases the extracted coarse system is already solved to machine precision (average reduction ratio ~1e-13). So this is no longer the bottleneck for those cases — but for larger SPE1-class grids and longer runs, the restriction/prolongation quality (quasi-IMPES-style weights, now implemented) matters more than the coarse solver itself.

**6. Frozen-regime Newton vs variable substitution** ← phase transition quality

We freeze regimes (Saturated/Undersaturated) during the Newton loop and switch only post-convergence. Combined with the Rs clamping bug this creates the gas stall. Even after fixing the clamping bug, frozen-regime means Newton can solve correctly for Rs > Rs_sat and then have the state overwritten by `classify_regimes`. Variable substitution (switch inside Newton) is OPM's approach and eliminates the inconsistency entirely. Medium effort.

---

### Why OPM Specifically Gets 2.5 Newton/Step on SPE1

The combination of the above differences means:

| Factor | OPM | Our FIM |
|--------|-----|---------|
| Linear solver per Newton | ~1.4 BiCGSTAB iters (AMG-CPR) | 10–20 FGMRES iters (ILU-CPR) |
| Newton per step (SPE1) | ~2.5 | ~16 (first step budget) + substep fragments |
| Failed steps | 0 | Multiple substep retries |
| Rs transition | Immediate variable substitution | Clamped + hysteresis → gas discarded |
| Well coupling | Schur complement | Relaxation approximation |
| Line search | None (tighter Appleyard only) | Full Jacobian rebuild per damped candidate |
| MB criterion | CNV + MB | CNV only |

OPM never cuts a timestep because: the linear solver direction is good enough that Newton descends cleanly in 2-3 iterations, the Appleyard chops prevent divergence without needing a line search, and the Rs transition is handled exactly so Newton isn't solving a physically inconsistent system.

---

### What Needs to Be Fixed (Priority Order from the Worklog Evidence)

1. **Fix the Rs clamping bug** — remove the 1% hysteresis band for undersaturated→saturated. The two failing classify_regimes tests pin this exactly. This is the highest-impact correctness fix.

2. **Add global MB convergence check** — sum-of-residuals per component in addition to per-cell CNV. Safety net that catches state-modification errors like (1).

3. **Residual-only assembly in damping loop + tighter Appleyard** — eliminate the full Jacobian rebuild cost per damped candidate. Tighten ΔSw to ~0.1–0.2 and ΔP to ~200 bar so the line search can be eliminated entirely.

4. **Hotspot-aware timestep growth** — the 3-substep oscillation cycle (accept→1.25×grow→fail→0.5×retry) that the worklog characterizes in detail at line 1622–1636 is caused by growth periodically crossing the minimum viable dt. OPM's DSMAXDT/DPMAX post-step saturation limiting prevents this.

5. **Variable substitution for Undersaturated→Saturated** — medium effort, eliminates the regime-inconsistency problem at its root rather than patching the hysteresis band.

6. **Well Schur complement elimination** — large effort, but the industry-standard fix for well-cell coupling.

Items 1–4 are all documented, localized, and actionable. The worklog at lines 1665–1668 already lists this exact priority order. The remaining question is whether items 2–4 are already partially done (some timestep work is in place) or still need to land.