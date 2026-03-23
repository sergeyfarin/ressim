# ResSim TODO

`TODO.md` is the active execution tracker. Completed work has been moved to `docs/DELIVERED_WORK_2026_Q1.md`.

## Now

- [x] Apply continuous saturation weighting to the vertical-only simulation sweep path so `E_V` no longer depends on a hard cutoff.
- [x] Apply continuous saturation weighting to the combined simulation volumetric sweep path so hidden `E_vol` diagnostics do not depend on a hard cutoff.
- [x] Switch areal simulation sweep from binary swept-column counting to continuous cell weighting, while keeping vertical/combined sweep semantics unchanged.
- [x] Make the simulation sweep threshold use the actual initial water saturation and add regression tests for nonzero-`initialSaturation` and degenerate-Welge fallback behavior.
- [x] Revert Buckley-Leverett analytical cum-oil and oil-rate overlays; fix recovery normalization for completed-run references.
- [x] Extend Buckley-Leverett analytical chart wiring so cumulative-oil preview curves and completed-run oil-rate references use the same analytical source.
- [x] Fix Buckley-Leverett preview recovery normalization so nonzero `initialSaturation` uses OOIP rather than unit pore volume.
- [ ] Add black-oil comparative-solution validation (`SPE1` / `SPE3` style coverage, or the closest practical subset) and document the acceptance policy.
  - [x] Per-layer cell thickness (dz) — Rust solver accepts `Vec<f64>` per layer instead of scalar.
  - [x] Per-layer initial gas saturation — Rust `setInitialGasSaturationPerLayer`.
  - [x] Worker wiring — TypeScript payload supports `cellDzPerLayer`, `initialSaturationPerLayer`, `initialGasSaturationPerLayer`.
  - [x] Per-layer well completions — worker supports `producerKLayers` / `injectorKLayers` for single-layer well completions.
  - [x] SPE1 scenario definition with published PVT, SCAL (Corey approximation), grid, and well data.
  - [x] Published reference data overlay — `publishedReferenceSeries` on Scenario type, wired through chart model with scatter markers.
  - [x] SPE1 chart layout (`spe1`) with pressure, GOR, oil rate, gas cut panels.
  - [ ] Tune SPE1 rate targets and validate against Eclipse reference (qualitative match expected; exact match requires tabular SCAL).
  - [ ] Add tabular SCAL support to Rust solver (currently Corey-only, SPE1 tables are approximated).
- [ ] Define the exit criteria for three-phase `experimental` status and add acceptance tests for gas injection and gas-drive scenarios.
- [ ] Reconcile all three-phase docs with the implemented state: corrected gas-oil capillary sign, `s_org`, explicit gas material-balance reporting, and remaining oil-phase diagnostic limits.
- [ ] Add regression tests for comparison-model preview mode, depletion per-variant analytical overlays, and color-index stability.
- [ ] Add a guard test for the duplicated undersaturated `c_o = 1e-5 /bar` assumption shared by the black-oil PVT generator and material-balance helper.
- [ ] Fix chart x-axis endpoint generation for cumulative/time modes: prepend zero anchors where appropriate and snap shared x-range/ticks to round values so Chart.js does not expose floating residues like `0.006` or `70.00000000006`.

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
