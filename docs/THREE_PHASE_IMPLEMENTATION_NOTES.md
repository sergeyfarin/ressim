# Three-Phase Flow Implementation Notes

Status: implemented in Rust and the TypeScript frontend. Still experimental because comparative-solution and acceptance-test coverage remain incomplete.

This document describes the architecture decisions made when adding oil/water/gas three-phase simulation. The existing two-phase code path is unchanged; three-phase is purely additive and activated by the `threePhaseModeEnabled` flag.

---

## Architecture Decisions

### Relative Permeability — Stone II

- **k_rw(S_w)**: Corey, function of S_w only (same form as two-phase)
- **k_rg(S_g)**: Corey, function of S_g only
- **k_ro(S_w, S_g)**: Stone II model:
  ```
  k_ro = k_ro_max · [(k_ro_w/k_ro_max + k_rw) · (k_ro_g/k_ro_max + k_rg) − k_rw − k_rg]
  ```
  Clamped to [0, k_ro_max]. Where k_ro_w = k_ro(S_w, 0) and k_ro_g = k_ro(0, S_g) are two-phase endpoints.

### Capillary Pressure

- Existing oil-water curve P_cow(S_w) — unchanged
- New oil-gas curve P_cog(S_g) — same Brooks-Corey form, own entry pressure + lambda
  - Phase pressures: P_water = P_oil − P_cow; P_gas = P_oil + P_cog

### Injected Phase

- New parameter `injectedFluid: "water" | "gas"` controls what the injector injects
- Default for three-phase mode: gas injection

### Pressure Equation

IMPES stays as one pressure unknown per cell (oil pressure as reference). Gas phase potential:
```
dphi_g = (P_oil_i − P_oil_j) + (P_cog_i − P_cog_j) − grav_g
```
Total mobility: λ_t = λ_w + λ_o + λ_g

Accumulation term expands to:
```
c_t = ϕ · (c_o · S_o + c_w · S_w + c_g · S_g) + c_r
```

### Saturation Update

Two explicit transport equations solved after pressure:
- Δv_water (from water fluxes)
- Δv_gas (from gas fluxes)
- S_o_new = 1 − S_w_new − S_g_new (enforced by material balance)
- All three saturations clamped and re-normalized if round-off causes sum ≠ 1

### CFL Check

Gas saturation change criterion added alongside existing water and pressure checks.

---

## New Parameter Reference

| Parameter | TS field | Rust field | Default | Units |
|-----------|----------|-----------|---------|-------|
| Critical gas saturation | `s_gc` | `s_gc` | 0.05 | fraction |
| Residual gas saturation | `s_gr` | `s_gr` | 0.05 | fraction |
| Gas Corey exponent | `n_g` | `n_g` | 1.5 | — |
| Max gas relative permeability | `k_rg_max` | `k_rg_max` | 1.0 | fraction |
| Gas viscosity | `mu_g` | `mu_g` | 0.02 | cP |
| Gas compressibility | `c_g` | `c_g` | 1e-4 | 1/bar |
| Gas density | `rho_g` | `rho_g` | 10.0 | kg/m³ |
| Gas-oil Pc entry pressure | `pcogPEntry` | `pc_og.p_entry` | 3.0 | bar |
| Gas-oil Pc lambda | `pcogLambda` | `pc_og.lambda` | 2.0 | — |
| Injected fluid | `injectedFluid` | `injected_fluid` | `"gas"` | — |
| Three-phase mode flag | `threePhaseModeEnabled` | `three_phase_mode` | false | — |
| Initial gas saturation | `initialGasSaturation` | — | 0.0 | fraction |

All parameters are optional when two-phase mode is active. Defaults apply when `threePhaseModeEnabled = false`.

---

## Files Changed

| Layer | Files |
|-------|-------|
| Rust | `relperm.rs` (Stone II struct), `capillary.rs` (gas-oil Pc), `lib.rs` (new WASM methods, sat_gas field), `step.rs` (pressure assembly, gas transport, CFL, sat update) |
| TS types | `simulator-types.ts` (payload + GridState), `modePanelTypes.ts` |
| TS pipeline | `buildCreatePayload.ts`, `sim.worker.ts` |
| Store | `simulationStore.svelte.ts` (new `$state` fields) |
| Catalog | `scenarios.ts` (3-phase scenario definitions) |
| UI | `GasFluidSection.svelte` (new), `RelativeCapillarySection.svelte` (gas row), `ScenarioSectionsPanel.svelte`, `ScenarioPicker.svelte` |

---

## Unchanged Files

The following are explicitly not modified by three-phase:

- `src/lib/analytical/fractionalFlow.ts` — two-phase waterflood analytical, unchanged
- `src/lib/analytical/depletionAnalytical.ts` — two-phase depletion analytical, unchanged
- All existing two-phase scenarios
- Existing `RockFluidProps` struct in Rust (two-phase)
- Existing `CapillaryPressure` struct in Rust (P_cow)
- Existing `setRelPermProps` WASM method

---

## Validation Status

Three-phase mode is **experimental**.

What is already covered:

- Stone II reductions and endpoint behavior are unit-testable.
- Gas injection / gas saturation behavior is exercised in Rust tests.
- Water and gas cumulative material-balance errors are reported explicitly at runtime.

What is still missing:

- No comparative-solution benchmark suite comparable to the Buckley-Leverett BL-Case-A/B coverage exists yet for three-phase behavior.
- Oil-phase closure is still indirect because oil remains the residual phase in current diagnostics.
- Gas-oriented scenario acceptance criteria have not yet been formalized.

## Remaining Gaps

- **Validation gap**: there is still no accepted comparative-solution benchmark set for three-phase behavior in this repository.
- **Diagnostics gap**: oil-phase material-balance closure is still not reported explicitly as its own cumulative diagnostic.
- **Product-status gap**: gas scenarios are implemented, but the documentation and acceptance policy still need a consistent definition of what qualifies as non-experimental.
