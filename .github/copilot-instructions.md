# Copilot Instructions for ResSim

## Project Overview

Browser-based reservoir simulator: Rust/WASM core + Svelte 5 UI. Runs IMPES (implicit pressure, explicit saturation) on a 3D Cartesian grid and validates against classical analytical solutions (Buckley-Leverett, Dietz decline, Craig/Dykstra-Parsons sweep).

See `README.md` for full feature list and `docs/` for technical deep-dives.

## Architecture

| Layer | Location | Role |
|-------|----------|------|
| Rust/WASM core | `src/lib/ressim/src/` | Physics: pressure solver (PCG), saturation transport, wells, relperm, capillary |
| Analytical | `src/lib/analytical/` | Reference solutions for BL, depletion, sweep efficiency |
| Catalog | `src/lib/catalog/` | Scenario definitions, preset runtime, benchmark cases |
| Charts | `src/lib/charts/` | Rate/comparison charts (Chart.js), chart panel model |
| UI | `src/lib/ui/` | Mode panels, sections, controls, feedback components |
| Workers | `src/lib/workers/` | Web Worker bridge — keeps UI responsive during simulation |
| Visualization | `src/lib/visualization/` | Three.js 3D property display |
| App | `src/App.svelte` | Root: orchestrates scenario state, worker lifecycle, playback |

## Technology Choices That Affect Coding

- **Svelte 5** — use runes syntax only (`$state`, `$derived`, `$derived.by`, `$effect`, `$props`, `$bindable`). No Svelte 4 legacy stores or `export let`.
- **TypeScript** throughout; strict mode.
- **Tailwind CSS 4.x** — utility classes, no custom CSS unless unavoidable.
- **Chart.js 4.x** — charts managed via `ChartSubPanel.svelte`; don't bypass the panel abstraction.
- **Three.js pinned** at 0.183.2 — do not upgrade casually; visualization behavior is version-sensitive.
- **WASM bindings** via `wasm-bindgen`; worker communication uses structured cloning only (no functions or class instances).

## Coding Conventions

### Rust
- Idiomatic Rust: explicit error handling, no `unwrap()` in library code.
- Keep physics in dedicated modules (`relperm.rs`, `capillary.rs`, etc.).
- `///` doc comments on public API.
- Format with `cargo fmt`.

### Svelte / TypeScript
- Svelte 5 runes only — no legacy syntax.
- Components should be single-purpose; keep state as high as needed, no higher.
- Prefer `$derived.by` for multi-step derivations over chained `$derived`.

## Key Concepts

**Simulation loop**: user configures scenario → worker creates Rust simulator → `step()` solves pressure, advances saturation, updates wells → worker posts state snapshots to UI.

**Benchmarking**: physics validated against analytical solutions via Rust `#[test]` functions in `src/lib/ressim/src/lib.rs`. Don't change benchmark tolerances without justification.

**CurveConfig / ChartSubPanel**: charts are driven by `CurveConfig[]` arrays. Curves grouped by `toggleGroupKey` into legend toggle buttons. `legendSection` / `legendSectionLabel` group buttons under collapsible section headers. Don't bypass this pattern.

**visibleCaseKeys**: `ReferenceComparisonChart` pre-filters curves by case visibility before passing to `ChartSubPanel`. `ChartSubPanel` has its own secondary per-panel toggle. These are independent layers.

## Best Practices

1. **Minimal changes** — only touch files directly related to the task.
2. **Preserve physics** — don't alter validated benchmark tolerances without explicit justification.
3. **No new dependencies** unless essential; use existing libraries.
4. **Worker safety** — state snapshots must be structured-cloneable.
5. **Avoid over-engineering** — don't add abstractions, error handling, or configurability beyond what the task requires.

## Known Constraints

- Three.js version pinned — do not upgrade.
- WASM requires `wasm32-unknown-unknown` target.
- Worker ↔ UI communication: structured cloning only.

## Working Style

**TODO discipline**: while working on any task, note discoveries in `TODO.md` — bugs, gaps, bad practices, physics concerns, UX improvements. When work is complete, mark finished items done. Keeping TODO current is part of the definition of done.

**Prefer fundamental fixes** over quick patches. Address root causes; avoid workaround accumulation.
