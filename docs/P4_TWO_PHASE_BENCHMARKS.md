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

- BL-Case-A: PV_BT_sim=0.5239, PV_BT_ref=0.5860, Relative Error=0.106 (10.6 percent)
- BL-Case-B: PV_BT_sim=0.4768, PV_BT_ref=0.5074, Relative Error=0.060 (6.0 percent)

Both benchmark cases pass within the defined acceptance limits.

### Refined Discretization Check (additional test)

To verify that mismatch is mainly numerical/discretization-driven, an additional test was run with:
- Finer grid: `nx=96` (vs baseline `nx=24`)
- Smaller timestep: `dt_days=0.125` (vs baseline `dt_days=0.5`)

Command:

```bash
cd src/lib/ressim
cargo test benchmark_buckley_leverett_refined_discretization_improves_alignment -- --nocapture
```

Observed output:
- Case-A coarse/refined rel_err: `0.106 -> 0.036`
- Case-B coarse/refined rel_err: `0.060 -> 0.033`

Comparison table:

| Case | Baseline Rel. Error (nx=24, dt=0.5) | Refined Rel. Error (nx=96, dt=0.125) | Improvement |
|---|---:|---:|---:|
| BL-Case-A | 10.6% | 3.6% | -7.0 percentage points |
| BL-Case-B | 6.0% | 3.3% | -2.7 percentage points |

This supports the conclusion that a significant part of baseline mismatch comes from discretization (grid + timestep), not from an incorrect analytical reference.

### Analytical vs Simulation Summary

`PV_BT_ref` in the table above is the analytical Buckley-Leverett reference (published theory basis), not another simulation.

| Case | Analytical PV_BT_ref | Simulation PV_BT_sim | Absolute Difference | Relative Difference |
|---|---:|---:|---:|---:|
| BL-Case-A | 0.5860 | 0.5239 | 0.0621 | 10.6% |
| BL-Case-B | 0.5074 | 0.4768 | 0.0306 | 6.0% |

## Interpretation of Differences

The 5-10 percent mismatch range here is expected for the current numerical setup and is within acceptance limits.

Main causes:
- Finite-volume numerical diffusion in explicit saturation transport smears the displacement front.
- BHP-controlled wells are used, while classical Buckley-Leverett closed-form references are often presented in idealized constant-rate form.
- Discrete timestep/substep behavior and breakthrough detection threshold (`water cut >= 0.01`) shift exact detection timing.
- Small discretization effects from grid resolution (`nx=24`) and shock-gradient approximation (`df_w/dS_w`) are present.

## Relation to Plot Analytical Curve

The plot analytical curve (and MAE/RMSE/MAPE shown in the rate chart) compares time-series rate behavior during interactive runs.

- Benchmark table in this page: breakthrough PV comparison against analytical Buckley-Leverett shock reference.
- Plot metrics in UI: time-series mismatch metrics between simulator rates and analytical rate curve for the current run settings.

Both are analytical comparisons, but they evaluate different observables (breakthrough PV versus full rate-time trajectory).

Frontend alignment note:
- `src/lib/FractionalFlow.svelte` was updated to use a tighter Buckley-Leverett shock search and derivative-matching inversion, consistent with the benchmark reference method.
- This improves consistency between plotted analytical trends and published Buckley-Leverett behavior.
- Analytical curve time samples now use simulation `rateHistory.time` directly (same x-axis basis as plotted simulation rates), so analytical and simulation series are compared at matching times.

## User-Facing Surface

- README includes a Model Validation Benchmarks (P4-1) section with tolerance policy and a link to this page.
- Chart-level analytical mismatch metrics (MAE/RMSE/MAPE) remain visible in the rate chart panel for quick run-time comparison.
- Frontend bottom-page benchmark summary now reads from `public/benchmark-results.json` generated by `npm run bench:export`.
- Frontend benchmark panel now includes a Baseline/Refined toggle so users can directly see discretization-impact evidence in UI.

## Artifact Generation (P4-5)

Generate/update the benchmark artifact with:

```bash
npm run bench:export
```

This runs:

```bash
cargo test benchmark_buckley_leverett -- --nocapture
```

and writes parsed case metrics to `public/benchmark-results.json`.

CI workflow note:
- `.github/workflows/benchmark-artifact.yml` runs `npm run build`.
- `build` invokes `prebuild`, and `prebuild` invokes `npm run bench:export`.
- Therefore benchmark artifact generation is executed on every CI build run.
