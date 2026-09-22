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

**No physics gate failed.** The only failures were the two tests that assert the routing itself:
`default_fim_solver_uses_iterative_fallback_before_sparse_lu` and
`gmsres_ilu0_backend_solves_simple_system_iteratively` (the latter fails because wasm's
`should_force_direct_solve` forces a direct solve for a `GmresIlu0` request, which native's does
not — a third behavioural difference, currently invisible because tests never run on wasm).

That asymmetry is the practical argument for a direction. Unifying **onto dense** (the wasm path)
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

## 8. Recommendation

1. **Do not change the routing as part of an unrelated commit.** It is a production solver change
   and belongs in its own, with the validation shortlist rerun on the final tree per the repo's
   promotion discipline.
2. **Unify on the dense/wasm path**, on the evidence above: it is the direction that improves
   convergence rather than degrading the shipped product, and the tests it breaks assert routing
   rather than physics. Update those two tests to state the unified expectation, and delete the
   `is_wasm` parameter threading once nothing varies by target.
3. **Fix `sparse_lu_debug.rs`'s module documentation regardless** of what is decided — it
   currently tells the reader the opposite of what the code does.
4. **Re-baseline `FIM_STATUS.md` after unification**, since its figures were measured on the wasm
   path and would then apply to both.
5. **Then re-open item 7**: with the dominant cause removed, the residual 1.24e-05 becomes
   measurable on its own and the libm/SIMD hypothesis can finally be tested rather than assumed.
6. Keep the two FIM cases in the parity matrix non-strict until 2 lands; make them strict as the
   change's acceptance criterion.
