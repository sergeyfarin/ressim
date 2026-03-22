# Implementation Review — 2026-03-19

Historical review snapshot. Several findings recorded here have since been addressed and moved into delivered work, roadmap items, or updated docs.

This review records verified gaps between the scientific implementation, the UI metadata, and the documentation state as of 2026-03-19. Use it as historical context, not as the current source of truth.

## Scope

- Analytical helpers: Buckley-Leverett, sweep-efficiency, depletion
- Rust physics notes that materially affect model interpretation
- User-facing docs at the time: `README.md`, `TODO.md`, and `docs/*`

## Verified Findings

### 1. Depletion well-location sensitivity is overstated

The depletion analytical helper does not currently receive producer location. `calculateDepletionAnalyticalProduction()` infers Dietz-like shape behavior from reservoir geometry and aspect ratio alone, while the `dep_pss` scenario metadata describes center-vs-corner well location as an analytical sensitivity.

Implication:
- Simulation changes with producer location.
- Analytical depletion curves do not yet have a producer-position-aware Dietz mapping.
- Any UI language claiming that well-location variants update both simulation and analytical results is too strong.

Industry reference:
- Dietz (1965) pseudo-steady-state shape factors depend on drainage geometry and well location.
- Commercial reservoir simulators typically use explicit geometry/well placement or tabulated shape factors rather than inferring center/corner behavior from aspect ratio alone.

Recommended fix:
- Add explicit producer location or explicit shape-factor input to the depletion analytical adapter.
- Add regression tests proving center, edge, and corner cases produce distinct analytical curves.

### 2. Sweep recovery overlays are valid as first-order guidance, not rigorous prediction

The implemented recovery overlay combines Craig areal sweep, Dykstra-Parsons vertical sweep, and Buckley-Leverett displacement efficiency through a local-PVI approximation.

Implication:
- Good for qualitative trends and teaching.
- Not rigorous enough to present as a general-purpose volumetric sweep predictor for arbitrary patterns or strongly communicating layered systems.

Industry reference:
- Craig (1971): confined five-spot areal sweep correlations.
- Dykstra and Parsons (1950): layered, non-communicating vertical sweep.
- Stiles (1949): preferred next-step upgrade for layered displacement accounting.

Recommended fix:
- Keep current overlay with explicit caveats.
- Implement Stiles-style layer-by-layer recovery as the next analytical upgrade.

### 3. Three-phase docs and comments still contain contradictory sign/diagnostic statements

The code path uses gas potential consistent with `P_gas = P_oil + P_cog`, but parts of the documentation and an inline solver comment still stated the opposite sign. The docs also overstated phase-by-phase material-balance diagnostics.

Implication:
- The implementation notes were misleading even where the numerical path was unchanged.
- Future three-phase work is more error-prone if the convention is not stated consistently.

Industry reference:
- Standard non-wetting capillary convention for gas-oil systems uses gas pressure above oil pressure by `P_cog`.

Recommended fix:
- Keep one authoritative sign convention in docs and code comments.
- Record current three-phase correctness gaps explicitly: gas-oil capillary direction model, missing residual-oil-to-gas endpoint, and water-only material-balance diagnostic.

### 4. Capillary pressure capping was implemented but under-documented

The Brooks-Corey implementation caps capillary pressure at 20 times entry pressure to avoid runaway sponge behavior. This is numerically pragmatic but scientifically relevant.

Implication:
- Users exploring gravity-capillary balance can misread the plateau as physical rather than numerical protection.

Industry reference:
- Practical reservoir simulators often regularize capillary curves for numerical stability, but the cap/regularization must be documented when user interpretation depends on it.

Recommended fix:
- Document the cap and its rationale in the unit/physics docs.

### 5. Documentation drift existed in active product docs

Verified stale items before this review update:
- README still described the scenario redesign as in progress.
- README listed obsolete scenario keys and an old project-layout path.
- README advertised the saturation-profile chart as an active output even though it is currently commented out in `App.svelte`.
- `docs/P4_TWO_PHASE_BENCHMARKS.md` still referenced old waterflood scenario keys.
- `docs/DOCUMENTATION_INDEX.md` still described the pre-S1 scenario inventory.

Recommended fix:
- Keep the active docs aligned with the current UI, even when intermediate components remain in the tree.

## Recommended Refactoring Follow-Ups

### A. Output selection view-model extraction

`App.svelte` currently maintains parallel families of derived values for:
- selected reference result
- output profile data
- 3D output data
- active analytical inputs

This makes it easy for analytical adapters and chart wiring to diverge.

Suggested direction:
- Extract a typed output-selection/view-model helper that returns one authoritative output payload for charts, 3D view, and analytical helpers.

### B. Typed analytical adapter contracts

The depletion mismatch exists because scenario metadata says a variant affects analytical output, but the actual analytical adapter does not consume the required inputs.

Suggested direction:
- Introduce small adapter builders per analytical family.
- Add tests asserting that every `affectsAnalytical: true` sensitivity dimension changes at least one input consumed by the analytical adapter.

## Suggested Priority Order

1. Keep current docs truthful.
2. Fix the depletion analytical contract gap.
3. Unify output/adaptor plumbing so future sensitivity work cannot silently drift.
4. Upgrade sweep recovery from local-PVI approximation toward Stiles-style layered accounting.