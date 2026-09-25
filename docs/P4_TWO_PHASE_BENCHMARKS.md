# P4 Two-Phase Benchmark Validation (Published References)

**Date:** 2026-02-15; methodology current, results moved to `BENCHMARKS.md` 2026-09-25
**Scope:** P4-1 - benchmark simulator against published two-phase references and define acceptance tolerances.

## Reference Basis

This benchmark uses classical Buckley-Leverett 1D immiscible displacement as the published reference model for two-phase waterflood behavior.

- Buckley, S. E., and Leverett, M. C. (1942). Mechanism of Fluid Displacement in Sands. Transactions of the AIME, 146(01), 107-116.
- Corey, A. T. (1954). The Interrelation Between Gas and Oil Relative Permeabilities. Producers Monthly.

Reference quantity for acceptance is breakthrough pore volumes injected (PV_BT), computed from the Buckley-Leverett shock condition:

PV_BT_ref = 1 / (df_w/dS_w at shock)

## Implemented Regression Benchmarks

Benchmarks are implemented as Rust unit tests in `src/lib/ressim/src/tests/buckley.rs`:

- `benchmark_buckley_leverett_case_a_favorable_mobility`
- `benchmark_buckley_leverett_case_b_more_adverse_mobility`
- `benchmark_buckley_leverett_breakthrough_is_independent_of_report_interval`
- `benchmark_buckley_leverett_grid_refinement_improves_alignment`

### Case Configuration Summary

Common setup:
- 1D grid (nx=24, ny=1, nz=1), IMPES
- Homogeneous permeability (k=2000 mD)
- Capillary entry pressure disabled (p_entry=0) to match Buckley-Leverett assumptions
- Injector at left (500 bar BHP), producer at right (100 bar BHP)
- Breakthrough criterion: producer water cut >= 0.01

Case-specific fluid and SCAL inputs:

| Case | Swc | Sor | nw | no | mu_w (cP) | mu_o (cP) | dt (days) |
|---|---:|---:|---:|---:|---:|---:|---:|
| BL-Case-A | 0.10 | 0.10 | 2.0 | 2.0 | 0.5 | 1.0 | 0.5 |
| BL-Case-B | 0.15 | 0.15 | 2.2 | 2.0 | 0.6 | 1.4 | 0.5 |

## Acceptance Tolerances

Tolerance metric:

Relative Error = abs((PV_BT_sim - PV_BT_ref) / PV_BT_ref)

- BL-Case-A acceptance: Relative Error <= 0.25 (25 percent)
- BL-Case-B acceptance: Relative Error <= 0.30 (30 percent)

These tolerances account for finite-volume numerical diffusion, BHP-driven well control (instead of strict constant-rate injection), and explicit saturation transport discretization in the current simulator.

Interpretation note:
- These thresholds are intentionally coarse regression guards for the present browser/WASM simulator configuration.
- They should not be read as the target accuracy of the Buckley-Leverett method itself; the measured errors are materially tighter (next section).

## Current Results

The current measured values, their commit and the replay command are recorded in
[`BENCHMARKS.md` §1](BENCHMARKS.md#1-buckleyleverett-breakthrough-analytical). This page does not
repeat them, so that there is only one place to update. Both cases pass well inside their bands.

IMPES picks its own substeps inside every `step` and records one rate point per substep, so the
case `dt` is only a report interval. The harness integrates injection over every substep point and
checks water cut at each one. Two tests guard the result:

- The report-interval test runs both cases at dt = 0.5 and 0.25 days and asserts that
  breakthrough moves by at most 1 %. The measured spread is 0.12 % or less.
- The grid test runs both cases at nx = 24 and 48. It asserts that breakthrough comes early
  (upstream numerical diffusion smears the front ahead of the shock) and that the finer grid is
  closer to the reference.

So the mismatch comes from spatial discretization, not from an incorrect reference solution.
`wf_numerics` shows the same first-order convergence in the app over a 40x cell-size range.

Until `92a57ec` (#53, 2026-09-25) the harness read only the last rate point of each outer step. That billed the
whole step at its final substep's rate and could see breakthrough only at step ends. Breakthrough
falls within the first 2–5 outer steps, so the result tracked the report interval. At dt = 0.5,
Case B read +40 % against −8 % once every substep is counted. The retired
`smaller_dt_improves_coarse_alignment` test was measuring that artifact, and so was the earlier
belief that Case B "needed" dt = 0.25.

Superseded: the 2026-02-15 version of this page recorded 4.0 % / 9.0 %, and 3.1 % / 2.5 % from a
`nx=96, dt=0.125` refined-discretization test that no longer exists.

## Interpretation of Differences

The remaining mismatch range here is expected for the current coarse numerical setup and is within acceptance limits.

Main causes:
- Finite-volume numerical diffusion in explicit saturation transport smears the displacement front.
- BHP-controlled wells are used, while classical Buckley-Leverett closed-form references are often presented in idealized constant-rate form.
- Discrete breakthrough detection threshold (`water cut >= 0.01`) and shock-gradient approximation (`df_w/dS_w`) still shift exact detection timing.

## Relation to Plot Analytical Curve

This page grades one number, breakthrough pore volumes injected. The charts in the app overlay the
full analytical water-cut, recovery and saturation-profile curves on the simulation, so they show
the whole trajectory but do not grade it.

The frontend reference is `src/lib/analytical/fractionalFlow.ts` (`@ressim/analytical`), which
uses the same Welge shock construction as the Rust benchmark. The waterflood scenario is `wf_bl1d`
(`src/lib/catalog/scenarios/wf_bl1d.ts`); its mobility, Corey-exponent and residual-oil sensitivities
are judged against the same reference. The grid, timestep and FIM-vs-IMPES comparisons are on
`wf_numerics`.

## Regression Execution

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml benchmark_buckley -- --nocapture
```

This also runs in pull-request CI. The benchmark acceptance evidence comes directly from Rust test
output rather than a generated artifact.
