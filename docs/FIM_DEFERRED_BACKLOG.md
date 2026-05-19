# Deferred FIM Backlog

FIM is intentionally out of the user-facing product path while ResSim is stabilized around IMPES and offline OPM Flow references. Keep the code, tests, and diagnostics available for developer work, but do not treat FIM as part of product readiness until the gates below are met.

## Product Boundary

- Public scenario runs default to IMPES.
- FIM remains accessible only through explicit developer tests, Rust APIs, and diagnostic scripts.
- Do not add public UI solver toggles until predefined-case acceptance is stable.

## Later FIM Work

- Nonlinear stabilization and acceptance policy aligned against OPM Flow traces.
- FIM/OPM side-by-side diagnostic reproduction for waterflood, gas, and SPE1-style cases.
- CPR/CPRW/AMG follow-up after the current pressure-first path is re-baselined.
- SPE1/gas-path convergence closure with stable fast smoke tests and documented slow diagnostics.

## Validation Rule

Full `cargo test --manifest-path src/lib/ressim/Cargo.toml` is not a product-readiness gate while FIM/SPE1 hangs or dominates runtime. Use targeted FIM commands only when working on this backlog.
