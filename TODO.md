# ResSim TODO

`TODO.md` is the active execution tracker. Completed work has been moved to `docs/DELIVERED_WORK_2026_Q1.md`.

## Now

- [ ] Add black-oil comparative-solution validation (`SPE1` / `SPE3` style coverage, or the closest practical subset) and document the acceptance policy.
- [ ] Define the exit criteria for three-phase `experimental` status and add acceptance tests for gas injection and gas-drive scenarios.
- [ ] Reconcile all three-phase docs with the implemented state: corrected gas-oil capillary sign, `s_org`, explicit gas material-balance reporting, and remaining oil-phase diagnostic limits.
- [ ] Add regression tests for comparison-model preview mode, depletion per-variant analytical overlays, and color-index stability.
- [ ] Add a guard test for the duplicated undersaturated `c_o = 1e-5 /bar` assumption shared by the black-oil PVT generator and material-balance helper.

## Next

- [ ] Enforce analytical-method semantics at the scenario type level so sweep scenarios cannot accidentally inherit Buckley-Leverett-style primary curves or disclosure rules.
- [ ] Generalize the `sweep_combined` analytical-method toggle into a reusable sweep-method framework.
- [ ] Collapse the remaining legacy benchmark layer into the scenario system where practical.
- [ ] Extract a typed output-selection view model from `App.svelte` for charts, 3D output, and analytical helpers.
- [ ] Decide whether `SwProfileChart` should be restored as a maintained output or removed completely.
- [ ] Document `sweep_areal` explicitly as a quarter-five-spot style interpretation with no-flow outer boundaries.

## Later

- [ ] Add per-scenario parameter overrides without forcing a switch into full custom mode.
- [ ] Add multi-case 3D inspection and synchronized case selection across outputs.
- [ ] Add scenario and results export/import.
- [ ] Add gas-cap scenarios on top of the validated black-oil base.
- [ ] Add Warren-Root style vertical sweep blending for finite vertical communication.
- [ ] Add aquifer models, schedules, non-uniform grids, and horizontal wells only after the current validation backlog is closed.

## Reference Notes To Keep

- `sweep_ladder` intentionally leaves analytical overlays shared even though the patched viscosity would change the analytical result. That is a teaching choice, not a bug.
- In black-oil mode, the saturated-region `c_o` fallback is intentional. Removing it would destabilize the IMPES pressure solve and double-count dissolved-gas effects already handled in phase-split logic.
- Water and gas cumulative material-balance errors are reported explicitly in three-phase mode. Oil is still the residual phase in diagnostics.
