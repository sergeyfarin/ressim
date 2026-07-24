# Copilot Instructions for ResSim

## Project Overview

Browser-based reservoir simulator: Rust/WASM core + Svelte 5 UI. Runs IMPES (implicit pressure, explicit saturation) on a 3D Cartesian grid and validates against classical analytical solutions (Buckley-Leverett, Dietz decline, Craig/Dykstra-Parsons sweep).

See `README.md` for full feature list and `docs/` for technical deep-dives.

**Workflow skills**: task-specific playbooks live in `.claude/skills/` (validation gates, engine changes, FIM debugging, frontend architecture, scenario authoring, OPM pipeline). Read the skill matching your task before working; index at `.claude/skills/README.md`.

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
- **Three.js pinned** to the exact version in `package.json` — do not upgrade casually; visualization behavior is version-sensitive.
- **WASM bindings** via `wasm-bindgen`; worker communication uses structured cloning only (no functions or class instances).
- **Python tooling** uses `uv` for commands, scripts, environments, and dependency management.
- **Package manager is pnpm** (`pnpm install`, `pnpm run dev`, `pnpm test`) — never npm/yarn.

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

**Testing caution**: full `cargo test` is NOT a valid gate — FIM/SPE1 tests can hang or dominate runtime. Use `bash scripts/validate-solver-coverage.sh {shared|fim|impes|all}` and the targeted commands in `.claude/skills/ressim-validation/SKILL.md`.

**CurveConfig / ChartSubPanel**: charts are driven by `CurveConfig[]` arrays. Curves grouped by `toggleGroupKey` into legend toggle buttons. `legendSection` / `legendSectionLabel` group buttons under collapsible section headers. Don't bypass this pattern.

**visibleCaseKeys**: `ReferenceComparisonChart` pre-filters curves by case visibility before passing to `ChartSubPanel`. `ChartSubPanel` has its own secondary per-panel toggle. These are independent layers.

## Best Practices

1. **Minimal changes** — only touch files directly related to the task.
2. **Preserve physics** — don't alter validated benchmark tolerances without explicit justification.
3. **No new dependencies** unless essential; use existing libraries.
4. **Worker safety** — state snapshots must be structured-cloneable. Svelte 5 `$state` values are
   deep Proxies and structured cloning rejects them (`DataCloneError: [object Array] could not be
   cloned`), so never post a store value by reference. All worker sends go through
   `RuntimeStoreImpl.#post()`, which applies `$state.snapshot()`; don't call `simWorker.postMessage`
   directly. Payload builders must still copy arrays they pass through — the snapshot is a backstop,
   not a licence to leak proxies into `buildCreatePayload`.
5. **Avoid over-engineering** — don't add abstractions, error handling, or configurability beyond what the task requires.

## Known Constraints

- Three.js version pinned — do not upgrade.
- WASM requires `wasm32-unknown-unknown` target.
- Worker ↔ UI communication: structured cloning only.
- **No runtime import cycles under `src/`.** A value-level cycle makes whichever module the bundler
  enters first read the other's top-level `const`s from their temporal dead zone, throwing
  `Cannot access 'X' before initialization` at load and blanking the app. Gated by
  `pnpm run check:cycles` (part of `validate` / `validate:product`). Break a cycle by making the edge
  `import type` or moving the shared value into a leaf module — not by reordering declarations.

## Working Style

**TODO discipline**: while working on any task, note discoveries in `TODO.md` — bugs, gaps, bad practices, physics concerns, UX improvements. When work is complete, mark finished items done. Keeping TODO current is part of the definition of done.

**Prefer fundamental fixes** over quick patches. Address root causes; avoid workaround accumulation.

**Baseline discipline**: never describe a solver/runtime/convergence number as the current baseline unless it was reproduced on a committed revision and the exact replay command is recorded next to it. When a diagnostic result matters for future comparison:
- record the exact commit hash or tag
- record the exact command line used
- copy the key summary output verbatim enough to reconstruct the claimed baseline
- treat results from temporary experiments, dirty worktrees, or partially reverted states as provisional until rerun on the intended committed state
- when replacing an older baseline in docs, explicitly say which older baseline was superseded and why

**Promotion discipline**: before promoting a convergence change into docs as the new baseline, rerun the agreed validation shortlist on the final post-revert/post-cleanup tree, not on an intermediate experiment commit. If a replay is too expensive to rerun broadly, mark the baseline as provisional instead of presenting it as settled.

**Experimental-verdict discipline**: a test suite passing proves the implementation satisfies
those tests; it does not by itself prove or refute a convergence hypothesis. Before using two
solver paths as independent checks, prove that both expose the same backend-neutral observables
(initial/RHS norm, final full-system residual norm, reduction, finite correction, and the same
row partition). A missing diagnostic, backend-specific success flag, or wrapper/reduced-system
norm mismatch makes the experiment `INCONCLUSIVE`. Never label a physics/update hypothesis
`REFUTED` because a second backend aborted before its correction quality was measured.

When porting OPM behavior, distinguish a narrow component probe from a coherent OPM lifecycle.
State storage, property endpoint extension, accumulation, primary-variable adaptation, well
unknowns, and linear acceptance can interact. A partial probe may establish a local mechanism,
but it cannot refute the complete mechanism while named coupled semantics are absent. Keep
commits reversible and causally scoped, but use dependency-aware bundles when the source
implementation is intrinsically coupled.
