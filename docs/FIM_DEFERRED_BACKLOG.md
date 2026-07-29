# Deferred FIM Backlog

FIM is in the user-facing product path as of `b88ee28` (2026-07-24). This file tracks the FIM work that is still deferred — convergence closure, linear-stack alignment, and diagnostic reproduction — not a product boundary.

## Solver Policy (shipped)

Declared completely in each module under `src/lib/catalog/scenarios/`; catalog assembly does not
inject solver parameters or sensitivities.

- Gas / three-phase scenarios default to FIM: `gas_injection`, `gas_drive`, `spe1_gas_injection`.
- Ordinary oil/water scenarios default to IMPES and do not carry a generic solver sensitivity.
- `solver_fim_impes` is the single public formulation comparison. Its scenario-owned sensitivity
  holds a coarse, rate-controlled waterflood fixed and changes only `fimEnabled`.
- Every scenario carries a `solverPolicy` with a user-visible rationale, surfaced in scenario cards and run labels.

## Later FIM Work

- Nonlinear stabilization and acceptance policy aligned against OPM Flow traces.
- FIM/OPM side-by-side diagnostic reproduction for waterflood, gas, and SPE1-style cases.
- CPR/CPRW/AMG follow-up after the current pressure-first path is re-baselined.
- SPE1/gas-path convergence closure with stable fast smoke tests and documented slow diagnostics.

## Validation Rule

Full `cargo test --manifest-path src/lib/ressim/Cargo.toml` is not a product-readiness gate while FIM/SPE1 hangs or dominates runtime. Use targeted FIM commands only when working on this backlog.
