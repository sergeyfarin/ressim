# ResSim Skill Library

Six core skills that encode how this project is debugged, extended, and validated. They exist so that smaller models (Sonnet-class) and engineers new to the project can work at the standard the project was built to. Written 2026-07-02 as part of the maintainer handoff.

## The skills

| Skill | Use when |
|---|---|
| `ressim-validation` | Before claiming any change done; choosing which test gate applies; separating green code gates from a valid experimental oracle. **Read this one first for every task.** |
| `engine-physics-change` | Any change under `src/lib/ressim/src/` (units, test placement, dual-implementation traps, tolerances) |
| `fim-solver-debug` | FIM convergence/Newton/timestep work; backend-neutral oracle checks; dependency-aware OPM probes; verdict, baseline, and promotion discipline |
| `frontend-architecture` | Any Svelte/TS change; chart stack layer map; which legacy files not to grow |
| `add-scenario` | Adding cases, sensitivities, analytical overlays to the catalog |
| `opm-reference-pipeline` | OPM Flow decks/artifacts/ground-truth comparison |

Companion docs: `docs/CASE_LIBRARY_ROADMAP.md` (where to find new cases), `docs/DOCUMENTATION_INDEX.md` (which docs are authoritative).

## How to use with Claude Code (any model, including Sonnet)

Skills in `.claude/skills/` are discovered automatically; Claude invokes them by name when the task matches the description. To be explicit — which is recommended with Sonnet-class models — name the skill in your prompt:

> Using the fim-solver-debug skill, reproduce the 20x20x3 water baseline and investigate the remaining retry ladder.

Effective session pattern for cheaper models:

1. Start the prompt with the goal AND the constraint: "Follow .claude/skills/ressim-validation/SKILL.md before declaring done."
2. One skill-sized task per session. Don't mix an engine change and a frontend change.
3. Ask for the validation command output verbatim at the end — don't accept "tests pass".
4. For convergence experiments, require the agent to name the oracle and classify missing or
   incomparable diagnostics as `INCONCLUSIVE`, never `REFUTED`.

## How to use with Codex / ChatGPT / other agents

These tools do not auto-load `.claude/skills/`. Two options:

- **AGENTS.md (already set up):** the repo root `AGENTS.md` points agents at this directory and the per-task skill files. Codex reads `AGENTS.md` automatically.
- **Manual:** paste the relevant `SKILL.md` into the prompt, or instruct: "Before working, read `.claude/skills/README.md` and the skill file matching your task; follow it exactly."

## Maintenance rule

Skills state current facts (file sizes, live/legacy status, known gaps). When one drifts from reality, fix it in the same PR that moved reality — the same rule `docs/DOCUMENTATION_INDEX.md` applies to docs. Do not add new skills for one-off tasks; extend an existing one or write a doc instead.
