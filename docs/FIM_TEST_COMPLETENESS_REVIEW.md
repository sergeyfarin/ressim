# FIM Engine Test Completeness Review

Date: 2026-04-02

This document reviews the completeness of FIM physics tests relative to the categories listed in
`FIM_PHYSICS_TEST_PLAN.md`, identifies material gaps, and recommends next tests. It also answers the
question of when correctness work is complete enough to prioritize convergence tuning.

---

## Summary verdict

The test suite is substantially built out through Phases 1–4 of the plan. Fast family regressions
exist for all nine coverage families, and the previously last targeted reporting gap is now closed:

1. **Oil MB is now a first-class diagnostic** — `TimePointRates` now reports
   `material_balance_error_oil_m3`, and the fast suite includes a direct single-step FIM oil
   inventory oracle.

The previously reported GAP-4 regressions are green in HEAD: the two Rs-switch tests in
`fim/state.rs` and the gas-injector surface-pressure derivative test in `fim/wells.rs` now pass.
That removes the earlier correctness-gate blocker, and the fast-suite material-balance tightening
work is now in place.

---

## What is well-covered

### Oil depletion (`physics/depletion_oil.rs`)
- Single-cell abs oil residual check (Tier 0) — good.
- Single-cell timestep stability (coarse vs fine) — good.
- Closed-system monotone pressure/rate/cumulative — good.
- Higher oil/rock compressibility cushions pressure drop — good.
- Stronger drawdown increases pressure-work proxy — good.
- Ignored dep_pss timestep refinement probe — good.
- Ignored late-time Dietz analytical comparison — good.

### Gas depletion (`physics/depletion_gas.rs`)
- Single-cell timestep stability (coarse vs fine) — good.
- Single-cell positive gas rate — good.
- Single-cell storage and material balance over 8 steps (15% envelope) — marginal tolerance.
- Gentle closed-system per-step gas accounting gate at `1e-3` relative — good.
- Gas inventory scales with Bg(p) direction — good.
- Closed-system monotone pressure/gas inventory — good.
- Case matrix across sat, perm, PVT, SCAL — good.
- Ignored refinement probe — good.

### Gas liberation (`physics/depletion_liberation.rs`)
- Flash below bubble point conserves total gas (with and without redissolution) — good.
- FIM stepping through bubble point liberates gas — good.
- Component balances (water, oil, gas) close across phase transition — good (5e-3 oil, 1e-3 gas).
- Undersaturated depletion keeps `Rs` constant and `Sg` at numerical zero while pressure stays above bubble — good.
- Ignored timestep refinement probe — good.

### Waterflood (`physics/waterflood.rs`)
- 1D mass conservation (water MB + saturation direction) — good.
- Injector saturation monotone increase — good.
- Case matrix across SCAL and capillary ranges — good.
- Buckley case A FIM-vs-IMPES early profile (ignored) — good.
- Buckley refined grid analytical alignment (ignored) — good.
- Ignored timestep refinement probe — good.

### Gas flood (`physics/gas_flood.rs`)
- 1D creates free gas and keeps MB bounded with relative gas limit — good.
- Short 1D per-component MB (water 5e-6, oil 5e-3, gas 1e-2) plus per-step gas-accounting gate — good.
- Saturation closure sw+so+sg=1 — good.
- Large steps keep state bounded — good.
- Case matrix across sat, perm, PVT, capillary — good.
- SPE1 coarse breakthrough (ignored) — good.
- Ignored timestep refinement probe — good.

### Gas cap / gravity (`physics/gas_cap.rs`)
- Vertical column builds hydrostatic gradient (gravity-on vs -off) — good.
- FIM vs IMPES hydrostatic benchmark (gradient + top Sw) — good.
- Gravity quieter than no-gravity when started at hydrostatic — good.
- Gravity matrix across perm and capillary — good.
- Free gas cap: capillary entry changes gas-cap support and balance — good.
- Free gas cap: gravity-on vs -off changes gas delivery and balance — good.
- Ignored gravity column refinement probe — good.

### PVT/flash (`physics/pvt_flash.rs`)
- No-PVT oil compressibility: Bo, density, dBo/dp — good.
- Two-phase zero-gas exact invariant (Sg=0, Rs=0, Sw+So=1) — good.
- Tabular Bg derivative FD consistency — good.

### Wells/sources (`physics/wells_sources.rs`)
- Single-cell producer: reporting matches perforation component rates — good.
- Transport reporting reuses rate-control decision — good.
- Rate-controlled gas injector FIM path converges and tracks target surface rate — good.
- Gas injection surface totals use Bg conversion (range check only) — marginal.
- Producing GOR is zero when oil rate is negligible — good.

### Geometry/anisotropy (`physics/geometry_anisotropy.rs`)
- 2D waterflood high-perm streak advances front faster — good.
- 2D gas flood streak (ignored) — good.
- 3D high kz spreads waterflood front (ignored) — good.
- 3D high kz accelerates gas segregation — good.
- 2D refined iterative backend (ignored) — good.

---

## Significant gaps

### GAP-1 (CLOSED 2026-04-02): Jacobian finite-difference consistency test

**Status**: `assembly.rs` already had an ignored internal full-system waterflood FD diagnostic, so
the original “no Jacobian consistency test” claim was overstated. The real gap was that default
acceptance coverage did not exercise the single-cell depletion path or a mixed
saturated/undersaturated state.

**What was added**: `src/lib/ressim/src/fim/assembly.rs` now contains two non-ignored acceptance
tests:

- `full_system_jacobian_matches_fd_for_single_cell_depletion`
- `full_system_jacobian_matches_fd_for_mixed_saturated_and_undersaturated_cells`

These close the practical risk behind GAP-1 by checking all unknown columns for a 5-unknown closed
depletion system and a 10-unknown 2-cell mixed-regime well system. The older ignored waterflood FD
diagnostic remains useful as a broader opt-in debug probe.

**File**: `physics/pvt_flash.rs` or a new `physics/assembly_jacobian.rs`.

---

### GAP-2 (CLOSED 2026-04-02): Local flux conservation oracle

**Status**: `assembly.rs` already had a useful conservative intercell residual test, so the original
claim was overstated. The real missing piece was an explicit residual-only oracle that isolates a
nonzero interface flux and proves equal-and-opposite face contributions without accumulation or well terms.

**What was added**: `src/lib/ressim/src/fim/assembly.rs` now contains two non-ignored acceptance
tests:

- `residual_only_two_cell_flux_is_component_conservative_for_oil_water`
- `residual_only_two_cell_flux_is_component_conservative_for_three_phase_gas`

These tests use `assemble_residual_only=true`, `include_wells=false`, and `previous_state == state`
to remove accumulation and well pollution. They then verify that the face contributions returned by
`cell_equation_residual_breakdown` are equal and opposite across the interface and that the assembled
residual sums to zero for the exercised components. The second test covers the gas-component path in
three-phase mode with nonzero gas flux.

---

### GAP-3 (CLOSED 2026-04-02): Direct transmissibility formula oracle

**Status**: the codebase already had a geometric transmissibility API check in `geometry_api.rs`,
but the review gap was still real because nothing verified that the assembled oil face flux matched
the analytic TPFA transmissibility formula after Darcy scaling, mobility, and `B_o` conversion.

**What was added**: `src/lib/ressim/src/fim/assembly_tests.rs` now contains two non-ignored
acceptance tests:

- `direct_transmissibility_formula_matches_homogeneous_two_cell_oil_flux`
- `direct_transmissibility_formula_matches_heterogeneous_two_cell_oil_flux`

These tests use `cell_face_phase_flux_diagnostics` on a 2-cell oil-water setup with gravity and
capillary disabled, then verify the oil flux against the analytic TPFA formula:

`q_o_sc_day = DARCY_METRIC_FACTOR * T_geom * dp * (kro / mu_o) / B_o`

The first test covers the homogeneous reduction `T = k * A / dx`; the second covers the harmonic
mean heterogeneous case. Together they close the constant-factor and missing-half-factor risk that
outcome-only geometry tests would miss.

---

### GAP-4 (CLOSED 2026-04-02): Rs-switch regime correctness

**Status**: this gap is no longer open in HEAD. The two Rs-switch regressions in `fim/state.rs`
and the gas-injector surface-pressure derivative regression in `fim/wells.rs` all pass.

**Why it matters**: The Rs-switch is the core of the saturated/undersaturated regime logic.  A
wrong switch direction means FIM will treat above-bubble-point cells as saturated (computing
incorrect gas accumulation) or fail to trigger gas liberation below the bubble point.  This is
confirmed as the root cause of the FIM vs IMPES parity gap on depletion cases (TODO.md, 2026-03-31).
The gas-injector derivative failure means the FIM Jacobian for rate-controlled gas injection is
wrong, which directly causes poor convergence on gas-injection cases.

**What was verified**: on 2026-04-02 the following regressions were re-run directly and passed:

- `classify_regimes_switches_immediately_when_rs_exceeds_rs_sat`
- `classify_regimes_preserves_gas_inventory_when_undersaturated_state_exceeds_rs_sat`
- `gas_injector_surface_pressure_derivatives_match_local_fd`

This removes GAP-4 as an active blocker. Keep the tests as part of the default correctness gate,
because they still guard a high-risk phase-switch and injection-derivative path.

---

### GAP-5 (CLOSED 2026-04-02): Accumulation term — water compressibility (Bw) path

**Status**: this gap was only partially open. The code already had an exact Jacobian check for the
water accumulation block, but it did not have a residual-level oracle that compared identical state
transitions under different `Bw` values.

**Why it matters**: If water compressibility is ever activated or if Bw is incorrectly wired to a
zero value, water would be infinitely stored per unit pore volume.  Even with constant Bw, a
factor error in the accumulation denominator produces an incorrect water material balance that the
current suite would not catch at the single-timestep level.

**What was added**: `src/lib/ressim/src/fim/assembly_tests.rs` now contains
`water_accumulation_residual_scales_with_bw_denominator`, which evaluates the same one-cell state
transition at `Bw = 1.0` and `Bw = 2.0` and asserts that the water accumulation residual scales by
exactly a factor of 2.

Together with the pre-existing `accumulation_block_has_exact_water_derivatives` test, this now
covers both the residual path and the exact Jacobian path for the `1 / Bw` denominator.

---

### GAP-6 (CLOSED 2026-04-02): Peaceman connection law direct oracle

**Status**: the legacy well-control suite already had `well_productivity_index_matches_metric_unit_conversion`,
so the geometric PI formula itself was partly covered. The review gap was still real because the FIM
perforation-source path had no direct oracle tying `perforation_component_rates_sc_day` back to the
analytic Peaceman connection law.

**Why it matters**: An error in `WI` (for example a missing `2π` factor or wrong log-ratio) produces
a systematically wrong productivity index for all wells. The current well tests verify that the
reporting numbers match the perforation-source function, but not that the perforation-source function
uses the correct formula.

**What was added**: `src/lib/ressim/src/tests/physics/wells_sources.rs` now contains the non-ignored
acceptance test `physics_wells_sources_peaceman_connection_law_matches_analytical_wi`.

It builds the 1-cell closed depletion producer fixture, calls the FIM perforation-source path
directly, and verifies that the oil component rate matches the analytic formula:

`q_o_sc_day = WI * (p_cell - p_bhp) * kro / (mu_o * B_o)`

where `WI` is recomputed from the full Peaceman metric formula using the fixture permeability,
cell dimensions, well radius, and skin. This closes the remaining gap between the already-covered
geometric PI helper and the actual FIM perforation-rate implementation.

---

### GAP-7 (CLOSED 2026-04-02): Per-component material balance at tight tolerance in fast suite

**Status**: this gap is now closed at the fast-suite acceptance level used elsewhere in the review:
the family-owned default tests now include `1e-3`-class material-balance gates for gas depletion,
liberation, and the short gas-flood path.

**What was added or tightened**:

- `physics_depletion_gas_gentle_case_keeps_per_step_material_balance_tight` now exercises a gentler
   closed-system gas depletion case and verifies per-step gas accounting stays within `1e-3`
   relative of initial inventory.
- `physics_depletion_liberation_component_balances_close_across_phase_transition` now keeps gas
   balance within `1e-3` relative after 5 steps.
- `physics_gas_flood_1d_short_material_balance_matches_inventory_change` now verifies per-step gas
   accounting stays within `1e-3` relative of `initial + injected`.

**Investigation note**: the harsher original gas-depletion storage fixture still fragments a single
`0.005 d` call into many accepted micro-substeps (123 in the checked repro), so a top-level
`1e-4` cumulative gate on that stress case overstates the per-accepted-step Newton tolerance issue.
The new gentler closed-system depletion gate is the right fast default conservation oracle for this
family, while the older high-drawdown fixture remains useful as a broader envelope test.

**Why it matters**: these tighter default checks now catch sign errors and factor-of-`Bg` bugs over
short runs without relying only on the older 1–15% scenario envelopes.

---

### GAP-8 (CLOSED 2026-04-02): Undersaturated regime accumulation — Rs stays constant

**Status**: this gap is now closed. The liberation family had strong below-bubble-point and
through-bubble-point coverage, but it did not explicitly pin the complementary above-bubble-point
case where a cell should remain undersaturated with constant `Rs` and numerically zero free gas.

**Why it matters**: a bug in the phase-switch logic could spuriously update `Rs` during
undersaturated stepping. That would incorrectly change the gas inventory without any phase
transition.

**What was added**: `src/lib/ressim/src/tests/physics/depletion_liberation.rs` now contains
`physics_depletion_liberation_undersaturated_rs_stays_constant`.

It applies a sequence of above-bubble pressure-lowering updates on the undersaturated continuation
branch, keeps the cell above the 150-bar bubble point throughout, and asserts after every update that:

- `Rs` stays exactly at its initial value.
- `Sg` stays at numerical zero.

This closes the missing above-bubble-point phase-switch oracle that the existing liberation tests
did not exercise directly.

---

### GAP-9 (CLOSED 2026-04-02): Rate-controlled injection in FIM path

**Status**: this gap is now closed. The earlier well-source coverage exercised rate control only in
the transport/reporting path, not through the FIM Newton well-constraint equations.

**Why it matters**: the `rate` control mode goes through a separate complementarity/Jacobian path
(`well_constraint_residual`, Fischer-Burmeister slacks, and the exact well-constraint Jacobian).
A bug there would show up specifically on rate-targeted injector cases even if BHP-controlled wells remain healthy.

**What was added**: `src/lib/ressim/src/tests/physics/wells_sources.rs` now contains
`physics_wells_sources_rate_controlled_injector_fim_path_converges`.

It builds a single-cell gas injector in FIM mode with a surface-rate target, runs one Newton timestep,
and verifies that:

- the FIM timestep converges,
- the accepted state satisfies both the physical-well constraint and perforation-rate residuals,
- the injector remains off its BHP limit, and
- the accepted and reported gas injection rates stay within 5% of the target surface rate.

---

### GAP-10 (CLOSED 2026-04-03): Multi-perforation well BHP consistency

**Status**: this gap is now closed. `src/lib/ressim/src/tests/physics/wells_sources.rs` contains
`physics_wells_sources_multi_layer_well_shares_bhp_and_splits_rate_by_mobility`, which builds a
shared-ID two-completion producer and verifies that the initialized FIM state keeps a single
physical-well BHP while the two perforation rates match the individual connection rates at that
shared BHP.

**Why it matters**: the `build_well_topology` grouping logic and the `well_constraint_residual`
sum over perforations. A bug in the summing could produce inconsistent BHP or wrong rate splits
between layers.

---

### GAP-11 (CLOSED 2026-04-02): Face-flux sign in gas component (dissolved + free gas flux)

**Status**: this gap is now closed. The conservative three-phase residual test already proved that
the gas-component face contribution was equal and opposite across the interface, but it did not
isolate whether the dissolved-gas `Rs * oil_flux` contribution was present with the correct upwind sign.

**Why it matters**: a sign or upwinding error in the dissolved-gas term would mis-transport gas
component inventory even when the free-gas path and scenario-level material-balance checks look acceptable.

**What was added**: `src/lib/ressim/src/fim/assembly_tests.rs` now contains
`gas_component_flux_includes_dissolved_gas_term_with_upwind_rs_sign`.

It builds a 2-cell undersaturated oil case with zero free-gas mobility, assembles residual-only
fluxes, and verifies that:

- the oil and gas component fluxes both upwind from the high-pressure cell,
- the free-gas mobility term is zero,
- the gas-component face flux matches `Rs_upwind * oil_flux`, and
- the assembled gas residual contributions are equal and opposite across the interface.

---

### GAP-12 (CLOSED 2026-04-03): Gravity term magnitude validation

**Status**: this gap is now closed. `src/lib/ressim/src/tests/physics/gas_cap.rs` contains
`physics_gas_cap_gravity_term_magnitude_matches_hydrostatic_analytical`, which seeds a 2-cell
vertical hydrostatic column, runs one gravity-enabled step, and verifies that the measured pressure
offset matches the analytical hydrostatic weight in bar.

**Why it matters**: a unit error (e.g., using `g = 9.80665` m/s² but forgetting the bar conversion,
or using a wrong density unit) would produce a gravity force that is off by a constant factor. The
new oracle checks the absolute magnitude directly.

---

### GAP-13 (CLOSED 2026-04-04): Oil-component MB diagnostic in rate history

**Previous gap**: `TimePointRates` had explicit `material_balance_error_m3` (water) and
`material_balance_error_gas_m3` (gas) fields, but oil MB was not tracked as a first-class field.
The physics tests computed oil MB by integrating cumulative production from rate history, which
was only an indirect check.

**Why it mattered**: If the oil production rate was misreported but the oil MB check passed
because cumulative integration compensated for short windows, a systematic oil reporting error
could remain hidden for a long time.

**What was changed**: `src/lib/ressim/src/reporting.rs` now records
`material_balance_error_oil_m3` on every `TimePointRates` entry for both IMPES and FIM paths,
using the same cumulative source-minus-inventory pattern already used for water and gas.
`src/lib/ressim/src/tests/physics/depletion_oil.rs` now contains the non-ignored acceptance test
`physics_depletion_oil_fim_single_step_reports_direct_oil_mb`, which runs a real FIM step on the
single-cell depletion fixture and verifies that the reported oil MB field matches the direct
inventory-drop oracle at single-step resolution.

---

### GAP-14 (CLOSED 2026-04-02): Gas MB absolute limit is too loose in gas flood fast suite

**Status**: this gap is now closed. The short gas-flood family already had a stronger per-step
relative accounting gate, but `physics_gas_flood_1d_creates_free_gas_and_keeps_balance_bounded`
was still using only an absolute `5e3 Sm3` limit.

**Why it matters**: for a small fixture (8 cells, 10 m × 10 m × 1 m) the pore volume is of order
`8 * 10 * 10 * 1 * 0.2 = 160 m3`, so a pure absolute limit can be many times the entire initial gas
inventory and would not catch a factor-of-2 gas-accumulation error.

**What was changed**: `physics_gas_flood_1d_creates_free_gas_and_keeps_balance_bounded` now checks

`material_balance_error_gas_m3 / (initial_gas_sc + cumulative_injected_gas_sc).max(1.0) < 0.01`

instead of the old absolute `5e3 Sm3` bound. That closes the remaining fast-suite absolute-limit
hole and aligns the broader gas-flood smoke test with the relative-MB policy used elsewhere.

---

## Summary table of recommended new tests

| Priority | Test name | Family | Tier | Notes |
|----------|-----------|--------|------|-------|
| P0 | Keep GAP-4 regressions green | all | — | Already resolved in HEAD |
| P1 | `physics_assembly_jacobian_fd_consistent_*` | assembly | 0 | Most important new test |
| P1 | `physics_assembly_flux_local_conservation_two_cell` | assembly | 0 | Fundamental FD property |
| P1 | `physics_geometry_transmissibility_formula_two_cell` | geometry | 0 | Direct formula oracle |
| P2 | `physics_wells_sources_peaceman_connection_law_matches_analytical_wi` | wells | 0 | Direct formula oracle |
| P2 | `physics_assembly_water_accumulation_uses_bw_denominator` | PVT | 0 | Accumulation path |
| P2 | `physics_depletion_liberation_undersaturated_rs_stays_constant` | liberation | 0 | Phase-switch gate |
| P3 | `physics_wells_sources_multi_layer_well_shares_bhp_and_splits_rate_by_mobility` | wells | 1 | Closed 2026-04-03 |
| P3 | Tighten gas depletion MB to <1e-3 relative | depletion gas | 1 | Tolerance upgrade |
| P3 | Tighten gas flood MB to <1e-3 relative per step | gas flood | 1 | Tolerance upgrade |
| P4 | Oil-rate-to-delta-inventory per-step check | all | 2 | Closed 2026-04-04 by first-class oil MB field + single-step oracle |

---

## When to switch to convergence

The test plan's exit criterion (Section "Exit Criterion For Physics-First Gate") lists 7 conditions.
Mapping the current state against each:

1. **Fast local storage, flash, and well/source consistency tests are green** — yes, including the
   previously failing GAP-4 regressions.  Status: **MET**.

2. **Fast 1D family tests for all major families are green** — yes for all families.  Status: **MET**.

3. **At least one small 2D and one small 3D physics sanity case are green** — yes (2D waterflood
   streak, 3D gas segregation).  Status: **MET**.

4. **Ignored refinement probes no longer show first-order contradictions** — not verified
   systematically; Dietz analytical probe is explicitly documented as showing a known gap.
   Status: **PARTIAL**.

5. **Any remaining mismatch is clearly benchmark/model-alignment work, not conservation failure** —
   with GAP-14 closed, this is now met at the fast-suite gate level. Status: **MET**.

6. **Gravity and capillary exercised by at least one runtime scenario test** — yes.  Status: **MET**.

7. **Geometry coverage includes at least one larger ignored case crossing iterative backend** — yes
   (the refined 2D waterflood streak).  Status: **MET**.

**Verdict**: the fast-suite conservation gate is now in place. The right time to prioritize
convergence over correctness is now controlled more by the remaining non-MB targeted correctness
oracles than by loose default balance tolerances.

- At least one Jacobian FD consistency test passes (GAP-1).
- At least one local flux conservation test passes (GAP-2).
- The remaining non-MB targeted correctness oracles for multi-perforation wells/gravity are addressed.

These 4 conditions together guarantee that:

- The Newton iterate can be trusted to reduce a physically correct residual.
- The Jacobian is consistent with the residual, so Newton steps go in the right direction.
- Conservation failures can be attributed to model choices, not assembler bugs.
- Convergence slow-down on hard cases (e.g., the 12x12x3 waterflood shelf) is genuinely a
  nonlinear globalization problem, not a residual sign error that has been masked by loose
  tolerance checks.

Once those 4 conditions are met, the correctness gate is solid enough to shift focus to:

- Stronger nonlinear globalization (Appleyard-only damping, trust-region controls).
- Hotspot-aware timestep failure memory.
- Well-aware CPR pressure coarse solve.
- Iterative backend tuning for moderate-grid cases.

---

## Notes on tolerances and acceptance criteria

The current material balance checks use mixed units and mixed norms that make it hard to compare
across families.  Recommend settling on one standard:

- Relative error = `|accounted - initial| / initial.max(1.0)` for inventory checks.
- Threshold ≤ 1e-3 (0.1%) for fast default checks; ≤ 1e-4 (0.01%) for ignored refinement probes.
- Never use absolute Sm3 limits without a corresponding relative bound, because the absolute limit
  becomes meaningless when the fixture size changes.

The current gas flood absolute limit of 5000 Sm3 is the clearest example of this problem.

---

## Notes on Jacobian test strategy

The `perturb_cell_unknown` and `finite_difference_step` helpers in `assembly.rs` are already
under `#[cfg(test)]`.  The Jacobian test should:

1. Build a non-trivial state with nonzero pressure gradient, nonzero saturation, and active wells.
2. Call `assemble_fim_system` to get both residual `R(x)` and Jacobian `J`.
3. For each column `k` of `J`:
   a. Perturb `x` by `h = finite_difference_step(state, k)` using `perturb_cell_unknown`.
   b. Call `assemble_fim_system` with `assemble_residual_only=true` to get `R(x+h*e_k)`.
   c. Assert `|(R(x+h) - R(x)) / h - J[:, k]| / max(|J[:, k]|, 1.0) < 1e-4`.
4. Cover both saturated and undersaturated cell regimes.
5. Cover at least one well BHP column and one perforation rate column.

This is a standard test used in all production FIM simulators (OPM, tNavigator, Eclipse).
Without it, Jacobian bugs can persist for months — as the water-gravity term mismatch
documented in TODO.md demonstrates.
