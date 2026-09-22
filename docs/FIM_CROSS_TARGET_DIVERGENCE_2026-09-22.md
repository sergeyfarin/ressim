# FIM cross-target divergence — root cause

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

## 7. There is a second, smaller cause still unidentified

Unifying the backend did **not** reduce every case to noise. On `adverse-mobility-fim` the
difference fell from 1.733e+00 to **1.24e-05** — four orders of magnitude better, and still far
above the 5.7e-14 the easier case reaches.

So this is at least two effects, which is why a single fix should not be declared a resolution.
The residual is consistent with genuine cross-target floating-point differences (libm `powf` in
the Corey relperm, or SIMD summation order in the iterative stack) amplified by a controller that
still branches on tolerances — the mechanism originally guessed at, now demoted from primary cause
to plausible secondary. It has not been confirmed.

## 8. Recommendation — **revised 2026-09-22 after attempting it**

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
