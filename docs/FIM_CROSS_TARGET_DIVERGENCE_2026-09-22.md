# FIM cross-target divergence — root cause

> **Superseded by §9 (FIM-DIRECT-001, 2026-09-22).** The backend split in §§1–3 is real, but
> it was the *trigger*, not the defect. Both LU backends are correct. The defect was in shared
> property evaluation: roundoff of one sign in the inactive two-phase gas unknown made every later
> Jacobian in the step **exactly singular**, so both backends had to reject it. Which backend's
> roundoff came out negative decided which target fragmented. §7's "second cause" was the same
> defect, and §8's backend trade no longer exists. After the fix, native (sparse) and wasm (dense)
> agree to 5.7e-14 on every parity case, with the routing unchanged.

Date: 2026-09-22. Base: `a708280`, clean tree. Investigation of the wasm32/x86-64 FIM difference
recorded in `OPEN_ITEMS_2026-09-21.md` §1.

**Root cause found, and the hypothesis recorded on 2026-09-21 was wrong.** That note guessed at
SIMD dispatch in the linear stack amplified by convergence branching. It is not that. The cause is
an explicit `cfg(target_arch)` split that makes the two targets run **different linear solver
backends** in production.

## 1. What the trace shows

Both targets were stepped through the committed Buckley case A fixture with
`step_with_diagnostics`, which the browser has always had and which is now bound for Python too.
Diffing the two traces, they part company at **iteration 0**, before any accumulated rounding
could exist:

| Newton iteration | wasm32 | x86-64 native |
|---|---|---|
| 0 | `used=dense-lu` | `used=sparse-lu` |
| 1 | `used=dense-lu` | `used=fgmres-cpr` (direct solve failed, fell back) |

The residual at iteration 0 is *identical* on both (`res=2.243e1`). What differs is which solver
is asked to produce the correction.

## 2. The code

`fim/linear/mod.rs::solve_linearized_system_with_routing` — small systems solve directly, and the
backend is chosen by target:

```rust
#[cfg(not(target_arch = "wasm32"))]
if allow_forced_direct && should_force_direct_solve(options.kind, jacobian.rows(), false) {
    let direct = sparse_lu_debug::solve(jacobian, rhs, options, false);   // native
...
#[cfg(target_arch = "wasm32")]
if allow_forced_direct && should_force_direct_solve(options.kind, jacobian.rows(), true) {
    let direct = dense_lu_debug::solve(jacobian, rhs, options, false);    // wasm
```

A second site does the same for the fallback kind, `fim/newton.rs::direct_fallback_kind_for_rows`:
native always returns `SparseLuDebug`; wasm returns `DenseLuDebug` below the row threshold.

`should_force_direct_solve` also branches on `is_wasm`, but for a requested `FgmresCpr` both
branches agree, so it is not part of this divergence. The row thresholds are **equal**
(`WASM_DIRECT_SOLVE_ROW_THRESHOLD == DIRECT_SOLVE_ROW_THRESHOLD == 512`), so the threshold is not
part of it either. Only the backend differs.

## 3. Proof, by experiment rather than by argument

Native was patched to take the wasm branch at both sites, rebuilt, and re-measured on the same
fixture:

| Measure | Native, as shipped | Native on the wasm backend path | wasm32 |
|---|---|---|---|
| Substeps, first 1-day step | 18 | **4** | 4 |
| Max pressure difference vs wasm | 8.282e+00 bar | **5.684e-14** | — |
| `adverse-mobility-fim`, 3 steps | 1.733e+00 bar | **1.24e-05** | — |

The divergence collapses from bars to floating-point noise. Nothing else was changed.

Three alternatives were ruled out beforehand, each by experiment:

- **Nondeterminism** — native gives byte-identical results across four runs in one process and
  three separate processes, so `HashMap` seeding and similar are not involved.
- **Build profile** — a `--release` native build gives the same 18 substeps as debug.
- **The `wasm` cargo feature added in S4** — a native build *with* the feature also gives 18. S4
  did not cause this.
- **Configuration drift between the two runners** — they had used `add_well` and `addWellWithId`,
  which select different `WellGroupingKey` variants. Binding `add_well_with_id` for Python so both
  configure identically changed nothing.

## 4. Scope: FIM only, and this is structural rather than lucky

`target_arch` sites outside tests, by area:

| Area | Sites |
|---|---|
| `impes/` | **0** |
| `compositional/` | **0** |
| `fluid/` | **0** |
| `fim/` | 45 |

IMPES does not route through `fim/linear` at all — it has its own solver — which is why it agrees
to 1e-12 on the same fixture and targets. The compositional model uses its own dense Gaussian
elimination in `compositional/newton.rs` with no target branching, so it is target-stable by
construction.

Of FIM's 45 sites, most are native-only tracing. Triaging the paired ones (those with both a wasm
and a non-wasm arm, which are the only ones that can diverge):

| Site | Diverges in a default run? |
|---|---|
| `linear/mod.rs` forced-direct backend | **Yes — this is the cause** |
| `newton.rs::direct_fallback_kind_for_rows` | **Yes** |
| `linear/mod.rs` row threshold | No — the two constants are equal |
| `newton.rs` trace macro | No — `eprintln!` only |
| `newton.rs` `y2b3_primary_variable_lifecycle` | No — env-gated, off unless `FIM_Y2B_RAW_SATURATION` is set |
| `timestep.rs` `max_substeps` | No — env override, same constant when unset |
| `newton.rs` `use_flow_lifecycle` | No — defaults false |

## 5. Is it a mistake?

The evidence points that way, but not unambiguously, and the ambiguity is worth stating.

**For "mistake":**

- `sparse_lu_debug.rs`'s own module documentation says *"This is intentionally diagnostic-only.
  The production FIM route does not select this backend, and this status must not be used as a
  nonlinear-convergence verdict."* On native, the production route **does** select it. The code
  and its documentation contradict each other.
- Both backends are named `*_debug`, and both are used in production, on different targets.
- The commit that introduced the wasm arm (`449308d`) says only *"Add Dense LU Debug solver and
  update FIM options for WASM compatibility"*. No measured rationale is recorded, and
  `sparse_lu_debug` compiles for wasm perfectly well — the split is not a capability constraint.
- Nothing anywhere states that the two targets are expected to produce different solver
  trajectories.

**Against "mistake":**

- The native routing is deliberately asserted by
  `fim::linear::tests::default_fim_solver_uses_iterative_fallback_before_sparse_lu`, which checks
  `backend_used == SparseLuDebug`. Somebody meant it, at least at the time.
- The surrounding comment describes the direct-then-iterative fallback as *"load-bearing, not
  generic defensive code: it is what keeps the OpmAligned default (WATER-026) converging on small
  well-dominated cases."*

**The sharpest observation is about coverage, not intent.** Those routing tests are native-only —
the whole Rust suite is. So the native routing is test-locked while **the wasm routing that
actually ships to users has no unit coverage at all**, and the convergence baselines in
`FIM_STATUS.md` were measured on the untested path while the tested path is the one nobody
measures. The two have been drifting in opposite directions with nothing watching.

## 6. Would fixing it degrade anything?

Tested directly, with native patched onto the wasm path:

| Gate | Result under the experiment |
|---|---|
| `benchmark_buckley` | 3 passed |
| `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas` | 1 passed |
| `spe1_fim_first_steps_converge_without_stall` | 1 passed |
| `spe1_fim_gas_injection_creates_free_gas` | 1 passed |
| `fim::tests::repair_lifecycle` | 5 passed |
| `physics_depletion_grid_convergence_impes` | 1 passed |
| `fim::linear::tests` | **2 failed**, both routing assertions |

> **Incomplete, and corrected in §8.** This table lists the gates run at the time. It does not
> include the `#[ignore]`d release replays, which are not part of `validate:full` and had to be
> run explicitly. One of them, `physics_depletion_grid_convergence_fim`, **does** fail under the
> change. "No physics gate failed" was true of what had been run and false of the change, which is
> the difference between a gate set and a claim.

Of the gates above, no physics gate failed. The only failures were the two tests that assert the
routing itself:
`default_fim_solver_uses_iterative_fallback_before_sparse_lu` and
`gmsres_ilu0_backend_solves_simple_system_iteratively` (the latter fails because wasm's
`should_force_direct_solve` forces a direct solve for a `GmresIlu0` request, which native's does
not — a third behavioural difference, currently invisible because tests never run on wasm).

That asymmetry looked like the practical argument for a direction, and §8 records why it was not.
Unifying **onto dense** (the wasm path)
improved convergence on the fixture, 18 substeps to 4, and broke only assertions about routing.
Unifying **onto sparse** would make the browser behave like native — 4 substeps to 18 — which is a
user-facing regression in the one place the simulator actually runs for people.

## 7. There is a second, smaller cause still unidentified — *superseded: same defect, see §9*

Unifying the backend did **not** reduce every case to noise. On `adverse-mobility-fim` the
difference fell from 1.733e+00 to **1.24e-05** — four orders of magnitude better, and still far
above the 5.7e-14 the easier case reaches.

So this is at least two effects, which is why a single fix should not be declared a resolution.
The residual is consistent with genuine cross-target floating-point differences (libm `powf` in
the Corey relperm, or SIMD summation order in the iterative stack) amplified by a controller that
still branches on tolerances — the mechanism originally guessed at, now demoted from primary cause
to plausible secondary. It has not been confirmed.

## 8. Recommendation — **revised 2026-09-22 after attempting it** — *superseded by §9*

> **The recommendation first written here was wrong, and the attempt is what showed it.** It said
> to unify on dense because that direction "improved convergence, 18 substeps to 4". Substep count
> is **speed**, not accuracy. The only gate that measures accuracy on the systems this path
> actually touches prefers the other backend. The attempt is preserved on branch
> `fim/unify-linear-routing` (`f5a9577`); it is not on master.

### What the attempt established

Unifying on dense does fix the divergence: `buckley-fim` falls from 8.282e+00 bar to **5.68e-14**,
`adverse-mobility-fim` from 1.733e+00 to **1.24e-05**, and the parity matrix gains a strict FIM
case. 38 solver gates, the three locked FIM baselines, `benchmark_buckley`, SPE1 full horizon and
SPE1 areal refinement all pass, and the wasm `.d.ts` is unchanged.

**But it breaks `physics_depletion_grid_convergence_fim`**: average pressure stops contracting
under refinement, 0.941 of the previous difference against a required 0.8 (it was 0.631). The
converged nx=40 answer barely moves — 122.2069 against 122.2055 — so the final answer is not lost;
the coarse-grid path shifts enough to fail the criterion. Isolated to the backend swap itself:
reverting only the retry-fallback kind reproduces the failure identically.

No tolerance was touched. `CONTRACTION_RATIO` is a benchmark tolerance, and "my change would
otherwise fail" is not the written justification the repo requires.

### Why OPM cannot arbitrate this

The forced-direct path fires only at **≤ 512 rows**. Measured:

| Case | Cells | Rows | Exercises the path? |
|---|---|---|---|
| OPM `gas-rate-10x10x3` | 300 | ~900 | **No** |
| SPE1 | 300 | ~900 | **No** |
| `physics_depletion_grid_convergence_fim`, nx=5…40 | 5–40 | 15–120 | Yes |
| Parity `buckley` | 50 | 154 | Yes |

So the OPM comparison was run on both backends and returned **identical** results — 1 accepted
substep, 0 retries, matching OPM Flow's 1 substep per 0.25-day report step — because the case never
reaches the divergent code. SPE1 full horizon was likewise **bit-identical** before and after
(worst case pressure 1.597%, oil rate 3.158%, GOR 4.314%). That is reassuring about blast radius
and is *not* evidence for either backend.

### The actual trade

| | Dense everywhere | Sparse everywhere |
|---|---|---|
| Cross-target parity | Achieved (5.7e-14) | Achieved |
| `physics_depletion_grid_convergence_fim` | **Fails** (0.941 vs 0.8) | Passes |
| Small-case browser cost | 4 substeps | **18 substeps** on the same case |
| Native behaviour | Changes | **Unchanged** |
| OPM / SPE1 | Unaffected either way | Unaffected either way |

Dense is faster on the small cases the browser runs. Sparse is what the one refinement-accuracy
gate covering those sizes endorses. Both remove the divergence; neither is free.

### What to do

1. **Decide the backend deliberately — this is a physics-accuracy call, not a cleanup**, and it is
   the reason nothing landed on master. The repo's own rule that benchmark tolerances are not moved
   to accommodate a change is what makes this a decision rather than a patch.
2. **On the evidence, sparse is the defensible default**: it is the only option that leaves every
   committed gate green, it requires no change to native behaviour, and the cost is browser speed
   on small cases rather than accuracy. Dense is defensible only if someone first shows the
   refinement failure is an artifact of the criterion rather than of the backend — which would mean
   investigating `physics_depletion_grid_convergence_fim` on its merits, not around it.
3. **Either way, take the rest of `f5a9577`**: one code path, one threshold, no `is_wasm`
   threading, and an explicitly requested iterative backend honoured rather than overridden — which
   removes the re-entry hazard the old wasm arm needed a guard for. Landing it on sparse is a
   one-line change to that branch.
4. **Re-baseline `FIM_STATUS.md` afterwards**, since its figures were measured on the dense/wasm
   path and would then apply to both targets.
5. **Then re-open the residual**: 1.24e-05 remains on `adverse-mobility-fim` even with the backend
   unified, so this was always at least two effects. With the dominant one settled, the libm/SIMD
   hypothesis becomes measurable on its own instead of assumed.
6. Keep `adverse-mobility-fim` non-strict until that residual is closed; `buckley-fim` becomes
   strict the moment the routing is unified, whichever backend wins.

## 9. The actual root cause: an inactive unknown whose slope depended on the sign of roundoff

*FIM-DIRECT-001, 2026-09-22. Base `916a124`, native release. This supersedes §§7–8 and the
backend recommendation in the
[dense/sparse review plan](FIM_DENSE_SPARSE_REVIEW_PLAN_2026-09-22.md), whose S0/S1 this answers.*

### 9.1 The experiment that decided it

Both LU backends were run on **the same matrix** at every forced-direct solve of the parity case
`buckley-fim` (50 cells, one 1-day step, 18 native substeps), with an equilibrated SVD of each
matrix. This used a temporary, uncommitted probe in `solve_linearized_system_with_routing`.

| Solves | Matrix | Sparse LU | Dense LU | ‖dx_sparse − dx_dense‖∞ |
|---|---|---|---|---|
| Newton iter 0 | non-singular, σmin/σmax 4.6e-4 | converged, res 7e-13 | converged, res 1e-12 | 5.5e-12 on ‖dx‖ 7e3 |
| every later solve in the step | **exactly singular**: one σ = 0, dense U has an exact 0 pivot | factorization refused | `lu().solve()` → `None` | both rejected |
| non-singular solves later in the run | σmin/σmax ~4e-4 | converged | converged | ≤ 4e-14 |

**Neither LU is wrong.** Whenever the matrix is non-singular they agree to roundoff; whenever it
is singular both refuse it. Every null vector was supported on local variable 2, the third cell
unknown. In a two-phase run that unknown (`hydrocarbon_var`) is **inactive**: no gas is present,
and it is meant to stay at 0.

### 9.2 Mechanism

`fim/properties.rs::cell_props_generic`, two-phase / no-PVT branch:

```rust
let sg = hydrocarbon_var.max_floor(0.0).min_of(total_hc);
```

`max_floor` is branch-selecting: at `hc >= 0` it keeps the variable's slope, below 0 it returns a
constant. The first direct solve does not return `dhc = 0` exactly. It returns roundoff: here
**−1.1e-14** in the producer cell. From then on that cell's gas row and column are identically
zero, the Jacobian is exactly singular, and the Newton correction can never move `hc` back, since
the column is empty. So the whole step runs on the fallback ladder. Dense LU on wasm happened to
produce roundoff of the other sign on this case; on other cases it does not, which is why dense
also failed gates.

The comment directly above the line already stated the intent: "the third unknown must keep LIVE
derivatives… The legacy assembler regularizes the same way". The legacy assembler (now
`#[cfg(test)]`) uses `d_sg/d_hc = 1` **for any sign**. The AD port kept the value and lost that
property. The existing gate `two_phase_singularity_check` evaluates at exactly `hc = 0`, where
`>=` keeps the slope, so it could not see this.

### 9.3 Fix

The value stays clamped (residuals unchanged); the slope is the legacy chain rule's,
`d_sg/d_hc = 1`, whatever the sign:

```rust
let sg_value = hydrocarbon_var.max_floor(0.0).min_of(total_hc).value();
let sg = hydrocarbon_var + S::from_f64(sg_value - hydrocarbon_var.value());
```

Two tests pin it, and both were checked against the old line (each fails there):

- `fim::properties::tests::two_phase_inactive_unknown_keeps_its_slope_at_negative_roundoff`: the unit contract
  at `hc ∈ {−1.1e-14, 0, +1.1e-14}`;
- `tests::buckley::fim_two_phase_parity_step_does_not_fragment`: the parity case takes ≤ 4
  substeps (old code: 18).

No path with a PVT table changes: those branches already keep the tagged primary raw, with no clamp. The edited branch also serves three-phase runs *without* a PVT table, where `hc` is a real Sg. There, below zero, it now keeps the same unit slope the PVT Saturated arm keeps, while the value stays clamped.

### 9.4 Results

Same case definitions as the earlier sparse/dense table (Buckley-style, 2 days, dt = 1 day,
native release). "Dense" = the temporary probe routing the top-level forced-direct solve to dense
LU. Timings are single observations on this machine, not baselines.

| Case | Rows | Substeps, sparse before → after | Substeps, dense after | Time, sparse after | Time, dense after |
|---|---|---|---|---|---|
| 1d-50, 1 day (parity) | 154 | 18 → **4** | 4 | 7.5 ms | 20 ms |
| 1d-50, 2 days | 154 | 19 → **5** | 5 | 9.4 ms | 26 ms |
| 1d-160, 2 days | 484 | 4,697 → **5** | 5 | 15 ms | 278 ms |
| 2d-12×12, 2 days | 436 | 9,021 → **6** | 7 | 62 ms | 627 ms |

Accuracy on 2d-12×12, scored against a dt = 0.02-day run (the fine limit):

| | Sparse | Dense |
|---|---|---|
| Fine limit, sparse vs dense | 3.1e-5 bar, 2.2e-7 Sw | — |
| dt = 1 error vs fine, max \|Δp\| / \|ΔSw\| | 12.15 bar / 0.081 | 12.73 bar / 0.082 |
| dt = 0.1 error vs fine | 3.09 bar / 0.018 | 3.09 bar / 0.018 (identical) |
| dt = 1, sparse vs dense | 0.66 bar / 0.006 | |

Per-solve corrections on 12×12 agree to ≤ 2.5e-10 on ‖dx‖ ~ 1e3. The 6-vs-7 split is roundoff
crossing an adaptive-controller threshold, and the resulting difference is ~5% of either
backend's time-discretization error. **There is no accuracy winner; sparse is 4–18× faster.**

Cross-target, via `bash scripts/validate-native-binding.sh` on this tree, with the
`cfg(target_arch)` routing **still in place**:

| Case | Before | After |
|---|---|---|
| `buckley-fim` | 8.28 bar, 18 vs 4 substeps | **5.68e-14**, rates 1.1e-13 |
| `adverse-mobility-fim` | 1.73 bar (1.24e-05 with dense unified) | **5.68e-14**, rates 2.1e-13 |

Both cases are now `strict` in `cases.json`. The libm/SIMD hypothesis of §7 is **refuted** for
this matrix: nothing else remained once the singular Jacobians were gone.

Gates on the fixed tree: `bash scripts/validate-solver-coverage.sh all` exit 0; `tests::buckley`,
`assembly_ad` 13, `fim::properties` 8, `fim_repair_` 6, `report_contract` 7, `fim::flux` 7,
`fim::state` 13 all pass.

### 9.5 What this does *not* fix: bubble-point fragmentation (a separate defect)

`physics_depletion_grid_convergence_fim` (three-phase, PVT table) is untouched by the fix, and
still passes on sparse and fails on dense exactly as §8 recorded. Neither result means anything
about the backends, because **the run itself is pathological on both**:

- A per-solve census found **zero** singular or rejected solves at every grid on either
  backend, yet 218,000–280,000 linear solves per grid on sparse and 195,000–283,000 on dense, for
  a 1-D, 100-day depletion.
- At nx = 10 the first 5-day report step, which crosses the bubble point, takes **21,797
  substeps**; every later step takes 1.
- The substeps form a sawtooth: dt ≈ 3e-5 to 1.5e-4 day, alternating between a 20-iteration
  accept via the final-iteration relaxed tier (growth ×0.4) and a 3–5 iteration accept (×2.6).
  Inside the capped ones, one cell's pressure ping-pongs (149.695 ↔ 149.723 bar) with the oil
  mass-balance residual bouncing between 2e-7 and 3e-6.
- 89% of hotspot iterations are in a cell tagged **Saturated holding a negative raw Sg**
  (typically −3e-4). Default `OpmAligned` keeps saturations raw (`opm_raw_saturation`, WATER-025),
  but the OPM Sg↔Rs primary-variable switch that goes with it (`FIM_Y2B_RAW_SATURATION`, Y2b3)
  is off by default, so redissolved gas never switches the cell to Rs.
- Neither flag alone repairs it. With the full Y2b3 lifecycle: 27,959 substeps. With the hard
  clamp restored (`FIM_W025_DISABLE_RAW_SW`): 36,562. Both also leave the pressure stuck near the
  bubble point after step 0: pmin 141.5 and 150.0 bar, against 124.5 on the default.

So the grid-convergence gate currently measures the outcome of a chaotic ~20k-substep bubble-point
crossing. Roundoff-level differences, such as the backend, move its coarse-grid values enough to
flip a 0.8 contraction check. That is why it could not arbitrate §8, and why the dt=1.0 variant
failed on both. This is the coupled-semantics case the review plan's S2/S4 are written for:
raw primaries, primary-variable adaptation and accept tiers interact, and each flag alone makes it
worse. It is recorded as an open item rather than patched here.

### 9.6 What follows

1. **Done (2026-09-22): routing unified.** Every target tries sparse LU, then dense LU as a backup,
   then CPR. `setFimDirectBackend` swaps the order at runtime. Cross-checked against OPM Flow on
   five decks under 512 rows ([`small-direct`](../opm/reference-decks/small-direct/README.md)):
   oil–water within 1 bar, 0.03 Sw and 0.8% on cumulatives, with Flow's substep counts.
2. **Bubble-point fragmentation (§9.5)** is the real remaining FIM defect on small cases. It needs
   the coupled investigation, not a flag flip.
3. `FIM_STATUS.md` wasm baselines: the two-phase ones may now move natively. Re-measure before
   citing any of them as cross-target.

