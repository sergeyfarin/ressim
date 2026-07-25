# Three-Phase Flow Implementation Notes

Status: implemented in Rust and the TypeScript frontend, and validated. The `experimental` label was removed on 2026-07-25 once the exit criteria in `docs/THREE_PHASE_VALIDATION.md` section 1 were met. This document covers architecture and parameters; grading, tolerances and measured baselines live in `docs/THREE_PHASE_VALIDATION.md`.

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
| Initial gas saturation per layer | `initialGasSaturationPerLayer` | — | — | fraction[] |
| Initial water saturation per layer | `initialSaturationPerLayer` | — | — | fraction[] |
| Cell thickness per layer | `cellDzPerLayer` | `dz` (Vec) | — | m[] |

All parameters are optional when two-phase mode is active. Defaults apply when `threePhaseModeEnabled = false`.

The per-layer arrays (`initialGasSaturationPerLayer`, `initialSaturationPerLayer`, `cellDzPerLayer`) override their scalar equivalents when present. They must have length equal to `nz`. These support scenarios like SPE1 where a gas cap occupies the top layer with different thickness and saturation from the oil zone below.

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

Three-phase mode is **validated**, against numerical comparative solutions rather than
closed-form ones. The authoritative record — exit criteria, cases, tolerances, measured errors,
recorded baseline and replay commands — is `docs/THREE_PHASE_VALIDATION.md`. In summary:

- **Gas injection** is graded against SPE1 Case 1 / `flow 2026.04` (`docs/BLACK_OIL_VALIDATION.md` §1).
- **Solution gas drive** is graded against an OPM Flow reference for the `gas_drive` scenario,
  whose PVT table, SCAL curves, grid and wells are shared with the deck by construction.
- **Gas-front behavior** — breakthrough timing inside an acceptance band and stable under
  timestep refinement, saturation profile monotone in space and advancing in time.
- **All three phases close explicitly**, oil included, each inside a 1 % drift tolerance.
- Stone II reductions, endpoints and SCAL table interpolation are unit-tested in
  `src/lib/ressim/src/tests/three_phase.rs`.

### Material-balance diagnostics: what is and is not reported

An earlier version of this document said oil-phase closure was "indirect" and not reported as
its own diagnostic. That was wrong, and the correction matters:

- `material_balance_error_m3` (water) and `material_balance_error_gas_m3` (gas, free plus
  dissolved) are explicit cumulative diagnostics.
- `material_balance_error_oil_m3` (oil) is **also** explicit and direct: reported surface oil
  production versus actual stock-tank oil inventory depletion. It is not backed out as a
  residual.

What *is* residual about oil is its **saturation**: transport solves water and gas and sets
S_o = 1 - S_w - S_g. The constraint therefore cannot fail — it is enforced by construction — so
the oil diagnostic grades the reporting and FVF path rather than the constraint. See
`docs/THREE_PHASE_VALIDATION.md` section 4.

## Remaining Gaps

These are envelope limits, not validation debt:

- **No three-phase analytical reference** exists in this repo; Buckley-Leverett is two-phase and
  Dietz is oil-only. Three-phase grading is numerical.
- **Vaporized oil (Rv) is not modelled** — the gas phase carries no oil, so wet-gas and
  gas-condensate behavior is outside the envelope. The OPM decks use dry-gas `PVDG` to match.
- **`gas_injection` has no OPM reference of its own**; it is covered by SPE1 (same mechanism)
  and the gas-front criteria.
- **A +4 % cumulative-oil bias** against OPM on `gas_drive` is inside the acceptance band but
  unexplained.
