# ressim-py configuration coverage

What the native Python client (`crates/ressim-py`) can and cannot configure, against the browser
bindings (`src/lib/ressim/src/frontend.rs`). Audited 2026-10-23 for
[#64](https://github.com/sergeyfarin/ressim/issues/64); the 2026-09-21 counts in
`docs/OPEN_ITEMS_2026-09-21.md` §2 (29 of 85 functions) are historical.

## How coverage is enforced

The parity gate (`scripts/validate-native-binding.sh`) no longer relies on this list being right.
`catalogCases.ts` builds every black-oil run the app can start from the scenario catalog, through
the app's own `buildCreatePayloadForRun` and the worker's own `configureReservoirSimulator`, and
records the setter calls that makes. `run_wasm.mjs` replays them through the browser bindings and
`compare_native.py` through `ressim-py`. A setter the shim does not bind fails the gate by name, so
a new catalog configuration that Python cannot express is caught when the catalog changes, not at
the next audit.

## `ReservoirSimulator`: bound

Every method the worker's `configureReservoirSimulator` can call, whether or not a catalog case
uses it today, plus the solver-backend, restore and reporting methods bound earlier:

| Area | Methods |
|---|---|
| Solver | `set_fim_enabled`, `set_fim_direct_backend`, `set_stability_params` |
| Grid and rock | `set_cell_dimensions`, `set_cell_dimensions_per_layer`, `set_rock_properties`, `set_rock_reference_pressure`, `set_gravity_enabled` |
| Permeability | `set_permeability_per_layer`, `set_permeability_field`, `set_permeability_random`, `set_permeability_random_seeded` |
| Fluids and PVT | `set_fluid_properties`, `set_fluid_compressibilities`, `set_fluid_densities`, `set_pvt_table`, `set_gas_fluid_properties`, `set_gas_redissolution_enabled`, `set_injected_fluid` |
| Rel perm and capillarity | `set_rel_perm_props`, `set_capillary_params`, `set_three_phase_mode_enabled`, `set_three_phase_rel_perm_props`, `set_three_phase_scal_tables`, `set_gas_oil_capillary_params` |
| Initial state | `set_initial_pressure`, `set_initial_pressure_hydrostatic`, `set_initial_saturation`, `set_initial_saturation_per_layer`, `set_initial_rs`, `set_initial_gas_saturation`, `set_initial_gas_saturation_per_layer`, `load_state` |
| Wells | `add_well`, `add_well_with_id`, `set_well_schedule`, `set_well_datum`, `set_well_control_modes`, `set_rate_controlled_wells`, `set_injector_enabled`, `set_target_well_rates`, `set_target_well_surface_rates`, `set_well_bhp_limits`, `set_sweep_config` |
| Reading | `pressures`, `sat_water`, `sat_oil`, `sat_gas`, `rs`, `layer_thicknesses`, `dimensions`, `time_days`, `grid_state`, `wells`, `rate_history`, `rate_history_since`, `latest_rate_point`, `last_solver_warning`, `last_fim_step_stats`, `fim_trace`, `step`, `step_with_diagnostics` |

## Not bound, on purpose

| Surface | Why |
|---|---|
| `CompositionalSimulator` (whole class) | A separate engine. `comp_co2_1d` is the only catalog scenario on it; its native check is `scripts/validate-compositional.sh`. The catalog replay lists its runs as excluded. |
| `setFimOpmAlignedNonlinear`, `setFimNestedWellSolve`, `setFimTrueFgmres`, `setFimFlowLifecycle`, `setFimFlowResvInjector`, `setFimOpmEndpointRelperm`, `setFimCoreyTablePoints`, `setFimOpmWaterHeavySwof` | FIM experiment toggles. No catalog case and no worker path sets them; they are reached from `scripts/fim-wasm-diagnostic.mjs` and the Rust tests. Bind one when a native experiment needs it. |
| `getFimStepStatsHistory` | Deletion candidate with no live call site ([#24](https://github.com/sergeyfarin/ressim/issues/24)). |

## The catalog matrix

All 185 black-oil catalog runs (each scenario's base run, or every variant of every sensitivity
dimension) collapse to 135 distinct engine setups; variants that differ only in chart or
analytical settings configure identically and are replayed once. The 7 `comp_co2_1d` runs are
excluded as above.

Each setup is replayed for its first two steps, each at most 0.1 day (`CATALOG_STEP_CAP`,
`CATALOG_DT_CAP_DAYS` in `catalogCases.ts`). Configuration is never altered, only the horizon:
parity is a property of the binding, and the full horizon is what the scenario tests and the
cross-solver gate run. The 0.1-day cap exists because the IMPES sweep cases report every five
days and substep to their CFL limit, which cost minutes of wall time while proving no more about the binding than a short step.

The bar is bit identity on pressures, water and gas saturation, Rs, the clock, the solver warning,
the rate history and the grid state. Both are the same Rust on two targets, and since
[#62](https://github.com/sergeyfarin/ressim/issues/62) the engine's math is target-independent.
