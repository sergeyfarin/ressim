# Agent Instructions — ResSim

Project instructions live in `.github/copilot-instructions.md` (also loaded via `CLAUDE.md`). Read that first.

Then, before working, read the skill file matching your task from `.claude/skills/` and follow it exactly:

- `ressim-validation/SKILL.md` — which tests to run for which change (read for EVERY task; full `cargo test` is not a valid gate here)
- `engine-physics-change/SKILL.md` — any Rust core change
- `fim-solver-debug/SKILL.md` — FIM solver convergence work
- `frontend-architecture/SKILL.md` — any Svelte/TS change
- `add-scenario/SKILL.md` — adding catalog cases/sensitivities
- `opm-reference-pipeline/SKILL.md` — OPM Flow reference data

Index and usage notes: `.claude/skills/README.md`. Doc authority map: `docs/DOCUMENTATION_INDEX.md`.
