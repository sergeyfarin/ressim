---
name: engine-physics-change
description: Safely modify the Rust/WASM reservoir simulation core (relperm, PVT, capillary, wells, IMPES/FIM solvers, grid, reporting). Use when adding or changing physics, editing anything under src/lib/ressim/src/, or exposing new simulator API to the frontend.
---

# Changing the Rust Engine Safely

The engine's value is that its physics is *validated*. Every change must preserve that. This skill covers conventions, ownership boundaries, and the dual-implementation trap.

## Unit system (memorize before touching any formula)

Consistent metric-oilfield units everywhere (`docs/UNIT_SYSTEM.md`, `docs/UNIT_REFERENCE.md`):

| Quantity | Unit |
|---|---|
| Pressure | bar |
| Length | m |
| Time | day |
| Volume / rate | m³, m³/day |
| Permeability | mD |
| Viscosity | cP |
| Compressibility | 1/bar |

Transmissibility: `T = 8.5269888e-3 × k[mD] × A[m²] × λ[1/cP] / L[m]` in m³/day/bar — the constant is `DARCY_METRIC_FACTOR` in `step.rs`. Derivation: `docs/TRANSMISSIBILITY_FACTOR.md`. Any `0.001127`-style field-unit constant you see referenced in old text is stale — do not reintroduce it.

## Module map

- Shared physics (used by BOTH solvers): `relperm.rs`, `pvt.rs`, `mobility.rs`, `capillary.rs`, `well.rs`, `well_control.rs`, `grid.rs`, `reporting.rs`.
- `step.rs` — thin dispatcher (IMPES vs FIM) + shared gas-split helpers. Keep it thin.
- `impes/` — product solver: `pressure.rs` (PCG), `transport.rs` (explicit saturation), `timestep.rs`.
- `fim/` — dev-only fully implicit solver (see `fim-solver-debug` skill before touching).
- `frontend.rs` + `lib.rs` — wasm-bindgen public API consumed by `src/lib/workers/sim.worker.ts`.

## The dual-implementation trap (most important)

Two places where one physical formula lives in **two implementations that must agree**:

1. **Legacy vs AD assembly in FIM.** The live FIM assembly is `fim/assembly_ad.rs` (AD-based); `fim/assembly.rs` is the legacy reference. Shared physics helpers (`relperm.rs`, `pvt.rs`, `mobility.rs`, `capillary.rs`) have AD twins (generic or `*_ad` functions). If you change a formula, change both and run the bit-parity gates:
   ```bash
   cargo test --manifest-path src/lib/ressim/Cargo.toml assembly_ad -- --nocapture
   ```
2. **IMPES vs FIM public behavior.** Shared contracts are enforced by `*_on_both_solvers` tests in `src/lib/ressim/src/tests/`. A physics change that moves one solver but not the other will fail these — that is the test doing its job.

Also duplicated across languages: the undersaturated `c_o = 1e-5 /bar` assumption exists in both `src/lib/physics/pvt.ts` and `src/lib/analytical/materialBalance.ts` (known gap, no regression guard yet). TS-side analytical formulas mirror Rust physics — check both sides when changing fractional-flow or PVT behavior.

## Test placement rules (from `src/lib/ressim/src/tests/README.md`)

| Test kind | Location |
|---|---|
| Physics outcomes, API contracts, stable-across-refactors behavior | `src/tests/` (`physics/`, `runtime_api.rs`, `geometry_api.rs`, …) |
| FIM-internal / Newton / FIM smoke | `src/fim/tests/` |
| IMPES-internal pressure/transport/timestep | `src/impes/tests/` |
| White-box private-helper invariants (exact Jacobian entries, etc.) | module-adjacent `*_tests.rs` (e.g. `fim/assembly_tests.rs`) |

Naming: cross-solver contracts are `*_on_both_solvers`; physics regressions are `physics_<family>_*`.

## Non-negotiables

- **Do not weaken benchmark tolerances** (`benchmark_buckley_*` in `lib.rs`, methodology in `docs/P4_TWO_PHASE_BENCHMARKS.md`) without written justification.
- No `unwrap()` in library code; explicit error handling.
- `///` doc comments on public API; `cargo fmt` before committing.
- New physics needs an oracle: an analytical solution, a conservation/invariant check, or a finite-difference Jacobian check — not just "runs without crashing". Existing patterns: FD Jacobian acceptance tests in `fim/assembly.rs`, Peaceman connection-law oracle in `tests/physics/wells_sources.rs`, closed-system inventory checks.
- Material-balance caveat: water and gas closure are explicit; **oil is the residual phase** in diagnostics. Don't claim oil MB validation that isn't there.

## Exposing new API to the frontend

1. Add the method/field in `frontend.rs` (wasm-bindgen).
2. Rebuild: `bash scripts/build-wasm.sh` (regenerates `src/lib/ressim/pkg/`, which is committed).
3. Wire through `src/lib/workers/sim.worker.ts` — worker messages must be **structured-cloneable** (plain objects/arrays only, no functions or class instances).
4. Update `src/lib/simulator-types.ts` / `buildCreatePayload.ts` as needed; `pnpm run typecheck`.

## Validation

Follow the `ressim-validation` skill decision table. Minimum for shared-physics changes: `bash scripts/validate-solver-coverage.sh all` + `cargo test --manifest-path src/lib/ressim/Cargo.toml benchmark_buckley` + parity gates. Never gate on full `cargo test`.
