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
  - Review findings to preserve during follow-up work:
    - Scalar-`cellDz` leakage remains in benchmark/chart normalization paths; `cellDzPerLayer` reaches the simulator core, but PV/OOIP and related review metrics still used the fallback scalar thickness in parts of the UI model.
    - SPE1 chart wiring violated the chart contract by mixing pressure and GOR in one panel and by styling simulation GOR as dashed instead of solid.
    - Hard-coded SPE1 published-reference curves are not yet auditable enough; the repo still needs raw-source provenance, variable mapping, unit conversions, and case-variant confirmation.
    - Per-layer completions currently mean repeated single-cell wells with shared controls, which is sufficient for current one-layer SPE1 wells but is not a general multi-completion-well model.
    - Validation and regression coverage for `cellDzPerLayer`, per-layer completions, and SPE1 reference-panel wiring remain incomplete above the Rust-core unit-test level.
  - Recommended implementation order:
    - Fix thickness normalization in benchmark/chart models first so SPE1 recovery/PV review uses the actual layered geometry.
    - Refactor SPE1 panels next so each quantity gets its own plot and reference curves use dashed styling consistently.
    - Reconstruct and check in auditable SPE1 reference provenance before tuning against the comparison curves.
    - Add focused regression tests for `cellDzPerLayer`, published-reference panel placement, and per-layer completion payload wiring.
  - [x] Per-layer cell thickness (dz) — Rust solver accepts `Vec<f64>` per layer instead of scalar.
  - [x] Per-layer initial gas saturation — Rust `setInitialGasSaturationPerLayer`.
  - [x] Worker wiring — TypeScript payload supports `cellDzPerLayer`, `initialSaturationPerLayer`, `initialGasSaturationPerLayer`.
  - [x] Per-layer well completions — worker supports `producerKLayers` / `injectorKLayers` for single-layer well completions.
  - [x] SPE1 scenario definition with published PVT, SCAL (Corey approximation), grid, and well data.
  - [x] Published reference data overlay — `publishedReferenceSeries` on Scenario type, wired through chart model with scatter markers.
  - [x] SPE1 chart layout (`spe1`) with pressure, GOR, oil rate, gas cut panels.
  - [x] Fix benchmark/chart thickness normalization so `cellDzPerLayer` drives PV/OOIP-based review metrics instead of falling back to scalar `cellDz`.
  - [x] Rework SPE1 chart layout to follow the chart convention: one quantity per panel, solid lines for simulation, dashed lines for reference, no mixed pressure/GOR panel.
  - [x] Honor SPE1 Case 1 `DRSDT = 0` semantics by disabling gas re-dissolution in the three-phase flash path.
  - [ ] Re-verify the SPE1 comparison source and metric mapping (Case 1 vs Case 2, average pressure vs field pressure, producing GOR vs summary-variable equivalent, units, and sampling cadence).
    - Current working mapping is `Avg Pressure vs time` and `producing GOR vs time` for Case 1 yearly samples. ResSim now carries explicit SPE1-style surface-rate targets plus BHP-limit diagnostics, but the control path is still a local PVT/state conversion rather than a full deck scheduler.
    - Exact OPM PVT plus SWOF/SGOF tables now reach the live simulator, and the earlier erratic GOR dips were traced to an implementation bug: the simulator was re-dissolving free gas even though SPE1 Case 1 is `DRSDT = 0`. Disabling gas re-dissolution removes that major non-physical oscillation source.
    - Unit audit status: the FIELD-to-metric scalar conversions used in the local SPE1 scenario check out (`psia → bar`, `Mscf/STB → Sm³/Sm³`, `rb/Mscf → rm³/Sm³`, `STB/d → Sm³/d`, `Mscf/d → Sm³/d`, `psi⁻¹ → bar⁻¹`). Remaining comparison risk is more likely quantity-definition mismatch than unit conversion, especially `FPR`/summary-pressure semantics versus the simulator's simple arithmetic cell-average pressure, and `FGOR`/`WGOR` semantics versus the simulator's produced-GOR diagnostic.
    - Pressure-shape mismatch still remains open after the `DRSDT = 0` fix: published pressure peaks then declines, while the current simulation still misses part of that peak-and-decline shape and declines too quickly after breakthrough. The remaining gap now looks more like well/schedule semantics or other black-oil transport details than benchmark-input fidelity.
  - [ ] Add regression tests for SPE1 scenario wiring, published-reference panel placement, and the `cellDzPerLayer` / per-layer completion payload path.
    - Current coverage now includes `cellDzPerLayer` normalization, the dedicated SPE1 GOR panel/style contract, pre-run published-reference visibility, exact SWOF/SGOF payload cloning, store wiring that preserves scenario-supplied PVT/SCAL tables, and scenario/create-payload wiring for `gasRedissolutionEnabled`. Completion-payload coverage is still missing.
  - [ ] Tune SPE1 rate targets and validate against Eclipse reference (qualitative match expected; exact match requires tabular SCAL).
    - Surface-rate targets and explicit BHP-limit diagnostics are now in place. Exact OPM PVT plus SWOF/SGOF inputs are also in place, SPE1 Case 1 now honors `DRSDT = 0`, and the fine-grid `grid_20` sensitivity carries tighter numerics to reduce stability complaints. Remaining mismatch should be explained by simulator behavior, not by benchmark-specific curve fitting.
  - [x] Add tabular SCAL support to Rust solver and worker/store payload path so SPE1 can use exact OPM SWOF/SGOF tables.
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
