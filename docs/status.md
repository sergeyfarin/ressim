## Current Snapshot (2026-03-07)

- Benchmark presets now resolve from a single frontend benchmark registry instead of duplicated catalog payloads.
- The remaining benchmark mismatch is semantic, not architectural:
  - frontend/public BL benchmark presets still reflect short-horizon rate-controlled definitions
  - validated Rust BL benchmarks are pressure-controlled and should become the benchmark-family physical reference
- The next workstream is benchmark modernization: exact benchmark-family semantics, generated sensitivities, benchmark-specific charts, and explicit reference policy.

## Legacy Cleanup

- Removed legacy Phase 1 / Phase 2 execution history from this live status file.
- Historical implementation detail remains available in git history and older benchmark/docs pages; this file now tracks only the current benchmark modernization state.
- Curated long-term options from the old backlog were reintroduced in `TODO.md` as a separate retained-options section, then classified into `Important Later` and `Nice To Have Only` with duplicates removed.

## Proposed End Objective (pending review)

Build a benchmark system with these properties:

- one logical source of truth for benchmark definitions
- exact Rust parity for benchmark-family base physics where applicable
- generated sensitivity variants instead of duplicated full-case payloads
- benchmark-specific chart layouts and defaults
- explicit analytical-vs-numerical reference policy
- readable multi-case comparison workflow in the frontend

## Proposed Benchmark Modernization Plan

### B0. Plan review and sign-off

Current state:
- pending user review before implementation starts

Goal:
- confirm the target objective, sensitivity scope, and chart policy before modifying benchmark semantics

### B1. Benchmark-family schema and ownership

Goal:
- define one benchmark registry contract that owns base physics, variants, reference policy, display defaults, and run policy

Key outcome:
- benchmark selector data and benchmark execution data come from the same source

### B2. Align BL families to exact Rust semantics

Goal:
- make BL benchmark families physically equivalent to the Rust benchmark builders

Key outcome:
- frontend BL benchmark means the same experiment as the validated Rust benchmark

### B3. Generate sensitivity variants from base families

Goal:
- support meaningful BL sensitivity studies without duplicating complete case payloads

Initial axes:
- grid refinement
- timestep refinement
- curated heterogeneity variants

Key outcome:
- one family definition can produce base and sensitivity runs deterministically

### B4. Multi-run benchmark execution model

Goal:
- execute one family plus selected variants and collect normalized comparison results

Key outcome:
- benchmark charts and summaries consume a stable multi-run result model

### B5. Explicit reference policy

Goal:
- define what counts as “reference” per benchmark family

Policy:
- homogeneous BL: analytical Buckley-Leverett reference
- heterogeneous BL: refined numerical reference
- depletion: depletion analytical reference where applicable

Key outcome:
- the UI no longer implies analytical equivalence where that claim is no longer valid

### B6. Benchmark-specific charts and defaults

Goal:
- replace one-size-fits-all chart defaults with benchmark-family-specific panel composition

Planned defaults:
- BL: water cut vs PVI with breakthrough markers, recovery/cumulative oil vs PVI, separate pressure diagnostics
- depletion: oil rate, cumulative oil/recovery, pressure/decline diagnostics

Styling policy:
- color identifies case/variant
- line style identifies quantity or reference type
- pressure and water-cut-first reading stay separate by default

### B7. Benchmark UI workflow

Goal:
- expose family selection, enabled sensitivities, and comparison/reference behavior without collapsing benchmark mode into the generic scenario builder

Key outcome:
- benchmark mode becomes a comparison tool rather than only a preset launcher

### B8. Regression coverage and docs

Goal:
- lock benchmark semantics, sensitivity generation, reference policy, and chart defaults with tests and synchronized docs

Key outcome:
- future benchmark edits cannot drift silently

## Risks / Design Constraints

- Pressure-controlled BL benchmarks are more correct for Rust parity but less immediately intuitive than fixed-rate storytelling.
- Exact benchmark semantics and user-facing display horizon should remain independent decisions.
- Heterogeneous BL sensitivity should not be presented as a strict analytical-comparison benchmark.
- Multi-case overlays will become unreadable if color encodes quantity instead of case; quantity should move to panel structure and line style.
- Benchmark mode may need a dedicated chart container rather than forcing all needs through the current single-run chart contract.

## Next Action After Review

- If the plan is accepted, start with `B1` and `B2` first:
  - lock the benchmark-family schema
  - align BL base families to exact Rust semantics
  - only then implement sensitivities and chart restructuring on top of that base
