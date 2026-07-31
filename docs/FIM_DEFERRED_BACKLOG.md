# Deferred FIM Backlog

FIM is in the user-facing product path as of `b88ee28` (2026-07-24). This file tracks the FIM work that is still deferred — convergence closure, linear-stack alignment, and diagnostic reproduction — not a product boundary.

## Solver Policy (shipped)

Declared completely in each module under `src/lib/catalog/scenarios/`; catalog assembly does not
inject solver parameters or sensitivities.

- Gas / three-phase scenarios default to FIM: `gas_injection`, `gas_drive`, `spe1_gas_injection`.
- Ordinary oil/water scenarios default to IMPES and do not carry a generic solver sensitivity.
- `wf_bl1d`'s `solver_formulation` dimension is the single public formulation comparison, folded in
  from the standalone `solver_fim_impes` scenario on 2026-07-31. It holds the 1D waterflood fixed
  and varies only `fimEnabled` and the report timestep (0.25 d and 5 d), so the four runs are judged
  against the timestep-independent Buckley-Leverett reference instead of only against each other.
  Measured over the 50-day flood: the formulations agree to 0.97% in cumulative oil at 0.25-day
  steps, while coarsening to 5-day steps costs IMPES 8.8% of its own fine-step recovery against
  FIM's 3.1%.
- Every scenario carries a `solverPolicy` with a user-visible rationale, surfaced in scenario cards and run labels.

## Later FIM Work

- Nonlinear stabilization and acceptance policy aligned against OPM Flow traces.
- FIM/OPM side-by-side diagnostic reproduction for waterflood, gas, and SPE1-style cases.
- CPR/CPRW/AMG follow-up after the current pressure-first path is re-baselined.
- SPE1/gas-path convergence closure with stable fast smoke tests and documented slow diagnostics.

## Validation Rule

Full `cargo test --manifest-path src/lib/ressim/Cargo.toml` is not a product-readiness gate while FIM/SPE1 hangs or dominates runtime. Use targeted FIM commands only when working on this backlog.
