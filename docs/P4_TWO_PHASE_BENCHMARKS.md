# P4 Two-Phase Benchmark Validation (Published References)

**Date:** 2026-02-15
**Scope:** P4-1 - benchmark simulator against published two-phase references and define acceptance tolerances.

## Reference Basis

This benchmark uses classical Buckley-Leverett 1D immiscible displacement as the published reference model for two-phase waterflood behavior.

- Buckley, S. E., and Leverett, M. C. (1942). Mechanism of Fluid Displacement in Sands. Transactions of the AIME, 146(01), 107-116.
- Corey, A. T. (1954). The Interrelation Between Gas and Oil Relative Permeabilities. Producers Monthly.

Reference quantity for acceptance is breakthrough pore volumes injected (PV_BT), computed from the Buckley-Leverett shock condition:

PV_BT_ref = 1 / (df_w/dS_w at shock)

## Implemented Regression Benchmarks

Benchmarks are implemented as Rust unit tests in src/lib/ressim/src/lib.rs:

- benchmark_buckley_leverett_case_a_favorable_mobility
- benchmark_buckley_leverett_case_b_more_adverse_mobility

### Case Configuration Summary

Common setup:
- 1D grid (nx=24, ny=1, nz=1)
- Homogeneous permeability (k=2000 mD)
- Capillary entry pressure disabled (p_entry=0) to match Buckley-Leverett assumptions
- Injector at left, producer at right
- Breakthrough criterion: producer water cut >= 0.01

Case-specific fluid and SCAL inputs:

| Case | Swc | Sor | nw | no | mu_w (cP) | mu_o (cP) |
|---|---:|---:|---:|---:|---:|---:|
| BL-Case-A | 0.10 | 0.10 | 2.0 | 2.0 | 0.5 | 1.0 |
| BL-Case-B | 0.15 | 0.15 | 2.2 | 2.0 | 0.6 | 1.4 |

## Acceptance Tolerances

Tolerance metric:

Relative Error = abs((PV_BT_sim - PV_BT_ref) / PV_BT_ref)

- BL-Case-A acceptance: Relative Error <= 0.25 (25 percent)
- BL-Case-B acceptance: Relative Error <= 0.30 (30 percent)

These tolerances account for finite-volume numerical diffusion, BHP-driven well control (instead of strict constant-rate injection), and explicit saturation transport discretization in the current simulator.

## Current Results (from test run)

Command:

```bash
cd src/lib/ressim
cargo test benchmark_buckley_leverett -- --nocapture
```

Observed output:

- BL-Case-A: PV_BT_sim=0.6093, PV_BT_ref=0.5860, Relative Error=0.040 (4.0 percent)
- BL-Case-B: PV_BT_sim=0.5531, PV_BT_ref=0.5074, Relative Error=0.090 (9.0 percent)

Both benchmark cases pass within the defined acceptance limits.

### Refined Discretization Check (additional test)

To verify that mismatch is mainly numerical/discretization-driven, an additional test was run with:
- Finer grid: `nx=96` (vs baseline `nx=24`)
- Smaller timestep: `dt_days=0.125` (vs baseline `dt_days=0.5` for Case A and `dt_days=0.25` for Case B)

Command:

```bash
cd src/lib/ressim
cargo test benchmark_buckley_leverett_refined_discretization_improves_alignment -- --nocapture
```

Observed output:
- Case-A coarse/refined rel_err: `0.040 -> 0.031`
- Case-B coarse/refined rel_err: `0.090 -> 0.025`

Comparison table:

| Case | Baseline Rel. Error | Refined Rel. Error (nx=96, dt=0.125) | Improvement |
|---|---:|---:|---:|
| BL-Case-A (`nx=24`, `dt=0.5`) | 4.0% | 3.1% | -0.9 percentage points |
| BL-Case-B (`nx=24`, `dt=0.25`) | 9.0% | 2.5% | -6.5 percentage points |

This supports the conclusion that a significant part of baseline mismatch comes from discretization (grid + timestep), not from an incorrect reference solution.

### Reference-Solution vs Simulation Summary

`PV_BT_ref` in the table above is the Buckley-Leverett reference solution (published theory basis), not another simulation.

| Case | Analytical PV_BT_ref | Simulation PV_BT_sim | Absolute Difference | Relative Difference |
|---|---:|---:|---:|---:|
| BL-Case-A | 0.5860 | 0.6093 | 0.0233 | 4.0% |
| BL-Case-B | 0.5074 | 0.5531 | 0.0457 | 9.0% |

## Interpretation of Differences

The remaining mismatch range here is expected for the current coarse numerical setup and is within acceptance limits.

Main causes:
- Finite-volume numerical diffusion in explicit saturation transport smears the displacement front.
- BHP-controlled wells are used, while classical Buckley-Leverett closed-form references are often presented in idealized constant-rate form.
- Coarse timestep choice matters materially. After the adaptive-step completion fix, BL-Case-B required `dt_days=0.25` rather than `0.5` to remain a fair coarse regression case.
- Discrete breakthrough detection threshold (`water cut >= 0.01`) and shock-gradient approximation (`df_w/dS_w`) still shift exact detection timing.

## Relation to Plot Analytical Curve

The plot analytical curve (and MAE/RMSE/MAPE shown in the rate chart) compares time-series rate behavior during interactive runs.

- Benchmark table in this page: breakthrough PV comparison against analytical Buckley-Leverett shock reference.
- Plot metrics in UI: time-series mismatch metrics between simulator rates and analytical rate curve for the current run settings.

Both are reference-solution comparisons, but they evaluate different observables (breakthrough PV versus full rate-time trajectory).

Frontend alignment note:
- `src/lib/FractionalFlow.svelte` was updated to use a tighter Buckley-Leverett shock search and derivative-matching inversion, consistent with the benchmark reference method.
- This improves consistency between plotted analytical trends and published Buckley-Leverett behavior.
- Analytical curve time samples now use simulation `rateHistory.time` directly (same x-axis basis as plotted simulation rates), so analytical and simulation series are compared at matching times.

## User-Facing Surface

- README includes a Model Validation Benchmarks (P4-1) section with tolerance policy and a link to this page.
- Chart-level reference-solution mismatch metrics (MAE/RMSE/MAPE) remain visible in the rate chart panel for quick run-time comparison.
- The family-owned reference workflow now exposes Buckley-Leverett families through the frontend benchmark registry rather than through duplicated preset payloads.
- Benchmark execution now uses the `Run Set` workflow: base only, or an explicit subset of variants within one selected sensitivity axis.
- Stored reference-run results now feed benchmark-specific comparison charts instead of relying on one live single-run chart path.
- There is no generated frontend benchmark artifact; benchmark evidence is kept in Rust tests and scenario presets.

## Current Frontend Benchmark Interpretation

The browser benchmark surface now distinguishes between benchmark families and benchmark variants.

- `bl_case_a_refined` and `bl_case_b_refined` are the homogeneous Rust-parity Buckley-Leverett base families.
- Grid-refinement and timestep-refinement variants preserve the same Buckley-Leverett reference-solution contract as the base family.
- Heterogeneity variants are still useful benchmark comparison runs, but their primary review baseline is a refined numerical reference rather than strict reference-solution equality.

The chart defaults in the family-owned reference workflow now reflect that interpretation:

- Buckley-Leverett charts default to `PVI` on the x-axis.
- The primary panels are breakthrough, recovery, and pressure.
- Multi-run overlays compare stored base-plus-variant results and keep the Buckley-Leverett reference-solution trace as shared context; for heterogeneous variants that trace is secondary context rather than the primary review metric.

For the broader benchmark workflow and UI contract, see `docs/BENCHMARK_MODE_GUIDE.md`.

## Regression Execution (P4-5)

Run the benchmark regression tests with:

```bash
cd src/lib/ressim
cargo test benchmark_buckley_leverett -- --nocapture
```

For broader validation, also run:

```bash
npm test
npm run build
```

The benchmark acceptance evidence now comes directly from Rust test output rather than a generated JSON artifact.
