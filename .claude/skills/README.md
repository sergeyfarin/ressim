# ResSim Skill Library

Task playbooks for how this project is debugged, extended and validated. The list of skills and
when to use each is in the root [`AGENTS.md`](../../AGENTS.md), which every agent loads; this
file only covers how to use them and how to keep them honest.

## Using them

Claude Code discovers `.claude/skills/*/SKILL.md` automatically and loads one when the task
matches its description. Other agents (Codex, Cursor) read `AGENTS.md`, which tells them
to open the matching file.

With smaller models, or for long tasks, be explicit:

1. Name the skill in the prompt: "Using the fim-solver-debug skill, reproduce the 20x20x3 water
   baseline."
2. One skill-sized task per session. Don't mix an engine change and a frontend change.
3. Ask for the validation command output verbatim at the end — not "tests pass".
4. For convergence experiments, require the agent to name its oracle and to classify missing or
   incomparable diagnostics as `INCONCLUSIVE`, never `REFUTED`.

## Where things go

- `AGENTS.md` — rules that apply to most tasks and are costly to miss. Always loaded, so keep it
  short.
- A skill — the procedure and the traps for one kind of task.
- `README.md` / `docs/` — product facts, measurements and history. A skill links to them rather
  than restating them.

## Maintenance rule

Skills state current facts (file sizes, live/legacy status, known gaps). When one drifts from
reality, fix it in the same PR that moved reality — the rule `docs/DOCUMENTATION_INDEX.md` applies
to docs. Don't add a skill for a one-off task; extend an existing one or write a doc.
